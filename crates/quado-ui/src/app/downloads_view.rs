// SPDX-License-Identifier: GPL-3.0-or-later
//! Lente Download: famiglie secondo l'estensione su `Downloads` o su un'altra cartella dell'utente, il
//! suffisso "(N)" come solo filtro, "Cosa si capisce di questa cartella" con uno stato per voce, il dettaglio
//! della voce con fatti, file e cosa si può fare.
//!
//! - Aprire la lente non legge il contenuto di nessun file: famiglie, gruppi della stessa
//!   dimensione e archivi con la cartella accanto vengono dal modello della scansione. La lente si
//!   costruisce in un thread (`build_view`), così una cartella con 100.000 file non ferma il frame.
//! - Restart Manager sui download incompleti parte da solo quando la lente è pronta: non legge il contenuto,
//!   non cambia la data di ultimo accesso (la cambiano solo le letture del contenuto: risorsa di versione,
//!   MSI, zip e impronte), costa pochi millisecondi in background, e lo stato di quei file dipende solo da
//!   lui. È una fotografia: la voce dice l'ora del controllo e "Controlla di nuovo l'uso" lo ripete.
//! - Le letture del contenuto (impronte, dati di versione, indici degli zip) partono solo da un comando
//!   che prima dice quanti file e quanti byte legge e che "aggiorna la data di ultimo accesso di questi
//!   file", e chiede conferma. Girano in `lens_jobs`, annullabili, con l'avanzamento.
//! - Le azioni sui file partono dai pulsanti sotto il record, come nell'ispettore: lo spostamento nel Cestino
//!   passa sempre dalla simulazione e dal dialogo di conferma.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Align, Color32, CornerRadius, EventFilter, Frame, Id, Layout, Margin, Rect, ScrollArea, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use quado_actions::shell;
use quado_kb::engine;
use quado_kb::extract::{ExtractError, Uninstall};
use quado_kb::installers::{self, ContentKey, InstallerCache, InstallerInfo, InstallerKind};
use quado_kb::lens::{self, Finding, InstallerMatch, InstallerRead, InstallerStatus, LensFinding, LensInput, UseCheck, UseStatus};
use quado_model::downloads::{self, ArchiveCheck, ArchivePair, GroupComparison, Hashed, Lens, LensFamily, SizeGroup};
use quado_model::record::{Estimate, Record, State};
use quado_model::{Flags, Tree};
use quado_scan_win::content::{ContentError, FileIdentity};

use super::data::{Loaded, Model, Task};
use super::inspector::{self, KbRequest};
use super::lens_jobs::{self, Batch, Readers, Target};
use super::{App, kbtext, table};
use crate::format::Formatter;
use crate::i18n::Lingue;
use crate::icons::{self, Icon};
use crate::tokens::{self, Palette, Type};
use crate::widgets::{self, ButtonKind, Evidence};

/// File elencati per voce nel dettaglio; gli altri si contano.
const MAX_FILES: usize = 30;
/// Righe dell'elenco disegnate per volta, le più grandi: dopo un confronto su una cartella con centinaia
/// di migliaia di file le copie identiche sono migliaia di righe, e disegnarle tutte a ogni frame costa
/// secondi (misurato sulla cartella Download del PC di sviluppo). "Mostra altre voci" ne aggiunge altre.
pub const ROWS_PAGE: usize = 200;
/// Voci mostrate nel dettaglio di una riga che ne raccoglie molte.
const MAX_VOCI: usize = 40;

// ---------------------------------------------------------------------------------------------
// Lente costruita

/// Letture fatte su richiesta in questa sessione, per nodo del modello caricato. Una nuova scansione le
/// azzera (i nodi cambiano); "Rileggi dal disco" le lascia: i nodi staccati non sono più nella lente e
/// un file che ha cambiato dimensione non vale più come confrontato (`GroupComparison::Changed`).
#[derive(Clone, Debug, Default)]
pub struct Readings {
    pub hashed: Vec<Hashed>,
    /// Copie da tenere scelte dall'utente (`LensInput::keep`).
    pub keep: Vec<u32>,
    pub installers: HashMap<u32, Result<InstallerInfo, ExtractError>>,
    pub archives: HashMap<u32, Result<ArchiveCheck, ExtractError>>,
    pub usage: HashMap<u32, UseCheck>,
}

/// Una riga dell'elenco: le voci con la stessa `Finding` (installatori con lo stesso esito, gruppi della
/// stessa dimensione, download incompleti con lo stesso uso). Le copie identiche e gli archivi restano
/// una riga per voce, perché ognuna ha la sua prova e la sua scelta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub finding: Finding,
    /// Indici in `LensView::findings`, in ordine di byte.
    pub voci: Vec<usize>,
    pub bytes: u64,
    pub files: usize,
}

/// La lente su una cartella, costruita fuori dal frame con le letture di una versione.
pub struct LensView {
    pub generation: u64,
    pub root: u32,
    /// Versione delle letture usate.
    pub version: u64,
    pub lens: Lens,
    pub findings: Vec<LensFinding>,
    pub rows: Vec<Row>,
    /// Gruppi della stessa dimensione (i soli file che "Confronta il contenuto" legge) e il loro esito.
    pub candidates: Vec<SizeGroup>,
    pub compared: Vec<GroupComparison>,
    /// File solo nel cloud con la stessa dimensione di un altro: non si leggono mai (leggerli li scaricherebbe).
    pub cloud_same_size: usize,
    pub pairs: Vec<ArchivePair>,
    /// File con il suffisso " (N)" nel nome, in ordine di nodo.
    pub suffixed: Vec<u32>,
    /// I file più grandi di ogni famiglia, per il dettaglio della famiglia.
    pub family_top: HashMap<LensFamily, Vec<u32>>,
    /// Installatori che nessun lettore sa leggere (MSIX, APPX, MSP).
    pub other_installers: usize,
    /// Data di creazione dei file letti dal confronto (copia da tenere), calcolata qui e non a ogni frame.
    pub created: HashMap<u32, i64>,
    pub elapsed: Duration,
}

/// Costruisce la lente su `root` con le letture date. Nessuna lettura di file: modello, prove della
/// scansione (voci di disinstallazione per la corrispondenza degli installatori) e letture già fatte.
pub fn build_view(model: &Model, root: u32, generation: u64, programs: Option<&Uninstall>, readings: &Readings, version: u64) -> LensView {
    let t0 = Instant::now();
    let tree = &model.tree;
    let lens = downloads::build(tree, root);
    let mut input = LensInput {
        hashed: readings.hashed.clone(),
        keep: readings.keep.clone(),
        archives: readings.archives.clone(),
        usage: readings.usage.clone(),
        ..Default::default()
    };
    for (&node, info) in &readings.installers {
        let matched = match (info, programs) {
            (Ok(i), Some(u)) => Some(InstallerMatch::compute(i, u)),
            _ => None,
        };
        input.installers.insert(node, InstallerRead { info: info.clone(), matched });
    }
    let findings = lens::findings(tree, &lens, &input);
    let candidates = downloads::same_size_groups(&lens);
    let compared = downloads::compare_groups(&candidates, &readings.hashed);
    let pairs = downloads::archive_pairs(tree, &lens);
    let suffixed = lens.items.iter().filter(|i| i.copy_suffix.is_some()).map(|i| i.node).collect();
    let mut family_top: HashMap<LensFamily, Vec<u32>> = HashMap::new();
    for fam in LensFamily::ALL {
        let mut v: Vec<(u64, u32)> = lens.of_family(fam).map(|i| (i.alloc, i.node)).collect();
        v.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        family_top.insert(fam, v.into_iter().take(MAX_FILES).map(|x| x.1).collect());
    }
    let other_installers = lens.of_family(LensFamily::Installers).filter(|i| installers::installer_kind(&tree.name_lossy(i.node)).is_none()).count();
    let rows = rows(&findings);
    LensView {
        generation,
        root,
        version,
        cloud_same_size: cloud_same_size(&lens),
        lens,
        findings,
        rows,
        candidates,
        compared,
        pairs,
        suffixed,
        family_top,
        other_installers,
        created: readings.hashed.iter().map(|h| (h.node, h.created)).collect(),
        elapsed: t0.elapsed(),
    }
}

/// Righe dell'elenco: voci uguali per `Finding` insieme, per byte decrescenti (a parità, l'ordine delle
/// voci).
fn rows(findings: &[LensFinding]) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for (k, f) in findings.iter().enumerate() {
        match rows.iter_mut().find(|r| r.finding == f.finding) {
            Some(r) => {
                r.voci.push(k);
                r.bytes += f.bytes;
                r.files += f.nodes.len();
            }
            None => rows.push(Row { finding: f.finding, voci: vec![k], bytes: f.bytes, files: f.nodes.len() }),
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.bytes));
    rows
}

/// Segnaposto cloud che hanno la dimensione di un altro file: esclusi dal confronto.
fn cloud_same_size(lens: &Lens) -> usize {
    let mut by_size: HashMap<u64, (usize, usize)> = HashMap::new();
    for i in lens.items.iter().filter(|i| i.logical > 0 && i.family != LensFamily::Incomplete) {
        let e = by_size.entry(i.logical).or_default();
        e.0 += 1;
        e.1 += usize::from(i.placeholder);
    }
    by_size.values().filter(|(n, _)| *n >= 2).map(|(_, c)| c).sum()
}

// ---------------------------------------------------------------------------------------------
// Letture su richiesta

/// Le tre letture del contenuto, ciascuna con il suo comando.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadKind {
    /// "Confronta il contenuto": SHA-256 dei file della stessa dimensione.
    Hash,
    /// "Leggi i dati degli installatori": risorsa di versione e tabella Property.
    Installers,
    /// "Leggi l'indice degli archivi": directory centrale degli zip con la cartella accanto.
    Zip,
}

enum PromptTargets {
    Files(Vec<Target>),
    Installers(Vec<(Target, InstallerKind, ContentKey)>),
}

/// Richiesta in attesa di conferma: quanti file e quanti byte legge.
pub struct Prompt {
    pub kind: ReadKind,
    pub files: usize,
    pub bytes: u64,
    /// File solo nel cloud saltati (solo per il confronto).
    pub cloud_skipped: usize,
    targets: PromptTargets,
}

impl Prompt {
    /// Tiene solo i file dei nodi per cui `keep` è vero e ricalcola file e byte. Per la sessione di prova
    /// (`quado --sessione-prova`), che confronta un solo gruppo invece di tutta la cartella.
    pub fn retain_nodes(&mut self, keep: impl Fn(u32) -> bool) {
        let (files, bytes) = match &mut self.targets {
            PromptTargets::Files(t) => {
                t.retain(|x| keep(x.node));
                (t.len(), t.iter().map(|x| x.size).sum())
            }
            PromptTargets::Installers(t) => {
                t.retain(|(x, _, _)| keep(x.node));
                (t.len(), t.iter().map(|(x, _, _)| x.size).sum())
            }
        };
        (self.files, self.bytes) = (files, bytes);
    }
}

/// Esito dell'ultima lettura, mostrato sotto i comandi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub kind: ReadKind,
    /// File letti con un esito, file non letti per un errore, file chiesti.
    pub read: usize,
    pub failed: usize,
    pub total: usize,
    /// Annullata prima della fine.
    pub interrupted: bool,
    /// Installatori: nessun file da leggere, tutto dalla cache.
    pub cached: bool,
}

