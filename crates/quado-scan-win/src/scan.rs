// SPDX-License-Identifier: GPL-3.0-or-later
//! Scansione di un volume o di una cartella.
//!
//! I thread di rayon elencano le cartelle e mandano un lotto per cartella a un solo costruttore,
//! che aggiunge i nodi all'albero. Un lotto parte prima che i lavori delle sottocartelle vengano
//! avviati, quindi il costruttore riceve sempre il genitore prima dei figli.
//!
//! Il core non produce frasi: problemi e stati sono codici che la UI traduce.

use std::cell::RefCell;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{Sender, channel};
use std::time::{Duration, Instant};

use quado_model::{Balance, DirExtra, Flags, IdEntry, Links, Metadata, NONE, NodeData, Space, Tree, links};

use crate::win::{self, EnumBuffer, NtfsVolumeData, UsnJournal};

const ATTR_REPARSE: u32 = 0x0400;
const ATTR_OFFLINE: u32 = 0x1000;
const ATTR_RECALL_ON_OPEN: u32 = 0x0004_0000;
const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
const TAG_NAME_SURROGATE: u32 = 0x2000_0000;

fn is_cloud_tag(tag: u32) -> bool {
    tag & 0xFFFF_0FFF == 0x9000_001A
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Thread di enumerazione; 0 = quanti processori logici.
    pub threads: usize,
    /// Elenca anche le cartelle cloud con `RECALL_ON_DATA_ACCESS` (può scaricarne l'elenco).
    pub list_cloud_dirs: bool,
    /// Corregge l'allocata dei file compressi WOF leggendo `FileStandardInfo`.
    pub fix_wof: bool,
    /// Solo test, per iniettare un errore che la prova deve vedere: non deduplica gli hard link.
    pub inject_no_dedup: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { threads: 0, list_cloud_dirs: false, fix_wof: true, inject_no_dedup: false }
    }
}

/// Totale dal vivo di un elemento di primo livello.
pub struct TopEntry {
    pub name: Vec<u16>,
    pub alloc: AtomicU64,
}

/// Avanzamento e annullamento, condivisi con chi mostra la scansione.
#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub alloc: AtomicU64,
    cancel: AtomicBool,
    top: OnceLock<Vec<TopEntry>>,
}

impl Progress {
    pub fn cancel(&self) {
        self.cancel.store(true, Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Relaxed)
    }

    /// Elementi di primo livello, disponibili appena elencata la radice.
    pub fn top(&self) -> Option<&[TopEntry]> {
        self.top.get().map(Vec::as_slice)
    }

    /// Avanzamento già a metà, per mostrare la schermata di scansione nei test e nelle immagini.
    pub fn preset(files: u64, dirs: u64, alloc: u64, top: Vec<(Vec<u16>, u64)>) -> Self {
        let p = Self { files: AtomicU64::new(files), dirs: AtomicU64::new(dirs), alloc: AtomicU64::new(alloc), ..Self::default() };
        let _ = p.top.set(top.into_iter().map(|(name, a)| TopEntry { name, alloc: AtomicU64::new(a) }).collect());
        p
    }
}

/// Perché una cartella non è stata elencata, o non tutta. Codici, non frasi.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Problem {
    AccessDenied,
    InUse,
    /// Sparita durante la scansione.
    NotFound,
    /// Elenco interrotto a metà.
    Interrupted(u32),
    /// Scansione annullata prima di arrivare qui.
    Cancelled,
    Other(u32),
}

impl Problem {
    fn from_open(code: u32) -> Self {
        match code {
            5 => Self::AccessDenied,
            32 | 33 => Self::InUse,
            2 | 3 => Self::NotFound,
            c => Self::Other(c),
        }
    }

    /// Inverso di [`Problem::code`] (protocollo dell'helper): `Interrupted` torna come `Other`.
    pub fn from_code(code: u32) -> Self {
        match code {
            1223 => Self::Cancelled,
            c => Self::from_open(c),
        }
    }

