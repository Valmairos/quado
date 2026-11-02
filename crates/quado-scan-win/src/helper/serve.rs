// SPDX-License-Identifier: GPL-3.0-or-later
//! Modalità helper dell'eseguibile: `quado --helper <pipe> <pid del genitore>`.
//!
//! Ordine: rifiuto se l'eseguibile sta in `Downloads` o in una cartella temporanea; controllo che il pid
//! passato sia davvero il genitore; connessione alla pipe ([`super::pipe::Client`]); lettura e
//! validazione della richiesta; `SeBackupPrivilege`; una risposta per cartella secondo il suo
//! [`super::proto::Scope`]; copie shadow; `Done`; uscita. Nessun file aperto in scrittura.
//!
//! La raccolta passa dal tratto [`Collector`]: [`super::collect::Live`] nel prodotto, uno finto nei test
//! del protocollo. Dopo la connessione tutto il lavoro sta in [`serve_connected`], che i test chiamano
//! direttamente con un client collegato a un server dello stesso processo.

use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{FILETIME, HANDLE};
use windows_sys::Win32::Globalization::{CSTR_EQUAL, CompareStringOrdinal};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_NO_RECALL, FILE_FLAG_OPEN_REPARSE_POINT, FILE_NAME_NORMALIZED,
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFinalPathNameByHandleW, GetTempPathW,
    OPEN_EXISTING, SYNCHRONIZE, VOLUME_NAME_DOS,
};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_sys::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Shell::{FOLDERID_Downloads, FOLDERID_LocalAppData, SHGetKnownFolderPath};
use windows_sys::core::GUID;

use super::pipe::{self, Client, Owned, PipeError};
use super::proto::{ProtoError, Reply, Scope, ShadowStorage, Totals, classify};
use crate::scan::{Problem, RootError};
use crate::win;
use quado_model::Tree;

/// Codici di uscita del processo helper.
pub mod exit {
    pub const OK: i32 = 0;
    /// Argomenti mancanti o non validi.
    pub const ARGS: i32 = 2;
    /// Eseguibile in `Downloads` o in una cartella temporanea.
    pub const LOCATION: i32 = 3;
    /// Il pid passato non è il genitore, o il server della pipe non è quel processo.
    pub const PEER: i32 = 4;
    /// Pipe non aperta o chiusa prima della fine.
    pub const PIPE: i32 = 5;
    /// Richiesta rifiutata dalla validazione.
    pub const REQUEST: i32 = 6;
}

/// Sottoalbero elencato dall'helper.
pub struct FolderScan {
    pub tree: Tree,
    pub problems: Vec<(u32, Problem)>,
}

/// Raccolta dei dati con i privilegi. Ogni metodo è in sola lettura.
pub trait Collector {
    /// Abilita `SeBackupPrivilege` nel token del processo.
    fn enable_backup_privilege(&self) -> Result<(), u32>;
    /// Elenca `path` con lo scanner, radice compresa.
    fn folder(&self, path: &str) -> Result<FolderScan, RootError>;
    /// Elenca `path` e restituisce solo i totali: i nomi non escono da questa funzione.
    fn totals(&self, path: &str) -> Result<Totals, RootError>;
    /// Il SID non corrisponde più a un account (`LookupAccountSidW` → `ERROR_NONE_MAPPED`).
    fn sid_is_orphan(&self, sid: &str) -> Result<bool, u32>;
    /// `Win32_ShadowStorage` del volume (`X:\`).
    fn shadow_storage(&self, volume: &str) -> Result<Option<ShadowStorage>, u32>;
    /// Cartella del profilo del SID (`ProfileList\<sid>\ProfileImagePath`), se registrata.
    fn profile_of(&self, sid: &str) -> Option<String>;
}

/// `ERROR_FILE_TOO_LARGE`: il sottoalbero di una cartella supera `MAX_REPLY_BYTES` e non si trasmette.
pub const FOLDER_TOO_LARGE: u32 = 223;

/// Punto d'ingresso: `args` sono gli argomenti dopo `--helper`. Restituisce il codice di uscita.
pub fn run(args: &[String]) -> i32 {
    run_with(args, &super::collect::Live)
}

/// Come [`run`], con una raccolta iniettata (test).
pub fn run_with(args: &[String], collector: &dyn Collector) -> i32 {
    let [pipe_name, pid] = args else { return exit::ARGS };
    let Ok(pid) = pid.parse::<u32>() else { return exit::ARGS };
    if !pipe::valid_name(pipe_name) {
        return exit::ARGS;
    }
    if exe_location_refused() {
        return exit::LOCATION;
    }
    // L'handle del genitore resta aperto fino alla fine: finché c'è, il suo pid non si riusa.
    let Some(_parent) = parent_process(pid) else { return exit::PEER };
    let client = match Client::connect(pipe_name, pid) {
        Ok(c) => c,
        Err(PipeError::WrongPeer(_)) => return exit::PEER,
        Err(_) => return exit::PIPE,
    };
    serve_connected(client, collector)
}

