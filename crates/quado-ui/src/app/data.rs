// SPDX-License-Identifier: GPL-3.0-or-later
//! Dati della scansione per l'interfaccia: la scansione in corso, il risultato con gli indici di
//! `quado_model::view`, i lavori in background (ricerca, File grandi, verifica di una voce).

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use quado_kb::programs::{self, Programs};
use quado_kb::{Classification, Evidence, Kb, classify};
use quado_model::record::{ColorGroup, Date, Manager};
use quado_model::view::{self, Filter, Needle, Ordered};
use quado_model::{Balance, Flags, Links, Metadata, NONE, Space, Tree};
use quado_scan_win::helper::proto::{Reply, ShadowStorage};
use quado_scan_win::volumes::{DriveKind, Support, Volume};
use quado_scan_win::{Options, Problem, Progress, RootError, ScanResult, Stats, VolumeFacts, system, win};

use super::helper_jobs::HelperRequest;

/// Volume scansionato, come lo mostra l'interfaccia.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeInfo {
    /// Radice della scansione (`C:\`).
    pub root: String,
    pub label: String,
    pub file_system: String,
    pub kind: DriveKind,
    pub support: Support,
    pub is_system: bool,
}

impl VolumeInfo {
    pub fn from_volume(v: &Volume) -> Option<Self> {
        Some(Self {
            root: v.paths.first()?.clone(),
            label: v.label.clone(),
            file_system: v.file_system.clone(),
            kind: v.kind,
            support: v.support(),
            is_system: v.is_system,
        })
    }

    /// `C:` dalla radice `C:\`.
    pub fn letter(&self) -> String {
        self.root.trim_end_matches('\\').to_owned()
    }
}

/// Scansione in corso in un thread a parte.
pub struct ScanJob {
    pub volume: VolumeInfo,
    pub progress: Arc<Progress>,
    pub started: Instant,
    pub cancel_requested: Option<Instant>,
    rx: Receiver<Result<Loaded, RootError>>,
    /// Solo per `preview`: tiene aperto il canale, così la scansione finta non finisce mai.
    _keep: Option<std::sync::mpsc::Sender<Result<Loaded, RootError>>>,
}

impl ScanJob {
    /// Avvia la scansione. Anche gli indici per le viste (figli ordinati, top list) si costruiscono
    /// in questo thread: con 3,4 milioni di voci richiedono circa 250 ms, che sul thread
    /// dell'interfaccia sarebbero un frame bloccato.
    pub fn start(volume: VolumeInfo, user_sid: Option<String>, kb: Arc<Kb>) -> Self {
        let progress = Arc::new(Progress::default());
        let (tx, rx) = channel();
        let p = Arc::clone(&progress);
        let v = volume.clone();
        std::thread::Builder::new()
            .name("quado-scansione".into())
            .spawn(move || {
                let t0 = Instant::now();
                // Gli estrattori (registro, manifest, configurazione) girano insieme alla scansione.
                let (root, sid) = (v.root.clone(), user_sid.clone());
                let ev = std::thread::Builder::new().name("quado-prove".into()).spawn(move || quado_kb::win::gather(&root, sid));
                let r = quado_scan_win::scan(&v.root, Options::default(), &p).map(|r| {
                    let duration = t0.elapsed();
                    let evidence = ev.ok().and_then(|h| h.join().ok()).unwrap_or_else(|| Evidence::empty(&v.root));
                    Loaded::with_rules(v, r, duration, user_sid.as_deref(), kb, evidence)
                });
                let _ = tx.send(r);
            })
            .expect("thread della scansione");
        Self { volume, progress, started: Instant::now(), cancel_requested: None, rx, _keep: None }
    }

    /// Scansione finta, ferma a un avanzamento dato: per i test e le immagini della schermata.
    pub fn preview(volume: VolumeInfo, progress: Progress, elapsed: Duration) -> Self {
        let (tx, rx) = channel();
        let started = Instant::now().checked_sub(elapsed).unwrap_or_else(Instant::now);
        Self { volume, progress: Arc::new(progress), started, cancel_requested: None, rx, _keep: Some(tx) }
    }

    /// Scansione già finita con `loaded`: per i test di quello che succede all'arrivo del risultato.
    pub fn done(volume: VolumeInfo, loaded: Loaded) -> Self {
        let (tx, rx) = channel();
        let _ = tx.send(Ok(loaded));
        Self { volume, progress: Arc::new(Progress::default()), started: Instant::now(), cancel_requested: None, rx, _keep: Some(tx) }
    }

    pub fn cancel(&mut self) {
        self.progress.cancel();
        self.cancel_requested.get_or_insert_with(Instant::now);
    }

