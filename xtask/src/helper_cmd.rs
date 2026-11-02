// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask helper-prova`: prova dell'helper elevato, con una sola richiesta UAC per
//! esecuzione. La esegue chi mantiene il progetto, che approva il prompt.
//!
//! - `helper-prova`: su `R:` (VHDX NTFS di prova). Prepara il contenuto di
//!   `quado_testkit::helper` (cartella negata, profilo finto di un altro SID, Cestini di un SID che
//!   esiste e di uno orfano) con i valori attesi misurati dall'oracolo prima della chiusura; scansione non
//!   elevata di `R:\` per trovare le cartelle negate; pipe (`helper::pipe::Server`); avvio di
//!   `quado.exe --helper <pipe> <pid di xtask>` con `ShellExecuteExW` e il verbo `runas`; richiesta;
//!   risposte fino a `Done`; confronti; pulizia di `R:`; resoconto in `target/misure/helper-prova.md`.
//! - `helper-prova --c`: le cartelle negate vere di C: e la lettura vera delle copie shadow (con il
//!   consenso di chi usa il PC). Il resoconto, `target/misure/helper-prova-c.md`, ha soli totali: nessun nome di
//!   cartella sotto i profili, nessun SID.
//! - `helper-prova --senza-helper`: preparazione, scansione non elevata e controlli fino alla richiesta,
//!   poi pulizia. Nessun prompt UAC: è il controllo da fare prima.
//! - `helper-prova prepara` e `pulisci`: il contenuto di prova resta su `R:` (per provare "Ripeti come
//!   amministratore" dall'interfaccia) e si toglie dopo.
//!
//! L'helper è `target\release\quado.exe`, accanto a xtask: prima `cargo build --release -p quado`.
//! L'avvio è scritto qui e non con `quado_actions::elevate::launch_helper`, che avvierebbe l'eseguibile
//! corrente, cioè xtask. Il pid passato all'helper è quello di xtask, che crea la pipe: l'helper verifica
//! che il server della pipe sia il suo genitore.

// Avvio con ShellExecuteExW e attesa del processo: ogni blocco ha il suo invariante.
#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use quado_model::{Flags, Tree};
use quado_scan_win::helper::pipe::{PipeError, Server};
use quado_scan_win::helper::proto::{self, Reply, Request, ShadowStorage, Totals};
use quado_scan_win::{Options, Problem, Progress, scan};
use quado_testkit::helper::{self as content, Expected, ExpectedFolder};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::core::{HSTRING, w};

const USAGE: &str = "helper-prova [--c | --senza-helper | prepara | pulisci]";

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None => test_r(true),
        Some("--senza-helper") => test_r(false),
        Some("--c") => test_c(),
        Some("prepara") => {
            let e = content::prepare();
            println!("Contenuto di prova su {} (resta fino a `cargo xtask helper-prova pulisci`):\n\n{}", e.volume, e.to_markdown());
            Ok(())
        }
        Some("pulisci") => content::cleanup(),
        Some(other) => Err(format!("{USAGE}: opzione sconosciuta {other}")),
    }
}

// ---------------------------------------------------------------------------------------------------
// Avvio elevato

/// `SHELLEXECUTEINFOW` di `shellapi.h`, dichiarata qui come in `quado_actions::launch`: la struttura
/// del crate `windows` richiede la feature `Win32_System_Registry`, che il workspace non attiva.
#[repr(C)]
struct ShellExecuteInfoW {
    cb_size: u32,
    f_mask: u32,
    hwnd: *mut c_void,
    lp_verb: *const u16,
    lp_file: *const u16,
    lp_parameters: *const u16,
    lp_directory: *const u16,
    n_show: i32,
    h_inst_app: *mut c_void,
    lp_id_list: *mut c_void,
    lp_class: *const u16,
    hkey_class: *mut c_void,
    dw_hot_key: u32,
    h_icon: *mut c_void,
    h_process: *mut c_void,
}

#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<ShellExecuteInfoW>() == 112);

const SEE_MASK_NOCLOSEPROCESS: u32 = 0x0000_0040;
const SEE_MASK_NOASYNC: u32 = 0x0000_0100;
const SW_HIDE: i32 = 0;
const WAIT_OBJECT_0: u32 = 0;
const ERROR_CANCELLED: u32 = 1223;

