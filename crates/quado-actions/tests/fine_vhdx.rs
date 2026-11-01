// SPDX-License-Identifier: GPL-3.0-or-later
//! Prove di fine di M5 sul VHDX di prova NTFS (`R:`): la catena intera, dalla simulazione al registro,
//! con le letture e lo spostamento veri (06 §5.3, §5.4, §6.1, §6.2; 03 F-AC-5, F-AC-6, F-AC-7, F-AC-8,
//! F-VE-1, F-VE-2, F-VE-3, F-VE-4; 07 §1.6 criterio 3).
//!
//! Catena di ogni prova: `simulate` con `WinProbe` (Cestino e Sensore memoria letti da HKCU come
//! `KbConfig` dell'interfaccia, riferimenti vuoti) → `plan::conferma` → misura di partenza →
//! `execute::esegui` con `ShellMover` → misura dopo l'azione → svuotamento del Cestino di `R:` con
//! `quado_testkit::bin::empty_test_volume` (per la misura equivale al dialogo di Windows che usa il
//! prodotto) → misura dopo lo svuotamento → riscansione dei sottoalberi toccati → `verification::compare`
//! → `journal::append` e `journal::read` in una cartella temporanea.
//!
//! Sicurezza: si scrive solo in `R:\interventi-prove-f` e nelle cartelle temporanee dei test; il Cestino
//! di `R:` si svuota all'inizio e alla fine di ogni prova (`empty_test_volume` rifiuta ogni altro volume);
//! capacità e `NukeOnDelete` si ripristinano alla fine. Ogni spostamento gira in un thread sorvegliato:
//! una finestra di Windows di questo processo si chiude (cioè "No" o "Annulla", niente si elimina) e la
//! prova la riporta. Si eseguono con
//! `cargo test -p quado-actions --test fine_vhdx -- --ignored --test-threads=1`.
//!
//! # Tolleranza di 06 §6.1 (07 §1.6 criterio 3)
//!
//! Misure del 14/10/2026 su `R:` (VHDX da 4 GiB, NTFS, cluster da 4 KiB), tre ripetizioni per scenario,
//! uguali al byte nelle tre (a parte il rumore descritto sotto). "Differenza" è la variazione del volume
//! dopo lo svuotamento meno la stima; "dopo lo spostamento" è la variazione prima dello svuotamento. Byte.
//! Primo giro (stima senza gli indici delle cartelle, uso dei file con Restart Manager):
//!
//! | Scenario | Stima | Differenza | Dopo lo spostamento |
//! |---|---|---|---|
//! | File normali | 50.065.408 | 0 | 0 |
//! | Hard link, tutti i nomi nella selezione (4 nomi) | 19.922.944 | 0 | 0 |
//! | Hard link, un nome fuori (16 MiB esclusi e citati) | 6.291.456 | 0 | 0 |
//! | Compressi NTFS | 24.641.536 | 0 | 0 |
//! | Compressi WOF | 14.397.440 | +10.768.384: Restart Manager li decomprimeva | 0 |
//! | Residenti nella MFT (4.000 file, più un file normale) | 8.388.608 | +1.048.576, l'indice della cartella | 0 |
//! | Sparse (256 MiB logici) | 6.356.992 | +131.072 | 0 |
//! | Cartella non elencabile (intervallo) | 6.291.456-18.874.368 | 0 (variazione 14.680.064) | 0 |
//! | Segnaposto cloud (più un file locale) | 3.145.728 | 0 | 0 |
//! | File aperto e chiuso prima del controllo | 2.097.152 | 0 | 0 |
//! | Molte cartelle, come `node_modules` (361 cartelle, 12.420 file) | 72.572.928 | +2.207.744 (3,04%), gli indici delle cartelle | 0 |
//!
//! Conclusione: la tolleranza iniziale (`verification::within_tolerance`, 1% della stima più 1 MiB) è
//! confermata, con margine: per i file la differenza è 0, tranne i file sparse (128 KiB). Quello che la
//! stima non vedeva erano gli indici delle cartelle ($I30): una cartella con migliaia di voci ne ha uno
//! non residente (1 MiB con 4.001 voci) e un albero con molte cartelle li somma (3,04% sopra, fuori
//! tolleranza). Correzione del 14/10/2026: la stima conta l'allocata di ogni cartella (`Attrs::alloc` di
//! una cartella è quella del suo indice), e la tolleranza resta com'è; dopo la correzione la differenza
//! di "molte cartelle" è 0. I file WOF e i segnaposto si provano con la lettura dell'uso attuale, che non
//! li tocca. Su un volume di sistema vero gli indici pesano lo 0,08-0,10% (misura di M5, `cargo xtask m5-misure`).
//!
//! Rumore: NTFS libera una parte dell'indice di una cartella grande circa 4 s dopo l'eliminazione (512 KiB
//! su 1 MiB, misurato su R:). Le prove aspettano 6 s di spazio libero stabile prima della misura di
//! partenza e dopo lo svuotamento; con mezzo secondo o due secondi di attesa 512 KiB comparivano nella
//! misura sbagliata (anche nella prova successiva), sempre dentro la parte fissa della tolleranza.
//!
//! Scenari non provati qui: disco virtuale dentro il VHDX. Montare un VHDX (`AttachVirtualDisk`) richiede
//! l'amministratore e le prove girano da utente standard; in più la simulazione non ha un caso proprio per
//! il contenuto di un disco virtuale: un elemento dentro un VHDX montato sta su un altro volume, e stima e
//! misure sono di quel volume, non di quello che contiene il file del disco virtuale (03 §6.3, "0 fino
//! alla compattazione", resta da progettare). I segnaposto cloud si provano su `R:` con `TMP` puntato
//! nell'area (`segnaposto_cloud`).
//!
//! # Errori del prodotto trovati e corretti
//!
//! Restart Manager, che la prima versione di `restart::processes_using` usava nella simulazione e nel
//! ricontrollo delle cartelle, non era una lettura innocua per due tipi di file: decomprimeva i file
//! compressi WOF (`stima_compressi_wof`) e chiedeva al provider i dati dei segnaposto cloud
//! (`segnaposto_cloud`). Corretto il 14/10/2026: per quei file l'uso si chiede direttamente a NTFS con un
//! handle dei soli attributi (`FileProcessIdsUsingFileInformation`), e le due prove lo verificano.

use std::cell::RefCell;
use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime};

use quado_actions::execute::{ExecProgress, ItemResult, MoveResult, Mover, Riepilogo, esegui};
use quado_actions::fake;
use quado_actions::journal;
use quado_actions::measure::measure;
use quado_actions::mover::{ShellMover, Trace};
use quado_actions::plan::{Block, CapacityCheck, ManualCheck, Rifiuto, SelectionEstimate, Simulazione, conferma};
use quado_actions::probe::{Attrs, Entry, ListError, Probe, SystemConfig, VolumeBin};
use quado_actions::restart::{ProcessUse, RmError};
use quado_actions::simulate::{Request, RequestItem, SimProgress, simulate};
use quado_actions::win_probe::WinProbe;
use quado_kb::extract::{self, ExtractError};
use quado_model::intervention::{
    Comparison, Explanation, InterventionKind, ItemOutcome, JournalEntry, JournalItem, Measure, MeasurePoint, ReferenceReport,
};
use quado_model::record::{Action, CheckOutcome, Estimate, Record, State};
use quado_model::verification::{ExplainFacts, TOLERANCE_FIXED, compare, within_tolerance};
use quado_model::{Flags, Tree};
use quado_scan_win::{Options as ScanOptions, Progress, RootError, scan};
use quado_testkit::bin::{self, BinSetting};
use quado_testkit::trees::{self, TestDir};

/// Cartella delle prove su `R:`.
const AREA: &str = "interventi-prove-f";
/// Capacità del Cestino di `R:` per gli scenari di 06 §6.1 e §6.2, in MiB.
const CAPACITY_MIB: u32 = 1024;
/// Capacità ridotta per le prove di 06 §5.3, in MiB.
const SMALL_MIB: u32 = 32;
/// Ripetizioni di ogni scenario di 06 §6.1.
const REPS: usize = 3;
/// Tempo massimo di uno spostamento sorvegliato (un segnaposto cloud può aspettare il provider 60 s).
const MOVE_DEADLINE: Duration = Duration::from_secs(150);

const MIB: u64 = 1 << 20;

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}

// ---------------------------------------------------------------------------------------------
// Configurazione e letture

/// Codice di Windows di un errore degli estrattori, come `KbConfig`.
fn extract_code(e: ExtractError) -> u32 {
    match e {
        ExtractError::Io(c) => c,
        ExtractError::NotPresent => 2,
        _ => 13,
    }
}

/// Le letture di `quado-kb` come le fa `KbConfig` dell'interfaccia (`quado-ui`, `intervention_jobs.rs`),
/// che questo crate non vede: capacità e `NukeOnDelete` del volume e Sensore memoria da HKCU, in sola
/// lettura. Riferimenti vuoti: le prove dei riferimenti sono di 06 §5.5.
struct HkcuConfig;

impl SystemConfig for HkcuConfig {
    fn bin_settings(&self, root: &str) -> Result<(Option<u64>, bool), u32> {
        let guid = quado_kb::win::volume_guid(root).ok_or(2u32)?;
        match extract::bin_settings(&quado_kb::win::LiveRegistry, &guid) {
            Ok(b) => Ok((b.max_capacity_mib.map(|m| u64::from(m) << 20), b.nuke_on_delete)),
            Err(ExtractError::NotPresent) => Ok((None, false)),
            Err(e) => Err(extract_code(e)),
        }
    }
    fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
        let s = extract::storage_sense(&quado_kb::win::LiveRegistry).map_err(extract_code)?;
        Ok(if s.enabled { s.recycle_days } else { None })
    }
    fn references(&self, _: &[String]) -> ReferenceReport {
        ReferenceReport::default()
    }
}

/// `WinProbe` che registra i file passati a Restart Manager, chiamata per chiamata (06 §5.4, cartella
/// grande). Tutto il resto passa invariato.
struct Recording<'a> {
    inner: &'a WinProbe,
    rm: Mutex<Vec<Vec<String>>>,
}

impl<'a> Recording<'a> {
    fn new(inner: &'a WinProbe) -> Self {
        Self { inner, rm: Mutex::new(Vec::new()) }
    }
}

