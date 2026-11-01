// SPDX-License-Identifier: GPL-3.0-or-later
//! Estrattori su Windows vero, in sola lettura (05 D6): registro con `KEY_READ`, file entro un limite
//! di dimensione, cartelle note, API di alimentazione, `PackageManager`, `SHQueryRecycleBinW`.
//! Nessun processo creato, nessuna chiave aperta in scrittura (06 §2.4). Unico modulo del crate con
//! codice `unsafe`; ogni blocco ha il suo invariante.

use std::ffi::c_void;

use windows_sys::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, LocalFree};
use windows_sys::Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Power::{CallNtPowerInformation, PowerGetActiveScheme, PowerReadDCValueIndex, SYSTEM_POWER_CAPABILITIES, SystemPowerCapabilities};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, REG_BINARY, REG_DWORD, REG_EXPAND_SZ, REG_MULTI_SZ, REG_QWORD, REG_SZ, RegCloseKey,
    RegEnumKeyExW, RegEnumValueW, RegOpenKeyExW, RegQueryInfoKeyW,
};
use windows_sys::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_LocalAppData, FOLDERID_LocalAppDataLow, FOLDERID_Music, FOLDERID_Pictures, FOLDERID_Profile, FOLDERID_ProgramData,
    FOLDERID_ProgramFilesX64, FOLDERID_ProgramFilesX86, FOLDERID_RoamingAppData, FOLDERID_Videos, FOLDERID_Windows, SHGetKnownFolderPath, SHQUERYRBINFO,
    SHQueryRecycleBinW,
};
use windows_sys::core::GUID;

use crate::evidence::{Evidence, KnownFolders, RecycleQuery};
use crate::extract::{self, ExtractError, FileSource, FormatIssue, MAX_FILE, MsixPackage, Power};
use crate::reg::{RegValue, RegistrySource, Root};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Chiave aperta in sola lettura, chiusa all'uscita.
struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: la chiave è stata aperta da `RegOpenKeyExW` e si chiude una volta sola.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

fn open(root: Root, path: &str) -> Result<Key, u32> {
    let base = match root {
        Root::LocalMachine => HKEY_LOCAL_MACHINE,
        Root::CurrentUser => HKEY_CURRENT_USER,
    };
    let p = wide(path);
    let mut h: HKEY = std::ptr::null_mut();
    // SAFETY: `p` è terminata da zero e vive per la chiamata; `h` riceve la chiave. Solo `KEY_READ`:
    // nessuna apertura in scrittura (06 §2.4).
    let r = unsafe { RegOpenKeyExW(base, p.as_ptr(), 0, KEY_READ | KEY_WOW64_64KEY, &mut h) };
    if r == ERROR_SUCCESS { Ok(Key(h)) } else { Err(r) }
}

/// Registro vero, in sola lettura.
pub struct LiveRegistry;

impl RegistrySource for LiveRegistry {
    fn subkeys(&self, root: Root, path: &str) -> Result<Vec<String>, u32> {
        let k = open(root, path)?;
        let mut out = Vec::new();
        let mut buf = vec![0u16; 512];
        for i in 0.. {
            let mut len = buf.len() as u32;
            // SAFETY: `buf` ha `len` caratteri; gli altri puntatori facoltativi sono nulli.
            let r = unsafe {
                RegEnumKeyExW(k.0, i, buf.as_mut_ptr(), &mut len, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut())
            };
            if r == ERROR_NO_MORE_ITEMS {
                break;
            }
            if r != ERROR_SUCCESS {
                return Err(r);
            }
            out.push(String::from_utf16_lossy(&buf[..len as usize]));
        }
        Ok(out)
    }

