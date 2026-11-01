// SPDX-License-Identifier: GPL-3.0-or-later
//! Simulazione obbligatoria prima di ogni spostamento nel Cestino (03 F-AC-2, 05 D7): rilegge gli
//! attributi, calcola la stima (03 §6.3) ed esegue i controlli preventivi, senza toccare niente. Produce
//! una [`Simulazione`], da cui solo `plan::conferma` costruisce un piano.
//!
//! Ordine dei controlli, per ogni elemento non fisso: attributi e visita delle cartelle (stima, campione
//! di Restart Manager, percorsi di 260 caratteri o più), volume e Cestino (F-AC-1, F-AC-5, F-AC-7), Restart Manager (F-AC-8), riferimenti
//! (F-AC-9), cartelle di sincronizzazione (R10) e per ultimi gli elementi protetti (F-AC-4), perché
//! l'eccezione di 03 §6.1 vale solo se tutti gli altri controlli dell'elemento sono passati.
//!
//! Gli elementi fissi (versione in uso, versione tenuta, copia che resta) non si leggono: compaiono con i
//! contatori a zero e `directory` falso, senza blocchi, e non entrano in stima, capacità e piano.
//!
//! Le letture lente vanno su [`Probe::threads`] thread (8 con `WinProbe`, scelta misurata il 26/10/2026,
//! vedi `win_probe::THREADS`): le cartelle di tutti gli elementi da una coda comune, e Restart Manager
//! su più elementi insieme. Il risultato non dipende dai thread: la visita si ricompone nell'ordine di una
//! lettura su un thread solo, quindi stima, blocchi e campione di Restart Manager sono gli stessi.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};

use quado_model::intervention::{ProtectedKind, ProtectedRoot, Reference, ReferenceReport};
use quado_model::record::{CheckKind, Record, State};

use crate::estimate::{self, LinkTable};
use crate::plan::{Block, ItemExtra, ManualCheck, SimItem, Simulazione, Snapshot};
use crate::probe::{Attrs, Entry, ListError, Probe, VolumeBin};

/// Perché un elemento compare nella conferma ma non si può scegliere (04 §6.6, schermata Intervento).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FixedReason {
    /// Versione in uso (CapCut): "esclusa".
    InUseVersion,
    /// Versione precedente tenuta per default (CapCut): "tenuta".
    KeptVersion,
    /// Copia che resta tra le copie identiche (F-DL-4).
    KeptCopy,
}

/// Un elemento proposto: file o cartella, percorso assoluto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestItem {
    pub path: String,
    /// `Some` se l'elemento si mostra ma non si sposta: non entra mai nel piano né nella stima.
    pub fixed: Option<FixedReason>,
}

/// Cosa simulare: gli elementi di un record (regola o funzione della lente), con la scheda della
/// raccomandazione e le radici protette del volume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub items: Vec<RequestItem>,
    /// Scheda della raccomandazione: stato, azione, controlli (automatici e manuali), eccezione di F-AC-4.
    pub record: Record,
    /// Radici protette (F-AC-4), da `quado_kb::protection`.
    pub protected: Vec<ProtectedRoot>,
}

/// Avanzamento della simulazione, letto dall'interfaccia; `cancel` la ferma.
#[derive(Debug, Default)]
pub struct SimProgress {
    /// File e cartelle letti finora.
    pub done: AtomicU64,
    pub cancel: AtomicBool,
}

/// La simulazione è stata annullata: nessun piano si può costruire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cancelled;

/// Chiavi dei controlli automatici che la simulazione esegue (05 D6: `controllo-<nome>`).
pub(crate) const CHECK_RESTART_MANAGER: &str = "controllo-restart-manager";
pub(crate) const CHECK_REFERENCES: &str = "controllo-riferimenti";
pub(crate) const CHECK_CAPACITY: &str = "controllo-capacita-cestino";

/// File registrati con Restart Manager per una cartella oltre agli eseguibili e alle DLL: i modificati
/// più di recente (05 D7).
pub(crate) const RM_SAMPLE: usize = 1000;

/// `ERROR_FILE_NOT_FOUND` e `ERROR_PATH_NOT_FOUND`: l'elemento non c'è.
pub(crate) fn is_missing(code: u32) -> bool {
    code == 2 || code == 3
}

// ---------------------------------------------------------------------------------------------
// Percorsi