type HashOut = Result<(FileIdentity, [u8; 32]), ContentError>;

enum JobKind {
    Hash { batch: Batch<HashOut>, targets: Vec<Target> },
    /// Prima della conferma degli installatori: soli attributi, per sapere cosa è nuovo o cambiato.
    Identity { batch: Batch<Result<FileIdentity, ContentError>>, targets: Vec<(Target, InstallerKind)>, ids: Vec<Option<FileIdentity>> },
    Installers { batch: Batch<Result<InstallerInfo, ExtractError>>, targets: Vec<(Target, InstallerKind, ContentKey)> },
    Zip { batch: Batch<Result<ArchiveCheck, ExtractError>>, targets: Vec<Target> },
}

struct Job {
    kind: JobKind,
    ok: usize,
    failed: usize,
}

impl Job {
    fn new(kind: JobKind) -> Self {
        Self { kind, ok: 0, failed: 0 }
    }

    fn cancel(&self) {
        match &self.kind {
            JobKind::Hash { batch, .. } => batch.cancel(),
            JobKind::Identity { batch, .. } => batch.cancel(),
            JobKind::Installers { batch, .. } => batch.cancel(),
            JobKind::Zip { batch, .. } => batch.cancel(),
        }
    }

    fn cancelled(&self) -> bool {
        match &self.kind {
            JobKind::Hash { batch, .. } => batch.cancelled(),
            JobKind::Identity { batch, .. } => batch.cancelled(),
            JobKind::Installers { batch, .. } => batch.cancelled(),
            JobKind::Zip { batch, .. } => batch.cancelled(),
        }
    }

    /// (lettura, file finiti, file chiesti, byte letti, byte chiesti).
    fn progress(&self) -> (Option<ReadKind>, usize, usize, u64, u64) {
        let p = |b_done: (usize, u64), files: usize, bytes: u64| (b_done.0, files, b_done.1, bytes);
        let (kind, (a, b, c, d)) = match &self.kind {
            JobKind::Hash { batch, .. } => (Some(ReadKind::Hash), p(batch.done(), batch.total_files, batch.total_bytes)),
            JobKind::Identity { batch, .. } => (None, p(batch.done(), batch.total_files, batch.total_bytes)),
            JobKind::Installers { batch, .. } => (Some(ReadKind::Installers), p(batch.done(), batch.total_files, batch.total_bytes)),
            JobKind::Zip { batch, .. } => (Some(ReadKind::Zip), p(batch.done(), batch.total_files, batch.total_bytes)),
        };
        (kind, a, b, c, d)
    }
}

/// Cosa mostra il dettaglio: la riga selezionata, una famiglia o i file con "(N)".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Panel {
    #[default]
    Rows,
    Family(LensFamily),
    Suffix,
}

/// Radice risolta per una generazione dei dati e una scelta.
struct Resolved {
    generation: u64,
    chosen: Option<String>,
    path: Option<String>,
    root: Option<u32>,
}

/// Stato della schermata e dei suoi lavori.
#[derive(Default)]
pub struct DownloadsState {
    /// Cartella scelta (percorso); `None`: la cartella Download dell'utente.
    pub chosen: Option<String>,
    resolved: Option<Resolved>,
    pub view: Option<LensView>,
    build: Option<Task<LensView>>,
    /// Lenti costruite (per i test e i banchi).
    pub builds: u64,
    pub readings: Readings,
    /// Cresce a ogni cambiamento delle letture: la lente si ricostruisce.
    version: u64,
    /// File della stessa dimensione non letti, con il motivo.
    pub hash_errors: HashMap<u32, ContentError>,
    /// Cache dei dati degli installatori: per percorso, dimensione, data di modifica e ID file.
    pub cache: InstallerCache,
    /// Chiave di contenuto degli installatori dall'ultimo controllo, per nodo.
    keys: HashMap<u32, ContentKey>,
    job: Option<Job>,
    rm: Option<Batch<Vec<(u32, UseCheck)>>>,
    /// Lente (generazione, radice) per cui Restart Manager è già partito.
    rm_for: Option<(u64, u32)>,
    pub prompt: Option<Prompt>,
    pub outcome: Option<Outcome>,
    /// Riga selezionata e il suo riferimento (tipo di voce e primo file), che la segue dopo una nuova lente.
    pub selected: Option<usize>,
    anchor: Option<(Finding, u32)>,
    /// Righe dell'elenco mostrate: almeno `ROWS_PAGE`, di più dopo "Mostra altre voci" o se la
    /// selezione va oltre.
    pub shown_rows: usize,
    pub panel: Panel,
    pub focus_pending: bool,
    scroll_to_selected: bool,
    /// Thread dell'impronta; 0: `lens_jobs::HASH_THREADS`.
    pub hash_threads: usize,
}

impl DownloadsState {
    /// Nuovi dati: le letture e la lente valgono per i nodi del modello di prima. Restano la cartella
    /// scelta e la cache degli installatori, che ha chiavi di contenuto e non nodi.
    pub fn reset(&mut self) {
        *self = Self {
            chosen: self.chosen.take(),
            cache: std::mem::take(&mut self.cache),
            hash_threads: self.hash_threads,
            focus_pending: self.focus_pending,
            ..Self::default()
        };
    }

    /// Lente su un'altra cartella (`None`: Download).
    pub fn choose(&mut self, path: Option<String>) {
        if self.chosen != path {
            self.chosen = path;
            self.resolved = None;
            self.prompt = None;
            self.selected = None;
            self.anchor = None;
            self.panel = Panel::Rows;
        }
    }

    fn resolve(&mut self, d: &Loaded) -> Option<u32> {
        let fresh = self.resolved.as_ref().is_some_and(|r| r.generation == d.generation && r.chosen == self.chosen);
        if !fresh {
            let path = self.chosen.clone().or_else(|| d.evidence.folders.downloads.clone());
            let t = d.tree();
            let root = path.as_deref().and_then(|p| engine::find(t, p)).filter(|&n| t.is_dir(n) && !t.flags(n).has(Flags::NOT_FOLLOWED));
            self.resolved = Some(Resolved { generation: d.generation, chosen: self.chosen.clone(), path, root });
        }
        self.root()
    }

    /// Radice della lente nell'ultimo frame.
    pub fn root(&self) -> Option<u32> {
        self.resolved.as_ref().and_then(|r| r.root)
    }

    /// Percorso della cartella della lente (anche se non è nella scansione).
    pub fn folder_path(&self) -> Option<&str> {
        self.resolved.as_ref().and_then(|r| r.path.as_deref())
    }

