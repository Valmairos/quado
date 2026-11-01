// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask m5-misure`: costo della stima in simulazione su dati veri (07 §1.6 criterio 4, 05 D7),
//! in sola lettura. Scrive `target/misure/m5-misure-C.md`.
//!
//! 1. Verifica preliminare in una cartella temporanea: la simulazione intera (attributi, elenchi, uso dei
//!    file, riferimenti) non cambia la data di ultimo accesso né la data di modifica di un file di prova.
//!    Se la cambia, il comando si ferma prima di toccare C:.
//! 2. Come l'app: scansione di C:, prove (`quado_kb::win::gather`), classificazione, Programmi, radici
//!    protette (`quado_kb::protection`).
//! 3. `simulate` con `WinProbe` e la configurazione vera dell'app (`KbConfig`: Cestino e Sensore memoria dal
//!    registro, riferimenti con `find_live`) sulle versioni precedenti di CapCut (regola `capcut-versioni`)
//!    e sul record più grande della regola `progetti-sviluppo`, se ci sono: 5 simulazioni ciascuno, con il
//!    tempo totale e quello di attributi, elenchi, uso dei file e riferimenti, separati da una sonda che
//!    misura le chiamate.
//!
//! Opzioni della baseline delle prestazioni: `--solo-capcut`, `--senza-verifica`, `--out FILE`; `--prima`
//! legge come prima delle letture in parallelo (una voce per volta, un thread), per il confronto.
//!
//! Mai `plan::conferma`, mai `execute::esegui`, mai il mover: questo modulo non li importa. L'uso dei
//! file si legge con `restart::processes_using`: Restart Manager per i file comuni, domanda diretta a NTFS
//! per i file WOF e i segnaposto, che così non si decomprimono né si scaricano (prove su R:,
//! `quado-actions/tests/fine_vhdx.rs`); il resoconto dice quanti file compressi WOF
//! (`Flags::ALLOC_FIXED`) e segnaposto (`Flags::CLOUD`) c'erano negli elementi. Gli elementi con una
//! parte di `--escludi "a;b"` nel percorso (senza distinzione di maiuscole) non si simulano.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant, SystemTime};

use quado_actions::plan::{Block, Simulazione};
use quado_actions::probe::{Attrs, Entry, ListError, Probe, VolumeBin};
use quado_actions::restart::{ProcessUse, RmError};
use quado_actions::simulate::{FixedReason, Request, RequestItem, SimProgress, simulate};
use quado_actions::win_probe::WinProbe;
use quado_kb::{Kb, classify};
use quado_model::intervention::{ProtectedRoot, ReferenceReport};
use quado_model::record::{Estimate, Record, Value};
use quado_model::{Flags, Tree};
use quado_scan_win::{Options, Progress, scan, system};
use quado_ui::app::intervention_jobs::KbConfig;

/// Parti di percorso mai simulate (`--escludi`, in minuscolo).
static EXCLUDED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
/// Simulazioni per elemento.
const RUNS: usize = 5;