    fn values(&self, root: Root, path: &str) -> Result<Vec<(String, RegValue)>, u32> {
        let k = open(root, path)?;
        let (mut name_max, mut data_max) = (0u32, 0u32);
        // SAFETY: solo i contatori richiesti, gli altri puntatori sono nulli.
        let r = unsafe {
            RegQueryInfoKeyW(
                k.0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut name_max,
                &mut data_max,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if r != ERROR_SUCCESS {
            return Err(r);
        }
        let mut name = vec![0u16; name_max as usize + 1];
        let mut data = vec![0u8; data_max as usize + 2];
        let mut out = Vec::new();
        for i in 0.. {
            let mut nlen = name.len() as u32;
            let mut dlen = data.len() as u32;
            let mut ty = 0u32;
            // SAFETY: i buffer hanno le dimensioni dichiarate, lette da `RegQueryInfoKeyW`.
            let r = unsafe { RegEnumValueW(k.0, i, name.as_mut_ptr(), &mut nlen, std::ptr::null(), &mut ty, data.as_mut_ptr(), &mut dlen) };
            if r == ERROR_NO_MORE_ITEMS {
                break;
            }
            if r != ERROR_SUCCESS {
                return Err(r);
            }
            let n = String::from_utf16_lossy(&name[..nlen as usize]);
            let d = &data[..dlen as usize];
            let text = |d: &[u8]| -> Vec<u16> { d.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect() };
            let v = match ty {
                REG_SZ | REG_EXPAND_SZ => {
                    let w = text(d);
                    let end = w.iter().position(|&c| c == 0).unwrap_or(w.len());
                    let s = String::from_utf16_lossy(&w[..end]);
                    if ty == REG_SZ { RegValue::Sz(s) } else { RegValue::ExpandSz(s) }
                }
                REG_MULTI_SZ => RegValue::MultiSz(text(d).split(|&c| c == 0).filter(|p| !p.is_empty()).map(String::from_utf16_lossy).collect()),
                REG_DWORD if d.len() == 4 => RegValue::Dword(u32::from_le_bytes([d[0], d[1], d[2], d[3]])),
                REG_QWORD if d.len() == 8 => RegValue::Qword(u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]])),
                REG_BINARY => RegValue::Binary(d.to_vec()),
                other => RegValue::Other(other),
            };
            out.push((n, v));
        }
        Ok(out)
    }
}

/// File veri, in sola lettura, entro `MAX_FILE`.
pub struct LiveFiles;

impl FileSource for LiveFiles {
    fn read(&self, path: &str) -> Result<String, ExtractError> {
        let p = std::path::Path::new(path);
        let meta = match std::fs::metadata(p) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ExtractError::NotPresent),
            Err(e) => return Err(ExtractError::Io(e.raw_os_error().unwrap_or(0) as u32)),
        };
        if meta.len() > MAX_FILE {
            return Err(FormatIssue::TooLarge.into());
        }
        let bytes = std::fs::read(p).map_err(|e| ExtractError::Io(e.raw_os_error().unwrap_or(0) as u32))?;
        String::from_utf8(bytes).map_err(|_| FormatIssue::Encoding.into())
    }

    fn list(&self, dir: &str) -> Result<Vec<String>, ExtractError> {
        let rd = match std::fs::read_dir(dir) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ExtractError::NotPresent),
            Err(e) => return Err(ExtractError::Io(e.raw_os_error().unwrap_or(0) as u32)),
        };
        Ok(rd.filter_map(Result::ok).filter_map(|e| e.file_name().into_string().ok()).collect())
    }
}

fn known(id: &GUID) -> Option<String> {
    let mut p: *mut u16 = std::ptr::null_mut();
    // SAFETY: `id` è un GUID valido; `p` riceve una stringa allocata con CoTaskMemAlloc, letta e liberata qui.
    unsafe {
        if SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut p) < 0 || p.is_null() {
            if !p.is_null() {
                CoTaskMemFree(p as *const c_void);
            }
            return None;
        }
        let mut len = 0;
        while *p.add(len) != 0 {
            len += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
        CoTaskMemFree(p as *const c_void);
        Some(extract::normalize(&s))
    }
}

pub fn known_folders() -> KnownFolders {
    KnownFolders {
        profile: known(&FOLDERID_Profile),
        local: known(&FOLDERID_LocalAppData),
        local_low: known(&FOLDERID_LocalAppDataLow),
        roaming: known(&FOLDERID_RoamingAppData),
        program_data: known(&FOLDERID_ProgramData),
        windows: known(&FOLDERID_Windows),
        program_files: known(&FOLDERID_ProgramFilesX64),
        program_files_x86: known(&FOLDERID_ProgramFilesX86),
        documents: known(&FOLDERID_Documents),
        downloads: known(&FOLDERID_Downloads),
        desktop: known(&FOLDERID_Desktop),
        pictures: known(&FOLDERID_Pictures),
        videos: known(&FOLDERID_Videos),
        music: known(&FOLDERID_Music),
    }
}

