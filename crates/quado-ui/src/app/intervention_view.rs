// SPDX-License-Identifier: GPL-3.0-or-later
//! Spostamento nel Cestino dall'interfaccia: simulazione in background, dialogo di conferma, esecuzione,
//! misure e riscansione, risultato con "Apri il Cestino" e "Svuota il Cestino…", registro.
//!
//! Flusso:
//! 1. [`open`] costruisce la `simulate::Request` dai dati correnti (elementi di una regola o voci della
//!    lente Download, con gli elementi fissi) e avvia la simulazione in un thread. Il dialogo mostra
//!    l'avanzamento; "Annulla" la ferma.
//! 2. Finita la simulazione, il dialogo di conferma. Esc, Annulla, la X e il clic fuori lo chiudono senza
//!    chiamare `plan::conferma`; il pulsante principale è attivo solo se `plan::check` è vuoto e chiama
//!    `plan::conferma`, l'unico costruttore del piano.
//! 3. In un thread: misura di partenza prima del mover, mover creato lì (COM a thread singolo),
//!    `execute::esegui`, misura dopo l'azione. Se la misura di partenza non riesce non si sposta niente:
//!    senza partenza non c'è verifica né registro.
//! 4. Riscansione delle cartelle che contenevano gli elementi e del Cestino dell'utente, modello
//!    aggiornato (`data::apply_rescans`), stimato contro ottenuto (`verification::compare`),
//!    registro (`journal::append`, una riga per misura con lo stesso `id`: vale l'ultima).
//! 5. Risultato in cima alla vista Interventi, con "Apri il Cestino", "Svuota il Cestino…"
//!    (dialogo di Windows, mai senza conferma) e la verifica dopo lo svuotamento.
//!
//! Il thread dell'interfaccia non aspetta mai un lavoro, salvo `App::wait_background` prima di
//! modificare il modello. Ogni testo viene dai file di lingua, ogni unità da `app.fmt`.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use egui::{Align, Align2, Color32, CornerRadius, Frame, Id, Layout, Margin, Response, RichText, ScrollArea, Sense, Stroke, Ui, vec2};
use quado_actions::execute::{self, ExecProgress, Riepilogo};
use quado_actions::journal::{self, JournalError};
use quado_actions::launch::Launch;
use quado_actions::measure;
use quado_actions::plan::{self, Block, CapacityCheck, ManualCheck, PianoConfermato, Rifiuto, Scelta, Simulazione};
use quado_actions::simulate::{self, Cancelled, FixedReason, Request, RequestItem, SimProgress};
use quado_actions::sync::SyncKind;
use quado_kb::Match;
use quado_kb::lens::Finding;
use quado_kb::rules::Recognizer;
use quado_model::intervention::{
    BinCount, Comparison, Explanation, InterventionKind, ItemOutcome, JournalEntry, JournalItem, Measure, MeasurePoint, ProtectedKind,
    ReferenceKind, SkipReason,
};
use quado_model::record::{CheckKind, Estimate, Origin as RecordOrigin, State, Value};
use quado_model::verification::{self, ExplainFacts};
use quado_model::{Flags, NONE, Tree};

use super::data::{self, Task};
use super::inspector::{self, KbRequest};
use super::intervention_jobs::{self, Actions, Origin, Rescanned};
use super::{App, Screen, kbtext};
use crate::format::Formatter;
use crate::i18n::Lingue;
use crate::icons::{self, Icon};
use crate::tokens::{self, Elevation, Palette, Type};
use crate::widgets::{self, ButtonKind, medium, mono, sans};

/// Controlli automatici che la simulazione esegue da sé: nel dialogo hanno una riga con il loro esito
/// dettagliato, non quella generica della regola.
const SIMULATION_CHECKS: [&str; 3] = ["controllo-restart-manager", "controllo-riferimenti", "controllo-capacita-cestino"];

// ---------------------------------------------------------------------------------------------
// Stato

/// Cosa si sposta: il nome da mostrare e da dove viene, presi all'apertura.
#[derive(Clone, Debug)]
struct Subject {
    /// Chiave del nome: regola (`regola-<id>-nome`) o funzione della lente (`funzione-<codice>-nome`).
    name_key: String,
    /// Regola della base (indice in `Kb::rules`), per la scheda: fonti, versioni e data; per il risultato:
    /// cosa provare prima di svuotare il Cestino (`Rule::try_before_empty`, `Rule::tick_tried`).
    rule: Option<usize>,
}

/// Simulazione in corso.
struct SimJob {
    task: Task<Result<Simulazione, Cancelled>>,
    progress: Arc<SimProgress>,
    subject: Subject,
}

/// Dialogo di conferma aperto: la simulazione e la scelta dell'utente.
struct Dialog {
    sim: Simulazione,
    scelta: Scelta,
    subject: Subject,
    /// Scheda completa della raccomandazione aperta (chiusa all'apertura del dialogo).
    details: bool,
}

/// Esito del thread di esecuzione.
struct Executed {
    start: Result<Measure, u32>,
    /// `None` se la misura di partenza non è riuscita: niente spostato.
    riepilogo: Option<Riepilogo>,
    after: Option<Result<Measure, u32>>,
}

/// Esito di "Svuota il Cestino…": la risposta del dialogo di Windows (HRESULT) e la misura dopo.
struct Emptied {
    result: Result<(), i32>,
    measure: Option<Result<Measure, u32>>,
}

/// Stato del registro degli interventi per l'ultimo intervento.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalState {
    /// Nessuna cartella dei dati (`Env::data_dir`): niente si scrive.
    NoDir,
    /// Riga in coda o in scrittura.
    Pending,
    Written,
    /// Errore di scrittura: codice di Windows.
    Failed(u32),
}

/// Esito dell'ultima riscansione dei sottoalberi toccati.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RescanNote {
    /// Tra le cartelle c'era la radice del volume: nessuna riscansione parziale.
    full_scan: bool,
    /// Cartelle non rilette.
    failed: usize,
}

/// Uno spostamento confermato in questa sessione: esecuzione, misure, confronti, svuotamento.
struct Outcome {
    subject: Subject,
    sim: Simulazione,
    /// Radice del volume (`C:\`).
    volume: String,
    /// Elementi del piano.
    total: usize,
    progress: Arc<ExecProgress>,
    exec: Option<Task<Executed>>,
    start_error: Option<u32>,
    riepilogo: Option<Riepilogo>,
    /// Misure nell'ordine in cui sono state prese; la prima è la partenza.
    measures: Vec<Measure>,
    measure_errors: Vec<(MeasurePoint, u32)>,
    /// Stimato contro ottenuto dopo l'azione e dopo lo svuotamento.
    comparisons: Vec<(MeasurePoint, Comparison)>,
    rescan: Option<(Task<Vec<Rescanned>>, MeasurePoint)>,
    rescan_note: Option<RescanNote>,
    empty: Option<Task<Emptied>>,
    empty_error: Option<i32>,
    /// Spuntata la casella della prova («Ho aperto CapCut e funziona»): attiva «Svuota il Cestino…».
    tried: bool,
}

/// Righe del registro da scrivere, una alla volta e in ordine, fuori dal thread dell'interfaccia.
#[derive(Default)]
struct JournalWriter {
    queue: VecDeque<(PathBuf, JournalEntry)>,
    task: Option<(i64, Task<Result<(), JournalError>>)>,
    /// Stato per `id` dell'ultima riga accodata o scritta.
    state: Option<(i64, JournalState)>,
}

/// Stato del flusso di uno spostamento nel Cestino.
#[derive(Default)]
pub struct InterventionState {
    sim: Option<SimJob>,
    dialog: Option<Dialog>,
    last: Option<Outcome>,
    journal: JournalWriter,
    /// Piani costruiti da `plan::conferma` in questa sessione.
    confirmations: usize,
}

impl InterventionState {
    /// Piani costruiti da `plan::conferma` in questa sessione (per i test: Esc e Annulla non ne costruiscono).
    pub fn confirmations(&self) -> usize {
        self.confirmations
    }

    /// La simulazione gira o il dialogo di conferma è aperto.
    pub fn dialog_open(&self) -> bool {
        self.sim.is_some() || self.dialog.is_some()
    }

    pub fn simulating(&self) -> bool {
        self.sim.is_some()
    }

    /// La simulazione del dialogo aperto.
    pub fn simulation(&self) -> Option<&Simulazione> {
        self.dialog.as_ref().map(|d| &d.sim)
    }

    /// Un lavoro in background è in corso: simulazione, esecuzione, svuotamento, riscansione, registro.
    pub fn busy(&self) -> bool {
        let o = self.last.as_ref();
        self.sim.is_some()
            || o.is_some_and(|o| o.exec.is_some() || o.empty.is_some() || o.rescan.is_some())
            || self.journal.task.is_some()
            || !self.journal.queue.is_empty()
    }

    /// Riepilogo dell'ultimo spostamento, quando l'esecuzione è finita.
    pub fn summary(&self) -> Option<&Riepilogo> {
        self.last.as_ref().and_then(|o| o.riepilogo.as_ref())
    }

    /// Misure dell'ultimo spostamento.
    pub fn measures(&self) -> &[Measure] {
        self.last.as_ref().map_or(&[], |o| o.measures.as_slice())
    }

    /// Stimato contro ottenuto all'ultima misura di questo punto.
    pub fn comparison(&self, point: MeasurePoint) -> Option<&Comparison> {
        self.last.as_ref()?.comparisons.iter().rev().find(|(p, _)| *p == point).map(|(_, c)| c)
    }

    /// Stato del registro per l'ultimo spostamento.
    pub fn journal(&self) -> Option<JournalState> {
        let id = self.last.as_ref()?.id()?;
        self.journal.state.filter(|(i, _)| *i == id).map(|(_, s)| s)
    }
}

impl Outcome {
    /// Identificativo nel registro: ora della misura di partenza.
    fn id(&self) -> Option<i64> {
        self.measures.first().filter(|m| m.point == MeasurePoint::Start).map(|m| m.at)
    }

    fn running(&self) -> bool {
        self.exec.is_some() || self.empty.is_some() || self.rescan.is_some()
    }

    /// Applica l'esito del thread di esecuzione; vero se c'è da riscansionare.
    fn executed(&mut self, e: Executed) -> bool {
        match e.start {
            Ok(m) => self.measures.push(m),
            Err(code) => {
                self.start_error = Some(code);
                return false;
            }
        }
        self.riepilogo = e.riepilogo;
        match e.after {
            Some(Ok(m)) => self.measures.push(m),
            Some(Err(code)) => self.measure_errors.push((MeasurePoint::AfterAction, code)),
            None => {}
        }
        self.riepilogo.is_some()
    }