    /// Il risultato, quando la scansione è finita.
    pub fn poll(&mut self) -> Option<Result<Loaded, RootError>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(RootError(1359))),
        }
    }
}

/// Data locale (scadenza delle regole).
pub fn today() -> Date {
    system::local_time(system::now_filetime()).map_or(Date::new(2026, 10, 13), |t| Date::new(t.year, t.month as u8, t.day as u8))
}

/// Codice del gruppo di colore di un nodo, come l'indice di `tokens::Gestore`.
pub fn group_code(cls: &Classification, node: u32) -> u8 {
    match cls.color(node) {
        ColorGroup::Manager(Manager::Programs) => 0,
        ColorGroup::Manager(Manager::You) => 1,
        ColorGroup::Manager(Manager::Windows) => 2,
        ColorGroup::Unclassified => 3,
    }
}

/// "Dove finisce lo spazio" con le unità decise dalle regole: un elemento riconosciuto da una regola che non
/// è un contenitore non si apre (la cartella di un gioco resta un solo elemento, senza scendere alle sue
/// sottocartelle).
pub fn top_list(tree: &Tree, ord: &Ordered, kb: &Kb, cls: &Classification) -> Vec<u32> {
    let closed = |n: u32| cls.own(n).is_some_and(|m| !kb.rules[cls.matches[m].rule].container);
    view::top_elements_with(tree, ord, Tree::ROOT, TOP_N, &closed)
}

/// Albero e figli ordinati, condivisi con i lavori in background.
pub struct Model {
    pub tree: Tree,
    pub ord: Ordered,
}

/// Cartelle non leggibili e cosa sono, per la card dei conti. Solo fatti dal percorso.
#[derive(Clone, Debug, Default)]
pub struct Observed {
    /// Tutte le cartelle con un problema, in ordine di profondità, tranne quelle che l'helper ha misurato
    /// come totale (`Flags::MEASURED`).
    pub unreadable: Vec<(u32, Problem)>,
    /// Cestini di altri account (`$Recycle.Bin\S-1-...` diverso dal SID corrente).
    pub other_bins: usize,
    /// Profili di altri account (`Users\<nome>` non leggibile).
    pub other_profiles: usize,
}

impl Observed {
    pub fn compute(tree: &Tree, problems: &[(u32, Problem)], user_sid: Option<&str>) -> Self {
        let mut unreadable: Vec<(u32, Problem)> =
            problems.iter().copied().filter(|&(n, p)| p != Problem::Cancelled && !tree.flags(n).any(Flags::DETACHED | Flags::MEASURED)).collect();
        unreadable.sort_by_key(|&(n, _)| (tree.depth(n), n));
        let mut o = Self { unreadable, ..Default::default() };
        for &(n, p) in &o.unreadable {
            if p != Problem::AccessDenied {
                continue;
            }
            let parent = tree.parent(n);
            if parent == NONE {
                continue;
            }
            let pname = tree.name_lossy(parent);
            let name = tree.name_lossy(n);
            let top_level = tree.parent(parent) == Tree::ROOT;
            if top_level && pname.eq_ignore_ascii_case("$Recycle.Bin") && name.starts_with("S-1-") && Some(name.as_str()) != user_sid {
                o.other_bins += 1;
            }
            if top_level && pname.eq_ignore_ascii_case("Users") {
                o.other_profiles += 1;
            }
        }
        o
    }
}

/// Cosa ha misurato l'helper elevato su questo modello, per la card dei conti. I nodi restano validi dopo
/// altre modifiche del modello (i nodi sostituiti si staccano, non si rinumerano): la card mostra solo
/// quelli ancora attaccati.
#[derive(Clone, Debug, Default)]
pub struct Elevated {
    /// Cartelle elencate come amministratore: radici dei sottoalberi innestati.
    pub folders: Vec<u32>,
    /// Profili di altri account, misurati come totale.
    pub profiles: Vec<u32>,
    /// Cestini di altri account, misurati come totale: nodo, SID, account che non esiste più.
    pub bins: Vec<(u32, String, bool)>,
    /// Spazio delle copie shadow all'ultima lettura (`None`: mai letto). Non entra nel bilancio: i file
    /// delle copie stanno in `System Volume Information`.
    pub shadow: Option<Result<Option<ShadowStorage>, u32>>,
}