/// `{GUID}` del volume (`\\?\Volume{...}\`), per la chiave `BitBucket`.
pub fn volume_guid(root: &str) -> Option<String> {
    let r = wide(root);
    let mut buf = [0u16; 64];
    // SAFETY: `r` è terminata da zero, `buf` ha la dimensione dichiarata.
    let ok = unsafe { GetVolumeNameForVolumeMountPointW(r.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    if ok == 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..end]);
    let a = s.find('{')?;
    let b = s.find('}')?;
    Some(s[a..=b].to_owned())
}

/// Impostazione "azione per batteria critica" (`BATACTIONCRIT`) del sottogruppo batteria.
const SUB_BATTERY: GUID = GUID::from_u128(0xe73a048d_bf27_4f12_9731_8b2076e8891f);
const BATACTIONCRIT: GUID = GUID::from_u128(0x637ea02f_bbcb_4015_8e2c_a1c7b9c0b546);

pub fn power(reg: &dyn RegistrySource) -> Result<Power, ExtractError> {
    // SAFETY: SYSTEM_POWER_CAPABILITIES è una struttura C di soli valori; la dimensione è passata.
    let caps = unsafe {
        let mut c: SYSTEM_POWER_CAPABILITIES = std::mem::zeroed();
        let st = CallNtPowerInformation(SystemPowerCapabilities, std::ptr::null(), 0, (&raw mut c).cast(), std::mem::size_of::<SYSTEM_POWER_CAPABILITIES>() as u32);
        if st < 0 {
            return Err(ExtractError::Io(st as u32));
        }
        c
    };
    let fast_startup = match reg.value(Root::LocalMachine, r"SYSTEM\CurrentControlSet\Control\Session Manager\Power", "HiberbootEnabled") {
        Ok(Some(RegValue::Dword(v))) => Some(v == 1),
        _ => None,
    };
    let mut critical = None;
    // SAFETY: `scheme` riceve un GUID allocato da Windows, liberato con LocalFree; `v` riceve il valore.
    unsafe {
        let mut scheme: *mut GUID = std::ptr::null_mut();
        if PowerGetActiveScheme(std::ptr::null_mut(), &mut scheme) == ERROR_SUCCESS && !scheme.is_null() {
            let mut v = 0u32;
            if PowerReadDCValueIndex(std::ptr::null_mut(), scheme, &SUB_BATTERY, &BATACTIONCRIT, &mut v) == ERROR_SUCCESS {
                critical = Some(v);
            }
            LocalFree(scheme.cast());
        }
    }
    Ok(Power {
        hibernate: caps.SystemS4 && caps.HiberFilePresent,
        hiberfile_present: caps.HiberFilePresent,
        fast_startup,
        has_battery: caps.SystemBatteriesPresent,
        modern_standby: caps.AoAc,
        critical_battery_action: critical,
    })
}

pub fn recycle_bin(root: &str) -> Result<RecycleQuery, ExtractError> {
    let r = wide(root);
    let mut info = SHQUERYRBINFO { cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32, i64Size: 0, i64NumItems: 0 };
    // SAFETY: `r` è terminata da zero, `info` ha `cbSize` impostato.
    let hr = unsafe { SHQueryRecycleBinW(r.as_ptr(), &mut info) };
    if hr < 0 {
        return Err(ExtractError::Io(hr as u32));
    }
    Ok(RecycleQuery { bytes: info.i64Size.max(0) as u64, items: info.i64NumItems.max(0) as u64 })
}

