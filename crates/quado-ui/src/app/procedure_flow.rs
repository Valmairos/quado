// SPDX-License-Identifier: GPL-3.0-or-later
//! Procedure e comandi (03 F-AC-1, F-VE-1, F-VE-2; 04 §6.7 "Procedure esterne", "Disinstallazione di un
//! programma", "Liberare davvero lo spazio"): misura di partenza prima di aprire la pagina con il
//! lanciatore tipizzato, prima di copiare un comando, prima di svuotare il Cestino o di aprire l'interfaccia
//! di disinstallazione; "Ho eseguito la procedura" con la nuova misura, la riscansione dei sottoalberi
//! toccati (F-VE-4), il confronto con la stima (F-VE-3, 03 §6.3) e il registro (F-AC-6). Quado non esegue
//! niente: apre URI tipizzati, copia comandi negli appunti, chiede a Windows il suo dialogo di svuotamento.
//!
//! - Una procedura aperta resta in attesa per il suo soggetto ([`Subject`]: la regola, o il programma della
//!   vista Programmi) con la misura di partenza. Ogni "Ho eseguito la procedura" aggiunge una misura allo
//!   stesso intervento: stesso `id` nel registro, dove vale l'ultima riga.
//! - Senza misura di partenza (procedura non aperta da Quado) la variazione non si calcola: si registra la
//!   sola misura e lo si dice.
//! - I pulsanti sotto il record (`inspector::action_buttons`) non vedono `App`: `poll` pubblica lo stato
//!   condiviso nei dati temporanei del contesto di egui, e i pulsanti lo leggono da lì.
//! - La riscansione passa da `Env::actions` (`Actions::rescan`): quella vera nell'app, l'albero simulato
//!   o una funzione del test nei test, così nessun test legge il disco vero.
//!
//! M5, agente E2.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use egui::{Id, Ui, vec2};
use quado_actions::journal::{self, JournalError};
use quado_actions::launch::Launch;
use quado_actions::measure;
use quado_actions::procedures::EMPTY_RECYCLE_BIN;
use quado_model::intervention::{BinCount, Comparison, InterventionKind, JournalEntry, Measure, MeasurePoint};
use quado_model::record::{Action, Estimate, Record};
use quado_model::verification::{self, ExplainFacts};
use quado_model::Tree;

use super::data::{self, Loaded, Task};
use super::inspector::KbRequest;
use super::intervention_jobs::{self, ActRequest, Origin, Rescanned};
use super::interventions_view::{self, History};
use super::programs_view::{Outcome, UninstallDialog};
use super::{App, Screen};
use crate::format::Formatter;
use crate::i18n::Lingue;
use crate::tokens::Palette;
use crate::widgets::{self, ButtonKind};

/// Codice della disinstallazione nel registro (`InterventionKind::Procedure`); l'URI aperto sta nel metodo.
pub const UNINSTALL: &str = "disinstallazione";

/// Ogni quanto si rilegge il Cestino secondo Windows mentre il suo record è visibile.
const BIN_REFRESH: Duration = Duration::from_secs(5);

/// Di chi è una procedura: l'elemento riconosciuto da una regola o un programma della vista Programmi.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Subject {
    /// Regola (`Record::origin_id`).
    Rule(String),
    /// Programma, per nome (gli indici cambiano quando il modello si aggiorna).
    Program(String),
}

/// Cosa serve per misurare e registrare: preso dal record quando la procedura si apre.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Target {
    subject: Subject,
    kind: InterventionKind,
    /// `Record::origin_id`.
    origin: String,
    /// URI della pagina o comando copiato.
    method: String,
    estimate: Estimate,
    /// Radice del volume misurato (`C:\`).
    volume: String,
    /// Percorsi degli elementi del record: quanto occupano ancora si legge dopo la riscansione.
    paths: Vec<String>,
    /// Cartelle da riscansionare; vuoto con `full_scan`.
    dirs: Vec<String>,
    /// Tra le cartelle c'è la radice del volume: serve una scansione completa («Scansiona di nuovo»), e la
    /// riscansione a metà non si fa.
    full_scan: bool,
}

/// Una procedura aperta (o un comando copiato), in attesa di "Ho eseguito la procedura".
#[derive(Clone, Debug, PartialEq, Eq)]
struct Opened {
    target: Target,
    /// Misura di partenza, o il codice di Windows se lo spazio libero non si è letto.
    start: Result<Measure, u32>,
    /// Misure dopo la procedura già registrate per questo intervento.
    after: Vec<Measure>,
    /// Pagina aperta: `None` per un comando copiato o una misura senza pagina.
    page: Option<Launch>,
    /// Errore della shell all'apertura (HRESULT).
    shell_error: Option<i32>,
}