fn gib(b: u64) -> String {
    format!("{:.2}", b as f64 / f64::from(1u32 << 30))
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn excluded(path: &str) -> bool {
    let p = path.to_lowercase();
    EXCLUDED.get().is_some_and(|parts| parts.iter().any(|x| p.contains(x.as_str())))
}

// ---------------------------------------------------------------------------------------------
// Sonda che misura

/// Tempi (nanosecondi) e numero di chiamate di una categoria di letture. Con più thread (`Probe::threads`)
/// la somma dei tempi delle chiamate supera il tempo reale: `wall_ms` è l'intervallo dall'inizio della
/// prima chiamata alla fine dell'ultima.
#[derive(Default)]
struct Meter {
    calls: AtomicU64,
    nanos: AtomicU64,
    span: Mutex<Option<(Instant, Instant)>>,
}

impl Meter {
    fn time<T>(&self, f: impl FnOnce() -> T) -> T {
        let t = Instant::now();
        let r = f();
        let end = Instant::now();
        self.nanos.fetch_add((end - t).as_nanos() as u64, Relaxed);
        self.calls.fetch_add(1, Relaxed);
        if let Ok(mut s) = self.span.lock() {
            *s = Some(match *s {
                None => (t, end),
                Some((a, b)) => (a.min(t), b.max(end)),
            });
        }
        r
    }

    /// Somma dei tempi delle chiamate.
    fn ms(&self) -> f64 {
        self.nanos.load(Relaxed) as f64 / 1e6
    }

    /// Tempo reale, dalla prima chiamata alla fine dell'ultima.
    fn wall_ms(&self) -> f64 {
        self.span.lock().ok().and_then(|s| *s).map_or(0.0, |(a, b)| ms(b - a))
    }

    fn calls(&self) -> u64 {
        self.calls.load(Relaxed)
    }
}

/// `WinProbe` con i tempi per categoria. `unsafe_rm` resta vuoto: serviva a tenere i file WOF e i
/// segnaposto lontani da Restart Manager, ora lo fa `restart::processes_using`. Con `before` (opzione
/// `--prima`) la simulazione legge come prima delle letture in parallelo: elenchi e attributi una voce per volta
/// (`entries_one_by_one`: `children`, che apre la cartella tre volte, poi `attributes` di ogni voce) e un
/// thread solo, anche per l'uso dei file.
struct Timed<'a> {
    inner: &'a WinProbe,
    unsafe_rm: &'a HashSet<String>,
    before: bool,
    attrs: Meter,
    lists: Meter,
    rm: Meter,
    rm_files: AtomicU64,
    rm_removed: AtomicU64,
    refs: Meter,
    other: Meter,
}

impl<'a> Timed<'a> {
    fn new(inner: &'a WinProbe, unsafe_rm: &'a HashSet<String>, before: bool) -> Self {
        Self {
            inner,
            unsafe_rm,
            before,
            attrs: Meter::default(),
            lists: Meter::default(),
            rm: Meter::default(),
            rm_files: AtomicU64::new(0),
            rm_removed: AtomicU64::new(0),
            refs: Meter::default(),
            other: Meter::default(),
        }
    }
}

impl Probe for Timed<'_> {
    fn attributes(&self, path: &str) -> Result<Attrs, u32> {
        self.attrs.time(|| self.inner.attributes(path))
    }
    fn children(&self, dir: &str) -> Result<Vec<String>, u32> {
        self.lists.time(|| self.inner.children(dir))
    }
    /// Elenco e attributi delle voci insieme (`WinProbe::entries`): contano negli elenchi.
    fn entries(&self, dir: &str, step: &dyn Fn() -> bool) -> Result<Vec<Entry>, ListError> {
        if self.before {
            // `children` e `attributes` hanno i loro tempi.
            return quado_actions::probe::entries_one_by_one(self, dir, step);
        }
        self.lists.time(|| self.inner.entries(dir, step))
    }
    fn threads(&self) -> usize {
        if self.before { 1 } else { self.inner.threads() }
    }
    fn processes_using(&self, paths: &[String]) -> Result<Vec<ProcessUse>, RmError> {
        let safe: Vec<String> = paths.iter().filter(|p| !self.unsafe_rm.contains(&p.to_lowercase())).cloned().collect();
        self.rm_removed.fetch_add((paths.len() - safe.len()) as u64, Relaxed);
        self.rm_files.fetch_add(safe.len() as u64, Relaxed);
        self.rm.time(|| self.inner.processes_using(&safe))
    }
    fn references(&self, paths: &[String]) -> ReferenceReport {
        self.refs.time(|| self.inner.references(paths))
    }
    fn volume_bin(&self, path: &str) -> Result<VolumeBin, u32> {
        self.other.time(|| self.inner.volume_bin(path))
    }
    fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
        self.other.time(|| self.inner.storage_sense_days())
    }
    fn sync_root(&self, path: &str) -> Option<String> {
        self.other.time(|| self.inner.sync_root(path))
    }
    fn free_space(&self, path: &str) -> Result<u64, u32> {
        self.other.time(|| self.inner.free_space(path))
    }
    fn now(&self) -> i64 {
        self.inner.now()
    }
}