/// Percorso nella forma di Win32, con le maiuscole com'erano: senza `\\?\` (`\\?\UNC\` torna `\\`), con
/// `\` al posto di `/`, senza `\` finale salvo la radice di un'unità (`C:\`).
pub(crate) fn canonical(path: &str) -> String {
    let p = path.replace('/', "\\");
    let mut p = match p.strip_prefix("\\\\?\\") {
        Some(rest) => match rest.get(..4) {
            Some(unc) if unc.eq_ignore_ascii_case("unc\\") => format!("\\\\{}", &rest[4..]),
            _ => rest.to_owned(),
        },
        None => p,
    };
    while p.len() > 1 && p.ends_with('\\') && !is_drive_root(&p) {
        p.pop();
    }
    p
}

/// Percorso normalizzato per i confronti: [`canonical`] in minuscolo, anche fuori dall'ASCII.
pub(crate) fn norm(path: &str) -> String {
    canonical(path).to_lowercase()
}

/// Lunghezza da cui la shell non sposta più nel Cestino: con un percorso di 260 caratteri UTF-16 o più
/// (`MAX_PATH` con il terminatore) chiede subito di eliminare per sempre (prove su VHDX dell'agente B,
/// 14/10/2026).
pub(crate) const MAX_PATH: usize = 260;

/// Il percorso, nella forma di Win32, ha almeno [`MAX_PATH`] caratteri UTF-16.
pub(crate) fn too_long(path: &str) -> bool {
    canonical(path).encode_utf16().count() >= MAX_PATH
}

/// `c:\`.
fn is_drive_root(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() == 3 && b[1] == b':' && b[2] == b'\\'
}

/// `a` sta strettamente sotto `b` (percorsi normalizzati).
pub(crate) fn is_under(a: &str, b: &str) -> bool {
    a.len() > b.len() && a.starts_with(b) && (b.ends_with('\\') || a.as_bytes()[b.len()] == b'\\')
}

/// `a` coincide con `b` o sta sotto (percorsi normalizzati).
pub(crate) fn is_within(a: &str, b: &str) -> bool {
    a == b || is_under(a, b)
}

/// Cartella che contiene `p` (normalizzato), se c'è.
pub(crate) fn parent(p: &str) -> Option<&str> {
    let i = p.rfind('\\')?;
    if i + 1 == p.len() {
        return None;
    }
    // `c:\x` → `c:\`; `c:\x\y` → `c:\x`.
    Some(if i == 2 && p.as_bytes()[1] == b':' { &p[..3] } else { &p[..i] })
}

/// Percorso di un figlio.
pub(crate) fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('\\') || dir.ends_with('/') { format!("{dir}{name}") } else { format!("{dir}\\{name}") }
}

/// Eseguibile o DLL, dall'estensione senza distinzione di maiuscole.
fn is_exe_or_dll(path: &str) -> bool {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((_, ext)) => ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("dll"),
        None => false,
    }
}

// ---------------------------------------------------------------------------------------------
// Visita

/// Ciò che una visita ha letto in un elemento.
#[derive(Debug, Default)]
pub(crate) struct Visit {
    pub(crate) files: u64,
    pub(crate) dirs: u64,
    pub(crate) logical: u64,
    /// Allocate dei file con un solo nome o senza identità.
    pub(crate) plain: u64,
    /// Cartelle negate e voci non leggibili: il loro contenuto non entra nella stima.
    pub(crate) denied: u64,
    /// File con più nomi: identità, numero di nomi, allocata.
    pub(crate) linked: Vec<(estimate::LinkKey, u32, u64)>,
    /// File (non collegamenti) con la data di modifica, da cui si sceglie il campione di Restart Manager.
    pub(crate) pool: Vec<(String, i64)>,
    /// Almeno una voce dentro ha un percorso di [`MAX_PATH`] caratteri o più.
    pub(crate) long: bool,
}

impl Visit {
    /// Conta una voce che non si attraversa: file o collegamento. Un file residente nella MFT vale 0
    /// nella stima, qualunque allocata riporti Windows (03 §6.3).
    fn add(&mut self, path: String, a: &Attrs) {
        if a.directory {
            self.dirs += 1;
        } else {
            self.files += 1;
        }
        self.logical += a.logical;
        let alloc = if a.resident { 0 } else { a.alloc };
        if !a.link && !a.directory && a.links > 1 && a.file_id != 0 {
            self.linked.push(((a.volume_serial, a.file_id), a.links, alloc));
        } else {
            self.plain += alloc;
        }
        if !a.link && !a.directory {
            self.pool.push((path, a.modified));
        }
    }
}