/// Importazioni private (le feature `Win32_System_Threading` del crate `windows` non sono attive in xtask).
mod ffi {
    use super::ShellExecuteInfoW;
    use std::ffi::c_void;
    windows_core::link!("shell32.dll" "system" fn ShellExecuteExW(pexecinfo: *mut ShellExecuteInfoW) -> windows_core::BOOL);
    windows_core::link!("kernel32.dll" "system" fn GetProcessId(process: *mut c_void) -> u32);
    windows_core::link!("kernel32.dll" "system" fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32);
    windows_core::link!("kernel32.dll" "system" fn GetExitCodeProcess(process: *mut c_void, code: *mut u32) -> windows_core::BOOL);
    windows_core::link!("kernel32.dll" "system" fn TerminateProcess(process: *mut c_void, code: u32) -> windows_core::BOOL);
    windows_core::link!("kernel32.dll" "system" fn CloseHandle(handle: *mut c_void) -> windows_core::BOOL);
    windows_core::link!("kernel32.dll" "system" fn GetLastError() -> u32);
    windows_core::link!("kernel32.dll" "system" fn GetConsoleWindow() -> *mut c_void);
}

/// COM sul thread per `ShellExecuteExW` (estensioni della shell), bilanciato solo se riuscito.
struct Com(bool);

impl Com {
    fn init() -> Self {
        // SAFETY: inizializzazione COM del thread corrente, bilanciata al drop se riuscita.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Com(hr.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: bilancia la CoInitializeEx riuscita sullo stesso thread.
            unsafe { CoUninitialize() };
        }
    }
}

/// Il processo dell'helper avviato; l'handle si chiude al drop, dopo averlo terminato se è ancora vivo.
struct Helper {
    process: *mut c_void,
    pid: u32,
}

impl Helper {
    /// Una richiesta UAC. `Err` con il codice di Windows (1223: la risposta al prompt è stata No).
    fn launch(exe: &Path, pipe: &str) -> Result<Helper, u32> {
        let _com = Com::init();
        let file = HSTRING::from(exe.as_os_str());
        let params = HSTRING::from(format!("--helper {pipe} {}", std::process::id()));
        let verb = w!("runas");
        let mut info = ShellExecuteInfoW {
            cb_size: std::mem::size_of::<ShellExecuteInfoW>() as u32,
            f_mask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            // Finestra della console come proprietaria: il prompt UAC compare davanti e non solo nella barra.
            // SAFETY: nessun prerequisito; nullo se il processo non ha una console.
            hwnd: unsafe { ffi::GetConsoleWindow() },
            lp_verb: verb.as_ptr(),
            lp_file: file.as_ptr(),
            lp_parameters: params.as_ptr(),
            lp_directory: std::ptr::null(),
            n_show: SW_HIDE,
            h_inst_app: std::ptr::null_mut(),
            lp_id_list: std::ptr::null_mut(),
            lp_class: std::ptr::null(),
            hkey_class: std::ptr::null_mut(),
            dw_hot_key: 0,
            h_icon: std::ptr::null_mut(),
            h_process: std::ptr::null_mut(),
        };
        // SAFETY: `info` ha `cb_size` uguale alla sua dimensione e la disposizione di SHELLEXECUTEINFOW;
        // verbo, file e parametri sono stringhe terminate da zero che vivono fino al ritorno. Con
        // SEE_MASK_NOCLOSEPROCESS la shell restituisce l'handle del processo, chiuso da `Helper`.
        let ok = unsafe { ffi::ShellExecuteExW(&mut info) };
        if !ok.as_bool() {
            // SAFETY: nessun prerequisito.
            return Err(unsafe { ffi::GetLastError() });
        }
        if info.h_process.is_null() {
            return Err(0);
        }
        // SAFETY: handle di processo valido restituito dalla shell.
        let pid = unsafe { ffi::GetProcessId(info.h_process) };
        Ok(Helper { process: info.h_process, pid })
    }

    /// Codice di uscita entro `timeout`; `None` se è ancora in esecuzione.
    fn wait(&self, timeout: Duration) -> Option<u32> {
        // SAFETY: handle valido fino al drop.
        if unsafe { ffi::WaitForSingleObject(self.process, timeout.as_millis().min(u128::from(u32::MAX - 1)) as u32) } != WAIT_OBJECT_0 {
            return None;
        }
        let mut code = 0u32;
        // SAFETY: handle valido; `code` è un'uscita locale.
        (unsafe { ffi::GetExitCodeProcess(self.process, &mut code) }).as_bool().then_some(code)
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        if self.wait(Duration::ZERO).is_none() {
            // SAFETY: handle valido; se la shell non ha concesso PROCESS_TERMINATE la chiamata fallisce e basta.
            let _ = unsafe { ffi::TerminateProcess(self.process, 1) };
        }
        // SAFETY: handle restituito da ShellExecuteExW, chiuso una volta sola.
        let _ = unsafe { ffi::CloseHandle(self.process) };
    }
}