impl Probe for Recording<'_> {
    fn attributes(&self, path: &str) -> Result<Attrs, u32> {
        self.inner.attributes(path)
    }
    fn children(&self, dir: &str) -> Result<Vec<String>, u32> {
        self.inner.children(dir)
    }
    fn entries(&self, dir: &str, step: &dyn Fn() -> bool) -> Result<Vec<Entry>, ListError> {
        self.inner.entries(dir, step)
    }
    fn threads(&self) -> usize {
        self.inner.threads()
    }
    fn processes_using(&self, paths: &[String]) -> Result<Vec<ProcessUse>, RmError> {
        self.rm.lock().unwrap().push(paths.to_vec());
        self.inner.processes_using(paths)
    }
    fn references(&self, paths: &[String]) -> ReferenceReport {
        self.inner.references(paths)
    }
    fn volume_bin(&self, path: &str) -> Result<VolumeBin, u32> {
        self.inner.volume_bin(path)
    }
    fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
        self.inner.storage_sense_days()
    }
    fn sync_root(&self, path: &str) -> Option<String> {
        self.inner.sync_root(path)
    }
    fn free_space(&self, path: &str) -> Result<u64, u32> {
        self.inner.free_space(path)
    }
    fn now(&self) -> i64 {
        self.inner.now()
    }
}

/// Record di prova che propone lo spostamento nel Cestino, con i controlli automatici di Restart
/// Manager e della capacità del Cestino.
fn record() -> Record {
    let mut r = fake::record(State::Manageable(fake::proof()));
    r.checks = vec![fake::automatic("controllo-restart-manager"), fake::automatic("controllo-capacita-cestino")];
    r
}

fn request(paths: &[String]) -> Request {
    Request { items: paths.iter().map(|p| RequestItem { path: p.clone(), fixed: None }).collect(), record: record(), protected: Vec::new() }
}

// ---------------------------------------------------------------------------------------------
// Area di prova

/// `R:\interventi-prove-f`, con il Cestino di `R:` vuoto e la capacità impostata. Alla fine si tolgono le
/// negazioni messe dalle prove, si cancella la cartella, si svuota il Cestino di `R:` e si ripristina la
/// capacità.
struct Area {
    root: PathBuf,
    dir: PathBuf,
    sid: String,
    probe: WinProbe,
    _capacity: BinSetting,
}

impl Area {
    fn new(capacity_mib: u32) -> Self {
        let root = trees::mounted_vhdx()
            .into_iter()
            .find(|(n, _)| n == "ntfs")
            .map(|(_, p)| p)
            .expect("VHDX di prova NTFS montato (cargo xtask vhdx crea)");
        assert!(s(&root).eq_ignore_ascii_case(r"R:\"), "il VHDX di prova NTFS non è R:\\ ma {}", root.display());
        let sid = quado_scan_win::system::user_sid().expect("SID dell'utente");
        let dir = root.join(AREA);
        release(&dir, &sid);
        for (p, _) in our_bin_items(&root, &dir) {
            release(&p, &sid);
        }
        let _ = fs::remove_dir_all(&dir);
        bin::empty_test_volume(&root);
        fs::create_dir_all(&dir).unwrap();
        let capacity = BinSetting::capacity(&root, capacity_mib);
        Self { root, dir, sid, probe: WinProbe::new(Box::new(HkcuConfig)), _capacity: capacity }
    }

    /// Cartella nuova per una ripetizione di uno scenario.
    fn scenario(&self, name: &str, k: usize) -> PathBuf {
        let d = self.dir.join(format!("{name}-{k}"));
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn attrs(&self, p: &Path) -> Attrs {
        self.probe.attributes(&s(p)).unwrap_or_else(|e| panic!("{}: errore {e}", p.display()))
    }

    fn bin(&self) -> quado_model::intervention::BinCount {
        self.probe.volume_bin(&s(&self.dir)).unwrap().content.expect("contenuto del Cestino di R:")
    }

    /// Voci della cartella del Cestino dell'utente su `R:`: (nome, cartella, byte).
    fn bin_entries(&self) -> Vec<(String, bool, u64)> {
        let Some(d) = bin::user_bin_dir(&self.root) else { return Vec::new() };
        let mut v: Vec<_> = fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| {
                let m = e.metadata().unwrap();
                (e.file_name().to_string_lossy().into_owned(), m.is_dir(), m.len())
            })
            .collect();
        v.sort();
        v
    }

    /// Nega l'elenco di `dir` all'utente (ACL di negazione, come `TestDir::deny_listing`); falso se Windows
    /// non lo permette.
    fn deny_listing(&self, dir: &Path) -> bool {
        Command::new("icacls").arg(dir).args(["/deny", &format!("*{}:(RX)", self.sid)]).output().is_ok_and(|o| o.status.success())
    }
}

impl Drop for Area {
    fn drop(&mut self) {
        // Prima le negazioni: senza, né la cartella né il Cestino si svuotano del tutto.
        release(&self.dir, &self.sid);
        for (p, _) in our_bin_items(&self.root, &self.dir) {
            release(&p, &self.sid);
        }
        let _ = fs::remove_dir_all(&self.dir);
        bin::empty_test_volume(&self.root);
    }
}

/// Toglie le negazioni dell'utente da `path` e da tutto ciò che contiene.
fn release(path: &Path, sid: &str) {
    if path.exists() {
        let _ = Command::new("icacls").arg(path).args(["/remove:d", &format!("*{sid}"), "/T", "/C", "/Q"]).output();
    }
}

/// Percorso originale di un elemento del Cestino dal suo file `$I` (Windows 10: versione 2, intestazione
/// di 24 byte, lunghezza in caratteri, percorso UTF-16; versione 1: percorso fisso di 260 caratteri).
fn original_path(i_file: &[u8]) -> Option<String> {
    let version = i64::from_le_bytes(i_file.get(0..8)?.try_into().ok()?);
    let raw = match version {
        2 => {
            let n = u32::from_le_bytes(i_file.get(24..28)?.try_into().ok()?) as usize;
            i_file.get(28..28 + 2 * n)?
        }
        1 => i_file.get(24..24 + 520)?,
        _ => return None,
    };
    let w: Vec<u16> = raw.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).take_while(|&c| c != 0).collect();
    Some(String::from_utf16_lossy(&w))
}

/// Elementi del Cestino di `R:` che venivano da `under`: (`$R…` nel Cestino, percorso originale).
fn our_bin_items(root: &Path, under: &Path) -> Vec<(PathBuf, String)> {
    let Some(d) = bin::user_bin_dir(root) else { return Vec::new() };
    let prefix = s(under).to_lowercase();
    let Ok(rd) = fs::read_dir(&d) else { return Vec::new() };
    rd.flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let rest = name.strip_prefix("$I")?;
            let original = original_path(&fs::read(e.path()).ok()?)?;
            original.to_lowercase().starts_with(&prefix).then(|| (d.join(format!("$R{rest}")), original))
        })
        .collect()
}

/// Stabilità chiesta allo spazio libero prima della misura di partenza e dopo uno svuotamento: NTFS libera
/// una parte dell'indice di una cartella grande circa 4 s dopo l'eliminazione (misurato su R:: cartella con
/// 4.000 file, 512 KiB subito e 512 KiB dopo 4,2 s).
const AFTER_DELETE: Duration = Duration::from_secs(6);
/// Stabilità chiesta dopo uno spostamento, che non libera niente.
const AFTER_MOVE: Duration = Duration::from_secs(1);

/// Aspetta che lo spazio libero di `R:` resti uguale per `window` (al più 30 s), così la misura non
/// prende a metà le liberazioni ritardate di NTFS.
fn settle(probe: &WinProbe, path: &str, window: Duration) {
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut last = probe.free_space(path).expect("spazio libero");
    let mut since = Instant::now();
    while since.elapsed() < window {
        std::thread::sleep(Duration::from_millis(100));
        let now = probe.free_space(path).expect("spazio libero");
        if now != last {
            (last, since) = (now, Instant::now());
        }
        if Instant::now() > deadline {
            eprintln!("spazio libero di R: non stabile entro 30 s");
            return;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Sorveglianza delle finestre di Windows (come `cestino_vhdx.rs`, solo chiusura)

/// Una finestra visibile di primo livello di questo processo, con i controlli che hanno un testo.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Window {
    hwnd: usize,
    class: String,
    title: String,
    controls: Vec<(usize, i32, String)>,
}

fn window_text(hwnd: windows_sys::Win32::Foundation::HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let mut buf = [0u16; 512];
    // SAFETY: buffer locale della lunghezza dichiarata; la finestra può essere già chiusa (risponde 0).
    let n = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn class_name(hwnd: windows_sys::Win32::Foundation::HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW;
    let mut buf = [0u16; 256];
    // SAFETY: come `window_text`.
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

unsafe extern "system" fn collect_top(hwnd: windows_sys::Win32::Foundation::HWND, lparam: isize) -> windows_sys::core::BOOL {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindowVisible};
    let mut pid = 0u32;
    // SAFETY: `hwnd` arriva da EnumWindows; `lparam` è il puntatore al Vec di `own_windows`, vivo per
    // tutta l'enumerazione e usato solo da questo thread.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == std::process::id() && IsWindowVisible(hwnd) != 0 {
            (*(lparam as *mut Vec<usize>)).push(hwnd as usize);
        }
    }
    1
}

unsafe extern "system" fn collect_child(hwnd: windows_sys::Win32::Foundation::HWND, lparam: isize) -> windows_sys::core::BOOL {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetDlgCtrlID;
    let t = window_text(hwnd);
    if !t.trim().is_empty() {
        // SAFETY: `hwnd` arriva da EnumChildWindows; `lparam` è il puntatore al Vec di `own_windows`, vivo
        // per tutta l'enumerazione e usato solo da questo thread.
        unsafe { (*(lparam as *mut Vec<(usize, i32, String)>)).push((hwnd as usize, GetDlgCtrlID(hwnd), t)) };
    }
    1
}

/// Finestre visibili di primo livello di questo processo (i dialoghi di `IFileOperation`).
fn own_windows() -> Vec<Window> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumChildWindows, EnumWindows};
    let mut handles: Vec<usize> = Vec::new();
    // SAFETY: il callback riceve il puntatore a `handles`, che vive fino alla fine della chiamata.
    unsafe { EnumWindows(Some(collect_top), (&raw mut handles) as isize) };
    handles
        .into_iter()
        .map(|h| {
            let hwnd = h as windows_sys::Win32::Foundation::HWND;
            let mut controls: Vec<(usize, i32, String)> = Vec::new();
            // SAFETY: come sopra, con `controls`.
            unsafe { EnumChildWindows(hwnd, Some(collect_child), (&raw mut controls) as isize) };
            Window { hwnd: h, class: class_name(hwnd), title: window_text(hwnd), controls }
        })
        .collect()
}