// ---------------------------------------------------------------------------------------------
// Verifica preliminare: ultimo accesso

/// Simula una cartella temporanea con un file dalla data di ultimo accesso spostata indietro di tre
/// giorni e confronta le date prima e dopo. Una lettura del contenuto fa da controprova: dice se su
/// questo volume Windows aggiorna davvero l'ultimo accesso.
fn last_access_check(out: &mut String) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("quado-m5-accesso-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sotto")).map_err(|e| e.to_string())?;
    let files = [dir.join("prova.bin"), dir.join("sotto").join("modulo.dll"), dir.join("controprova.bin")];
    let old = SystemTime::now() - Duration::from_secs(3 * 86_400);
    for (k, f) in files.iter().enumerate() {
        std::fs::write(f, vec![b'a' + k as u8; 64 * 1024]).map_err(|e| e.to_string())?;
        let h = std::fs::File::options().write(true).open(f).map_err(|e| e.to_string())?;
        h.set_times(std::fs::FileTimes::new().set_accessed(old).set_modified(old)).map_err(|e| e.to_string())?;
    }
    let times = |p: &Path| -> Result<(SystemTime, SystemTime), String> {
        let m = std::fs::metadata(p).map_err(|e| e.to_string())?;
        Ok((m.accessed().map_err(|e| e.to_string())?, m.modified().map_err(|e| e.to_string())?))
    };
    let before: Vec<_> = files.iter().map(|f| times(f)).collect::<Result<_, _>>()?;
    let probe = WinProbe::new(Box::new(KbConfig));
    let req = Request { items: vec![RequestItem { path: dir.to_string_lossy().into_owned(), fixed: None }], record: references_record(), protected: Vec::new() };
    let t = Instant::now();
    let sim = simulate(req, &probe, &SimProgress::default()).map_err(|_| "simulazione annullata")?;
    let took = t.elapsed();
    let after: Vec<_> = files.iter().map(|f| times(f)).collect::<Result<_, _>>()?;
    // Controprova: lettura del contenuto di un file.
    let _ = std::fs::read(&files[2]).map_err(|e| e.to_string())?;
    let control = times(&files[2])?;
    let _ = std::fs::remove_dir_all(&dir);
    let same = before == after;
    let control_moved = control.0 != before[2].0;
    let _ = writeln!(out, "## Ultimo accesso (verifica preliminare)\n");
    let _ = writeln!(
        out,
        "Cartella temporanea con tre file da 64 KiB (uno `.dll`), ultimo accesso e modifica spostati indietro di tre giorni; simulazione intera con `WinProbe` e `KbConfig` ({:.0} ms, {} file letti, stima {}).\n",
        ms(took),
        sim.items()[0].files,
        estimate_text(&sim.estimate(&[true]).estimate)
    );
    let _ = writeln!(out, "| Controllo | Esito |\n|---|---|");
    let _ = writeln!(out, "| Ultimo accesso e modifica dei file dopo la simulazione | {} |", if same { "invariati" } else { "**cambiati**" });
    let _ = writeln!(
        out,
        "| Controprova: ultimo accesso dopo una lettura del contenuto | {} |",
        if control_moved {
            "cambiato: Windows aggiorna l'ultimo accesso, la verifica distingue"
        } else {
            "invariato: su questo PC Windows non aggiorna l'ultimo accesso (`fsutil behavior query disablelastaccess` = 2, gestito dal sistema e spento), quindi la verifica non può distinguere; resta la garanzia degli handle (`FILE_READ_ATTRIBUTES`)"
        }
    );
    out.push('\n');
    if same { Ok(()) } else { Err("la simulazione cambia la data di ultimo accesso o di modifica: niente misure su C:".into()) }
}