/// `target\release\quado.exe`, accanto a xtask (che gira con `--release`).
fn helper_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?.with_file_name("quado.exe");
    if !exe.is_file() {
        return Err(format!("{} non c'è: prima `cargo build --release -p quado`", exe.display()));
    }
    Ok(exe)
}

/// Quello che l'helper ha risposto.
struct Outcome {
    replies: Vec<Reply>,
    /// Errore della pipe che ha interrotto la lettura, se c'è.
    pipe_error: Option<PipeError>,
    exit: Option<u32>,
    elapsed: Duration,
}

/// Pipe, avvio elevato (una richiesta UAC), richiesta, risposte fino a `Done` o alla chiusura.
fn run_helper(volume: &str, folders: &[String]) -> Result<Outcome, String> {
    let exe = helper_exe()?;
    let request = Request { volume: volume.to_owned(), folders: folders.to_vec() };
    request.validate().map_err(|e| format!("la richiesta non passerebbe la validazione dell'helper: {e:?}"))?;
    let mut server = Server::create().map_err(|e| format!("pipe non creata: {e:?}"))?;
    eprintln!("Avvio di {} come amministratore: approvare la richiesta UAC.", exe.display());
    let started = Instant::now();
    let helper = Helper::launch(&exe, server.name()).map_err(|e| match e {
        ERROR_CANCELLED => "richiesta UAC rifiutata (1223): nessun helper avviato".to_owned(),
        e => format!("avvio dell'helper non riuscito: {e}"),
    })?;
    let cancel = AtomicBool::new(false);
    if let Err(e) = server.accept(helper.pid, Duration::from_secs(60), &cancel) {
        let code = helper.wait(Duration::from_secs(5));
        return Err(format!("l'helper (pid {}) non si è collegato: {e:?}; codice di uscita {code:?}", helper.pid));
    }
    server.send_request(&request).map_err(|e| format!("richiesta non inviata: {e:?}"))?;
    let mut replies = Vec::new();
    let mut pipe_error = None;
    loop {
        match server.recv(&cancel) {
            Ok(r) => {
                let done = matches!(r, Reply::Done { .. });
                replies.push(r);
                if done {
                    break;
                }
            }
            Err(PipeError::Closed) => break,
            Err(e) => {
                pipe_error = Some(e);
                break;
            }
        }
    }
    let exit = helper.wait(Duration::from_secs(30));
    Ok(Outcome { replies, pipe_error, exit, elapsed: started.elapsed() })
}

// ---------------------------------------------------------------------------------------------------
// Scansione non elevata e confronti

/// Cartelle negate all'utente trovate dalla scansione non elevata di `root`, ordinate, e la scansione.
fn denied_folders(root: &str) -> Result<(Vec<String>, quado_scan_win::ScanResult), String> {
    let r = scan(root, Options::default(), &Progress::default()).map_err(|e| format!("scansione di {root}: errore {}", e.0))?;
    let mut v: Vec<String> = r
        .problems
        .iter()
        .filter(|(_, p)| *p == Problem::AccessDenied)
        // Un nome non valido in UTF-16 non si trasmette: la richiesta ha percorsi in UTF-8.
        .filter_map(|(n, _)| String::from_utf16(&r.tree.path(*n)).ok())
        .collect();
    v.sort();
    v.dedup();
    Ok((v, r))
}

/// Percorso di `i` relativo alla radice del sottoalbero.
fn rel_path(t: &Tree, i: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut n = i;
    while n != Tree::ROOT {
        parts.push(t.name_lossy(n));
        n = t.parent(n);
    }
    parts.reverse();
    parts.join("\\")
}

fn expected_totals(f: &ExpectedFolder) -> Totals {
    Totals { logical: f.totals.logical, alloc: f.totals.alloc, files: f.totals.files, dirs: f.totals.dirs, unreadable: 0 }
}

/// Totali di un albero ricevuto, contati come `collect::totals_of`.
fn tree_totals(t: &Tree, problems: &[(u32, Problem)]) -> Totals {
    quado_scan_win::helper::collect::totals_of(t, problems)
}