/// Il lavoro dopo la connessione: legge la richiesta (se non è valida esce con [`exit::REQUEST`] senza
/// rispondere), poi `Started`, una risposta per cartella, `Shadow`, `Done`. Pipe rotta: [`exit::PIPE`].
pub fn serve_connected(mut client: Client, collector: &dyn Collector) -> i32 {
    let req = match client.read_request() {
        Ok(r) => r,
        Err(PipeError::Proto(_) | PipeError::Timeout) => return exit::REQUEST,
        Err(_) => return exit::PIPE,
    };
    match answer(&mut client, &req.volume, &req.folders, collector) {
        Ok(()) => exit::OK,
        Err(_) => exit::PIPE,
    }
}

fn answer(client: &mut Client, volume: &str, folders: &[String], collector: &dyn Collector) -> Result<(), PipeError> {
    let privilege = collector.enable_backup_privilege();
    client.send(&Reply::Started { backup_privilege: privilege.is_ok() })?;
    let own_sid = client.server_user_sid();
    let own_profile = own_sid.as_deref().and_then(|s| collector.profile_of(s));
    for (i, folder) in folders.iter().enumerate() {
        let index = i as u32;
        let reply = match scope_of(volume, folder, own_profile.as_deref(), own_sid.as_deref()) {
            Scope::Tree => match collector.folder(folder) {
                Ok(f) => Reply::Folder { index, tree: Box::new(f.tree), problems: f.problems },
                Err(RootError(code)) => Reply::Failed { index, code },
            },
            Scope::Profile => match collector.totals(folder) {
                Ok(totals) => Reply::Totals { index, totals },
                Err(RootError(code)) => Reply::Failed { index, code },
            },
            Scope::Bin { sid } => match collector.totals(folder) {
                // Se la ricerca dell'account non riesce il SID non si dichiara orfano.
                Ok(totals) => Reply::Bin { index, orphan: collector.sid_is_orphan(&sid).unwrap_or(false), sid, totals },
                Err(RootError(code)) => Reply::Failed { index, code },
            },
        };
        match client.send(&reply) {
            Err(PipeError::Proto(ProtoError::TooLarge)) => client.send(&Reply::Failed { index, code: FOLDER_TOO_LARGE })?,
            r => r?,
        }
    }
    client.send(&Reply::Shadow(collector.shadow_storage(volume)))?;
    // Errore grave: senza il privilegio le cartelle negate restano tali (le risposte lo dicono già una
    // per una, `Done` lo riassume).
    client.send(&Reply::Done { code: privilege.err().unwrap_or(0) })
}

/// Ambito effettivo di una cartella: [`classify`] sul percorso chiesto e, se quello risulta un albero
/// con i nomi, anche sul percorso finale (`GetFinalPathNameByHandleW`, junction intermedie seguite come
/// le segue lo scanner, nomi brevi 8.3 espansi, maiuscole del disco). Vince il più restrittivo: un
/// alias come `C:\$RECYC~1.BIN\S-...` o una junction verso `Users\<altro>` non fa uscire i nomi.
///
/// Se il percorso non si apre vale il solo percorso chiesto: la raccolta fallirà allo stesso modo.
/// Limite: fra questo controllo e l'elenco un processo dell'utente potrebbe sostituire una cartella
/// intermedia con una junction; la difesa piena è un elenco per handle, nella raccolta.
pub fn scope_of(volume: &str, folder: &str, own_profile: Option<&str>, own_sid: Option<&str>) -> Scope {
    let asked = classify(volume, folder, own_profile, own_sid);
    if asked != Scope::Tree {
        return asked;
    }
    let Some(fin) = final_path(folder, false) else { return asked };
    let own_final = own_profile.map(|p| final_path(p, true).unwrap_or_else(|| p.to_owned()));
    classify(volume, &fin, own_final.as_deref(), own_sid)
}

// ---------------------------------------------------------------------------------------------------
// Percorsi

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Stringa terminata da zero scritta da Windows.
///
/// # Safety
/// `p` punta a una stringa UTF-16 terminata da zero, valida per tutta la chiamata.
unsafe fn from_wide_z(p: *const u16) -> String {
    let mut n = 0;
    // SAFETY: la stringa è terminata da zero (contratto della funzione).
    while unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    // SAFETY: le `n` unità prima dello zero sono leggibili.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, n) })
}