/// Risultato della scansione caricato nell'interfaccia.
pub struct Loaded {
    pub volume: VolumeInfo,
    pub model: Arc<Model>,
    pub links: Links,
    pub problems: Vec<(u32, Problem)>,
    pub stats: Stats,
    pub facts: Option<VolumeFacts>,
    pub balance: Option<Balance>,
    pub cancelled: bool,
    /// Fine della scansione (`FILETIME`) e durata.
    pub finished: i64,
    pub duration: Duration,
    pub top: Vec<u32>,
    pub observed: Observed,
    /// Cresce a ogni modifica del modello: invalida le cache delle viste.
    pub generation: u64,
    /// Regole, prove degli estrattori e classificazione.
    pub kb: Arc<Kb>,
    pub evidence: Arc<Evidence>,
    pub cls: Arc<Classification>,
    /// Vista Programmi, dalle stesse prove e dalla classificazione.
    pub programs: Arc<Programs>,
    /// Misure dell'helper elevato.
    pub elevated: Elevated,
    /// Caratteri non ASCII dei nomi, per i font di ripiego ([`name_chars`]).
    pub name_chars: Arc<Vec<char>>,
}

pub const TOP_N: usize = 10;

/// Caratteri non ASCII dei nomi dei nodi da `from` in poi, uniti a `base`: ordinati, senza ripetizioni e
/// senza caratteri di controllo, come li disegna l'interfaccia (surrogati spaiati come U+FFFD). Il
/// controllo dei font di ripiego guarda solo questi, sul thread dell'interfaccia, invece di tutti i nomi.
pub fn name_chars(tree: &Tree, from: usize, base: &[char]) -> Arc<Vec<char>> {
    let mut seen: std::collections::BTreeSet<char> = base.iter().copied().collect();
    for i in from..tree.len() {
        if let quado_model::Name::Wide(w) = tree.name(i as u32) {
            for c in char::decode_utf16(w.iter().copied()).map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER)) {
                if !c.is_ascii() && !c.is_control() {
                    seen.insert(c);
                }
            }
        }
    }
    Arc::new(seen.into_iter().collect())
}

impl Loaded {
    /// Senza prove degli estrattori: valgono solo le regole sui percorsi (test e verifiche).
    pub fn new(volume: VolumeInfo, r: ScanResult, duration: Duration, user_sid: Option<&str>) -> Self {
        let kb = Arc::new(Kb::embedded().unwrap_or(Kb { rules: Vec::new() }));
        let mut ev = Evidence::empty(&volume.root);
        ev.user_sid = user_sid.map(str::to_owned);
        Self::with_rules(volume, r, duration, user_sid, kb, ev)
    }

    pub fn with_rules(volume: VolumeInfo, r: ScanResult, duration: Duration, user_sid: Option<&str>, kb: Arc<Kb>, evidence: Evidence) -> Self {
        let ScanResult { tree, links, problems, stats, volume: facts, balance, cancelled } = r;
        // Indici delle viste e classificazione insieme: dopo un annullamento il risultato parziale
        // arriva prima (obiettivo 500 ms).
        let (ord, (cls, programs), chars) = std::thread::scope(|s| {
            let c = s.spawn(|| {
                let cls = classify(&kb, &tree, &evidence, today());
                let p = programs::build(&tree, &evidence, &kb, &cls);
                (cls, p)
            });
            let ch = s.spawn(|| name_chars(&tree, 0, &[]));
            let ord = Ordered::build(&tree);
            // Un errore nel motore non deve perdere la scansione: senza classificazione e senza programmi.
            let empty = || (Classification::default(), Programs { items: Vec::new(), residues: Vec::new(), errors: Vec::new(), tracking: quado_kb::extract::TrackProgs::Absent });
            (ord, c.join().unwrap_or_else(|_| empty()), ch.join().unwrap_or_default())
        });
        let top = top_list(&tree, &ord, &kb, &cls);
        let observed = Observed::compute(&tree, &problems, user_sid);
        Self {
            volume,
            model: Arc::new(Model { tree, ord }),
            links,
            problems,
            stats,
            facts,
            balance,
            cancelled,
            finished: system::now_filetime(),
            duration,
            top,
            observed,
            generation: 1,
            kb,
            evidence: Arc::new(evidence),
            cls: Arc::new(cls),
            programs: Arc::new(programs),
            elevated: Elevated::default(),
            name_chars: chars,
        }
    }

    pub fn tree(&self) -> &Tree {
        &self.model.tree
    }

    pub fn ord(&self) -> &Ordered {
        &self.model.ord
    }

    /// Spazio usato secondo Windows a fine scansione: denominatore delle quote ("% dello spazio usato").
    pub fn used(&self) -> u64 {
        self.balance.map_or_else(|| self.tree().alloc(Tree::ROOT), |b| b.used).max(1)
    }

    /// Problema registrato su una cartella, se c'è.
    pub fn problem(&self, node: u32) -> Option<Problem> {
        self.problems.iter().find(|&&(n, _)| n == node).map(|&(_, p)| p)
    }
}