    /// Applica l'esito di "Svuota il Cestino…"; vero se c'è da riscansionare.
    fn emptied(&mut self, e: Emptied) -> bool {
        match e.result {
            Err(hr) => {
                self.empty_error = Some(hr);
                false
            }
            Ok(()) => {
                self.empty_error = None;
                match e.measure {
                    Some(Ok(m)) => self.measures.push(m),
                    Some(Err(code)) => self.measure_errors.push((MeasurePoint::AfterEmpty, code)),
                    None => {}
                }
                true
            }
        }
    }

    fn measure(&self, point: MeasurePoint) -> Option<&Measure> {
        self.measures.iter().rev().find(|m| m.point == point)
    }

    /// Percorsi degli elementi che dovrebbero essere spariti dal loro posto: nel Cestino, o spariti senza
    /// arrivarci. La riscansione dice quanto occupano ancora (`Measure::still_present`).
    fn gone_paths(&self) -> Vec<String> {
        self.riepilogo.as_ref().map_or_else(Vec::new, |r| {
            r.items.iter().filter(|i| matches!(i.outcome, ItemOutcome::Recycled | ItemOutcome::NotInRecycleBin(_))).map(|i| i.path.clone()).collect()
        })
    }

    /// Fatti misurati per le voci di spiegazione, all'ultima misura di `point`.
    fn facts(&self, point: MeasurePoint) -> ExplainFacts {
        let Some(r) = &self.riepilogo else { return ExplainFacts::default() };
        let (mut sf_items, mut sf_bytes, mut recycled) = (0u64, 0u64, 0u64);
        for i in &r.items {
            match i.outcome {
                ItemOutcome::Recycled => recycled += i.alloc,
                ItemOutcome::Skipped(_) | ItemOutcome::Failed(_) | ItemOutcome::NotReached => {
                    sf_items += 1;
                    sf_bytes += i.alloc;
                }
                // Sparito senza arrivare nel Cestino: il suo spazio può essere stato liberato.
                ItemOutcome::NotInRecycleBin(_) => {}
            }
        }
        let still_in_bin = match point {
            // Appena spostati: `PostDeleteItem` ha dato l'elemento del Cestino per ciascuno (riepilogo).
            MeasurePoint::AfterAction => Some(recycled),
            _ => still_in_bin(self.measure(MeasurePoint::AfterAction).and_then(|m| m.bin), self.measures.last().and_then(|m| m.bin), recycled),
        };
        ExplainFacts { skipped_or_failed: (sf_items, sf_bytes), excluded_names: r.estimate.excluded_names, recycled_bytes: recycled, still_in_bin, still_open_files: 0 }
    }

    /// Stimato contro ottenuto, se l'ultima misura è di `point`.
    fn compare(&self, point: MeasurePoint) -> Option<Comparison> {
        let r = self.riepilogo.as_ref()?;
        if self.measures.last()?.point != point {
            return None;
        }
        verification::compare(&r.estimate.estimate, &self.measures, &self.facts(point))
    }

    /// Riga del registro con tutte le misure prese finora e il confronto all'ultima.
    fn journal_entry(&self) -> Option<JournalEntry> {
        let r = self.riepilogo.as_ref()?;
        let id = self.id()?;
        let last = self.measures.last()?.point;
        Some(JournalEntry {
            id,
            volume: r.volume.clone(),
            kind: InterventionKind::Recycle,
            origin: r.origin.clone(),
            method: self.sim.record().action.code().to_owned(),
            estimate: r.estimate.estimate,
            items: r.items.iter().map(|i| JournalItem { path: i.path.clone(), bytes: i.alloc, outcome: i.outcome }).collect(),
            measures: self.measures.clone(),
            comparison: self.comparisons.iter().rev().find(|(p, _)| *p == last).map(|(_, c)| c.clone()),
        })
    }
}

/// Byte di questo intervento ancora nel Cestino dopo lo svuotamento, dal conteggio di Windows: 0 se il
/// Cestino è vuoto; tutti se il Cestino non è sceso sotto il conteggio dopo l'azione (dialogo annullato);
/// altrimenti non si sa.
fn still_in_bin(after_action: Option<BinCount>, now: Option<BinCount>, recycled: u64) -> Option<u64> {
    let now = now?;
    if now.items == 0 {
        return Some(0);
    }
    let a = after_action?;
    (now.items >= a.items && now.bytes >= a.bytes).then_some(recycled)
}

// ---------------------------------------------------------------------------------------------
// Apertura e simulazione

/// Avvia la simulazione per `origin` e apre il dialogo di conferma.
pub fn open(app: &mut App, origin: Origin) {
    let st = &app.intervention;
    if st.sim.is_some() || st.dialog.is_some() {
        return;
    }
    if st.last.as_ref().is_some_and(Outcome::running) {
        app.notice.set(app.lingue.t("spostamento-occupato"));
        return;
    }
    let Some((req, subject)) = request(app, &origin) else { return };
    let probe = Arc::clone(&app.env.actions.probe);
    let progress = Arc::new(SimProgress::default());
    let p = Arc::clone(&progress);
    let task = Task::spawn(move || simulate::simulate(req, &*probe, &p));
    app.intervention.sim = Some(SimJob { task, progress, subject });
}

/// La richiesta di simulazione dai dati correnti, con il soggetto per i testi. `None` per i programmi
/// (non sono uno spostamento) e se i dati non ci sono più.
fn request(app: &App, origin: &Origin) -> Option<(Request, Subject)> {
    let d = app.data.as_ref()?;
    let t = d.tree();
    match origin {
        Origin::Rule(k) => {
            let m = d.cls.matches.get(*k)?;
            let rule = d.kb.rules.get(m.rule)?;
            let capcut = rule.recognizer == Recognizer::CapCutVersions;
            let mut items: Vec<RequestItem> = m.nodes.iter().map(|&n| RequestItem { path: t.path_lossy(n), fixed: None }).collect();
            if capcut {
                items.extend(capcut_kept(t, m));
            }
            let req = Request { items, record: m.record.clone(), protected: intervention_jobs::protected_roots(d) };
            Some((req, Subject { name_key: rule.key("nome"), rule: Some(m.rule) }))
        }
        Origin::Lens(voci) => {
            let v = app.downloads.current(d)?;
            let first = v.findings.get(*voci.first()?)?;
            let mut items = Vec::new();
            for x in voci.iter().filter_map(|&k| v.findings.get(k)) {
                let keep = match x.finding {
                    Finding::IdenticalCopies { keep } => Some(keep),
                    _ => None,
                };
                items.extend(x.nodes.iter().map(|&n| RequestItem { path: t.path_lossy(n), fixed: (keep == Some(n)).then_some(FixedReason::KeptCopy) }));
            }
            let name_key = match first.record.origin {
                RecordOrigin::Function(f) => format!("funzione-{}-nome", f.code()),
                _ => String::new(),
            };
            let rule = first.record.rule().and_then(|r| d.kb.index(&r.id));
            let req = Request { items, record: first.record.clone(), protected: intervention_jobs::protected_roots(d) };
            Some((req, Subject { name_key, rule }))
        }
        Origin::Program(_) => None,
    }
}