/// Albero dell'helper contro i valori attesi: ogni nome di file con logica e allocata, poi i totali.
fn compare_tree(t: &Tree, problems: &[(u32, Problem)], e: &ExpectedFolder) -> Result<(), String> {
    let mut got: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for i in 1..t.len() as u32 {
        if !t.flags(i).has(Flags::DIR) {
            got.insert(rel_path(t, i).to_ascii_lowercase(), (t.logical(i), t.alloc(i)));
        }
    }
    let mut diffs = Vec::new();
    for f in &e.files {
        match got.remove(&f.rel.to_ascii_lowercase()) {
            Some(v) if v == (f.logical, f.alloc) => {}
            Some(v) => diffs.push(format!("{}: {v:?} invece di {:?}", f.rel, (f.logical, f.alloc))),
            None => diffs.push(format!("{}: manca", f.rel)),
        }
    }
    diffs.extend(got.keys().map(|k| format!("{k}: in più")));
    let tt = tree_totals(t, problems);
    if tt != expected_totals(e) {
        diffs.push(format!("totali {tt:?} invece di {:?}", expected_totals(e)));
    }
    if diffs.is_empty() { Ok(()) } else { Err(diffs.join("; ")) }
}

/// Nomi che non devono comparire nelle risposte senza elenco: componenti dei percorsi del profilo finto e
/// dei Cestini.
fn private_names(e: &Expected) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let folders = std::iter::once(&e.profile).chain(e.bins.iter().map(|b| &b.folder));
    for f in folders {
        for file in &f.files {
            for part in file.rel.split('\\') {
                if part.len() >= 4 && !v.iter().any(|x| x == part) {
                    v.push(part.to_owned());
                }
            }
        }
    }
    v
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

/// Una riga dei controlli del resoconto.
struct Check {
    what: String,
    result: Result<String, String>,
}

impl Check {
    fn ok(what: impl Into<String>, detail: impl Into<String>) -> Self {
        Check { what: what.into(), result: Ok(detail.into()) }
    }
    fn fail(what: impl Into<String>, detail: impl Into<String>) -> Self {
        Check { what: what.into(), result: Err(detail.into()) }
    }
    fn of(what: impl Into<String>, r: Result<String, String>) -> Self {
        Check { what: what.into(), result: r }
    }
}

fn index_of(folders: &[String], path: &str) -> Option<u32> {
    folders.iter().position(|f| f.eq_ignore_ascii_case(path)).map(|i| i as u32)
}

fn reply_index(r: &Reply) -> Option<u32> {
    match r {
        Reply::Folder { index, .. } | Reply::Totals { index, .. } | Reply::Bin { index, .. } | Reply::Failed { index, .. } => Some(*index),
        _ => None,
    }
}

fn reply_for(o: &Outcome, index: u32) -> Option<&Reply> {
    o.replies.iter().find(|r| reply_index(r) == Some(index))
}

fn describe(r: &Reply) -> String {
    match r {
        Reply::Folder { tree, problems, .. } => {
            let t = tree_totals(tree, problems);
            format!("elenco: {} nodi, {} file, allocata {}, problemi {}", tree.len(), t.files, t.alloc, problems.len())
        }
        Reply::Totals { totals, .. } => format!("solo totali: {totals:?}"),
        Reply::Bin { sid, totals, orphan, .. } => format!("Cestino di {sid}, orfano: {orphan}, {totals:?}"),
        Reply::Failed { code, .. } => format!("non aperta: errore {code}"),
        other => format!("{other:?}"),
    }
}