    /// Codice di errore Windows, se c'è.
    pub fn code(self) -> u32 {
        match self {
            Self::AccessDenied => 5,
            Self::InUse => 32,
            Self::NotFound => 2,
            Self::Interrupted(c) | Self::Other(c) => c,
            Self::Cancelled => 1223,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub files: u64,
    pub dirs: u64,
    /// File con l'allocata corretta (WOF).
    pub alloc_fixed: u64,
    pub cloud_files: u64,
    pub cloud_dirs_not_listed: u64,
    /// Junction, symlink e punti di montaggio non seguiti.
    pub not_followed: u64,
    pub denied: u64,
    pub other_problems: u64,
    pub id_entries: u64,
    pub enumeration: Duration,
    pub finish: Duration,
    /// Tempi per fase, solo per le misure: preparazione prima dell'enumerazione (radice, dati
    /// NTFS, memoria riservata, thread), lavoro del costruttore senza le attese sul canale, poi le fasi
    /// della chiusura e le letture del volume dopo la chiusura.
    pub setup: Duration,
    pub builder_busy: Duration,
    pub seal: Duration,
    pub links: Duration,
    pub aggregate: Duration,
    pub tail: Duration,
    /// Tempo dei thread di enumerazione per chiamata, sommato su tutti i thread (solo misure): apertura
    /// delle cartelle, indice della cartella (`FileStandardInfo`), elenco, chiusura, file WOF e sparse.
    pub thread_open: Duration,
    pub thread_index: Duration,
    pub thread_enumerate: Duration,
    pub thread_close: Duration,
    pub thread_wof: Duration,
}

/// Fatti del volume, se la radice della scansione è la radice di un volume.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VolumeFacts {
    pub ntfs: Option<NtfsVolumeData>,
    pub usn: Option<UsnJournal>,
    pub space_start: Space,
    pub space_end: Space,
}

pub struct ScanResult {
    pub tree: Tree,
    pub links: Links,
    pub problems: Vec<(u32, Problem)>,
    pub stats: Stats,
    pub volume: Option<VolumeFacts>,
    pub balance: Option<Balance>,
    pub cancelled: bool,
}

/// La radice non si può aprire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootError(pub u32);

struct Job {
    id: u32,
    path: Vec<u16>,
    top: u32,
}

#[derive(Clone, Copy)]
struct RawEntry {
    name_off: u32,
    name_len: u16,
    flags: Flags,
    attrs: u32,
    logical: u64,
    alloc: u64,
    mtime: i64,
    id: u128,
    child_job: u32,
}

#[derive(Default)]
struct Batch {
    job: u32,
    open_error: Option<u32>,
    cancelled: bool,
    interrupted: Option<u32>,
    index_alloc: u64,
    names: Vec<u16>,
    entries: Vec<RawEntry>,
}

struct Ctx<'a> {
    tx: Sender<Batch>,
    next_job: AtomicU32,
    progress: &'a Progress,
    opts: Options,
    /// Nanosecondi per chiamata, sommati sui thread: apertura, indice, elenco, chiusura, WOF (solo misure).
    nanos: [AtomicU64; 5],
}

/// Aggiunge a `slot` il tempo da `since`, in nanosecondi.
fn add_nanos(slot: &AtomicU64, since: Instant) {
    slot.fetch_add(since.elapsed().as_nanos() as u64, Relaxed);
}

thread_local! {
    static BUF: RefCell<EnumBuffer> = RefCell::new(EnumBuffer::default());
}

fn is_volume_root(p: &[u16]) -> bool {
    p.len() == 3 && p[1] == b':' as u16 && p[2] == b'\\' as u16
}