/// Percorso confrontabile: senza `\\?\` (o con `\\` al posto di `\\?\UNC\`) e senza `\` finale.
fn plain(p: &str) -> String {
    let p = match p.strip_prefix(r"\\?\UNC\") {
        Some(rest) => format!(r"\\{rest}"),
        None => p.strip_prefix(r"\\?\").unwrap_or(p).to_owned(),
    };
    p.trim_end_matches('\\').to_owned()
}

/// Percorso finale (`FILE_NAME_NORMALIZED`, lettera del volume) di `path`, aperto per i soli attributi
/// con `FILE_FLAG_BACKUP_SEMANTICS` e `\\?\` come lo apre lo scanner. `follow`: segue anche un reparse
/// point sull'ultimo componente (eseguibile, cartelle vietate); senza, come lo scanner, si ferma lì.
pub fn final_path(path: &str, follow: bool) -> Option<String> {
    let z = win::wide_z(&win::long_path(&wide(path)));
    let flags = FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_NO_RECALL | (if follow { 0 } else { FILE_FLAG_OPEN_REPARSE_POINT });
    // SAFETY: percorso terminato da zero; accesso ai soli attributi (nessuna lettura del contenuto,
    // nessuna scrittura); l'handle si chiude al drop di `Owned`.
    let h = unsafe {
        CreateFileW(z.as_ptr(), FILE_READ_ATTRIBUTES | SYNCHRONIZE, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, null(), OPEN_EXISTING, flags, null_mut())
    };
    let h = Owned::new(h)?;
    let mut buf = vec![0u16; 512];
    loop {
        // SAFETY: handle valido; buffer locale con la sua lunghezza in caratteri.
        let n = unsafe { GetFinalPathNameByHandleW(h.raw(), buf.as_mut_ptr(), buf.len() as u32, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS) } as usize;
        if n == 0 || n > 1 << 16 {
            return None;
        }
        // Buffer piccolo: `n` è la lunghezza necessaria, zero compreso.
        if n >= buf.len() {
            buf.resize(n + 1, 0);
            continue;
        }
        return String::from_utf16(&buf[..n]).ok().map(|s| plain(&s));
    }
}

/// `path` è `dir` o sta dentro `dir`, senza maiuscole (`CompareStringOrdinal`, come i nomi di NTFS).
fn within(path: &str, dir: &str) -> bool {
    let (p, d) = (wide(path), wide(dir));
    if d.is_empty() || p.len() < d.len() || (p.len() > d.len() && p[d.len()] != u16::from(b'\\')) {
        return false;
    }
    // SAFETY: entrambi i buffer hanno almeno `d.len()` unità; le lunghezze sono esplicite.
    unsafe { CompareStringOrdinal(p.as_ptr(), d.len() as i32, d.as_ptr(), d.len() as i32, 1) == CSTR_EQUAL }
}

/// `path` sta in una delle cartelle `dirs` (o è una di esse).
pub fn inside_any(path: &str, dirs: &[String]) -> bool {
    let p = plain(path);
    dirs.iter().any(|d| within(&p, d))
}

fn known_folder(id: &GUID) -> Option<String> {
    let mut p: *mut u16 = null_mut();
    // SAFETY: GUID costante; token nullo = utente corrente; Windows alloca `p` con CoTaskMemAlloc.
    let hr = unsafe { SHGetKnownFolderPath(id, 0, null_mut(), &mut p) };
    // SAFETY: con esito positivo `p` è una stringa terminata da zero.
    let s = (hr >= 0 && !p.is_null()).then(|| unsafe { from_wide_z(p) });
    // SAFETY: va liberato anche se la chiamata non riesce; CoTaskMemFree accetta un puntatore nullo.
    unsafe { CoTaskMemFree(p.cast()) };
    s
}

fn temp_path() -> Option<String> {
    let mut buf = vec![0u16; 1024];
    // SAFETY: buffer locale con la sua lunghezza in caratteri.
    let n = unsafe { GetTempPathW(buf.len() as u32, buf.as_mut_ptr()) } as usize;
    (n > 0 && n < buf.len()).then(|| String::from_utf16_lossy(&buf[..n]))
}

fn windows_dir() -> Option<String> {
    let mut buf = vec![0u16; 1024];
    // SAFETY: buffer locale con la sua lunghezza in caratteri.
    let n = unsafe { GetSystemWindowsDirectoryW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    (n > 0 && n < buf.len()).then(|| String::from_utf16_lossy(&buf[..n]))
}

fn module_path() -> Option<String> {
    let mut buf = vec![0u16; 1024];
    loop {
        // SAFETY: modulo nullo = l'eseguibile del processo; buffer locale con la sua lunghezza.
        let n = unsafe { GetModuleFileNameW(null_mut(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if n == 0 {
            return None;
        }
        // Troncato: il risultato riempie il buffer.
        if n >= buf.len() {
            if buf.len() >= 1 << 16 {
                return None;
            }
            buf.resize(buf.len() * 2, 0);
            continue;
        }
        return Some(String::from_utf16_lossy(&buf[..n]));
    }
}

/// Cartelle da cui l'helper non parte, perché lì una DLL esca può finire accanto all'eseguibile:
/// `Downloads` dell'utente, la cartella temporanea (`GetTempPathW`), `%LOCALAPPDATA%\Temp`,
/// `%SystemRoot%\Temp` e `%SystemRoot%\SystemTemp`, ciascuna anche come percorso finale (junction e
/// reindirizzamenti risolti, nomi brevi espansi).
///
/// `GetTempPath2W` non si usa: per un processo che non è SYSTEM dà lo stesso risultato e su Windows 10
/// senza aggiornamenti recenti manca, e la sua importazione impedirebbe l'avvio dell'eseguibile.
pub fn forbidden_dirs() -> Vec<String> {
    let mut base = Vec::new();
    base.extend(known_folder(&FOLDERID_Downloads));
    base.extend(known_folder(&FOLDERID_LocalAppData).map(|l| format!(r"{}\Temp", plain(&l))));
    base.extend(temp_path());
    if let Some(w) = windows_dir() {
        let w = plain(&w);
        base.push(format!(r"{w}\Temp"));
        base.push(format!(r"{w}\SystemTemp"));
    }
    let mut all: Vec<String> = Vec::new();
    for d in base {
        let p = plain(&d);
        all.extend(final_path(&p, true));
        all.push(p);
    }
    all.retain(|d| !d.is_empty());
    all.sort();
    all.dedup();
    all
}

/// L'eseguibile (percorso del modulo e percorso finale) sta in una cartella di [`forbidden_dirs`]. Se il
/// percorso non si legge, rifiuto.
pub fn exe_location_refused() -> bool {
    let Some(module) = module_path() else { return true };
    let mut paths = vec![plain(&module)];
    paths.extend(final_path(&module, true));
    let dirs = forbidden_dirs();
    paths.iter().any(|p| inside_any(p, &dirs))
}

// ---------------------------------------------------------------------------------------------------
// Genitore

/// Handle del processo genitore verificato, tenuto aperto: finché esiste il pid non si riusa.
pub struct Parent {
    _handle: Owned,
}

fn creation_time(process: HANDLE) -> Option<i64> {
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: handle di processo valido con almeno PROCESS_QUERY_LIMITED_INFORMATION; strutture locali.
    let ok = unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) };
    (ok != 0).then(|| (i64::from(created.dwHighDateTime) << 32) | i64::from(created.dwLowDateTime))
}

/// Pid del genitore del processo corrente, dall'istantanea dei processi (Toolhelp).
fn parent_pid() -> Option<u32> {
    // SAFETY: istantanea dei soli processi; l'handle si chiude al drop di `Owned`.
    let snap = Owned::new(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) })?;
    // SAFETY: nessun prerequisito.
    let me = unsafe { GetCurrentProcessId() };
    let mut e = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    // SAFETY: istantanea valida; `e` ha `dwSize` impostato come richiesto.
    let mut ok = unsafe { Process32FirstW(snap.raw(), &mut e) };
    while ok != 0 {
        if e.th32ProcessID == me {
            return Some(e.th32ParentProcessID);
        }
        // SAFETY: come sopra.
        ok = unsafe { Process32NextW(snap.raw(), &mut e) };
    }
    None
}

/// `pid` è il genitore del processo corrente (Toolhelp) ed è stato creato prima di questo processo (un
/// processo creato dopo avrebbe riusato il pid di un genitore già uscito). `None` altrimenti.
pub fn parent_process(pid: u32) -> Option<Parent> {
    if parent_pid()? != pid {
        return None;
    }
    // SAFETY: apertura in sola interrogazione; l'handle si chiude al drop di `Owned`.
    let parent = Owned::new(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) })?;
    // SAFETY: pseudo-handle del processo corrente.
    let own = creation_time(unsafe { GetCurrentProcess() })?;
    (creation_time(parent.raw())? <= own).then_some(Parent { _handle: parent })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_handles_prefixes() {
        assert_eq!(plain(r"\\?\C:\Users\io\"), r"C:\Users\io");
        assert_eq!(plain(r"\\?\UNC\srv\share\x"), r"\\srv\share\x");
    }

    #[test]
    fn within_is_case_insensitive_and_respects_components() {
        assert!(within(r"C:\Users\Io\Downloads\x.exe", r"c:\users\io\downloads"));
        assert!(within(r"C:\Users\Io\Downloads", r"C:\Users\Io\Downloads"));
        assert!(!within(r"C:\Users\Io\Downloads2\x.exe", r"C:\Users\Io\Downloads"));
        assert!(!within(r"C:\Users", r"C:\Users\Io"));
        assert!(within(r"C:\UNIVERSITÀ\x", r"C:\università"));
    }
}