/// Controlli della prova su `R:`.
fn checks_r(e: &Expected, folders: &[String], o: &Outcome) -> Vec<Check> {
    let mut c = Vec::new();
    match o.replies.first() {
        Some(Reply::Started { backup_privilege: true }) => c.push(Check::ok("`Started`, `SeBackupPrivilege` abilitato", "sì")),
        other => c.push(Check::fail("`Started`, `SeBackupPrivilege` abilitato", format!("{other:?}"))),
    }
    let indexed = o.replies.iter().filter(|r| reply_index(r).is_some()).count();
    c.push(if indexed == folders.len() {
        Check::ok("una risposta per cartella", format!("{indexed}"))
    } else {
        Check::fail("una risposta per cartella", format!("{indexed} su {}", folders.len()))
    });

    // Cartella negata: elenco completo, uguale all'oracolo eseguito come utente prima della chiusura.
    let what = format!("`{}`: elenco uguale ai valori attesi", e.denied.path);
    c.push(match index_of(folders, &e.denied.path).and_then(|i| reply_for(o, i)) {
        Some(Reply::Folder { tree, problems, .. }) => Check::of(what, compare_tree(tree, problems, &e.denied).map(|_| format!("{} nomi di file", e.denied.files.len()))),
        other => Check::fail(what, format!("{:?}", other.map(describe))),
    });
    // Profilo finto: solo totali.
    let what = format!("`{}`: solo totali, uguali ai valori attesi", e.profile.path);
    c.push(match index_of(folders, &e.profile.path).and_then(|i| reply_for(o, i)) {
        Some(Reply::Totals { totals, .. }) if *totals == expected_totals(&e.profile) => Check::ok(what, format!("{totals:?}")),
        other => Check::fail(what, format!("{:?} invece di Totals {:?}", other.map(describe), expected_totals(&e.profile))),
    });
    // Cestini: totale per SID, orfano segnato.
    for b in &e.bins {
        let what = format!("`{}`: Cestino di {}, orfano {}", b.folder.path, b.sid, b.orphan);
        c.push(match index_of(folders, &b.folder.path).and_then(|i| reply_for(o, i)) {
            Some(Reply::Bin { sid, totals, orphan, .. }) if sid.eq_ignore_ascii_case(b.sid) && *orphan == b.orphan && *totals == expected_totals(&b.folder) => {
                Check::ok(what, format!("{totals:?}"))
            }
            other => Check::fail(what, format!("{:?} invece di {:?}", other.map(describe), expected_totals(&b.folder))),
        });
    }
    // Nessun nome del profilo o dei Cestini nelle risposte senza elenco (ricodificate come sulla pipe).
    let names = private_names(e);
    let mut leaks = Vec::new();
    for r in o.replies.iter().filter(|r| !matches!(r, Reply::Folder { .. })) {
        let bytes = proto::encode_reply(r);
        for n in &names {
            let utf16: Vec<u8> = n.encode_utf16().flat_map(u16::to_le_bytes).collect();
            if contains(&bytes, n.as_bytes()) || contains(&bytes, &utf16) {
                leaks.push(n.clone());
            }
        }
    }
    c.push(if leaks.is_empty() {
        Check::ok("nessun nome del profilo né dei Cestini nelle risposte senza elenco", format!("{} nomi cercati", names.len()))
    } else {
        Check::fail("nessun nome del profilo né dei Cestini nelle risposte senza elenco", leaks.join(", "))
    });
    // Copie shadow: su un VHDX "non disponibile", senza errori.
    c.push(match o.replies.iter().find_map(|r| if let Reply::Shadow(s) = r { Some(*s) } else { None }) {
        Some(Ok(None)) => Check::ok("copie shadow di R:: `Ok(None)`", "nessuna associazione per il volume"),
        // Servizio Copia shadow del volume disattivato (questo PC): "non disponibile", con il motivo.
        Some(Err(1058)) => Check::ok("copie shadow di R:: `Ok(None)`", "non disponibili: servizio Copia shadow del volume disattivato (1058)"),
        other => Check::fail("copie shadow di R:: `Ok(None)`", format!("{other:?}")),
    });
    c.extend(end_checks(o));
    c
}

/// `Done { code: 0 }`, nessun errore della pipe, uscita 0.
fn end_checks(o: &Outcome) -> Vec<Check> {
    let mut c = Vec::new();
    c.push(match o.replies.last() {
        Some(Reply::Done { code: 0 }) => Check::ok("`Done` con codice 0, ultimo messaggio", "sì"),
        other => Check::fail("`Done` con codice 0, ultimo messaggio", format!("{other:?}")),
    });
    c.push(match o.pipe_error {
        None => Check::ok("nessun errore della pipe", "sì"),
        Some(e) => Check::fail("nessun errore della pipe", format!("{e:?}")),
    });
    c.push(match o.exit {
        Some(0) => Check::ok("l'helper esce da solo con 0", format!("{:.1} s dall'avvio", o.elapsed.as_secs_f64())),
        other => Check::fail("l'helper esce da solo con 0", format!("{other:?}")),
    });
    c
}

fn checks_table(c: &[Check]) -> String {
    let mut s = String::from("| Controllo | Esito | Dettaglio |\n|---|---|---|\n");
    for k in c {
        let (mark, d) = match &k.result {
            Ok(d) => ("passato", d),
            Err(d) => ("FALLITO", d),
        };
        let _ = writeln!(s, "| {} | {mark} | {} |", k.what, d.replace('|', "/"));
    }
    s
}

fn doc_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("target").join("misure").join(name)
}