/// Pacchetti MSIX dell'utente corrente (`PackageManager.FindPackagesForUser("")`), in un thread
/// con il proprio apartment.
pub fn msix() -> Result<Vec<MsixPackage>, ExtractError> {
    let h = std::thread::spawn(|| -> Result<Vec<MsixPackage>, ExtractError> {
        use windows::Management::Deployment::PackageManager;
        use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
        // SAFETY: inizializzazione di WinRT nel thread appena creato, senza altri apartment attivi.
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let io = |e: windows::core::Error| ExtractError::Io(e.code().0 as u32);
        let pm = PackageManager::new().map_err(io)?;
        let it = pm.FindPackagesByUserSecurityId(&windows::core::HSTRING::new()).map_err(io)?;
        let mut out = Vec::new();
        for p in it {
            let Ok(id) = p.Id() else { continue };
            let Ok(pfn) = id.FamilyName() else { continue };
            out.push(MsixPackage {
                family_name: pfn.to_string(),
                installed_path: p.InstalledPath().ok().map(|s| s.to_string()).filter(|s| !s.is_empty()),
                framework: p.IsFramework().unwrap_or(false),
            });
        }
        out.sort_by(|a, b| a.family_name.cmp(&b.family_name));
        out.dedup_by(|a, b| a.family_name == b.family_name);
        extract::validate_msix(&out)?;
        Ok(out)
    });
    h.join().unwrap_or(Err(ExtractError::Io(0)))
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.trim().is_empty())
}

/// Raccoglie le prove per un volume (`C:\`). `user_sid` viene dalla scansione.
pub fn gather(volume_root: &str, user_sid: Option<String>) -> Evidence {
    let reg = LiveRegistry;
    let files = LiveFiles;
    let msix_job = std::thread::spawn(msix);
    let folders = known_folders();
    let local = folders.local.clone().unwrap_or_default();
    let npmrc = folders.profile.as_deref().and_then(|p| files.read(&extract::join(p, ".npmrc")).ok());
    let npm_cache = extract::tool_path(env("npm_config_cache"), npmrc.as_deref(), "cache", extract::join(&local, "npm-cache"));
    let pnpm_env = env("npm_config_store_dir").or_else(|| env("PNPM_HOME").map(|h| extract::join(&h, "store")));
    let pnpm_store = extract::tool_path(pnpm_env, npmrc.as_deref(), "store-dir", extract::join(&extract::join(&local, "pnpm"), "store"));
    let system_volume = folders.windows.as_deref().is_some_and(|w| w.to_ascii_lowercase().starts_with(&volume_root.to_ascii_lowercase()));
    let bin = match volume_guid(volume_root) {
        Some(g) => extract::bin_settings(&reg, &g),
        None => Err(ExtractError::NotPresent),
    };
    let restore = match reg.values(Root::LocalMachine, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\SPP\Clients") {
        Ok(v) => {
            let g = volume_guid(volume_root).unwrap_or_default().to_ascii_lowercase();
            Ok(v.iter().any(|(_, x)| matches!(x, RegValue::MultiSz(list) if list.iter().any(|s| !g.is_empty() && s.to_ascii_lowercase().contains(&g)))))
        }
        Err(e) => Err(ExtractError::Io(e)),
    };
    Evidence {
        volume_root: volume_root.to_owned(),
        system_volume,
        user_sid,
        uninstall: extract::uninstall(&reg),
        steam: extract::steam(&reg, &files, volume_root),
        epic: folders.program_data.as_deref().map_or(Err(ExtractError::NotPresent), |pd| extract::epic(&files, pd)),
        capcut: extract::capcut(&files, &local),
        capcut_drafts: extract::capcut_drafts(&files, &local),
        track_progs: extract::track_progs(&reg),
        userassist: extract::userassist(&reg),
        power: power(&reg),
        storage_sense: extract::storage_sense(&reg),
        bin,
        recycle: recycle_bin(volume_root),
        npm_cache,
        pnpm_store,
        restore,
        msix: msix_job.join().unwrap_or(Err(ExtractError::Io(0))),
        folders,
    }
}

// ---------------------------------------------------------------------------------------------
// Riferimenti (03 F-AC-9, 05 D7 controllo 6): collegamenti `.lnk` e attività pianificate, in sola
// lettura. COM si inizializza e si bilancia in ogni funzione che lo usa: il chiamante può essere un
// thread qualunque, e gli oggetti COM si rilasciano prima di `CoUninitialize`.

use std::os::windows::fs::MetadataExt;