/// Esito di una misura dopo la procedura o dopo lo svuotamento, mostrato vicino ai pulsanti.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Ora e spazio libero della misura.
    pub at: i64,
    pub free: u64,
    /// Confronto con la misura di partenza; `None` senza misura di partenza.
    pub comparison: Option<Comparison>,
    /// Byte che gli elementi del record occupano ancora secondo la riscansione.
    pub still_present: Option<u64>,
    /// Serve «Scansiona di nuovo»: gli elementi stanno nella radice del volume.
    pub full_scan: bool,
    /// Cartelle non riscansionate: percorso e codice di Windows.
    pub rescan_failed: Vec<(String, u32)>,
    /// La misura non si è presa: codice di Windows (niente registro).
    pub measure_error: Option<u32>,
    /// Svuotamento: la misura di partenza non si è presa (codice di Windows).
    pub start_error: Option<u32>,
    /// Svuotamento: codice restituito da Windows (l'esito vero lo dicono le misure).
    pub shell_error: Option<i32>,
    /// Scrittura nel registro; `None` senza cartella dei dati.
    pub journal: Option<Result<(), JournalError>>,
}

impl Report {
    /// Variazione del volume, se c'è la misura di partenza.
    pub fn obtained(&self) -> Option<i64> {
        self.comparison.as_ref().map(|c| c.obtained)
    }
}

/// Cestino del volume secondo Windows, per il testo prima di "Svuota il Cestino…".
#[derive(Clone, Debug, Default)]
struct BinState {
    volume: String,
    /// Contenuto (`None` se Windows non lo dice) o il codice dell'errore.
    read: Option<Result<Option<BinCount>, u32>>,
    at: Option<Instant>,
    /// Il record del Cestino è visibile: la lettura serve.
    wanted: bool,
}

/// Stato visto dai pulsanti sotto il record: `poll` lo pubblica nei dati temporanei di egui.
#[derive(Debug, Default)]
pub struct Shared {
    opened: HashMap<Subject, Opened>,
    reports: HashMap<Subject, Report>,
    busy: Vec<Subject>,
    bin: BinState,
}

fn lock(s: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    s.lock().unwrap_or_else(PoisonError::into_inner)
}

fn board_id() -> Id {
    Id::new("quado_procedure_flow")
}

/// Lo stato pubblicato da `poll` (assente prima del primo frame).
fn board(ctx: &egui::Context) -> Option<Arc<Mutex<Shared>>> {
    ctx.data(|d| d.get_temp::<Arc<Mutex<Shared>>>(board_id()))
}

struct StartOut {
    start: Result<Measure, u32>,
    page: Option<Launch>,
    shell_error: Option<i32>,
}

struct AfterOut {
    /// Misura di partenza presa nello stesso lavoro (svuotamento del Cestino).
    start: Option<Result<Measure, u32>>,
    measure: Result<Measure, u32>,
    shell_error: Option<i32>,
}

enum Step {
    /// Misura di partenza, poi la pagina se c'è.
    Start(Task<StartOut>),
    /// Misura dopo la procedura (o svuotamento con le sue due misure).
    After(Task<AfterOut>),
    /// Riscansione dei sottoalberi toccati.
    Rescan(AfterOut, Task<Vec<Rescanned>>),
}

struct Job {
    target: Target,
    step: Step,
}

/// Lettura del Cestino secondo Windows: radice del volume e contenuto (o il codice dell'errore).
type BinRead = (String, Result<Option<BinCount>, u32>);

/// Procedure aperte in questa sessione, lavori in background e registro letto.
#[derive(Default)]
pub struct ProcedureState {
    shared: Arc<Mutex<Shared>>,
    jobs: Vec<Job>,
    bin_task: Option<Task<BinRead>>,
    /// Contesto di egui, per copiare un comando dagli appunti fuori dal disegno.
    ctx: Option<egui::Context>,
    /// Programma il cui dialogo di disinstallazione si riapre dopo la riscansione (che lo chiude).
    reopen: Option<String>,
    /// Registro degli interventi, riletto quando il file cambia (vista Interventi, barra laterale).
    pub history: History,
}

impl ProcedureState {
    /// Misure, riscansioni o letture del Cestino in corso.
    pub fn busy(&self) -> bool {
        !self.jobs.is_empty() || self.bin_task.is_some()
    }

    /// Misura di partenza della procedura aperta per `s`.
    pub fn start_measure(&self, s: &Subject) -> Option<Result<Measure, u32>> {
        lock(&self.shared).opened.get(s).map(|o| o.start)
    }

    /// Ultimo esito di "Ho eseguito la procedura" o dello svuotamento per `s`.
    pub fn report(&self, s: &Subject) -> Option<Report> {
        lock(&self.shared).reports.get(s).cloned()
    }

