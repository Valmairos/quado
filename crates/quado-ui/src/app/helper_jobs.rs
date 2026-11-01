// SPDX-License-Identifier: GPL-3.0-or-later
//! "Ripeti come amministratore" (05 D3, 03 F-SC-3, F-OV-3): l'helper elevato dietro un insieme
//! iniettabile ([`Elevator`]), la richiesta costruita dalle cartelle negate ([`build_request`]) e il
//! lavoro in background ([`HelperState`]): attesa della conferma di Windows, lettura, esito.
//!
//! Le risposte si raccolgono durante il lavoro e si applicano al modello tutte insieme all'arrivo di
//! `Done`, sul thread dell'interfaccia (`data::apply_helper`): sottoalberi innestati, totali dei profili e
//! dei Cestini altrui, copie shadow, bilancio ricalcolato. Un errore o un annullamento prima di `Done` non
//! cambia niente.
//!
//! Nei test `Elevator::fake` consegna risposte preparate: niente prompt UAC, niente processi.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use quado_model::{Flags, Tree};
use quado_scan_win::Problem;
use quado_scan_win::helper::pipe::{PipeError, Server};
use quado_scan_win::helper::proto::{self, Reply, Request};

use super::App;
use super::data::{self, HelperSummary, Loaded};

/// Cosa arriva all'interfaccia da un lavoro dell'helper, in ordine.
#[derive(Debug)]
pub enum HelperEvent {
    /// Prompt UAC accettato e helper collegato alla pipe.
    Connected,
    Reply(Reply),
    /// Fine senza `Done`, o errore prima: l'ultimo evento.
    Failed(HelperFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperFailure {
    /// L'utente ha risposto No al prompt UAC (`ERROR_CANCELLED`).
    Declined,
    /// Avvio non riuscito: codice di Windows.
    Launch(u32),
    Pipe(PipeError),
    /// L'helper è uscito con questo codice prima di `Done` (vedi `helper::serve::exit`).
    Exited(u32),
    /// `quado.exe` sta in Download o in una cartella temporanea, da cui l'helper non parte (05 D3, RS11):
    /// controllo fatto prima della richiesta di Windows, che così non compare.
    Location,
}

impl HelperFailure {
    /// Il motivo è la cartella dell'eseguibile: il controllo prima dell'avvio, o l'helper stesso che esce
    /// con `exit::LOCATION`.
    pub fn is_location(self) -> bool {
        self == Self::Location || self == Self::Exited(quado_scan_win::helper::serve::exit::LOCATION as u32)
    }
}

/// Controlli prima di avviare l'helper, quindi prima della richiesta di Windows: una richiesta che l'helper
/// rifiuterebbe, o un eseguibile in una cartella da cui l'helper non parte (`location_refused`, da
/// `helper::serve::exe_location_refused`), non valgono un prompt UAC.
pub fn precheck(req: &Request, location_refused: bool) -> Result<(), HelperFailure> {
    req.validate().map_err(|e| HelperFailure::Pipe(PipeError::Proto(e)))?;
    if location_refused {
        return Err(HelperFailure::Location);
    }
    Ok(())
}

/// Esegue l'helper per la richiesta e consegna gli eventi su `tx`, l'ultimo è `Reply(Done)` o
/// `Failed`. Blocca fino alla fine: si chiama da un thread di lavoro. `owner`: finestra proprietaria del
/// prompt UAC; `cancel`: annullamento (termina l'helper).
pub type ElevatorFn = Arc<dyn Fn(Request, Option<isize>, Sender<HelperEvent>, Arc<AtomicBool>) + Send + Sync>;

#[derive(Clone)]
pub struct Elevator {
    pub run: ElevatorFn,
}

impl std::fmt::Debug for Elevator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Elevator")
    }
}

/// `ERROR_CANCELLED`: risposta No al prompt UAC.
const ERROR_CANCELLED: u32 = 1223;
/// Attesa della connessione dell'helper dopo il prompt.
pub const ACCEPT_TIMEOUT: Duration = Duration::from_secs(60);
/// Dopo `Done` l'helper esce da solo: se non lo fa entro questo tempo, si termina.
const EXIT_WAIT: Duration = Duration::from_secs(5);
/// Dopo un errore della pipe: il tempo per vedere se l'helper è uscito da solo, con il suo codice.
const ERROR_WAIT: Duration = Duration::from_secs(1);