/// Versione precedente e versione in uso di CapCut, fisse nella conferma: si ritrovano per nome
/// (argomenti "precedente" e "versione" della corrispondenza) nella stessa cartella `Apps` delle versioni
/// da spostare.
fn capcut_kept(t: &Tree, m: &Match) -> Vec<RequestItem> {
    let Some(&first) = m.nodes.first() else { return Vec::new() };
    let apps = t.parent(first);
    if apps == NONE {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (arg, reason) in [("precedente", FixedReason::KeptVersion), ("versione", FixedReason::InUseVersion)] {
        let Some(Value::Text(name)) = m.arg(arg) else { continue };
        if let Some(c) = t.children(apps).find(|&c| !t.flags(c).has(Flags::DETACHED) && t.name_lossy(c).eq_ignore_ascii_case(name)) {
            out.push(RequestItem { path: t.path_lossy(c), fixed: Some(reason) });
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Lavori in background

/// Raccoglie i lavori in background (simulazione, esecuzione, misure, riscansione, registro), a ogni frame.
pub fn poll(app: &mut App, ctx: &egui::Context) {
    // Simulazione: il dialogo di conferma parte dalla scelta proposta.
    if let Some(job) = app.intervention.sim.as_mut()
        && let Some(r) = job.task.poll()
    {
        let subject = job.subject.clone();
        app.intervention.sim = None;
        if let Ok(sim) = r {
            let scelta = sim.default_choice();
            app.intervention.dialog = Some(Dialog { sim, scelta, subject, details: false });
            // Il focus non resta su un controllo sotto il dialogo: Tab parte dal primo controllo del dialogo e
            // non esce (egui limita il focus allo strato modale).
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
    }
    // Esecuzione e svuotamento: ogni nuova misura porta una riscansione.
    let mut rescan_at = None;
    if let Some(o) = app.intervention.last.as_mut() {
        if let Some(t) = o.exec.as_mut()
            && let Some(done) = t.poll()
        {
            o.exec = None;
            if o.executed(done) {
                rescan_at = Some(MeasurePoint::AfterAction);
            }
        }
        if let Some(t) = o.empty.as_mut()
            && let Some(done) = t.poll()
        {
            o.empty = None;
            if o.emptied(done) {
                rescan_at = Some(MeasurePoint::AfterEmpty);
            }
        }
    }
    if let Some(point) = rescan_at {
        start_rescan(app, point);
    }
    // Riscansione finita: modello, confronto, registro.
    let mut finished = None;
    if let Some(o) = app.intervention.last.as_mut()
        && let Some((t, point)) = o.rescan.as_mut()
        && let Some(v) = t.poll()
    {
        finished = Some((v, *point));
        o.rescan = None;
    }
    if let Some((rescans, point)) = finished {
        finish_rescan(app, rescans, point);
    }
    poll_journal(&mut app.intervention.journal);
    if app.intervention.busy() {
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

/// Riscansione delle cartelle che contenevano gli elementi del piano e del Cestino dell'utente.
fn start_rescan(app: &mut App, point: MeasurePoint) {
    let sid = app.env.user_sid.clone();
    let rescan = Arc::clone(&app.env.actions.rescan);
    let Some(o) = app.intervention.last.as_mut() else { return };
    let items: Vec<String> = o.riepilogo.as_ref().map_or_else(Vec::new, |r| r.items.iter().map(|i| i.path.clone()).collect());
    let extra: Vec<String> = sid.iter().map(|s| intervention_jobs::user_bin_dir(&o.volume, s)).collect();
    let dirs = intervention_jobs::rescan_dirs(&items, &extra);
    o.rescan = Some((intervention_jobs::spawn_rescan_with(&rescan, dirs), point));
    o.rescan_note = None;
}

/// Applica la riscansione al modello, completa la misura con `still_present`, confronta stimato
/// e ottenuto e accoda la riga del registro.
fn finish_rescan(app: &mut App, rescans: Vec<Rescanned>, point: MeasurePoint) {
    let gone = app.intervention.last.as_ref().map_or_else(Vec::new, Outcome::gone_paths);
    let mut note = RescanNote::default();
    let mut present = None;
    if app.data.is_some() {
        // Nessun lavoro in background tiene il modello mentre lo si modifica.
        app.wait_background();
        let sid = app.env.user_sid.clone();
        if let Some(d) = app.data.as_mut() {
            let applied = data::apply_rescans(d, rescans, sid.as_deref());
            note = RescanNote { full_scan: applied.needs_full_scan, failed: applied.failed.len() };
            if !applied.needs_full_scan && applied.failed.is_empty() {
                present = Some(intervention_jobs::still_present(d.tree(), &gone));
            }
            // Le viste seguono il modello: la selezione resta se il nodo c'è ancora; se è stato sostituito,
            // passa alla cartella riletta.
            let t = d.tree();
            let alive = |n: u32| (n as usize) < t.len() && !t.flags(n).has(Flags::DETACHED);
            let root = app.explore.outline.root;
            let sel = match app.explore.selected {
                Some(n) if alive(n) => n,
                Some(_) => applied.nodes.first().copied().unwrap_or(Tree::ROOT),
                None if alive(root) => root,
                None => Tree::ROOT,
            };
            app.explore.after_model_change(d, sel);
            app.search.invalidate();
            app.big.invalidate();
        }
    }
    let data_dir = app.env.data_dir.clone();
    let st = &mut app.intervention;
    let Some(o) = st.last.as_mut() else { return };
    o.rescan_note = Some(note);
    if let Some(m) = o.measures.last_mut()
        && m.point == point
    {
        m.still_present = present;
    }
    if let Some(c) = o.compare(point) {
        o.comparisons.retain(|(p, _)| *p != point);
        o.comparisons.push((point, c));
    }
    let Some(entry) = o.journal_entry() else { return };
    match data_dir {
        Some(dir) => {
            st.journal.state = Some((entry.id, JournalState::Pending));
            st.journal.queue.push_back((dir, entry));
        }
        None => st.journal.state = Some((entry.id, JournalState::NoDir)),
    }
}

/// Scrive le righe del registro una alla volta, nell'ordine in cui sono state accodate.
fn poll_journal(w: &mut JournalWriter) {
    if let Some((id, t)) = w.task.as_mut()
        && let Some(r) = t.poll()
    {
        let id = *id;
        w.task = None;
        let state = match r {
            Ok(()) => JournalState::Written,
            Err(JournalError::Io(code)) => JournalState::Failed(code),
        };
        // Una riga successiva già in coda per lo stesso intervento resta "in scrittura".
        if !w.queue.iter().any(|(_, e)| e.id == id) {
            w.state = Some((id, state));
        }
    }
    if w.task.is_none()
        && let Some((dir, entry)) = w.queue.pop_front()
    {
        let id = entry.id;
        w.task = Some((id, Task::spawn(move || journal::append(&dir, &entry))));
    }
}

/// Conferma: `plan::conferma` (l'unico costruttore del piano) e l'esecuzione in un thread.
fn start(app: &mut App, dlg: Dialog) {
    let piano = match plan::conferma(&dlg.sim, &dlg.scelta) {
        Ok(p) => p,
        Err(_) => {
            // Non succede: il pulsante è attivo solo con `plan::check` vuoto. Il dialogo resta.
            app.intervention.dialog = Some(dlg);
            return;
        }
    };
    app.intervention.confirmations += 1;
    let total = piano.items().len();
    let volume = piano.volume().to_owned();
    let progress = Arc::new(ExecProgress::default());
    let (actions, owner, pr, vol) = (app.env.actions.clone(), app.env.owner, Arc::clone(&progress), volume.clone());
    let exec = Task::spawn(move || run(piano, &actions, owner, &vol, &pr));
    app.intervention.last = Some(Outcome {
        subject: dlg.subject,
        sim: dlg.sim,
        volume,
        total,
        progress,
        exec: Some(exec),
        start_error: None,
        riepilogo: None,
        measures: Vec::new(),
        measure_errors: Vec::new(),
        comparisons: Vec::new(),
        rescan: None,
        rescan_note: None,
        empty: None,
        empty_error: None,
        tried: false,
    });
    app.go(Screen::Interventions);
}

/// L'esecuzione, in un thread: misura di partenza prima del mover, mover creato qui, piano eseguito e
/// consumato, misura dopo l'azione. Senza misura di partenza il piano si scarta.
fn run(piano: PianoConfermato, actions: &Actions, owner: Option<isize>, volume: &str, progress: &ExecProgress) -> Executed {
    let probe = &*actions.probe;
    let start = measure::measure(probe, volume, MeasurePoint::Start);
    if start.is_err() {
        return Executed { start, riepilogo: None, after: None };
    }
    let mover = (actions.mover)(owner);
    let riepilogo = execute::esegui(piano, probe, &*mover, progress);
    drop(mover);
    let after = measure::measure(probe, volume, MeasurePoint::AfterAction);
    Executed { start, riepilogo: Some(riepilogo), after: Some(after) }
}

/// "Svuota il Cestino…": il dialogo di Windows in un thread, poi la misura dopo lo svuotamento.
fn start_empty(app: &mut App) {
    let (actions, owner) = (app.env.actions.clone(), app.env.owner);
    let Some(o) = app.intervention.last.as_mut() else { return };
    if o.running() {
        return;
    }
    let volume = o.volume.clone();
    o.empty_error = None;
    o.empty = Some(Task::spawn(move || {
        let result = (actions.empty_bin)(&volume, owner).map_err(|e| e.0);
        let measure = result.is_ok().then(|| measure::measure(&*actions.probe, &volume, MeasurePoint::AfterEmpty));
        Emptied { result, measure }
    }));
}

// ---------------------------------------------------------------------------------------------
// Testi

/// Nome della regola o della funzione.
fn subject_name(l: &Lingue, s: &Subject) -> String {
    if !s.name_key.is_empty() && l.has(&s.name_key) { l.t(&s.name_key) } else { l.t("spostamento-elementi-scelti") }
}

/// Ultimo componente di un percorso.
fn file_name(path: &str) -> String {
    path.trim_end_matches('\\').rsplit('\\').next().unwrap_or(path).to_owned()
}

/// Valore con il segno.
fn signed(l: &Lingue, f: &Formatter, v: i64) -> String {
    let a = f.size(v.unsigned_abs());
    match v.cmp(&0) {
        std::cmp::Ordering::Greater => l.ta("spostamento-piu", &[("valore", a.into())]),
        std::cmp::Ordering::Less => l.ta("spostamento-meno", &[("valore", a.into())]),
        std::cmp::Ordering::Equal => a,
    }
}

/// Punti base come percentuale: due decimali sotto il 10%, uno sopra.
fn percent(f: &Formatter, bp: u64) -> String {
    format!("{}%", f.decimal(bp as f64 / 100.0, if bp < 1000 { 2 } else { 1 }))
}

/// Codice HRESULT come lo scrive Windows.
fn hresult(hr: i32) -> String {
    format!("0x{:08X}", hr as u32)
}

fn fixed_text(l: &Lingue, r: FixedReason) -> String {
    l.t(match r {
        FixedReason::InUseVersion => "spostamento-fisso-versione-in-uso",
        FixedReason::KeptVersion => "spostamento-fisso-versione-tenuta",
        FixedReason::KeptCopy => "spostamento-fisso-copia",
    })
}

fn reference_kind(l: &Lingue, k: ReferenceKind) -> String {
    l.t(match k {
        ReferenceKind::UninstallString => "spostamento-rif-uninstall-string",
        ReferenceKind::DisplayIcon => "spostamento-rif-display-icon",
        ReferenceKind::InstallLocation => "spostamento-rif-install-location",
        ReferenceKind::AppPaths => "spostamento-rif-app-paths",
        ReferenceKind::RunUser => "spostamento-rif-run-utente",
        ReferenceKind::RunMachine => "spostamento-rif-run-sistema",
        ReferenceKind::Shortcut => "spostamento-rif-collegamento",
        ReferenceKind::ScheduledTask => "spostamento-rif-attivita",
    })
}

/// Il motivo di un blocco, una variante per riga.
fn block_text(l: &Lingue, f: &Formatter, b: &Block) -> String {
    match b {
        Block::Protected(k) => l.t(match k {
            ProtectedKind::WindowsDir => "spostamento-blocco-protetto-windows",
            ProtectedKind::WinSxS => "spostamento-blocco-protetto-winsxs",
            ProtectedKind::WindowsInstaller => "spostamento-blocco-protetto-installer",
            ProtectedKind::SystemVolumeInformation => "spostamento-blocco-protetto-svi",
            ProtectedKind::PagingFile => "spostamento-blocco-protetto-paging",
            ProtectedKind::Hibernation => "spostamento-blocco-protetto-ibernazione",
            ProtectedKind::SwapFile => "spostamento-blocco-protetto-swap",
            ProtectedKind::RegisteredProgram => "spostamento-blocco-protetto-programma",
        }),
        Block::NoRecycleBin => l.t("spostamento-blocco-senza-cestino"),
        Block::NukeOnDelete => l.t("spostamento-blocco-elimina-subito"),
        Block::CapacityUnknown => l.t("spostamento-blocco-capacita-ignota"),
        Block::LargerThanBin { logical, capacity } => {
            l.ta("spostamento-blocco-piu-grande", &[("dimensione", f.size(*logical).into()), ("capacita", f.size(*capacity).into())])
        }
        Block::InUse(procs) => {
            let names: Vec<String> = procs.iter().map(|p| l.ta("spostamento-processo", &[("nome", p.app_name.clone().into()), ("pid", p.pid.to_string().into())])).collect();
            l.ta("spostamento-blocco-in-uso", &[("processi", names.join(", ").into())])
        }
        Block::RestartManager(e) => l.ta("spostamento-blocco-restart-manager", &[("codice", e.code().to_string().into())]),
        Block::Referenced(refs) => {
            let list: Vec<String> =
                refs.iter().map(|r| l.ta("spostamento-riferimento", &[("tipo", reference_kind(l, r.kind).into()), ("dove", r.location.clone().into())])).collect();
            l.ta("spostamento-blocco-riferimenti", &[("elenco", list.join("; ").into())])
        }
        Block::LongPath => l.t("spostamento-blocco-percorso-lungo"),
        Block::ContainsLongPath => l.t("spostamento-blocco-contiene-lungo"),
        Block::Unreadable(code) => l.ta("spostamento-blocco-illeggibile", &[("codice", code.to_string().into())]),
        Block::Missing => l.t("spostamento-blocco-mancante"),
        Block::OtherVolume => l.t("spostamento-blocco-altra-unita"),
    }
}

/// Esito di un elemento non spostato.
fn outcome_text(l: &Lingue, o: ItemOutcome) -> String {
    match o {
        ItemOutcome::Recycled => String::new(),
        ItemOutcome::Skipped(SkipReason::Changed) => l.t("spostamento-esito-cambiato"),
        ItemOutcome::Skipped(SkipReason::Missing) => l.t("spostamento-esito-mancante"),
        ItemOutcome::Skipped(SkipReason::InUse) => l.t("spostamento-esito-in-uso"),
        ItemOutcome::Skipped(SkipReason::Unreadable(code)) => l.ta("spostamento-esito-illeggibile", &[("codice", code.to_string().into())]),
        ItemOutcome::Failed(hr) => l.ta("spostamento-esito-fallito", &[("codice", hresult(hr).into())]),
        ItemOutcome::NotInRecycleBin(hr) => l.ta("spostamento-esito-non-nel-cestino", &[("codice", hresult(hr).into())]),
        ItemOutcome::NotReached => l.t("spostamento-esito-non-raggiunto"),
    }
}

/// Perché il pulsante principale è disattivato: il primo motivo, nell'ordine in cui l'utente può agire.
fn missing_text(l: &Lingue, reasons: &[Rifiuto]) -> Option<String> {
    let has = |p: fn(&Rifiuto) -> bool| reasons.iter().any(p);
    let key = if has(|r| matches!(r, Rifiuto::NotARecycle)) {
        "spostamento-manca-azione"
    } else if has(|r| matches!(r, Rifiuto::Empty | Rifiuto::Shape)) {
        "spostamento-manca-scelta"
    } else if has(|r| matches!(r, Rifiuto::Blocked(..) | Rifiuto::Fixed(_))) {
        "spostamento-manca-blocco"
    } else if has(|r| matches!(r, Rifiuto::Capacity(_))) {
        "spostamento-manca-capacita"
    } else if has(|r| matches!(r, Rifiuto::ItemNotTicked(..) | Rifiuto::RuleNotTicked(_))) {
        "spostamento-manca-spunta"
    } else {
        return None;
    };
    Some(l.t(key))
}

/// Valore della stima: esatta, intervallo o nessuno.
fn estimate_value(l: &Lingue, f: &Formatter, e: &Estimate) -> String {
    match e {
        Estimate::Exact(b) => f.size(*b),
        Estimate::Range { min, max } => l.ta("spostamento-stima-intervallo-valore", &[("minimo", f.size(*min).into()), ("massimo", f.size(*max).into())]),
        Estimate::NotEstimable(_) => l.t("spostamento-stima-nessun-valore"),
    }
}

/// Stato della stima sotto il suo valore.
fn estimate_state(l: &Lingue, f: &Formatter, e: &Estimate) -> String {
    match e {
        Estimate::Exact(_) => l.t("spostamento-stima-esatta"),
        Estimate::Range { .. } => l.t("spostamento-stima-intervallo"),
        Estimate::NotEstimable(_) => kbtext::estimate_text(l, f, e),
    }
}

fn explanation_text(l: &Lingue, f: &Formatter, e: &Explanation) -> String {
    let plural = |key: &str, n: u64, bytes: u64| l.ta(key, &[("n", n.into()), ("n_testo", f.count(n).into()), ("byte", f.size(bytes).into())]);
    match *e {
        Explanation::SkippedOrFailed { items, bytes } => plural("spostamento-voce-saltati", items, bytes),
        Explanation::ExcludedNames { files, bytes } => plural("spostamento-voce-altri-nomi", files, bytes),
        Explanation::StillInBin { bytes } => l.ta("spostamento-voce-nel-cestino", &[("byte", f.size(bytes).into())]),
        Explanation::OtherBinItems { items, bytes } => plural("spostamento-voce-altri-elementi", items, bytes),
        Explanation::StillOpen { files } => l.ta("spostamento-voce-file-aperti", &[("n", files.into()), ("n_testo", f.count(files).into())]),
        Explanation::OtherActivity { bytes } => l.ta("spostamento-voce-altri-processi", &[("valore", signed(l, f, bytes).into())]),
    }
}

/// Estremo della stima più vicino all'ottenuto: il riferimento della percentuale.
fn reference(e: &Estimate, obtained: i64) -> Option<u64> {
    match *e {
        Estimate::Exact(v) => Some(v),
        Estimate::Range { min, max } => Some(if obtained < i64::try_from(min.min(max)).unwrap_or(i64::MAX) { min.min(max) } else { min.max(max) }),
        Estimate::NotEstimable(_) => None,
    }
    .filter(|&v| v > 0)
}

// ---------------------------------------------------------------------------------------------
// Componenti

/// Testo a capo nel ruolo della scala tipografica più vicino a `size`: corpo da 13 in su, Callout sotto.
fn wrapped(ui: &mut Ui, s: &str, size: f32, color: Color32) -> Response {
    widgets::paragraph(ui, s, if size >= 13.0 { Type::Body } else { Type::Callout }, color)
}

/// Fila di cifre (`.dlg .nums`) dentro la card del risultato: prima e dopo, stimato e variazione.
fn figures(ui: &mut Ui, p: &Palette, items: &[(String, String, Option<String>)]) {
    let items: Vec<widgets::Figure> = items.iter().map(|(label, value, sub)| (value.clone(), label.clone(), sub.clone())).collect();
    ui.add_space(4.0);
    widgets::figures(ui, p, p.panel2, 24.0, &items);
    ui.add_space(4.0);
}

/// Riga di un controllo con il suo esito: segno di spunta, avviso o informazione, sempre con il testo.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Ok,
    Fail,
    Info,
}

/// Come [`check_row`] del dialogo, nel ruolo Callout: l'icona dell'esito nel suo colore e il testo.
fn check_line(ui: &mut Ui, p: &Palette, tone: Tone, s: &str) {
    let (icon, icon_color, text) = match tone {
        Tone::Ok => (Icon::StateManageable, p.ok, p.text),
        Tone::Fail => (Icon::Warning, p.warn, p.warn),
        Tone::Info => (Icon::Info, p.text_2, p.text_2),
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(vec2(15.0, 17.0), Sense::hover());
        icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(r.center().x, r.min.y + 8.5), vec2(14.0, 14.0)), icon, icon_color);
        widgets::paragraph(ui, s, Type::Callout, text);
    });
}

// ---------------------------------------------------------------------------------------------
// Dialogo

/// Cosa chiede il dialogo dopo il frame.
enum DialogAction {
    Keep,
    Close,
    Confirm,
}

/// Il dialogo (modale), a ogni frame, sopra la schermata: avanzamento della simulazione, poi la conferma.
pub fn dialog(app: &mut App, ctx: &egui::Context) {
    let p = app.palette(ctx);
    if app.intervention.sim.is_some() {
        progress_dialog(app, ctx, &p);
        return;
    }
    let Some(mut dlg) = app.intervention.dialog.take() else { return };
    let mut req = None;
    match confirm_dialog(app, ctx, &p, &mut dlg, &mut req) {
        DialogAction::Keep => app.intervention.dialog = Some(dlg),
        // Esc, Annulla, la X, il clic fuori: il dialogo si chiude e il piano non si costruisce.
        DialogAction::Close => {}
        DialogAction::Confirm => start(app, dlg),
    }
    if let Some(KbRequest::Copy(c)) = req {
        ctx.copy_text(c);
        app.notice.set(app.lingue.t("avviso-comando-copiato"));
    }
}

/// Cornice dei dialoghi della variante A (`.dlg`): superficie in rilievo, raggio 16, ombra del dialogo.
pub(crate) fn dialog_frame(p: &Palette) -> Frame {
    Frame::new().fill(p.raised).corner_radius(CornerRadius::same(tokens::radius::DIALOG)).shadow(p.shadow(Elevation::Dialog))
}

/// Titolo del dialogo (`.dlg .head`, Title 20/600) con la X a destra; vero se la X è stata premuta.
pub(crate) fn dialog_title(ui: &mut Ui, p: &Palette, l: &Lingue, title: &str) -> bool {
    let mut close = false;
    ui.horizontal_top(|ui| {
        let w = (ui.available_width() - 40.0).max(120.0);
        ui.allocate_ui_with_layout(vec2(w, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.add_space(4.0);
            widgets::paragraph(ui, title, Type::Title, p.text);
        });
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            close = widgets::icon_button(ui, p, Icon::Close, &l.t("spostamento-chiudi"), true).clicked();
        });
    });
    close
}

/// Piede del dialogo (`.dlg .foot`): filetto sopra, a sinistra una nota, a destra i pulsanti.
pub(crate) fn dialog_foot(ui: &mut Ui, p: &Palette, note: &str, note_color: Color32, buttons: impl FnOnce(&mut Ui)) {
    let top = ui.cursor().min.y;
    let r = ui.max_rect();
    ui.painter().hline(r.x_range(), top + 0.5, Stroke::new(1.0, p.separator));
    Frame::new().inner_margin(Margin { left: 28, right: 20, top: 14, bottom: 14 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let w = (ui.available_width() - 330.0).max(120.0);
            ui.allocate_ui_with_layout(vec2(w, tokens::focus::MIN_TARGET), Layout::left_to_right(Align::Center), |ui| {
                widgets::paragraph(ui, note, Type::Callout, note_color);
            });
            ui.with_layout(Layout::right_to_left(Align::Center), buttons);
        });
    });
}