    /// La lente pronta per questi dati, se c'è.
    pub fn current<'a>(&'a self, d: &Loaded) -> Option<&'a LensView> {
        self.view.as_ref().filter(|v| v.generation == d.generation && Some(v.root) == self.root())
    }

    fn threads(&self) -> usize {
        if self.hash_threads == 0 { lens_jobs::HASH_THREADS } else { self.hash_threads }
    }

    /// Una lettura in corso o in attesa di conferma: gli altri comandi aspettano.
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.prompt.is_some()
    }

    /// Lavori in corso, o una lente da ricostruire.
    pub fn pending(&self) -> bool {
        self.build.is_some()
            || self.job.is_some()
            || self.rm.is_some()
            || self.view.as_ref().is_some_and(|v| v.version != self.version || self.rm_for != Some((v.generation, v.root)))
    }

    /// Un frame: raccoglie i risultati, ricostruisce la lente se serve, avvia Restart Manager.
    pub fn poll(&mut self, ctx: Option<&egui::Context>, d: Option<&Loaded>, readers: &Readers) {
        let Some(d) = d else { return };
        if self.collect(false, false) {
            self.finish();
        }
        self.collect_usage(false);
        if let Some(t) = self.build.as_mut()
            && let Some(v) = t.poll()
        {
            self.build = None;
            self.install(v);
        }
        let root = self.resolve(d);
        if self.build.is_none() {
            match root {
                Some(root) => {
                    let stale = self.view.as_ref().is_none_or(|v| v.generation != d.generation || v.root != root || v.version != self.version);
                    if stale {
                        self.spawn_build(d, root);
                    }
                }
                None => {
                    self.view = None;
                    self.selected = None;
                }
            }
        }
        self.auto_usage(d, readers);
        if let Some(c) = ctx
            && self.pending()
        {
            c.request_repaint_after(Duration::from_millis(30));
        }
    }

    /// Attende la fine dei lavori senza annullarli, finché non resta niente da fare (test, immagini).
    pub fn settle(&mut self, d: Option<&Loaded>, readers: &Readers) {
        for _ in 0..1000 {
            self.poll(None, d, readers);
            if !self.pending() {
                break;
            }
            if self.collect(true, false) {
                self.finish();
            }
            self.collect_usage(true);
            if let Some(t) = self.build.take()
                && let Some(v) = t.wait()
            {
                self.install(v);
            }
        }
    }

    /// Prima di cambiare il modello (`App::wait_background`): annulla le letture, attende i thread (le
    /// letture finite restano) e la lente in costruzione. Dopo, nessun thread tiene il modello.
    pub fn wait(&mut self) {
        if self.collect(true, true) {
            self.finish();
        }
        self.collect_usage(true);
        if let Some(t) = self.build.take()
            && let Some(v) = t.wait()
        {
            self.install(v);
        }
    }

    fn spawn_build(&mut self, d: &Loaded, root: u32) {
        let model = Arc::clone(&d.model);
        let evidence = Arc::clone(&d.evidence);
        let readings = self.readings.clone();
        let (generation, version) = (d.generation, self.version);
        self.builds += 1;
        self.build = Some(Task::spawn(move || {
            let v = build_view(&model, root, generation, evidence.uninstall.as_ref().ok(), &readings, version);
            drop(model);
            v
        }));
    }

    fn install(&mut self, v: LensView) {
        if self.view.as_ref().is_some_and(|old| old.root != v.root) {
            self.selected = None;
            self.anchor = None;
            self.shown_rows = 0;
        }
        self.view = Some(v);
        self.remap_selection();
    }

    /// Dopo una nuova lente la selezione segue la riga con lo stesso tipo di voce e lo stesso file, poi
    /// una riga con lo stesso file.
    fn remap_selection(&mut self) {
        let Some(v) = &self.view else {
            self.selected = None;
            return;
        };
        let has = |r: &Row, n: u32| r.voci.iter().any(|&k| v.findings[k].nodes.contains(&n));
        self.selected = self.anchor.and_then(|(f, n)| {
            v.rows.iter().position(|r| r.finding == f && has(r, n)).or_else(|| v.rows.iter().position(|r| has(r, n)))
        });
        if self.selected.is_none() {
            self.anchor = None;
        }
    }

    /// Seleziona una riga dell'elenco e la mostra nel dettaglio.
    pub fn select(&mut self, row: usize) {
        let Some(v) = &self.view else { return };
        let Some(r) = v.rows.get(row) else { return };
        let first = r.voci.first().and_then(|&k| v.findings[k].nodes.first().copied());
        self.selected = Some(row);
        self.anchor = first.map(|n| (r.finding, n));
        self.panel = Panel::Rows;
        self.scroll_to_selected = true;
    }

    /// Seleziona la riga che contiene la voce `k` (dalla card interventi).
    pub fn select_finding(&mut self, k: usize) {
        let row = self.view.as_ref().and_then(|v| v.rows.iter().position(|r| r.voci.contains(&k)));
        if let Some(r) = row {
            self.select(r);
            self.focus_pending = true;
        }
    }

    /// La copia da tenere in un gruppo di copie identiche (`downloads::with_keep`).
    pub fn set_keep(&mut self, group: &[u32], node: u32) {
        self.readings.keep.retain(|n| !group.contains(n));
        self.readings.keep.push(node);
        self.version += 1;
    }

    /// Dimentica le letture di un tipo, cache compresa: solo per il banco di misura, che ripete la stessa
    /// lettura più volte.
    pub fn forget(&mut self, kind: ReadKind) {
        match kind {
            ReadKind::Hash => {
                self.readings.hashed.clear();
                self.readings.keep.clear();
                self.hash_errors.clear();
            }
            ReadKind::Installers => {
                self.readings.installers.clear();
                self.cache = InstallerCache::new();
                self.keys.clear();
            }
            ReadKind::Zip => self.readings.archives.clear(),
        }
        self.outcome = None;
        self.version += 1;
    }

    // ------------------------------------------------------------------ comandi

    /// File che "Confronta il contenuto" leggerebbe: quelli dei gruppi della stessa dimensione non ancora
    /// confrontati, tranne quelli già letti con la dimensione della scansione. In ordine di gruppo, così un
    /// annullamento lascia interi i primi gruppi.
    /// Bersagli del confronto, senza percorso: lo costruiscono i thread del lavoro (`spawn_hash_in`).
    pub fn hash_targets(&self, d: &Loaded) -> Vec<Target> {
        let Some(v) = self.current(d) else { return Vec::new() };
        let read: HashMap<u32, u64> = self.readings.hashed.iter().map(|h| (h.node, h.size)).collect();
        let mut out = Vec::new();
        for (g, c) in v.candidates.iter().zip(&v.compared) {
            if !matches!(c, GroupComparison::NotRead { .. }) {
                continue;
            }
            for &n in &g.nodes {
                if read.get(&n) != Some(&g.logical) {
                    out.push(Target { node: n, path: String::new(), size: g.logical });
                }
            }
        }
        out
    }

    /// Installatori che i lettori sanno leggere (exe e MSI), esclusi i segnaposto cloud.
    pub fn installer_targets(&self, d: &Loaded) -> Vec<(Target, InstallerKind)> {
        let Some(v) = self.current(d) else { return Vec::new() };
        let t = d.tree();
        v.lens
            .of_family(LensFamily::Installers)
            .filter(|i| !i.placeholder)
            .filter_map(|i| {
                let kind = installers::installer_kind(&t.name_lossy(i.node))?;
                Some((Target { node: i.node, path: t.path_lossy(i.node), size: i.logical }, kind))
            })
            .collect()
    }

    /// Zip con la cartella accanto, esclusi i segnaposto cloud.
    pub fn zip_targets(&self, d: &Loaded) -> Vec<Target> {
        let Some(v) = self.current(d) else { return Vec::new() };
        let t = d.tree();
        v.pairs.iter().filter(|p| !t.flags(p.archive).has(Flags::CLOUD)).map(|p| Target { node: p.archive, path: t.path_lossy(p.archive), size: t.logical(p.archive) }).collect()
    }

    /// Un comando: il confronto e gli zip chiedono subito conferma; gli installatori prima controllano gli
    /// attributi (nessuna lettura del contenuto) per dire quanti file sono nuovi o cambiati.
    pub fn request(&mut self, kind: ReadKind, d: &Loaded, readers: &Readers) {
        if self.busy() {
            return;
        }
        match kind {
            ReadKind::Hash => {
                let targets = self.hash_targets(d);
                let cloud = self.current(d).map_or(0, |v| v.cloud_same_size);
                if !targets.is_empty() {
                    self.prompt = Some(Prompt { kind, files: targets.len(), bytes: targets.iter().map(|t| t.size).sum(), cloud_skipped: cloud, targets: PromptTargets::Files(targets) });
                }
            }
            ReadKind::Zip => {
                let targets = self.zip_targets(d);
                if !targets.is_empty() {
                    self.prompt = Some(Prompt { kind, files: targets.len(), bytes: targets.iter().map(|t| t.size).sum(), cloud_skipped: 0, targets: PromptTargets::Files(targets) });
                }
            }
            ReadKind::Installers => {
                let targets = self.installer_targets(d);
                if !targets.is_empty() {
                    self.outcome = None;
                    let batch = lens_jobs::spawn_identity(readers, targets.iter().map(|(t, _)| t.clone()).collect());
                    let ids = vec![None; targets.len()];
                    self.job = Some(Job::new(JobKind::Identity { batch, targets, ids }));
                }
            }
        }
    }

    /// Conferma della lettura chiesta.
    pub fn confirm(&mut self, d: &Loaded, readers: &Readers) {
        if self.job.is_some() {
            return;
        }
        let Some(p) = self.prompt.take() else { return };
        self.outcome = None;
        let kind = match (p.kind, p.targets) {
            (ReadKind::Hash, PromptTargets::Files(targets)) => JobKind::Hash { batch: lens_jobs::spawn_hash_in(readers, Arc::clone(&d.model), targets.clone(), self.threads()), targets },
            (ReadKind::Zip, PromptTargets::Files(targets)) => {
                let folders: HashMap<u32, u32> = self.current(d).map(|v| v.pairs.iter().map(|p| (p.archive, p.folder)).collect()).unwrap_or_default();
                let model = Arc::clone(&d.model);
                let compare = move |archive: u32, ix: &quado_scan_win::zip::ZipIndex| match folders.get(&archive) {
                    Some(&folder) => downloads::compare_archive(&model.tree, folder, &ix.entries),
                    None => ArchiveCheck::default(),
                };
                JobKind::Zip { batch: lens_jobs::spawn_zip(readers, targets.clone(), compare), targets }
            }
            (ReadKind::Installers, PromptTargets::Installers(targets)) => {
                let batch = lens_jobs::spawn_installers(readers, targets.iter().map(|(t, k, _)| (t.clone(), *k)).collect());
                JobKind::Installers { batch, targets }
            }
            _ => return,
        };
        self.job = Some(Job::new(kind));
    }

    pub fn dismiss(&mut self) {
        self.prompt = None;
    }

    /// Annulla la lettura in corso: i file finiti restano letti.
    pub fn cancel_job(&self) {
        if let Some(j) = &self.job {
            j.cancel();
        }
    }

    /// "Controlla di nuovo l'uso": Restart Manager sui download incompleti della lente.
    pub fn recheck_usage(&mut self, d: &Loaded, readers: &Readers) {
        if self.rm.is_none() {
            self.rm_for = None;
            self.auto_usage(d, readers);
        }
    }

    fn auto_usage(&mut self, d: &Loaded, readers: &Readers) {
        let Some(v) = self.current(d) else { return };
        if self.rm.is_some() || self.rm_for == Some((v.generation, v.root)) {
            return;
        }
        let t = d.tree();
        let targets: Vec<Target> =
            v.lens.of_family(LensFamily::Incomplete).map(|i| Target { node: i.node, path: t.path_lossy(i.node), size: i.logical }).collect();
        self.rm_for = Some((v.generation, v.root));
        if !targets.is_empty() {
            self.rm = Some(lens_jobs::spawn_usage(readers, targets));
        }
    }

    /// Avanzamento della lettura in corso: (lettura, file finiti, file chiesti, byte letti, byte chiesti);
    /// la lettura è `None` per il controllo degli attributi degli installatori.
    pub fn progress(&self) -> Option<(Option<ReadKind>, usize, usize, u64, u64)> {
        self.job.as_ref().map(Job::progress)
    }

    /// Restart Manager in corso.
    pub fn checking_usage(&self) -> bool {
        self.rm.is_some()
    }

    // ------------------------------------------------------------------ risultati

    /// Raccoglie i risultati del lavoro in corso; con `block` attende la fine (con `cancel` dopo averlo
    /// annullato). Restituisce vero se il lavoro è finito.
    fn collect(&mut self, block: bool, cancel: bool) -> bool {
        let Some(job) = self.job.as_mut() else { return false };
        if cancel {
            job.cancel();
        }
        macro_rules! take {
            ($batch:expr) => {
                if block { $batch.wait() } else { $batch.drain() }
            };
        }
        match &mut job.kind {
            JobKind::Hash { batch, targets } => {
                for (i, r) in take!(batch) {
                    let Some(t) = targets.get(i) else { continue };
                    match r {
                        Ok((id, digest)) => {
                            self.readings.hashed.push(lens_jobs::hashed(t.node, &id, digest));
                            self.hash_errors.remove(&t.node);
                            job.ok += 1;
                        }
                        Err(ContentError::Cancelled) => {}
                        Err(e) => {
                            self.hash_errors.insert(t.node, e);
                            job.failed += 1;
                        }
                    }
                }
                batch.is_finished()
            }
            JobKind::Identity { batch, targets, ids } => {
                for (i, r) in take!(batch) {
                    let Some((t, _)) = targets.get(i) else { continue };
                    match r {
                        Ok(id) => ids[i] = Some(id),
                        Err(ContentError::Cancelled) => {}
                        Err(e) => {
                            // Un installatore che non si è potuto controllare: il motivo nella sua voce.
                            self.keys.remove(&t.node);
                            self.readings.installers.insert(t.node, Err(lens_jobs::identity_error(e)));
                        }
                    }
                }
                batch.is_finished()
            }
            JobKind::Installers { batch, targets } => {
                for (i, r) in take!(batch) {
                    let Some((_, _, key)) = targets.get(i) else { continue };
                    if r.is_ok() {
                        job.ok += 1;
                    } else {
                        job.failed += 1;
                    }
                    self.cache.insert(key.clone(), r);
                }
                batch.is_finished()
            }
            JobKind::Zip { batch, targets } => {
                for (i, r) in take!(batch) {
                    let Some(t) = targets.get(i) else { continue };
                    if r.is_ok() {
                        job.ok += 1;
                    } else {
                        job.failed += 1;
                    }
                    self.readings.archives.insert(t.node, r);
                }
                batch.is_finished()
            }
        }
    }

    /// Lavoro finito: esito, letture nuove, lente da ricostruire; dopo il controllo degli attributi, la
    /// richiesta di conferma degli installatori nuovi o cambiati.
    fn finish(&mut self) {
        let Some(job) = self.job.take() else { return };
        let interrupted = job.cancelled();
        let (ok, failed) = (job.ok, job.failed);
        let outcome = |kind: ReadKind, total: usize| Outcome { kind, read: ok, failed, total, interrupted: interrupted && ok + failed < total, cached: false };
        match job.kind {
            JobKind::Hash { targets, .. } => self.outcome = Some(outcome(ReadKind::Hash, targets.len())),
            JobKind::Zip { targets, .. } => self.outcome = Some(outcome(ReadKind::Zip, targets.len())),
            JobKind::Installers { targets, .. } => {
                self.apply_installers();
                self.outcome = Some(outcome(ReadKind::Installers, targets.len()));
            }
            JobKind::Identity { targets, ids, .. } => {
                if interrupted {
                    self.version += 1;
                    return;
                }
                let mut todo = Vec::new();
                let mut keys = Vec::new();
                for ((t, kind), id) in targets.into_iter().zip(ids) {
                    let Some(id) = id else { continue };
                    let key = ContentKey { path: t.path.clone(), size: id.size, modified: id.modified, file_id: id.file_id };
                    self.keys.insert(t.node, key.clone());
                    keys.push(key.clone());
                    todo.push((Target { size: id.size, ..t }, kind, key));
                }
                let fresh: HashSet<String> = self.cache.to_read(&keys).into_iter().map(|k| k.path.clone()).collect();
                todo.retain(|(t, _, _)| fresh.contains(&t.path));
                if todo.is_empty() {
                    // Seconda richiesta sugli stessi file: niente da leggere, i dati vengono dalla cache.
                    self.apply_installers();
                    self.outcome = Some(Outcome { kind: ReadKind::Installers, read: 0, failed: 0, total: keys.len(), interrupted: false, cached: true });
                } else {
                    let bytes = todo.iter().map(|(t, _, _)| t.size).sum();
                    self.prompt = Some(Prompt { kind: ReadKind::Installers, files: todo.len(), bytes, cloud_skipped: 0, targets: PromptTargets::Installers(todo) });
                }
                return;
            }
        }
        self.version += 1;
    }

    /// Dati degli installatori dalla cache, per ogni file con la sua chiave di contenuto attuale. Un file
    /// cambiato e non ancora riletto non ha dati: torna "non letto".
    fn apply_installers(&mut self) {
        for (node, key) in &self.keys {
            match self.cache.get(key) {
                Some(r) => {
                    self.readings.installers.insert(*node, r.clone());
                }
                None => {
                    self.readings.installers.remove(node);
                }
            }
        }
        self.version += 1;
    }

    fn collect_usage(&mut self, block: bool) {
        let Some(b) = self.rm.as_mut() else { return };
        let got = if block { b.wait() } else { b.drain() };
        for (_, list) in got {
            for (node, check) in list {
                self.readings.usage.insert(node, check);
            }
        }
        if b.is_finished() {
            self.rm = None;
            self.version += 1;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Disponibilità dei comandi

/// Perché un comando non si può usare, o quanti file legge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Avail {
    Ready,
    Busy,
    /// Chiave del motivo.
    No(&'static str),
}

fn hash_avail(st: &DownloadsState, v: &LensView) -> Avail {
    if st.busy() {
        return Avail::Busy;
    }
    let open = v.compared.iter().any(|c| matches!(c, GroupComparison::NotRead { .. }));
    if open {
        Avail::Ready
    } else if !v.candidates.is_empty() {
        Avail::No("download-motivo-gia-confrontati")
    } else if v.cloud_same_size > 0 {
        Avail::No("download-motivo-cloud")
    } else {
        Avail::No("download-motivo-nessun-gruppo")
    }
}

fn installers_avail(st: &DownloadsState, v: &LensView, t: &Tree) -> Avail {
    if st.busy() {
        return Avail::Busy;
    }
    let any = v.lens.of_family(LensFamily::Installers).any(|i| !i.placeholder && installers::installer_kind(&t.name_lossy(i.node)).is_some());
    if any { Avail::Ready } else { Avail::No("download-motivo-nessun-installatore") }
}

fn zip_avail(st: &DownloadsState, v: &LensView, t: &Tree) -> Avail {
    if st.busy() {
        return Avail::Busy;
    }
    if v.pairs.iter().any(|p| !t.flags(p.archive).has(Flags::CLOUD)) { Avail::Ready } else { Avail::No("download-motivo-nessun-archivio") }
}

// ---------------------------------------------------------------------------------------------
// Testi

/// Titolo di una riga: il tipo di voce, al singolare o al plurale secondo i file.
pub fn row_title(l: &Lingue, row: &Row) -> String {
    let key = match row.finding {
        Finding::SameSize => "download-voce-stessa-dimensione",
        Finding::IdenticalCopies { .. } => "download-voce-copie",
        Finding::DifferentContent => "download-voce-contenuto-diverso",
        Finding::Installer(s) => match s {
            InstallerStatus::NotRead => "download-voce-installatori-non-letti",
            InstallerStatus::CloudOnly => "download-voce-installatori-cloud",
            InstallerStatus::Failed => "download-voce-installatori-non-riusciti",
            InstallerStatus::SameVersion => "download-voce-installatori-stessa-versione",
            InstallerStatus::OtherVersion => "download-voce-installatori-altra-versione",
            InstallerStatus::NotInstalled => "download-voce-installatori-non-installati",
            InstallerStatus::NotMatched => "download-voce-installatori-senza-confronto",
        },
        Finding::ArchiveWithFolder { .. } => "download-voce-archivio",
        Finding::DiskImages => "download-voce-immagini-disco",
        Finding::Incomplete(u) => match u {
            UseStatus::Free => "download-voce-incompleti-liberi",
            UseStatus::InUse => "download-voce-incompleti-in-uso",
            UseStatus::Unknown => "download-voce-incompleti-uso-non-noto",
            UseStatus::NotChecked => "download-voce-incompleti-non-controllati",
        },
    };
    l.ta(key, &[("n", row.files.into())])
}

fn files_text(l: &Lingue, f: &Formatter, n: u64) -> String {
    l.ta("conteggio-file", &[("n", n.into()), ("testo", f.count(n).into())])
}

/// Nome del primo file e quanti altri.
fn names(l: &Lingue, f: &Formatter, t: &Tree, nodes: &[u32]) -> String {
    let first = nodes.first().map(|&n| t.name_lossy(n)).unwrap_or_default();
    if nodes.len() <= 1 {
        return first;
    }
    let others = (nodes.len() - 1) as u64;
    l.ta("download-nomi", &[("primo", first.into()), ("altri", others.into()), ("altri_testo", f.count(others).into())])
}

/// Spiegazione della riga: il primo fatto d'identità della voce (il suo riassunto); per più voci, i
/// nomi dei file o quanti gruppi.
fn row_explanation(l: &Lingue, f: &Formatter, t: &Tree, v: &LensView, row: &Row) -> (String, Option<String>) {
    let first = &v.findings[row.voci[0]];
    if row.voci.len() == 1 {
        let fact = first.record.identity.0.first().map(|x| kbtext::fact_text(l, f, x)).unwrap_or_default();
        return (fact, Some(names(l, f, t, &first.nodes)));
    }
    if row.voci.iter().all(|&k| v.findings[k].nodes.len() == 1) {
        let nodes: Vec<u32> = row.voci.iter().map(|&k| v.findings[k].nodes[0]).collect();
        return (names(l, f, t, &nodes), None);
    }
    let n = row.voci.len() as u64;
    let s = l.ta("download-riga-gruppi", &[("n", n.into()), ("testo", f.count(n).into()), ("file", files_text(l, f, row.files as u64).into())]);
    let nodes: Vec<u32> = row.voci.iter().flat_map(|&k| v.findings[k].nodes.iter().copied()).collect();
    (s, Some(names(l, f, t, &nodes)))
}

/// Stima della riga: la somma delle stime delle voci (`engine::sum_estimates`).
fn row_estimate(v: &LensView, row: &Row) -> Estimate {
    let mut it = row.voci.iter().map(|&k| v.findings[k].record.estimate);
    let first = it.next().unwrap_or(Estimate::NotEstimable(quado_model::record::NotEstimable::NoAction));
    it.fold(first, engine::sum_estimates)
}

fn content_error(l: &Lingue, e: &ContentError) -> String {
    match e {
        ContentError::Io(2 | 3) => l.t("download-non-letto-assente"),
        ContentError::Io(5) => l.t("download-non-letto-accesso"),
        ContentError::Io(32 | 33) => l.t("download-non-letto-in-uso"),
        ContentError::Io(c) => l.ta("download-non-letto-errore", &[("codice", c.to_string().into())]),
        ContentError::CloudPlaceholder => l.t("download-non-letto-cloud"),
        ContentError::Changed => l.t("download-non-letto-cambiato"),
        ContentError::NotAFile => l.t("download-non-letto-non-file"),
        ContentError::Cancelled => l.t("download-non-letto-annullato"),
    }
}

fn prompt_text(l: &Lingue, f: &Formatter, p: &Prompt) -> String {
    let n = p.files as u64;
    let args = [("n", n.into()), ("testo", f.count(n).into()), ("byte", f.size(p.bytes).into())];
    let key = match p.kind {
        ReadKind::Hash => "download-confronta-domanda",
        ReadKind::Installers => "download-installatori-domanda",
        ReadKind::Zip => "download-archivi-domanda",
    };
    l.ta(key, &args)
}

fn outcome_text(l: &Lingue, f: &Formatter, o: &Outcome) -> String {
    if o.cached {
        return l.t("download-esito-installatori-cache");
    }
    let key = match (o.kind, o.interrupted) {
        (ReadKind::Hash, false) => "download-esito-confronto",
        (ReadKind::Hash, true) => "download-esito-confronto-interrotto",
        (ReadKind::Installers, false) => "download-esito-installatori",
        (ReadKind::Installers, true) => "download-esito-installatori-interrotto",
        (ReadKind::Zip, false) => "download-esito-archivi",
        (ReadKind::Zip, true) => "download-esito-archivi-interrotto",
    };
    let mut s = l.ta(key, &[("letti", f.count(o.read as u64).into()), ("totale", f.count(o.total as u64).into())]);
    if o.failed > 0 {
        s.push(' ');
        s.push_str(&l.ta("download-esito-non-letti", &[("n", o.failed.into()), ("testo", f.count(o.failed as u64).into())]));
    }
    s
}

// ---------------------------------------------------------------------------------------------
// Schermata

/// Cosa chiede il frame, applicato dopo il disegno.
#[derive(Default)]
struct Act {
    choose: Option<Option<String>>,
    request: Option<ReadKind>,
    confirm: bool,
    dismiss: bool,
    cancel: bool,
    recheck: bool,
    select: Option<usize>,
    panel: Option<Panel>,
    keep: Option<(Vec<u32>, u32)>,
    reveal: Option<String>,
    copy: Option<String>,
    focus_done: bool,
    scrolled: bool,
    more_rows: bool,
    /// Pulsante degli interventi con le voci della riga (indici in `LensView::findings`).
    intervention: Option<(super::intervention_jobs::ActRequest, Vec<usize>)>,
}

/// Vista Download (variante A): come Esplora, con il dettaglio a destra su tutta l'altezza nel tono della
/// barra laterale. A sinistra la barra (cartella, filtro "(N)", letture su richiesta), la riga con percorso,
/// dimensioni e file, le famiglie come fila di cifre cliccabili, la conferma o l'avanzamento di una lettura e
/// la card "Cosa riconosce Quado in questa cartella".
pub fn show(app: &mut App, ui: &mut Ui, p: &Palette) {
    let Some(d) = app.data.as_ref() else { return };
    let split = super::shell::split(ui, p);
    let mut child = ui.new_child(UiBuilder::new().max_rect(split.content).layout(Layout::top_down(Align::Min)));
    let ui = &mut child;
    super::overview::interrupted_banner(app, ui, p);
    let mut act = Act::default();
    let (l, f, st) = (&app.lingue, &app.fmt, &app.downloads);
    let view = st.current(d);
    toolbar(ui, p, l, f, d, st, view, &mut act);
    ui.add_space(6.0);
    header(ui, p, l, f, st, view);
    ui.add_space(14.0);
    match view {
        None => {
            let msg = if st.root().is_some() {
                l.t("download-costruzione")
            } else {
                let what = match st.folder_path() {
                    Some(path) => l.ta("download-non-trovata", &[("percorso", path.to_owned().into())]),
                    None => l.t("download-non-indicata"),
                };
                format!("{what} {}", l.t("download-scegli"))
            };
            let rest = ui.available_rect_before_wrap();
            super::shell::empty_state(ui, p, rest, &msg, None);
        }
        Some(v) => {
            tiles(ui, p, l, f, st, v, &mut act);
            others_line(ui, p, l, f, v);
            band(ui, p, l, f, st, &mut act);
            let rest = ui.available_rect_before_wrap();
            let mut lui = ui.new_child(UiBuilder::new().max_rect(rest).layout(Layout::top_down(Align::Min)));
            list(&mut lui, p, l, f, d.tree(), st, v, &mut act);
            let mut rui = ui.new_child(UiBuilder::new().max_rect(split.detail).layout(Layout::top_down(Align::Min)));
            detail(&mut rui, p, l, f, d, st, v, &mut act);
        }
    }
    apply(app, ui.ctx(), act);
}

fn apply(app: &mut App, ctx: &egui::Context, act: Act) {
    let readers = app.env.readers.clone();
    let st = &mut app.downloads;
    if act.focus_done {
        st.focus_pending = false;
    }
    if act.scrolled {
        st.scroll_to_selected = false;
    }
    if let Some(c) = act.choose {
        st.choose(c);
    }
    if act.more_rows {
        st.shown_rows = st.shown_rows.max(ROWS_PAGE) + ROWS_PAGE;
    }
    if let Some(r) = act.select {
        // La tastiera può andare oltre le righe mostrate: si mostrano fino a quella.
        st.shown_rows = st.shown_rows.max(ROWS_PAGE).max(r + 1);
        st.select(r);
    }
    if let Some(panel) = act.panel {
        st.panel = if st.panel == panel { Panel::Rows } else { panel };
    }
    if let Some((group, node)) = act.keep {
        st.set_keep(&group, node);
    }
    if act.dismiss {
        st.dismiss();
    }
    if act.cancel {
        st.cancel_job();
    }
    if let Some(d) = app.data.as_ref() {
        if let Some(k) = act.request {
            st.request(k, d, &readers);
        }
        if act.confirm {
            st.confirm(d, &readers);
        }
        if act.recheck {
            st.recheck_usage(d, &readers);
        }
    }
    if let Some(path) = act.reveal
        && let Err(e) = shell::reveal(&path)
    {
        app.notice.set(app.lingue.ta("avviso-shell-errore", &[("codice", format!("0x{:08X}", e.0 as u32).into())]));
    }
    if let Some(path) = act.copy {
        ctx.copy_text(path);
        app.notice.set(app.lingue.t("avviso-percorso-copiato"));
    }
    if let Some((a, voci)) = act.intervention {
        super::intervention_jobs::dispatch(app, a, super::intervention_jobs::Origin::Lens(voci));
    }
}

/// Riga sotto la barra: percorso, dimensioni e file della cartella, con il metodo nella "i"; sotto, in
/// avviso, le cartelle non lette.
fn header(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, st: &DownloadsState, view: Option<&LensView>) {
    let path = st.folder_path().unwrap_or_default().to_owned();
    let meta = match view {
        Some(v) => l.ta("download-meta", &[("percorso", path.into()), ("dimensione", f.size(v.lens.alloc).into()), ("file", files_text(l, f, v.lens.files).into())]),
        None => path,
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let w = (ui.available_width() - 40.0).max(80.0);
        let r = widgets::label_elided(ui, &meta, widgets::font(Type::Callout), p.text_2, w);
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &meta));
        widgets::info(ui, p, &l.t("download-metodo"));
    });
    if let Some(v) = view.filter(|v| v.lens.unread_dirs > 0) {
        let n = v.lens.unread_dirs;
        widgets::paragraph(ui, l.ta("download-meta-minimo", &[("n", n.into()), ("testo", f.count(n).into())]), Type::Callout, p.warn);
    }
}