/// Record di prova per la verifica preliminare: spostamento nel Cestino con il controllo dei riferimenti,
/// così passano anche le letture di `find_live`.
fn references_record() -> Record {
    let mut r = quado_actions::fake::record(quado_model::record::State::YouDecide);
    r.checks = vec![quado_actions::fake::automatic("controllo-riferimenti"), quado_actions::fake::automatic("controllo-restart-manager")];
    r
}

// ---------------------------------------------------------------------------------------------
// Elementi da simulare

/// Un elemento della card da simulare: regola, record, percorsi (con quelli fissi) e nodi del modello.
struct Target {
    rule: &'static str,
    record: Record,
    items: Vec<RequestItem>,
    nodes: Vec<u32>,
}

fn text_arg(m: &quado_kb::engine::Match, name: &str) -> Option<String> {
    match m.arg(name)? {
        Value::Text(t) => Some(t.clone()),
        _ => None,
    }
}

/// Versioni precedenti di CapCut: le cartelle proposte, più la versione in uso e la precedente come
/// elementi fissi (come la conferma le mostra).
fn capcut(kb: &Kb, cls: &quado_kb::Classification, tree: &Tree) -> Option<Target> {
    let m = cls.matches.iter().find(|m| kb.rules[m.rule].id == "capcut-versioni")?;
    let mut items: Vec<RequestItem> = m.nodes.iter().map(|&n| RequestItem { path: tree.path_lossy(n), fixed: None }).collect();
    let apps = m.nodes.first().map(|&n| tree.path_lossy(tree.parent(n)))?;
    for (arg, why) in [("versione", FixedReason::InUseVersion), ("precedente", FixedReason::KeptVersion)] {
        if let Some(v) = text_arg(m, arg) {
            items.push(RequestItem { path: format!(r"{apps}\{v}"), fixed: Some(why) });
        }
    }
    Some(Target { rule: "capcut-versioni", record: m.record.clone(), items, nodes: m.nodes.clone() })
}

/// Il record più grande di `progetti-sviluppo` (allocata nel modello), fuori dalle cartelle escluse.
fn biggest_project(kb: &Kb, cls: &quado_kb::Classification, tree: &Tree) -> Option<Target> {
    let m = cls
        .matches
        .iter()
        .filter(|m| kb.rules[m.rule].id == "progetti-sviluppo")
        .filter(|m| m.nodes.iter().all(|&n| !excluded(&tree.path_lossy(n))))
        .max_by_key(|m| m.nodes.iter().map(|&n| tree.alloc(n)).sum::<u64>())?;
    let items = m.nodes.iter().map(|&n| RequestItem { path: tree.path_lossy(n), fixed: None }).collect();
    Some(Target { rule: "progetti-sviluppo", record: m.record.clone(), items, nodes: m.nodes.clone() })
}

/// Nodi nel sottoalbero di `root` (compreso).
fn subtree(tree: &Tree, root: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        out.push(n);
        stack.extend(tree.children(n).filter(|&c| !tree.flags(c).has(Flags::DETACHED)));
    }
    out
}

/// Fatti del modello sugli elementi: file, cartelle, allocata, allocata degli indici delle cartelle,
/// file compressi WOF (o sparse senza allocata nella voce) e segnaposto, contati.
struct ModelFacts {
    files: u64,
    dirs: u64,
    alloc: u64,
    index: u64,
    unsafe_rm: HashSet<String>,
    wof_or_sparse: u64,
    cloud: u64,
}

fn model_facts(tree: &Tree, nodes: &[u32]) -> ModelFacts {
    let mut f = ModelFacts { files: 0, dirs: 0, alloc: 0, index: 0, unsafe_rm: HashSet::new(), wof_or_sparse: 0, cloud: 0 };
    for &root in nodes {
        f.alloc += tree.alloc(root);
        for n in subtree(tree, root) {
            let fl = tree.flags(n);
            if fl.has(Flags::DIR) {
                f.dirs += 1;
                f.index += tree.dir_extra(n).map_or(0, |e| e.index_alloc);
                continue;
            }
            f.files += 1;
            let wof = fl.has(Flags::ALLOC_FIXED);
            let cloud = fl.has(Flags::CLOUD);
            if wof || cloud {
                f.wof_or_sparse += u64::from(wof);
                f.cloud += u64::from(cloud);
            }
        }
    }
    f
}