/// Un passo dell'helper finto (`Elevator::fake`).
#[derive(Debug)]
pub enum FakeStep {
    /// Consegna l'evento.
    Send(HelperEvent),
    /// Si ferma finché l'interfaccia non annulla (al più qualche secondo, per non bloccare un test), poi
    /// consegna `Failed(Pipe(Cancelled))` e finisce.
    UntilCancel,
}

impl Elevator {
    /// Dal vivo: `helper::pipe::Server`, `quado_actions::elevate::launch_helper`, risposte dalla pipe.
    pub fn live() -> Self {
        Self { run: Arc::new(run_live) }
    }

    /// Nessun helper (`Env::fixed`): risponde subito con un errore di avvio.
    pub fn inert() -> Self {
        Self {
            run: Arc::new(|_, _, tx, _| {
                let _ = tx.send(HelperEvent::Failed(HelperFailure::Launch(120)));
            }),
        }
    }

    /// Helper finto per i test: niente prompt UAC e niente processi. A ogni avvio registra la richiesta in
    /// `requests` e consegna i passi in ordine (anche errori).
    pub fn fake(steps: Vec<FakeStep>, requests: Arc<Mutex<Vec<Request>>>) -> Self {
        Self {
            run: Arc::new(move |req, _, tx, cancel| {
                if let Ok(mut r) = requests.lock() {
                    r.push(req);
                }
                for step in &steps {
                    match step {
                        FakeStep::Send(e) => {
                            let _ = tx.send(copy_event(e));
                        }
                        FakeStep::UntilCancel => {
                            let t0 = Instant::now();
                            while !cancel.load(Ordering::Relaxed) && t0.elapsed() < Duration::from_secs(10) {
                                std::thread::sleep(Duration::from_millis(1));
                            }
                            if cancel.load(Ordering::Relaxed) {
                                let _ = tx.send(HelperEvent::Failed(HelperFailure::Pipe(PipeError::Cancelled)));
                                return;
                            }
                        }
                    }
                }
            }),
        }
    }
}

fn copy_event(e: &HelperEvent) -> HelperEvent {
    match e {
        HelperEvent::Connected => HelperEvent::Connected,
        HelperEvent::Reply(r) => HelperEvent::Reply(r.clone()),
        HelperEvent::Failed(f) => HelperEvent::Failed(*f),
    }
}

/// Il lavoro dal vivo, in un thread di lavoro: pipe, prompt UAC (sincrono), connessione, richiesta,
/// risposte fino a `Done`. In errore o annullamento l'helper si termina; se era già uscito da solo, il
/// suo codice di uscita dice di più dell'errore della pipe.
fn run_live(req: Request, owner: Option<isize>, tx: Sender<HelperEvent>, cancel: Arc<AtomicBool>) {
    let fail = |f: HelperFailure| {
        let _ = tx.send(HelperEvent::Failed(f));
    };
    // Prima del prompt UAC: una richiesta che l'helper rifiuterebbe, o un eseguibile in Download o in una
    // cartella temporanea (l'helper uscirebbe con `exit::LOCATION` dopo la conferma).
    if let Err(f) = precheck(&req, quado_scan_win::helper::serve::exe_location_refused()) {
        return fail(f);
    }
    let mut server = match Server::create() {
        Ok(s) => s,
        Err(e) => return fail(HelperFailure::Pipe(e)),
    };
    if cancel.load(Ordering::Relaxed) {
        return fail(HelperFailure::Pipe(PipeError::Cancelled));
    }
    let helper = match quado_actions::elevate::launch_helper(server.name(), owner) {
        Ok(h) => h,
        Err(ERROR_CANCELLED) => return fail(HelperFailure::Declined),
        Err(code) => return fail(HelperFailure::Launch(code)),
    };
    match converse(&mut server, helper.pid(), &req, &tx, &cancel) {
        Ok(()) => {
            if helper.wait(EXIT_WAIT).is_none() {
                helper.terminate();
            }
        }
        Err(e) => {
            let exited = if e == PipeError::Cancelled { None } else { helper.wait(ERROR_WAIT) };
            if exited.is_none() {
                helper.terminate();
            }
            fail(exited.map_or(HelperFailure::Pipe(e), HelperFailure::Exited));
        }
    }
}