/// Legge un elemento già aperto con `a`: un file o un collegamento conta da solo; una cartella si visita,
/// con [`Probe::threads`] thread. `step` si chiama per ogni voce letta (anche da più thread) e
/// restituisce falso per fermarsi (annullamento): allora `None`.
pub(crate) fn read_item(probe: &dyn Probe, path: &str, a: &Attrs, step: &(dyn Fn() -> bool + Sync)) -> Option<Visit> {
    let listings = if a.directory && !a.link { read_dirs(probe, &[path.to_owned()], step)?.pop().unwrap_or_default() } else { Listings::new() };
    Some(visit(path, a, listings))
}

/// Ciò che la visita sa di un elemento aperto con `a`: un file o un collegamento conta da solo; una
/// cartella si visita sulle sue cartelle già lette da [`read_dirs`] (`listings`).
fn visit(path: &str, a: &Attrs, mut listings: Listings) -> Visit {
    let mut v = Visit::default();
    if a.directory && !a.link {
        // L'indice della cartella stessa: lo svuotamento lo libera.
        v.plain += a.alloc;
        visit_dir(path, &mut listings, &mut v);
    } else {
        v.add(path.to_owned(), a);
    }
    v
}

/// Cartelle lette da [`read_dirs`], per percorso: le voci con i loro attributi, o il codice dell'errore
/// dell'elenco.
type Listings = HashMap<String, Result<Vec<Entry>, u32>>;

/// Voce da attraversare: una cartella, non un collegamento, con gli attributi letti.
fn crossed(e: &Entry) -> bool {
    matches!(&e.1, Ok(a) if a.directory && !a.link)
}

/// Visita di una cartella (con una pila, senza ricorsione) sulle cartelle già lette, nell'ordine di una
/// lettura su un thread solo: le voci di una cartella nell'ordine dell'elenco, poi l'ultima sottocartella
/// trovata. Così il campione di Restart Manager, che a parità di data tiene i file nell'ordine della visita,
/// non dipende dai thread. I collegamenti contano da soli e non si attraversano; una cartella con l'elenco
/// negato conta in `denied`; una voce sparita durante la visita (errore 2 o 3) non conta; una voce con gli
/// attributi non leggibili conta in `denied`. Ogni voce trovata (anche negata o non leggibile) controlla la
/// lunghezza del suo percorso.
fn visit_dir(root: &str, listings: &mut Listings, v: &mut Visit) {
    let mut stack = vec![root.to_owned()];
    while let Some(dir) = stack.pop() {
        let entries = match listings.remove(&dir) {
            Some(Ok(e)) => e,
            // `None` non capita: [`read_dirs`] legge ogni cartella che la visita attraversa.
            Some(Err(_)) | None => {
                v.denied += 1;
                continue;
            }
        };
        for (name, attrs) in entries {
            if matches!(attrs, Err(e) if is_missing(e)) {
                continue;
            }
            let p = join(&dir, &name);
            v.long |= too_long(&p);
            match attrs {
                Ok(a) if a.directory && !a.link => {
                    v.dirs += 1;
                    // L'indice della sottocartella (03 §6.3, misurato su R:).
                    v.plain += a.alloc;
                    stack.push(p);
                }
                Ok(a) => v.add(p, &a),
                Err(_) => v.denied += 1,
            }
        }
    }
}

/// Legge le cartelle sotto ciascuna delle `roots` (comprese), una [`Listings`] per radice, con
/// [`Probe::threads`] thread che prendono la cartella successiva da una coda comune. Con un thread solo
/// l'ordine delle letture è quello della visita. `None` se `step` ha fermato la lettura.
fn read_dirs(probe: &dyn Probe, roots: &[String], step: &(dyn Fn() -> bool + Sync)) -> Option<Vec<Listings>> {
    let mut out: Vec<Listings> = roots.iter().map(|_| HashMap::new()).collect();
    let threads = probe.threads();
    if threads <= 1 {
        for (k, root) in roots.iter().enumerate() {
            let mut stack = vec![root.clone()];
            while let Some(dir) = stack.pop() {
                let listing = match probe.entries(&dir, step) {
                    Err(ListError::Stopped) => return None,
                    Err(ListError::Code(e)) => Err(e),
                    Ok(entries) => {
                        stack.extend(entries.iter().filter(|e| crossed(e)).map(|e| join(&dir, &e.0)));
                        Ok(entries)
                    }
                };
                out[k].insert(dir, listing);
            }
        }
        return Some(out);
    }
    let queue = Queue {
        state: Mutex::new(QueueState { jobs: roots.iter().cloned().enumerate().rev().collect(), busy: 0, stopped: false, out }),
        ready: Condvar::new(),
    };
    std::thread::scope(|s| {
        // Un thread che non parte non ferma la lettura: lavora anche questo thread.
        for k in 1..threads {
            let spawned = std::thread::Builder::new().name(format!("quado-simulazione-{k}")).spawn_scoped(s, || queue.work(probe, step));
            if spawned.is_err() {
                break;
            }
        }
        queue.work(probe, step);
    });
    let state = queue.state.into_inner().unwrap_or_else(PoisonError::into_inner);
    if state.stopped { None } else { Some(state.out) }
}

