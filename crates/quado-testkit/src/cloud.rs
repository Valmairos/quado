// SPDX-License-Identifier: GPL-3.0-or-later
//! Provider cloud di prova: una cartella di sincronizzazione registrata con la
//! Cloud Filter API, con segnaposto creati dal test. Il provider non scarica nulla: conta soltanto le
//! richieste di dati (`FETCH_DATA`) e di elenco (`FETCH_PLACEHOLDERS`) che riceve. La scansione non
//! deve provocarne nessuna.
//!
//! Alla fine (anche se il test fallisce) si cancellano i segnaposto, si annulla la registrazione e si
//! elimina la cartella: sul sistema non resta nulla.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};

use windows_sys::Win32::Storage::CloudFilters::{
    CF_CALLBACK_INFO, CF_CALLBACK_PARAMETERS, CF_CALLBACK_REGISTRATION, CF_CALLBACK_TYPE_FETCH_DATA, CF_CALLBACK_TYPE_FETCH_PLACEHOLDERS,
    CF_CALLBACK_TYPE_NONE, CF_CONNECT_FLAG_NONE, CF_CONNECTION_KEY, CF_CREATE_FLAG_NONE, CF_FS_METADATA, CF_HARDLINK_POLICY_NONE,
    CF_HYDRATION_POLICY, CF_HYDRATION_POLICY_FULL, CF_HYDRATION_POLICY_MODIFIER_NONE, CF_INSYNC_POLICY_NONE,
    CF_PLACEHOLDER_CREATE_FLAG_DISABLE_ON_DEMAND_POPULATION, CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC, CF_PLACEHOLDER_CREATE_INFO,
    CF_PLACEHOLDER_MANAGEMENT_POLICY_DEFAULT, CF_POPULATION_POLICY, CF_POPULATION_POLICY_FULL, CF_POPULATION_POLICY_MODIFIER_NONE,
    CF_REGISTER_FLAG_NONE, CF_SYNC_POLICIES, CF_SYNC_REGISTRATION, CfConnectSyncRoot, CfCreatePlaceholders, CfDisconnectSyncRoot,
    CfRegisterSyncRoot, CfUnregisterSyncRoot,
};
use windows_sys::Win32::Storage::FileSystem::FILE_BASIC_INFO;
use windows_sys::core::GUID;

pub static FETCH_DATA: AtomicU64 = AtomicU64::new(0);
pub static FETCH_PLACEHOLDERS: AtomicU64 = AtomicU64::new(0);

unsafe extern "system" fn on_fetch_data(_: *const CF_CALLBACK_INFO, _: *const CF_CALLBACK_PARAMETERS) {
    FETCH_DATA.fetch_add(1, SeqCst);
}

unsafe extern "system" fn on_fetch_placeholders(_: *const CF_CALLBACK_INFO, _: *const CF_CALLBACK_PARAMETERS) {
    FETCH_PLACEHOLDERS.fetch_add(1, SeqCst);
}

static CALLBACKS: [CF_CALLBACK_REGISTRATION; 3] = [
    CF_CALLBACK_REGISTRATION { Type: CF_CALLBACK_TYPE_FETCH_DATA, Callback: Some(on_fetch_data) },
    CF_CALLBACK_REGISTRATION { Type: CF_CALLBACK_TYPE_FETCH_PLACEHOLDERS, Callback: Some(on_fetch_placeholders) },
    CF_CALLBACK_REGISTRATION { Type: CF_CALLBACK_TYPE_NONE, Callback: None },
];

const PROVIDER_ID: GUID = GUID { data1: 0x5249_4c49, data2: 0x4556, data3: 0x4f30, data4: [0x91, 0x7e, 0x13, 0x10, 0x20, 0x26, 0x00, 0x01] };