/// Connessione, richiesta, `Connected`, poi le risposte fino a `Done`.
fn converse(server: &mut Server, pid: u32, req: &Request, tx: &Sender<HelperEvent>, cancel: &AtomicBool) -> Result<(), PipeError> {
    server.accept(pid, ACCEPT_TIMEOUT, cancel)?;
    server.send_request(req)?;
    let _ = tx.send(HelperEvent::Connected);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(PipeError::Cancelled);
        }
        let r = server.recv(cancel)?;
        let done = matches!(r, Reply::Done { .. });
        // Canale chiuso: l'interfaccia ha annullato e non aspetta più niente.
        if tx.send(HelperEvent::Reply(r)).is_err() && !done {
            return Err(PipeError::Cancelled);
        }
        if done {
            return Ok(());
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Richiesta

/// La richiesta per l'helper e da dove viene.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperRequest {
    pub request: Request,
    /// Nodo di ogni cartella di `request.folders`, nello stesso ordine.
    pub nodes: Vec<u32>,
    /// Il modello di allora (fine della scansione e generazione): se all'arrivo delle risposte è
    /// cambiato, i nodi si ritrovano dal percorso.
    pub finished: i64,
    pub generation: u64,
}

/// Radice di un volume con lettera (`X:\`), l'unica forma che l'helper accetta.
fn is_drive_root(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\'
}

/// Percorso che la validazione dell'helper accetta così com'è (`proto::Request::validate`): sotto la
/// radice, senza caratteri di controllo, `/`, `:` (flussi), componenti vuoti, `.` o `..`.
fn plain_path(p: &str) -> bool {
    let Some(rest) = p.get(3..) else { return false };
    !rest.is_empty() && !rest.chars().any(|c| c.is_control() || c == '/' || c == ':') && rest.split('\\').all(|c| !c.is_empty() && c != "." && c != "..")
}

/// Byte stimati per la parte fissa della richiesta (versione, volume, numero di cartelle).
const REQUEST_HEADER: usize = 64;

/// La richiesta per "Ripeti come amministratore", solo dopo la scansione completa di un volume con
/// lettera: le cartelle negate (`Problem::AccessDenied`, non staccate, non ancora misurate) più alte,
/// nessuna dentro un'altra, in ordine di profondità. Si escludono i percorsi che non si convertono in
/// UTF-8 senza perdita e quelli che l'helper rifiuterebbe; ci si ferma ai limiti di `proto` (numero di
/// cartelle, dimensione della richiesta, stimata per eccesso). `None` se non c'è niente da chiedere.
pub fn build_request(d: &Loaded) -> Option<HelperRequest> {
    if d.cancelled || !is_drive_root(&d.volume.root) {
        return None;
    }
    let t = d.tree();
    if !t.path_lossy(Tree::ROOT).eq_ignore_ascii_case(&d.volume.root) {
        return None;
    }
    let mut request = Request { volume: d.volume.root.clone(), folders: Vec::new() };
    let mut nodes = Vec::new();
    let mut keys: HashSet<String> = HashSet::new();
    let mut bytes = REQUEST_HEADER;
    for &(n, p) in &d.observed.unreadable {
        if p != Problem::AccessDenied || n == Tree::ROOT || t.flags(n).any(Flags::DETACHED | Flags::MEASURED) {
            continue;
        }
        let wide = t.path(n);
        // L'helper riceve testo: un nome con surrogati spaiati non tornerebbe uguale.
        let Ok(path) = String::from_utf16(&wide) else { continue };
        if wide.len() > proto::MAX_PATH_UNITS || !plain_path(&path) {
            continue;
        }
        // Nessuna cartella dentro un'altra, né uguale a un'altra a meno di maiuscole: l'elenco è in
        // ordine di profondità, quindi gli antenati sono già in `keys`.
        let key = path.to_lowercase();
        let nested = key.match_indices('\\').map(|(k, _)| k).filter(|&k| k > 2).any(|k| keys.contains(&key[..k]));
        if nested || keys.contains(&key) {
            continue;
        }
        // UTF-8 o UTF-16, con la lunghezza: la codifica di `proto` non supera nessuna delle due.
        let cost = 4 + path.len().max(2 * wide.len());
        if request.folders.len() == proto::MAX_FOLDERS || bytes + cost > proto::MAX_REQUEST_BYTES {
            break;
        }
        bytes += cost;
        keys.insert(key);
        request.folders.push(path);
        nodes.push(n);
    }
    (!nodes.is_empty()).then_some(HelperRequest { request, nodes, finished: d.finished, generation: d.generation })
}

// ---------------------------------------------------------------------------------------------
// Lavoro in background

/// Fase del lavoro in corso, per la card dei conti.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Prompt di Windows aperto, helper non ancora collegato.
    Consent,
    /// Cartelle con una risposta, su quelle chieste.
    Reading { done: usize, total: usize },
}