use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, STGM_READ};
use windows::Win32::System::TaskScheduler::{IExecAction, IRegisteredTask, ITaskService, TASK_ACTION_EXEC, TASK_ACTION_TYPE, TASK_ENUM_HIDDEN, TaskScheduler};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Shell::{IShellLinkW, SLGP_RAWPATH, ShellLink};
use windows::core::{BSTR, HSTRING, Interface};
use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS, FILE_ATTRIBUTE_RECALL_ON_OPEN};
use windows_sys::Win32::UI::Shell::{FOLDERID_CommonStartMenu, FOLDERID_PublicDesktop, FOLDERID_StartMenu, FOLDERID_System};

use crate::references::{CLOUD_PLACEHOLDER, ScheduledTask, Shortcut, ShortcutList, ShortcutSource, TaskList, TaskSource};

/// COM sul thread corrente per la durata del valore; bilancia solo un'inizializzazione riuscita (come
/// la guardia di `quado-actions/src/shell.rs`).
struct ComGuard(bool);

impl ComGuard {
    fn init() -> Self {
        // SAFETY: inizializzazione COM del thread corrente. S_OK e S_FALSE vanno bilanciati; con
        // RPC_E_CHANGED_MODE il thread è già in un altro modello, COM è utilizzabile e non si tocca.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        ComGuard(hr.is_ok())
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: bilancia la CoInitializeEx riuscita di `init` sullo stesso thread; gli oggetti COM
            // della funzione sono già stati rilasciati (dichiarati dopo la guardia).
            unsafe { CoUninitialize() };
        }
    }
}

/// Codice di Windows di un HRESULT: il codice Win32 se è un `HRESULT_FROM_WIN32` (0x8007xxxx, per
/// esempio 5 per accesso negato), altrimenti l'HRESULT intero.
fn hr_code(hr: i32) -> u32 {
    let v = hr as u32;
    if v & 0xFFFF_0000 == 0x8007_0000 { v & 0xFFFF } else { v }
}

fn io_code(e: &std::io::Error) -> u32 {
    e.raw_os_error().unwrap_or(0) as u32
}

/// Cartella nota, con il codice di errore se non si legge.
fn known_path(id: &GUID) -> Result<String, u32> {
    let mut p: *mut u16 = std::ptr::null_mut();
    // SAFETY: `id` è un GUID valido; `p` riceve una stringa allocata con CoTaskMemAlloc, letta e liberata
    // qui (anche in caso di errore, se non nulla).
    unsafe {
        let hr = SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut p);
        if hr < 0 || p.is_null() {
            if !p.is_null() {
                CoTaskMemFree(p as *const c_void);
            }
            return Err(hr_code(hr));
        }
        let mut len = 0;
        while *p.add(len) != 0 {
            len += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
        CoTaskMemFree(p as *const c_void);
        Ok(extract::normalize(&s))
    }
}

/// Segnaposto cloud: il contenuto (o l'elenco, per una cartella) si chiederebbe al provider.
fn cloud_placeholder(attrs: u32) -> bool {
    attrs & (FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_OFFLINE) != 0
}

/// Bersaglio grezzo di un `.lnk` (`GetPath` con `SLGP_RAWPATH`); `None` se il collegamento non ha un
/// percorso nel file system. COM è inizializzato dal chiamante.
fn link_target(path: &str, buf: &mut [u16]) -> Result<Option<String>, u32> {
    let code = |e: windows::core::Error| hr_code(e.code().0);
    // SAFETY: CLSID del collegamento della shell e la sua interfaccia; COM è inizializzato sul thread.
    let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.map_err(code)?;
    let file: IPersistFile = link.cast().map_err(code)?;
    // SAFETY: percorso terminato da zero valido per la chiamata; `STGM_READ`: il file si legge soltanto.
    unsafe { file.Load(&HSTRING::from(path), STGM_READ) }.map_err(code)?;
    buf[0] = 0;
    // SAFETY: `buf` è un buffer locale con la sua lunghezza; nessuna WIN32_FIND_DATAW (puntatore nullo,
    // ammesso). `SLGP_RAWPATH`: il percorso come scritto nel file, senza espansione né ricerca. Non si
    // chiama mai `Resolve`: nessuna ricerca del bersaglio, nessun accesso di rete.
    unsafe { link.GetPath(buf, std::ptr::null_mut(), SLGP_RAWPATH.0 as u32) }.map_err(code)?;
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..n]);
    Ok((!s.trim().is_empty()).then_some(s))
}