// ---------------------------------------------------------------------------------------------
// Resoconto di una simulazione

fn estimate_text(e: &Estimate) -> String {
    match *e {
        Estimate::Exact(b) => format!("esatta, {} GiB ({b} byte)", gib(b)),
        Estimate::Range { min, max } => format!("intervallo {}-{} GiB", gib(min), gib(max)),
        Estimate::NotEstimable(n) => format!("non stimabile ({n:?})"),
    }
}

fn block_name(b: &Block) -> &'static str {
    match b {
        Block::Protected(_) => "protetto",
        Block::NoRecycleBin => "senza Cestino",
        Block::NukeOnDelete => "NukeOnDelete",
        Block::CapacityUnknown => "capacità sconosciuta",
        Block::LargerThanBin { .. } => "più grande del Cestino",
        Block::InUse(_) => "in uso",
        Block::RestartManager(_) => "uso dei file non letto",
        Block::Referenced(_) => "riferimenti",
        Block::LongPath => "percorso lungo",
        Block::ContainsLongPath => "contiene percorsi lunghi",
        Block::Unreadable(_) => "illeggibile",
        Block::Missing => "mancante",
        Block::OtherVolume => "altro volume",
    }
}

/// Blocchi della simulazione: quanti elementi per tipo, con i processi per "in uso".
fn blocks_text(sim: &Simulazione) -> String {
    let mut by: BTreeMap<&str, usize> = BTreeMap::new();
    let mut apps: Vec<String> = Vec::new();
    for it in sim.items() {
        for b in &it.blocks {
            *by.entry(block_name(b)).or_default() += 1;
            if let Block::InUse(u) = b {
                for p in u {
                    if !apps.contains(&p.app_name) {
                        apps.push(p.app_name.clone());
                    }
                }
            }
        }
    }
    if by.is_empty() {
        return "nessuno".into();
    }
    let mut s: Vec<String> = by.iter().map(|(k, n)| format!("{k}: {n}")).collect();
    if !apps.is_empty() {
        s.push(format!("processi: {}", apps.join(", ")));
    }
    s.join("; ")
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    if s.is_empty() {
        0.0
    } else if s.len() % 2 == 1 {
        s[s.len() / 2]
    } else {
        (s[s.len() / 2 - 1] + s[s.len() / 2]) / 2.0
    }
}

/// Allocata e file dei percorsi, da una riscansione: controllo che le simulazioni non abbiano cambiato
/// niente.
fn rescan_totals(paths: &[String]) -> Vec<(u64, u32)> {
    paths
        .iter()
        .map(|p| scan(p, Options::default(), &Progress::default()).map(|r| (r.tree.alloc(Tree::ROOT), r.tree.files(Tree::ROOT))).unwrap_or((0, 0)))
        .collect()
}