/// Avanzamento della simulazione, con "Annulla" che la ferma.
fn progress_dialog(app: &mut App, ctx: &egui::Context, p: &Palette) {
    let Some(job) = app.intervention.sim.as_ref() else { return };
    let (l, f) = (&app.lingue, &app.fmt);
    let done = job.progress.done.load(Ordering::Relaxed);
    let title = l.ta("spostamento-simulazione-titolo", &[("nome", subject_name(l, &job.subject).into())]);
    let mut close = false;
    let width = (ctx.content_rect().width() - 64.0).min(560.0);
    let resp = egui::Modal::new(Id::new(("spostamento", "simulazione"))).frame(dialog_frame(p)).backdrop_color(p.scrim).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 6.0;
        Frame::new().inner_margin(Margin { left: 28, right: 16, top: 22, bottom: 16 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            close |= dialog_title(ui, p, l, &title);
            widgets::paragraph(ui, l.t("spostamento-sottotitolo"), Type::Body, p.text_2);
            ui.add_space(6.0);
            widgets::paragraph(ui, l.t("spostamento-simulazione-testo"), Type::Callout, p.text_2);
            widgets::text(ui, l.ta("spostamento-simulazione-letti", &[("n", done.into()), ("testo", f.count(done).into())]), widgets::num(13.0), p.text);
        });
        dialog_foot(ui, p, &l.t("spostamento-esc"), p.text_2, |ui| {
            close |= widgets::button(ui, p, &l.t("spostamento-annulla"), None, ButtonKind::Secondary, true).clicked();
        });
    });
    if close || resp.should_close() {
        // La simulazione si ferma alla prossima voce; il suo esito non serve più.
        if let Some(job) = app.intervention.sim.take() {
            job.progress.cancel.store(true, Ordering::Relaxed);
        }
    }
}

