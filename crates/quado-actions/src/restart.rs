// SPDX-License-Identifier: GPL-3.0-or-later
//! Quali processi usano dei file (F-DL-6, F-AC-8). Sola lettura, mai `RmShutdown` né `RmRestart`.
//!
//! Due strade, misurate il 14/10/2026 (su `R:` e sulle versioni di CapCut su C:):
//! - **Restart Manager** (`RmRegisterResources`, `RmGetList`) per i file comuni: una sessione per
//!   chiamata, con un costo quasi indipendente dal numero di file (oltre trentamila file in 30-51 s a cache calda).
//! - **Domanda diretta a NTFS** (`NtQueryInformationFile` con `FileProcessIdsUsingFileInformation`, con un
//!   handle dei soli attributi) per i file su cui Restart Manager non è innocuo: i file compressi da WOF,
//!   che Restart Manager decomprime (il volume perde lo spazio della compressione), e i segnaposto cloud,
//!   per cui chiede al provider i dati (idratazione); più i percorsi di 260 caratteri o più senza nome
//!   breve, che Restart Manager rifiuta. La domanda diretta non decomprime, non scarica e non cambia
//!   l'ultimo accesso, ma costa circa 28 ms per file (Windows scorre tutti gli handle del sistema): per
//!   questo non vale per tutti.
//!
//! Per sapere la strada di un file si apre con i soli attributi (`FILE_FLAG_OPEN_REPARSE_POINT`,
//! `FILE_FLAG_OPEN_NO_RECALL`): attributi e tag di reparse dicono se è un segnaposto,
//! `FSCTL_GET_EXTERNAL_BACKING` se è un file WOF (gli attributi non lo mostrano).
//!
//! Nella domanda diretta chi tiene un file per un attimo non conta: Defender (`MsMpEng.exe`) apre per
//! qualche centinaio di millisecondi i file appena scritti; restano i processi presenti in
//! [`CONFIRMATIONS`] domande successive a [`CONFIRM_EVERY`] di distanza. Il nome è quello
//! dell'eseguibile (dall'istantanea dei processi per quelli protetti); il servizio non si conosce.
//!
//! L'elenco è una fotografia: un file chiuso subito dopo, o aperto subito dopo, non cambia la risposta già
//! data.

use std::collections::HashMap;
use std::time::Duration;

use quado_scan_win::{content, win};
use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
use windows::Win32::System::RestartManager::{CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources, RmStartSession};
use windows::core::{PCWSTR, PWSTR};
use windows_sys::Wdk::Storage::FileSystem::{FileProcessIdsUsingFileInformation, NtQueryInformationFile};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INVALID_NAME, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_NO_RECALL, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FileAttributeTagInfo, GetFileInformationByHandleEx, OPEN_EXISTING,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
use windows_sys::Win32::System::IO::{DeviceIoControl, IO_STATUS_BLOCK};
use windows_sys::Win32::System::Ioctl::FSCTL_GET_EXTERNAL_BACKING;
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};

/// Un processo che usa almeno uno dei file chiesti.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProcessUse {
    pub pid: u32,
    /// Nome dell'applicazione: quello di Restart Manager (descrizione del programma o nome dell'eseguibile,
    /// oppure nome visualizzato del servizio) o, nella domanda diretta, il nome dell'eseguibile.
    pub app_name: String,
    /// Nome breve del servizio, se il processo è un servizio (solo da Restart Manager).
    pub service: Option<String>,
}

/// Fase non riuscita, con il codice di Windows. Codici, non frasi (05 D13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RmError {
    /// Sessione di Restart Manager non aperta.
    Session(u32),
    /// Registrazione dei file non riuscita (Restart Manager), o file non apribile per leggerne gli attributi.
    Register(u32),
    /// Elenco dei processi non riuscito (`RmGetList`, o `NTSTATUS` della domanda diretta).
    List(u32),
}