fn write_doc(name: &str, text: &str) -> Result<(), String> {
    let p = doc_path(name);
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&p, text).map_err(|e| e.to_string())?;
    eprintln!("scritto target/misure/{name}");
    Ok(())
}

fn conditions() -> String {
    let mut s = String::from("| Condizione | Valore |\n|---|---|\n");
    for (k, v) in crate::measure_cmd::environment().into_iter().filter(|(k, _)| matches!(*k, "Data" | "Windows" | "Versione di Quado")) {
        let _ = writeln!(s, "| {k} | {v} |");
    }
    s
}

// ---------------------------------------------------------------------------------------------------
// Prova su R:

/// Toglie il contenuto di prova anche se la prova si interrompe.
struct Clean;

impl Drop for Clean {
    fn drop(&mut self) {
        match content::cleanup() {
            Ok(()) => eprintln!("contenuto di prova tolto da R:"),
            Err(e) => eprintln!("pulizia di R: incompleta: {e} (riprovare con `cargo xtask helper-prova pulisci`)"),
        }
    }
}

fn test_r(elevated: bool) -> Result<(), String> {
    if elevated {
        // Prima di preparare: senza l'eseguibile non si arriva al prompt.
        helper_exe()?;
    }
    let vol = content::volume();
    let vol_s = vol.to_string_lossy().into_owned();
    let _clean = Clean;
    let e = content::prepare();
    let (folders, _) = denied_folders(&vol_s)?;
    eprintln!("cartelle negate su {vol_s}: {folders:#?}");
    let mut pre = Vec::new();
    for f in e.folders() {
        pre.push(match index_of(&folders, &f.path) {
            Some(_) => Check::ok(format!("`{}` negata alla scansione non elevata", f.path), "sì"),
            None => Check::fail(format!("`{}` negata alla scansione non elevata", f.path), "non trovata tra i problemi"),
        });
    }
    let request = Request { volume: vol_s.clone(), folders: folders.clone() };
    pre.push(Check::of("richiesta valida per l'helper", request.validate().map(|_| format!("{} cartelle", folders.len())).map_err(|e| format!("{e:?}"))));
    if !elevated {
        println!("{}\n{}", e.to_markdown(), checks_table(&pre));
        return if pre.iter().all(|c| c.result.is_ok()) { Ok(()) } else { Err("controlli prima della richiesta falliti".into()) };
    }
    if let Some(bad) = pre.iter().find(|c| c.result.is_err()) {
        return Err(format!("controllo prima della richiesta fallito, nessun prompt UAC: {} ({:?})", bad.what, bad.result));
    }
    let o = run_helper(&vol_s, &folders)?;
    let mut checks = pre;
    checks.extend(checks_r(&e, &folders, &o));
    let failed = checks.iter().filter(|c| c.result.is_err()).count();

    let mut doc = String::new();
    let _ = writeln!(doc, "# Prova dell'helper elevato su R:\n");
    let _ = writeln!(
        doc,
        "Generato da `cargo xtask helper-prova`: una richiesta UAC, helper `target\\release\\quado.exe --helper <pipe> <pid di xtask>` avviato con `ShellExecuteExW` e il verbo `runas`. Il contenuto di prova (`quado_testkit::helper`) si crea senza elevazione, si misura con l'oracolo come utente prima di chiudere le ACL e si toglie alla fine.\n"
    );
    doc.push_str(&conditions());
    let _ = writeln!(doc, "\n## Valori attesi (oracolo, prima della chiusura)\n\n{}", e.to_markdown());
    let _ = writeln!(doc, "## Richiesta\n\nVolume `{vol_s}`, {} cartelle negate trovate dalla scansione non elevata:\n", folders.len());
    for (i, f) in folders.iter().enumerate() {
        let r = reply_for(&o, i as u32).map(describe).unwrap_or_else(|| "nessuna risposta".into());
        let _ = writeln!(doc, "- {i}: `{f}`: {r}");
    }
    let shadow = o.replies.iter().find_map(|r| if let Reply::Shadow(s) = r { Some(*s) } else { None });
    let _ = writeln!(doc, "\nCopie shadow: {shadow:?}. Messaggi ricevuti: {}. Codice di uscita dell'helper: {:?}.\n", o.replies.len(), o.exit);
    let _ = writeln!(doc, "## Controlli\n\n{}", checks_table(&checks));
    let _ = writeln!(doc, "Esito: {}.", if failed == 0 { "tutti i controlli passati".to_owned() } else { format!("{failed} controlli falliti") });
    write_doc("helper-prova.md", &doc)?;
    println!("{doc}");
    if failed == 0 { Ok(()) } else { Err(format!("{failed} controlli falliti")) }
}