/// Coda delle cartelle da leggere, condivisa dai thread di [`read_dirs`].
struct Queue {
    state: Mutex<QueueState>,
    /// Segnale per i thread in attesa: una cartella nuova in coda, la fine o l'arresto.
    ready: Condvar,
}

struct QueueState {
    /// Cartelle da leggere, con l'indice della radice.
    jobs: Vec<(usize, String)>,
    /// Cartelle in lettura: con la coda vuota e nessuna in lettura il lavoro è finito.
    busy: usize,
    stopped: bool,
    out: Vec<Listings>,
}

/// Una cartella in lettura: se la lettura si interrompe (panico), il drop ferma gli altri thread, che
/// altrimenti aspetterebbero per sempre la sua fine.
struct Reading<'a> {
    queue: &'a Queue,
    finished: bool,
}

impl Drop for Reading<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let mut s = self.queue.lock();
            s.busy -= 1;
            s.stopped = true;
            self.queue.ready.notify_all();
        }
    }
}

impl Queue {
    fn lock(&self) -> std::sync::MutexGuard<'_, QueueState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Prende cartelle dalla coda finché ce ne sono o qualcuno ne sta leggendo una (che può aggiungerne).
    fn work(&self, probe: &dyn Probe, step: &(dyn Fn() -> bool + Sync)) {
        loop {
            let (k, dir) = {
                let mut s = self.lock();
                loop {
                    if s.stopped {
                        return;
                    }
                    if let Some(job) = s.jobs.pop() {
                        s.busy += 1;
                        break job;
                    }
                    if s.busy == 0 {
                        return;
                    }
                    s = self.ready.wait(s).unwrap_or_else(PoisonError::into_inner);
                }
            };
            let mut reading = Reading { queue: self, finished: false };
            let result = probe.entries(&dir, step);
            reading.finished = true;
            let mut s = self.lock();
            s.busy -= 1;
            match result {
                Err(ListError::Stopped) => s.stopped = true,
                Err(ListError::Code(e)) => {
                    s.out[k].insert(dir, Err(e));
                }
                Ok(entries) => {
                    for e in entries.iter().filter(|e| crossed(e)) {
                        s.jobs.push((k, join(&dir, &e.0)));
                    }
                    s.out[k].insert(dir, Ok(entries));
                }
            }
            self.ready.notify_all();
        }
    }
}