/// La conferma, variante A: titolo-domanda, stato e prove, le tre cifre, ipotesi e rischi del record (sempre
/// visibili, prima dell'elenco), l'elenco con le caselle (gli elementi tenuti visibili e disattivati), i
/// controlli con l'icona, la reversibilità e la scheda completa della raccomandazione, chiusa finché non la
/// apri. Titolo e cifre seguono le caselle. Il dialogo è modale: il focus resta dentro ed Esc chiude senza
/// fare niente.
fn confirm_dialog(app: &App, ctx: &egui::Context, p: &Palette, dlg: &mut Dialog, req: &mut Option<KbRequest>) -> DialogAction {
    let (l, f) = (&app.lingue, &app.fmt);
    let restore = app.data.as_ref().map(|d| d.evidence.restore);
    let selected_n = dlg.sim.items().iter().zip(&dlg.scelta.selected).filter(|(it, s)| **s && it.fixed.is_none()).count();
    let name = subject_name(l, &dlg.subject);
    let title = l.ta("spostamento-titolo", &[("n", selected_n.into()), ("n_testo", f.count(selected_n as u64).into()), ("nome", name.into())]);
    let screen = ctx.content_rect();
    let width = (screen.width() - 64.0).min(660.0);
    let max_h = (screen.height() - 64.0 - 250.0).max(200.0);
    let (mut close, mut confirm) = (false, false);
    let mut reasons = Vec::new();
    let resp = egui::Modal::new(Id::new(("spostamento", "conferma"))).frame(dialog_frame(p)).backdrop_color(p.scrim).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 4.0;
        Frame::new().inner_margin(Margin { left: 28, right: 16, top: 22, bottom: 6 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            close |= dialog_title(ui, p, l, &title);
            ui.add_space(2.0);
            widgets::paragraph(ui, l.t("spostamento-sottotitolo"), Type::Body, p.text_2);
            ui.add_space(10.0);
            summary_line(ui, p, l, f, app, &dlg.sim, &dlg.subject);
        });
        // Altezza fissata qui: dentro un'area il posto disponibile è la misura del frame prima, e il corpo
        // non crescerebbe mai.
        ScrollArea::vertical().id_salt(("spostamento", "corpo")).max_height(max_h).min_scrolled_height(max_h).auto_shrink([false, true]).show(ui, |ui| {
            Frame::new().inner_margin(Margin { left: 28, right: 28, top: 10, bottom: 14 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                numbers(ui, p, l, f, &dlg.sim, &dlg.scelta.selected);
                assumptions(ui, p, l, dlg.sim.record());
                items_list(ui, p, l, f, &dlg.sim, &mut dlg.scelta);
                checks(ui, p, l, f, &dlg.sim, &dlg.scelta.selected);
                manual_checks(ui, p, l, &dlg.sim, &mut dlg.scelta);
                ui.add_space(14.0);
                widgets::paragraph(ui, l.t("spostamento-reversibilita"), Type::Body, p.text);
                match &restore {
                    Some(Ok(false)) => {}
                    Some(Ok(true)) => {
                        widgets::paragraph(ui, l.t("spostamento-punti-ripristino"), Type::Callout, p.text_2);
                    }
                    _ => {
                        widgets::paragraph(ui, l.t("spostamento-punti-ripristino-ignoti"), Type::Callout, p.text_2);
                    }
                }
                ui.add_space(6.0);
                ui.push_id("scheda", |ui| widgets::disclosure(ui, p, &l.t("spostamento-scheda"), Type::BodyStrong, &mut dlg.details));
                if dlg.details {
                    ui.add_space(4.0);
                    Frame::new().inner_margin(Margin { left: 26, right: 0, top: 0, bottom: 0 }).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        record_card(ui, p, l, f, app, &dlg.sim, &dlg.scelta.selected, &dlg.subject, req);
                    });
                }
            });
        });
        // La scelta può essere cambiata in questo frame: il pulsante segue la scelta di adesso.
        reasons = plan::check(&dlg.sim, &dlg.scelta);
        let (note, color) = match missing_text(l, &reasons) {
            Some(why) => (why, p.warn),
            None => (l.t("spostamento-esc"), p.text_2),
        };
        dialog_foot(ui, p, &note, color, |ui| {
            confirm |= widgets::button(ui, p, &l.t("spostamento-sposta"), None, ButtonKind::Primary, reasons.is_empty()).clicked();
            close |= widgets::button(ui, p, &l.t("spostamento-annulla"), None, ButtonKind::Secondary, true).clicked();
        });
    });
    if close || resp.should_close() {
        DialogAction::Close
    } else if confirm && reasons.is_empty() {
        DialogAction::Confirm
    } else {
        DialogAction::Keep
    }
}

/// Riga sotto il titolo: stato con icona e parola, prove, regola o funzione.
fn summary_line(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, app: &App, sim: &Simulazione, subject: &Subject) {
    let r = sim.record();
    ui.spacing_mut().item_spacing.y = 3.0;
    inspector::state_line(ui, p, l, &r.state);
    let proof = r.state.manageability().map(|x| x.facts()).unwrap_or_default();
    // Lo stesso fatto da due fonti (la versione in uso di CapCut) qui compare una volta: le fonti sono
    // nella scheda in fondo.
    let mut facts: Vec<String> = Vec::new();
    for s in r.identity.0.iter().chain(proof).map(|fa| kbtext::fact_text(l, f, fa)) {
        if !facts.contains(&s) {
            facts.push(s);
        }
    }
    if !facts.is_empty() {
        widgets::paragraph(ui, l.ta("spostamento-riga-prove", &[("prove", facts.join("; ").into())]), Type::Callout, p.text_2);
    }
    match subject.rule.and_then(|i| app.kb.rules.get(i)) {
        Some(rule) => {
            let s = l.ta("spostamento-riga-regola", &[("nome", l.t(&rule.key("nome")).into()), ("id", rule.id.clone().into()), ("data", f.day(rule.verified).into())]);
            widgets::paragraph(ui, s, Type::Callout, p.text_2);
        }
        None => {
            widgets::paragraph(ui, l.ta("spostamento-riga-funzione", &[("nome", subject_name(l, subject).into())]), Type::Callout, p.text_2);
        }
    }
}

/// I tre numeri (`.dlg .nums`): da spostare, liberato subito (0), dopo lo svuotamento con lo stato della
/// stima, in un riquadro con le colonne separate da un filetto; poi la nota, i file esclusi per altri nomi
/// e le cartelle negate, se ci sono.
fn numbers(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, sim: &Simulazione, selected: &[bool]) {
    let est = sim.estimate(selected);
    let moving: u64 = sim.items().iter().zip(selected).filter(|(it, s)| **s && it.fixed.is_none()).map(|(it, _)| it.alloc).sum();
    let items = [
        (l.t("spostamento-numero-da-spostare"), f.size(moving), None),
        (l.t("spostamento-numero-subito"), f.size(0), None),
        (l.t("spostamento-numero-dopo"), estimate_value(l, f, &est.estimate), Some(estimate_state(l, f, &est.estimate))),
    ];
    ui.add_space(6.0);
    let items: Vec<widgets::Figure> = items.into_iter().map(|(label, value, sub)| (value, label, sub)).collect();
    widgets::figures(ui, p, p.panel2, 30.0, &items);
    ui.add_space(8.0);
    widgets::paragraph(ui, l.t("spostamento-numeri-nota"), Type::Callout, p.text_2);
    let (n, bytes) = est.excluded_names;
    if n > 0 {
        let s = l.ta("spostamento-esclusi-nomi", &[("n", n.into()), ("n_testo", f.count(n).into()), ("byte", f.size(bytes).into())]);
        widgets::paragraph(ui, s, Type::Callout, p.text_2);
    }
    if est.denied_dirs > 0 {
        let s = l.ta("spostamento-cartelle-negate", &[("n", est.denied_dirs.into()), ("n_testo", f.count(est.denied_dirs).into())]);
        widgets::paragraph(ui, s, Type::Callout, p.text_2);
    }
}

/// Ipotesi e rischi del record nel corpo della conferma, non solo nella scheda chiusa: le ipotesi con il glifo
/// del dedotto, ogni rischio in un riquadro di avviso.
fn assumptions(ui: &mut Ui, p: &Palette, l: &Lingue, r: &quado_model::record::Record) {
    if r.hypotheses.is_empty() && r.risks.is_empty() {
        return;
    }
    dialog_heading(ui, p, &l.t("spostamento-ipotesi-rischi"), None);
    for h in &r.hypotheses {
        inspector::fact_in(ui, p, widgets::Evidence::Inferred, &l.t(h), l, p.text);
    }
    for k in &r.risks {
        ui.add_space(4.0);
        widgets::banner(ui, p, &l.t(k));
    }
}

/// Titolo di una parte del corpo (`.dlg h4`), con un dettaglio facoltativo accanto.
fn dialog_heading(ui: &mut Ui, p: &Palette, title: &str, detail: Option<&str>) {
    ui.add_space(18.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        widgets::typed(ui, title, Type::BodyStrong, p.text);
        if let Some(d) = detail {
            widgets::typed(ui, d, Type::Callout, p.text_2);
        }
    });
    ui.add_space(6.0);
}

/// Elenco con le caselle (`Scelta::selected`), in un riquadro che scorre (`.dlg .items`): gli elementi fissi
/// visibili e disattivati con il loro segno, quelli bloccati disattivati con il motivo di ogni blocco. Con più
/// di un elemento da scegliere, «Seleziona tutti» accanto al titolo: per Da verificare e Decidi tu le caselle
/// partono vuote (`Simulazione::default_selection`).
fn items_list(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, sim: &Simulazione, scelta: &mut Scelta) {
    let choosable = sim.items().iter().filter(|it| it.fixed.is_none()).count();
    let chosen = sim.items().iter().zip(&scelta.selected).filter(|(it, s)| **s && it.fixed.is_none()).count();
    let detail = l.ta("spostamento-selezionati", &[("n", chosen.into()), ("testo", f.count(chosen as u64).into()), ("totale", f.count(choosable as u64).into())]);
    dialog_heading(ui, p, &l.t("spostamento-elenco"), Some(&detail));
    let all = sim.selectable();
    if all.iter().filter(|s| **s).count() > 1 {
        let missing = all.iter().zip(&scelta.selected).any(|(a, s)| *a && !*s);
        if ui.push_id(("spostamento", "tutti"), |ui| widgets::button(ui, p, &l.t("spostamento-seleziona-tutti"), None, ButtonKind::Quiet, missing)).inner.clicked() {
            scelta.selected = all;
        }
        ui.add_space(4.0);
    }
    Frame::new().fill(p.panel2).corner_radius(CornerRadius::same(tokens::radius::GROUP)).inner_margin(Margin::symmetric(14, 4)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ScrollArea::vertical().id_salt(("spostamento", "elementi")).max_height(188.0).auto_shrink([false, true]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, it) in sim.items().iter().enumerate() {
                if i > 0 {
                    let y = ui.cursor().min.y;
                    let r = ui.max_rect();
                    ui.painter().hline(r.x_range(), y + 0.5, Stroke::new(1.0, p.separator));
                }
                ui.push_id(("spostamento", i), |ui| item_row(ui, p, l, f, it, scelta.selected.get_mut(i)));
            }
        });
    });
}