// ---------------------------------------------------------------------------------------------------
// Prova su C:

/// Primo componente dopo il volume (`Windows`, `ProgramData`, `Users`...): mai il nome di un profilo.
fn group_of(volume: &str, path: &str) -> String {
    path.get(volume.len()..).unwrap_or(path).split('\\').next().unwrap_or("").to_owned()
}

#[derive(Default)]
struct Sum {
    folders: u32,
    logical: u64,
    alloc: u64,
    files: u64,
    unreadable: u64,
}

impl Sum {
    fn add(&mut self, t: &Totals) {
        self.folders += 1;
        self.logical += t.logical;
        self.alloc += t.alloc;
        self.files += t.files;
        self.unreadable += u64::from(t.unreadable);
    }
}

fn gib(b: u64) -> String {
    format!("{:.2} GiB", b as f64 / f64::from(1 << 30))
}

fn shadow_text(s: &Option<Result<Option<ShadowStorage>, u32>>) -> String {
    match s {
        Some(Ok(Some(v))) => format!(
            "usato {} ({} byte), allocato {} ({} byte), massimo {}",
            gib(v.used),
            v.used,
            gib(v.allocated),
            v.allocated,
            if v.max == u64::MAX { "senza limite".to_owned() } else { format!("{} ({} byte)", gib(v.max), v.max) }
        ),
        Some(Ok(None)) => "nessuno spazio per le copie shadow sul volume".to_owned(),
        Some(Err(c)) => format!("errore {c:#x}"),
        None => "nessuna risposta".to_owned(),
    }
}