/// Lavoro in background con un solo risultato.
pub struct Task<T> {
    rx: Receiver<T>,
    pub started: Instant,
}

impl<T: Send + 'static> Task<T> {
    pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("quado-lavoro".into())
            .spawn(move || {
                let _ = tx.send(f());
            })
            .expect("thread di lavoro");
        Self { rx, started: Instant::now() }
    }

    pub fn poll(&mut self) -> Option<T> {
        self.rx.try_recv().ok()
    }

    /// Attende la fine (per i test e prima di modificare il modello).
    pub fn wait(self) -> Option<T> {
        self.rx.recv().ok()
    }
}

/// Esito di una ricerca: la query a cui risponde e i nodi trovati.
pub struct SearchOutcome {
    pub query: (String, Filter, u32),
    pub hits: Vec<u32>,
    pub elapsed: Duration,
    pub generation: u64,
}

/// Avvia una ricerca sul modello. Il thread rilascia il modello prima di consegnare il risultato,
/// così chi aspetta il risultato può poi modificare il modello senza copiarlo.
pub fn spawn_search(model: &Arc<Model>, cls: &Arc<Classification>, text: String, filter: Filter, root: u32, generation: u64) -> Task<SearchOutcome> {
    let m = Arc::clone(model);
    let c = Arc::clone(cls);
    Task::spawn(move || {
        let t0 = Instant::now();
        let g = |i: u32| group_code(&c, i);
        let hits = view::search(&m.tree, root, &Needle::new(&text), &filter, Some(&g));
        drop(m);
        SearchOutcome { query: (text, filter, root), hits, elapsed: t0.elapsed(), generation }
    })
}

pub struct LargestOutcome {
    pub filter: Filter,
    pub files: Vec<u32>,
    pub generation: u64,
}

/// Quanti file mostra la vista File grandi.
pub const LARGEST_LIMIT: usize = 10_000;

pub fn spawn_largest(model: &Arc<Model>, cls: &Arc<Classification>, filter: Filter, generation: u64) -> Task<LargestOutcome> {
    let m = Arc::clone(model);
    let c = Arc::clone(cls);
    Task::spawn(move || {
        let g = |i: u32| group_code(&c, i);
        let files = view::largest_files(&m.tree, &filter, LARGEST_LIMIT, Some(&g));
        drop(m);
        LargestOutcome { filter, files, generation }
    })
}

/// Esito di "Rileggi dal disco", da applicare al modello sul thread dell'interfaccia.
pub enum Verified {
    /// File: nuove dimensioni lette con `FileStandardInfo`.
    File { node: u32, logical: u64, alloc: u64 },
    /// Cartella: riscansione del sottoalbero.
    Dir { node: u32, result: Box<ScanResult> },
    /// Non si è potuto aprire: codice di errore di Windows.
    Failed { node: u32, code: u32 },
}

/// Legge di nuovo una voce, in sola lettura e senza aprirne il contenuto.
pub fn spawn_verify(path: String, node: u32, is_dir: bool) -> Task<Verified> {
    Task::spawn(move || {
        if is_dir {
            match quado_scan_win::scan(&path, Options::default(), &Progress::default()) {
                Ok(result) => Verified::Dir { node, result: Box::new(result) },
                Err(RootError(code)) => Verified::Failed { node, code },
            }
        } else {
            let w: Vec<u16> = win::long_path(&path.encode_utf16().collect::<Vec<u16>>());
            match win::open_attributes(&w).and_then(|h| win::standard_info(&h)) {
                Ok(s) => Verified::File { node, logical: s.logical, alloc: s.alloc },
                Err(code) => Verified::Failed { node, code },
            }
        }
    })
}

/// Esito mostrato nell'ispettore dopo la verifica.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyReport {
    pub node: u32,
    pub before: u64,
    pub after: Option<u64>,
    pub error: Option<u32>,
    pub at: i64,
}

/// Applica una verifica al modello. Restituisce il nodo aggiornato (nuovo indice dopo una riscansione).
pub fn apply_verified(loaded: &mut Loaded, v: Verified, user_sid: Option<&str>) -> VerifyReport {
    let at = system::now_filetime();
    let old_len = loaded.tree().len();
    let model = Arc::get_mut(&mut loaded.model).expect("nessun lavoro in background sul modello");
    let mut changed = Vec::new();
    let report = match v {
        Verified::File { node, logical, alloc } => {
            let before = model.tree.alloc(node);
            model.tree.update_file(node, logical, alloc);
            changed.push(node);
            VerifyReport { node, before, after: Some(alloc), error: None, at }
        }
        Verified::Dir { node, result } => {
            let before = model.tree.alloc(node);
            let (new_root, map) = model.tree.graft(node, &result.tree);
            changed.extend([node, new_root]);
            // I problemi del vecchio sottoalbero restano sui nodi staccati e si filtrano; quelli nuovi si aggiungono.
            for &(n, p) in &result.problems {
                if n != Tree::ROOT {
                    loaded.problems.push((map[n as usize], p));
                }
            }
            VerifyReport { node: new_root, before, after: Some(model.tree.alloc(new_root)), error: None, at }
        }
        Verified::Failed { node, code } => VerifyReport { node, before: model.tree.alloc(node), after: None, error: Some(code), at },
    };
    rebuild(loaded, user_sid, old_len, &changed);
    report
}

