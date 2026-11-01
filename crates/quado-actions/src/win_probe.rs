// SPDX-License-Identifier: GPL-3.0-or-later
//! Letture vere di Windows per simulazione, esecuzione e misure (05 D7), in sola lettura: attributi con
//! i soli permessi di lettura degli attributi (`quado_scan_win`), elenco delle cartelle, Restart
//! Manager (`restart`), Cestino del volume (`GetVolumePathNameW`, `GetDriveTypeW`, `SHQueryRecycleBinW`
//! e la chiave `BitBucket` da [`SystemConfig`]), cartelle di sincronizzazione (`CfGetSyncRootInfoByPath`),
//! spazio libero.
//!
//! Niente qui apre il contenuto di un file o scarica un segnaposto cloud: gli handle sono quelli della
//! scansione (`FILE_READ_ATTRIBUTES`, `FILE_FLAG_OPEN_REPARSE_POINT`, `FILE_FLAG_OPEN_NO_RECALL`).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use quado_model::intervention::{BinCount, ReferenceReport};
use quado_scan_win::{attrs, system, win};
use windows_sys::Win32::Foundation::{ERROR_INVALID_NAME, GetLastError};
use windows_sys::Win32::Storage::CloudFilters::{CF_SYNC_ROOT_INFO_PROVIDER, CF_SYNC_ROOT_PROVIDER_INFO, CfGetSyncRootInfoByPath};
use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceW, GetDriveTypeW, GetVolumePathNameW};

use crate::probe::{Attrs, Entry, ListError, Probe, SystemConfig, VolumeBin};
use crate::restart::{self, ProcessUse, RmError};

/// `DRIVE_FIXED` di `GetDriveTypeW`: disco locale, anche esterno USB o VHDX montato. Rimovibili, rete,
/// CD e disco RAM non hanno un Cestino.
const DRIVE_FIXED: u32 = 3;

/// Le letture vere. Registro, collegamenti e attività pianificate arrivano da `config`.
pub struct WinProbe {
    config: Box<dyn SystemConfig>,
    /// Dimensione del cluster per numero di serie del volume, letta una volta per volume.
    clusters: Mutex<HashMap<u64, Option<u64>>>,
}

impl WinProbe {
    pub fn new(config: Box<dyn SystemConfig>) -> Self {
        // I segnaposto cloud si mostrano come tali (con i loro attributi e il tag), come nella scansione:
        // senza, Windows li presenta come file normali ad alcuni processi. Vale per tutto il processo.
        win::expose_placeholders();
        Self { config, clusters: Mutex::new(HashMap::new()) }
    }

    /// Dimensione del cluster del volume che contiene `path` (`GetDiskFreeSpaceW`), con la cache per
    /// numero di serie del volume. `None` se non si legge.
    fn cluster(&self, path: &str, serial: u64) -> Option<u64> {
        if let Some(c) = self.clusters.lock().ok().and_then(|m| m.get(&serial).copied()) {
            return c;
        }
        let c = volume_root(path).ok().and_then(|root| cluster_size(&root));
        if let Ok(mut m) = self.clusters.lock() {
            m.insert(serial, c);
        }
        c
    }
}

/// Byte per cluster del volume con radice `root`.
fn cluster_size(root: &str) -> Option<u64> {
    let z = wide_z(root);
    let (mut spc, mut bps, mut free, mut total) = (0u32, 0u32, 0u32, 0u32);
    // SAFETY: `z` è terminata da zero e vive per la chiamata; i quattro puntatori sono variabili locali.
    let ok = unsafe { GetDiskFreeSpaceW(z.as_ptr(), &mut spc, &mut bps, &mut free, &mut total) };
    let c = u64::from(spc) * u64::from(bps);
    (ok != 0 && c > 0).then_some(c)
}

/// Dati residenti nella MFT (03 §6.3): su NTFS l'allocata di un file non residente è un multiplo del
/// cluster (anche se compresso o sparse), quella di un file residente no (16 byte per un file da 10,
/// misurato su R: il 14/10/2026). Le cartelle e i file senza allocata non sono residenti per la stima.
fn is_resident(alloc: u64, directory: bool, cluster: Option<u64>) -> bool {
    !directory && alloc > 0 && cluster.is_some_and(|c| !alloc.is_multiple_of(c))
}

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn last_error() -> u32 {
    // SAFETY: nessun prerequisito.
    unsafe { GetLastError() }
}