/// Cinque simulazioni di `t`, con la tabella dei tempi.
fn measure_target(out: &mut String, tree: &Tree, t: &Target, protected: &[ProtectedRoot], probe: &WinProbe, before: bool) {
    let facts = model_facts(tree, &t.nodes);
    let proposed: Vec<String> = t.items.iter().filter(|i| i.fixed.is_none()).map(|i| i.path.clone()).collect();
    let fixed = t.items.len() - proposed.len();
    let _ = writeln!(out, "## `{}`\n", t.rule);
    let _ = writeln!(
        out,
        "Elementi proposti: {} ({} file e {} cartelle nel modello, {} GiB allocati); elementi fissi mostrati e non letti: {fixed}. Stato del record: {}, eccezione di F-AC-4: {}. Il primo: `{}`.\n",
        proposed.len(),
        facts.files,
        facts.dirs,
        gib(facts.alloc),
        t.record.state.code(),
        if t.record.protected_exception { "sì" } else { "no" },
        proposed.first().map(String::as_str).unwrap_or("-")
    );
    if proposed.iter().any(|p| excluded(p)) {
        let _ = writeln!(out, "Non simulato: dentro una cartella esclusa.\n");
        return;
    }
    let baseline = rescan_totals(&proposed);
    let _ = writeln!(
        out,
        "{} Thread della simulazione (`Probe::threads`): {}. Attributi, elenchi e uso dei file: tempo reale (dalla prima chiamata alla fine dell'ultima), poi la somma dei tempi delle chiamate nei thread.\n",
        if before {
            "Letture come prima delle letture in parallelo (`--prima`): elenco di ogni cartella con `children` (tre aperture), poi gli attributi di ogni voce."
        } else {
            "Letture in parallelo: un'apertura per l'elenco di ogni cartella e gli attributi delle voci nella stessa chiamata (`Probe::entries`, negli elenchi); gli attributi sono quelli degli elementi."
        },
        if before { 1 } else { probe.threads() }
    );
    let _ = writeln!(
        out,
        "| Simulazione | Totale | Attributi | Elenchi | Uso dei file | Riferimenti | Altre letture | File e cartelle letti | Stima | Blocchi |\n|---|---|---|---|---|---|---|---|---|---|"
    );
    let mut totals = Vec::new();
    let mut parts: Vec<[f64; 5]> = Vec::new();
    let mut last: Option<(Simulazione, u64)> = None;
    for run in 0..RUNS {
        let timed = Timed::new(probe, &facts.unsafe_rm, before);
        let progress = SimProgress::default();
        let req = Request { items: t.items.clone(), record: t.record.clone(), protected: protected.to_vec() };
        let start = Instant::now();
        let sim = match simulate(req, &timed, &progress) {
            Ok(s) => s,
            Err(_) => {
                let _ = writeln!(out, "| {} | annullata | | | | | | | | |", run + 1);
                continue;
            }
        };
        let total = ms(start.elapsed());
        totals.push(total);
        parts.push([timed.attrs.wall_ms(), timed.lists.wall_ms(), timed.rm.wall_ms(), timed.refs.ms(), timed.other.ms()]);
        let (files, dirs): (u64, u64) = sim.items().iter().fold((0, 0), |(f, d), it| (f + it.files, d + it.dirs));
        let all: Vec<bool> = sim.items().iter().map(|it| it.fixed.is_none()).collect();
        let _ = writeln!(
            out,
            "| {}{} | {total:.0} ms | {:.0} ms ({:.0} nei thread, {} chiamate) | {:.0} ms ({:.0} nei thread, {}) | {:.0} ms ({:.0} nei thread, {} chiamate, {} file) | {:.0} ms ({}) | {:.0} ms | {files} file, {dirs} cartelle ({} voci) | {} | {} |",
            run + 1,
            if run == 0 { " (dopo la scansione)" } else { "" },
            timed.attrs.wall_ms(),
            timed.attrs.ms(),
            timed.attrs.calls(),
            timed.lists.wall_ms(),
            timed.lists.ms(),
            timed.lists.calls(),
            timed.rm.wall_ms(),
            timed.rm.ms(),
            timed.rm.calls(),
            timed.rm_files.load(Relaxed),
            timed.refs.ms(),
            timed.refs.calls(),
            timed.other.ms(),
            progress.done.load(Relaxed),
            estimate_text(&sim.estimate(&all).estimate),
            blocks_text(&sim)
        );
        eprintln!("{} simulazione {}: {total:.0} ms", t.rule, run + 1);
        last = Some((sim, timed.rm_removed.load(Relaxed)));
    }
    let after = rescan_totals(&proposed);
    let col = |k: usize| median(&parts.iter().map(|p| p[k]).collect::<Vec<_>>());
    let _ = writeln!(
        out,
        "\nMediana di {RUNS}: totale {:.0} ms (minimo {:.0}, massimo {:.0}); tempo reale di attributi {:.0} ms, elenchi {:.0} ms, uso dei file {:.0} ms; riferimenti {:.0} ms, altre letture {:.0} ms.",
        median(&totals),
        totals.iter().copied().fold(f64::INFINITY, f64::min),
        totals.iter().copied().fold(0.0, f64::max),
        col(0),
        col(1),
        col(2),
        col(3),
        col(4)
    );
    if let Some((sim, removed)) = &last {
        let sel = sim.default_selection();
        let chosen = sel.iter().filter(|&&s| s).count();
        let refs = sim.references();
        let _ = writeln!(
            out,
            "Ultima simulazione: selezione proposta {chosen} elementi su {}, stima della selezione proposta {}; riferimenti trovati {}, fonti parziali {:?}; Sensore memoria {:?} giorni; capacità {:?}.",
            proposed.len(),
            estimate_text(&sim.estimate(&sel).estimate),
            refs.found.len(),
            refs.partial,
            sim.storage_sense_days(),
            sim.capacity(&sel)
        );
        let _ = writeln!(
            out,
            "File tolti dalla lettura dell'uso: {removed} per simulazione. Negli elementi: {} file WOF o sparse senza allocata nella voce, {} segnaposto (letti senza decomprimerli né scaricarli).",
            facts.wof_or_sparse, facts.cloud
        );
    }
    let _ = writeln!(
        out,
        "Indici delle cartelle ($I30) nel modello: {} GiB, {:.2}% dell'allocata dei file: la stima li conta, perché lo svuotamento li libera (`fine_vhdx.rs`, `stima_molte_cartelle_piccole`).",
        gib(facts.index),
        if facts.alloc > 0 { facts.index as f64 * 100.0 / facts.alloc as f64 } else { 0.0 }
    );
    let changed: Vec<String> = proposed
        .iter()
        .zip(baseline.iter().zip(&after))
        .filter(|(_, (a, b))| a != b)
        .map(|(p, (a, b))| format!("`{p}` {a:?} → {b:?}"))
        .collect();
    let _ = writeln!(
        out,
        "Riscansione degli elementi prima e dopo le {RUNS} simulazioni (allocata, file): {}.\n",
        if changed.is_empty() { "uguali".to_owned() } else { format!("diverse in {} elementi: {}", changed.len(), changed.join("; ")) }
    );
}