/// Il panico di un thread di `rebuild` continua nel thread dell'interfaccia, come quando giravano lì.
fn resume<T>(e: Box<dyn std::any::Any + Send>) -> T {
    std::panic::resume_unwind(e)
}

/// Dopo una modifica del modello: indici delle viste, classificazione, top list, programmi, cartelle
/// non leggibili, caratteri dei nomi, generazione. `old_len` sono i nodi prima della modifica e `changed` i
/// nodi toccati (vedi `Ordered::update`): senza modifiche non si rifà niente.
///
/// I figli ordinati si aggiornano solo per le cartelle toccate (prima: `Ordered::build` di tutto l'albero,
/// 220-260 ms con C:) mentre classificazione e programmi girano in un altro thread: il frame dura quanto il
/// più lungo dei due, con gli stessi risultati di una ricostruzione completa.
fn rebuild(loaded: &mut Loaded, user_sid: Option<&str>, old_len: usize, changed: &[u32]) {
    if changed.is_empty() {
        return;
    }
    let (ord, (cls, programs), chars) = {
        let model = &*loaded.model;
        let (kb, evidence, base) = (&loaded.kb, &loaded.evidence, &loaded.name_chars);
        std::thread::scope(|s| {
            // La classificazione segue il modello: i nodi riscansionati hanno indici nuovi.
            let c = s.spawn(|| {
                let cls = classify(kb, &model.tree, evidence, today());
                let p = programs::build(&model.tree, evidence, kb, &cls);
                (cls, p)
            });
            // I nomi dei nodi staccati restano tra i caratteri: come prima, quando si guardavano tutti i nomi.
            let ch = s.spawn(|| name_chars(&model.tree, old_len, base));
            let ord = model.ord.update(&model.tree, old_len, changed);
            (ord, c.join().unwrap_or_else(resume), ch.join().unwrap_or_else(resume))
        })
    };
    let model = Arc::get_mut(&mut loaded.model).expect("nessun lavoro in background sul modello");
    model.ord = ord;
    let model = &*loaded.model;
    loaded.top = top_list(&model.tree, &model.ord, &loaded.kb, &cls);
    loaded.programs = Arc::new(programs);
    loaded.cls = Arc::new(cls);
    loaded.observed = Observed::compute(&model.tree, &loaded.problems, user_sid);
    loaded.name_chars = chars;
    loaded.generation += 1;
}

/// Esito di `apply_helper`, per la riga di riepilogo della card dei conti.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HelperSummary {
    /// Cartelle chieste.
    pub asked: usize,
    /// Cartelle applicate al modello: elencate (sottoalbero) o misurate come totale.
    pub applied: usize,
    /// Cartelle che neanche l'helper ha aperto: percorso e codice di Windows. Restano negate.
    pub failed: Vec<(String, u32)>,
    /// Risposte senza la loro cartella nel modello di adesso: ignorate.
    pub ignored: usize,
    /// `SeBackupPrivilege` abilitato (`Started`); `None` se `Started` non è arrivato.
    pub backup_privilege: Option<bool>,
    /// Codice di `Done`: 0 se l'helper non ha avuto errori gravi.
    pub code: u32,
    /// Vecchio nodo e nuova radice di ogni cartella innestata.
    pub grafted: Vec<(u32, u32)>,
}

/// Cartella senza figli: può prendere i totali misurati (`Tree::set_measured`).
fn measurable(tree: &Tree, n: u32) -> bool {
    tree.is_dir(n) && tree.children(n).next().is_none()
}