    fn update_busy(&self) {
        lock(&self.shared).busy = self.jobs.iter().map(|j| j.target.subject.clone()).collect();
    }

    fn is_busy(&self, s: &Subject) -> bool {
        self.jobs.iter().any(|j| &j.target.subject == s)
    }

    fn push(&mut self, job: Job) {
        self.jobs.push(job);
        self.update_busy();
    }
}

// ---------------------------------------------------------------------------------------------
// Cosa si misura e si riscansiona

/// Radice dell'unità di `path` (`D:\`), o `fallback`.
fn drive_root(path: &str, fallback: &str) -> String {
    match path.as_bytes() {
        [l, b':', b'\\', ..] if l.is_ascii_alphabetic() => path[..3].to_owned(),
        _ => fallback.to_owned(),
    }
}

/// Elementi del record e cartelle da riscansionare: le cartelle stesse (se spariscono, la riscansione le
/// stacca dal modello), per i file la cartella che li contiene.
#[allow(clippy::too_many_arguments)]
fn touched(d: &Loaded, subject: Subject, kind: InterventionKind, origin: String, method: String, estimate: Estimate, items: &[u32], rescan: &[u32]) -> Target {
    let t = d.tree();
    let paths = items.iter().map(|&n| t.path_lossy(n)).collect();
    let (mut files, mut dirs) = (Vec::new(), Vec::new());
    for &n in rescan {
        if t.is_dir(n) { dirs.push(t.path_lossy(n)) } else { files.push(t.path_lossy(n)) }
    }
    let dirs = intervention_jobs::rescan_dirs(&files, &dirs);
    let full_scan = rescan.contains(&Tree::ROOT) || dirs.iter().any(|p| intervention_jobs::find_node(t, p) == Some(Tree::ROOT));
    Target { subject, kind, origin, method, estimate, volume: d.volume.root.clone(), paths, dirs: if full_scan { Vec::new() } else { dirs }, full_scan }
}

/// Comando del metodo (`None`) o dell'alternativa `i`.
fn step_command(r: &Record, alternative: Option<usize>) -> Option<String> {
    match alternative {
        None => r.preferred.as_ref(),
        Some(i) => r.alternatives.get(i),
    }?
    .command
    .clone()
}

/// Procedura o comando dell'elemento riconosciuto `m` (indice in `Classification::matches`).
fn rule_target(d: &Loaded, m: usize) -> Option<Target> {
    let mk = d.cls.matches.get(m)?;
    let r = &mk.record;
    let origin = r.origin_id().to_owned();
    let (kind, method) = match r.action {
        Action::Command => (InterventionKind::Command(origin.clone()), step_command(r, None).unwrap_or_default()),
        _ => {
            let uri = r.preferred.as_ref().and_then(|s| s.uri.clone());
            // Il codice della procedura viene dal lanciatore; per le procedure dentro un'app è la regola.
            let code = uri.as_deref().and_then(Launch::for_rule_uri).and_then(|l| l.procedure_code()).map_or_else(|| origin.clone(), str::to_owned);
            (InterventionKind::Procedure(code), uri.unwrap_or_default())
        }
    };
    Some(touched(d, Subject::Rule(origin.clone()), kind, origin, method, r.estimate, &mk.nodes, &mk.nodes))
}

/// Disinstallazione del programma `i` (indice in `Programs::items`): la cartella del programma, e per la
/// riscansione l'area che la contiene con i suoi dati quando c'è (CapCut).
fn program_target(d: &Loaded, i: usize) -> Option<Target> {
    let x = d.programs.items.get(i)?;
    let folder = x.location.node();
    let items: Vec<u32> = folder.into_iter().collect();
    let rescan: Vec<u32> = x.area.or(folder).into_iter().collect();
    let r = &x.record;
    let method = r.preferred.as_ref().and_then(|s| s.uri.clone().or_else(|| s.command.clone())).unwrap_or_default();
    let mut t = touched(d, Subject::Program(x.name.clone()), InterventionKind::Procedure(UNINSTALL.to_owned()), r.origin_id().to_owned(), method, r.estimate, &items, &rescan);
    if let quado_kb::programs::Location::Folder { path, .. } = &x.location {
        t.volume = drive_root(path, &d.volume.root);
    }
    Some(t)
}

// ---------------------------------------------------------------------------------------------
// Richieste dei pulsanti