/// Scansiona `root` (per esempio `C:\` o una cartella). Blocca fino alla fine o all'annullamento.
pub fn scan(root: &str, opts: Options, progress: &Progress) -> Result<ScanResult, RootError> {
    let setup_start = Instant::now();
    let root_w: Vec<u16> = root.encode_utf16().collect();
    win::expose_placeholders();
    let root_handle = win::open_dir(&root_w).map_err(RootError)?;
    let volume_root = is_volume_root(&root_w);
    let ntfs = win::ntfs_volume_data(&root_handle).ok();
    let space_start = if volume_root { win::disk_space(&root_w).ok() } else { None };

    // Memoria riservata una volta: i record della MFT sono un limite superiore dei nomi.
    let cap = match (volume_root, ntfs) {
        (true, Some(n)) => {
            let r = n.mft_records() as usize;
            r + r / 20
        }
        _ => 1 << 16,
    };
    let mut tree = Tree::with_root(&root_w, cap);
    let mut ids: Vec<IdEntry> = Vec::with_capacity(if volume_root { cap } else { 1 << 16 });

    let started = Instant::now();
    let (tx, rx) = channel::<Batch>();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(opts.threads)
        .thread_name(|i| format!("quado-scan-{i}"))
        .build()
        .expect("pool di thread");

    let mut problems: Vec<(u32, Problem)> = Vec::new();
    let mut stats = Stats { setup: started - setup_start, ..Stats::default() };
    let root_job = Job { id: 0, path: root_w.clone(), top: NONE };
    let mut thread_times = [Duration::ZERO; 5];
    let (job_node, reported) = std::thread::scope(|ts| {
        let builder = ts.spawn(|| build(rx, &mut tree, &mut ids, &mut problems, &mut stats));
        {
            let ctx = Ctx { tx, next_job: AtomicU32::new(1), progress, opts, nanos: Default::default() };
            pool.scope(|s| {
                let ctx = &ctx;
                s.spawn(move |s| run_job(s, ctx, root_job));
            });
            let d = |k: usize| Duration::from_nanos(ctx.nanos[k].load(Relaxed));
            thread_times = [d(0), d(1), d(2), d(3), d(4)];
            // Qui `ctx` e il suo mittente si chiudono: il costruttore finisce dopo l'ultimo lotto.
        }
        builder.join().expect("costruttore")
    });
    stats.enumeration = started.elapsed();
    [stats.thread_open, stats.thread_index, stats.thread_enumerate, stats.thread_close, stats.thread_wof] = thread_times;

    // Cartelle avviate ma mai elencate: scansione annullata.
    let cancelled = progress.is_cancelled();
    for (job, &node) in job_node.iter().enumerate() {
        if node != NONE && !reported.get(job).copied().unwrap_or(false) {
            tree.add_flags(node, Flags::PARTIAL);
            problems.push((node, Problem::Cancelled));
        }
    }
    let space_end = if volume_root { win::disk_space(&root_w).ok() } else { None };

    let finish_start = Instant::now();
    tree.seal();
    stats.seal = finish_start.elapsed();
    stats.id_entries = ids.len() as u64;
    let t = Instant::now();
    let links = links::resolve(ids, &mut tree, opts.inject_no_dedup);
    stats.links = t.elapsed();
    let t = Instant::now();
    tree.aggregate();
    stats.aggregate = t.elapsed();
    stats.alloc_fixed = ctx_alloc_fixed(&tree);
    stats.finish = finish_start.elapsed();
    let tail_start = Instant::now();

    let usn = win::usn_journal(&root_handle).ok();
    let volume = match (space_start, space_end) {
        (Some(s0), Some(s1)) => Some(VolumeFacts { ntfs, usn, space_start: s0, space_end: s1 }),
        _ => None,
    };
    let balance = volume.map(|v| {
        let m = Metadata {
            mft: v.ntfs.map(|n| n.mft_valid_data_length),
            dir_indexes: tree.dir_index_total(),
            usn_journal: v.usn.map(|u| u.active_bytes),
            reserved: win::reserved_space(&root_w).ok(),
        };
        Balance::compute(v.space_start, v.space_end, tree.alloc(Tree::ROOT), m, links.extra_alloc)
    });
    stats.tail = tail_start.elapsed();
    Ok(ScanResult { tree, links, problems, stats, volume, balance, cancelled })
}

fn ctx_alloc_fixed(tree: &Tree) -> u64 {
    (0..tree.len() as u32).filter(|&i| tree.flags(i).has(Flags::ALLOC_FIXED)).count() as u64
}