/// Barra (`.exbar`): la cartella come filtro in linea con il suo menu, il filtro "(N)" e le tre letture su
/// richiesta; ognuna dice prima cosa legge e chiede conferma.
#[allow(clippy::too_many_arguments)]
fn toolbar(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, d: &Loaded, st: &DownloadsState, view: Option<&LensView>, act: &mut Act) {
    let t = d.tree();
    // Cartelle dell'utente presenti nella scansione, più quella scelta da Esplora.
    let known: Vec<(&'static str, String)> = d
        .evidence
        .folders
        .user_folders()
        .iter()
        .filter_map(|(code, path)| path.filter(|p| engine::find(t, p).is_some()).map(|p| (*code, p.to_owned())))
        .collect();
    let default = d.evidence.folders.downloads.clone();
    let label_of = |path: Option<&str>| -> String {
        let path = path.or(default.as_deref()).unwrap_or_default();
        match known.iter().find(|(_, p)| p.eq_ignore_ascii_case(path)) {
            Some((code, _)) => l.ta("download-cartella-nota", &[("codice", (*code).into())]),
            None => path.rsplit('\\').next().unwrap_or(path).to_owned(),
        }
    };
    let mut hash_motive = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        let current = label_of(st.chosen.as_deref());
        let r = super::explore::filter_chip(ui, p, &l.t("download-cartella"), &current, false);
        egui::Popup::menu(&r).show(|ui| {
            for (code, path) in &known {
                let on = st.chosen.as_deref().unwrap_or(default.as_deref().unwrap_or_default()).eq_ignore_ascii_case(path);
                if super::explore::menu_choice(ui, p, &l.ta("download-cartella-nota", &[("codice", (*code).into())]), on).clicked() {
                    let is_default = default.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(path));
                    act.choose = Some(if is_default { None } else { Some(path.clone()) });
                }
            }
            if let Some(c) = st.chosen.as_deref()
                && !known.iter().any(|(_, p)| p.eq_ignore_ascii_case(c))
            {
                super::explore::menu_choice(ui, p, c.rsplit('\\').next().unwrap_or(c), true);
            }
        });
        let Some(v) = view else { return };
        let n = v.suffixed.len() as u64;
        let chip = widgets::chip(ui, p, &l.ta("download-filtro-suffisso", &[("n", n.into()), ("testo", f.count(n).into())]), st.panel == Panel::Suffix);
        if chip.clicked() {
            act.panel = Some(Panel::Suffix);
        }
        ui.add_space(12.0);
        let mut command = |ui: &mut Ui, key: &str, icon: Icon, avail: Avail, kind: ReadKind| {
            let r = widgets::button(ui, p, &l.t(key), Some(icon), ButtonKind::Secondary, avail == Avail::Ready);
            if r.clicked() {
                act.request = Some(kind);
            }
            match avail {
                Avail::No(m) => {
                    r.on_hover_text(l.t(m));
                }
                Avail::Busy => {
                    r.on_hover_text(l.t("download-motivo-lettura-in-corso"));
                }
                Avail::Ready => {}
            }
        };
        let ha = hash_avail(st, v);
        if let Avail::No(m) = ha
            && m == "download-motivo-cloud"
        {
            hash_motive = Some(m);
        }
        command(ui, "download-comando-confronta", Icon::Verify, ha, ReadKind::Hash);
        command(ui, "download-comando-installatori", Icon::Info, installers_avail(st, v, t), ReadKind::Installers);
        command(ui, "download-comando-archivi", Icon::Explore, zip_avail(st, v, t), ReadKind::Zip);
    });
    // Il motivo di un comando disattivato per i file nel cloud si vede, non solo nel suggerimento.
    if let Some(m) = hash_motive {
        widgets::paragraph(ui, l.t(m), Type::Callout, p.text_2);
    }
}