/// Misura di partenza in un thread, poi l'apertura della pagina (F-VE-1): la misura precede sempre la
/// chiamata al lanciatore.
fn spawn_start(app: &mut App, target: Target, page: Option<Launch>) {
    if app.procedures.is_busy(&target.subject) {
        return;
    }
    let probe = Arc::clone(&app.env.actions.probe);
    let launcher = app.env.actions.launcher;
    let root = target.volume.clone();
    let task = Task::spawn(move || {
        let start = measure::measure(&*probe, &root, MeasurePoint::Start);
        let shell_error = page.as_ref().and_then(|l| launcher(l).err().map(|e| e.0));
        StartOut { start, page, shell_error }
    });
    app.procedures.push(Job { target, step: Step::Start(task) });
}

/// Misura di partenza e apertura della pagina della procedura di `origin` (URI del metodo, solo tra quelli
/// che il lanciatore conosce).
pub fn open(app: &mut App, origin: Origin) {
    let Origin::Rule(m) = origin else { return };
    let Some(target) = app.data.as_ref().and_then(|d| rule_target(d, m)) else { return };
    let Some(page) = Launch::for_rule_uri(&target.method) else {
        app.notice.set(app.lingue.t("procedura-pagina-non-valida"));
        return;
    };
    spawn_start(app, target, Some(page));
}

/// Misura di partenza senza aprire niente: procedure dentro un'app, che Quado non apre.
pub fn measure_start(app: &mut App, origin: Origin) {
    let Origin::Rule(m) = origin else { return };
    if let Some(target) = app.data.as_ref().and_then(|d| rule_target(d, m)) {
        spawn_start(app, target, None);
    }
}

/// Copia negli appunti il comando del metodo o di un'alternativa e prende la misura di partenza. Quado
/// non lo esegue (F-AC-1).
pub fn copy_command(app: &mut App, origin: Origin, alternative: Option<usize>) {
    let Origin::Rule(m) = origin else { return };
    let Some(d) = app.data.as_ref() else { return };
    let Some(command) = d.cls.matches.get(m).and_then(|mk| step_command(&mk.record, alternative)) else { return };
    let Some(mut target) = rule_target(d, m) else { return };
    if let Some(ctx) = &app.procedures.ctx {
        ctx.copy_text(command.clone());
    }
    app.notice.set(app.lingue.t("avviso-comando-copiato"));
    target.method = command;
    spawn_start(app, target, None);
}

/// "Ho eseguito la procedura" per `origin`: misura dopo la procedura, riscansione, confronto, registro.
pub fn done(app: &mut App, origin: Origin) {
    let fresh = match origin {
        Origin::Rule(m) => app.data.as_ref().and_then(|d| rule_target(d, m)),
        Origin::Program(i) => app.data.as_ref().and_then(|d| program_target(d, i)),
        Origin::Lens(_) => None,
    };
    let Some(fresh) = fresh else { return };
    if app.procedures.is_busy(&fresh.subject) {
        return;
    }
    // La procedura aperta da Quado porta il suo metodo (pagina o comando copiato) e i suoi elementi.
    let target = lock(&app.procedures.shared).opened.get(&fresh.subject).map(|o| o.target.clone()).unwrap_or(fresh);
    let probe = Arc::clone(&app.env.actions.probe);
    let root = target.volume.clone();
    let task = Task::spawn(move || AfterOut { start: None, measure: measure::measure(&*probe, &root, MeasurePoint::AfterProcedure), shell_error: None });
    app.procedures.push(Job { target, step: Step::After(task) });
}

/// "Svuota il Cestino…" dal record del Cestino: in un thread la misura di partenza, il dialogo di Windows
/// (che chiede conferma; se l'utente annulla non elimina niente) e la misura dopo lo svuotamento.
pub fn empty_bin(app: &mut App, origin: Origin) {
    let Origin::Rule(m) = origin else { return };
    let Some(d) = app.data.as_ref() else { return };
    let Some(mut target) = rule_target(d, m) else { return };
    target.kind = InterventionKind::Procedure(EMPTY_RECYCLE_BIN.to_owned());
    target.method = EMPTY_RECYCLE_BIN.to_owned();
    // Anche il Cestino dell'utente, se il record non lo comprende già.
    if let Some(sid) = &app.env.user_sid
        && !target.full_scan
    {
        let mut dirs = target.dirs.clone();
        dirs.push(intervention_jobs::user_bin_dir(&target.volume, sid));
        target.dirs = intervention_jobs::rescan_dirs(&[], &dirs);
    }
    if app.procedures.is_busy(&target.subject) {
        return;
    }
    let probe = Arc::clone(&app.env.actions.probe);
    let empty = Arc::clone(&app.env.actions.empty_bin);
    let owner = app.env.owner;
    let root = target.volume.clone();
    let task = Task::spawn(move || {
        let start = measure::measure(&*probe, &root, MeasurePoint::Start);
        let shell_error = empty(&root, owner).err().map(|e| e.0);
        let measure = measure::measure(&*probe, &root, MeasurePoint::AfterEmpty);
        AfterOut { start: Some(start), measure, shell_error }
    });
    app.procedures.push(Job { target, step: Step::After(task) });
}