/// Applica le risposte dell'helper elevato al modello, sul thread dell'interfaccia, all'arrivo di `Done`:
/// - `Folder`: il sottoalbero sostituisce la cartella negata (`Tree::graft`), con i suoi problemi;
/// - `Totals` (profilo altrui) e `Bin` (Cestino altrui): la cartella prende i totali misurati, senza figli
///   e senza nomi (`Tree::set_measured`);
/// - `Failed`: la cartella resta negata, il codice va nel riepilogo;
/// - `Started`, `Shadow`, `Done`: nel riepilogo e nei fatti della card ([`Elevated`]).
///
/// Poi classificazione, viste, cartelle non leggibili e generazione (`rebuild`) e il bilancio con il nuovo
/// allocato letto e gli indici delle cartelle innestate: il non attribuito scende esattamente dei byte
/// letti. Le copie shadow non entrano nel bilancio.
///
/// I nodi sono quelli della richiesta se il modello è lo stesso di allora; altrimenti si ritrovano dal
/// percorso, e una cartella che non c'è più fa ignorare la sua risposta. Una risposta ripetuta per la
/// stessa cartella si ignora. Chiede che nessun lavoro in background tenga il modello
/// (`App::wait_background`).
pub fn apply_helper(loaded: &mut Loaded, sent: &HelperRequest, replies: Vec<Reply>, user_sid: Option<&str>) -> HelperSummary {
    use super::intervention_jobs::find_node;
    let mut s = HelperSummary { asked: sent.nodes.len(), ..HelperSummary::default() };
    let same_volume = loaded.volume.root.eq_ignore_ascii_case(&sent.request.volume);
    let same_model = same_volume && loaded.finished == sent.finished && loaded.generation == sent.generation;
    let mut answered: HashSet<u32> = HashSet::new();
    let old_len = loaded.tree().len();
    let mut changed: Vec<u32> = Vec::new();
    {
        let model = Arc::get_mut(&mut loaded.model).expect("nessun lavoro in background sul modello");
        let tree = &mut model.tree;
        // Nodo di adesso della cartella `index`, se c'è ancora e non ha già avuto una risposta.
        let mut node_of = |tree: &Tree, index: u32| -> Option<u32> {
            if !same_volume || !answered.insert(index) {
                return None;
            }
            let k = index as usize;
            let n = if same_model { *sent.nodes.get(k)? } else { find_node(tree, sent.request.folders.get(k)?)? };
            (n != Tree::ROOT && (n as usize) < tree.len() && !tree.flags(n).has(Flags::DETACHED)).then_some(n)
        };
        for r in replies {
            match r {
                Reply::Started { backup_privilege } => s.backup_privilege = Some(backup_privilege),
                Reply::Folder { index, tree: sub, problems } => {
                    let Some(n) = node_of(tree, index).filter(|_| !sub.is_empty()) else {
                        s.ignored += 1;
                        continue;
                    };
                    let (root, map) = tree.graft(n, &sub);
                    // Come in `apply_verified`: i problemi del sottoalbero, tranne quello della sua radice.
                    for (k, p) in problems {
                        if k != Tree::ROOT
                            && let Some(&m) = map.get(k as usize)
                        {
                            loaded.problems.push((m, p));
                        }
                    }
                    loaded.elevated.folders.push(root);
                    s.grafted.push((n, root));
                    s.applied += 1;
                    changed.extend([n, root]);
                }
                Reply::Totals { index, totals } => {
                    let Some(n) = node_of(tree, index).filter(|&n| measurable(tree, n)) else {
                        s.ignored += 1;
                        continue;
                    };
                    tree.set_measured(n, totals.logical, totals.alloc, u32::try_from(totals.files).unwrap_or(u32::MAX));
                    if !loaded.elevated.profiles.contains(&n) {
                        loaded.elevated.profiles.push(n);
                    }
                    s.applied += 1;
                    changed.push(n);
                }
                Reply::Bin { index, sid, totals, orphan } => {
                    let Some(n) = node_of(tree, index).filter(|&n| measurable(tree, n)) else {
                        s.ignored += 1;
                        continue;
                    };
                    tree.set_measured(n, totals.logical, totals.alloc, u32::try_from(totals.files).unwrap_or(u32::MAX));
                    loaded.elevated.bins.retain(|b| b.0 != n);
                    loaded.elevated.bins.push((n, sid, orphan));
                    s.applied += 1;
                    changed.push(n);
                }
                Reply::Failed { index, code } => {
                    // Conta come risposta per quella cartella: un'altra dopo si ignora.
                    let _ = node_of(tree, index);
                    if let Some(p) = sent.request.folders.get(index as usize) {
                        s.failed.push((p.clone(), code));
                    }
                }
                Reply::Shadow(r) => loaded.elevated.shadow = Some(r),
                Reply::Done { code } => s.code = code,
            }
        }
    }
    if !changed.is_empty() {
        rebuild(loaded, user_sid, old_len, &changed);
        recompute_balance(loaded);
    }
    s
}