/// Sposta `path` con `ShellMover` in un thread a parte e chiude ogni finestra di Windows di questo
/// processo che compare (`WM_CLOSE`: "No" o "Annulla").
fn recycle_watched(path: &str) -> (Trace, Vec<Window>) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let p = path.to_owned();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(ShellMover::new(None).recycle_traced(&p));
    });
    let deadline = Instant::now() + MOVE_DEADLINE;
    let mut windows: Vec<Window> = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(trace) => return (trace, windows),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => panic!("il thread dello spostamento è terminato senza esito"),
        }
        for w in own_windows() {
            if !windows.iter().any(|x| x.hwnd == w.hwnd) {
                eprintln!("finestra di Windows durante lo spostamento di {path}: {w:?}");
                windows.push(w.clone());
            }
            // SAFETY: messaggio asincrono a una finestra di questo processo; se è già chiusa non succede niente.
            unsafe { PostMessageW(w.hwnd as windows_sys::Win32::Foundation::HWND, WM_CLOSE, 0, 0) };
        }
        assert!(Instant::now() < deadline, "IFileOperation non ha finito entro {MOVE_DEADLINE:?}; finestre: {windows:?}");
    }
}

/// Il mover dell'esecuzione: `ShellMover` sorvegliato, con il registro delle chiamate (percorso e ora) e
/// delle finestre comparse.
#[derive(Default)]
struct WatchedMover {
    calls: RefCell<Vec<(String, i64)>>,
    windows: RefCell<Vec<Window>>,
    traces: RefCell<Vec<Trace>>,
}

impl Mover for WatchedMover {
    fn recycle(&self, path: &str) -> MoveResult {
        self.calls.borrow_mut().push((path.to_owned(), quado_scan_win::system::now_filetime()));
        let (trace, windows) = recycle_watched(path);
        self.windows.borrow_mut().extend(windows);
        let r = trace.result;
        self.traces.borrow_mut().push(trace);
        r
    }
}

// ---------------------------------------------------------------------------------------------
// Riscansione dei sottoalberi toccati (come `intervention_jobs` dell'interfaccia)

/// Chiave di confronto dei percorsi: `/` come `\`, senza `\` finale, minuscole.
fn key(p: &str) -> String {
    let p = p.strip_prefix(r"\\?\").unwrap_or(p).replace('/', "\\");
    p.trim_end_matches('\\').to_lowercase()
}

/// Nodo con questo percorso assoluto, come `intervention_jobs::find_node`.
fn find_node(tree: &Tree, path: &str) -> Option<u32> {
    let root = key(&tree.path_lossy(Tree::ROOT));
    let p = key(path);
    if p == root {
        return Some(Tree::ROOT);
    }
    let rest = p.strip_prefix(&root)?.strip_prefix('\\')?;
    let mut n = Tree::ROOT;
    for comp in rest.split('\\').filter(|c| !c.is_empty()) {
        n = tree.children(n).find(|&c| !tree.flags(c).has(Flags::DETACHED) && tree.name_lossy(c).to_lowercase() == comp)?;
    }
    Some(n)
}

/// Cartelle da riscansionare, come `intervention_jobs::rescan_dirs`: la cartella di ogni elemento più
/// `extra`, senza doppioni e senza quelle dentro un'altra della lista.
fn rescan_dirs(items: &[String], extra: &[String]) -> Vec<String> {
    let mut dirs: Vec<String> = items
        .iter()
        .filter_map(|p| {
            let t = p.trim_end_matches('\\');
            t.rfind('\\').map(|k| if k <= 2 { t[..=k].to_owned() } else { t[..k].to_owned() })
        })
        .chain(extra.iter().cloned())
        .collect();
    dirs.sort_by_key(|d| key(d));
    dirs.dedup_by_key(|d| key(d));
    let keys: Vec<String> = dirs.iter().map(|d| key(d)).collect();
    dirs.into_iter()
        .zip(&keys)
        .filter(|(_, k)| !keys.iter().any(|o| o != *k && k.starts_with(o.as_str()) && k.as_bytes().get(o.len()) == Some(&b'\\')))
        .map(|(d, _)| d)
        .collect()
}

fn scan_tree(root: &str) -> Result<Tree, u32> {
    scan(root, ScanOptions::default(), &Progress::default()).map(|r| r.tree).map_err(|RootError(e)| e)
}

/// Byte allocati che gli elementi occupano ancora ai loro percorsi, dalla riscansione delle cartelle
/// che li contengono (F-VE-2, `Measure::still_present`).
fn still_present(paths: &[String]) -> u64 {
    let mut total = 0;
    for dir in rescan_dirs(paths, &[]) {
        let Ok(t) = scan_tree(&dir) else { continue };
        total += paths.iter().filter_map(|p| find_node(&t, p)).map(|n| t.alloc(n)).sum::<u64>();
    }
    total
}

// ---------------------------------------------------------------------------------------------
// Catena completa

/// Momenti della catena in cui una prova può intervenire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Subito dopo la misura di partenza.
    Started,
    /// Subito dopo l'esecuzione, prima della misura dopo l'azione.
    Executed,
}

/// Fatti per `compare` a una misura (03 §6.3): elementi rimasti al loro posto dal riepilogo, nomi esclusi
/// dalla simulazione, byte della stima ancora nel Cestino. I byte spostati sono quelli della stima, non
/// la somma di `ItemResult::alloc`, che comprende anche i file esclusi perché hanno altri nomi fuori.
fn facts(r: &Riepilogo, est: &SelectionEstimate, last: &Measure) -> ExplainFacts {
    let left: Vec<&ItemResult> =
        r.items.iter().filter(|i| matches!(i.outcome, ItemOutcome::Skipped(_) | ItemOutcome::Failed(_) | ItemOutcome::NotReached)).collect();
    let all_recycled = r.items.iter().all(|i| i.outcome == ItemOutcome::Recycled);
    let estimated = match est.estimate {
        Estimate::Exact(e) => e,
        Estimate::Range { max, .. } => max,
        Estimate::NotEstimable(_) => 0,
    };
    let recycled = if all_recycled {
        estimated
    } else {
        r.items.iter().filter(|i| i.outcome == ItemOutcome::Recycled).map(|i| i.alloc).sum::<u64>().min(estimated)
    };
    let in_bin = match last.bin {
        Some(b) if b.items == 0 => 0,
        _ => recycled,
    };
    ExplainFacts {
        skipped_or_failed: (left.len() as u64, left.iter().map(|i| i.alloc).sum()),
        excluded_names: est.excluded_names,
        recycled_bytes: recycled,
        still_in_bin: Some(in_bin),
        still_open_files: 0,
    }
}

/// Tutto quello che una catena ha prodotto.
struct Chain {
    sim: Simulazione,
    estimate: SelectionEstimate,
    riepilogo: Riepilogo,
    /// Partenza, dopo l'azione, dopo lo svuotamento.
    measures: Vec<Measure>,
    at_action: Comparison,
    at_empty: Comparison,
    mover: WatchedMover,
    items: Vec<String>,
}

impl Chain {
    fn obtained_action(&self) -> i64 {
        self.measures[1].free as i64 - self.measures[0].free as i64
    }

    fn obtained_empty(&self) -> i64 {
        self.measures[2].free as i64 - self.measures[0].free as i64
    }

    fn exact(&self) -> u64 {
        match self.estimate.estimate {
            Estimate::Exact(e) => e,
            e => panic!("stima non esatta: {e:?}"),
        }
    }

    fn assert_all_recycled(&self) {
        let w = self.mover.windows.borrow();
        assert!(w.is_empty(), "nessuna finestra di Windows attesa: {w:?}");
        for it in &self.riepilogo.items {
            assert_eq!(it.outcome, ItemOutcome::Recycled, "{}", it.path);
            assert!(!Path::new(&it.path).exists(), "{} è ancora al suo posto", it.path);
        }
        assert!(!self.riepilogo.stopped);
        assert_eq!(self.measures[1].still_present, Some(0), "la riscansione trova ancora la selezione");
        assert_eq!(self.measures[2].still_present, Some(0));
    }

    /// Riga di misura per il resoconto: stima, variazione dopo lo spostamento, dopo lo svuotamento e la
    /// differenza dalla stima (o dall'intervallo).
    fn report(&self, scenario: &str, k: usize) {
        let (a, b) = (self.obtained_action(), self.obtained_empty());
        let (diff, what) = match self.estimate.estimate {
            Estimate::Exact(e) => (b - e as i64, format!("esatta {e}")),
            Estimate::Range { min, max } => {
                let d = if b < min as i64 {
                    b - min as i64
                } else if b > max as i64 {
                    b - max as i64
                } else {
                    0
                };
                (d, format!("intervallo {min}-{max}"))
            }
            Estimate::NotEstimable(n) => (0, format!("non stimabile {n:?}")),
        };
        eprintln!(
            "MISURA {scenario} #{}: stima {what}; variazione dopo lo spostamento {a}; dopo lo svuotamento {b}; differenza {diff} ({:.4}%); confronto {:?}",
            k + 1,
            if b != 0 { diff as f64 * 100.0 / b as f64 } else { 0.0 },
            self.at_empty
        );
    }

    /// Criteri comuni di 06 §6.1 per una stima esatta: subito dopo lo spostamento la variazione è circa 0
    /// e il confronto la spiega con il contenuto ancora nel Cestino; dopo lo svuotamento la variazione è
    /// entro la tolleranza dalla stima.
    fn assert_exact_within_tolerance(&self, scenario: &str, k: usize) {
        self.report(scenario, k);
        let e = self.exact();
        assert!(self.obtained_action().unsigned_abs() <= TOLERANCE_FIXED, "{scenario}: subito dopo lo spostamento la variazione è {}", self.obtained_action());
        if e > 0 {
            assert!(self.at_action.explanations.contains(&Explanation::StillInBin { bytes: e }), "{scenario}: {:?}", self.at_action);
            assert_eq!(self.at_action.unexplained, None, "{scenario}: {:?}", self.at_action);
        }
        assert!(within_tolerance(e, self.obtained_empty()), "{scenario}: stima {e}, variazione {}", self.obtained_empty());
        assert_eq!(self.at_empty.unexplained, None);
    }
}