fn run_job<'s>(s: &rayon::Scope<'s>, ctx: &'s Ctx<'s>, job: Job) {
    let mut batch = Batch { job: job.id, ..Default::default() };
    if ctx.progress.is_cancelled() {
        batch.cancelled = true;
        let _ = ctx.tx.send(batch);
        return;
    }
    let t = Instant::now();
    let opened = win::open_dir(&job.path);
    add_nanos(&ctx.nanos[0], t);
    let h = match opened {
        Ok(h) => h,
        Err(e) => {
            batch.open_error = Some(e);
            let _ = ctx.tx.send(batch);
            return;
        }
    };
    let t = Instant::now();
    batch.index_alloc = win::standard_info(&h).map(|s| s.alloc).unwrap_or(0);
    add_nanos(&ctx.nanos[1], t);
    let t = Instant::now();
    let mut children: Vec<Job> = Vec::new();
    let mut top_names: Vec<Vec<u16>> = Vec::new();
    let opts = ctx.opts;
    let result = BUF.with(|b| {
        let mut buf = b.borrow_mut();
        win::enumerate(&h, &mut buf, |e| {
            let mut flags = Flags::default();
            let is_dir = e.is_dir();
            if is_dir {
                flags |= Flags::DIR;
            }
            let reparse = e.attrs & ATTR_REPARSE != 0;
            if reparse {
                flags |= Flags::REPARSE;
                if e.reparse_tag & TAG_NAME_SURROGATE != 0 {
                    flags |= Flags::NOT_FOLLOWED;
                }
            }
            let cloud = e.attrs & (ATTR_RECALL_ON_DATA_ACCESS | ATTR_RECALL_ON_OPEN) != 0 || (reparse && is_cloud_tag(e.reparse_tag));
            if cloud {
                flags |= Flags::CLOUD;
            }
            let mut child_job = NONE;
            if is_dir && !flags.has(Flags::NOT_FOLLOWED) {
                if e.attrs & ATTR_RECALL_ON_DATA_ACCESS != 0 && !opts.list_cloud_dirs {
                    flags |= Flags::CLOUD_NOT_LISTED;
                } else {
                    child_job = ctx.next_job.fetch_add(1, Relaxed);
                    let top = if job.id == 0 {
                        top_names.push(e.name.to_vec());
                        (top_names.len() - 1) as u32
                    } else {
                        job.top
                    };
                    children.push(Job { id: child_job, path: win::join(&job.path, e.name), top });
                }
            }
            let name_off = batch.names.len() as u32;
            batch.names.extend_from_slice(e.name);
            batch.entries.push(RawEntry {
                name_off,
                name_len: e.name.len() as u16,
                flags,
                attrs: e.attrs,
                logical: e.logical,
                alloc: e.alloc,
                mtime: e.mtime,
                id: e.file_id,
                child_job,
            });
        })
    });
    add_nanos(&ctx.nanos[2], t);
    if let Err(code) = result {
        batch.interrupted = Some(code);
    }
    let t = Instant::now();
    drop(h);
    add_nanos(&ctx.nanos[3], t);
    let t = Instant::now();

    // File compressi WOF: la voce di cartella riporta allocata 0. Lo stesso vale per i file
    // sparse, la cui voce può restare a 0 anche dopo la scrittura (misurato il 13/10/2026):
    // si legge `FileStandardInfo` anche per loro. Sono esclusi solo i file offline o cloud.
    if opts.fix_wof {
        for e in batch.entries.iter_mut() {
            let needs = !e.flags.has(Flags::DIR)
                && e.logical > 0
                && e.alloc == 0
                && e.attrs & (ATTR_OFFLINE | ATTR_RECALL_ON_OPEN | ATTR_RECALL_ON_DATA_ACCESS) == 0
                && !e.flags.has(Flags::CLOUD);
            if needs {
                let name = &batch.names[e.name_off as usize..e.name_off as usize + e.name_len as usize];
                if let Ok(fh) = win::open_attributes(&win::join(&job.path, name))
                    && let Ok(si) = win::standard_info(&fh)
                {
                    e.alloc = si.alloc;
                    e.flags |= Flags::ALLOC_FIXED;
                }
            }
        }
    }
    add_nanos(&ctx.nanos[4], t);

    let (mut files, mut alloc) = (0u64, 0u64);
    for e in &batch.entries {
        if !e.flags.has(Flags::DIR) {
            files += 1;
            alloc += e.alloc;
        }
    }
    let p = ctx.progress;
    p.files.fetch_add(files, Relaxed);
    p.dirs.fetch_add(1, Relaxed);
    p.alloc.fetch_add(alloc, Relaxed);
    if job.id == 0 {
        let _ = p.top.set(top_names.into_iter().map(|name| TopEntry { name, alloc: AtomicU64::new(0) }).collect());
    } else if job.top != NONE
        && let Some(t) = p.top().and_then(|t| t.get(job.top as usize))
    {
        t.alloc.fetch_add(alloc, Relaxed);
    }

    // Il lotto parte prima dei figli: il costruttore conosce già i loro nodi quando arrivano.
    let _ = ctx.tx.send(batch);
    for c in children {
        s.spawn(move |s| run_job(s, ctx, c));
    }
}