/// Collegamenti `.lnk` letti in sola lettura con `IPersistFile::Load` (`STGM_READ`) e
/// `IShellLinkW::GetPath` (`SLGP_RAWPATH`), mai `Resolve`. Le cartelle si visitano ricorsivamente senza
/// seguire collegamenti simbolici e giunzioni; i segnaposto cloud non si aprono e non si elencano
/// (`CLOUD_PLACEHOLDER`).
pub struct LiveShortcuts {
    folders: Vec<String>,
    /// Cartelle note non lette: nome del `FOLDERID` e codice.
    missing: Vec<(String, u32)>,
}

impl LiveShortcuts {
    /// Collegamenti sotto queste cartelle (per esempio una cartella temporanea nei test).
    pub fn new(folders: Vec<String>) -> Self {
        Self { folders, missing: Vec::new() }
    }

    /// Start e Desktop dell'utente e pubblici: `FOLDERID_StartMenu`, `FOLDERID_CommonStartMenu`,
    /// `FOLDERID_Desktop`, `FOLDERID_PublicDesktop`.
    pub fn system() -> Self {
        let mut s = Self::new(Vec::new());
        let ids = [
            ("FOLDERID_StartMenu", &FOLDERID_StartMenu),
            ("FOLDERID_CommonStartMenu", &FOLDERID_CommonStartMenu),
            ("FOLDERID_Desktop", &FOLDERID_Desktop),
            ("FOLDERID_PublicDesktop", &FOLDERID_PublicDesktop),
        ];
        for (name, id) in ids {
            match known_path(id) {
                Ok(p) => {
                    if !s.folders.iter().any(|f| f.eq_ignore_ascii_case(&p)) {
                        s.folders.push(p);
                    }
                }
                Err(e) => s.missing.push((name.to_owned(), e)),
            }
        }
        s
    }

    /// Le cartelle visitate.
    pub fn folders(&self) -> &[String] {
        &self.folders
    }
}

impl ShortcutSource for LiveShortcuts {
    fn shortcuts(&self) -> ShortcutList {
        let _com = ComGuard::init();
        let mut out = ShortcutList { links: Vec::new(), errors: self.missing.clone() };
        let mut seen = std::collections::HashSet::new();
        let mut buf = vec![0u16; 32_768];
        for root in &self.folders {
            match std::fs::metadata(root) {
                Ok(m) if cloud_placeholder(m.file_attributes()) => {
                    out.errors.push((root.clone(), CLOUD_PLACEHOLDER));
                    continue;
                }
                Ok(_) => {}
                Err(e) => {
                    out.errors.push((root.clone(), io_code(&e)));
                    continue;
                }
            }
            let mut stack = vec![std::path::PathBuf::from(root)];
            while let Some(dir) = stack.pop() {
                let rd = match std::fs::read_dir(&dir) {
                    Ok(r) => r,
                    Err(e) => {
                        out.errors.push((dir.to_string_lossy().into_owned(), io_code(&e)));
                        continue;
                    }
                };
                for entry in rd {
                    let entry = match entry {
                        Ok(x) => x,
                        Err(e) => {
                            out.errors.push((dir.to_string_lossy().into_owned(), io_code(&e)));
                            continue;
                        }
                    };
                    let path = entry.path();
                    let p = path.to_string_lossy().into_owned();
                    // Su Windows i metadati della voce vengono dall'elenco: nessuna apertura, nessun
                    // collegamento seguito.
                    let meta = match entry.metadata() {
                        Ok(m) => m,
                        Err(e) => {
                            out.errors.push((p, io_code(&e)));
                            continue;
                        }
                    };
                    let ft = meta.file_type();
                    if ft.is_symlink() {
                        continue;
                    }
                    if ft.is_dir() {
                        if cloud_placeholder(meta.file_attributes()) {
                            out.errors.push((p, CLOUD_PLACEHOLDER));
                        } else {
                            stack.push(path);
                        }
                        continue;
                    }
                    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
                        continue;
                    }
                    if cloud_placeholder(meta.file_attributes()) {
                        out.errors.push((p, CLOUD_PLACEHOLDER));
                        continue;
                    }
                    if !seen.insert(p.to_lowercase()) {
                        continue;
                    }
                    match link_target(&p, &mut buf) {
                        Ok(Some(target)) => out.links.push(Shortcut { link: p, target }),
                        Ok(None) => {}
                        Err(c) => out.errors.push((p, c)),
                    }
                }
            }
        }
        out
    }
}