/// Esegue la catena completa su `items`. Con `tick` spunta tutti i controlli manuali (cartelle di
/// sincronizzazione). `hook` interviene nei momenti di [`Phase`].
fn chain(area: &Area, items: &[PathBuf], tick: bool, hook: &mut dyn FnMut(Phase)) -> Chain {
    let probe = &area.probe;
    let paths: Vec<String> = items.iter().map(|p| s(p)).collect();
    let sim = simulate(request(&paths), probe, &SimProgress::default()).expect("simulazione non annullata");
    let mut scelta = sim.default_choice();
    if tick {
        for t in &mut scelta.ticked_items {
            t.iter_mut().for_each(|x| *x = true);
        }
        scelta.ticked_rule.iter_mut().for_each(|x| *x = true);
    }
    let piano = match conferma(&sim, &scelta) {
        Ok(p) => p,
        Err(r) => panic!("piano non confermato: {r:?}; elementi {:#?}", sim.items()),
    };
    let estimate = *piano.estimate();
    let at = s(&area.dir);
    settle(&area.probe, &at, AFTER_DELETE);
    let start = measure(probe, &at, MeasurePoint::Start).expect("misura di partenza");
    hook(Phase::Started);
    let mover = WatchedMover::default();
    let riepilogo = esegui(piano, probe, &mover, &ExecProgress::default());
    hook(Phase::Executed);
    settle(&area.probe, &at, AFTER_MOVE);
    let mut after_action = measure(probe, &at, MeasurePoint::AfterAction).expect("misura dopo l'azione");
    after_action.still_present = Some(still_present(&paths));
    bin::empty_test_volume(&area.root);
    settle(&area.probe, &at, AFTER_DELETE);
    let mut after_empty = measure(probe, &at, MeasurePoint::AfterEmpty).expect("misura dopo lo svuotamento");
    after_empty.still_present = Some(still_present(&paths));
    let measures = vec![start, after_action, after_empty];
    let at_action = compare(&estimate.estimate, &measures[..2], &facts(&riepilogo, &estimate, &measures[1])).expect("confronto");
    let at_empty = compare(&estimate.estimate, &measures, &facts(&riepilogo, &estimate, &measures[2])).expect("confronto");
    Chain { sim, estimate, riepilogo, measures, at_action, at_empty, mover, items: paths }
}

fn no_hook(_: Phase) {}

/// Testo poco comprimibile ma non casuale (cifre esadecimali pseudocasuali): la compressione NTFS e WOF
/// lo riduce a una parte della logica, non a quasi niente.
fn write_hex_noise(path: &Path, bytes: usize, seed: u64) {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    let mut x = seed | 1;
    let mut buf = Vec::with_capacity(bytes + 16);
    while buf.len() < bytes {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        for k in 0..16 {
            buf.push(b"0123456789abcdef"[((x >> (k * 4)) & 15) as usize]);
        }
    }
    buf.truncate(bytes);
    fs::write(path, buf).unwrap();
}

// ---------------------------------------------------------------------------------------------
// 06 §6.1: stima contro variazione reale