/// Il bilancio dopo una modifica dell'allocato letto (helper): stesso usato, stessi metadati misurati
/// salvo gli indici delle cartelle, che seguono il modello. Senza i fatti del volume, gli spazi di
/// inizio e fine si ricavano dal bilancio precedente.
fn recompute_balance(loaded: &mut Loaded) {
    let Some(b) = loaded.balance else { return };
    let (start, end) = match loaded.facts {
        Some(f) => (f.space_start, f.space_end),
        None => {
            let free = b.capacity.saturating_sub(b.used);
            let start_free = b.capacity.saturating_sub(b.used_start);
            (
                Space { capacity: b.capacity, free: start_free, free_to_caller: start_free },
                Space { capacity: b.capacity, free, free_to_caller: free.saturating_sub(b.quota.unwrap_or(0)) },
            )
        }
    };
    let t = loaded.tree();
    let metadata = Metadata { dir_indexes: t.dir_index_total(), ..b.metadata };
    loaded.balance = Some(Balance::compute(start, end, t.alloc(Tree::ROOT), metadata, loaded.links.extra_alloc));
}

/// Esito di `apply_rescans`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// Nuovi nodi delle cartelle riscansionate.
    pub nodes: Vec<u32>,
    /// Cartelle che non esistono più: staccate dal modello.
    pub removed: usize,
    /// Cartelle non lette (percorso e codice di Windows): il modello resta com'era per loro.
    pub failed: Vec<(String, u32)>,
    /// Tra le cartelle c'era la radice del volume: serve una scansione completa (`App::start_scan`).
    pub needs_full_scan: bool,
}

/// Applica le riscansioni dei sottoalberi toccati da un intervento: ogni cartella si ritrova per
/// percorso nel modello di adesso e si sostituisce con la sua riscansione (nodi vecchi staccati, nuovi
/// aggiunti, antenati riaggregati); una cartella sparita si stacca. Classificazione e viste si ricostruiscono
/// una volta sola. Chiede che nessun lavoro in background tenga il modello (`App::wait_background`).
pub fn apply_rescans(loaded: &mut Loaded, rescans: Vec<super::intervention_jobs::Rescanned>, user_sid: Option<&str>) -> Applied {
    use super::intervention_jobs::find_node;
    let mut out = Applied::default();
    let old_len = loaded.tree().len();
    let mut changed: Vec<u32> = Vec::new();
    {
        let model = Arc::get_mut(&mut loaded.model).expect("nessun lavoro in background sul modello");
        for r in rescans {
            let Some(node) = find_node(&model.tree, &r.path) else { continue };
            if node == Tree::ROOT {
                out.needs_full_scan = true;
                continue;
            }
            match r.result {
                Ok(result) => {
                    let (new_root, map) = model.tree.graft(node, &result.tree);
                    for &(n, p) in &result.problems {
                        if n != Tree::ROOT {
                            loaded.problems.push((map[n as usize], p));
                        }
                    }
                    out.nodes.push(new_root);
                    changed.extend([node, new_root]);
                }
                Err(2 | 3) => {
                    model.tree.detach(node);
                    out.removed += 1;
                    changed.push(node);
                }
                Err(code) => out.failed.push((r.path, code)),
            }
        }
    }
    rebuild(loaded, user_sid, old_len, &changed);
    out
}

/// Coda di messaggi brevi per la barra di stato (percorso copiato, errore della shell).
#[derive(Default)]
pub struct Notice {
    pub text: Mutex<Option<(String, Instant)>>,
}

impl Notice {
    pub fn set(&self, s: String) {
        if let Ok(mut t) = self.text.lock() {
            *t = Some((s, Instant::now()));
        }
    }