/// Attività pianificate visibili all'utente, in sola lettura: `ITaskService::Connect` locale con
/// l'utente corrente (nessun nome, nessuna password), cartelle ricorsive da `\`,
/// `GetTasks(TASK_ENUM_HIDDEN)`, azioni `IExecAction::Path`. Cartelle e attività negate vanno tra le non
/// leggibili. Nessuna attività si crea, si modifica o si avvia.
///
/// `GetTasks` e `GetFolders` omettono senza errore le attività e le cartelle che l'utente non può leggere
/// (sul PC di sviluppo, il 14/10/2026: 166 attività visibili, 48 tra attività e cartelle di sistema omesse), e
/// senza amministratore né `TaskCache` nel registro né la cartella `System32\Tasks` si elencano (codice
/// 5). Le sue sottocartelle invece sì, quasi tutte: per ogni cartella della libreria si elencano i nomi
/// della cartella `System32\Tasks` corrispondente (solo i nomi, nessun contenuto), e ogni attività o
/// cartella che c'è ma che la libreria non ha mostrato va tra le non leggibili con il codice 5. Una
/// cartella che non si elenca (la radice, senza amministratore) non si confronta: le sue attività
/// invisibili restano non rilevate.
pub struct LiveTasks;

impl TaskSource for LiveTasks {
    fn tasks(&self) -> TaskList {
        let _com = ComGuard::init();
        let mut out = TaskList::default();
        let mut folders = Vec::new();
        if let Err(e) = read_tasks(&mut out, &mut folders) {
            out.unreadable.push(e);
        }
        if let Ok(system) = known_path(&FOLDERID_System) {
            invisible_tasks(&extract::join(&system, "Tasks"), &folders, &mut out);
        }
        out.tasks.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }
}

/// Attività e cartelle che stanno in `System32\Tasks` ma che la libreria non ha mostrato all'utente:
/// tra le non leggibili, con il codice 5. Solo elenchi di cartelle, nessun file aperto.
fn invisible_tasks(store: &str, folders: &[String], out: &mut TaskList) {
    let tasks: std::collections::HashSet<String> = out.tasks.iter().map(|t| t.path.to_lowercase()).collect();
    let known: std::collections::HashSet<String> = folders.iter().map(|f| f.to_lowercase()).collect();
    for f in folders {
        let dir = if f == r"\" { store.to_owned() } else { format!("{store}{f}") };
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if f == r"\" { format!(r"\{name}") } else { format!(r"{f}\{name}") };
            let Ok(ft) = entry.file_type() else { continue };
            let visible = if ft.is_dir() { known.contains(&path.to_lowercase()) } else { tasks.contains(&path.to_lowercase()) };
            let already = out.unreadable.iter().any(|(p, _)| p.eq_ignore_ascii_case(&path));
            if !visible && !already {
                out.unreadable.push((path, 5));
            }
        }
    }
}