// ---------------------------------------------------------------------------------------------

pub fn run(args: &[String]) -> Result<(), String> {
    // Opzioni della baseline delle prestazioni: solo CapCut, senza la verifica preliminare (che scrive tre
    // file di prova nella cartella temporanea), resoconto in un altro file.
    let only_capcut = args.iter().any(|a| a == "--solo-capcut");
    // Le letture di prima del parallelo (una voce per volta, un thread), per il confronto con lo stesso eseguibile.
    let before = args.iter().any(|a| a == "--prima");
    let no_check = args.iter().any(|a| a == "--senza-verifica");
    let out_path: Option<PathBuf> = args.iter().position(|a| a == "--out").and_then(|k| args.get(k + 1)).map(PathBuf::from);
    let exclude: Vec<String> = args.iter().position(|a| a == "--escludi").and_then(|k| args.get(k + 1)).map(|v| v.split(';').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
    let _ = EXCLUDED.set(exclude);
    let mut out = String::new();
    let lt = system::local_time(system::now_filetime()).ok_or("data")?;
    let _ = writeln!(out, "# M5 · Costo della simulazione su C: ({:02}/{:02}/{})\n", lt.day, lt.month, lt.year);
    let _ = writeln!(
        out,
        "Generato da `cargo xtask m5-misure` (07 §1.6 criterio 4, 05 D7), in sola lettura: scansione, prove, classificazione, Programmi e radici protette come l'app, poi `simulate` con `WinProbe` e la configurazione vera dell'app (`KbConfig`). Mai `conferma`, `esegui` né il mover. Unità: GiB (1024³ byte).\n"
    );
    let _ = writeln!(out, "| Condizione | Valore |\n|---|---|");
    for (k, v) in crate::measure_cmd::environment() {
        let _ = writeln!(out, "| {k} | {v} |");
    }
    out.push('\n');

    if no_check {
        let _ = writeln!(out, "Verifica preliminare dell'ultimo accesso non eseguita (`--senza-verifica`).\n");
    } else {
        eprintln!("verifica dell'ultimo accesso…");
        if let Err(e) = last_access_check(&mut out) {
            write_doc(&out, out_path.as_deref())?;
            return Err(e);
        }
    }

    eprintln!("scansione di C:…");
    let kb = Kb::embedded().map_err(|e| format!("regole: {e:?}"))?;
    let sid = system::user_sid();
    let t0 = Instant::now();
    let r = scan("C:\\", Options::default(), &Progress::default()).map_err(|e| format!("scansione: errore {}", e.0))?;
    let scan_ms = ms(t0.elapsed());
    let tree = r.tree;
    let t = Instant::now();
    let ev = quado_kb::win::gather("C:\\", sid);
    let ev_ms = ms(t.elapsed());
    let t = Instant::now();
    let cls = classify(&kb, &tree, &ev, quado_ui::app::data::today());
    let cls_ms = ms(t.elapsed());
    let t = Instant::now();
    let programs = quado_kb::programs::build(&tree, &ev, &kb, &cls);
    let prog_ms = ms(t.elapsed());
    let t = Instant::now();
    let protected = quado_kb::protection::protected_roots(&ev, &programs);
    let prot_ms = ms(t.elapsed());
    let _ = writeln!(out, "## Preparazione, come l'app\n\n| Fase | Tempo | Risultato |\n|---|---|---|");
    let _ = writeln!(out, "| Scansione di C: | {:.1} s | {} nodi |", scan_ms / 1000.0, tree.len());
    let _ = writeln!(out, "| Prove (`gather`) | {ev_ms:.0} ms | |");
    let _ = writeln!(out, "| Classificazione | {cls_ms:.0} ms | {} corrispondenze |", cls.matches.len());
    let _ = writeln!(out, "| Programmi | {prog_ms:.0} ms | {} programmi |", programs.items.len());
    let _ = writeln!(out, "| Radici protette | {prot_ms:.1} ms | {} radici |\n", protected.len());

    let probe = WinProbe::new(Box::new(KbConfig));
    let mut targets = vec![("capcut-versioni", capcut(&kb, &cls, &tree))];
    if !only_capcut {
        targets.push(("progetti-sviluppo", biggest_project(&kb, &cls, &tree)));
    }
    for (rule, target) in &targets {
        match target {
            Some(t) => {
                eprintln!("{rule}: {} elementi…", t.items.len());
                measure_target(&mut out, &tree, t, &protected, &probe, before);
            }
            None => {
                let _ = writeln!(out, "## `{rule}`\n\nNessun elemento su questo PC: misura non fatta.\n");
            }
        }
    }
    let _ = writeln!(
        out,
        "Le simulazioni girano una dopo l'altra sullo stesso modello; la prima di ciascun elemento è subito dopo la scansione (metadati in cache). La lettura dell'uso riceve tutti gli eseguibili e le DLL di ogni elemento e fino a 1.000 altri file, i più recenti (05 D7)."
    );
    write_doc(&out, out_path.as_deref())?;
    println!("{out}");
    Ok(())
}

fn write_doc(out: &str, to: Option<&Path>) -> Result<(), String> {
    if let Some(p) = to {
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(p, out).map_err(|e| e.to_string())?;
        eprintln!("scritto {}", p.display());
        return Ok(());
    }
    let dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("target").join("misure");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("m5-misure-C.md"), out).map_err(|e| e.to_string())?;
    eprintln!("scritto target/misure/m5-misure-C.md");
    Ok(())
}