/// `GetVolumePathNameW` su un solo candidato: la radice con la barra finale.
fn volume_path_name(candidate: &str) -> Result<String, u32> {
    let z = wide_z(candidate);
    let mut buf = vec![0u16; 32_768];
    // SAFETY: `z` è terminata da zero e vive per la chiamata; `buf` è un buffer locale della lunghezza
    // dichiarata. La funzione cerca i punti di montaggio lungo il percorso e non legge contenuti (anche
    // su un segnaposto cloud nessuna richiesta al provider, `tests/probe_win.rs`).
    let ok = unsafe { GetVolumePathNameW(z.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    if ok == 0 {
        return Err(last_error());
    }
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Ok(String::from_utf16_lossy(&buf[..n]))
}

/// Radice del volume che contiene `path` (`C:\`, oppure la cartella di un punto di montaggio), senza
/// `\\?\`. Prima il percorso così com'è; se Windows lo rifiuta (per esempio oltre i 260 caratteri) si
/// riprova con `\\?\`.
fn volume_root(path: &str) -> Result<String, u32> {
    if path.contains('\0') {
        return Err(ERROR_INVALID_NAME);
    }
    let plain = path.strip_prefix(r"\\?\").unwrap_or(path);
    let root = match volume_path_name(plain) {
        Ok(r) => r,
        Err(e) => volume_path_name(&format!(r"\\?\{plain}")).map_err(|_| e)?,
    };
    Ok(root.strip_prefix(r"\\?\").map(str::to_owned).unwrap_or(root))
}

/// Thread della simulazione (`Probe::threads`): lettura delle cartelle e uso dei file. Le aperture dei
/// soli attributi delle oltre centomila voci delle versioni di CapCut vanno da 18-22 s su un thread a 3,8-5,0 s su
/// 8 (`cargo xtask attributi-fasi --aperture`, 26/10/2026); l'uso dei file degli stessi elementi (Restart
/// Manager) 7,6-11,1 s con 8 thread contro 41-77 s con uno.
pub const THREADS: usize = 8;

impl Probe for WinProbe {
    fn attributes(&self, path: &str) -> Result<Attrs, u32> {
        let a = attrs::read(path)?;
        let resident = !a.directory && a.alloc > 0 && is_resident(a.alloc, a.directory, self.cluster(path, a.volume_serial));
        Ok(Attrs {
            logical: a.logical,
            alloc: a.alloc,
            links: a.links,
            file_id: a.file_id,
            volume_serial: a.volume_serial,
            modified: a.modified,
            attributes: a.attributes,
            directory: a.directory,
            link: a.link(),
            placeholder: a.placeholder(),
            resident,
        })
    }

    /// Come `children` e poi `attributes` di ogni voce, con una sola apertura della cartella invece di tre
    /// (`attrs::list_dir`): la simulazione chiede l'elenco solo di cartelle di cui ha appena letto gli
    /// attributi. Le voci si leggono una per una con `attributes`, quindi gli attributi sono gli stessi:
    /// l'elenco non dà il numero di nomi, e aprire le voci in un altro modo non costa meno
    /// (`quado_scan_win::attrs`).
    fn entries(&self, dir: &str, step: &dyn Fn() -> bool) -> Result<Vec<Entry>, ListError> {
        let names = attrs::list_dir(dir).map_err(ListError::Code)?;
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            if !step() {
                return Err(ListError::Stopped);
            }
            let a = self.attributes(&crate::simulate::join(dir, &name));
            out.push((name, a));
        }
        Ok(out)
    }

    fn threads(&self) -> usize {
        THREADS
    }

    /// Un collegamento (junction, symlink, punto di montaggio) non si attraversa (05 D4): non ha figli
    /// suoi, e spostarlo nel Cestino sposta solo il collegamento. Un file: `Err(267)`.
    ///
    /// Una cartella cloud non ancora popolata (`RECALL_ON_DATA_ACCESS`, anche la radice di una sync root)
    /// si elenca con l'handle della scansione (`FILE_FLAG_OPEN_NO_RECALL`): Windows restituisce i soli
    /// figli presenti sul disco e il provider non riceve richieste di elenco né di dati (misurato il
    /// 14/10/2026 con il provider di prova, `tests/probe_win.rs`). Sono proprio i figli che contano per i
    /// byte locali (03 §6.3).
    fn children(&self, dir: &str) -> Result<Vec<String>, u32> {
        if attrs::read(dir)?.link() {
            return Ok(Vec::new());
        }
        attrs::list(dir)
    }

    fn processes_using(&self, paths: &[String]) -> Result<Vec<ProcessUse>, RmError> {
        restart::processes_using(paths)
    }

    fn references(&self, paths: &[String]) -> ReferenceReport {
        self.config.references(paths)
    }

    fn volume_bin(&self, path: &str) -> Result<VolumeBin, u32> {
        let root = volume_root(path)?;
        let z = wide_z(&root);
        // SAFETY: `z` è terminata da zero e vive per la chiamata; la funzione legge solo il tipo dell'unità.
        let has_bin = unsafe { GetDriveTypeW(z.as_ptr()) } == DRIVE_FIXED;
        if !has_bin {
            return Ok(VolumeBin { root, has_bin, ..Default::default() });
        }
        // Un errore nella lettura delle impostazioni lascia la capacità sconosciuta: la simulazione blocca
        // lo spostamento, e il resto (contenuto del Cestino) si legge lo stesso.
        let (capacity, nuke_on_delete) = self.config.bin_settings(&root).unwrap_or((None, false));
        let content = crate::recycle::query(Path::new(&root))
            .ok()
            .map(|(items, bytes)| BinCount { items: items.max(0) as u64, bytes: bytes.max(0) as u64 });
        Ok(VolumeBin { root, has_bin, nuke_on_delete, capacity, content })
    }

    fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
        self.config.storage_sense_days()
    }

    /// Chiede a Windows le informazioni della sync root registrata (nome del provider). Non apre il
    /// contenuto e non parla con il provider: zero richieste di dati, provato nella cartella di prova
    /// (`tests/probe_win.rs`). Qualunque errore, compreso "non sotto una sync root" (390), dà `None`.
    fn sync_root(&self, path: &str) -> Option<String> {
        if path.contains('\0') {
            return None;
        }
        let w: Vec<u16> = path.encode_utf16().collect();
        let z = win::wide_z(&win::long_path(&w));
        // SAFETY: la struttura è di soli numeri e array di u16: lo zero è valido.
        let mut info: CF_SYNC_ROOT_PROVIDER_INFO = unsafe { std::mem::zeroed() };
        let mut returned = 0u32;
        // SAFETY: `z` è terminata da zero e vive per la chiamata; il buffer d'uscita è `info`, una
        // variabile locale grande quanto dichiarato; `returned` è locale.
        let hr = unsafe {
            CfGetSyncRootInfoByPath(
                z.as_ptr(),
                CF_SYNC_ROOT_INFO_PROVIDER,
                (&raw mut info).cast(),
                size_of::<CF_SYNC_ROOT_PROVIDER_INFO>() as u32,
                &mut returned,
            )
        };
        if hr < 0 {
            return None;
        }
        let n = info.ProviderName.iter().position(|&c| c == 0).unwrap_or(info.ProviderName.len());
        Some(String::from_utf16_lossy(&info.ProviderName[..n]))
    }

    /// Cartelle sincronizzate senza l'interfaccia cloud di Windows (`crate::sync`): le cartelle dichiarate
    /// dalla configurazione (`SystemConfig::sync_roots`) e i marcatori, cercati con i soli attributi
    /// (`attrs::read`, nessuna apertura del contenuto). Si chiede solo fuori da una sync root registrata, quindi
    /// nessun provider cloud riceve richieste.
    fn sync_marker(&self, path: &str) -> Option<crate::sync::SyncFolder> {
        if path.contains('\0') {
            return None;
        }
        let roots = self.config.sync_roots();
        crate::sync::find(path, &roots, &|p: &str| attrs::read(p).is_ok())
    }

    /// Byte liberi del volume (`TotalNumberOfFreeBytes`), non la quota del chiamante: la variazione del
    /// volume (F-VE-3) si misura sul volume intero.
    fn free_space(&self, path: &str) -> Result<u64, u32> {
        let root = volume_root(path)?;
        let w: Vec<u16> = root.encode_utf16().collect();
        win::disk_space(&w).map(|s| s.free)
    }

    fn now(&self) -> i64 {
        system::now_filetime()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots() {
        assert_eq!(volume_root(r"C:\Windows\System32").as_deref(), Ok(r"C:\"));
        assert_eq!(volume_root(r"\\?\C:\Windows").as_deref(), Ok(r"C:\"));
        assert_eq!(volume_root("C:\\Windows\0"), Err(ERROR_INVALID_NAME));
    }

    #[test]
    fn resident_only_off_cluster_allocations() {
        assert!(is_resident(16, false, Some(4096)));
        assert!(!is_resident(4096, false, Some(4096)));
        assert!(!is_resident(249_856, false, Some(4096)));
        assert!(!is_resident(0, false, Some(4096)));
        assert!(!is_resident(16, true, Some(4096)));
        assert!(!is_resident(16, false, None));
        assert!(cluster_size(r"C:\").is_some_and(|c| c >= 512));
    }
}