/// Come è finito l'ultimo lavoro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// `Done` arrivato: risposte applicate al modello.
    Finished(HelperSummary),
    /// No al prompt di Windows: niente è cambiato.
    Declined,
    /// Annullato dall'utente: niente è cambiato.
    Cancelled,
    /// Errore prima di `Done`: niente è cambiato.
    Failed(HelperFailure),
}

struct Job {
    rx: Receiver<HelperEvent>,
    cancel: Arc<AtomicBool>,
    sent: HelperRequest,
    connected: bool,
    /// Risposte per cartella arrivate (`Folder`, `Totals`, `Bin`, `Failed`).
    answered: usize,
    replies: Vec<Reply>,
}

/// Stato di "Ripeti come amministratore".
#[derive(Default)]
pub struct HelperState {
    job: Option<Job>,
    /// Esito dell'ultimo lavoro e scansione a cui si riferisce (`Loaded::finished`).
    last: Option<(i64, Outcome)>,
    /// Lavori avviati in questa sessione.
    started: usize,
}

impl HelperState {
    pub fn running(&self) -> bool {
        self.job.is_some()
    }

    pub fn phase(&self) -> Option<Phase> {
        let j = self.job.as_ref()?;
        Some(if j.connected || !j.replies.is_empty() { Phase::Reading { done: j.answered, total: j.sent.nodes.len() } } else { Phase::Consent })
    }

    /// Esito dell'ultimo lavoro, qualunque sia la scansione.
    pub fn outcome(&self) -> Option<&Outcome> {
        self.last.as_ref().map(|(_, o)| o)
    }

    /// Esito dell'ultimo lavoro, se riguarda la scansione mostrata.
    pub fn outcome_for(&self, d: &Loaded) -> Option<&Outcome> {
        self.last.as_ref().filter(|(f, _)| *f == d.finished).map(|(_, o)| o)
    }

    /// Lavori avviati in questa sessione (ogni avvio è un prompt di Windows).
    pub fn started(&self) -> usize {
        self.started
    }
}

/// Avvia "Ripeti come amministratore" sulla scansione mostrata: la richiesta e il lavoro dell'helper in
/// un thread. Niente se un lavoro è già in corso o se non c'è niente da chiedere.
pub fn start(app: &mut App) {
    if app.helper.job.is_some() {
        return;
    }
    let Some(d) = app.data.as_ref() else { return };
    let Some(sent) = build_request(d) else { return };
    let finished = d.finished;
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let run = Arc::clone(&app.env.elevator.run);
    let (req, owner, c) = (sent.request.clone(), app.env.owner, Arc::clone(&cancel));
    app.helper.last = None;
    app.helper.started += 1;
    match std::thread::Builder::new().name("quado helper".into()).spawn(move || run(req, owner, tx, c)) {
        Ok(_) => app.helper.job = Some(Job { rx, cancel, sent, connected: false, answered: 0, replies: Vec::new() }),
        Err(e) => {
            // ERROR_NOT_ENOUGH_MEMORY se Windows non dice di più.
            let code = e.raw_os_error().and_then(|c| u32::try_from(c).ok()).unwrap_or(8);
            app.helper.last = Some((finished, Outcome::Failed(HelperFailure::Launch(code))));
        }
    }
}

/// "Annulla": l'helper si termina nel thread di lavoro; le risposte arrivate si scartano.
pub fn cancel(app: &mut App) {
    if let Some(job) = app.helper.job.take() {
        job.cancel.store(true, Ordering::Relaxed);
        let finished = app.data.as_ref().map_or(0, |d| d.finished);
        app.helper.last = Some((finished, Outcome::Cancelled));
    }
}

/// Raccoglie gli eventi del lavoro, a ogni frame; con `Done` applica le risposte al modello.
pub fn poll(app: &mut App, ctx: &egui::Context) {
    let Some(job) = app.helper.job.as_mut() else { return };
    let mut end: Option<Result<(), HelperFailure>> = None;
    loop {
        match job.rx.try_recv() {
            Ok(HelperEvent::Connected) => job.connected = true,
            Ok(HelperEvent::Reply(r)) => {
                let done = matches!(r, Reply::Done { .. });
                if matches!(r, Reply::Folder { .. } | Reply::Totals { .. } | Reply::Bin { .. } | Reply::Failed { .. }) {
                    job.answered += 1;
                }
                job.replies.push(r);
                if done {
                    end = Some(Ok(()));
                    break;
                }
            }
            Ok(HelperEvent::Failed(f)) => {
                end = Some(Err(f));
                break;
            }
            Err(TryRecvError::Empty) => break,
            // Il thread è finito senza l'ultimo evento.
            Err(TryRecvError::Disconnected) => {
                end = Some(Err(HelperFailure::Pipe(PipeError::Closed)));
                break;
            }
        }
    }
    match end {
        None => ctx.request_repaint_after(Duration::from_millis(50)),
        Some(Ok(())) => {
            if let Some(job) = app.helper.job.take() {
                finish(app, job);
            }
            ctx.request_repaint();
        }
        Some(Err(f)) => {
            app.helper.job = None;
            let outcome = match f {
                HelperFailure::Declined => Outcome::Declined,
                HelperFailure::Pipe(PipeError::Cancelled) => Outcome::Cancelled,
                f => Outcome::Failed(f),
            };
            let finished = app.data.as_ref().map_or(0, |d| d.finished);
            app.helper.last = Some((finished, outcome));
            ctx.request_repaint();
        }
    }
}