#[test]
#[ignore = "richiede i VHDX di prova"]
fn stima_file_normali() {
    let area = Area::new(CAPACITY_MIB);
    for k in 0..REPS {
        let base = area.scenario("normali", k);
        let d = base.join("cartella");
        trees::write_file(&d.join("a.bin"), 12 << 20, 1);
        trees::write_file(&d.join("sotto").join("b.bin"), (5 << 20) + 123, 2);
        trees::write_file(&d.join("sotto").join("c.bin"), 777_777, 3);
        let f = base.join("singolo.bin");
        trees::write_file(&f, 30 << 20, 4);
        let files = [d.join("a.bin"), d.join("sotto").join("b.bin"), d.join("sotto").join("c.bin"), f.clone()];
        let expected: u64 = files.iter().map(|p| area.attrs(p).alloc).sum();
        let c = chain(&area, &[d, f], false, &mut no_hook);
        c.assert_all_recycled();
        assert_eq!(c.estimate.estimate, Estimate::Exact(expected));
        c.assert_exact_within_tolerance("normali", k);
    }
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn stima_hard_link_tutti_i_nomi() {
    let area = Area::new(CAPACITY_MIB);
    for k in 0..REPS {
        let base = area.scenario("link-tutti", k);
        let d = base.join("collegati");
        let a = d.join("originale.bin");
        trees::write_file(&a, 16 << 20, 5);
        fs::create_dir_all(d.join("sotto")).unwrap();
        fs::hard_link(&a, d.join("secondo.bin")).unwrap();
        fs::hard_link(&a, d.join("sotto").join("terzo.bin")).unwrap();
        // Il quarto nome in un altro elemento della selezione.
        let other = base.join("anche-qui");
        fs::create_dir_all(&other).unwrap();
        fs::hard_link(&a, other.join("quarto.bin")).unwrap();
        let n = d.join("normale.bin");
        trees::write_file(&n, 3 << 20, 6);
        let link = area.attrs(&a);
        assert_eq!(link.links, 4);
        let expected = link.alloc + area.attrs(&n).alloc;
        let c = chain(&area, &[d, other], false, &mut no_hook);
        c.assert_all_recycled();
        assert_eq!(c.estimate.estimate, Estimate::Exact(expected), "il file con quattro nomi conta una volta");
        assert_eq!(c.estimate.excluded_names, (0, 0));
        c.assert_exact_within_tolerance("link-tutti", k);
    }
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn stima_hard_link_con_un_nome_fuori() {
    let area = Area::new(CAPACITY_MIB);
    for k in 0..REPS {
        let base = area.scenario("link-fuori", k);
        let d = base.join("selezione");
        let a = d.join("con-nome-fuori.bin");
        trees::write_file(&a, 16 << 20, 7);
        let outside = base.join("fuori").join("nome-fuori.bin");
        fs::create_dir_all(outside.parent().unwrap()).unwrap();
        fs::hard_link(&a, &outside).unwrap();
        let n = d.join("normale.bin");
        trees::write_file(&n, 6 << 20, 8);
        let linked = area.attrs(&a);
        let expected = area.attrs(&n).alloc;
        let c = chain(&area, &[d], false, &mut no_hook);
        c.assert_all_recycled();
        assert_eq!(c.estimate.estimate, Estimate::Exact(expected), "il file con un nome fuori è escluso");
        assert_eq!(c.estimate.excluded_names, (1, linked.alloc), "e citato");
        // `ItemResult::alloc` è la quota dell'elemento nella stima: il file escluso non c'è, così il
        // confronto per elemento non vede 16 MiB di "altri processi" (corretto il 14/10/2026).
        assert_eq!(c.riepilogo.items[0].alloc, expected);
        c.assert_exact_within_tolerance("link-fuori", k);
        // La variazione non include quel file: il suo nome fuori lo tiene in vita.
        assert!(c.obtained_empty() < (expected + linked.alloc / 2) as i64, "variazione {}", c.obtained_empty());
        let still = area.attrs(&outside);
        assert_eq!((still.file_id, still.alloc, still.links), (linked.file_id, linked.alloc, 1));
    }
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn stima_compressi_ntfs() {
    let area = Area::new(CAPACITY_MIB);
    for k in 0..REPS {
        let base = area.scenario("compressi-ntfs", k);
        let d = base.join("compressi");
        let c1 = d.join("cifre.txt");
        write_hex_noise(&c1, 24 << 20, 11 + k as u64);
        let c2 = d.join("testo.log");
        trees::write_compressible(&c2, 8 << 20);
        assert!(trees::ntfs_compress(&c1) && trees::ntfs_compress(&c2), "compressione NTFS");
        let (a1, a2) = (area.attrs(&c1), area.attrs(&c2));
        eprintln!("compressi NTFS: {a1:?}\n{a2:?}");
        assert!(a1.alloc < a1.logical && a2.alloc < a2.logical, "allocate {} e {}", a1.alloc, a2.alloc);
        let c = chain(&area, &[d], false, &mut no_hook);
        c.assert_all_recycled();
        assert_eq!(c.estimate.estimate, Estimate::Exact(a1.alloc + a2.alloc), "allocata, non logica");
        c.assert_exact_within_tolerance("compressi-ntfs", k);
    }
}

/// File compressi WOF (06 §6.1, `compact /exe:lzx`). **Errore trovato e corretto il 14/10/2026**: con
/// Restart Manager la simulazione decomprimeva i file WOF (dopo `RmRegisterResources` e `RmGetList`
/// l'allocata diventava la logica, `compact /q` diceva "0 are compressed" e il volume perdeva i 10.768.384
/// byte che la compressione aveva liberato), e lo svuotamento liberava la logica (25.165.824) contro una
/// stima di 14.397.440. Con la lettura dell'uso attuale il file resta compresso e spostamento e
/// svuotamento liberano esattamente l'allocata.
#[test]
#[ignore = "richiede i VHDX di prova"]
fn stima_compressi_wof() {
    let area = Area::new(CAPACITY_MIB);
    let at = s(&area.dir);
    for k in 0..REPS {
        let base = area.scenario("compressi-wof", k);
        let d = base.join("compressi");
        let f = d.join("wof.txt");
        write_hex_noise(&f, 24 << 20, 23 + k as u64);
        assert!(trees::wof_compress(&f), "compressione WOF non riuscita su R:");
        settle(&area.probe, &at, AFTER_DELETE);
        let (a, free0) = (area.attrs(&f), area.probe.free_space(&at).unwrap());
        eprintln!("compresso WOF: {a:?}");
        assert!(a.alloc < a.logical, "allocata {} su {}", a.alloc, a.logical);
        // La simulazione non scrive: il file resta compresso e lo spazio libero non cambia.
        let sim = simulate(request(&[s(&d)]), &area.probe, &SimProgress::default()).unwrap();
        settle(&area.probe, &at, AFTER_MOVE);
        let (b, free1) = (area.attrs(&f), area.probe.free_space(&at).unwrap());
        assert_eq!(
            (b.alloc, free1),
            (a.alloc, free0),
            "la simulazione ha decompresso il file WOF: allocata {} → {}, spazio libero {}",
            a.alloc,
            b.alloc,
            free1 as i64 - free0 as i64
        );
        assert_eq!(sim.estimate(&[true]).estimate, Estimate::Exact(a.alloc));
        let c = chain(&area, &[d], false, &mut no_hook);
        c.assert_all_recycled();
        assert_eq!(c.estimate.estimate, Estimate::Exact(a.alloc), "allocata, non logica");
        c.assert_exact_within_tolerance("compressi-wof", k);
    }
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn stima_file_residenti_nella_mft() {
    let area = Area::new(CAPACITY_MIB);
    for k in 0..REPS {
        let base = area.scenario("residenti", k);
        let d = base.join("residenti");
        fs::create_dir_all(&d).unwrap();
        let mut nominal = 0u64;
        for i in 0..4000usize {
            let p = d.join(format!("r{i:04}.txt"));
            fs::write(&p, vec![b'r'; 300 + (i * 7) % 300]).unwrap();
        }
        for i in 0..4000usize {
            let a = area.attrs(&d.join(format!("r{i:04}.txt")));
            assert!(a.resident, "r{i:04}.txt non è residente: {a:?}");
            nominal += a.alloc;
        }
        let n = d.join("normale.bin");
        trees::write_file(&n, 8 << 20, 9);
        let expected = area.attrs(&n).alloc;
        // Allocata dell'indice della cartella ($I30, `FileStandardInfo` sulla cartella): con 4.001 voci non
        // sta più nella MFT, e l'eliminazione della cartella la libera. La stima la conta (03 §6.3, dal
        // 14/10/2026); i residenti no.
        let index = area.attrs(&d).alloc;
        let c = chain(&area, &[d], false, &mut no_hook);
        c.assert_all_recycled();
        assert_eq!(c.estimate.estimate, Estimate::Exact(expected + index), "i residenti valgono 0, l'indice della cartella conta");
        eprintln!("residenti: allocata riportata da Windows {nominal} byte, non contata; indice della cartella {index} byte");
        c.report("residenti", k);
        assert!(c.obtained_action().unsigned_abs() <= TOLERANCE_FIXED, "subito dopo lo spostamento: {}", c.obtained_action());
        // Criterio di 06 §6.1: la variazione non cresce per i residenti, e cade nella tolleranza della
        // stima, che conta l'indice.
        let over = c.obtained_empty() - (expected + index) as i64;
        let msg = format!("variazione oltre la stima {over}, indice {index}, allocate dei residenti {nominal}");
        assert!(within_tolerance(expected + index, c.obtained_empty()), "{msg}");
        assert!(over < (nominal / 2) as i64, "{msg}");
    }
}

/// Un albero come `node_modules`: molte cartelle con decine di file piccoli, una parte residenti nella
/// MFT. Svuotando il Cestino si liberano anche gli indici delle cartelle ($I30): prima del 14/10/2026 la
/// stima non li contava e la variazione la superava del 3% (fuori tolleranza); ora li conta (07 §1.6
/// criterio 3).
#[test]
#[ignore = "richiede i VHDX di prova"]
fn stima_molte_cartelle_piccole() {
    let area = Area::new(CAPACITY_MIB);
    for k in 0..REPS {
        let base = area.scenario("cartelle", k);
        let d = base.join("node_modules");
        let mut dirs = vec![d.clone()];
        for p in 0..120usize {
            let pkg = d.join(format!("pacchetto-{p:03}"));
            for sub in [pkg.clone(), pkg.join("lib"), pkg.join("dist")] {
                fs::create_dir_all(&sub).unwrap();
                for f in 0..(20 + p % 30) {
                    let size = 700 + (f * 997 + p * 131) % 6000;
                    fs::write(sub.join(format!("modulo-{f:02}.js")), vec![b'j'; size]).unwrap();
                }
                dirs.push(sub);
            }
        }
        let index: u64 = dirs.iter().map(|p| area.attrs(p).alloc).sum();
        let c = chain(&area, &[d], false, &mut no_hook);
        c.assert_all_recycled();
        let e = c.exact();
        c.report("cartelle", k);
        let over = c.obtained_empty() - e as i64;
        eprintln!(
            "cartelle: {} file in {} cartelle; stima {e}; indici delle cartelle {index} ({:.2}% della stima); oltre la stima {over}; entro la tolleranza: {}",
            c.sim.items()[0].files,
            dirs.len(),
            index as f64 * 100.0 / e as f64,
            within_tolerance(e, c.obtained_empty())
        );
        assert!(c.obtained_action().unsigned_abs() <= TOLERANCE_FIXED);
        // Gli indici delle cartelle sono nella stima: la variazione cade nella tolleranza.
        assert!(within_tolerance(e, c.obtained_empty()), "variazione oltre la stima {over}, indici {index}");
    }
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn stima_file_sparse() {
    let area = Area::new(CAPACITY_MIB);
    for k in 0..REPS {
        let base = area.scenario("sparse", k);
        let f = base.join("sparso.bin");
        trees::sparse_file(&f, 256 << 20);
        // Tre zone scritte davvero, oltre a quella in fondo.
        let mut h = fs::OpenOptions::new().write(true).open(&f).unwrap();
        for (at, fill) in [(10u64, 1u8), (100, 2), (200, 3)] {
            h.seek(SeekFrom::Start(at << 20)).unwrap();
            h.write_all(&vec![fill; 2 << 20]).unwrap();
        }
        // Senza `sync_all` l'allocata letta subito dopo è 0: NTFS assegna i cluster di un file sparse
        // quando scrive la cache sul disco (misurato: allocata 0, poi 6.381.568 liberati svuotando).
        h.sync_all().unwrap();
        drop(h);
        settle(&area.probe, &s(&area.dir), AFTER_MOVE);
        let a = area.attrs(&f);
        eprintln!("sparse: {a:?}");
        assert_eq!(a.logical, 256 << 20);
        assert!(a.alloc < 16 << 20, "allocata {}", a.alloc);
        let c = chain(&area, &[f], false, &mut no_hook);
        c.assert_all_recycled();
        assert_eq!(c.estimate.estimate, Estimate::Exact(a.alloc), "allocata, non logica");
        c.assert_exact_within_tolerance("sparse", k);
    }
}

/// Selezione con una cartella che non si può elencare (06 §6.1, "cartella negata"). La cartella diventa non
/// elencabile con un nome UTF-16 non valido (`trees::invalid_utf16_file`), che la simulazione tratta come una
/// cartella negata (`Probe::children` in errore, 123 invece di 5: stesso ramo, `denied_dirs`, stima a
/// intervallo) e che Windows sposta comunque nel Cestino. Con un'ACL di negazione vera Windows non sposta la
/// cartella (`cartella_con_negazione_vera_resta_al_suo_posto`): fino al 16/10/2026 su `R:` l'ACL non si
/// poteva mettere (`WRITE_DAC` negato) e la prova ripiegava da sola sul nome non valido; dal 27/10/2026
/// `vhdx crea` dà all'utente il controllo completo di `R:` e la negazione riesce.
#[test]
#[ignore = "richiede i VHDX di prova"]
fn stima_con_una_cartella_negata() {
    let area = Area::new(CAPACITY_MIB);
    for k in 0..REPS {
        let base = area.scenario("negata", k);
        let d = base.join("con-negata");
        let n = d.join("normale.bin");
        trees::write_file(&n, 6 << 20, 10);
        // h1: l'altro nome sta nella cartella negata, dentro la selezione; h2: fuori dalla selezione.
        let h1 = d.join("visibile").join("h1.bin");
        trees::write_file(&h1, 8 << 20, 11);
        let negata = d.join("negata");
        fs::create_dir_all(&negata).unwrap();
        fs::hard_link(&h1, negata.join("h1-altro.bin")).unwrap();
        let h2 = d.join("visibile").join("h2.bin");
        trees::write_file(&h2, 4 << 20, 12);
        fs::create_dir_all(base.join("fuori")).unwrap();
        fs::hard_link(&h2, base.join("fuori").join("h2-altro.bin")).unwrap();
        let (an, a1, a2) = (area.attrs(&n).alloc, area.attrs(&h1).alloc, area.attrs(&h2).alloc);
        trees::invalid_utf16_file(&negata);
        let listed = area.probe.children(&s(&negata));
        assert!(matches!(listed, Err(123)), "cartella non elencabile: {listed:?}");
        // Prima dello svuotamento la negazione si toglie dalla cartella ora nel Cestino: senza, Windows non
        // ne svuoterebbe il contenuto (e mostrerebbe un errore).
        let (root, dir, sid) = (area.root.clone(), d.clone(), area.sid.clone());
        let mut hook = |p: Phase| {
            if p == Phase::Executed {
                for (r, original) in our_bin_items(&root, &dir) {
                    eprintln!("nel Cestino: {} da {original}", r.display());
                    release(&r, &sid);
                }
            }
        };
        let c = chain(&area, &[d], false, &mut hook);
        assert_eq!(c.sim.items()[0].denied_dirs, 1);
        c.assert_all_recycled();
        assert_eq!(c.estimate.estimate, Estimate::Range { min: an, max: an + a1 + a2 }, "intervallo per i nomi non conoscibili");
        c.report("negata", k);
        assert!(c.obtained_action().unsigned_abs() <= TOLERANCE_FIXED, "subito dopo lo spostamento: {}", c.obtained_action());
        // Liberati il file normale e h1 (entrambi i nomi nel Cestino), non h2: dentro l'intervallo.
        let b = c.obtained_empty();
        assert!(b >= an as i64 && b <= (an + a1 + a2) as i64, "variazione {b} fuori da {an}-{}", an + a1 + a2);
        assert!(within_tolerance(an + a1, b), "variazione {b}, attesa {}", an + a1);
        assert_eq!(c.at_empty.difference, Some(0));
        assert!(area.attrs(&base.join("fuori").join("h2-altro.bin")).links == 1);
    }
}

/// Segnaposto cloud (06 §6.1): `TestSyncRoot` crea la cartella di sincronizzazione in
/// `std::env::temp_dir()`; per averla su `R:` la prova punta `TMP` dentro l'area per la sola costruzione.
/// Su `R:` la cartella si registra e i segnaposto si creano.
///
/// **Errore trovato e corretto il 14/10/2026**: Restart Manager chiedeva al provider i dati di ogni
/// segnaposto che riceveva (`FETCH_DATA`, cioè idratazione: con OneDrive vorrebbe dire scaricare il file),
/// 60 s a richiesta con il provider di prova che non risponde. Con la lettura dell'uso attuale nessuna
/// richiesta. Spostamento e svuotamento: nessuna finestra, stima 3.145.728 (solo il file locale) e
/// variazione 3.145.728.
#[test]
#[ignore = "richiede i VHDX di prova"]
fn segnaposto_cloud() {
    use quado_testkit::cloud::{Placeholder, TestSyncRoot};
    let area = Area::new(CAPACITY_MIB);
    let tmp = area.dir.join("tmp");
    fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os("TMP");
    // SAFETY: su Windows l'ambiente del processo è protetto dal sistema (SetEnvironmentVariableW); le
    // prove girano con --test-threads=1 e in questo momento nessun altro thread di questo processo legge
    // le variabili d'ambiente. Il valore torna com'era subito dopo.
    unsafe { std::env::set_var("TMP", &tmp) };
    let made = TestSyncRoot::new("fine");
    // SAFETY: come sopra.
    unsafe {
        match &old {
            Some(v) => std::env::set_var("TMP", v),
            None => std::env::remove_var("TMP"),
        }
    }
    let root = match made {
        Ok(r) => r,
        Err(e) => panic!("cartella di sincronizzazione di prova non registrata su R:: {e:?}"),
    };
    assert!(s(&root.path).to_lowercase().starts_with(&s(&tmp).to_lowercase()), "{}", root.path.display());
    root.create(
        "",
        &[
            Placeholder { name: "nel-cloud.bin", size: 50 << 20, dir: false, populated: false },
            Placeholder { name: "cartella", size: 0, dir: true, populated: true },
        ],
    )
    .expect("segnaposto su R:");
    root.create("cartella", &[Placeholder { name: "dentro.bin", size: 20 << 20, dir: false, populated: false }]).unwrap();
    let local = root.path.join("locale.bin");
    trees::write_file(&local, 3 << 20, 13);
    let items = [root.path.join("nel-cloud.bin"), root.path.join("cartella"), local.clone()];
    let p0 = area.attrs(&items[0]);
    eprintln!("segnaposto: {p0:?}");
    assert!(p0.placeholder && p0.alloc == 0);
    let expected = area.attrs(&local).alloc;

    // Lettura dell'uso su un solo segnaposto: quante richieste riceve il provider (che non risponde mai).
    let r0 = TestSyncRoot::requests();
    let t = Instant::now();
    let rm = area.probe.processes_using(&[s(&items[0])]);
    let r1 = TestSyncRoot::requests();
    eprintln!("uso di un segnaposto: {rm:?} in {:.1} s; richieste (dati, elenchi) {r0:?} → {r1:?}", t.elapsed().as_secs_f64());
    // La catena, con le richieste contate dopo la simulazione, dopo l'esecuzione e dopo lo svuotamento.
    let (mut after_sim, mut after_exec) = (r1, r1);
    let t = Instant::now();
    let c = chain(&area, &items, true, &mut |p| match p {
        Phase::Started => {
            after_sim = TestSyncRoot::requests();
            eprintln!("simulazione e conferma in {:.1} s", t.elapsed().as_secs_f64());
        }
        Phase::Executed => {
            after_exec = TestSyncRoot::requests();
            eprintln!("esecuzione (ricontrollo e spostamento) finita a {:.1} s", t.elapsed().as_secs_f64());
        }
    });
    let r3 = TestSyncRoot::requests();
    eprintln!("richieste: dopo la simulazione {after_sim:?}, dopo l'esecuzione {after_exec:?}, dopo lo svuotamento {r3:?}");
    eprintln!("riepilogo: {:#?}\nfinestre: {:?}", c.riepilogo.items, c.mover.windows.borrow());
    c.report("cloud", 0);

    // Senza la spunta "lo elimina anche nel cloud" il piano non si costruisce.
    for it in c.sim.items() {
        assert_eq!(it.manual, vec![ManualCheck::Synced { provider: "QuadoProvaCloud".into(), kind: quado_actions::sync::SyncKind::Registered }], "{}", it.path);
    }
    let refused = conferma(&c.sim, &c.sim.default_choice()).expect_err("senza spunta");
    assert!(refused.iter().all(|x| matches!(x, Rifiuto::ItemNotTicked(..))), "{refused:?}");
    assert_eq!(c.estimate.estimate, Estimate::Exact(expected), "solo i byte locali");
    c.assert_all_recycled();
    c.assert_exact_within_tolerance("cloud", 0);
    assert_eq!(r1, r0, "la lettura dell'uso ha chiesto al provider i dati del segnaposto (idratazione)");
    assert_eq!(after_sim, r1, "la simulazione ha chiesto dati o elenchi al provider");
    assert_eq!(after_exec, after_sim, "ricontrollo e spostamento hanno chiesto dati o elenchi al provider");
    assert_eq!(r3, after_exec, "lo svuotamento ha chiesto dati o elenchi al provider");
}

// ---------------------------------------------------------------------------------------------
// 06 §5.3: Cestino piccolo

#[test]
#[ignore = "richiede i VHDX di prova"]
fn capacita_superata_blocca_prima_dello_spostamento() {
    let area = Area::new(SMALL_MIB);
    // Un elemento già nel Cestino.
    let old = area.dir.join("vecchio.bin");
    trees::write_file(&old, 10 << 20, 14);
    let first = chain_once(&area, std::slice::from_ref(&old));
    assert_eq!(first, vec![ItemOutcome::Recycled]);
    let (bin0, entries0) = (area.bin(), area.bin_entries());
    assert_eq!(bin0.items, 1);

    // Due file da 12 MiB: ciascuno sta nel Cestino, insieme al contenuto attuale no.
    let a = area.dir.join("a.bin");
    let b = area.dir.join("b.bin");
    trees::write_file(&a, 12 << 20, 15);
    trees::write_file(&b, 12 << 20, 16);
    let paths = vec![s(&a), s(&b)];
    let sim = simulate(request(&paths), &area.probe, &SimProgress::default()).unwrap();
    assert!(sim.items().iter().all(|it| it.blocks.is_empty()), "{:?}", sim.items());
    let capacity = u64::from(SMALL_MIB) << 20;
    let total = bin0.bytes + (24 << 20);
    assert_eq!(sim.capacity(&[true, true]), CapacityCheck::Exceeded { capacity, total });
    let checks = sim.rule_checks();
    assert!(checks.iter().any(|c| c.key == "controllo-capacita-cestino" && c.outcome == CheckOutcome::Failed), "{checks:?}");
    // Sensore memoria: la simulazione riporta i giorni letti da HKCU (06 §5.3, F-AC-7).
    let ss = extract::storage_sense(&quado_kb::win::LiveRegistry).expect("Sensore memoria");
    assert_eq!(sim.storage_sense_days(), if ss.enabled { ss.recycle_days } else { None });
    eprintln!("Sensore memoria: {:?} giorni", sim.storage_sense_days());
    // Nessun piano: senza `PianoConfermato` nessuna chiamata a IFileOperation è possibile (05 D7, V6).
    let refused = conferma(&sim, &sim.default_choice()).expect_err("capacità superata");
    assert_eq!(refused, vec![Rifiuto::Capacity(CapacityCheck::Exceeded { capacity, total })]);
    assert!(a.exists() && b.exists());
    assert_eq!(area.bin(), bin0, "gli elementi già nel Cestino restano tutti");
    assert_eq!(area.bin_entries(), entries0);

    // Uno solo dei due ci sta: si sposta, e l'elemento vecchio resta nel Cestino.
    let sim = simulate(request(&paths[..1]), &area.probe, &SimProgress::default()).unwrap();
    assert!(matches!(sim.capacity(&[true]), CapacityCheck::Fits { .. }));
    let piano = conferma(&sim, &sim.default_choice()).expect("piano");
    let mover = WatchedMover::default();
    let r = esegui(piano, &area.probe, &mover, &ExecProgress::default());
    assert_eq!(r.items[0].outcome, ItemOutcome::Recycled);
    assert!(mover.windows.borrow().is_empty());
    let entries1 = area.bin_entries();
    assert!(entries0.iter().all(|e| entries1.contains(e)), "il vecchio elemento è ancora nel Cestino: {entries1:?}");
    assert_eq!(area.bin().items, 2);
}

/// Simula, conferma la selezione proposta ed esegue: gli esiti.
fn chain_once(area: &Area, items: &[PathBuf]) -> Vec<ItemOutcome> {
    let paths: Vec<String> = items.iter().map(|p| s(p)).collect();
    let sim = simulate(request(&paths), &area.probe, &SimProgress::default()).unwrap();
    let piano = conferma(&sim, &sim.default_choice()).expect("piano");
    let mover = WatchedMover::default();
    let r = esegui(piano, &area.probe, &mover, &ExecProgress::default());
    assert!(mover.windows.borrow().is_empty(), "{:?}", mover.windows.borrow());
    r.items.iter().map(|i| i.outcome).collect()
}

/// Cartella con dentro una sottocartella negata da un'ACL vera (lettura ed elenco negati all'utente): Windows
/// non la mette nel Cestino (`COPYENGINE_E_ACCESS_DENIED_SRC`, 0x80270021), nessuna finestra, l'esito lo dice e
/// tutto resta al suo posto. Se l'ACL non si può mettere (`WRITE_DAC` negato) la prova non ha senso e lo dice.
#[test]
#[ignore = "richiede i VHDX di prova"]
fn cartella_con_negazione_vera_resta_al_suo_posto() {
    let area = Area::new(CAPACITY_MIB);
    let d = area.scenario("negazione-vera", 0).join("con-negata");
    trees::write_file(&d.join("normale.bin"), 1 << 20, 10);
    let negata = d.join("negata");
    trees::write_file(&negata.join("dentro.bin"), 1 << 20, 11);
    if !area.deny_listing(&negata) {
        eprintln!("ACL di negazione non messa (WRITE_DAC negato): prova non eseguita");
        return;
    }
    assert!(matches!(area.probe.children(&s(&negata)), Err(5)));
    let outcomes = chain_once(&area, std::slice::from_ref(&d));
    assert_eq!(outcomes, [ItemOutcome::Failed(0x8027_0021_u32 as i32)]);
    assert!(d.join("normale.bin").exists() && negata.exists(), "la cartella è rimasta al suo posto");
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn elemento_piu_grande_del_cestino_resta_al_suo_posto() {
    let area = Area::new(SMALL_MIB);
    let big = area.dir.join("troppo-grande.bin");
    trees::sparse_file(&big, 100 << 20);
    let small = area.dir.join("piccolo.bin");
    trees::write_file(&small, MIB as usize, 17);
    let before = area.attrs(&big);
    let paths = vec![s(&big), s(&small)];
    let sim = simulate(request(&paths), &area.probe, &SimProgress::default()).unwrap();
    let capacity = u64::from(SMALL_MIB) << 20;
    assert_eq!(sim.items()[0].blocks, vec![Block::LargerThanBin { logical: 100 << 20, capacity }]);
    assert!(sim.items()[1].blocks.is_empty());
    assert_eq!(sim.default_selection(), vec![false, true]);
    let mut forced = sim.default_choice();
    forced.selected = vec![true, true];
    let refused = conferma(&sim, &forced).expect_err("elemento più grande del Cestino");
    assert!(refused.contains(&Rifiuto::Blocked(0, Block::LargerThanBin { logical: 100 << 20, capacity })), "{refused:?}");
    // La selezione proposta sposta solo il piccolo: il mover non riceve mai il grande.
    let piano = conferma(&sim, &sim.default_choice()).expect("piano");
    let mover = WatchedMover::default();
    let r = esegui(piano, &area.probe, &mover, &ExecProgress::default());
    assert_eq!(mover.calls.borrow().iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(), vec![s(&small)]);
    assert!(mover.windows.borrow().is_empty());
    assert_eq!(r.items.len(), 1);
    assert_eq!(r.items[0].outcome, ItemOutcome::Recycled);
    // F-AC-5: il grande è al suo posto, con lo stesso ID e la stessa dimensione.
    let after = area.attrs(&big);
    assert_eq!((after.file_id, after.logical, after.alloc, after.modified), (before.file_id, before.logical, before.alloc, before.modified));
    assert_eq!(area.bin().items, 1);
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn nuke_on_delete_blocca_senza_chiamare_il_mover() {
    let area = Area::new(CAPACITY_MIB);
    let f = area.dir.join("con-nuke.bin");
    trees::write_file(&f, 2 << 20, 18);
    let before = area.attrs(&f);
    let bin0 = area.bin();
    {
        let _nuke = BinSetting::nuke(&area.root);
        let paths = vec![s(&f)];
        let sim = simulate(request(&paths), &area.probe, &SimProgress::default()).unwrap();
        assert!(sim.volume().nuke_on_delete);
        assert_eq!(sim.items()[0].blocks, vec![Block::NukeOnDelete]);
        assert_eq!(sim.default_selection(), vec![false]);
        assert_eq!(conferma(&sim, &sim.default_choice()).expect_err("niente da spostare"), vec![Rifiuto::Empty]);
        let mut forced = sim.default_choice();
        forced.selected = vec![true];
        assert_eq!(conferma(&sim, &forced).expect_err("NukeOnDelete"), vec![Rifiuto::Blocked(0, Block::NukeOnDelete)]);
    }
    // Nessun piano, quindi nessuna chiamata al mover: il file è com'era e il Cestino pure.
    let after = area.attrs(&f);
    assert_eq!((after.file_id, after.logical, after.modified), (before.file_id, before.logical, before.modified));
    assert_eq!(area.bin(), bin0);
    assert!(!area.probe.volume_bin(&s(&f)).unwrap().nuke_on_delete, "NukeOnDelete ripristinato");
}

// ---------------------------------------------------------------------------------------------
// 06 §5.4: Restart Manager

/// Processo di prova (PowerShell) che tiene aperto un file senza condivisione finché vive; termina al drop.
struct Holder {
    child: Child,
}

impl Holder {
    fn open(path: &Path) -> Self {
        let script = format!(
            "$f = [System.IO.File]::Open('{}', 'Open', 'Read', 'None'); [Console]::Out.WriteLine('aperto'); [Console]::Out.Flush(); Start-Sleep -Seconds 600",
            path.display()
        );
        let mut child = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("powershell");
        let mut line = String::new();
        let _ = BufReader::new(child.stdout.take().expect("uscita")).read_line(&mut line);
        let me = Holder { child };
        assert_eq!(line.trim(), "aperto", "il processo di prova non ha aperto {}", path.display());
        me
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn file_tenuto_aperto_da_un_processo_bloccato_con_il_nome() {
    let area = Area::new(CAPACITY_MIB);
    let f = area.dir.join("tenuto-aperto.bin");
    trees::write_file(&f, 2 << 20, 19);
    let holder = Holder::open(&f);
    let sim = simulate(request(&[s(&f)]), &area.probe, &SimProgress::default()).unwrap();
    let blocks = &sim.items()[0].blocks;
    eprintln!("blocchi: {blocks:?}");
    let Some(Block::InUse(users)) = blocks.iter().find(|b| matches!(b, Block::InUse(_))) else { panic!("non bloccato: {blocks:?}") };
    let user = users.iter().find(|u| u.pid == holder.pid()).expect("il processo di prova tra quelli che usano il file");
    assert!(!user.app_name.is_empty(), "nome del processo: {user:?}");
    assert!(sim.rule_checks().iter().any(|c| c.key == "controllo-restart-manager" && c.outcome == CheckOutcome::Failed));
    assert_eq!(conferma(&sim, &sim.default_choice()).expect_err("bloccato"), vec![Rifiuto::Empty]);
    drop(holder);
    assert!(f.exists());
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn file_aperto_e_chiuso_prima_del_controllo_ammesso() {
    let area = Area::new(CAPACITY_MIB);
    let f = area.dir.join("aperto-e-chiuso.bin");
    trees::write_file(&f, 2 << 20, 20);
    let script = format!("$f = [System.IO.File]::Open('{}', 'Open', 'Read', 'None'); $f.Close()", f.display());
    let st = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).status().expect("powershell");
    assert!(st.success());
    let c = chain(&area, &[f], false, &mut no_hook);
    assert!(c.sim.items()[0].blocks.is_empty(), "{:?}", c.sim.items()[0].blocks);
    c.assert_all_recycled();
    c.assert_exact_within_tolerance("aperto-e-chiuso", 0);
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn file_aperto_dopo_la_simulazione_fallisce_e_gli_altri_proseguono() {
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
    let area = Area::new(CAPACITY_MIB);
    let files: Vec<PathBuf> = ["a.bin", "b.bin", "c.bin"].iter().map(|n| area.dir.join(n)).collect();
    for (k, f) in files.iter().enumerate() {
        trees::write_file(f, 2 << 20, 21 + k as u8);
    }
    let paths: Vec<String> = files.iter().map(|p| s(p)).collect();
    let sim = simulate(request(&paths), &area.probe, &SimProgress::default()).unwrap();
    let piano = conferma(&sim, &sim.default_choice()).expect("piano");
    // Aperto dal test dopo la simulazione, senza condivisione in cancellazione.
    let held = fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE).open(&files[1]).unwrap();
    let mover = WatchedMover::default();
    let r = esegui(piano, &area.probe, &mover, &ExecProgress::default());
    drop(held);
    eprintln!("riepilogo: {:#?}\ntracce: {:#?}", r.items, mover.traces.borrow());
    assert!(mover.windows.borrow().is_empty(), "nessuna finestra (FOF_NOERRORUI): {:?}", mover.windows.borrow());
    let outcomes: Vec<ItemOutcome> = r.items.iter().map(|i| i.outcome).collect();
    assert_eq!(outcomes[0], ItemOutcome::Recycled);
    assert!(matches!(outcomes[1], ItemOutcome::Failed(_)), "{outcomes:?}");
    assert_eq!(outcomes[2], ItemOutcome::Recycled, "gli altri proseguono");
    assert!(!r.stopped);
    assert!(files[1].exists() && !files[0].exists() && !files[2].exists());
    assert_eq!(mover.calls.borrow().len(), 3);
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn cartella_grande_campione_con_tutti_gli_eseguibili_e_le_dll() {
    let area = Area::new(CAPACITY_MIB);
    let d = area.dir.join("cartella-grande");
    let base = SystemTime::now() - Duration::from_secs(3600);
    let mut data = Vec::new();
    for i in 0..1100usize {
        let p = d.join("dati").join(format!("g{:02}", i / 100)).join(format!("f{i:04}.dat"));
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, b"dati di prova").unwrap();
        // Il file `i` è più recente di tutti quelli prima.
        fs::File::options().write(true).open(&p).unwrap().set_modified(base + Duration::from_secs(i as u64)).unwrap();
        data.push(s(&p));
    }
    let mut code = Vec::new();
    for i in 0..15usize {
        for (sub, ext) in [("bin", "exe"), ("lib", "dll")] {
            let p = d.join(sub).join(format!("modulo{i:02}.{ext}"));
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, b"MZ non eseguibile, solo il nome conta").unwrap();
            // Più vecchi di tutti i dati: entrano nel campione per l'estensione, non per la data.
            fs::File::options().write(true).open(&p).unwrap().set_modified(base - Duration::from_secs(86_400)).unwrap();
            code.push(s(&p));
        }
    }
    let holder = Holder::open(&d.join("lib").join("modulo07.dll"));
    let rec = Recording::new(&area.probe);
    let sim = simulate(request(&[s(&d)]), &rec, &SimProgress::default()).unwrap();
    let calls = rec.rm.lock().unwrap().clone();
    assert_eq!(calls.len(), 1, "una chiamata a Restart Manager per elemento");
    let sample = &calls[0];
    eprintln!("campione: {} file", sample.len());
    let key = |p: &String| p.to_lowercase();
    let set: std::collections::HashSet<String> = sample.iter().map(key).collect();
    for c in &code {
        assert!(set.contains(&key(c)), "{c} manca dal campione");
    }
    assert_eq!(sample.len(), code.len() + 1000, "eseguibili e DLL più i 1.000 file più recenti");
    for (i, p) in data.iter().enumerate() {
        assert_eq!(set.contains(&key(p)), i >= 100, "{p}");
    }
    let blocks = &sim.items()[0].blocks;
    assert!(
        blocks.iter().any(|b| matches!(b, Block::InUse(u) if u.iter().any(|x| x.pid == holder.pid()))),
        "la DLL aperta dal processo di prova blocca la cartella: {blocks:?}"
    );
    assert_eq!(sim.items()[0].files, 1130);
}

// ---------------------------------------------------------------------------------------------
// 06 §6.2: flusso di verifica

#[test]
#[ignore = "richiede i VHDX di prova"]
fn misura_di_partenza_prima_del_primo_spostamento() {
    let area = Area::new(CAPACITY_MIB);
    let base = area.scenario("partenza", 0);
    let (a, b) = (base.join("a.bin"), base.join("b.bin"));
    trees::write_file(&a, 4 << 20, 30);
    trees::write_file(&b, 4 << 20, 31);
    let c = chain(&area, &[a, b], false, &mut no_hook);
    c.assert_all_recycled();
    let start = &c.measures[0];
    let first_call = c.mover.calls.borrow()[0].1;
    eprintln!("partenza {}, inizio del riepilogo {}, prima chiamata {first_call}", start.at, c.riepilogo.started);
    assert_eq!(start.point, MeasurePoint::Start);
    // Misurato il 14/10/2026: le tre ore sono uguali. `system::now_filetime` usa `GetSystemTimeAsFileTime`,
    // che avanza a passi di circa 15,6 ms: l'ordine lo garantisce il codice (la misura precede
    // `esegui`), ma il registro da solo non mostra un'ora strettamente precedente (F-VE-1).
    assert!(start.at <= c.riepilogo.started, "la misura di partenza precede l'inizio dell'esecuzione");
    assert!(start.at <= first_call, "e la prima chiamata a IFileOperation");
    assert!(start.bin.is_some() && start.free > 0);
    // F-VE-2: dopo l'azione e dopo lo svuotamento, spazio libero, Cestino, riscansione e ora.
    for (m, point) in c.measures[1..].iter().zip([MeasurePoint::AfterAction, MeasurePoint::AfterEmpty]) {
        assert_eq!(m.point, point);
        assert!(m.at >= c.riepilogo.finished && m.bin.is_some() && m.still_present.is_some());
    }
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn attivita_concorrente_dichiarata_e_non_attribuita() {
    let area = Area::new(CAPACITY_MIB);
    let base = area.scenario("concorrente", 0);
    let sel = base.join("selezione");
    trees::write_file(&sel.join("a.bin"), 24 << 20, 32);
    let writer_dir = area.dir.join("scrittore");
    fs::create_dir_all(&writer_dir).unwrap();
    let written = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let mut handle = None;
    let (wd, w) = (writer_dir.clone(), written.clone());
    let mut hook = |p: Phase| match p {
        // Un thread scrive 12 MiB su R: tra la misura di partenza e la misura dopo l'azione.
        Phase::Started => {
            let (wd, w) = (wd.clone(), w.clone());
            handle = Some(std::thread::spawn(move || {
                for i in 0..12u8 {
                    trees::write_file(&wd.join(format!("w{i:02}.bin")), MIB as usize, i);
                    w.fetch_add(MIB, std::sync::atomic::Ordering::Relaxed);
                    std::thread::sleep(Duration::from_millis(20));
                }
            }));
        }
        Phase::Executed => {
            handle.take().expect("scrittore").join().expect("scrittore");
        }
    };
    let c = chain(&area, std::slice::from_ref(&sel), false, &mut hook);
    c.assert_all_recycled();
    c.report("concorrente", 0);
    let e = c.exact();
    let writer_alloc: u64 = (0..12u8).map(|i| area.attrs(&writer_dir.join(format!("w{i:02}.bin"))).alloc).sum();
    assert_eq!(written.load(std::sync::atomic::Ordering::Relaxed), 12 * MIB);
    // La variazione del volume cambia di quanto scritto; la riscansione dice che la selezione non c'è più.
    assert!(!within_tolerance(e, c.obtained_empty()), "la variazione dovrebbe risentire dello scrittore");
    assert!(within_tolerance(e - writer_alloc, c.obtained_empty()), "stima {e}, scritti {writer_alloc}, variazione {}", c.obtained_empty());
    assert_eq!(c.measures[2].still_present, Some(0));
    // La riscansione del sottoalbero toccato non vede lo scrittore: la cartella della selezione è vuota.
    let t = scan_tree(&s(&base)).expect("riscansione");
    assert_eq!((t.alloc(Tree::ROOT), t.files(Tree::ROOT)), (0, 0));
    eprintln!("confronto dopo lo svuotamento: {:?}", c.at_empty);
    let other = c.at_empty.explanations.iter().find_map(|x| match x {
        Explanation::OtherActivity { bytes } => Some(*bytes),
        _ => None,
    });
    let other = other.expect("differenza dichiarata come attività di altri processi");
    assert!((other + writer_alloc as i64).unsigned_abs() <= TOLERANCE_FIXED, "attività {other}, scritti {writer_alloc}");
    assert_eq!(c.at_empty.unexplained, None, "non \"non spiegata\"");
    assert!(!c.at_empty.explanations.iter().any(|x| matches!(x, Explanation::SkippedOrFailed { .. })), "non attribuita all'intervento");
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn riscansione_innestata_uguale_alla_scansione_completa() {
    let area = Area::new(CAPACITY_MIB);
    let base = area.scenario("innesto", 0);
    let d = base.join("cartella");
    trees::write_file(&d.join("a.bin"), 3 << 20, 33);
    trees::write_file(&d.join("sotto").join("b.bin"), 1 << 20, 34);
    fs::write(d.join("piccolo.txt"), b"residente").unwrap();
    let f = base.join("altra").join("file.bin");
    trees::write_file(&f, 2 << 20, 35);
    trees::write_file(&base.join("altra").join("resta.bin"), 1 << 20, 36);
    trees::write_file(&base.join("resta.bin"), 1 << 20, 37);
    let root = s(&area.root);
    let mut model = scan_tree(&root).expect("scansione di R:");
    let before = (model.alloc(Tree::ROOT), model.logical(Tree::ROOT), model.files(Tree::ROOT));
    let c = chain(&area, &[d, f], false, &mut no_hook);
    c.assert_all_recycled();
    // Come l'interfaccia: la cartella di ogni elemento e il Cestino dell'utente, innestati nel modello.
    let user_bin = format!(r"{}\$Recycle.Bin\{}", root.trim_end_matches('\\'), area.sid);
    let dirs = rescan_dirs(&c.items, &[user_bin]);
    eprintln!("riscansioni: {dirs:?}");
    for dir in &dirs {
        let node = find_node(&model, dir).unwrap_or_else(|| panic!("{dir} non è nel modello"));
        match scan_tree(dir) {
            Ok(sub) => {
                model.graft(node, &sub);
            }
            Err(2 | 3) => model.detach(node),
            Err(e) => panic!("{dir}: errore {e}"),
        }
    }
    let fresh = scan_tree(&root).expect("scansione nuova di R:");
    let grafted = (model.alloc(Tree::ROOT), model.logical(Tree::ROOT), model.files(Tree::ROOT));
    let full = (fresh.alloc(Tree::ROOT), fresh.logical(Tree::ROOT), fresh.files(Tree::ROOT));
    eprintln!("prima {before:?}, innestato {grafted:?}, scansione completa {full:?}");
    assert_eq!(grafted, full, "F-VE-4: totali del modello aggiornato e di una scansione completa nuova");
    for dir in &dirs {
        let (m, n) = (find_node(&model, dir).unwrap(), find_node(&fresh, dir).unwrap());
        assert_eq!((model.alloc(m), model.files(m)), (fresh.alloc(n), fresh.files(n)), "{dir}");
    }
    assert!(before.0 > grafted.0, "la selezione non occupa più spazio");
}

// ---------------------------------------------------------------------------------------------
// F-AC-6: registro degli interventi

#[test]
#[ignore = "richiede i VHDX di prova"]
fn registro_con_tutti_i_campi() {
    let area = Area::new(CAPACITY_MIB);
    let data = TestDir::new("registro-f");
    assert_eq!(journal::size(&data.path), None, "il registro non esiste prima del primo intervento");
    let base = area.scenario("registro", 0);
    let (d, f) = (base.join("cartella"), base.join("file.bin"));
    trees::write_file(&d.join("a.bin"), 5 << 20, 40);
    trees::write_file(&f, 3 << 20, 41);
    let c = chain(&area, &[d, f], false, &mut no_hook);
    c.assert_all_recycled();
    let r = &c.riepilogo;
    let entry = JournalEntry {
        id: c.measures[0].at,
        volume: r.volume.clone(),
        kind: InterventionKind::Recycle,
        origin: r.origin.clone(),
        method: Action::Recycle.code().to_owned(),
        estimate: r.estimate.estimate,
        items: r.items.iter().map(|i| JournalItem { path: i.path.clone(), bytes: i.alloc, outcome: i.outcome }).collect(),
        measures: c.measures.clone(),
        comparison: Some(c.at_empty.clone()),
    };
    journal::append(&data.path, &entry).expect("registro");
    let j = journal::read(&data.path).expect("lettura del registro");
    assert_eq!(j.unreadable_lines, 0);
    assert_eq!(j.entries, vec![entry.clone()]);
    let raw = fs::read_to_string(journal::path(&data.path)).unwrap();
    eprintln!("riga del registro: {raw}");
    let v: serde_json::Value = serde_json::from_str(raw.trim()).expect("JSON");
    for k in ["versione", "id", "volume", "tipo", "origine", "metodo", "stima", "elementi", "misure", "confronto"] {
        assert!(!v[k].is_null(), "manca {k}");
    }
    assert_eq!(v["volume"], r"R:\");
    assert_eq!(v["tipo"], "spostamento");
    assert_eq!(v["metodo"], "cestino");
    assert_eq!(v["stima"]["stato"], "esatta");
    assert!(v["stima"]["byte"].as_u64().is_some_and(|b| b > 0));
    let items = v["elementi"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    for it in items {
        assert!(it["percorso"].as_str().is_some_and(|p| p.starts_with(r"R:\interventi-prove-f")));
        assert!(it["byte"].as_u64().is_some_and(|b| b > 0));
        assert_eq!(it["esito"], "nel-cestino");
    }
    let measures = v["misure"].as_array().unwrap();
    let points: Vec<&str> = measures.iter().map(|m| m["punto"].as_str().unwrap()).collect();
    assert_eq!(points, ["partenza", "dopo-azione", "dopo-svuotamento"]);
    for (k, m) in measures.iter().enumerate() {
        assert!(m["ora"].as_i64().is_some_and(|t| t > 0) && m["libero"].as_u64().is_some_and(|b| b > 0), "{m}");
        assert!(m["cestino"]["elementi"].is_u64() && m["cestino"]["byte"].is_u64(), "{m}");
        assert_eq!(m["ancora_presenti"].is_u64(), k > 0, "{m}");
    }
    let cmp = &v["confronto"];
    assert_eq!(cmp["stimato"]["stato"], "esatta");
    assert!(cmp["stimato"]["byte"].as_u64().is_some_and(|b| b > 0), "stimato");
    assert!(cmp["ottenuto"].as_i64().is_some_and(|b| b > 0), "ottenuto");
    assert!(cmp["differenza"].is_i64() && cmp["punti_base"].is_u64() && cmp["spiegazioni"].is_array(), "{cmp}");
}