/// Misura di partenza della disinstallazione del programma `i`, presa subito, prima di aprire l'interfaccia
/// ufficiale (vista Programmi).
pub fn uninstall_start(app: &App, i: usize) -> Result<Measure, u32> {
    let root = app.data.as_ref().and_then(|d| program_target(d, i)).map(|t| t.volume).or_else(|| app.data.as_ref().map(|d| d.volume.root.clone()));
    measure::measure(&*app.env.actions.probe, &root.unwrap_or_default(), MeasurePoint::Start)
}

/// L'interfaccia di disinstallazione del programma `i` si è aperta con l'URI `method`: la procedura resta
/// in attesa di "Ho eseguito la procedura".
pub fn uninstall_opened(app: &mut App, i: usize, start: Result<Measure, u32>, method: String) {
    let Some(mut target) = app.data.as_ref().and_then(|d| program_target(d, i)) else { return };
    target.method = method;
    let mut sh = lock(&app.procedures.shared);
    sh.reports.remove(&target.subject);
    sh.opened.insert(target.subject.clone(), Opened { target, start, after: Vec::new(), page: None, shell_error: None });
}

// ---------------------------------------------------------------------------------------------
// Lavori in background

/// Raccoglie i lavori in background, a ogni frame, e pubblica lo stato per i pulsanti sotto il record.
pub fn poll(app: &mut App, ctx: &egui::Context) {
    let st = &mut app.procedures;
    st.ctx = Some(ctx.clone());
    ctx.data_mut(|d| d.insert_temp(board_id(), Arc::clone(&st.shared)));
    st.history.refresh(app.env.data_dir.as_deref());
    reopen_dialog(app);
    read_bin(app);
    advance(app);
    if app.procedures.busy() {
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

/// Dopo la riscansione il modello cambia e la vista Programmi chiude il dialogo: al frame dopo si riapre
/// sullo stesso programma, con l'esito.
fn reopen_dialog(app: &mut App) {
    let Some(name) = app.procedures.reopen.take() else { return };
    if app.screen != Screen::Programs || app.programs.dialog.is_some() {
        return;
    }
    let Some(i) = app.data.as_ref().and_then(|d| d.programs.items.iter().position(|x| x.name == name)) else { return };
    app.programs.selected = Some(i);
    app.programs.dialog = Some(UninstallDialog { program: i, copied: true, outcome: Some(Outcome::Opened) });
}

/// Legge il Cestino del volume secondo Windows quando il record del Cestino è visibile (al più ogni
/// [`BIN_REFRESH`]).
fn read_bin(app: &mut App) {
    let st = &mut app.procedures;
    if let Some(t) = st.bin_task.as_mut()
        && let Some((volume, read)) = t.poll()
    {
        st.bin_task = None;
        let mut sh = lock(&st.shared);
        sh.bin.volume = volume;
        sh.bin.read = Some(read);
        sh.bin.at = Some(Instant::now());
    }
    let Some(root) = app.data.as_ref().map(|d| d.volume.root.clone()) else { return };
    let mut sh = lock(&st.shared);
    if sh.bin.volume != root {
        // Il conteggio di un altro volume non vale: niente testo né pulsante finché non arriva quello giusto.
        sh.bin.read = None;
    }
    let wanted = std::mem::take(&mut sh.bin.wanted);
    let stale = sh.bin.volume != root || sh.bin.at.is_none_or(|t| t.elapsed() > BIN_REFRESH);
    drop(sh);
    if wanted && stale && st.bin_task.is_none() {
        let probe = Arc::clone(&app.env.actions.probe);
        st.bin_task = Some(Task::spawn(move || {
            let read = probe.volume_bin(&root).map(|v| v.content);
            (root, read)
        }));
    }
}

fn advance(app: &mut App) {
    let jobs = std::mem::take(&mut app.procedures.jobs);
    let mut keep = Vec::new();
    for Job { target, step } in jobs {
        match step {
            Step::Start(mut t) => match t.poll() {
                Some(out) => started(app, target, out),
                None => keep.push(Job { target, step: Step::Start(t) }),
            },
            Step::After(mut t) => match t.poll() {
                Some(after) if !target.dirs.is_empty() => {
                    let task = intervention_jobs::spawn_rescan_with(&app.env.actions.rescan, target.dirs.clone());
                    keep.push(Job { target, step: Step::Rescan(after, task) });
                }
                Some(after) => finish(app, target, after, None),
                None => keep.push(Job { target, step: Step::After(t) }),
            },
            Step::Rescan(after, mut t) => match t.poll() {
                Some(r) => finish(app, target, after, Some(r)),
                None => keep.push(Job { target, step: Step::Rescan(after, t) }),
            },
        }
    }
    // `started` e `finish` non aggiungono lavori: quelli nuovi arrivano dai pulsanti, dopo `poll`.
    app.procedures.jobs = keep;
    app.procedures.update_busy();
}

fn hex(code: i32) -> String {
    format!("0x{:08X}", code as u32)
}

fn started(app: &mut App, target: Target, out: StartOut) {
    if let Some(code) = out.shell_error {
        app.notice.set(app.lingue.ta("avviso-shell-errore", &[("codice", hex(code).into())]));
    }
    let mut sh = lock(&app.procedures.shared);
    sh.reports.remove(&target.subject);
    sh.opened.insert(target.subject.clone(), Opened { target, start: out.start, after: Vec::new(), page: out.page, shell_error: out.shell_error });
}

/// Il nodo con questo percorso o, se non c'è più, il primo antenato che c'è.
fn nearest(t: &Tree, path: &str) -> u32 {
    let mut p = path.trim_end_matches('\\');
    loop {
        if let Some(n) = intervention_jobs::find_node(t, p) {
            return n;
        }
        match p.rfind('\\') {
            Some(k) if k > 2 => p = &p[..k],
            _ => return Tree::ROOT,
        }
    }
}

/// Applica le riscansioni al modello (F-VE-4) e riallinea le viste che tengono indici di nodi.
fn apply(app: &mut App, rescans: Vec<Rescanned>) -> data::Applied {
    app.wait_background();
    let sid = app.env.user_sid.clone();
    let Some(d) = app.data.as_mut() else { return data::Applied::default() };
    let selected = app.explore.selected.filter(|&n| (n as usize) < d.tree().len()).map(|n| d.tree().path_lossy(n));
    let applied = data::apply_rescans(d, rescans, sid.as_deref());
    let node = selected.map_or(Tree::ROOT, |p| nearest(d.tree(), &p));
    app.explore.after_model_change(d, node);
    app.search.invalidate();
    app.big.invalidate();
    applied
}

/// Fine di una misura: riscansione applicata, ancora presenti, confronto, registro, esito.
fn finish(app: &mut App, target: Target, after: AfterOut, rescans: Option<Vec<Rescanned>>) {
    let mut report = Report { full_scan: target.full_scan, shell_error: after.shell_error, ..Report::default() };
    if let Some(r) = rescans {
        report.rescan_failed = apply(app, r).failed;
        report.still_present = app.data.as_ref().map(|d| intervention_jobs::still_present(d.tree(), &target.paths));
        if let Subject::Program(name) = &target.subject {
            app.procedures.reopen = Some(name.clone());
        }
    }
    lock(&app.procedures.shared).bin.at = None;
    let measure = match after.measure {
        Ok(mut m) => {
            m.still_present = report.still_present;
            m
        }
        Err(code) => {
            report.measure_error = Some(code);
            lock(&app.procedures.shared).reports.insert(target.subject, report);
            return;
        }
    };
    (report.at, report.free) = (measure.at, measure.free);
    let measures: Vec<Measure> = {
        let mut sh = lock(&app.procedures.shared);
        match after.start {
            // Svuotamento: un intervento a sé, con la sua misura di partenza.
            Some(start) => {
                report.start_error = start.err();
                start.ok().into_iter().chain([measure]).collect()
            }
            None => match sh.opened.get_mut(&target.subject) {
                Some(o) if o.start.is_ok() => {
                    o.after.push(measure);
                    o.start.ok().into_iter().chain(o.after.iter().copied()).collect()
                }
                // Senza misura di partenza si registra la sola misura.
                _ => vec![measure],
            },
        }
    };
    let comparison = if measures.len() >= 2 { verification::compare(&target.estimate, &measures, &ExplainFacts::default()) } else { None };
    report.comparison = comparison.clone();
    let entry = JournalEntry {
        id: measures[0].at,
        volume: target.volume.clone(),
        kind: target.kind.clone(),
        origin: target.origin.clone(),
        method: target.method.clone(),
        estimate: target.estimate,
        items: Vec::new(),
        measures,
        comparison,
    };
    report.journal = app.env.data_dir.as_deref().map(|dir| journal::append(dir, &entry));
    app.procedures.history.invalidate();
    lock(&app.procedures.shared).reports.insert(target.subject, report);
}

// ---------------------------------------------------------------------------------------------
// Testi

/// Tono di una riga: testo, nota secondaria, avviso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Text,
    Note,
    Warn,
}

/// Quello che i pulsanti mostrano per un soggetto, copiato dallo stato condiviso.
#[derive(Clone, Debug, Default)]
struct View {
    busy: bool,
    opened: Option<Opened>,
    report: Option<Report>,
    bin: Option<Result<Option<BinCount>, u32>>,
}

fn view_of(sh: &Shared, s: &Subject) -> View {
    View { busy: sh.busy.contains(s), opened: sh.opened.get(s).cloned(), report: sh.reports.get(s).cloned(), bin: sh.bin.read }
}

/// Testo della pagina che si apre (04 §6.7): il nome nel pulsante.
fn page_key(page: &Launch) -> &'static str {
    match page {
        Launch::RecycleBin => "procedura-apri-cestino",
        Launch::StorageSense => "procedura-apri-archiviazione",
        Launch::TempFiles => "procedura-apri-file-temporanei",
        _ => "azione-apri-procedura",
    }
}