fn wz(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wpath(p: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// Segnaposto da creare.
pub struct Placeholder<'a> {
    pub name: &'a str,
    pub size: i64,
    pub dir: bool,
    /// Per le cartelle: già popolata (niente richiesta di elenco quando la si apre).
    pub populated: bool,
}

pub struct TestSyncRoot {
    pub path: PathBuf,
    key: Option<CF_CONNECTION_KEY>,
    registered: bool,
}

#[derive(Debug)]
pub struct CloudError(pub &'static str, pub i32);

impl TestSyncRoot {
    pub fn new(name: &str) -> Result<Self, CloudError> {
        let path = std::env::temp_dir().join(format!("quado-cloud-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).map_err(|_| CloudError("cartella", 0))?;
        let mut me = Self { path, key: None, registered: false };
        let p = wpath(&me.path);
        let provider = wz("QuadoProvaCloud");
        let version = wz("1.0");
        let identity = b"quado-prova";
        let reg = CF_SYNC_REGISTRATION {
            StructSize: size_of::<CF_SYNC_REGISTRATION>() as u32,
            ProviderName: provider.as_ptr(),
            ProviderVersion: version.as_ptr(),
            SyncRootIdentity: identity.as_ptr().cast(),
            SyncRootIdentityLength: identity.len() as u32,
            FileIdentity: std::ptr::null(),
            FileIdentityLength: 0,
            ProviderId: PROVIDER_ID,
        };
        let policies = CF_SYNC_POLICIES {
            StructSize: size_of::<CF_SYNC_POLICIES>() as u32,
            Hydration: CF_HYDRATION_POLICY { Primary: CF_HYDRATION_POLICY_FULL, Modifier: CF_HYDRATION_POLICY_MODIFIER_NONE },
            Population: CF_POPULATION_POLICY { Primary: CF_POPULATION_POLICY_FULL, Modifier: CF_POPULATION_POLICY_MODIFIER_NONE },
            InSync: CF_INSYNC_POLICY_NONE,
            HardLink: CF_HARDLINK_POLICY_NONE,
            PlaceholderManagement: CF_PLACEHOLDER_MANAGEMENT_POLICY_DEFAULT,
        };
        // SAFETY: stringhe terminate da zero e strutture valide per la durata della chiamata.
        let hr = unsafe { CfRegisterSyncRoot(p.as_ptr(), &reg, &policies, CF_REGISTER_FLAG_NONE) };
        if hr < 0 {
            return Err(CloudError("CfRegisterSyncRoot", hr));
        }
        me.registered = true;
        let mut key: CF_CONNECTION_KEY = 0;
        // SAFETY: la tabella dei callback è statica e termina con CF_CALLBACK_TYPE_NONE; i callback
        // contano soltanto e non usano i parametri.
        let hr = unsafe { CfConnectSyncRoot(p.as_ptr(), CALLBACKS.as_ptr(), std::ptr::null(), CF_CONNECT_FLAG_NONE, &mut key) };
        if hr < 0 {
            return Err(CloudError("CfConnectSyncRoot", hr));
        }
        me.key = Some(key);
        Ok(me)
    }

    /// Crea segnaposto in `rel_dir` (relativa alla radice, `""` per la radice).
    pub fn create(&self, rel_dir: &str, items: &[Placeholder<'_>]) -> Result<(), CloudError> {
        let base = wpath(&self.path.join(rel_dir));
        let names: Vec<Vec<u16>> = items.iter().map(|i| wz(i.name)).collect();
        let mut infos: Vec<CF_PLACEHOLDER_CREATE_INFO> = items
            .iter()
            .zip(&names)
            .map(|(i, n)| {
                // SAFETY: FILE_BASIC_INFO è una struttura di soli numeri: lo zero è valido.
                let mut basic: FILE_BASIC_INFO = unsafe { std::mem::zeroed() };
                basic.FileAttributes = if i.dir { 0x10 } else { 0x80 };
                let mut flags = CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC;
                if i.dir && i.populated {
                    flags |= CF_PLACEHOLDER_CREATE_FLAG_DISABLE_ON_DEMAND_POPULATION;
                }
                CF_PLACEHOLDER_CREATE_INFO {
                    RelativeFileName: n.as_ptr(),
                    FsMetadata: CF_FS_METADATA { BasicInfo: basic, FileSize: if i.dir { 0 } else { i.size } },
                    FileIdentity: n.as_ptr().cast(),
                    FileIdentityLength: (n.len() * 2) as u32,
                    Flags: flags,
                    Result: 0,
                    CreateUsn: 0,
                }
            })
            .collect();
        let mut done = 0u32;
        // SAFETY: `base` e i nomi sono terminati da zero e vivono fino alla fine della chiamata;
        // l'array ha `infos.len()` elementi.
        let hr = unsafe { CfCreatePlaceholders(base.as_ptr(), infos.as_mut_ptr(), infos.len() as u32, CF_CREATE_FLAG_NONE, &mut done) };
        if hr < 0 || done as usize != items.len() {
            return Err(CloudError("CfCreatePlaceholders", hr));
        }
        Ok(())
    }

    pub fn requests() -> (u64, u64) {
        (FETCH_DATA.load(SeqCst), FETCH_PLACEHOLDERS.load(SeqCst))
    }
}

impl Drop for TestSyncRoot {
    fn drop(&mut self) {
        let p = wpath(&self.path);
        // I segnaposto si cancellano mentre il provider è ancora collegato.
        if let Ok(rd) = std::fs::read_dir(&self.path) {
            for e in rd.flatten() {
                let _ = if e.path().is_dir() { std::fs::remove_dir_all(e.path()) } else { std::fs::remove_file(e.path()) };
            }
        }
        if let Some(key) = self.key.take() {
            // SAFETY: chiave restituita da CfConnectSyncRoot, usata una volta.
            unsafe { CfDisconnectSyncRoot(key) };
        }
        if self.registered {
            // SAFETY: percorso terminato da zero della cartella registrata da questo valore.
            unsafe { CfUnregisterSyncRoot(p.as_ptr()) };
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
