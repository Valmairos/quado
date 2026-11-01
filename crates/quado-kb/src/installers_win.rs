// SPDX-License-Identifier: GPL-3.0-or-later
//! Letture su richiesta degli installatori (F-DL-2): risorsa di versione PE (`GetFileVersionInfoExW`) e
//! tabella Property degli MSI (`MsiOpenDatabaseW` in sola lettura, mai `MsiOpenPackage`).
//!
//! Regole comuni (05 D6, 06 §11):
//! - prima del contenuto si leggono solo gli attributi: i segnaposto cloud non si aprono, perché la
//!   lettura li scaricherebbe (`ExtractError::CloudPlaceholder`);
//! - nessun handle in scrittura, nessun processo creato, nessun codice caricato: `GetFileVersionInfoExW`
//!   mappa il file come dati, `MsiOpenDatabaseW` apre il database in sola lettura senza una sessione di
//!   installazione (quindi senza interfaccia dell'installer);
//! - percorsi assoluti con `\\?\`, validi anche oltre 260 caratteri; il percorso deve essere già
//!   normalizzato (niente `.` o `..`), come quelli della scansione;
//! - ogni lettura aggiorna la data di ultimo accesso del file (06 §3.6): la UI lo dice prima di chiedere.
//!
//! Le funzioni sono sincrone e leggono un file: lettura in background, annullamento tra un file e
//! l'altro e cache (`installers::InstallerCache`) sono di chi chiama.

use std::ffi::c_void;
use std::os::windows::fs::OpenOptionsExt;

use windows_sys::Win32::Foundation::{
    ERROR_BAD_EXE_FORMAT, ERROR_BAD_FORMAT, ERROR_BAD_QUERY_SYNTAX, ERROR_EXE_MARKED_INVALID, ERROR_FILE_CORRUPT, ERROR_FILE_NOT_FOUND, ERROR_FILENAME_EXCED_RANGE,
    ERROR_FUNCTION_FAILED, ERROR_INSTALL_PACKAGE_INVALID, ERROR_INSTALL_PACKAGE_OPEN_FAILED, ERROR_INVALID_DATA, ERROR_INVALID_EXE_SIGNATURE, ERROR_INVALID_TABLE,
    ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_OPEN_FAILED, ERROR_PATH_NOT_FOUND, ERROR_RESOURCE_DATA_NOT_FOUND, ERROR_RESOURCE_LANG_NOT_FOUND,
    ERROR_RESOURCE_NAME_NOT_FOUND, ERROR_RESOURCE_TYPE_NOT_FOUND, ERROR_SUCCESS, GetLastError,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS, FILE_ATTRIBUTE_RECALL_ON_OPEN, FILE_FLAG_OPEN_NO_RECALL, FILE_SHARE_READ,
    FILE_VER_GET_NEUTRAL, GetFileAttributesExW, GetFileExInfoStandard, GetFileVersionInfoExW, GetFileVersionInfoSizeExW, GetShortPathNameW, VerQueryValueW,
    WIN32_FILE_ATTRIBUTE_DATA,
};
use windows_sys::Win32::System::ApplicationInstallationAndServicing::{
    MSIDBOPEN_READONLY, MSIHANDLE, MsiCloseHandle, MsiDatabaseOpenViewW, MsiOpenDatabaseW, MsiRecordGetStringW, MsiViewExecute, MsiViewFetch,
};

use crate::extract::{self, ExtractError, FormatIssue};
use crate::installers::{self, FIXED_LEN, FIXED_SIGNATURE, InstallerInfo, InstallerKind, MAX_TEXT};

