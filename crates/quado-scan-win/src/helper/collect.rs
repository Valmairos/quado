// SPDX-License-Identifier: GPL-3.0-or-later
//! Raccolta con i privilegi (05 D3): `SeBackupPrivilege`, elenco delle cartelle negate con lo stesso
//! scanner, totali senza nomi, SID orfani, copie shadow con WMI dentro il processo (non `vssadmin`, il
//! cui output è localizzato), profilo dal registro.
//!
//! Nessuna lettura del contenuto dei file, nessun handle in scrittura: le aperture sono quelle di
//! [`crate::win`] (cartelle con `FILE_LIST_DIRECTORY`, file con `FILE_READ_ATTRIBUTES`), il registro si
//! apre con `KEY_READ`, il token del processo con `TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY`.
//!
//! # Perché con il privilegio lo scanner legge le cartelle negate
//!
//! Verificato su `win.rs` (M6): ogni apertura passa da `win::open`, che chiama `CreateFileW` sempre con
//! `FILE_FLAG_BACKUP_SEMANTICS` (oltre a `FILE_FLAG_OPEN_REPARSE_POINT` e `FILE_FLAG_OPEN_NO_RECALL`).
//! Con quel flag e `SeBackupPrivilege` abilitato nel token, Windows concede le richieste di lettura senza
//! guardare la DACL: il privilegio copre `READ_CONTROL`, `ACCESS_SYSTEM_SECURITY`, `FILE_GENERIC_READ` e
//! `FILE_TRAVERSE`. Le richieste dello scanner stanno tutte lì dentro:
//! - `win::open_dir`: `FILE_LIST_DIRECTORY` (lo stesso bit di `FILE_READ_DATA`) | `FILE_READ_ATTRIBUTES`
//!   | `SYNCHRONIZE`;
//! - `win::open_attributes` (correzione dei file WOF e sparse): `FILE_READ_ATTRIBUTES` | `SYNCHRONIZE`.
//!
//! Elenco (`GetFileInformationByHandleEx` con `FileIdExtdDirectoryInfo`), `FileStandardInfo` e le due
//! `FSCTL` di sola lettura usano l'handle già aperto, senza un altro controllo d'accesso. L'attraversamento
//! delle cartelle intermedie non dipende dalla DACL: `SeChangeNotifyPrivilege` ("Ignora controllo
//! incrociato") è abilitato per tutti, e il privilegio di backup comprende `FILE_TRAVERSE`. Un'ACL di
//! negazione per l'utente vale anche per il suo token elevato: senza il privilegio l'helper avrebbe lo
//! stesso errore 5 della scansione normale.
//!
//! # Senza elevazione (misurato su questo PC, 16/10/2026, `tests/helper_collect.rs`)
//!
//! - `enable_backup_privilege`: `Err(1300)` (`ERROR_NOT_ALL_ASSIGNED`). Il token filtrato di un
//!   amministratore non ha `SeBackupPrivilege`, e `AdjustTokenPrivileges` riesce lo stesso: l'esito sta
//!   in `GetLastError`.
//! - `shadow_storage`: `Err(1058)`. Su questo PC il servizio Copia shadow del volume (VSS) è
//!   disattivato e il provider di `Win32_ShadowStorage` non parte (`WBEM_E_INITIALIZATION_FAILURE`),
//!   anche da amministratore (prova elevata su `R:`, 16/10/2026): l'errore WMI diventa
//!   `ERROR_SERVICE_DISABLED`, che l'interfaccia spiega.

use std::ptr::{null, null_mut};