/// Famiglie come fila di cifre in una card (`.nums`): nome, dimensione grande e leggera, numero di file;
/// quella dei download incompleti solo se ce ne sono. Ogni colonna è un pulsante che mostra nel dettaglio i
/// file più grandi della famiglia.
fn tiles(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, st: &DownloadsState, v: &LensView, act: &mut Act) {
    let fams: Vec<LensFamily> = LensFamily::SHOWN.into_iter().filter(|&x| x != LensFamily::Incomplete || v.lens.total(x).files > 0).collect();
    let w = ui.available_width();
    let n = fams.len().max(1) as f32;
    let pad = 6.0;
    let tw = ((w - 2.0 * pad) / n).floor();
    let painter = ui.painter().clone();
    let laid: Vec<_> = fams
        .iter()
        .map(|&fam| {
            let total = v.lens.total(fam);
            let name = l.t(&format!("lente-famiglia-{}", fam.code()));
            let ng = widgets::galley(&painter, name.clone(), Type::Callout, p.text_2, Some(tw - 28.0));
            let sg = painter.layout_no_wrap(f.size(total.alloc), widgets::light(24.0), p.text);
            let cg = widgets::galley(&painter, files_text(l, f, total.files), Type::Callout, p.text_2, Some(tw - 28.0));
            (fam, name, total, ng, sg, cg)
        })
        .collect();
    let name_h = laid.iter().map(|x| x.3.size().y).fold(0.0, f32::max);
    let size_h = laid.iter().map(|x| x.4.size().y).fold(0.0, f32::max);
    let count_h = laid.iter().map(|x| x.5.size().y).fold(0.0, f32::max);
    let h = 14.0 + name_h + 4.0 + size_h + 2.0 + count_h + 14.0 + 2.0 * pad;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let sh = p.shadow(tokens::Elevation::Card);
    if sh != egui::epaint::Shadow::NONE {
        painter.add(sh.as_shape(rect, CornerRadius::same(tokens::radius::CARD)));
    }
    painter.rect_filled(rect, CornerRadius::same(tokens::radius::CARD), p.panel);
    let count = laid.len();
    let mut lit = vec![false; count];
    let mut cells = Vec::with_capacity(count);
    for k in 0..count {
        let x0 = rect.min.x + pad + k as f32 * tw;
        let r = Rect::from_min_size(pos2(x0, rect.min.y + pad), vec2(tw, h - 2.0 * pad));
        let resp = ui.interact(r, ui.id().with(("lente_famiglia", k)), Sense::click());
        lit[k] = st.panel == Panel::Family(laid[k].0) || resp.hovered();
        cells.push((r, resp));
    }
    for (k, ((fam, name, total, ng, sg, cg), (r, resp))) in laid.into_iter().zip(cells).enumerate() {
        let on = st.panel == Panel::Family(fam);
        let fill = if on {
            p.select
        } else if resp.hovered() {
            p.hover
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(r, CornerRadius::same(tokens::radius::GROUP - 2), fill);
        // Filetto tra le colonne, nascosto accanto a una colonna accesa.
        if k > 0 && !lit[k] && !lit[k - 1] {
            painter.vline(r.min.x, (r.min.y + 14.0)..=(r.max.y - 14.0), Stroke::new(1.0, p.separator));
        }
        let x = r.min.x + 14.0;
        let y = r.min.y + 14.0;
        painter.galley(pos2(x, y), ng, p.text_2);
        painter.galley(pos2(x, y + name_h + 4.0), sg, p.text);
        painter.galley(pos2(x, y + name_h + 4.0 + size_h + 2.0), cg, p.text_2);
        if resp.has_focus() {
            widgets::focus_ring_inside(&painter, r, p, tokens::radius::GROUP - 2);
        }
        let a11y = l.ta("download-riquadro", &[("famiglia", name.into()), ("dimensione", f.size(total.alloc).into()), ("file", files_text(l, f, total.files).into())]);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, &a11y));
        if resp.clicked() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
            act.panel = Some(Panel::Family(fam));
        }
    }
}