/// Limite della risorsa di versione: una risorsa a 32 bit ha la radice di al massimo 64 KiB e Windows
/// chiede circa il doppio per le conversioni; oltre questo valore la risorsa è anomala e non si alloca.
pub const MAX_VERSION_RESOURCE: u32 = 4 << 20;
/// Limite delle righe lette dalla tabella Property.
pub const MAX_PROPERTY_ROWS: usize = 100_000;
/// Lunghezza massima dei percorsi senza `\\?\`, terminatore escluso.
const MAX_PATH: usize = 260;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Percorso per le API Win32 con `\\?\` (anche `\\?\UNC\` per i percorsi di rete), terminato da zero.
/// I percorsi relativi restano come sono.
pub fn long_path(path: &str) -> Vec<u16> {
    let p = path.replace('/', "\\");
    let full = if p.starts_with(r"\\?\") || p.starts_with(r"\\.\") {
        p
    } else if let Some(unc) = p.strip_prefix(r"\\") {
        format!(r"\\?\UNC\{unc}")
    } else if extract::is_absolute(&p) {
        format!(r"\\?\{p}")
    } else {
        p
    };
    wide(&full)
}

fn io_error(e: u32) -> ExtractError {
    match e {
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => ExtractError::NotPresent,
        _ => ExtractError::Io(e),
    }
}

/// Attributi e dimensione, senza aprire il contenuto. Rifiuta i segnaposto cloud (dati non presenti in
/// locale o richiamati all'apertura) e le cartelle; un file vuoto è `FormatIssue::Empty`.
fn check_file(path: &[u16]) -> Result<u64, ExtractError> {
    let mut data = WIN32_FILE_ATTRIBUTE_DATA::default();
    // SAFETY: `path` è terminata da zero e vive per la chiamata; `data` è una struttura C di soli valori
    // della dimensione attesa per `GetFileExInfoStandard`.
    let ok = unsafe { GetFileAttributesExW(path.as_ptr(), GetFileExInfoStandard, (&raw mut data).cast()) };
    if ok == 0 {
        // SAFETY: nessun argomento; legge l'errore del thread subito dopo la chiamata non riuscita.
        return Err(io_error(unsafe { GetLastError() }));
    }
    let a = data.dwFileAttributes;
    if a & (FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_OFFLINE) != 0 {
        return Err(ExtractError::CloudPlaceholder);
    }
    if a & FILE_ATTRIBUTE_DIRECTORY != 0 {
        return Err(FormatIssue::Unexpected.into());
    }
    let size = (u64::from(data.nFileSizeHigh) << 32) | u64::from(data.nFileSizeLow);
    if size == 0 {
        return Err(FormatIssue::Empty.into());
    }
    Ok(size)
}

fn no_nul(path: &str) -> Result<(), ExtractError> {
    // Un percorso con uno zero dentro arriverebbe tagliato a Windows, cioè a un altro file.
    if path.contains('\0') { Err(ExtractError::NotPresent) } else { Ok(()) }
}

/// Apre il file per i controlli di struttura (`installers::pe_check`, `installers::compound_file_check`)
/// con un handle di sola lettura che non ammette scrittori (`FILE_SHARE_READ`): un file ancora in
/// scrittura (download in corso) dà `Io(32)` invece di un formato inatteso. `FILE_FLAG_OPEN_NO_RECALL`
/// per prudenza, anche se i segnaposto sono già esclusi dagli attributi.
fn open_for_check(path: &str) -> Result<std::fs::File, ExtractError> {
    std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_NO_RECALL)
        .open(path)
        .map_err(|e| io_error(e.raw_os_error().unwrap_or(0) as u32))
}

// ---------------------------------------------------------------------------------------------
// Risorsa di versione

/// Errore di `GetFileVersionInfoSizeExW`/`GetFileVersionInfoExW`. Un file senza risorsa di versione è un
/// esito normale: dati vuoti con il tipo, che `installers::info_facts` scrive come "nessun dato".
fn version_error(e: u32) -> Result<InstallerInfo, ExtractError> {
    match e {
        ERROR_RESOURCE_DATA_NOT_FOUND | ERROR_RESOURCE_TYPE_NOT_FOUND | ERROR_RESOURCE_NAME_NOT_FOUND | ERROR_RESOURCE_LANG_NOT_FOUND => {
            Ok(InstallerInfo { kind: Some(InstallerKind::Exe), ..Default::default() })
        }
        // ERROR_INVALID_DATA: Windows rifiuta la risorsa (per esempio senza VS_FIXEDFILEINFO o con una
        // radice diversa da VS_VERSION_INFO).
        ERROR_BAD_EXE_FORMAT | ERROR_BAD_FORMAT | ERROR_INVALID_EXE_SIGNATURE | ERROR_EXE_MARKED_INVALID | ERROR_FILE_CORRUPT | ERROR_INVALID_DATA => {
            Err(FormatIssue::Unexpected.into())
        }
        other => Err(io_error(other)),
    }
}

/// Risorsa di versione copiata da Windows: `words` è il buffer passato alle API (allineato a 4 byte),
/// `bytes` la sua copia in byte, limitata a quanto Windows ha dichiarato. Su una risorsa a 32 bit
/// `VerQueryValueW` non scrive nel buffer: restituisce puntatori dentro la risorsa, che si leggono dalla copia.
struct Resource {
    words: Vec<u32>,
    bytes: Vec<u8>,
}

impl Resource {
    /// Intervallo `(inizio, lunghezza)` restituito da `VerQueryValueW`, relativo al buffer. `None` se la
    /// voce non c'è o se il puntatore non cade nel buffer.
    fn query(&self, sub: &str) -> Option<(usize, usize)> {
        let s = wide(sub);
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let mut len = 0u32;
        // SAFETY: `words` contiene la risorsa scritta da `GetFileVersionInfoExW`, già controllata da
        // `installers::version_resource_tables` (ogni blocco dentro la radice, la radice dentro il buffer):
        // `VerQueryValueW` cammina solo dentro il buffer. `s` è terminata da zero; `ptr` e `len` sono uscite,
        // e il puntatore restituito si controlla prima di usarlo.
        let ok = unsafe { VerQueryValueW(self.words.as_ptr().cast(), s.as_ptr(), &mut ptr, &mut len) };
        if ok == 0 || ptr.is_null() {
            return None;
        }
        let off = (ptr as usize).checked_sub(self.words.as_ptr() as usize)?;
        (off < self.bytes.len()).then_some((off, len as usize))
    }

    /// Stringa di una tabella (`\StringFileInfo\040904B0\ProductName`), senza spazi ai lati; `None` se
    /// manca o è vuota.
    fn text(&self, sub: &str) -> Result<Option<String>, ExtractError> {
        let Some((off, chars)) = self.query(sub) else { return Ok(None) };
        let end = chars.checked_mul(2).and_then(|b| off.checked_add(b)).filter(|&e| e <= self.bytes.len()).ok_or(FormatIssue::Truncated)?;
        let units: Vec<u16> = self.bytes[off..end].as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        let units = &units[..units.iter().position(|&c| c == 0).unwrap_or(units.len())];
        if units.len() > MAX_TEXT {
            return Err(FormatIssue::TooLarge.into());
        }
        let s = String::from_utf16(units).map_err(|_| FormatIssue::Encoding)?;
        let t = s.trim();
        Ok((!t.is_empty()).then(|| t.to_owned()))
    }

    /// Coppie (lingua, code page) di `\VarFileInfo\Translation`, al massimo 16.
    fn translations(&self) -> Vec<(u16, u16)> {
        let Some((off, len)) = self.query(r"\VarFileInfo\Translation") else { return Vec::new() };
        let end = off.saturating_add(len).min(self.bytes.len());
        self.bytes[off..end].as_chunks::<4>().0.iter().take(16).map(|c| (u16::from_le_bytes([c[0], c[1]]), u16::from_le_bytes([c[2], c[3]]))).collect()
    }

    /// Versione del prodotto e del file da `VS_FIXEDFILEINFO` (`a.b.c.d`), se c'è e ha la sua firma.
    fn fixed(&self) -> (Option<String>, Option<String>) {
        let Some((off, len)) = self.query(r"\") else { return (None, None) };
        let b = &self.bytes;
        if len < FIXED_LEN || installers::u32_at(b, off) != Some(FIXED_SIGNATURE) {
            return (None, None);
        }
        let version = |ms_at: usize| -> Option<String> {
            let ms = installers::u32_at(b, off + ms_at)?;
            let ls = installers::u32_at(b, off + ms_at + 4)?;
            (ms != 0 || ls != 0).then(|| format!("{}.{}.{}.{}", ms >> 16, ms & 0xFFFF, ls >> 16, ls & 0xFFFF))
        };
        // dwFileVersionMS a 8, dwProductVersionMS a 16.
        (version(16), version(8))
    }
}

/// Nome, versione, editore e versione del file dalla risorsa di versione di un eseguibile.
///
/// - Segnaposto cloud: `CloudPlaceholder`, senza aprire il file.
/// - `GetFileVersionInfoSizeExW`/`GetFileVersionInfoExW` con `FILE_VER_GET_NEUTRAL`: stringhe e numeri dal
///   file stesso. Senza questo flag le stringhe possono venire dal file MUI della lingua dell'interfaccia
///   (`en-US\nome.exe.mui`), come fa PowerShell con `VersionInfo`: su System32 di questo PC (14/10/2026)
///   198 eseguibili su 594 hanno nel MUI una `ProductVersion` vecchia (10.0.19041.1 invece di 10.0.19041.3636),
///   con nomi ed editori uguali in 593 casi. Un installatore scaricato non ha un MUI, ma un file accanto non
///   deve poter cambiare i dati letti. `FILE_VER_GET_PREFETCHED` guarderebbe i moduli caricati nel processo.
/// - Stringhe della prima traduzione di `\VarFileInfo\Translation`, poi delle code page comuni
///   (040904B0, 040904E4, 000004B0), poi delle altre traduzioni e delle altre tabelle presenti, campo per
///   campo; `ProductVersion` e `FileVersion` ripiegano su `VS_FIXEDFILEINFO`.
/// - Prima delle API di versione, le intestazioni (`installers::pe_check`): un file che non è un PE dà
///   `Format(Unexpected)`, uno troncato (sezioni o tabella dei certificati oltre la fine) `Format(Truncated)`,
///   uno ancora in scrittura `Io(32)`.
/// - File senza risorsa di versione: `Ok` con il solo `kind`, è un esito normale. Risorsa con lunghezze
///   incoerenti: `Format(Truncated)` o `Format(Unexpected)`, prima di qualunque `VerQueryValueW`. Testo non
///   UTF-16 valido: `Format(Encoding)`. Risorsa oltre `MAX_VERSION_RESOURCE` o valore oltre
///   `installers::MAX_TEXT`: `Format(TooLarge)`.
pub fn version_info(path: &str) -> Result<InstallerInfo, ExtractError> {
    no_nul(path)?;
    let p = long_path(path);
    let file_size = check_file(&p)?;
    installers::pe_check(&mut open_for_check(path)?, file_size)?;
    let mut ignored = 0u32;
    // SAFETY: `p` è terminata da zero e vive per la chiamata; `ignored` è un'uscita non usata.
    let size = unsafe { GetFileVersionInfoSizeExW(FILE_VER_GET_NEUTRAL, p.as_ptr(), &mut ignored) };
    if size == 0 {
        // SAFETY: nessun argomento; legge l'errore del thread subito dopo la chiamata non riuscita.
        return version_error(unsafe { GetLastError() });
    }
    if size > MAX_VERSION_RESOURCE {
        return Err(FormatIssue::TooLarge.into());
    }
    let mut words = vec![0u32; (size as usize).div_ceil(4)];
    // SAFETY: `words` ha almeno `size` byte, allineati a 4 e inizializzati; Windows vi scrive al massimo
    // `size` byte. `p` è terminata da zero.
    let ok = unsafe { GetFileVersionInfoExW(FILE_VER_GET_NEUTRAL, p.as_ptr(), 0, size, words.as_mut_ptr().cast()) };
    if ok == 0 {
        // SAFETY: nessun argomento; legge l'errore del thread subito dopo la chiamata non riuscita.
        return version_error(unsafe { GetLastError() });
    }
    let mut bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    bytes.truncate(size as usize);
    let found = installers::version_resource_tables(&bytes)?;
    let res = Resource { words, bytes };

    let translations = res.translations();
    let mut tables: Vec<String> = Vec::new();
    let first = translations.first().map(|(l, c)| format!("{l:04X}{c:04X}"));
    let others = translations.iter().skip(1).map(|(l, c)| format!("{l:04X}{c:04X}"));
    for t in first.into_iter().chain(["040904B0", "040904E4", "000004B0"].map(str::to_owned)).chain(others).chain(found) {
        if !tables.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
            tables.push(t);
        }
    }
    let read = |name: &str| -> Result<Option<String>, ExtractError> {
        for t in &tables {
            if let Some(v) = res.text(&format!(r"\StringFileInfo\{t}\{name}"))? {
                return Ok(Some(v));
            }
        }
        Ok(None)
    };
    let (fixed_product, fixed_file) = res.fixed();
    Ok(InstallerInfo {
        kind: Some(InstallerKind::Exe),
        product_name: read("ProductName")?,
        product_version: read("ProductVersion")?.or(fixed_product),
        company: read("CompanyName")?,
        product_code: None,
        upgrade_code: None,
        file_version: read("FileVersion")?.or(fixed_file),
    })
}

// ---------------------------------------------------------------------------------------------
// MSI

/// Handle MSI (database, vista, record) chiuso all'uscita.
struct MsiHandle(MSIHANDLE);

impl Drop for MsiHandle {
    fn drop(&mut self) {
        if self.0 != 0 {
            // SAFETY: l'handle è stato restituito da un'API MSI e si chiude una volta sola, qui.
            unsafe {
                MsiCloseHandle(self.0);
            }
        }
    }
}

/// Errore delle API MSI. Dopo il controllo della struttura (`installers::compound_file_check`) un'apertura
/// o una query non riuscite vogliono dire un pacchetto non valido o danneggiato: formato inatteso. Gli altri
/// codici restano errori di Windows.
fn msi_error(e: u32) -> ExtractError {
    match e {
        ERROR_OPEN_FAILED | ERROR_INSTALL_PACKAGE_INVALID | ERROR_INSTALL_PACKAGE_OPEN_FAILED | ERROR_BAD_QUERY_SYNTAX => FormatIssue::Unexpected.into(),
        ERROR_INVALID_DATA | ERROR_FUNCTION_FAILED | ERROR_INVALID_TABLE | ERROR_FILE_CORRUPT => FormatIssue::Unexpected.into(),
        other => io_error(other),
    }
}

/// Percorso per `MsiOpenDatabaseW`, che non apre i percorsi oltre 260 caratteri nemmeno con `\\?\`:
/// oltre quel limite si usa il nome breve 8.3 (`GetShortPathNameW`), se il volume lo ha.
fn msi_path(p: &[u16]) -> Result<Vec<u16>, ExtractError> {
    let plain = p.strip_suffix(&[0]).unwrap_or(p);
    let prefix: Vec<u16> = r"\\?\".encode_utf16().collect();
    let unprefixed = |s: &[u16]| -> usize { s.strip_prefix(&prefix[..]).unwrap_or(s).len() };
    if unprefixed(plain) < MAX_PATH {
        return Ok(p.to_vec());
    }
    let mut buf = vec![0u16; plain.len() + 1];
    for _ in 0..3 {
        // SAFETY: `p` è terminata da zero; `buf` ha `buf.len()` caratteri, il numero passato.
        let n = unsafe { GetShortPathNameW(p.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
        if n == 0 {
            // SAFETY: nessun argomento; legge l'errore del thread subito dopo la chiamata non riuscita.
            return Err(io_error(unsafe { GetLastError() }));
        }
        if n < buf.len() {
            let short = &buf[..n];
            if unprefixed(short) >= MAX_PATH {
                return Err(ExtractError::Io(ERROR_FILENAME_EXCED_RANGE));
            }
            return Ok(short.iter().copied().chain(std::iter::once(0)).collect());
        }
        // Buffer piccolo: `n` è la lunghezza necessaria, zero compreso.
        buf = vec![0u16; n];
    }
    Err(ExtractError::Io(ERROR_FILENAME_EXCED_RANGE))
}

/// Testo di un campo di un record, con il ciclo su `ERROR_MORE_DATA`.
fn record_string(rec: &MsiHandle, field: u32) -> Result<String, ExtractError> {
    let mut buf = vec![0u16; 128];
    for _ in 0..4 {
        let mut len = buf.len() as u32;
        // SAFETY: `buf` ha `len` caratteri; Windows ne scrive al massimo `len`, zero compreso, e in `len`
        // restituisce quelli scritti o, con ERROR_MORE_DATA, quelli necessari.
        let r = unsafe { MsiRecordGetStringW(rec.0, field, buf.as_mut_ptr(), &mut len) };
        match r {
            ERROR_SUCCESS => {
                let n = (len as usize).min(buf.len());
                return String::from_utf16(&buf[..n]).map_err(|_| FormatIssue::Encoding.into());
            }
            ERROR_MORE_DATA => {
                if len as usize > MAX_TEXT {
                    return Err(FormatIssue::TooLarge.into());
                }
                buf = vec![0u16; len as usize + 1];
            }
            e => return Err(msi_error(e)),
        }
    }
    Err(FormatIssue::Unexpected.into())
}

/// Nome, versione, editore, `ProductCode` e `UpgradeCode` dalla tabella Property di un MSI.
///
/// - Segnaposto cloud: `CloudPlaceholder`, senza aprire il file.
/// - Prima di Windows Installer, la struttura del file composto (`installers::compound_file_check`): file
///   vuoto `Format(Empty)`; senza la firma o con un'intestazione incoerente `Format(Unexpected)`; troncato
///   (settori in uso oltre la fine) `Format(Truncated)`; ancora in scrittura `Io(32)`.
/// - Percorso oltre 260 caratteri: `MsiOpenDatabaseW` non lo apre, si usa il nome breve 8.3; senza nomi
///   brevi sul volume `Io(206)` (`ERROR_FILENAME_EXCED_RANGE`).
/// - `MsiOpenDatabaseW` con `MSIDBOPEN_READONLY`, poi ``SELECT `Property`, `Value` FROM `Property` `` con
///   `MsiDatabaseOpenViewW`, `MsiViewExecute` e `MsiViewFetch` fino a `ERROR_NO_MORE_ITEMS`. Ogni handle
///   si chiude con `MsiCloseHandle` (guardia `MsiHandle`), anche sugli errori. Mai `MsiOpenPackage` né
///   `MsiOpenPackageEx`: dipendono dallo stato della macchina e aprono una sessione dell'installer.
/// - Pacchetto senza tabella Property o danneggiato: `Format(Unexpected)`. Proprietà mancanti: campi `None`.
pub fn msi_info(path: &str) -> Result<InstallerInfo, ExtractError> {
    no_nul(path)?;
    let p = long_path(path);
    let size = check_file(&p)?;
    installers::compound_file_check(&mut open_for_check(path)?, size)?;
    let db_path = msi_path(&p)?;
    let mut db = MsiHandle(0);
    // SAFETY: `db_path` è terminata da zero; `MSIDBOPEN_READONLY` non scrive sul file; `db.0` riceve
    // l'handle, chiuso dalla guardia.
    let r = unsafe { MsiOpenDatabaseW(db_path.as_ptr(), MSIDBOPEN_READONLY, &mut db.0) };
    if r != ERROR_SUCCESS {
        return Err(msi_error(r));
    }
    let query = wide("SELECT `Property`, `Value` FROM `Property`");
    let mut view = MsiHandle(0);
    // SAFETY: `db.0` è un database aperto; `query` è terminata da zero; `view.0` riceve l'handle, chiuso
    // dalla guardia prima del database (ordine inverso di dichiarazione).
    let r = unsafe { MsiDatabaseOpenViewW(db.0, query.as_ptr(), &mut view.0) };
    if r != ERROR_SUCCESS {
        return Err(msi_error(r));
    }
    // SAFETY: `view.0` è una vista aperta; nessun record di parametri.
    let r = unsafe { MsiViewExecute(view.0, 0) };
    if r != ERROR_SUCCESS {
        return Err(msi_error(r));
    }
    let mut info = InstallerInfo { kind: Some(InstallerKind::Msi), ..Default::default() };
    let mut rows = 0usize;
    loop {
        let mut rec = MsiHandle(0);
        // SAFETY: `view.0` è una vista eseguita; `rec.0` riceve il record, chiuso dalla guardia a ogni giro.
        let r = unsafe { MsiViewFetch(view.0, &mut rec.0) };
        if r == ERROR_NO_MORE_ITEMS {
            break;
        }
        if r != ERROR_SUCCESS {
            return Err(msi_error(r));
        }
        rows += 1;
        if rows > MAX_PROPERTY_ROWS {
            return Err(FormatIssue::TooLarge.into());
        }
        let slot = match record_string(&rec, 1)?.as_str() {
            "ProductName" => &mut info.product_name,
            "ProductVersion" => &mut info.product_version,
            "Manufacturer" => &mut info.company,
            "ProductCode" => &mut info.product_code,
            "UpgradeCode" => &mut info.upgrade_code,
            _ => continue,
        };
        let value = record_string(&rec, 2)?;
        let value = value.trim();
        *slot = (!value.is_empty()).then(|| value.to_owned());
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    //! MSI generati nei test con le API di Windows Installer (`MSIDBOPEN_CREATE`) in una cartella
    //! temporanea, cancellata alla fine. La creazione sta qui perché richiede `unsafe`, ammesso solo in
    //! questo modulo, e perché non deve finire nel codice del programma, che non scrive negli installatori.

    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::System::ApplicationInstallationAndServicing::{MSIDBOPEN_CREATE, MsiCloseAllHandles, MsiDatabaseCommit};
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible};
    use windows_sys::core::BOOL;

    use super::*;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let p = std::env::temp_dir().join(format!("quado-installatori-{name}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).expect("cartella temporanea");
            Self(p)
        }

        fn file(&self, name: &str) -> String {
            self.0.join(name).to_string_lossy().into_owned()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn exec(db: &MsiHandle, sql: &str) {
        let q = wide(sql);
        let mut view = MsiHandle(0);
        // SAFETY: `db.0` è un database aperto in scrittura dal test; `q` è terminata da zero.
        let r = unsafe { MsiDatabaseOpenViewW(db.0, q.as_ptr(), &mut view.0) };
        assert_eq!(r, ERROR_SUCCESS, "{sql}");
        // SAFETY: `view.0` è una vista aperta.
        let r = unsafe { MsiViewExecute(view.0, 0) };
        assert_eq!(r, ERROR_SUCCESS, "{sql}");
    }

    /// Database MSI con la sola tabella Property (o con un'altra tabella, se `table` è diverso).
    fn create_msi(path: &str, table: &str, props: &[(&str, &str)]) {
        let p = wide(path);
        let mut db = MsiHandle(0);
        // SAFETY: `p` è terminata da zero; il file si crea nella cartella temporanea del test.
        let r = unsafe { MsiOpenDatabaseW(p.as_ptr(), MSIDBOPEN_CREATE, &mut db.0) };
        assert_eq!(r, ERROR_SUCCESS, "creazione di {path}");
        exec(&db, &format!("CREATE TABLE `{table}` (`Property` CHAR(72) NOT NULL, `Value` LONGCHAR NOT NULL LOCALIZABLE PRIMARY KEY `Property`)"));
        for (k, v) in props {
            exec(&db, &format!("INSERT INTO `{table}` (`Property`, `Value`) VALUES ('{k}', '{v}')"));
        }
        // SAFETY: `db.0` è il database aperto sopra.
        let r = unsafe { MsiDatabaseCommit(db.0) };
        assert_eq!(r, ERROR_SUCCESS);
    }

    const CODE: &str = "{6A1C2D3E-4F50-4A6B-8C7D-9E0F1A2B3C4D}";
    const UPGRADE: &str = "{0F1E2D3C-4B5A-4968-8776-655443322110}";

    fn sample(dir: &TempDir, name: &str) -> String {
        let path = dir.file(name);
        create_msi(
            &path,
            "Property",
            &[
                ("ProductName", "Prodotto di prova"),
                ("ProductVersion", "1.2.3"),
                ("ProductCode", CODE),
                ("Manufacturer", "Editore di prova"),
                ("UpgradeCode", UPGRADE),
                ("ProductLanguage", "1033"),
            ],
        );
        path
    }

    fn modified(path: &str) -> std::time::SystemTime {
        std::fs::metadata(path).and_then(|m| m.modified()).expect("data di modifica")
    }

    /// Nessun handle MSI resta aperto in questo thread (`MsiCloseAllHandles` conta quelli che chiude).
    fn no_open_handles() {
        // SAFETY: chiude solo gli handle MSI del thread corrente; il test non ne tiene di aperti.
        let left = unsafe { MsiCloseAllHandles() };
        assert_eq!(left, 0, "handle MSI rimasti aperti");
    }

    unsafe extern "system" fn count_visible(hwnd: HWND, data: LPARAM) -> BOOL {
        let mut pid = 0u32;
        // SAFETY: `hwnd` viene da EnumWindows; `data` è l'indirizzo del contatore di `visible_windows`,
        // vivo per tutta l'enumerazione, che è sincrona.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == GetCurrentProcessId() && IsWindowVisible(hwnd) != 0 {
                *(data as *mut usize) += 1;
            }
        }
        1
    }

    /// Finestre visibili di questo processo: l'apertura di un database non deve mostrare interfacce
    /// dell'installer (una finestra modale bloccherebbe il test, una non modale si conta qui).
    fn visible_windows() -> usize {
        let mut n = 0usize;
        // SAFETY: la callback scrive solo in `n`, che vive fino alla fine di EnumWindows.
        unsafe { EnumWindows(Some(count_visible), (&raw mut n) as LPARAM) };
        n
    }

    #[test]
    fn msi_property_table_read_only() {
        let dir = TempDir::new("msi");
        let path = sample(&dir, "prova.msi");
        no_open_handles();
        let before = std::fs::read(&path).expect("contenuto");
        let mtime = modified(&path);
        let windows = visible_windows();
        let info = msi_info(&path).expect("lettura");
        assert_eq!(visible_windows(), windows, "l'apertura del database ha mostrato una finestra");
        assert_eq!(info.kind, Some(InstallerKind::Msi));
        assert_eq!(info.product_name.as_deref(), Some("Prodotto di prova"));
        assert_eq!(info.product_version.as_deref(), Some("1.2.3"));
        assert_eq!(info.company.as_deref(), Some("Editore di prova"));
        assert_eq!(info.product_code.as_deref(), Some(CODE));
        assert_eq!(info.upgrade_code.as_deref(), Some(UPGRADE));
        assert_eq!(info.file_version, None);
        // Contenuto e data di modifica invariati (06 §3.5), nessun handle rimasto aperto.
        assert_eq!(std::fs::read(&path).expect("contenuto"), before);
        assert_eq!(modified(&path), mtime);
        no_open_handles();
        // Una seconda lettura dà lo stesso esito.
        assert_eq!(msi_info(&path), Ok(info));
        no_open_handles();
    }

    #[test]
    fn msi_damaged_or_foreign_files() {
        let dir = TempDir::new("msi-rotti");
        let path = sample(&dir, "buono.msi");
        let good = std::fs::read(&path).expect("contenuto");
        let windows = visible_windows();
        // Troncato in più punti: un errore, mai un panic né dati presentati come quelli di un file intero.
        for cut in [8, 300, 512, 1024, good.len() / 2, good.len() - 512, good.len() - 1] {
            let p = dir.file(&format!("troncato-{cut}.msi"));
            std::fs::write(&p, &good[..cut]).expect("scrittura");
            assert_eq!(msi_info(&p), Err(FormatIssue::Truncated.into()), "troncato a {cut} di {}", good.len());
            no_open_handles();
        }
        let p = dir.file("vuoto.msi");
        std::fs::write(&p, b"").expect("scrittura");
        assert_eq!(msi_info(&p), Err(FormatIssue::Empty.into()));
        let p = dir.file("testo.msi");
        std::fs::write(&p, "non sono un pacchetto MSI\r\n".repeat(100)).expect("scrittura");
        assert_eq!(msi_info(&p), Err(FormatIssue::Unexpected.into()));
        // Firma giusta e resto a caso.
        let p = dir.file("firma.msi");
        let mut fake = installers::CFB_SIGNATURE.to_vec();
        fake.extend((0..4096u32).map(|k| (k * 31 % 251) as u8));
        std::fs::write(&p, &fake).expect("scrittura");
        assert!(msi_info(&p).is_err());
        // Database valido ma senza tabella Property.
        let p = dir.file("senza-property.msi");
        create_msi(&p, "Altro", &[("ProductName", "X")]);
        assert_eq!(msi_info(&p), Err(FormatIssue::Unexpected.into()));
        // Assente.
        assert_eq!(msi_info(&dir.file("assente.msi")), Err(ExtractError::NotPresent));
        no_open_handles();
        assert_eq!(visible_windows(), windows, "un errore di apertura ha mostrato una finestra");
    }

    #[test]
    fn msi_long_values() {
        let dir = TempDir::new("msi-valori");
        // Un nome più lungo del primo buffer (128 caratteri): il ciclo su ERROR_MORE_DATA lo legge intero.
        let long = "Nome lungo ".repeat(30);
        let path = dir.file("lungo.msi");
        create_msi(&path, "Property", &[("ProductName", &long), ("ProductCode", CODE)]);
        assert_eq!(msi_info(&path).map(|i| i.product_name), Ok(Some(long.trim().to_owned())));
        // Oltre MAX_TEXT: errore, il valore non si tronca.
        let path = dir.file("troppo.msi");
        create_msi(&path, "Property", &[("Manufacturer", &"x".repeat(MAX_TEXT + 1))]);
        assert_eq!(msi_info(&path), Err(FormatIssue::TooLarge.into()));
        no_open_handles();
    }

    #[test]
    fn msi_long_path() {
        let dir = TempDir::new("msi-lungo");
        let mut deep = dir.0.clone();
        while deep.as_os_str().len() < 300 {
            deep = deep.join("cartella-con-un-nome-abbastanza-lungo");
        }
        std::fs::create_dir_all(&deep).expect("cartelle");
        let short = sample(&dir, "breve.msi");
        let long = deep.join("lungo.msi");
        std::fs::copy(&short, &long).expect("copia");
        let long = long.to_string_lossy().into_owned();
        assert!(long.len() > 260);
        assert_eq!(msi_info(&long).map(|i| i.product_code), Ok(Some(CODE.to_owned())));
        // Lo stesso percorso già con `\\?\` e con le barre normali.
        assert!(msi_info(&format!(r"\\?\{long}")).is_ok());
        assert!(msi_info(&short.replace('\\', "/")).is_ok());
        assert!(Path::new(&long).exists());
    }

    #[test]
    fn paths_for_windows() {
        let s = |p: &str| String::from_utf16_lossy(&long_path(p)).trim_end_matches('\0').to_owned();
        assert_eq!(s(r"C:\Users\u\Downloads\a.msi"), r"\\?\C:\Users\u\Downloads\a.msi");
        assert_eq!(s("C:/Users/u/a.exe"), r"\\?\C:\Users\u\a.exe");
        assert_eq!(s(r"\\server\share\a.exe"), r"\\?\UNC\server\share\a.exe");
        assert_eq!(s(r"\\?\C:\a.exe"), r"\\?\C:\a.exe");
        assert_eq!(s("relativo.exe"), "relativo.exe");
        assert_eq!(version_info("C:\\a.exe\0altro"), Err(ExtractError::NotPresent));
    }
}