/// Visita la libreria delle attività e annota le cartelle visitate. `Err` se il servizio o la cartella
/// radice non si leggono.
fn read_tasks(out: &mut TaskList, folders: &mut Vec<String>) -> Result<(), (String, u32)> {
    let root_err = |e: windows::core::Error| (r"\".to_owned(), hr_code(e.code().0));
    // SAFETY: CLSID dell'Utilità di pianificazione e la sua interfaccia; COM è inizializzato sul thread.
    let service: ITaskService = unsafe { CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER) }.map_err(root_err)?;
    let empty = VARIANT::default();
    // SAFETY: VARIANT vuoti, validi per la chiamata: computer locale, utente corrente, nessuna password.
    unsafe { service.Connect(&empty, &empty, &empty, &empty) }.map_err(root_err)?;
    // SAFETY: BSTR valido per la chiamata; il servizio è connesso.
    let root = unsafe { service.GetFolder(&BSTR::from(r"\")) }.map_err(root_err)?;
    let mut stack = vec![(r"\".to_owned(), root)];
    while let Some((path, folder)) = stack.pop() {
        folders.push(path.clone());
        let code = |e: windows::core::Error| (path.clone(), hr_code(e.code().0));
        // SAFETY: cartella valida; `TASK_ENUM_HIDDEN` include le attività nascoste. Sola lettura.
        match unsafe { folder.GetTasks(TASK_ENUM_HIDDEN.0) } {
            Ok(coll) => {
                // SAFETY: raccolta valida.
                match unsafe { coll.Count() } {
                    Ok(n) => {
                        for i in 1..=n {
                            // SAFETY: indice da 1 a `Count`, come documentato; VARIANT valido per la chiamata.
                            match unsafe { coll.get_Item(&VARIANT::from(i)) } {
                                Ok(t) => match read_task(&t) {
                                    Ok(x) => out.tasks.push(x),
                                    Err(e) => out.unreadable.push(e),
                                },
                                Err(e) => out.unreadable.push(code(e)),
                            }
                        }
                    }
                    Err(e) => out.unreadable.push(code(e)),
                }
            }
            Err(e) => out.unreadable.push(code(e)),
        }
        // SAFETY: cartella valida; il parametro dei flag è riservato e vale 0.
        match unsafe { folder.GetFolders(0) } {
            Ok(coll) => {
                // SAFETY: raccolta valida.
                match unsafe { coll.Count() } {
                    Ok(n) => {
                        for i in 1..=n {
                            // SAFETY: indice da 1 a `Count`, come documentato; VARIANT valido per la chiamata.
                            match unsafe { coll.get_Item(&VARIANT::from(i)) } {
                                Ok(f) => {
                                    // SAFETY: cartella valida; restituisce un BSTR posseduto.
                                    let p = unsafe { f.Path() }.map(|b| b.to_string()).unwrap_or_else(|_| path.clone());
                                    stack.push((p, f));
                                }
                                Err(e) => out.unreadable.push(code(e)),
                            }
                        }
                    }
                    Err(e) => out.unreadable.push(code(e)),
                }
            }
            Err(e) => out.unreadable.push(code(e)),
        }
    }
    Ok(())
}

/// Percorso dell'attività e percorsi delle sue azioni `Exec`.
fn read_task(t: &IRegisteredTask) -> Result<ScheduledTask, (String, u32)> {
    // SAFETY: attività valida; restituisce un BSTR posseduto.
    let path = unsafe { t.Path() }.map(|b| b.to_string()).map_err(|e| (String::new(), hr_code(e.code().0)))?;
    let err = |e: windows::core::Error| (path.clone(), hr_code(e.code().0));
    // SAFETY: lettura della definizione dell'attività e delle sue azioni, senza modificarle.
    let actions = unsafe { t.Definition().and_then(|d| d.Actions()) }.map_err(err)?;
    let mut n = 0i32;
    // SAFETY: `n` riceve il numero di azioni.
    unsafe { actions.Count(&mut n) }.map_err(err)?;
    let mut executables = Vec::new();
    for i in 1..=n {
        // SAFETY: indice da 1 a `Count`, come documentato.
        let a = unsafe { actions.get_Item(i) }.map_err(err)?;
        let mut ty = TASK_ACTION_TYPE::default();
        // SAFETY: `ty` riceve il tipo dell'azione.
        unsafe { a.Type(&mut ty) }.map_err(err)?;
        if ty != TASK_ACTION_EXEC {
            continue;
        }
        let exec: IExecAction = a.cast().map_err(err)?;
        let mut p = BSTR::new();
        // SAFETY: `p` riceve un BSTR posseduto, liberato alla sua uscita di scopo.
        unsafe { exec.Path(&mut p) }.map_err(err)?;
        let p = p.to_string();
        if !p.trim().is_empty() {
            executables.push(p);
        }
    }
    Ok(ScheduledTask { path, executables })
}