use quado_model::{Flags, Tree};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NONE_MAPPED, ERROR_NOT_ALL_ASSIGNED, ERROR_SERVICE_DISABLED, GetLastError, HANDLE, LUID,
    LocalFree,
};
use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupAccountSidW, LookupPrivilegeValueW, PSID, SE_BACKUP_NAME, SE_PRIVILEGE_ENABLED,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW;
use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QUERY_SERVICE_CONFIGW, QueryServiceConfigW, SC_MANAGER_CONNECT,
    SERVICE_DISABLED, SERVICE_QUERY_CONFIG,
};
use windows_sys::Win32::System::Registry::{HKEY, HKEY_LOCAL_MACHINE, KEY_READ, REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::proto::{ShadowStorage, Totals};
use super::serve::{Collector, FolderScan};
use crate::scan::{Options, Problem, Progress, RootError};

/// Raccolta reale.
pub struct Live;

fn last_error() -> u32 {
    // SAFETY: nessun prerequisito.
    unsafe { GetLastError() }
}

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Totali di un sottoalbero elencato, senza nomi (proto `Totals`): logica e allocata unica della radice
/// (hard link contati una volta dentro il sottoalbero), file della radice, cartelle esclusa la radice
/// (junction e collegamenti non seguiti compresi, come nodi), cartelle con un problema diverso da
/// `Cancelled` (la radice compresa, se il suo elenco si è interrotto a metà).
pub fn totals_of(tree: &Tree, problems: &[(u32, Problem)]) -> Totals {
    let dirs = (1..tree.len() as u32).filter(|&i| tree.flags(i).has(Flags::DIR)).count() as u64;
    let mut bad: Vec<u32> = problems.iter().filter(|(_, p)| *p != Problem::Cancelled).map(|(n, _)| *n).collect();
    bad.sort_unstable();
    bad.dedup();
    Totals {
        logical: tree.logical(Tree::ROOT),
        alloc: tree.alloc(Tree::ROOT),
        files: u64::from(tree.files(Tree::ROOT)),
        dirs,
        unreadable: bad.len() as u32,
    }
}

/// Un SID nella forma testuale `S-1-...`: solo cifre e trattini dopo il prefisso. Il SID finisce in un
/// percorso del registro (`profile_of`): niente barre né altri caratteri.
fn is_sid_text(s: &str) -> bool {
    s.len() <= 184 && s.starts_with("S-1-") && s[4..].bytes().all(|b| b.is_ascii_digit() || b == b'-') && !s.ends_with('-')
}

/// SID binario allocato da `ConvertStringSidToSidW`, liberato al drop.
struct Sid(PSID);

impl Sid {
    fn parse(s: &str) -> Result<Sid, u32> {
        let z = wide_z(s);
        let mut p: PSID = null_mut();
        // SAFETY: `z` è terminata da zero e vive per la chiamata; `p` riceve un SID allocato con
        // LocalAlloc, che `Drop` libera.
        let ok = unsafe { ConvertStringSidToSidW(z.as_ptr(), &mut p) };
        if ok == 0 { Err(last_error()) } else { Ok(Sid(p)) }
    }
}

impl Drop for Sid {
    fn drop(&mut self) {
        // SAFETY: `self.0` viene da ConvertStringSidToSidW (LocalAlloc) e si libera una volta sola.
        unsafe { LocalFree(self.0) };
    }
}

/// Handle del token, chiuso al drop.
struct Token(HANDLE);

impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: handle restituito da OpenProcessToken, chiuso una volta sola.
        unsafe { CloseHandle(self.0) };
    }
}

/// Chiave del registro aperta in lettura, chiusa al drop.
struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: chiave restituita da RegOpenKeyExW, chiusa una volta sola.
        unsafe { RegCloseKey(self.0) };
    }
}

/// Il servizio Copia shadow del volume (`VSS`) ha l'avvio "Disabilitato". Solo lettura della
/// configurazione (`SC_MANAGER_CONNECT`, `SERVICE_QUERY_CONFIG`); se non si legge, `false`.
pub fn vss_disabled() -> bool {
    // SAFETY: computer locale, database predefinito, diritto di sola connessione; l'handle si chiude sotto.
    let scm = unsafe { OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT) };
    if scm.is_null() {
        return false;
    }
    let name = wide_z("VSS");
    // SAFETY: handle del gestore valido; nome terminato da zero; solo lettura della configurazione.
    let svc = unsafe { OpenServiceW(scm, name.as_ptr(), SERVICE_QUERY_CONFIG) };
    let mut disabled = false;
    if !svc.is_null() {
        let mut need = 0u32;
        // SAFETY: prima chiamata senza buffer per la dimensione richiesta.
        unsafe { QueryServiceConfigW(svc, null_mut(), 0, &mut need) };
        // Buffer di u64 per l'allineamento della struttura.
        let mut buf = vec![0u64; (need as usize).div_ceil(8).max(1)];
        let len = (buf.len() * 8) as u32;
        // SAFETY: `buf` è lungo `len` byte e allineato per QUERY_SERVICE_CONFIGW.
        if unsafe { QueryServiceConfigW(svc, buf.as_mut_ptr().cast(), len, &mut need) } != 0 {
            // SAFETY: la chiamata è riuscita, quindi l'inizio di `buf` è una QUERY_SERVICE_CONFIGW valida.
            let cfg = unsafe { &*(buf.as_ptr().cast::<QUERY_SERVICE_CONFIGW>()) };
            disabled = cfg.dwStartType == SERVICE_DISABLED;
        }
        // SAFETY: handle del servizio valido, chiuso una volta.
        unsafe { CloseServiceHandle(svc) };
    }
    // SAFETY: handle del gestore valido, chiuso una volta.
    unsafe { CloseServiceHandle(scm) };
    disabled
}