/// Stato della procedura: misura in corso, misura di partenza (o perché manca), poi l'esito.
fn status_lines(l: &Lingue, f: &Formatter, v: &View, command: bool) -> Vec<(String, Tone)> {
    let mut out = Vec::new();
    if v.busy {
        out.push((l.t("procedura-misura-in-corso"), Tone::Note));
    }
    match &v.opened {
        // Con l'esito la misura di partenza è già nel confronto: niente invito ripetuto.
        Some(o) if v.report.is_some() && o.start.is_ok() && o.shell_error.is_none() => {}
        Some(o) => {
            match o.start {
                Ok(m) => {
                    let key = if o.page.is_some() {
                        "procedura-aperta"
                    } else if matches!(o.target.kind, InterventionKind::Command(_)) {
                        "procedura-copiato"
                    } else {
                        "procedura-partenza"
                    };
                    let args = [("ora", f.time(m.at).into()), ("libero", f.size(m.free).into()), ("volume", o.target.volume.clone().into())];
                    out.push((l.ta(key, &args), Tone::Note));
                }
                Err(code) => out.push((l.ta("procedura-partenza-errore", &[("codice", code.to_string().into())]), Tone::Warn)),
            }
            if let Some(code) = o.shell_error {
                out.push((l.ta("procedura-pagina-errore", &[("codice", hex(code).into())]), Tone::Warn));
            }
        }
        None if command => out.push((l.t("procedura-comando-nota"), Tone::Note)),
        None => out.push((l.t("procedura-senza-partenza"), Tone::Note)),
    }
    if let Some(r) = &v.report {
        out.extend(report_lines(l, f, r));
    }
    out
}