/// Una riga dell'elenco: casella con il nome (il percorso nel suggerimento), segno degli elementi fissi,
/// dimensioni su disco, poi il motivo di ogni blocco.
fn item_row(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, it: &plan::SimItem, selected: Option<&mut bool>) {
    let enabled = it.fixed.is_none() && it.blocks.is_empty();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let mut on = enabled && selected.as_deref().copied().unwrap_or(false);
        let r = widgets::checkbox(ui, p, &mut on, &file_name(&it.path), enabled);
        if enabled && let Some(s) = selected {
            *s = on;
        }
        r.on_hover_text(&it.path);
        if let Some(fx) = it.fixed {
            widgets::typed(ui, fixed_text(l, fx), Type::Callout, p.text_2);
        }
        if it.fixed.is_none() && (it.files > 0 || it.dirs > 0 || it.alloc > 0) {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widgets::text(ui, f.size(it.alloc), widgets::num(13.0), p.text_2);
            });
        }
    });
    for b in &it.blocks {
        ui.horizontal_top(|ui| {
            ui.add_space(26.0);
            widgets::paragraph(ui, block_text(l, f, b), Type::Callout, p.warn);
        });
        ui.add_space(4.0);
    }
}

/// Riga di un controllo nel dialogo: l'icona dell'esito (superato nel colore `ok`, non superato in quello
/// di avviso, informazione neutra) e il testo, sempre.
fn check_row(ui: &mut Ui, p: &Palette, tone: Tone, s: &str) {
    let (icon, icon_color, text) = match tone {
        Tone::Ok => (Icon::StateManageable, p.ok, p.text),
        Tone::Fail => (Icon::Warning, p.warn, p.warn),
        Tone::Info => (Icon::Info, p.text_2, p.text_2),
    };
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 20.0), Sense::hover());
        icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(r.center().x, r.min.y + 10.0), vec2(16.0, 16.0)), icon, icon_color);
        widgets::paragraph(ui, s, Type::Body, text);
    });
    ui.add_space(4.0);
}

/// Controlli con il loro esito: file in uso con l'ora, riferimenti (con "parziale"), capacità
/// del Cestino, Sensore memoria, gli altri controlli automatici della regola.
fn checks(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, sim: &Simulazione, selected: &[bool]) {
    dialog_heading(ui, p, &l.t("spostamento-controlli"), None);
    let proposed: Vec<_> = sim.items().iter().filter(|it| it.fixed.is_none()).collect();
    let with = |pred: fn(&Block) -> bool| proposed.iter().filter(|it| it.blocks.iter().any(pred)).count() as u64;
    let ora = f.time(sim.checked_at());
    // Restart Manager.
    let (in_use, rm_err) = (with(|b| matches!(b, Block::InUse(_))), with(|b| matches!(b, Block::RestartManager(_))));
    if in_use > 0 {
        check_row(ui, p, Tone::Fail, &l.ta("spostamento-rm-in-uso", &[("n", in_use.into()), ("n_testo", f.count(in_use).into()), ("ora", ora.clone().into())]));
    }
    if rm_err > 0 {
        check_row(ui, p, Tone::Fail, &l.ta("spostamento-rm-errore", &[("n", rm_err.into()), ("n_testo", f.count(rm_err).into())]));
    }
    if in_use == 0 && rm_err == 0 {
        check_row(ui, p, Tone::Ok, &l.ta("spostamento-rm-ok", &[("ora", ora.into())]));
    }
    // Riferimenti: se la regola li chiede o se c'è l'eccezione per un elemento protetto.
    let r = sim.record();
    let refs_checked = r.checks.iter().any(|c| c.key == SIMULATION_CHECKS[1]) || (r.protected_exception && matches!(r.state, State::ToVerify(_)));
    if refs_checked {
        let found = with(|b| matches!(b, Block::Referenced(_)));
        if found > 0 {
            check_row(ui, p, Tone::Fail, &l.ta("spostamento-rif-trovati", &[("n", found.into()), ("n_testo", f.count(found).into())]));
        } else {
            check_row(ui, p, Tone::Ok, &l.t("spostamento-rif-ok"));
        }
        let partial = &sim.references().partial;
        if !partial.is_empty() {
            let mut kinds: Vec<ReferenceKind> = partial.iter().map(|(k, _)| *k).collect();
            kinds.sort();
            kinds.dedup();
            let fonti: Vec<String> = kinds.into_iter().map(|k| reference_kind(l, k)).collect();
            check_row(ui, p, Tone::Info, &l.ta("spostamento-rif-parziale", &[("fonti", fonti.join(", ").into())]));
        }
    }
    // Capacità del Cestino.
    let volume = sim.volume().root.trim_end_matches('\\').to_owned();
    match sim.capacity(selected) {
        CapacityCheck::Fits { capacity, total } => check_row(
            ui,
            p,
            Tone::Ok,
            &l.ta("spostamento-capacita-ok", &[("volume", volume.into()), ("totale", f.size(total).into()), ("capacita", f.size(capacity).into())]),
        ),
        CapacityCheck::Exceeded { capacity, total } => check_row(
            ui,
            p,
            Tone::Fail,
            &l.ta("spostamento-capacita-superata", &[("volume", volume.into()), ("capacita", f.size(capacity).into()), ("totale", f.size(total).into())]),
        ),
        CapacityCheck::Unknown => check_row(ui, p, Tone::Fail, &l.ta("spostamento-capacita-ignota", &[("volume", volume.into())])),
    }
    // Cartelle sincronizzate: cosa Quado ha guardato quando non ne trova, e dove si recuperano le copie
    // nel cloud quando ne trova.
    let mut clouds: Vec<&str> = Vec::new();
    for m in proposed.iter().flat_map(|it| &it.manual) {
        let ManualCheck::Synced { provider, kind } = m;
        if *kind != SyncKind::Devices && !clouds.contains(&provider.as_str()) {
            clouds.push(provider);
        }
    }
    if proposed.iter().all(|it| it.manual.is_empty()) {
        check_row(ui, p, Tone::Info, &l.t("spostamento-sinc-ok"));
    }
    for c in clouds {
        check_row(ui, p, Tone::Info, &l.ta("spostamento-sincronizzato-ripristino", &[("provider", c.to_owned().into())]));
    }
    // Sensore memoria.
    if let Some(days) = sim.storage_sense_days() {
        let d = u64::from(days);
        check_row(ui, p, Tone::Info, &l.ta("spostamento-sensore", &[("giorni", d.into()), ("giorni_testo", f.count(d).into())]));
    }
    // Gli altri controlli automatici della regola, con l'esito che hanno.
    for c in sim.rule_checks().iter().filter(|c| c.kind == CheckKind::Automatic && !SIMULATION_CHECKS.contains(&c.key.as_str())) {
        let tone = if c.outcome == quado_model::record::CheckOutcome::Failed { Tone::Fail } else { Tone::Info };
        check_row(ui, p, tone, &l.ta("spostamento-controllo-regola", &[("controllo", l.t(&c.key).into()), ("esito", kbtext::check_outcome(l, c.outcome).into())]));
    }
}

/// Testo della spunta per un elemento in una cartella sincronizzata: il cloud c'è per le cartelle registrate in
/// Windows e per Dropbox e Google Drive; Syncthing e Resilio Sync sincronizzano solo tra dispositivi.
fn synced_key(kind: SyncKind) -> &'static str {
    match kind {
        SyncKind::Registered => "spostamento-sincronizzato",
        SyncKind::Cloud => "spostamento-sincronizzato-cloud",
        SyncKind::Devices => "spostamento-sincronizzato-dispositivi",
    }
}

/// Controlli manuali da spuntare: quelli della regola e quelli degli elementi scegliibili (file sincronizzati).
fn manual_checks(ui: &mut Ui, p: &Palette, l: &Lingue, sim: &Simulazione, scelta: &mut Scelta) {
    let open = |it: &plan::SimItem| it.fixed.is_none() && it.blocks.is_empty() && !it.manual.is_empty();
    if sim.rule_manual().is_empty() && !sim.items().iter().any(open) {
        return;
    }
    dialog_heading(ui, p, &l.t("spostamento-manuali"), None);
    for (k, key) in sim.rule_manual().iter().enumerate() {
        if let Some(t) = scelta.ticked_rule.get_mut(k) {
            ui.push_id(("regola", k), |ui| widgets::checkbox(ui, p, t, &l.t(key), true));
        }
    }
    for (i, it) in sim.items().iter().enumerate().filter(|(_, it)| open(it)) {
        for (k, m) in it.manual.iter().enumerate() {
            let text = match m {
                ManualCheck::Synced { provider, kind } => l.ta(synced_key(*kind), &[("provider", provider.clone().into())]),
            };
            let label = l.ta("spostamento-manuale-elemento", &[("nome", file_name(&it.path).into()), ("testo", text.into())]);
            if let Some(t) = scelta.ticked_items.get_mut(i).and_then(|v| v.get_mut(k)) {
                ui.push_id(("elemento", i, k), |ui| widgets::checkbox(ui, p, t, &label, true));
            }
        }
    }
}