fn test_c() -> Result<(), String> {
    helper_exe()?;
    let vol = r"C:\";
    eprintln!("scansione non elevata di C: (qualche decina di secondi)…");
    let (mut folders, scanned) = denied_folders(vol)?;
    // Una cartella che la validazione dell'helper rifiuterebbe (percorso troppo lungo, caratteri non
    // ammessi) si toglie qui: altrimenti l'helper rifiuterebbe tutta la richiesta.
    let before = folders.len();
    folders.retain(|f| Request { volume: vol.to_owned(), folders: vec![f.clone()] }.validate().is_ok());
    let rejected = before - folders.len();
    let truncated = folders.len().saturating_sub(proto::MAX_FOLDERS);
    folders.truncate(proto::MAX_FOLDERS);
    let unattributed = scanned.balance.map(|b| b.unattributed);
    let drift = scanned.balance.map(|b| b.drift);
    drop(scanned);
    let o = run_helper(vol, &folders)?;

    // Riepilogo per ambito e per gruppo, senza nomi sotto i profili e senza SID.
    let mut trees: BTreeMap<String, Sum> = BTreeMap::new();
    let (mut profiles, mut bins, mut orphan_bins) = (Sum::default(), Sum::default(), Sum::default());
    let mut failed: BTreeMap<u32, u32> = BTreeMap::new();
    // File con più nomi nei sottoalberi: se un altro nome sta fuori dalla cartella, l'innesto li conta
    // due volte fino alla scansione completa successiva (`Tree::graft`). Limite superiore del doppio.
    let (mut multi_files, mut multi_alloc) = (0u64, 0u64);
    for r in &o.replies {
        match r {
            Reply::Folder { index, tree, problems } => {
                for n in 0..tree.len() as u32 {
                    let f = tree.flags(n);
                    if f.has(Flags::MULTI_LINK) && !f.has(Flags::SECONDARY_LINK) && !f.has(Flags::DIR) {
                        multi_files += 1;
                        multi_alloc += tree.alloc(n);
                    }
                }
                let g = folders.get(*index as usize).map(|p| group_of(vol, p)).unwrap_or_default();
                trees.entry(g).or_default().add(&tree_totals(tree, problems));
            }
            Reply::Totals { totals, .. } => profiles.add(totals),
            Reply::Bin { totals, orphan, .. } => {
                bins.add(totals);
                if *orphan {
                    orphan_bins.add(totals);
                }
            }
            Reply::Failed { code, .. } => *failed.entry(*code).or_default() += 1,
            _ => {}
        }
    }
    let measured: u64 = trees.values().map(|s| s.alloc).sum::<u64>() + profiles.alloc + bins.alloc;
    let shadow = o.replies.iter().find_map(|r| if let Reply::Shadow(s) = r { Some(*s) } else { None });
    let mut checks = vec![match o.replies.first() {
        Some(Reply::Started { backup_privilege: true }) => Check::ok("`SeBackupPrivilege` abilitato", "sì"),
        other => Check::fail("`SeBackupPrivilege` abilitato", format!("{other:?}")),
    }];
    checks.push(match shadow {
        Some(Ok(_)) => Check::ok("copie shadow lette con WMI", shadow_text(&shadow)),
        // Servizio Copia shadow del volume disattivato: Windows non dà il dato, e lo si dice.
        Some(Err(1058)) => Check::ok("copie shadow lette con WMI", "non disponibili: servizio Copia shadow del volume disattivato (1058)".to_owned()),
        _ => Check::fail("copie shadow lette con WMI", shadow_text(&shadow)),
    });
    checks.extend(end_checks(&o));
    let bad = checks.iter().filter(|c| c.result.is_err()).count();

    let mut doc = String::new();
    let _ = writeln!(doc, "# Prova dell'helper elevato su C:\n");
    let _ = writeln!(
        doc,
        "Generato da `cargo xtask helper-prova --c`, con il consenso di chi usa il PC: una richiesta UAC; le cartelle sono quelle negate alla scansione non elevata di C:, le copie shadow si leggono davvero. Solo totali: nessun nome di cartella sotto i profili, nessun SID. Gruppi = primo componente del percorso dopo `C:\\`.\n"
    );
    doc.push_str(&conditions());
    let _ = writeln!(
        doc,
        "\n## Richiesta\n\n{} cartelle negate; non chieste: {rejected} rifiutate dalla validazione, {truncated} oltre il massimo del protocollo.\n",
        folders.len()
    );
    let _ = writeln!(doc, "## Cartelle elencate dall'helper, per gruppo\n\n| Gruppo | Cartelle | Logica | Allocata | File | Sottocartelle non leggibili |\n|---|---:|---:|---:|---:|---:|");
    for (g, s) in &trees {
        let _ = writeln!(doc, "| `{g}` | {} | {} | {} | {} | {} |", s.folders, gib(s.logical), gib(s.alloc), s.files, s.unreadable);
    }
    let _ = writeln!(doc, "\n## Profili e Cestini di altri account (solo totali)\n\n| Ambito | Cartelle | Logica | Allocata | File |\n|---|---:|---:|---:|---:|");
    for (k, s) in [("Profili altrui", &profiles), ("Cestini di altri SID", &bins), ("di cui SID orfani", &orphan_bins)] {
        let _ = writeln!(doc, "| {k} | {} | {} | {} | {} |", s.folders, gib(s.logical), gib(s.alloc), s.files);
    }
    let _ = writeln!(doc, "\nNon aperte neanche dall'helper: {}.\n", if failed.is_empty() { "nessuna".to_owned() } else { failed.iter().map(|(c, n)| format!("{n} con errore {c}")).collect::<Vec<_>>().join(", ") });
    let _ = writeln!(doc, "## Copie shadow e bilancio\n\n- Copie shadow di C:: {}.", shadow_text(&shadow));
    let _ = writeln!(doc, "- Allocato misurato dall'helper: {} ({measured} byte).", gib(measured));
    let _ = writeln!(doc, "- Di cui file con più nomi nei sottoalberi: {multi_files} file, {} ({multi_alloc} byte). Se un altro nome sta fuori dalla cartella, l'innesto li conta due volte fino alla scansione completa successiva: è il limite superiore del doppio conteggio.", gib(multi_alloc));
    if let Some(u) = unattributed {
        let _ = writeln!(doc, "- Non attribuito della scansione non elevata: {} ({u} byte); deriva durante la scansione {} byte. Le copie shadow stanno in `System Volume Information`: se l'helper l'ha elencata sono già nell'allocato misurato.", gib(u.max(0) as u64), drift.unwrap_or(0));
        let _ = writeln!(doc, "- Non attribuito dopo l'helper, come lo calcola l'interfaccia: {} byte.", u - measured as i64);
    }
    let _ = writeln!(doc, "\n## Controlli\n\n{}", checks_table(&checks));
    let _ = writeln!(doc, "Esito: {}.", if bad == 0 { "tutti i controlli passati".to_owned() } else { format!("{bad} controlli falliti") });
    write_doc("helper-prova-c.md", &doc)?;
    println!("{doc}");
    if bad == 0 { Ok(()) } else { Err(format!("{bad} controlli falliti")) }
}