/// Esito di una misura (04 §6.8, "Verifica"): stimato e variazione del volume, differenza e spiegazione,
/// quanto occupano ancora gli elementi, registro.
pub fn report_lines(l: &Lingue, f: &Formatter, r: &Report) -> Vec<(String, Tone)> {
    let mut out = Vec::new();
    if let Some(code) = r.measure_error {
        out.push((l.ta("procedura-misura-errore", &[("codice", code.to_string().into())]), Tone::Warn));
        return out;
    }
    if let Some(code) = r.start_error {
        out.push((l.ta("procedura-partenza-errore", &[("codice", code.to_string().into())]), Tone::Warn));
    }
    match &r.comparison {
        Some(c) => out.extend(interventions_view::comparison_lines(l, f, c)),
        None => out.push((l.ta("procedura-solo-misura", &[("ora", f.time(r.at).into()), ("libero", f.size(r.free).into())]), Tone::Text)),
    }
    if let Some(b) = r.still_present {
        out.push((l.ta("storico-ancora-presenti", &[("dimensione", f.size(b).into())]), Tone::Text));
    }
    if r.full_scan {
        out.push((l.t("procedura-serve-riesamina"), Tone::Note));
    }
    if let Some((_, code)) = r.rescan_failed.first() {
        let n = r.rescan_failed.len() as u64;
        out.push((l.ta("procedura-riscansione-errore", &[("n", n.into()), ("testo", f.count(n).into()), ("codice", code.to_string().into())]), Tone::Warn));
    }
    if let Some(code) = r.shell_error {
        out.push((l.ta("procedura-svuota-codice", &[("codice", hex(code).into())]), Tone::Note));
    }
    match r.journal {
        Some(Ok(())) => out.push((l.t("procedura-registrato"), Tone::Note)),
        Some(Err(JournalError::Io(code))) => out.push((l.ta("procedura-registro-errore", &[("codice", code.to_string().into())]), Tone::Warn)),
        None => {}
    }
    out
}

/// Righe con il loro tono, a capo.
pub fn lines(ui: &mut Ui, p: &Palette, lines: &[(String, Tone)]) {
    for (s, tone) in lines {
        let color = match tone {
            Tone::Text => p.text_2,
            Tone::Note => p.text_3,
            Tone::Warn => p.warn,
        };
        widgets::paragraph(ui, s, crate::tokens::Type::Callout, color);
    }
}