/// Scheda completa della raccomandazione, dentro la riga apribile: prove di identità e di
/// gestibilità con la loro provenienza, affidabilità, i campi del record con i controlli come li ha visti la
/// simulazione, la regola con fonte e data.
#[allow(clippy::too_many_arguments)]
fn record_card(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, app: &App, sim: &Simulazione, selected: &[bool], subject: &Subject, req: &mut Option<KbRequest>) {
    let r = sim.record();
    if !r.identity.0.is_empty() {
        widgets::text(ui, l.t("spostamento-scheda-identita"), medium(12.0), p.text_2);
        for fa in &r.identity.0 {
            inspector::fact(ui, p, kbtext::evidence(fa.provenance), &kbtext::fact_with_source(l, f, fa), l);
        }
    }
    if let Some(proof) = r.state.manageability() {
        widgets::text(ui, l.t("ispettore-prova-gestibilita"), medium(12.0), p.text_2);
        for fa in proof.facts() {
            inspector::fact(ui, p, kbtext::evidence(fa.provenance), &kbtext::fact_with_source(l, f, fa), l);
        }
    }
    if kbtext::has_deduced(r) {
        widgets::text(ui, kbtext::reliability_label(l, r.reliability), sans(12.0), p.text_3);
    }
    ui.add_space(4.0);
    let mut shown = r.clone();
    shown.checks = sim.rule_checks();
    inspector::record_fields(ui, p, l, f, &shown, &sim.estimate(selected).estimate, req);
    ui.add_space(4.0);
    match subject.rule.and_then(|i| app.kb.rules.get(i)) {
        Some(rule) => {
            widgets::text(ui, l.ta("spostamento-scheda-regola", &[("nome", l.t(&rule.key("nome")).into()), ("id", rule.id.clone().into())]), medium(12.5), p.text);
            let mut line = l.ta("regole-verificata", &[("data", f.day(rule.verified).into())]);
            if let Some(v) = &rule.versions {
                line.push_str(" · ");
                line.push_str(&l.ta("regole-versioni", &[("programma", v.programma.clone().into()), ("min", v.min.clone().into()), ("max", v.max.clone().into())]));
            }
            wrapped(ui, &line, 12.5, p.text_2);
            for s in &rule.sources {
                let kind = l.t(if s.ufficiale { "regole-fonte-ufficiale" } else { "regole-fonte-non-ufficiale" });
                wrapped(ui, &l.ta("spostamento-scheda-fonte", &[("titolo", s.titolo.clone().into()), ("tipo", kind.into())]), 12.5, p.text_2);
                let (label, shown, _) = kbtext::source_place(l, &s.url);
                if let Some(label) = label {
                    wrapped(ui, &label, 12.5, p.text_2);
                }
                ui.add(egui::Label::new(RichText::new(shown).font(mono(12.0)).color(p.text_2)).wrap());
                ui.add(egui::Label::new(RichText::new(format!("«{}»", s.estratto)).font(sans(12.5)).color(p.text)).wrap());
            }
        }
        None => {
            wrapped(ui, &l.ta("spostamento-riga-funzione", &[("nome", subject_name(l, subject).into())]), 12.5, p.text_2);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Risultato

/// Cosa chiede il risultato dopo il frame.
#[derive(Default)]
struct PanelAction {
    open_bin: bool,
    empty: bool,
    /// Nuovo stato della casella della prova, se è cambiato in questo frame.
    tried: Option<bool>,
}

/// Il risultato dell'ultimo spostamento di questa sessione, in cima alla vista Interventi. Restituisce falso
/// se non c'è niente da mostrare.
pub fn result_panel(app: &mut App, ui: &mut Ui, p: &Palette) -> bool {
    let Some(o) = app.intervention.last.as_ref() else { return false };
    let (l, f) = (&app.lingue, &app.fmt);
    let journal = app.intervention.journal();
    let restore = app.data.as_ref().map(|d| d.evidence.restore);
    let mut act = PanelAction::default();
    widgets::bare_card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        // Testata: cosa, quando, registro.
        Frame::new().inner_margin(Margin { left: 24, right: 24, top: 22, bottom: 4 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            widgets::paragraph(ui, l.t("spostamento-risultato-titolo"), Type::Title, p.text);
            let name = subject_name(l, &o.subject);
            match o.measures.first() {
                Some(m) => {
                    let s = l.ta("spostamento-risultato-quando", &[("nome", name.into()), ("data", f.date(m.at).into()), ("ora", f.time(m.at).into())]);
                    widgets::paragraph(ui, s, Type::Body, p.text_2);
                }
                None => {
                    widgets::paragraph(ui, name, Type::Body, p.text_2);
                }
            }
            ui.add_space(4.0);
            match journal {
                Some(JournalState::Written) => check_line(ui, p, Tone::Ok, &l.t("spostamento-registro-scritto")),
                Some(JournalState::Pending) => check_line(ui, p, Tone::Info, &l.t("spostamento-registro-in-corso")),
                Some(JournalState::NoDir) => check_line(ui, p, Tone::Info, &l.t("spostamento-registro-nessuna-cartella")),
                Some(JournalState::Failed(code)) => check_line(ui, p, Tone::Fail, &l.ta("spostamento-registro-errore", &[("codice", code.to_string().into())])),
                None => {}
            }
        });
        // I quattro passi, uniti da una linea come una sequenza.
        Frame::new().inner_margin(Margin { left: 24, right: 24, top: 6, bottom: 20 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            step(ui, p, 1, &l.t("spostamento-passo-spostamento"), true, |ui| step_move(ui, p, l, f, o));
            step(ui, p, 2, &l.t("spostamento-passo-spazio"), true, |ui| step_space(ui, p, l, f, o));
            let rule = o.subject.rule.and_then(|i| app.kb.rules.get(i));
            step(ui, p, 3, &l.t("spostamento-passo-cestino"), true, |ui| step_bin(ui, p, l, f, o, rule, &mut act));
            step(ui, p, 4, &l.t("spostamento-passo-verifica"), false, |ui| step_verify(ui, p, l, f, o, &restore));
        });
    });
    if act.open_bin
        && let Err(e) = (app.env.actions.launcher)(&Launch::RecycleBin)
    {
        app.notice.set(app.lingue.ta("avviso-shell-errore", &[("codice", hresult(e.0).into())]));
    }
    if let (Some(v), Some(o)) = (act.tried, app.intervention.last.as_mut()) {
        o.tried = v;
    }
    if act.empty {
        start_empty(app);
    }
    true
}

/// Un passo numerato del risultato: il numero in un cerchio nel tono dei riquadri, il titolo e il contenuto
/// accanto; con `more` una linea scende dal cerchio fino al passo dopo.
fn step(ui: &mut Ui, p: &Palette, n: u8, title: &str, more: bool, body: impl FnOnce(&mut Ui)) {
    ui.add_space(14.0);
    let r = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 14.0;
        let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 13.0, p.panel2);
        ui.painter().circle_stroke(r.center(), 12.5, Stroke::new(1.0, p.separator));
        ui.painter().text(r.center(), Align2::CENTER_CENTER, n.to_string(), widgets::font(Type::Caption), p.text_2);
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.add_space(3.0);
            widgets::paragraph(ui, title, Type::BodyStrong, p.text);
            ui.add_space(2.0);
            body(ui);
        });
        r
    });
    if more {
        let c = r.inner.center();
        ui.painter().vline(c.x, (r.inner.max.y + 4.0)..=(r.response.rect.max.y + 10.0), Stroke::new(1.0, p.separator));
    }
}

/// Passo 1: riusciti, saltati e falliti con il motivo; con esito parziale cosa è nel Cestino e come ripristinarlo.
fn step_move(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, o: &Outcome) {
    if o.exec.is_some() {
        let done = o.progress.done.load(Ordering::Relaxed);
        widgets::paragraph(ui, l.ta("spostamento-in-corso", &[("fatti", f.count(done).into()), ("totale", f.count(o.total as u64).into())]), Type::Body, p.text);
        return;
    }
    if let Some(code) = o.start_error {
        wrapped(ui, &l.ta("spostamento-partenza-errore", &[("codice", code.to_string().into())]), 13.0, p.warn);
        return;
    }
    let Some(r) = &o.riepilogo else { return };
    let recycled = r.items.iter().filter(|i| i.outcome == ItemOutcome::Recycled).count() as u64;
    let others: Vec<_> = r.items.iter().filter(|i| i.outcome != ItemOutcome::Recycled).collect();
    let total = f.count(r.items.len() as u64);
    widgets::paragraph(ui, l.ta("spostamento-spostati", &[("n", recycled.into()), ("n_testo", f.count(recycled).into()), ("totale", total.into())]), Type::Body, p.text);
    if others.is_empty() {
        wrapped(ui, &l.t("spostamento-nessuno-saltato"), 12.5, p.text_2);
        return;
    }
    wrapped(ui, &l.t("spostamento-ricontrollati"), 12.5, p.text_2);
    for i in &others {
        let s = l.ta("spostamento-esito-riga", &[("nome", file_name(&i.path).into()), ("esito", outcome_text(l, i.outcome).into())]);
        wrapped(ui, &s, 12.5, if matches!(i.outcome, ItemOutcome::NotInRecycleBin(_)) { p.warn } else { p.text });
    }
    if recycled == 0 {
        wrapped(ui, &l.t("spostamento-nessuno"), 12.5, p.text);
    } else {
        let n = others.len() as u64;
        let s = l.ta("spostamento-parziale", &[("n", n.into()), ("n_testo", f.count(n).into()), ("m", f.count(recycled).into())]);
        wrapped(ui, &s, 12.5, p.text);
    }
}

/// Passo 2: spazio libero prima, ora e differenza; il confronto subito dopo l'azione.
fn step_space(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, o: &Outcome) {
    let (Some(start), Some(after)) = (o.measure(MeasurePoint::Start), o.measure(MeasurePoint::AfterAction)) else {
        if let Some((_, code)) = o.measure_errors.iter().find(|(pt, _)| *pt == MeasurePoint::AfterAction) {
            wrapped(ui, &l.ta("spostamento-misura-errore", &[("codice", code.to_string().into())]), 12.5, p.warn);
        }
        return;
    };
    let diff = i64::try_from(i128::from(after.free) - i128::from(start.free)).unwrap_or(0);
    figures(
        ui,
        p,
        &[
            (l.t("spostamento-spazio-prima"), f.size(start.free), None),
            (l.t("spostamento-spazio-ora"), f.size(after.free), None),
            (l.t("spostamento-spazio-differenza"), signed(l, f, diff), None),
        ],
    );
    wrapped(ui, &l.t("spostamento-spazio-nota"), 12.5, p.text_2);
    if let Some((_, c)) = o.comparisons.iter().find(|(pt, _)| *pt == MeasurePoint::AfterAction) {
        ui.add_space(4.0);
        widgets::typed(ui, l.t("spostamento-confronto-dopo-azione"), Type::Caption, p.text_2);
        comparison(ui, p, l, f, c);
    }
}