/// GUID del volume (`{...}`, maiuscolo) di una radice `X:\`, da `GetVolumeNameForVolumeMountPointW`
/// (`\\?\Volume{...}\`).
pub fn volume_guid(volume: &str) -> Result<String, u32> {
    let mut root = volume.to_owned();
    if !root.ends_with('\\') {
        root.push('\\');
    }
    let z = wide_z(&root);
    let mut buf = [0u16; 64];
    // SAFETY: `z` terminata da zero; `buf` è un buffer di 64 caratteri dichiarato come tale (il nome del
    // volume ne occupa 49 con lo zero finale).
    let ok = unsafe { GetVolumeNameForVolumeMountPointW(z.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    if ok == 0 {
        return Err(last_error());
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    guid_in(&String::from_utf16_lossy(&buf[..len])).ok_or(1)
}

/// Il primo `{...}` di un nome di volume o di un riferimento WMI (`Win32_Volume.DeviceID="\\\\?\\Volume{...}\\"`),
/// in maiuscolo.
fn guid_in(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s[start..].find('}')? + start;
    Some(s[start..=end].to_ascii_uppercase())
}

impl Collector for Live {
    fn enable_backup_privilege(&self) -> Result<(), u32> {
        let mut h: HANDLE = null_mut();
        // SAFETY: pseudo-handle del processo corrente (non va chiuso); `h` riceve un handle del token,
        // chiuso da `Token`. Diritti: solo regolare i privilegi e leggerli.
        let ok = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut h) };
        if ok == 0 {
            return Err(last_error());
        }
        let token = Token(h);
        let mut luid = LUID { LowPart: 0, HighPart: 0 };
        // SAFETY: nome del privilegio costante terminato da zero; `luid` è un'uscita locale.
        if unsafe { LookupPrivilegeValueW(null(), SE_BACKUP_NAME, &mut luid) } == 0 {
            return Err(last_error());
        }
        let tp = TOKEN_PRIVILEGES { PrivilegeCount: 1, Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }] };
        // SAFETY: token aperto con TOKEN_ADJUST_PRIVILEGES; `tp` è una struttura completa con un solo
        // privilegio; nessuno stato precedente richiesto (buffer nullo e lunghezza 0).
        let ok = unsafe { AdjustTokenPrivileges(token.0, 0, &tp, 0, null_mut(), null_mut()) };
        // La funzione riesce anche se il token non ha il privilegio: l'esito vero è in GetLastError.
        let e = last_error();
        if ok == 0 {
            return Err(e);
        }
        if e == ERROR_NOT_ALL_ASSIGNED { Err(ERROR_NOT_ALL_ASSIGNED) } else { Ok(()) }
    }

    fn folder(&self, path: &str) -> Result<FolderScan, RootError> {
        let r = crate::scan(path, Options::default(), &Progress::default())?;
        Ok(FolderScan { tree: r.tree, problems: r.problems })
    }

    fn totals(&self, path: &str) -> Result<Totals, RootError> {
        let r = crate::scan(path, Options::default(), &Progress::default())?;
        // L'albero con i nomi finisce qui: esce solo il riepilogo.
        Ok(totals_of(&r.tree, &r.problems))
    }

    fn sid_is_orphan(&self, sid: &str) -> Result<bool, u32> {
        let sid = Sid::parse(sid)?;
        let mut name = [0u16; 256];
        let mut domain = [0u16; 256];
        let (mut cn, mut cd) = (name.len() as u32, domain.len() as u32);
        let mut kind = 0;
        // SAFETY: SID valido (da ConvertStringSidToSidW); i due buffer hanno la lunghezza dichiarata in
        // caratteri; `kind` è un'uscita locale. Sistema locale (nome nullo).
        let ok = unsafe { LookupAccountSidW(null(), sid.0, name.as_mut_ptr(), &mut cn, domain.as_mut_ptr(), &mut cd, &mut kind) };
        if ok != 0 {
            return Ok(false);
        }
        match last_error() {
            ERROR_NONE_MAPPED => Ok(true),
            // Trovato, ma con un nome più lungo dei buffer: l'account esiste.
            ERROR_INSUFFICIENT_BUFFER => Ok(false),
            e => Err(e),
        }
    }

    /// `Win32_ShadowStorage` del volume, con WMI dentro il processo. Le righe sono associazioni tra il
    /// volume delle copie (`Volume`) e quello che le ospita (`DiffVolume`): si sommano le righe del volume
    /// chiesto (di norma una). `UsedSpace`, `AllocatedSpace` e `MaxSpace` sono `uint64`, che WMI passa come
    /// stringhe; `MaxSpace` "senza limite" vale `u64::MAX`.
    ///
    /// Senza elevazione su questo PC (16/10/2026): `Err(0x80041014)` (`WBEM_E_INITIALIZATION_FAILURE`)
    /// sia per `C:\` sia per `R:\`, due volte di seguito nello stesso processo: il provider delle copie
    /// shadow non si inizializza per un utente che non è amministratore (lo stesso errore di
    /// `Get-CimInstance Win32_ShadowStorage` da PowerShell non elevato). L'altro codice atteso senza
    /// privilegi è `WBEM_E_ACCESS_DENIED` (0x80041003). Un volume che non esiste dà l'errore di
    /// `GetVolumeNameForVolumeMountPointW`, prima di WMI.
    fn shadow_storage(&self, volume: &str) -> Result<Option<ShadowStorage>, u32> {
        let guid = volume_guid(volume)?;
        // Con il servizio VSS disattivato il provider WMI non parte: si dice il motivo vero.
        wmi::shadow_storage(&guid).map_err(|e| if vss_disabled() { ERROR_SERVICE_DISABLED } else { e })
    }

    fn profile_of(&self, sid: &str) -> Option<String> {
        if !is_sid_text(sid) {
            return None;
        }
        let key = wide_z(&format!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\{sid}"));
        let mut hk: HKEY = null_mut();
        // SAFETY: chiave predefinita HKLM; sottochiave terminata da zero; solo lettura (KEY_READ); `hk`
        // riceve una chiave chiusa da `Key`.
        if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, key.as_ptr(), 0, KEY_READ, &mut hk) } != 0 {
            return None;
        }
        let hk = Key(hk);
        let value = wide_z("ProfileImagePath");
        let (mut kind, mut bytes) = (0u32, 0u32);
        // SAFETY: chiave aperta; nome terminato da zero; prima chiamata senza buffer per la dimensione.
        if unsafe { RegQueryValueExW(hk.0, value.as_ptr(), null(), &mut kind, null_mut(), &mut bytes) } != 0 {
            return None;
        }
        if kind != REG_SZ && kind != REG_EXPAND_SZ {
            return None;
        }
        // Un carattere in più: il valore può essere salvato senza lo zero finale.
        let mut buf = vec![0u16; bytes as usize / 2 + 1];
        let mut len = (buf.len() * 2) as u32;
        // SAFETY: `buf` è lungo `len` byte; tipo e lunghezza sono uscite locali.
        if unsafe { RegQueryValueExW(hk.0, value.as_ptr(), null(), &mut kind, buf.as_mut_ptr().cast(), &mut len) } != 0 {
            return None;
        }
        let n = (len as usize / 2).min(buf.len());
        let end = buf[..n].iter().position(|&c| c == 0).unwrap_or(n);
        let raw = &buf[..end];
        if kind == REG_SZ {
            return Some(String::from_utf16_lossy(raw));
        }
        // REG_EXPAND_SZ: variabili d'ambiente del processo (di norma `%SystemDrive%`).
        let src: Vec<u16> = raw.iter().copied().chain(std::iter::once(0)).collect();
        // SAFETY: `src` terminata da zero; destinazione nulla con dimensione 0 per sapere la lunghezza.
        let need = unsafe { ExpandEnvironmentStringsW(src.as_ptr(), null_mut(), 0) };
        if need == 0 {
            return None;
        }
        let mut out = vec![0u16; need as usize];
        // SAFETY: `out` ha `need` caratteri, come dichiarato.
        let got = unsafe { ExpandEnvironmentStringsW(src.as_ptr(), out.as_mut_ptr(), need) };
        if got == 0 || got > need {
            return None;
        }
        let end = out.iter().position(|&c| c == 0).unwrap_or(out.len());
        Some(String::from_utf16_lossy(&out[..end]))
    }
}