/// I file di altri tipi, così la somma delle famiglie torna con il totale della riga sopra.
fn others_line(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, v: &LensView) {
    let o = v.lens.total(LensFamily::Other);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        let s = l.ta("download-altri-file", &[("dimensione", f.size(o.alloc).into()), ("file", files_text(l, f, o.files).into())]);
        widgets::paragraph(ui, s, Type::Callout, p.text_2);
    });
    ui.add_space(12.0);
}

/// Richiesta di conferma, avanzamento o esito dell'ultima lettura. La conferma sta in un riquadro con
/// l'icona, cosa legge Quado e la conseguenza, e la risposta: il pulsante principale e «Annulla».
fn band(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, st: &DownloadsState, act: &mut Act) {
    let frame = || Frame::new().fill(p.panel).corner_radius(CornerRadius::same(tokens::radius::GROUP)).shadow(p.shadow(tokens::Elevation::Card)).inner_margin(Margin::symmetric(16, 14));
    let icon_line = |ui: &mut Ui, icon: Icon, color: Color32, body: &mut dyn FnMut(&mut Ui)| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let (r, _) = ui.allocate_exact_size(vec2(16.0, 19.0), Sense::hover());
            icons::paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 9.5), vec2(16.0, 16.0)), icon, color);
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                body(ui);
            });
        });
    };
    if let Some(pr) = &st.prompt {
        frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            icon_line(ui, Icon::Info, p.text_2, &mut |ui: &mut Ui| {
                widgets::paragraph(ui, prompt_text(l, f, pr), Type::Body, p.text);
                if pr.cloud_skipped > 0 {
                    let n = pr.cloud_skipped as u64;
                    widgets::paragraph(ui, l.ta("download-confronta-cloud", &[("n", n.into()), ("testo", f.count(n).into())]), Type::Callout, p.text_2);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let key = if pr.kind == ReadKind::Hash { "download-conferma-confronta" } else { "download-conferma-leggi" };
                    if widgets::button(ui, p, &l.t(key), None, ButtonKind::Primary, true).clicked() {
                        act.confirm = true;
                    }
                    if widgets::button(ui, p, &l.t("download-annulla"), None, ButtonKind::Secondary, true).clicked() {
                        act.dismiss = true;
                    }
                });
            });
        });
        ui.add_space(14.0);
    } else if let Some((kind, files, total, bytes, total_bytes)) = st.progress() {
        frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            icon_line(ui, Icon::Refresh, p.text_2, &mut |ui: &mut Ui| {
                let args = [
                    ("file", f.count(files as u64).into()),
                    ("totale", f.count(total as u64).into()),
                    ("letti", f.size(bytes).into()),
                    ("byte", f.size(total_bytes).into()),
                ];
                let key = match kind {
                    Some(ReadKind::Hash) => "download-avanzamento-confronto",
                    Some(ReadKind::Installers) => "download-avanzamento-installatori",
                    Some(ReadKind::Zip) => "download-avanzamento-archivi",
                    None => "download-avanzamento-attributi",
                };
                widgets::paragraph(ui, l.ta(key, &args), Type::Body, p.text);
                ui.add_space(4.0);
                let ratio = if total_bytes > 0 { bytes as f64 / total_bytes as f64 } else { files as f64 / total.max(1) as f64 };
                let (r, _) = ui.allocate_exact_size(vec2(ui.available_width().min(520.0), 6.0), Sense::hover());
                widgets::share_bar(ui.painter(), r, ratio, p.text_2, p.track);
                ui.add_space(6.0);
                if widgets::button(ui, p, &l.t("download-annulla-lettura"), Some(Icon::Close), ButtonKind::Secondary, true).clicked() {
                    act.cancel = true;
                }
            });
        });
        ui.add_space(14.0);
    } else if let Some(o) = &st.outcome {
        let (icon, color) = if o.interrupted { (Icon::Warning, p.warn) } else { (Icon::StateManageable, p.ok) };
        ui.horizontal_top(|ui| {
            ui.add_space(4.0);
            ui.spacing_mut().item_spacing.x = 8.0;
            let (r, _) = ui.allocate_exact_size(vec2(14.0, 17.0), Sense::hover());
            icons::paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 8.5), vec2(14.0, 14.0)), icon, color);
            widgets::paragraph(ui, outcome_text(l, f, o), Type::Callout, if o.interrupted { p.warn } else { p.text_2 });
        });
        ui.add_space(12.0);
    }
}

// ---------------------------------------------------------------------------------------------
// Elenco "Cosa riconosce Quado in questa cartella"

const STATE_W: f32 = 168.0;

/// Card dell'elenco (`.card` con `.gh`): titolo con la regola nella "i", le voci che scorrono, la legenda
/// degli stati in fondo. Con il focus le frecce scelgono la voce.
#[allow(clippy::too_many_arguments)]
fn list(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, t: &Tree, st: &DownloadsState, v: &LensView, act: &mut Act) {
    let full = ui.available_rect_before_wrap();
    let sh = p.shadow(tokens::Elevation::Card);
    if sh != egui::epaint::Shadow::NONE {
        ui.painter().add(sh.as_shape(full, CornerRadius::same(tokens::radius::CARD)));
    }
    ui.painter().rect_filled(full, CornerRadius::same(tokens::radius::CARD), p.panel);
    let list_id = Id::new("lente_elenco");
    let focus = ui.interact(full, list_id, Sense::focusable_noninteractive());
    let a11y = l.t("download-elenco-accessibile");
    focus.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &a11y));
    if st.focus_pending {
        focus.request_focus();
        act.focus_done = true;
    }
    // Legenda dei cinque stati in fondo: altezza misurata prima, va a capo se serve.
    let inner = full.shrink2(vec2(20.0, 14.0));
    let legend = legend_items(l);
    let legend_h = legend_height(ui, &legend, inner.width());
    let mut head = ui.new_child(UiBuilder::new().max_rect(full).layout(Layout::top_down(Align::Min)));
    widgets::card_header(&mut head, p, &l.t("download-elenco-titolo"), None, Some(&l.t("download-elenco-regola")));
    let top = head.min_rect().max.y;
    let body = Rect::from_min_max(pos2(full.min.x + 8.0, top), pos2(full.max.x - 8.0, inner.max.y - legend_h - 12.0));
    let mut bui = ui.new_child(UiBuilder::new().max_rect(body).layout(Layout::top_down(Align::Min)));
    ScrollArea::vertical().id_salt("lente_voci").auto_shrink([false, false]).show(&mut bui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if v.rows.is_empty() {
            Frame::new().inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
                widgets::paragraph(ui, l.t("download-elenco-vuoto"), Type::Body, p.text_2);
            });
        }
        let shown = st.shown_rows.max(ROWS_PAGE).min(v.rows.len());
        let selected = if st.panel == Panel::Rows { st.selected } else { None };
        let mut prev_lit = false;
        for (k, row) in v.rows.iter().enumerate().take(shown) {
            let on = selected == Some(k);
            // Il filetto sopra la riga sparisce accanto a una riga accesa (scelta o sotto il puntatore).
            let next_on = selected == Some(k);
            let (clicked, lit) = row_ui(ui, p, l, f, t, v, row, on, st.scroll_to_selected, k > 0 && !prev_lit && !next_on, act);
            prev_lit = lit;
            if clicked {
                act.select = Some(k);
                ui.memory_mut(|m| m.request_focus(list_id));
            }
        }
        if v.rows.len() > shown {
            let rest = (v.rows.len() - shown) as u64;
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                let label = l.ta("download-altre-voci", &[("n", rest.into()), ("testo", f.count(rest).into())]);
                if widgets::button(ui, p, &label, None, ButtonKind::Quiet, true).clicked() {
                    act.more_rows = true;
                }
            });
            ui.add_space(8.0);
        }
    });
    let lr = Rect::from_min_max(pos2(inner.min.x, inner.max.y - legend_h), inner.max);
    ui.painter().hline(full.x_range(), lr.min.y - 7.5, Stroke::new(1.0, p.separator));
    paint_legend(ui, p, &legend, lr);
    if focus.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(list_id, EventFilter { horizontal_arrows: false, vertical_arrows: true, escape: false, ..Default::default() }));
        widgets::focus_ring_inside(ui.painter(), full, p, tokens::radius::CARD);
        let keys = table::NavKeys::read(ui);
        let current = if st.panel == Panel::Rows { st.selected } else { None };
        if let Some(r) = keys.apply(current, v.rows.len(), 5)
            && (Some(r) != current || keys.moves())
        {
            act.select = Some(r);
        }
    }
}

/// Una voce (`.arow`): titolo, spiegazione, nomi dei file; a destra la cifra, la stima (solo se il record ne
/// ha una) e lo stato con icona e parola. Restituisce (cliccata, accesa).
#[allow(clippy::too_many_arguments)]
fn row_ui(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, t: &Tree, v: &LensView, row: &Row, selected: bool, scroll: bool, separator: bool, act: &mut Act) -> (bool, bool) {
    let w = ui.available_width();
    let painter = ui.painter().clone();
    let tw = (w - 28.0 - STATE_W - 16.0).max(120.0);
    let title = row_title(l, row);
    let (expl, names) = row_explanation(l, f, t, v, row);
    let first = &v.findings[row.voci[0]];
    let state = &first.record.state;
    let tone = if matches!(state, State::ToVerify(_)) { p.warn } else { p.text_2 };
    let tg = widgets::galley(&painter, title.clone(), Type::BodyStrong, p.text, Some(tw));
    let eg = widgets::galley(&painter, expl, Type::Callout, p.text_2, Some(tw));
    let ng = names.map(|n| widgets::galley(&painter, n, Type::Callout, p.text_3, Some(tw)));
    let sg = painter.layout_no_wrap(f.size(row.bytes), widgets::num_medium(14.0), p.text);
    let estimate = row_estimate(v, row);
    let est = matches!(estimate, Estimate::Exact(_) | Estimate::Range { .. }).then(|| widgets::galley(&painter, kbtext::estimate_text(l, f, &estimate), Type::Callout, p.text_2, Some(STATE_W)));
    let stg = widgets::galley(&painter, kbtext::state_label(l, state), Type::Callout, tone, Some(STATE_W - 20.0));
    let left_h = tg.size().y + 3.0 + eg.size().y + ng.as_ref().map_or(0.0, |g| g.size().y + 2.0);
    let right_h = sg.size().y + 4.0 + est.as_ref().map_or(0.0, |g| g.size().y + 3.0) + stg.size().y;
    let h = left_h.max(right_h) + 24.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::CLICK);
    let lit = selected || resp.hovered();
    if selected {
        painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.select);
        if scroll {
            ui.scroll_to_rect(rect, None);
            act.scrolled = true;
        }
    } else if resp.hovered() {
        painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.hover);
    }
    if separator && !lit {
        painter.hline((rect.min.x + 14.0)..=(rect.max.x - 12.0), rect.min.y + 0.5, Stroke::new(1.0, p.separator));
    }
    let x = rect.min.x + 14.0;
    let mut y = rect.min.y + 12.0;
    let th = tg.size().y;
    painter.galley(pos2(x, y), tg, p.text);
    y += th + 3.0;
    let eh = eg.size().y;
    painter.galley(pos2(x, y), eg, p.text_2);
    y += eh + 2.0;
    if let Some(g) = ng {
        painter.galley(pos2(x, y), g, p.text_3);
    }
    let rx = rect.max.x - 14.0;
    let mut ry = rect.min.y + 12.0;
    let sh = sg.size().y;
    painter.galley(pos2(rx - sg.size().x, ry - 1.0), sg, p.text);
    ry += sh + 4.0;
    if let Some(g) = est {
        let gh = g.size().y;
        painter.galley(pos2(rx - g.size().x, ry), g, p.text_2);
        ry += gh + 3.0;
    }
    let sw = stg.size().x;
    let first_h = stg.rows.first().map_or(stg.size().y, |r| r.height());
    icons::paint(&painter, Rect::from_center_size(pos2(rx - sw - 12.0, ry + first_h / 2.0), vec2(14.0, 14.0)), kbtext::state_icon(state), tone);
    painter.galley(pos2(rx - sw, ry), stg, tone);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &title));
    (resp.clicked(), lit)
}