    pub fn current(&self) -> Option<String> {
        let t = self.text.lock().ok()?;
        t.as_ref().filter(|(_, at)| at.elapsed() < Duration::from_secs(6)).map(|(s, _)| s.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::intervention_jobs::{Rescanned, find_node};
    use crate::app::synthetic;
    use quado_model::{Links, NodeData};

    const SID: &str = "S-1-5-21-1-2-3-1001";

    /// Dopo una modifica del modello, indici e dati derivati sono quelli di una ricostruzione completa:
    /// figli ordinati (`Ordered::update`), caratteri dei nomi, classificazione, top list, programmi.
    fn assert_like_a_full_rebuild(d: &Loaded) {
        assert_eq!(*d.ord(), Ordered::build(d.tree()), "figli ordinati");
        assert_eq!(*d.name_chars, *name_chars(d.tree(), 0, &[]), "caratteri dei nomi");
        let cls = classify(&d.kb, d.tree(), &d.evidence, today());
        assert_eq!(d.top, top_list(d.tree(), d.ord(), &d.kb, &cls), "top list");
        assert_eq!(d.cls.matches.len(), cls.matches.len());
        assert!((0..d.tree().len() as u32).all(|n| d.cls.match_of(n) == cls.match_of(n)), "classificazione");
        let p = programs::build(d.tree(), &d.evidence, &d.kb, &cls);
        assert_eq!(d.programs.items.len(), p.items.len());
        assert_eq!(d.observed.unreadable, Observed::compute(d.tree(), &d.problems, Some(SID)).unreadable);
    }

    /// Una riscansione finta di `path`: due file e una cartella con un nome in giapponese.
    fn rescan_of(path: &str, bytes: u64) -> ScanResult {
        let w = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
        let mut t = Tree::with_root(&w(path), 0);
        let d = t.push(Tree::ROOT, &w("日本語"), NodeData { flags: Flags::DIR, ..Default::default() });
        t.push(d, &w("a.bin"), NodeData { logical: bytes, alloc: bytes, ..Default::default() });
        t.push(Tree::ROOT, &w("b.bin"), NodeData { logical: 7, alloc: 4096, ..Default::default() });
        t.aggregate();
        t.seal();
        ScanResult { tree: t, links: Links::default(), problems: Vec::new(), stats: Stats::default(), volume: None, balance: None, cancelled: false }
    }

    #[test]
    fn model_changes_give_the_indexes_of_a_full_rebuild() {
        let (v, r) = synthetic::sample_c(false);
        let kb = Arc::new(Kb::embedded().expect("regole"));
        let mut d = Loaded::with_rules(v, r, Duration::from_secs(1), Some(SID), kb, synthetic::sample_c_evidence());
        assert_like_a_full_rebuild(&d);
        // "Rileggi dal disco" su un file, che diventa il più grande del volume.
        let g = d.generation;
        let file = (1..d.tree().len() as u32).find(|&i| !d.tree().is_dir(i) && d.tree().alloc(i) > 0).expect("un file");
        apply_verified(&mut d, Verified::File { node: file, logical: 1 << 40, alloc: 1 << 40 }, Some(SID));
        assert!(d.generation > g);
        assert_eq!(d.ord().children(Tree::ROOT)[0], top_ancestor(d.tree(), file), "la cartella del file sale in cima");
        assert_like_a_full_rebuild(&d);
        // "Rileggi dal disco" su una cartella: sottoalbero innestato, con un nome in un'altra scrittura.
        let capcut = r"C:\Users\demo\AppData\Local\CapCut";
        let node = find_node(d.tree(), capcut).expect("CapCut");
        apply_verified(&mut d, Verified::Dir { node, result: Box::new(rescan_of(capcut, 5 << 30)) }, Some(SID));
        assert!(d.name_chars.contains(&'語'));
        assert_like_a_full_rebuild(&d);
        // Riscansioni di un intervento: una cartella sparita e una riletta.
        let gone = r"C:\Users\demo\Downloads";
        let again = r"C:\Users\demo\AppData\Local\CapCut\日本語";
        let rescans = vec![Rescanned { path: gone.into(), result: Err(3) }, Rescanned { path: again.into(), result: Ok(Box::new(rescan_of(again, 1 << 20))) }];
        let applied = apply_rescans(&mut d, rescans, Some(SID));
        assert_eq!((applied.removed, applied.nodes.len()), (1, 1));
        assert!(find_node(d.tree(), gone).is_none());
        assert_like_a_full_rebuild(&d);
        // Una verifica non riuscita non cambia niente: nemmeno la generazione.
        let g = d.generation;
        apply_verified(&mut d, Verified::Failed { node: file, code: 5 }, Some(SID));
        assert_eq!(d.generation, g);
        assert_like_a_full_rebuild(&d);
    }

    /// Il figlio della radice che contiene `n`.
    fn top_ancestor(t: &Tree, mut n: u32) -> u32 {
        while t.parent(n) != Tree::ROOT {
            n = t.parent(n);
        }
        n
    }

    #[test]
    fn observed_lists_from_the_fixture() {
        // Cartelle negate con il loro numero, cestini di altri SID, profili altrui.
        let (_, r) = synthetic::sample_c(false);
        let o = Observed::compute(&r.tree, &r.problems, Some("S-1-5-21-1-2-3-1001"));
        assert_eq!(o.unreadable.len(), 4);
        assert_eq!(o.other_bins, 1, "il cestino dell'utente corrente non conta");
        assert_eq!(o.other_profiles, 1);
        // Senza SID noto ogni cestino S-1-... negato conta come di un altro account.
        let o = Observed::compute(&r.tree, &r.problems, None);
        assert_eq!(o.other_bins, 1);
    }
}