/// Passo 3: "Apri il Cestino" con gli elementi spostati da Quado e come ripristinarli, "Svuota il Cestino…" con
/// gli altri elementi secondo il conteggio di Windows. Prima dei pulsanti, cosa provare prima di svuotare
/// (`regola-<id>-prova`) o i rischi del record; per le versioni di CapCut «Svuota il Cestino…» resta
/// disattivato finché non spunti la prova.
fn step_bin(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, o: &Outcome, rule: Option<&quado_kb::rules::Rule>, act: &mut PanelAction) {
    let Some(r) = &o.riepilogo else { return };
    let moved: Vec<usize> = (0..r.items.len()).filter(|&k| r.items[k].outcome == ItemOutcome::Recycled).collect();
    if moved.is_empty() {
        return;
    }
    let n = moved.len() as u64;
    let s = l.ta("spostamento-apri-testo", &[("n", n.into()), ("n_testo", f.count(n).into()), ("data", f.date(r.finished).into()), ("ora", f.time(r.finished).into())]);
    wrapped(ui, &s, 12.5, p.text);
    // I nomi in fila, a capo quando serve; il percorso completo nel suggerimento.
    let names: Vec<String> = moved.iter().map(|&k| file_name(&r.items[k].path)).collect();
    let paths: Vec<&str> = moved.iter().map(|&k| r.items[k].path.as_str()).collect();
    widgets::paragraph(ui, names.join(", "), Type::Callout, p.text_2).on_hover_text(paths.join("\n"));
    wrapped(ui, &l.t("spostamento-ripristina"), 12.5, p.text_2);
    // Altri elementi del Cestino: il conteggio dopo l'azione meno quelli di questo intervento.
    let ours_logical: u64 = r.items.iter().filter(|i| i.outcome == ItemOutcome::Recycled).filter_map(|i| o.sim.items().iter().find(|s| s.path == i.path)).map(|s| s.logical).sum();
    let bin = o.measure(MeasurePoint::AfterAction).and_then(|m| m.bin);
    let other = match bin {
        None => l.t("spostamento-svuota-ignoto"),
        Some(b) => {
            let items = b.items.saturating_sub(n);
            let bytes = b.bytes.saturating_sub(ours_logical);
            if items == 0 {
                l.t("spostamento-svuota-nessun-altro")
            } else {
                l.ta("spostamento-svuota-testo", &[("n", items.into()), ("n_testo", f.count(items).into()), ("byte", f.size(bytes).into())])
            }
        }
    };
    ui.add_space(4.0);
    wrapped(ui, &other, 12.5, p.text);
    let emptied = o.measure(MeasurePoint::AfterEmpty).is_some();
    // Cosa provare prima di svuotare: il testo della regola, altrimenti i rischi del record (copie identiche).
    let before: Vec<String> = match rule.and_then(quado_kb::rules::Rule::try_key) {
        Some(k) => vec![l.t(&k)],
        None => o.sim.record().risks.iter().map(|k| l.t(k)).collect(),
    };
    if !emptied {
        for s in &before {
            ui.add_space(4.0);
            widgets::banner(ui, p, s);
        }
    }
    let mut ready = true;
    if let Some(k) = rule.and_then(quado_kb::rules::Rule::tried_key)
        && !emptied
    {
        ui.add_space(4.0);
        let mut tried = o.tried;
        if ui.push_id("prova_fatta", |ui| widgets::checkbox(ui, p, &mut tried, &l.t(&k), true)).inner.changed() || tried != o.tried {
            act.tried = Some(tried);
        }
        ready = tried;
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        act.open_bin |= widgets::button(ui, p, &l.t("spostamento-apri"), Some(Icon::Trash), ButtonKind::Secondary, true).clicked();
        act.empty |= widgets::button(ui, p, &l.t("spostamento-svuota"), None, ButtonKind::Secondary, !o.running() && !emptied && ready).clicked();
    });
    if o.empty.is_some() {
        wrapped(ui, &l.t("spostamento-svuota-in-corso"), 12.5, p.text_2);
    }
    if let Some(hr) = o.empty_error {
        wrapped(ui, &l.ta("spostamento-svuota-errore", &[("codice", hresult(hr).into())]), 12.5, p.warn);
    }
}

/// Passo 4: verifica dopo lo svuotamento, con i due numeri separati e le voci di spiegazione.
fn step_verify(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, o: &Outcome, restore: &Option<Result<bool, quado_kb::extract::ExtractError>>) {
    match o.comparisons.iter().find(|(pt, _)| *pt == MeasurePoint::AfterEmpty) {
        Some((_, c)) => {
            widgets::typed(ui, l.t("spostamento-confronto-dopo-svuotamento"), Type::Caption, p.text_2);
            comparison(ui, p, l, f, c);
        }
        None => {
            wrapped(ui, &l.t("spostamento-verifica-attesa"), 12.5, p.text_2);
        }
    }
    if let Some((_, code)) = o.measure_errors.iter().find(|(pt, _)| *pt == MeasurePoint::AfterEmpty) {
        wrapped(ui, &l.ta("spostamento-misura-errore", &[("codice", code.to_string().into())]), 12.5, p.warn);
    }
    match restore {
        Some(Ok(false)) => {}
        Some(Ok(true)) => {
            wrapped(ui, &l.t("spostamento-punti-ripristino"), 12.5, p.text_2);
        }
        _ => {
            wrapped(ui, &l.t("spostamento-punti-ripristino-ignoti"), 12.5, p.text_2);
        }
    }
    wrapped(ui, &l.t("spostamento-regola-5"), 12.0, p.text_3);
    // Nuova lettura delle cartelle toccate.
    if o.rescan.is_some() {
        wrapped(ui, &l.t("spostamento-riscansione-in-corso"), 12.0, p.text_3);
    } else if let Some(n) = o.rescan_note {
        if n.full_scan {
            wrapped(ui, &l.t("spostamento-riscansione-radice"), 12.0, p.text_2);
        } else if n.failed > 0 {
            let k = n.failed as u64;
            wrapped(ui, &l.ta("spostamento-riscansione-fallite", &[("n", k.into()), ("n_testo", f.count(k).into())]), 12.0, p.warn);
        } else {
            wrapped(ui, &l.t("spostamento-riscansione-fatta"), 12.0, p.text_3);
        }
    }
}

/// Stimato e variazione del volume, due numeri separati e mai sommati, poi la differenza e, oltre il 5%, le
/// voci di spiegazione o la differenza non spiegata.
fn comparison(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, c: &Comparison) {
    figures(
        ui,
        p,
        &[(l.t("spostamento-stimato"), estimate_value(l, f, &c.estimated), Some(estimate_state(l, f, &c.estimated))), (l.t("spostamento-variazione"), signed(l, f, c.obtained), None)],
    );
    wrapped(ui, &l.t("spostamento-variazione-nota"), 12.0, p.text_3);
    match (c.difference, c.basis_points) {
        (None, _) => {
            wrapped(ui, &l.t("spostamento-solo-ottenuto"), 12.5, p.text_2);
        }
        (Some(0), _) => {
            wrapped(ui, &l.t("spostamento-differenza-nessuna"), 12.5, p.text_2);
        }
        (Some(d), Some(bp)) => {
            wrapped(ui, &l.ta("spostamento-differenza", &[("valore", signed(l, f, d).into()), ("percento", percent(f, bp).into())]), 12.5, p.text);
        }
        (Some(d), None) => {
            wrapped(ui, &l.ta("spostamento-differenza-senza-percento", &[("valore", signed(l, f, d).into())]), 12.5, p.text);
        }
    }
    for e in &c.explanations {
        check_line(ui, p, Tone::Info, &explanation_text(l, f, e));
    }
    if let Some(u) = c.unexplained {
        let pct = reference(&c.estimated, c.obtained).map(|r| {
            let bp = (u128::from(u.unsigned_abs()) * 10_000).div_ceil(u128::from(r));
            percent(f, u64::try_from(bp).unwrap_or(u64::MAX))
        });
        let s = match pct {
            Some(pct) => l.ta("spostamento-non-spiegata", &[("valore", signed(l, f, u).into()), ("percento", pct.into())]),
            None => l.ta("spostamento-differenza-senza-percento", &[("valore", signed(l, f, u).into())]),
        };
        check_line(ui, p, Tone::Fail, &s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bin_left_after_emptying_from_the_windows_count() {
        let after = Some(BinCount { items: 27, bytes: 1000 });
        assert_eq!(still_in_bin(after, Some(BinCount::default()), 900), Some(0), "Cestino vuoto");
        assert_eq!(still_in_bin(after, after, 900), Some(900), "dialogo annullato: tutto ancora lì");
        assert_eq!(still_in_bin(after, Some(BinCount { items: 3, bytes: 10 }), 900), None, "svuotato in parte: non si sa");
        assert_eq!(still_in_bin(None, Some(BinCount { items: 3, bytes: 10 }), 900), None);
        assert_eq!(still_in_bin(after, None, 900), None);
    }

    #[test]
    fn reference_is_the_nearest_bound() {
        assert_eq!(reference(&Estimate::Exact(100), 0), Some(100));
        assert_eq!(reference(&Estimate::Range { min: 50, max: 100 }, 10), Some(50));
        assert_eq!(reference(&Estimate::Range { min: 50, max: 100 }, 200), Some(100));
        assert_eq!(reference(&Estimate::Exact(0), 10), None);
    }

    #[test]
    fn every_block_and_outcome_has_its_text() {
        use quado_actions::restart::{ProcessUse, RmError};
        use quado_model::intervention::Reference;
        let f = Formatter::default();
        for tag in ["it", "en"] {
            let l = Lingue::load(tag, None);
            let refs = vec![Reference { kind: ReferenceKind::ScheduledTask, location: "\\Attività".into(), target: "C:\\x".into() }];
            let mut blocks = vec![
                Block::NoRecycleBin,
                Block::NukeOnDelete,
                Block::CapacityUnknown,
                Block::LargerThanBin { logical: 10, capacity: 5 },
                Block::InUse(vec![ProcessUse { pid: 7, app_name: "CapCut".into(), service: None }]),
                Block::RestartManager(RmError::List(5)),
                Block::Referenced(refs),
                Block::LongPath,
                Block::ContainsLongPath,
                Block::Unreadable(5),
                Block::Missing,
                Block::OtherVolume,
            ];
            for k in [
                ProtectedKind::WindowsDir,
                ProtectedKind::WinSxS,
                ProtectedKind::WindowsInstaller,
                ProtectedKind::SystemVolumeInformation,
                ProtectedKind::PagingFile,
                ProtectedKind::Hibernation,
                ProtectedKind::SwapFile,
                ProtectedKind::RegisteredProgram,
            ] {
                blocks.push(Block::Protected(k));
            }
            let mut seen = std::collections::HashSet::new();
            for b in &blocks {
                let s = block_text(&l, &f, b);
                assert!(!s.is_empty() && !s.starts_with('['), "{tag}: {b:?} → {s}");
                assert!(seen.insert(s), "{tag}: {b:?} ha lo stesso testo di un altro blocco");
            }
            for o in [
                ItemOutcome::Skipped(SkipReason::Changed),
                ItemOutcome::Skipped(SkipReason::Missing),
                ItemOutcome::Skipped(SkipReason::InUse),
                ItemOutcome::Skipped(SkipReason::Unreadable(5)),
                ItemOutcome::Failed(-2_147_024_864),
                ItemOutcome::NotInRecycleBin(0),
                ItemOutcome::NotReached,
            ] {
                let s = outcome_text(&l, o);
                assert!(!s.is_empty() && !s.starts_with('['), "{tag}: {o:?}");
            }
            for e in [
                Explanation::SkippedOrFailed { items: 2, bytes: 10 },
                Explanation::ExcludedNames { files: 1, bytes: 10 },
                Explanation::StillInBin { bytes: 10 },
                Explanation::OtherBinItems { items: 3, bytes: 10 },
                Explanation::StillOpen { files: 1 },
                Explanation::OtherActivity { bytes: -10 },
            ] {
                let s = explanation_text(&l, &f, &e);
                assert!(!s.is_empty() && !s.starts_with('['), "{tag}: {e:?}");
            }
        }
    }
}