/// Costruttore: unico scrittore dell'albero. Restituisce, per ogni lavoro, il nodo e se è arrivato.
fn build(
    rx: std::sync::mpsc::Receiver<Batch>,
    tree: &mut Tree,
    ids: &mut Vec<IdEntry>,
    problems: &mut Vec<(u32, Problem)>,
    stats: &mut Stats,
) -> (Vec<u32>, Vec<bool>) {
    let mut job_node: Vec<u32> = vec![Tree::ROOT];
    let mut reported: Vec<bool> = vec![false];
    for b in rx {
        let busy = Instant::now();
        let job = b.job as usize;
        if reported.len() <= job {
            reported.resize(job + 1, false);
        }
        reported[job] = true;
        let node = job_node[job];
        if b.cancelled {
            reported[job] = false;
            continue;
        }
        stats.dirs += 1;
        if let Some(code) = b.open_error {
            let p = Problem::from_open(code);
            tree.add_flags(node, if p == Problem::AccessDenied { Flags::DENIED } else { Flags::ERROR });
            if p == Problem::AccessDenied {
                stats.denied += 1;
            } else {
                stats.other_problems += 1;
            }
            tree.set_dir_extra(DirExtra { node, index_alloc: 0, error: code });
            problems.push((node, p));
            continue;
        }
        tree.set_dir_extra(DirExtra { node, index_alloc: b.index_alloc, error: b.interrupted.unwrap_or(0) });
        if let Some(code) = b.interrupted {
            tree.add_flags(node, Flags::PARTIAL);
            problems.push((node, Problem::Interrupted(code)));
            stats.other_problems += 1;
        }
        for e in &b.entries {
            let name = &b.names[e.name_off as usize..e.name_off as usize + e.name_len as usize];
            let n = tree.push(node, name, NodeData { logical: e.logical, alloc: e.alloc, mtime: e.mtime, attrs: e.attrs, flags: e.flags });
            if e.flags.has(Flags::DIR) {
                if e.flags.has(Flags::NOT_FOLLOWED) {
                    stats.not_followed += 1;
                }
                if e.flags.has(Flags::CLOUD_NOT_LISTED) {
                    stats.cloud_dirs_not_listed += 1;
                }
            } else {
                stats.files += 1;
                if e.flags.has(Flags::CLOUD) {
                    stats.cloud_files += 1;
                }
                if e.flags.has(Flags::NOT_FOLLOWED) {
                    stats.not_followed += 1;
                }
                // Identità 0: file system senza ID dei file (exFAT, FAT32), quindi senza hard link.
                if e.id != 0 {
                    ids.push(IdEntry::new(e.id, n));
                }
            }
            if e.child_job != NONE {
                let cj = e.child_job as usize;
                if job_node.len() <= cj {
                    job_node.resize(cj + 1, NONE);
                }
                job_node[cj] = n;
            }
        }
        stats.builder_busy += busy.elapsed();
    }
    (job_node, reported)
}