fn legend_items(l: &Lingue) -> Vec<(Icon, String, bool)> {
    vec![
        (Icon::StateManageable, l.t("stato-gestibile"), false),
        (Icon::StateToVerify, l.t("stato-da-verificare"), true),
        (Icon::StateYouDecide, l.t("stato-decidi-tu"), false),
        (Icon::StateManagedBy, l.t("download-legenda-gestisce"), false),
        (Icon::StateUnknown, l.t("stato-non-sappiamo"), false),
    ]
}

/// Posizioni della legenda: le cinque voci, a capo quando non c'è posto.
fn legend_layout(ui: &Ui, items: &[(Icon, String, bool)], width: f32) -> (Vec<(Rect, usize)>, f32) {
    let painter = ui.painter();
    let mut out = Vec::new();
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let line = 18.0;
    for (k, (_, s, _)) in items.iter().enumerate() {
        let w = 14.0 + 6.0 + painter.layout_no_wrap(s.clone(), widgets::font(Type::Callout), Color32::WHITE).size().x;
        if x > 0.0 && x + w > width {
            x = 0.0;
            y += line + 4.0;
        }
        out.push((Rect::from_min_size(pos2(x, y), vec2(w, line)), k));
        x += w + 18.0;
    }
    (out, y + line)
}

fn legend_height(ui: &Ui, items: &[(Icon, String, bool)], width: f32) -> f32 {
    legend_layout(ui, items, width).1
}

fn paint_legend(ui: &Ui, p: &Palette, items: &[(Icon, String, bool)], rect: Rect) {
    let (pos, _) = legend_layout(ui, items, rect.width());
    let painter = ui.painter();
    for (r, k) in pos {
        let (icon, s, warn) = &items[k];
        let tone = if *warn { p.warn } else { p.text_2 };
        let r = r.translate(rect.min.to_vec2());
        icons::paint(painter, Rect::from_min_size(pos2(r.min.x, r.min.y + 2.0), vec2(14.0, 14.0)), *icon, tone);
        painter.text(pos2(r.min.x + 20.0, r.center().y), egui::Align2::LEFT_CENTER, s, widgets::font(Type::Callout), tone);
    }
}

// ---------------------------------------------------------------------------------------------
// Dettaglio

/// Il dettaglio a destra (`.insp`): la voce scelta, una famiglia o i file con "(N)".
#[allow(clippy::too_many_arguments)]
fn detail(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, d: &Loaded, st: &DownloadsState, v: &LensView, act: &mut Act) {
    ScrollArea::vertical().id_salt("lente_dettaglio").auto_shrink([false, false]).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match st.panel {
            Panel::Family(fam) => family_detail(ui, p, l, f, d.tree(), v, fam, act),
            Panel::Suffix => suffix_detail(ui, p, l, f, d.tree(), v, act),
            Panel::Rows => match st.selected.and_then(|k| v.rows.get(k)) {
                Some(row) => row_detail(ui, p, l, f, d, st, v, row, act),
                None => {
                    widgets::paragraph(ui, l.t("download-dettaglio-vuoto"), Type::Body, p.text_2);
                }
            },
        }
        ui.add_space(12.0);
    });
}

/// Testata del dettaglio, come l'ispettore: titolo e la riga con file e dimensioni.
fn heading(ui: &mut Ui, p: &Palette, title: &str, sub: &str) {
    widgets::paragraph(ui, title, Type::Title, p.text);
    widgets::paragraph(ui, sub, Type::Callout, p.text_2);
}

fn note(ui: &mut Ui, p: &Palette, s: &str) {
    widgets::paragraph(ui, s, Type::Callout, p.text_2);
}

/// Titolo di una parte del dettaglio (`.insp h3`).
fn part_title(ui: &mut Ui, p: &Palette, s: &str) {
    ui.add_space(18.0);
    widgets::typed(ui, s, Type::BodyStrong, p.text);
    ui.add_space(6.0);
}

/// Livello dei fatti dedotti delle voci, se ce ne sono: per i record senza fatti dedotti la riga non si
/// mostra.
fn deduced_level(v: &LensView, row: &Row) -> Option<quado_model::record::Criterion> {
    row.voci
        .iter()
        .map(|&k| &v.findings[k].record)
        .filter(|r| kbtext::has_deduced(r))
        .map(|r| r.reliability)
        .min_by_key(|c| c.level())
}

/// Dettaglio di una voce: testata, fatti con la provenienza, "Cosa puoi fare" in un riquadro dal record (con
/// le letture che servono alla voce), poi i file con la casella o la scelta della copia da tenere.
#[allow(clippy::too_many_arguments)]
fn row_detail(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, d: &Loaded, st: &DownloadsState, v: &LensView, row: &Row, act: &mut Act) {
    let t = d.tree();
    let sub = l.ta("download-dettaglio-sottotitolo", &[("file", files_text(l, f, row.files as u64).into()), ("dimensione", f.size(row.bytes).into())]);
    heading(ui, p, &row_title(l, row), &sub);
    let one = row.voci.len() == 1;
    // Una voce sola: i fatti subito sotto la testata. Più voci: ognuna con i suoi fatti e i suoi file più giù.
    if one {
        let voce = &v.findings[row.voci[0]];
        ui.add_space(10.0);
        voce_facts(ui, p, l, f, t, st, voce);
    }
    if let Some(c) = deduced_level(v, row) {
        ui.add_space(4.0);
        note(ui, p, &kbtext::reliability_label(l, c));
    }
    // Cosa puoi fare: dal record, come nell'ispettore; la stima è la somma delle voci.
    part_title(ui, p, &l.t("ispettore-cosa-puoi-fare"));
    Frame::new().fill(p.panel).corner_radius(CornerRadius::same(tokens::radius::GROUP)).shadow(p.shadow(tokens::Elevation::Card)).inner_margin(Margin::same(14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 3.0;
        let first = &v.findings[row.voci[0]];
        let mut req: Option<KbRequest> = None;
        inspector::record_block(ui, p, l, f, &first.record, &row_estimate(v, row), &mut req);
        match req {
            Some(KbRequest::Copy(c)) => act.copy = Some(c),
            Some(KbRequest::Act(a)) => act.intervention = Some((a, row.voci.clone())),
            _ => {}
        }
        // Letture che servono a questa voce.
        let command = match row.finding {
            Finding::SameSize => Some(("download-comando-confronta", Icon::Verify, ReadKind::Hash, hash_avail(st, v))),
            Finding::Installer(s) if s != InstallerStatus::CloudOnly => Some(("download-comando-installatori", Icon::Info, ReadKind::Installers, installers_avail(st, v, t))),
            Finding::ArchiveWithFolder { .. } => Some(("download-comando-archivi", Icon::Explore, ReadKind::Zip, zip_avail(st, v, t))),
            _ => None,
        };
        if matches!(row.finding, Finding::Installer(InstallerStatus::NotRead)) && v.other_installers > 0 {
            ui.add_space(4.0);
            note(ui, p, &l.t("download-installatori-altri-tipi"));
        }
        if let Some((key, icon, kind, avail)) = command {
            ui.add_space(8.0);
            let w = ui.available_width();
            if inspector::wide_button(ui, p, &l.t(key), Some(icon), w, true, avail == Avail::Ready).clicked() {
                act.request = Some(kind);
            }
            if let Avail::No(m) = avail {
                note(ui, p, &l.t(m));
            }
        }
        if matches!(row.finding, Finding::Incomplete(_)) {
            ui.add_space(8.0);
            let busy = st.checking_usage();
            let w = ui.available_width();
            let r = inspector::wide_button(ui, p, &l.t("download-comando-uso"), Some(Icon::Refresh), w, true, !busy);
            if r.clicked() {
                act.recheck = true;
            }
            r.on_hover_text(l.t("download-comando-uso-suggerimento"));
        }
        ui.add_space(6.0);
        note(ui, p, &l.t("download-origine"));
    });
    if one {
        let voce = &v.findings[row.voci[0]];
        part_title(ui, p, &files_text(l, f, voce.nodes.len() as u64));
        files(ui, p, l, f, t, voce, &v.created, act);
    } else {
        for (k, &vi) in row.voci.iter().enumerate().take(MAX_VOCI) {
            let voce = &v.findings[vi];
            if k == 0 {
                ui.add_space(8.0);
            }
            ui.add_space(14.0);
            widgets::paragraph(ui, names(l, f, t, &voce.nodes), Type::BodyStrong, p.text);
            ui.add_space(2.0);
            voce_facts(ui, p, l, f, t, st, voce);
            files(ui, p, l, f, t, voce, &v.created, act);
        }
        if row.voci.len() > MAX_VOCI {
            let rest = (row.voci.len() - MAX_VOCI) as u64;
            ui.add_space(6.0);
            note(ui, p, &l.ta("download-dettaglio-altre-voci", &[("n", rest.into()), ("testo", f.count(rest).into())]));
        }
    }
}

/// Fatti di una voce con la loro provenienza, la prova di gestibilità e i file non letti con il motivo.
fn voce_facts(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, t: &Tree, st: &DownloadsState, voce: &LensFinding) {
    for fa in &voce.record.identity.0 {
        inspector::fact_in(ui, p, kbtext::evidence(fa.provenance), &kbtext::fact_with_source(l, f, fa), l, p.text);
    }
    if let Some(proof) = voce.record.state.manageability() {
        ui.add_space(4.0);
        widgets::typed(ui, l.t("ispettore-prova-gestibilita"), Type::Caption, p.text_2);
        for fa in proof.facts() {
            inspector::fact_in(ui, p, kbtext::evidence(fa.provenance), &kbtext::fact_with_source(l, f, fa), l, p.text);
        }
    }
    for n in voce.nodes.iter().filter_map(|n| st.hash_errors.get(n).map(|e| (n, e))) {
        let s = l.ta("download-non-letto", &[("nome", t.name_lossy(*n.0).into()), ("motivo", content_error(l, n.1).into())]);
        inspector::fact_in(ui, p, Evidence::Unavailable, &s, l, p.text);
    }
}

/// Casella o scelta della copia da tenere accanto a un file.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    None,
    /// Prima del confronto: caselle disattivate.
    Disabled,
    /// Dopo il confronto: la copia da tenere.
    Keep(u32),
}