/// `Done`: le risposte nel modello, poi le viste che dipendono dagli indici dei nodi.
fn finish(app: &mut App, job: Job) {
    // Nessun lavoro in background tiene il modello mentre lo si modifica.
    app.wait_background();
    let sid = app.env.user_sid.clone();
    let Some(d) = app.data.as_mut() else {
        let replies = job.replies.iter().filter(|r| matches!(r, Reply::Folder { .. } | Reply::Totals { .. } | Reply::Bin { .. })).count();
        app.helper.last = Some((0, Outcome::Finished(HelperSummary { asked: job.sent.nodes.len(), ignored: replies, ..Default::default() })));
        return;
    };
    let summary = data::apply_helper(d, &job.sent, job.replies, sid.as_deref());
    // La selezione resta se il nodo c'è ancora; una cartella innestata passa alla sua nuova radice.
    let t = d.tree();
    let alive = |n: u32| (n as usize) < t.len() && !t.flags(n).has(Flags::DETACHED);
    let root = app.explore.outline.root;
    let sel = match app.explore.selected {
        Some(n) if alive(n) => n,
        Some(n) => summary.grafted.iter().find(|&&(old, _)| old == n).map_or(if alive(root) { root } else { Tree::ROOT }, |&(_, new)| new),
        None if alive(root) => root,
        None => Tree::ROOT,
    };
    app.explore.after_model_change(d, sel);
    app.search.invalidate();
    app.big.invalidate();
    app.helper.last = Some((d.finished, Outcome::Finished(summary)));
}

/// Codice di Windows come si scrive nei messaggi: decimale, esadecimale se è un HRESULT.
pub fn code_text(code: u32) -> String {
    if code >= 0x8000_0000 { format!("0x{code:08X}") } else { code.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_the_helper_accepts() {
        assert!(is_drive_root(r"C:\") && is_drive_root(r"r:\"));
        assert!(!is_drive_root(r"C:") && !is_drive_root(r"C:\x") && !is_drive_root(r"\\?\C:\"));
        assert!(plain_path(r"C:\Users\ospite"));
        assert!(plain_path(r"C:\Program Files\WindowsApps"));
        for bad in [r"C:\", r"C:\a\\b", r"C:\a\.\b", r"C:\a\..", r"C:\a/b", r"C:\a:flusso", "C:\\a\u{1}b", r"C:\a\"] {
            assert!(!plain_path(bad), "{bad:?}");
        }
    }

    #[test]
    fn codes_in_messages() {
        assert_eq!(code_text(5), "5");
        assert_eq!(code_text(0x8004_1003), "0x80041003");
    }

    #[test]
    fn the_location_is_checked_before_the_windows_prompt() {
        // `run_live` esce con l'esito di `precheck` prima di creare la pipe e di avviare l'helper: con
        // l'eseguibile in Download o in una cartella temporanea la richiesta di Windows non compare.
        let ok = Request { volume: r"C:\".into(), folders: vec![r"C:\Users\ospite".into()] };
        assert_eq!(precheck(&ok, false), Ok(()));
        assert_eq!(precheck(&ok, true), Err(HelperFailure::Location));
        // Una richiesta non valida resta il primo motivo.
        let bad = Request { volume: "C:".into(), folders: Vec::new() };
        assert!(matches!(precheck(&bad, true), Err(HelperFailure::Pipe(PipeError::Proto(_)))));
        // Lo stesso motivo se l'helper parte e rifiuta la cartella da solo (codice di uscita 3).
        assert!(HelperFailure::Location.is_location());
        assert!(HelperFailure::Exited(3).is_location());
        assert!(!HelperFailure::Exited(4).is_location() && !HelperFailure::Launch(3).is_location());
    }
}