/// Copie shadow con WMI (`ROOT\CIMV2`, `Win32_ShadowStorage`), con il crate `windows`.
mod wmi {
    use windows::Win32::Foundation::RPC_E_TOO_LATE;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoInitializeSecurity, CoSetProxyBlanket,
        CoUninitialize, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_AUTHN_LEVEL_DEFAULT, RPC_C_IMP_LEVEL_IMPERSONATE,
    };
    use windows::Win32::System::Variant::{VARIANT, VT_BSTR, VT_I4, VT_I8, VT_UI4, VT_UI8, VariantClear};
    use windows::Win32::System::Wmi::{
        IWbemClassObject, IWbemLocator, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_GENERIC_FLAG_TYPE, WBEM_INFINITE,
        WbemLocator,
    };
    use windows::core::{BSTR, PCWSTR, w};

    use super::super::proto::ShadowStorage;

    /// `RPC_C_AUTHN_WINNT` e `RPC_C_AUTHZ_NONE` (`rpcdce.h`): la feature `Win32_System_Rpc` non serve
    /// per due costanti.
    const RPC_C_AUTHN_WINNT: u32 = 10;
    const RPC_C_AUTHZ_NONE: u32 = 0;

    const QUERY: &str = "SELECT AllocatedSpace, MaxSpace, UsedSpace, Volume FROM Win32_ShadowStorage";

    /// COM sul thread corrente per la durata del valore. Bilancia solo un'inizializzazione riuscita
    /// (S_OK o S_FALSE); con `RPC_E_CHANGED_MODE` il thread è già in un altro modello e si usa così.
    struct Com(bool);

    impl Drop for Com {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: bilancia la CoInitializeEx riuscita sullo stesso thread; gli oggetti COM di
                // `rows` sono già stati rilasciati.
                unsafe { CoUninitialize() };
            }
        }
    }

    fn code(e: &windows::core::Error) -> u32 {
        e.code().0 as u32
    }

    pub(super) fn shadow_storage(guid: &str) -> Result<Option<ShadowStorage>, u32> {
        // SAFETY: inizializzazione COM del thread corrente, bilanciata da `Com`.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_err() && hr != windows::Win32::Foundation::RPC_E_CHANGED_MODE {
            return Err(hr.0 as u32);
        }
        let _com = Com(hr.is_ok());
        // Sicurezza COM del processo: una volta sola. Se qualcuno l'ha già fissata (o COM l'ha fissata
        // da sé), RPC_E_TOO_LATE: valgono quelle impostazioni, e il proxy si regola sotto.
        // SAFETY: descrittore nullo, nessun elenco di servizi né di autenticazioni; costanti documentate.
        let sec = unsafe {
            CoInitializeSecurity(None, -1, None, None, RPC_C_AUTHN_LEVEL_DEFAULT, RPC_C_IMP_LEVEL_IMPERSONATE, None, EOAC_NONE, None)
        };
        if let Err(e) = sec
            && e.code() != RPC_E_TOO_LATE
        {
            return Err(code(&e));
        }
        rows(guid).map_err(|e| code(&e))
    }

    fn rows(guid: &str) -> windows::core::Result<Option<ShadowStorage>> {
        // SAFETY: COM è inizializzato sul thread (sopra); il localizzatore WMI è un server in-process.
        let locator: IWbemLocator = unsafe { CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)? };
        let empty = BSTR::new();
        // SAFETY: stringhe BSTR valide per la chiamata; credenziali dell'utente del processo (vuote).
        let services = unsafe { locator.ConnectServer(&BSTR::from(r"ROOT\CIMV2"), &empty, &empty, &empty, 0, &empty, None)? };
        // SAFETY: proxy appena ottenuto; nome del principale nullo; costanti documentate.
        unsafe {
            CoSetProxyBlanket(
                &services,
                RPC_C_AUTHN_WINNT,
                RPC_C_AUTHZ_NONE,
                PCWSTR::null(),
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            )?
        };
        let flags = WBEM_GENERIC_FLAG_TYPE(WBEM_FLAG_FORWARD_ONLY.0 | WBEM_FLAG_RETURN_IMMEDIATELY.0);
        // SAFETY: stringhe BSTR valide per la chiamata; nessun contesto.
        let rows = unsafe { services.ExecQuery(&BSTR::from("WQL"), &BSTR::from(QUERY), flags, None)? };
        let mut found: Option<ShadowStorage> = None;
        loop {
            let mut row: [Option<IWbemClassObject>; 1] = [None];
            let mut n = 0u32;
            // SAFETY: un elemento d'uscita, `n` locale. Con WBEM_FLAG_RETURN_IMMEDIATELY gli errori del
            // provider (accesso negato) arrivano qui.
            unsafe { rows.Next(WBEM_INFINITE, &mut row, &mut n) }.ok()?;
            let Some(obj) = row[0].take().filter(|_| n > 0) else { break };
            let volume = match prop(&obj, w!("Volume"))? {
                Prop::Text(s) => s,
                _ => continue,
            };
            if super::guid_in(&volume).as_deref() != Some(guid) {
                continue;
            }
            let f = found.get_or_insert_with(ShadowStorage::default);
            f.used = f.used.saturating_add(prop(&obj, w!("UsedSpace"))?.number());
            f.allocated = f.allocated.saturating_add(prop(&obj, w!("AllocatedSpace"))?.number());
            f.max = f.max.saturating_add(prop(&obj, w!("MaxSpace"))?.number());
        }
        Ok(found)
    }

    enum Prop {
        Text(String),
        Number(u64),
        Missing,
    }

    impl Prop {
        /// `uint64` di WMI: stringa decimale; un valore mancante o non numerico conta 0.
        fn number(&self) -> u64 {
            match self {
                Prop::Number(n) => *n,
                Prop::Text(s) => s.trim().parse().unwrap_or(0),
                Prop::Missing => 0,
            }
        }
    }

    fn prop(obj: &IWbemClassObject, name: PCWSTR) -> windows::core::Result<Prop> {
        let mut v = VARIANT::default();
        // SAFETY: nome costante terminato da zero; `v` azzerato (VT_EMPTY) e liberato sotto.
        unsafe { obj.Get(name, 0, &mut v, None, None)? };
        // SAFETY: `Get` ha scritto un VARIANT valido; si legge il campo indicato da `vt` e poi lo si
        // libera con VariantClear (il BSTR resta in `ManuallyDrop` e non si libera due volte).
        let out = unsafe {
            let inner = &v.Anonymous.Anonymous;
            match inner.vt {
                VT_BSTR => Prop::Text(inner.Anonymous.bstrVal.to_string()),
                VT_UI8 => Prop::Number(inner.Anonymous.ullVal),
                VT_I8 => Prop::Number(inner.Anonymous.llVal.max(0) as u64),
                VT_UI4 => Prop::Number(u64::from(inner.Anonymous.ulVal)),
                VT_I4 => Prop::Number(inner.Anonymous.lVal.max(0) as u64),
                _ => Prop::Missing,
            }
        };
        // SAFETY: `v` è un VARIANT inizializzato da `Get`; dopo la liberazione non si usa più.
        let _ = unsafe { VariantClear(&mut v) };
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sid_text() {
        assert!(is_sid_text("S-1-5-18"));
        assert!(is_sid_text("S-1-5-21-1-2-3-9999"));
        assert!(!is_sid_text("S-1-5-18\\..\\x"));
        assert!(!is_sid_text("S-1-"));
        assert!(!is_sid_text("S-1-5-"));
        assert!(!is_sid_text("s-1-5-18"));
        assert!(!is_sid_text("S-1-5-18 "));
    }

    #[test]
    fn guid_of_volume_and_wmi_reference() {
        let wmi = r#"Win32_Volume.DeviceID="\\\\?\\Volume{4a3b2c1d-0000-0000-0000-100000000000}\\""#;
        assert_eq!(guid_in(wmi).as_deref(), Some("{4A3B2C1D-0000-0000-0000-100000000000}"));
        assert_eq!(guid_in(r"\\?\Volume{4a3b2c1d-0000-0000-0000-100000000000}\"), guid_in(wmi));
        assert_eq!(guid_in("nessuno"), None);
    }
}