/// I file di una voce, con la casella o la scelta della copia da tenere, la data e le azioni sempre
/// disponibili ("Apri in Esplora file", "Copia percorso").
#[allow(clippy::too_many_arguments)]
fn files(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, t: &Tree, voce: &LensFinding, created: &HashMap<u32, i64>, act: &mut Act) {
    let control = match voce.finding {
        Finding::SameSize => Control::Disabled,
        Finding::IdenticalCopies { keep } => Control::Keep(keep),
        _ => Control::None,
    };
    match control {
        Control::Disabled => note(ui, p, &l.t("download-caselle-dopo")),
        Control::Keep(_) => note(ui, p, &l.t("download-tieni-nota")),
        Control::None => {}
    }
    ui.add_space(4.0);
    for &n in voce.nodes.iter().take(MAX_FILES) {
        file_row(ui, p, l, f, t, n, control, created.get(&n).copied(), &voce.nodes, act);
    }
    if voce.nodes.len() > MAX_FILES {
        let rest = (voce.nodes.len() - MAX_FILES) as u64;
        note(ui, p, &l.ta("download-dettaglio-altri-file", &[("n", rest.into()), ("testo", f.count(rest).into())]));
    }
}

/// Riga di un file: casella o scelta (area di 32 punti), nome e data, dimensioni, i due pulsanti con l'icona.
#[allow(clippy::too_many_arguments)]
fn file_row(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, t: &Tree, n: u32, control: Control, created: Option<i64>, group: &[u32], act: &mut Act) {
    if n as usize >= t.len() || t.flags(n).has(Flags::DETACHED) {
        return;
    }
    let w = ui.available_width();
    let painter = ui.painter().clone();
    let ctrl_w = if control == Control::None { 0.0 } else { 26.0 };
    let size = painter.layout_no_wrap(f.size(t.alloc(n)), widgets::num(12.5), p.text);
    let buttons_w = 2.0 * tokens::focus::MIN_TARGET;
    let name_w = (w - ctrl_w - size.size().x - buttons_w - 14.0).max(80.0);
    let name = t.name_lossy(n);
    let ng = widgets::galley(&painter, name.clone(), Type::Body, p.text, Some(name_w));
    let kept = control == Control::Keep(n);
    let mut sub = match created {
        Some(c) if matches!(control, Control::Keep(_)) => l.ta("download-file-creato", &[("data", f.date(c).into())]),
        _ => l.ta("download-file-modificato", &[("data", f.date(t.mtime(n)).into())]),
    };
    if let Control::Keep(_) = control {
        sub.push_str(" · ");
        sub.push_str(&l.t(if kept { "download-tenuta" } else { "download-da-spostare" }));
    }
    let sg = widgets::galley(&painter, sub, Type::Callout, p.text_2, Some(name_w));
    let h = (ng.size().y + 2.0 + sg.size().y).max(tokens::focus::MIN_TARGET) + 12.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    painter.hline(rect.x_range(), rect.min.y + 0.5, Stroke::new(1.0, p.separator));
    let top = rect.min.y + 6.0;
    let first = ng.rows.first().map_or(ng.size().y, |r| r.height());
    let line_cy = top + first / 2.0;
    match control {
        Control::None => {}
        Control::Disabled => {
            let b = Rect::from_center_size(pos2(rect.min.x + 8.0, line_cy), vec2(16.0, 16.0));
            let resp = ui.interact(b.expand(8.0), ui.id().with(("lente_casella", n)), Sense::hover());
            painter.rect(b, CornerRadius::same(4), p.field, Stroke::new(1.5, p.border), egui::StrokeKind::Inside);
            let label = l.ta("download-casella", &[("nome", name.clone().into())]);
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, false, false, &label));
            resp.on_hover_text(l.t("download-caselle-dopo"));
        }
        Control::Keep(_) => {
            let c = pos2(rect.min.x + 8.0, line_cy);
            let resp = ui.interact(Rect::from_center_size(c, vec2(tokens::focus::MIN_TARGET, tokens::focus::MIN_TARGET)), ui.id().with(("lente_tieni", n)), Sense::click());
            painter.circle(c, 7.5, if resp.hovered() { p.hover } else { p.panel }, Stroke::new(1.5, if kept { p.accent } else { p.text_3 }));
            if kept {
                painter.circle_filled(c, 3.8, p.accent);
            }
            if resp.has_focus() && widgets::focus_visible(ui.ctx()) {
                painter.circle_stroke(c, 11.0, Stroke::new(tokens::focus::WIDTH, p.ring));
            }
            let label = l.ta("download-tieni", &[("nome", name.clone().into())]);
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, kept, &label));
            if (resp.clicked() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))) && !kept {
                act.keep = Some((group.to_vec(), n));
            }
        }
    }
    let x = rect.min.x + ctrl_w;
    let nh = ng.size().y;
    painter.galley(pos2(x, top), ng, p.text);
    painter.galley(pos2(x, top + nh + 2.0), sg, p.text_2);
    let sx = rect.max.x - buttons_w - 6.0 - size.size().x;
    painter.galley(pos2(sx, line_cy - size.size().y / 2.0), size, p.text);
    let path = t.path_lossy(n);
    for (k, (icon, key)) in [(Icon::External, "azione-apri-in-esplora"), (Icon::Copy, "azione-copia-percorso")].into_iter().enumerate() {
        let r = Rect::from_center_size(pos2(rect.max.x - buttons_w + (k as f32 + 0.5) * tokens::focus::MIN_TARGET, line_cy), vec2(tokens::focus::MIN_TARGET, tokens::focus::MIN_TARGET));
        let mut bui = ui.new_child(UiBuilder::new().max_rect(r).layout(Layout::left_to_right(Align::Center)));
        // Il nome accessibile dice l'azione e il file; il suggerimento lo stesso.
        let label = format!("{}: {name}", l.t(key));
        let resp = bui.push_id(("lente_azione", n, k), |ui| widgets::icon_button(ui, p, icon, &label, true)).inner;
        if resp.clicked() {
            if k == 0 {
                act.reveal = Some(path.clone());
            } else {
                act.copy = Some(path.clone());
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn family_detail(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, t: &Tree, v: &LensView, fam: LensFamily, act: &mut Act) {
    let total = v.lens.total(fam);
    let sub = l.ta("download-dettaglio-sottotitolo", &[("file", files_text(l, f, total.files).into()), ("dimensione", f.size(total.alloc).into())]);
    heading(ui, p, &l.t(&format!("lente-famiglia-{}", fam.code())), &sub);
    ui.add_space(8.0);
    note(ui, p, &l.t("download-famiglia-nota"));
    ui.add_space(8.0);
    let nodes = v.family_top.get(&fam).cloned().unwrap_or_default();
    for &n in &nodes {
        file_row(ui, p, l, f, t, n, Control::None, None, &nodes, act);
    }
    if total.files > nodes.len() as u64 {
        let rest = total.files - nodes.len() as u64;
        ui.add_space(6.0);
        note(ui, p, &l.ta("download-dettaglio-altri-file", &[("n", rest.into()), ("testo", f.count(rest).into())]));
    }
}

fn suffix_detail(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, t: &Tree, v: &LensView, act: &mut Act) {
    let n = v.suffixed.len() as u64;
    let bytes: u64 = v.suffixed.iter().map(|&x| t.alloc(x)).sum();
    let sub = l.ta("download-dettaglio-sottotitolo", &[("file", files_text(l, f, n).into()), ("dimensione", f.size(bytes).into())]);
    heading(ui, p, &l.t("download-filtro-titolo"), &sub);
    ui.add_space(8.0);
    note(ui, p, &l.t("download-filtro-nota"));
    if v.suffixed.is_empty() {
        ui.add_space(6.0);
        note(ui, p, &l.t("download-filtro-vuoto"));
    }
    ui.add_space(8.0);
    let shown: Vec<u32> = v.suffixed.iter().copied().take(MAX_FILES * 3).collect();
    for &x in &shown {
        file_row(ui, p, l, f, t, x, Control::None, None, &shown, act);
    }
    if v.suffixed.len() > shown.len() {
        let rest = (v.suffixed.len() - shown.len()) as u64;
        ui.add_space(6.0);
        note(ui, p, &l.ta("download-dettaglio-altri-file", &[("n", rest.into()), ("testo", f.count(rest).into())]));
    }
}

/// Voci della lente con un'azione, per la card "Interventi possibili" della Panoramica: le copie identiche
/// dopo il confronto e i download incompleti non in uso, mai le copie non confrontate. Raggruppate per
/// funzione: (funzione, voci, byte, stima, stato della prima).
pub fn interventions(v: &LensView) -> Vec<(quado_model::record::Function, Vec<usize>, u64, Estimate, &Record)> {
    let mut out: Vec<(quado_model::record::Function, Vec<usize>, u64, Estimate, &Record)> = Vec::new();
    for (k, x) in v.findings.iter().enumerate() {
        let r = &x.record;
        let quado_model::record::Origin::Function(func) = r.origin else { continue };
        if !r.state.proposes() || matches!(r.action, quado_model::record::Action::None | quado_model::record::Action::Uninstall) {
            continue;
        }
        match out.iter_mut().find(|e| e.0 == func) {
            Some(e) => {
                e.1.push(k);
                e.2 += x.bytes;
                e.3 = engine::sum_estimates(e.3, r.estimate);
            }
            None => out.push((func, vec![k], x.bytes, r.estimate, r)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::synthetic;

    fn loaded() -> Loaded {
        let (v, r) = synthetic::sample_c(false);
        Loaded::with_rules(
            v,
            r,
            Duration::ZERO,
            Some("S-1-5-21-1-2-3-1001"),
            Arc::new(quado_kb::Kb::embedded().unwrap_or(quado_kb::Kb { rules: Vec::new() })),
            synthetic::sample_c_evidence(),
        )
    }

    #[test]
    fn every_family_has_its_name() {
        for tag in ["it", "en"] {
            let l = Lingue::load(tag, None);
            for fam in LensFamily::ALL {
                let k = format!("lente-famiglia-{}", fam.code());
                assert!(l.has(&k) && !l.t(&k).starts_with('['), "{tag}: {k}");
            }
        }
    }

    #[test]
    fn rows_group_equal_findings() {
        let d = loaded();
        let root = engine::find(d.tree(), r"C:\Users\demo\Downloads").expect("Download");
        let v = build_view(&d.model, root, d.generation, None, &Readings::default(), 0);
        // Ogni voce sta in una riga sola, e una riga ha voci dello stesso tipo.
        let mut seen = HashSet::new();
        for r in &v.rows {
            for &k in &r.voci {
                assert!(seen.insert(k));
                assert_eq!(v.findings[k].finding, r.finding);
            }
        }
        assert_eq!(seen.len(), v.findings.len());
        assert!(v.rows.windows(2).all(|w| w[0].bytes >= w[1].bytes));
        // Prima di ogni lettura nessuna voce della lente entra nella card interventi.
        assert!(interventions(&v).is_empty());
    }

    /// La lente su una cartella con 100.000 file si costruisce fuori dal frame; il tempo si legge con
    /// `--nocapture` (in release, circa quello dell'app).
    #[test]
    fn a_big_folder_builds_in_the_background() {
        let (v, r, folder) = synthetic::wide_folder(100_000);
        let d = Loaded::new(v, r, Duration::ZERO, None);
        let t0 = Instant::now();
        let view = build_view(&d.model, folder, d.generation, None, &Readings::default(), 0);
        eprintln!("lente su 100.000 file: {:.1} ms (build_view {:.1} ms)", t0.elapsed().as_secs_f64() * 1000.0, view.elapsed.as_secs_f64() * 1000.0);
        assert_eq!(view.lens.files, 100_000);
        assert_eq!(view.rows.iter().map(|r| r.voci.len()).sum::<usize>(), view.findings.len());
    }
}