/// Pulsanti e stato di una procedura o di un comando sotto il record (`inspector::action_buttons`): la
/// pagina che si apre, "Svuota il Cestino…" preceduto dal conteggio di Windows, "Ho eseguito la
/// procedura", la misura di partenza e l'esito.
pub fn record_controls(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, r: &Record, req: &mut Option<KbRequest>) {
    let subject = Subject::Rule(r.origin_id().to_owned());
    let page = r.preferred.as_ref().and_then(|s| s.uri.as_deref()).and_then(Launch::for_rule_uri);
    let is_bin = page == Some(Launch::RecycleBin);
    let v = match board(ui.ctx()) {
        Some(b) => {
            let mut sh = lock(&b);
            sh.bin.wanted |= is_bin;
            view_of(&sh, &subject)
        }
        None => View::default(),
    };
    ui.add_space(6.0);
    // Cestino: il numero e i GB secondo Windows prima del pulsante (F-AC-1, 04 §6.7).
    let mut can_empty = false;
    if is_bin {
        let (s, tone) = match v.bin {
            None => (l.t("procedura-cestino-lettura"), Tone::Note),
            Some(Ok(Some(c))) if c.items == 0 => (l.t("procedura-cestino-vuoto"), Tone::Note),
            Some(Ok(Some(c))) => {
                can_empty = true;
                (l.ta("procedura-cestino-conteggio", &[("n", c.items.into()), ("testo", f.count(c.items).into()), ("dimensione", f.size(c.bytes).into())]), Tone::Text)
            }
            Some(Ok(None)) => (l.t("procedura-cestino-non-letto"), Tone::Warn),
            Some(Err(code)) => (l.ta("procedura-cestino-errore", &[("codice", code.to_string().into())]), Tone::Warn),
        };
        lines(ui, p, &[(s, tone)]);
        ui.add_space(4.0);
    }
    ui.push_id("procedura_pulsanti", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            let free = !v.busy;
            match (&r.action, &page) {
                (Action::Procedure, Some(pg)) => {
                    if widgets::button(ui, p, &l.t(page_key(pg)), None, ButtonKind::Secondary, free).clicked() {
                        *req = Some(KbRequest::Act(ActRequest::OpenProcedure));
                    }
                }
                (Action::Procedure, None) => {
                    let b = widgets::button(ui, p, &l.t("procedura-misura-partenza"), None, ButtonKind::Secondary, free);
                    if b.clicked() {
                        *req = Some(KbRequest::Act(ActRequest::MeasureStart));
                    }
                    b.on_hover_text(l.t("procedura-misura-partenza-suggerimento"));
                }
                _ => {}
            }
            if is_bin && widgets::button(ui, p, &l.t("procedura-svuota-cestino"), None, ButtonKind::Secondary, free && can_empty).clicked() {
                *req = Some(KbRequest::Act(ActRequest::EmptyBin));
            }
            if widgets::button(ui, p, &l.t("azione-ho-eseguito"), None, ButtonKind::Secondary, free).clicked() {
                *req = Some(KbRequest::Act(ActRequest::ProcedureDone));
            }
        });
    });
    ui.add_space(2.0);
    lines(ui, p, &status_lines(l, f, &v, r.action == Action::Command));
}

/// Stato della disinstallazione del programma `name` per il dialogo della vista Programmi: misura in
/// corso, misura di partenza ed esito.
pub fn program_status(app: &App, name: &str) -> (bool, Vec<(String, Tone)>) {
    let v = view_of(&lock(&app.procedures.shared), &Subject::Program(name.to_owned()));
    (v.busy, status_lines(&app.lingue, &app.fmt, &v, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_roots() {
        assert_eq!(drive_root(r"D:\Giochi\X", r"C:\"), r"D:\");
        assert_eq!(drive_root(r"\\server\share", r"C:\"), r"C:\");
        assert_eq!(drive_root("", r"C:\"), r"C:\");
    }

    #[test]
    fn nearest_existing_ancestor() {
        let (_, r) = super::super::synthetic::sample_c(false);
        let t = &r.tree;
        let npm = intervention_jobs::find_node(t, r"C:\Users\demo\AppData\Local\npm-cache").expect("npm-cache");
        assert_eq!(nearest(t, r"C:\Users\demo\AppData\Local\npm-cache\non-esiste\x"), npm);
        assert_eq!(nearest(t, r"C:\non-esiste"), Tree::ROOT);
        assert_eq!(nearest(t, r"C:\"), Tree::ROOT);
    }
}