/// Gruppi di elementi da chiedere a Restart Manager uno dopo l'altro: quelli che possono avere file in
/// comune, cioè uno dentro l'altro (o lo stesso percorso) o con un nome ciascuno dello stesso file (hard
/// link). Due elementi così non si chiedono insieme: la domanda diretta a NTFS tiene aperti per tutta la
/// chiamata i file WOF e i segnaposto, e l'apertura dei file per sceglierne la strada lascia handle per
/// un attimo, quindi Restart Manager o la domanda diretta dell'altro elemento vedrebbero la simulazione
/// stessa come un processo che usa i file (`probe_win.rs`, elemento dentro un altro). Gruppi e elementi
/// nell'ordine di `asked`.
fn rm_groups(asked: &[usize], items: &[SimItem], linked: &[estimate::Linked]) -> Vec<Vec<usize>> {
    // Unione per insiemi disgiunti sulle posizioni in `asked`.
    let mut up: Vec<usize> = (0..asked.len()).collect();
    fn root(up: &mut [usize], mut k: usize) -> usize {
        while up[k] != k {
            up[k] = up[up[k]];
            k = up[k];
        }
        k
    }
    let mut join_sets = |a: usize, b: usize| {
        let (ra, rb) = (root(&mut up, a), root(&mut up, b));
        if ra != rb {
            up[ra.max(rb)] = ra.min(rb);
        }
    };
    // Percorsi in un ordine in cui ciò che sta dentro un percorso viene subito dopo di lui (`\` prima di
    // ogni altro carattere); una pila tiene gli antenati della voce corrente.
    let mut by_path: Vec<(String, usize)> = asked.iter().enumerate().map(|(k, &i)| (norm(&items[i].path).replace('\\', "\u{1}"), k)).collect();
    by_path.sort();
    let mut ancestors: Vec<(String, usize)> = Vec::new();
    for (key, k) in by_path {
        let path = key.replace('\u{1}', "\\");
        while ancestors.last().is_some_and(|(a, _)| !is_within(&path, a)) {
            ancestors.pop();
        }
        if let Some(&(_, a)) = ancestors.last() {
            join_sets(a, k);
        }
        ancestors.push((path, k));
    }
    let position: HashMap<usize, usize> = asked.iter().enumerate().map(|(k, &i)| (i, k)).collect();
    for g in linked {
        let mut ks = g.names.iter().filter_map(|(i, _)| position.get(i).copied());
        if let Some(first) = ks.next() {
            for k in ks {
                join_sets(first, k);
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut slot: HashMap<usize, usize> = HashMap::new();
    for (k, &i) in asked.iter().enumerate() {
        let r = root(&mut up, k);
        let g = *slot.entry(r).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push(i);
    }
    groups
}

/// `f` su ogni elemento di `items`, con al più `threads` thread; `stop` si controlla prima di ogni
/// elemento. Risultati nell'ordine di `items`; `None` se `stop` ne ha lasciato qualcuno senza risultato.
/// Con un thread solo gli elementi vanno in ordine.
fn each<T: Sync, R: Send>(threads: usize, items: &[T], stop: &(dyn Fn() -> bool + Sync), f: &(dyn Fn(&T) -> R + Sync)) -> Option<Vec<R>> {
    if threads <= 1 || items.len() <= 1 {
        let mut out = Vec::with_capacity(items.len());
        for it in items {
            if stop() {
                return None;
            }
            out.push(f(it));
        }
        return Some(out);
    }
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new(items.iter().map(|_| None).collect());
    let work = || {
        loop {
            if stop() {
                return;
            }
            let k = next.fetch_add(1, Ordering::Relaxed);
            let Some(it) = items.get(k) else { return };
            let r = f(it);
            results.lock().unwrap_or_else(PoisonError::into_inner)[k] = Some(r);
        }
    };
    std::thread::scope(|s| {
        for k in 1..threads.min(items.len()) {
            if std::thread::Builder::new().name(format!("quado-uso-{k}")).spawn_scoped(s, work).is_err() {
                break;
            }
        }
        work();
    });
    results.into_inner().unwrap_or_else(PoisonError::into_inner).into_iter().collect()
}

/// Campione di Restart Manager (05 D7): tutti gli eseguibili e le DLL, più i [`RM_SAMPLE`] altri file
/// modificati più di recente (tutti, se non sono di più).
pub(crate) fn rm_sample(pool: Vec<(String, i64)>) -> Vec<String> {
    let (mut keep, mut others): (Vec<_>, Vec<_>) = pool.into_iter().partition(|(p, _)| is_exe_or_dll(p));
    others.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
    others.truncate(RM_SAMPLE);
    keep.extend(others);
    keep.into_iter().map(|(p, _)| p).collect()
}

fn push_unique(blocks: &mut Vec<Block>, b: Block) {
    if !blocks.contains(&b) {
        blocks.push(b);
    }
}

// ---------------------------------------------------------------------------------------------
// Simulazione

/// Simula `req`: attributi, stima, controlli. Non scrive niente e non legge il contenuto dei file.
pub fn simulate(req: Request, probe: &dyn Probe, progress: &SimProgress) -> Result<Simulazione, Cancelled> {
    let cancelled = || progress.cancel.load(Ordering::Relaxed);
    let Request { items: requested, record, protected } = req;
    let mut items = Vec::with_capacity(requested.len());
    let mut extra = Vec::with_capacity(requested.len());
    let mut links = LinkTable::default();
    // Restart Manager eseguito senza processi e senza errore, per elemento.
    let mut rm_clear = vec![false; requested.len()];
    let mut read = vec![false; requested.len()];

    // 1. Attributi degli elementi, poi le loro cartelle, lette tutte insieme con `Probe::threads` thread;
    // infine visita, stima e campione, elemento per elemento.
    let mut opened: Vec<Option<Attrs>> = vec![None; requested.len()];
    for (i, r) in requested.iter().enumerate() {
        if cancelled() {
            return Err(Cancelled);
        }
        let mut it = SimItem {
            path: r.path.clone(),
            directory: false,
            fixed: r.fixed,
            blocks: Vec::new(),
            manual: Vec::new(),
            files: 0,
            dirs: 0,
            logical: 0,
            alloc: 0,
            denied_dirs: 0,
        };
        if r.fixed.is_none() {
            match probe.attributes(&r.path) {
                Err(e) if is_missing(e) => it.blocks.push(Block::Missing),
                Err(e) => it.blocks.push(Block::Unreadable(e)),
                Ok(a) => {
                    progress.done.fetch_add(1, Ordering::Relaxed);
                    opened[i] = Some(a);
                }
            }
            // Percorsi lunghi: la shell eliminerebbe per sempre. Si blocca anche se esistesse un nome
            // breve 8.3 con cui spostarlo (scelta prudente della v1).
            if too_long(&r.path) {
                it.blocks.push(Block::LongPath);
            }
        }
        items.push(it);
        extra.push(ItemExtra::default());
    }
    let step = || {
        progress.done.fetch_add(1, Ordering::Relaxed);
        !cancelled()
    };
    let folders: Vec<usize> = (0..requested.len()).filter(|&i| opened[i].is_some_and(|a| a.directory && !a.link)).collect();
    let roots: Vec<String> = folders.iter().map(|&i| requested[i].path.clone()).collect();
    let mut listings: HashMap<usize, Listings> = folders.into_iter().zip(read_dirs(probe, &roots, &step).ok_or(Cancelled)?).collect();
    for (i, r) in requested.iter().enumerate() {
        let Some(a) = opened[i] else { continue };
        let v = visit(&r.path, &a, listings.remove(&i).unwrap_or_default());
        let (it, x) = (&mut items[i], &mut extra[i]);
        it.directory = a.directory;
        it.files = v.files;
        it.dirs = v.dirs;
        it.logical = v.logical;
        it.denied_dirs = v.denied;
        x.plain = v.plain;
        x.snapshot = Snapshot { file_id: a.file_id, volume_serial: a.volume_serial, logical: v.logical, modified: a.modified, files: v.files };
        for (key, n, alloc) in v.linked {
            links.add(i, key, n, alloc);
        }
        x.sample = rm_sample(v.pool);
        read[i] = true;
        if v.long && !too_long(&r.path) {
            it.blocks.push(Block::ContainsLongPath);
        }
    }
    let linked = links.into_groups();
    for (i, it) in items.iter_mut().enumerate() {
        it.alloc = estimate::item_alloc(extra[i].plain, &linked, i);
    }
    let proposed: Vec<usize> = (0..items.len()).filter(|&i| items[i].fixed.is_none()).collect();

    // 2. Volume e Cestino (F-AC-1, F-AC-5, F-AC-7): tutti gli elementi nel volume del primo.
    let volume = match proposed.first() {
        None => VolumeBin::default(),
        Some(&first) => match probe.volume_bin(&items[first].path) {
            Err(e) => {
                for &i in &proposed {
                    push_unique(&mut items[i].blocks, Block::Unreadable(e));
                }
                VolumeBin::default()
            }
            Ok(v) => {
                let root = norm(&v.root);
                // Elementi nella stessa cartella stanno nello stesso volume: una lettura per cartella.
                let mut roots: HashMap<String, Result<String, u32>> = HashMap::new();
                if let Some(p) = parent(&norm(&items[first].path)) {
                    roots.insert(p.to_owned(), Ok(root.clone()));
                }
                for &i in &proposed {
                    if cancelled() {
                        return Err(Cancelled);
                    }
                    let path = norm(&items[i].path);
                    let other = if i == first {
                        Ok(root.clone())
                    } else {
                        let key = parent(&path).unwrap_or(&path).to_owned();
                        roots.entry(key).or_insert_with(|| probe.volume_bin(&items[i].path).map(|b| norm(&b.root))).clone()
                    };
                    let it = &mut items[i];
                    let same = match other {
                        Err(e) => {
                            push_unique(&mut it.blocks, Block::Unreadable(e));
                            false
                        }
                        Ok(r) if r != root => {
                            push_unique(&mut it.blocks, Block::OtherVolume);
                            false
                        }
                        Ok(_) => true,
                    };
                    if !v.has_bin {
                        push_unique(&mut it.blocks, Block::NoRecycleBin);
                    }
                    if v.nuke_on_delete {
                        push_unique(&mut it.blocks, Block::NukeOnDelete);
                    }
                    match v.capacity {
                        None => push_unique(&mut it.blocks, Block::CapacityUnknown),
                        Some(capacity) if same && read[i] && it.logical > capacity => {
                            push_unique(&mut it.blocks, Block::LargerThanBin { logical: it.logical, capacity });
                        }
                        Some(_) => {}
                    }
                }
                v
            }
        },
    };

    // 3. Restart Manager (F-AC-8): una chiamata per elemento letto, con il suo campione, su
    // `Probe::threads` thread (una chiamata costa secondi, quasi tutti in `RmGetList`).
    // Gli elementi che possono avere file in comune stanno in un gruppo e si chiedono uno dopo l'altro.
    let asked: Vec<usize> = proposed.iter().copied().filter(|&i| read[i]).collect();
    let groups = if probe.threads() <= 1 { vec![asked.clone()] } else { rm_groups(&asked, &items, &linked) };
    let ask = |group: &Vec<usize>| -> Option<Vec<_>> {
        let mut out = Vec::with_capacity(group.len());
        for &i in group {
            if cancelled() {
                return None;
            }
            out.push((i, probe.processes_using(&extra[i].sample)));
        }
        Some(out)
    };
    let answers: Option<Vec<_>> = each(probe.threads(), &groups, &cancelled, &ask).ok_or(Cancelled)?.into_iter().collect();
    let mut answers: Vec<_> = answers.ok_or(Cancelled)?.into_iter().flatten().collect();
    answers.sort_by_key(|(i, _)| *i);
    for (i, answer) in answers {
        match answer {
            Ok(p) if p.is_empty() => rm_clear[i] = true,
            Ok(p) => items[i].blocks.push(Block::InUse(p)),
            Err(e) => items[i].blocks.push(Block::RestartManager(e)),
        }
    }

    // 4. Elementi protetti (F-AC-4): relazioni con le radici. L'eccezione si valuta alla fine.
    let roots: Vec<(String, ProtectedKind)> = protected.iter().map(|r| (norm(&r.path), r.kind)).collect();
    let exception_allowed = record.protected_exception && matches!(record.state, State::ToVerify(_));
    let mut hard: Vec<Vec<ProtectedKind>> = vec![Vec::new(); items.len()];
    let mut candidate = vec![false; items.len()];
    for &i in &proposed {
        let p = norm(&items[i].path);
        for (root, kind) in &roots {
            if *kind == ProtectedKind::RegisteredProgram && is_under(&p, root) {
                candidate[i] = true;
            } else if (p == *root || is_under(&p, root) || is_under(root, &p)) && !hard[i].contains(kind) {
                hard[i].push(*kind);
            }
        }
    }

    // 5. Riferimenti (F-AC-9): se la regola li chiede, e sempre per gli elementi con l'eccezione.
    let wanted = record.checks.iter().any(|c| c.key == CHECK_REFERENCES);
    let interested: Vec<usize> = proposed.iter().copied().filter(|&i| wanted || (candidate[i] && exception_allowed)).collect();
    let mut refs_checked = vec![false; items.len()];
    let references = if interested.is_empty() {
        ReferenceReport::default()
    } else {
        if cancelled() {
            return Err(Cancelled);
        }
        let paths: Vec<String> = interested.iter().map(|&i| items[i].path.clone()).collect();
        let report = probe.references(&paths);
        for &i in &interested {
            let p = norm(&items[i].path);
            let those: Vec<Reference> = report.found.iter().filter(|r| is_within(&norm(&r.target), &p)).cloned().collect();
            if !those.is_empty() {
                items[i].blocks.push(Block::Referenced(those));
            }
            refs_checked[i] = true;
        }
        report
    };

    // 6. Cartelle di sincronizzazione (F-AC-2, R10): controllo manuale. Prima le cartelle registrate in
    // Windows, poi quelle che Quado riconosce dalla configurazione dei programmi o dai marcatori.
    for &i in &proposed {
        let found = match probe.sync_root(&items[i].path) {
            Some(provider) => Some((provider, crate::sync::SyncKind::Registered)),
            None => probe.sync_marker(&items[i].path).map(|f| (f.provider, f.kind)),
        };
        if let Some((provider, kind)) = found {
            items[i].manual.push(ManualCheck::Synced { provider, kind });
        }
    }

    // 7. Blocchi di protezione, con l'eccezione di 03 §6.1 solo se ogni altro controllo è passato:
    // Restart Manager senza processi né errori, riferimenti controllati senza riferimenti all'elemento,
    // nessun altro blocco. Un controllo dei riferimenti "parziale" non la fa cadere: su questo PC le
    // attività pianificate di sistema non sono mai leggibili da un utente standard (misura dell'agente C,
    // 14/10/2026); la conferma lo dichiara da `Simulazione::references().partial` (F-AC-9).
    for &i in &proposed {
        let others = !items[i].blocks.is_empty() || !hard[i].is_empty();
        for kind in &hard[i] {
            push_unique(&mut items[i].blocks, Block::Protected(*kind));
        }
        if candidate[i] {
            let holds = exception_allowed && rm_clear[i] && refs_checked[i] && !others;
            if !holds {
                push_unique(&mut items[i].blocks, Block::Protected(ProtectedKind::RegisteredProgram));
            }
        }
    }

    let storage_sense_days = probe.storage_sense_days().ok().flatten();
    let rule_manual = record.checks.iter().filter(|c| c.kind == CheckKind::Manual).map(|c| c.key.clone()).collect();
    let checked_at = probe.now();
    Ok(Simulazione { record, items, volume, storage_sense_days, references, rule_manual, checked_at, extra, linked })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_normalized() {
        assert_eq!(norm("C:\\Windows\\"), "c:\\windows");
        assert_eq!(norm("C:\\"), "c:\\");
        assert_eq!(norm("c:/Users/X/"), "c:\\users\\x");
        assert_eq!(norm("\\\\?\\C:\\Program Files\\ÉL"), "c:\\program files\\él");
        assert_eq!(norm("\\\\?\\UNC\\srv\\share\\x"), "\\\\srv\\share\\x");
        assert_eq!(norm("\\\\?\\C:\\"), "c:\\");
    }

    #[test]
    fn relations_respect_component_boundaries() {
        assert!(is_under("c:\\windows\\winsxs", "c:\\windows"));
        assert!(!is_under("c:\\windows2", "c:\\windows"));
        assert!(!is_under("c:\\windows", "c:\\windows"));
        assert!(is_under("c:\\windows", "c:\\"));
        assert!(is_within("c:\\windows", "c:\\windows"));
        assert_eq!(parent("c:\\x"), Some("c:\\"));
        assert_eq!(parent("c:\\x\\y"), Some("c:\\x"));
        assert_eq!(parent("c:\\"), None);
    }

    #[test]
    fn long_paths_count_utf16_units_without_the_prefix() {
        let base = format!("C:\\{}", "a".repeat(255));
        assert_eq!(base.len(), 258);
        assert!(!too_long(&base));
        assert!(!too_long(&format!("{base}\\")));
        assert!(!too_long(&format!("\\\\?\\{base}")));
        assert!(!too_long(&format!("{base}b")));
        assert!(too_long(&format!("{base}bb")));
        // Un carattere fuori dal piano di base vale due unità UTF-16.
        assert!(too_long(&format!("{base}\u{1F600}")));
        assert!(!too_long(&format!("{}è", &base[..base.len() - 1])));
    }

    #[test]
    fn items_that_may_share_files_are_asked_in_one_group() {
        let item = |path: &str| SimItem {
            path: path.to_owned(),
            directory: true,
            fixed: None,
            blocks: Vec::new(),
            manual: Vec::new(),
            files: 0,
            dirs: 0,
            logical: 0,
            alloc: 0,
            denied_dirs: 0,
        };
        let items: Vec<SimItem> = ["C:\\a", "C:\\a b", "C:\\A\\x", "C:\\c", "C:\\d", "C:\\e", "c:/a b/"].iter().map(|p| item(p)).collect();
        // Un file con un nome in `C:\c` e uno in `C:\e`.
        let linked = vec![estimate::Linked { links: 2, alloc: 4096, names: vec![(3, 1), (5, 1)] }];
        let asked = [0, 1, 2, 3, 4, 5, 6];
        assert_eq!(rm_groups(&asked, &items, &linked), vec![vec![0, 2], vec![1, 6], vec![3, 5], vec![4]]);
        // Solo alcuni elementi chiesti: gli altri non legano niente.
        assert_eq!(rm_groups(&[1, 2, 4], &items, &linked), vec![vec![1], vec![2], vec![4]]);
        let drive = vec![item("C:\\"), item("C:\\z")];
        assert_eq!(rm_groups(&[0, 1], &drive, &[]), vec![vec![0, 1]]);
    }

    #[test]
    fn executables_by_extension() {
        assert!(is_exe_or_dll("C:\\a\\CapCut.EXE"));
        assert!(is_exe_or_dll("C:\\a\\x.Dll"));
        assert!(!is_exe_or_dll("C:\\a.exe\\x"));
        assert!(!is_exe_or_dll("C:\\a\\exe"));
    }
}