impl RmError {
    /// Codice di errore di Windows, qualunque sia la fase (per `FileUse::Unknown`).
    pub fn code(self) -> u32 {
        match self {
            Self::Session(c) | Self::Register(c) | Self::List(c) => c,
        }
    }
}

/// Domande in più su un file che ha dei processi (domanda diretta): restano quelli presenti in tutte.
pub const CONFIRMATIONS: usize = 3;
/// Distanza tra le domande di conferma.
pub const CONFIRM_EVERY: Duration = Duration::from_millis(250);

/// Processi che usano almeno uno dei file `paths` (percorsi assoluti, con o senza `\\?\`). Con `paths`
/// vuoto un elenco vuoto. Un file che non esiste non dà errore e nessun processo. Un processo compare una
/// volta sola, anche se usa più file.
pub fn processes_using(paths: &[String]) -> Result<Vec<ProcessUse>, RmError> {
    // Un carattere nullo troncherebbe il nome: si interrogherebbe un altro file.
    if paths.iter().any(|p| p.contains('\0')) {
        return Err(RmError::Register(ERROR_INVALID_NAME));
    }
    let mut common: Vec<String> = Vec::new();
    let mut direct: Vec<Handle> = Vec::new();
    for p in paths {
        let Some(h) = open(p)? else { continue };
        match route(p, &h) {
            Route::RestartManager => common.push(p.clone()),
            Route::Direct => direct.push(h),
        }
    }
    let mut out = restart_manager(&common)?;
    for h in &direct {
        for pid in lasting_pids(h)? {
            if !out.iter().any(|u| u.pid == pid) {
                out.push(ProcessUse { pid, app_name: image_name(pid), service: None });
            }
        }
    }
    // Processi protetti nella domanda diretta: il nome dall'istantanea dei processi.
    if out.iter().any(|u| u.app_name.is_empty()) {
        let names = snapshot_names();
        for u in out.iter_mut().filter(|u| u.app_name.is_empty()) {
            u.app_name = names.get(&u.pid).cloned().unwrap_or_default();
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Strada di un file

/// Handle chiuso al drop.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: handle valido (aperto da `open`, `OpenProcess` o `CreateToolhelp32Snapshot` e controllato)
        // chiuso una sola volta, qui.
        unsafe { CloseHandle(self.0) };
    }
}

fn last_error() -> u32 {
    // SAFETY: nessun prerequisito.
    unsafe { GetLastError() }
}

/// Apre `path` con i soli attributi; `Ok(None)` se non esiste.
fn open(path: &str) -> Result<Option<Handle>, RmError> {
    let w: Vec<u16> = path.strip_prefix(r"\\?\").unwrap_or(path).encode_utf16().collect();
    let z = win::wide_z(&win::long_path(&w));
    // SAFETY: `z` è terminata da zero e vive per la chiamata; nessuna sicurezza né modello. Accesso ai soli
    // attributi: niente lettura né scrittura dei dati, nessun richiamo dei segnaposto, nessuna
    // decompressione (il punto di reparse si apre com'è).
    let h = unsafe {
        CreateFileW(
            z.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_OPEN_NO_RECALL,
            std::ptr::null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE {
        return match last_error() {
            2 | 3 => Ok(None),
            e => Err(RmError::Register(e)),
        };
    }
    Ok(Some(Handle(h)))
}

enum Route {
    RestartManager,
    Direct,
}

/// La strada di un file aperto: diretta per segnaposto, file WOF e percorsi lunghi senza nome breve (vedi
/// il doc del modulo); Restart Manager per gli altri. Nel dubbio (attributi non letti) diretta.
fn route(path: &str, h: &Handle) -> Route {
    // SAFETY: struttura di soli valori; lo zero è valido.
    let mut tag: FILE_ATTRIBUTE_TAG_INFO = unsafe { std::mem::zeroed() };
    // SAFETY: handle valido; `tag` è una variabile locale della dimensione dichiarata.
    let ok = unsafe { GetFileInformationByHandleEx(h.0, FileAttributeTagInfo, (&raw mut tag).cast(), size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32) };
    if ok == 0 || content::is_placeholder(tag.FileAttributes, tag.ReparseTag) || externally_backed(h) {
        return Route::Direct;
    }
    let plain = path.strip_prefix(r"\\?\").unwrap_or(path);
    if plain.encode_utf16().count() >= 260 && crate::shell::path_candidates(plain).len() < 2 {
        return Route::Direct;
    }
    Route::RestartManager
}

/// File con un'origine esterna dei dati (WOF): `FSCTL_GET_EXTERNAL_BACKING` riesce. Con un altro errore
/// (di solito `ERROR_OBJECT_NOT_EXTERNALLY_BACKED`, 342) non lo è.
fn externally_backed(h: &Handle) -> bool {
    let mut buf = [0u8; 64];
    let mut returned = 0u32;
    // SAFETY: handle valido; nessun ingresso; uscita in un buffer locale della dimensione dichiarata;
    // chiamata sincrona (nessuna OVERLAPPED). È una domanda di sola lettura sullo stato del file.
    let ok = unsafe { DeviceIoControl(h.0, FSCTL_GET_EXTERNAL_BACKING, std::ptr::null(), 0, buf.as_mut_ptr().cast(), buf.len() as u32, &mut returned, std::ptr::null_mut()) };
    // Con un buffer troppo piccolo (ERROR_INSUFFICIENT_BUFFER, 122, o ERROR_MORE_DATA, 234) l'origine esterna
    // c'è lo stesso.
    ok != 0 || matches!(last_error(), 122 | 234)
}

// ---------------------------------------------------------------------------------------------
// Restart Manager

/// Nomi registrati per chiamata di `RmRegisterResources`.
const REGISTER_CHUNK: usize = 1024;
/// Tentativi se l'elenco cresce tra una chiamata e l'altra.
const LIST_ATTEMPTS: usize = 8;
/// Posti in più nell'elenco, per i processi che arrivano tra una chiamata e l'altra.
const LIST_SLACK: usize = 8;

/// Sessione di Restart Manager, chiusa al drop.
struct Session(u32);

impl Session {
    fn start() -> Result<Self, RmError> {
        let mut handle = 0u32;
        let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
        // SAFETY: `handle` è una variabile locale; la chiave è un buffer locale di CCH_RM_SESSION_KEY + 1
        // caratteri, come richiesto; nessun flag.
        let e = unsafe { RmStartSession(&mut handle, None, PWSTR(key.as_mut_ptr())) };
        if e == ERROR_SUCCESS { Ok(Self(handle)) } else { Err(RmError::Session(e.0)) }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: sessione aperta da RmStartSession e chiusa una sola volta, qui.
        let _ = unsafe { RmEndSession(self.0) };
    }
}

/// Nome da registrare: Restart Manager rifiuta i percorsi oltre 260 caratteri, anche con `\\?\` (errore
/// 29); per quelli si usa il nome breve 8.3 (chi non lo ha va per la strada diretta).
fn registered_name(path: &str) -> Vec<u16> {
    let name = crate::shell::path_candidates(path).pop().unwrap_or_else(|| path.to_owned());
    name.encode_utf16().chain(std::iter::once(0)).collect()
}

fn utf16_until_nul(s: &[u16]) -> String {
    let n = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..n])
}

fn process_use(info: &RM_PROCESS_INFO) -> ProcessUse {
    let service = utf16_until_nul(&info.strServiceShortName);
    ProcessUse {
        pid: info.Process.dwProcessId,
        app_name: utf16_until_nul(&info.strAppName),
        service: (!service.is_empty()).then_some(service),
    }
}

/// Restart Manager su file comuni (né WOF né segnaposto): una sessione, registrazione a blocchi, elenco.
fn restart_manager(paths: &[String]) -> Result<Vec<ProcessUse>, RmError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let session = Session::start()?;
    let wide: Vec<Vec<u16>> = paths.iter().map(|p| registered_name(p)).collect();
    for chunk in wide.chunks(REGISTER_CHUNK) {
        let names: Vec<PCWSTR> = chunk.iter().map(|w| PCWSTR(w.as_ptr())).collect();
        // SAFETY: sessione aperta; ogni nome è terminato da zero e vive in `wide` fino alla fine della
        // funzione; l'elenco ha al più REGISTER_CHUNK elementi. Nessuna applicazione né servizio.
        let e = unsafe { RmRegisterResources(session.0, Some(&names), None, None) };
        if e != ERROR_SUCCESS {
            return Err(RmError::Register(e.0));
        }
    }
    let mut infos: Vec<RM_PROCESS_INFO> = Vec::new();
    for _ in 0..LIST_ATTEMPTS {
        let (mut needed, mut count, mut reasons) = (0u32, infos.len() as u32, 0u32);
        let array = if infos.is_empty() { None } else { Some(infos.as_mut_ptr()) };
        // SAFETY: sessione aperta; `array` è nullo con `count` = 0, oppure punta a `infos`, che ha
        // esattamente `count` elementi scrivibili; gli altri puntatori sono a variabili locali.
        let e = unsafe { RmGetList(session.0, &mut needed, &mut count, array, &mut reasons) };
        if e == ERROR_SUCCESS {
            infos.truncate(count as usize);
            return Ok(infos.iter().map(process_use).collect());
        }
        if e != ERROR_MORE_DATA {
            return Err(RmError::List(e.0));
        }
        infos = vec![RM_PROCESS_INFO::default(); needed as usize + LIST_SLACK];
    }
    Err(RmError::List(ERROR_MORE_DATA.0))
}

// ---------------------------------------------------------------------------------------------
// Domanda diretta

/// `STATUS_INFO_LENGTH_MISMATCH`: il buffer è troppo piccolo per l'elenco.
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;
/// Posti iniziali nell'elenco dei processi.
const FIRST_LIST: usize = 16;

/// Processi che hanno aperto il file dell'handle.
fn pids_using(h: &Handle) -> Result<Vec<u32>, RmError> {
    // FILE_PROCESS_IDS_USING_FILE_INFORMATION: un ULONG con il numero, poi un ULONG_PTR per processo. Un
    // buffer di `usize` dà l'allineamento giusto; il primo `usize` contiene il numero nei 32 bit bassi.
    let mut buf: Vec<usize> = vec![0; 1 + FIRST_LIST];
    for _ in 0..LIST_ATTEMPTS {
        // SAFETY: lo stato d'uscita è una variabile locale azzerata, valida per la struttura.
        let mut iosb: IO_STATUS_BLOCK = unsafe { std::mem::zeroed() };
        let len = (buf.len() * size_of::<usize>()) as u32;
        // SAFETY: handle valido; `buf` ha `len` byte scrivibili e allineati per la struttura.
        let st = unsafe { NtQueryInformationFile(h.0, &mut iosb, buf.as_mut_ptr().cast(), len, FileProcessIdsUsingFileInformation) };
        if st == STATUS_INFO_LENGTH_MISMATCH {
            buf = vec![0; buf.len() * 4];
            continue;
        }
        if st < 0 {
            return Err(RmError::List(st as u32));
        }
        let n = (buf[0] & 0xFFFF_FFFF).min(buf.len() - 1);
        return Ok(buf[1..=n].iter().map(|&p| p as u32).collect());
    }
    Err(RmError::List(STATUS_INFO_LENGTH_MISMATCH as u32))
}

/// Processi che tengono il file per più di un attimo: quelli presenti nella prima risposta e in ciascuna
/// delle [`CONFIRMATIONS`] successive.
fn lasting_pids(h: &Handle) -> Result<Vec<u32>, RmError> {
    let mut pids = pids_using(h)?;
    for _ in 0..CONFIRMATIONS {
        if pids.is_empty() {
            break;
        }
        std::thread::sleep(CONFIRM_EVERY);
        let again = pids_using(h)?;
        pids.retain(|p| again.contains(p));
    }
    Ok(pids)
}

/// Nome dell'eseguibile del processo, vuoto se Windows non lo dà.
fn image_name(pid: u32) -> String {
    // SAFETY: accesso di sola interrogazione limitata; un PID non valido dà un handle nullo.
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if h.is_null() {
        return String::new();
    }
    let h = Handle(h);
    let mut buf = vec![0u16; 32_768];
    let mut len = buf.len() as u32;
    // SAFETY: handle di processo valido; `buf` ha `len` caratteri.
    let ok = unsafe { QueryFullProcessImageNameW(h.0, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) };
    if ok == 0 {
        return String::new();
    }
    let full = String::from_utf16_lossy(&buf[..len as usize]);
    full.rsplit('\\').next().unwrap_or(&full).to_owned()
}

/// Nomi degli eseguibili di tutti i processi, dall'istantanea dei processi di Windows: serve per i processi
/// protetti (Defender, servizi di sistema), che non si lasciano aprire nemmeno per leggerne il nome.
fn snapshot_names() -> HashMap<u32, String> {
    let mut out = HashMap::new();
    // SAFETY: istantanea dei soli processi; l'handle si chiude con `Handle`.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return out;
    }
    let snap = Handle(snap);
    // SAFETY: struttura di soli valori; `dwSize` impostato come richiesto.
    let mut e: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    e.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    // SAFETY: istantanea valida; `e` ha la dimensione dichiarata.
    let mut ok = unsafe { Process32FirstW(snap.0, &mut e) } != 0;
    while ok {
        let n = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
        out.insert(e.th32ProcessID, String::from_utf16_lossy(&e.szExeFile[..n]));
        // SAFETY: come sopra.
        ok = unsafe { Process32NextW(snap.0, &mut e) } != 0;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_stop_at_the_terminator() {
        let mut info = RM_PROCESS_INFO::default();
        info.Process.dwProcessId = 42;
        for (d, c) in info.strAppName.iter_mut().zip("Prova".encode_utf16()) {
            *d = c;
        }
        assert_eq!(process_use(&info), ProcessUse { pid: 42, app_name: "Prova".into(), service: None });
        for (d, c) in info.strServiceShortName.iter_mut().zip("svc".encode_utf16()) {
            *d = c;
        }
        assert_eq!(process_use(&info).service.as_deref(), Some("svc"));
        // Buffer pieno senza terminatore: si legge tutto, senza uscire dal buffer.
        info.strServiceShortName = [b'x' as u16; 64];
        assert_eq!(process_use(&info).service.map(|s| s.len()), Some(64));
    }

    #[test]
    fn no_paths_no_reads() {
        assert_eq!(processes_using(&[]), Ok(Vec::new()));
    }

    #[test]
    fn nul_in_a_path_is_rejected() {
        let e = processes_using(&["C:\\Windows\0\\x".to_owned()]);
        assert_eq!(e, Err(RmError::Register(ERROR_INVALID_NAME)));
        assert_eq!(e.map_err(RmError::code), Err(123));
    }

    #[test]
    fn process_names() {
        assert!(image_name(std::process::id()).to_ascii_lowercase().ends_with(".exe"));
        // Anche dall'istantanea, che vale per i processi protetti.
        assert!(snapshot_names().get(&std::process::id()).is_some_and(|n| n.to_ascii_lowercase().ends_with(".exe")));
    }

    #[test]
    fn ordinary_files_go_to_restart_manager() {
        let exe = std::env::current_exe().unwrap().to_string_lossy().into_owned();
        let h = open(&exe).unwrap().expect("l'eseguibile del test");
        assert!(matches!(route(&exe, &h), Route::RestartManager));
        assert!(!externally_backed(&h));
    }
}
