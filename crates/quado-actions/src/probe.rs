// SPDX-License-Identifier: GPL-3.0-or-later
//! Letture che servono a simulazione, esecuzione e misure (05 D7): attributi senza aprire il contenuto,
//! Restart Manager, riferimenti, Cestino del volume, Sensore memoria, cartelle di sincronizzazione,
//! spazio libero. Tutte in sola lettura.
//!
//! Le letture passano da [`Probe`], così la logica di simulazione, conferma ed esecuzione si prova con
//! dati simulati (`fake`) e la difesa vera si prova su VHDX con [`crate::win_probe::WinProbe`] (06,
//! principi). Registro, collegamenti e attività pianificate li legge `quado-kb`: l'interfaccia li passa
//! a `WinProbe` con [`SystemConfig`] (05 D10: `quado-actions` non dipende da `quado-kb`).

use quado_model::intervention::{BinCount, ReferenceReport};

use crate::restart::{ProcessUse, RmError};

/// Attributi di un file o di una cartella, letti con `FILE_READ_ATTRIBUTES`,
/// `FILE_FLAG_OPEN_REPARSE_POINT` e `FILE_FLAG_OPEN_NO_RECALL`: il contenuto non si apre, i segnaposto
/// non si scaricano, la data di ultimo accesso non cambia (03 §6.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Attrs {
    /// Dimensione logica (`FileStandardInfo.EndOfFile`).
    pub logical: u64,
    /// Dimensione su disco (`FileStandardInfo.AllocationSize`): per i compressi, WOF e sparse l'allocata;
    /// per un segnaposto cloud i soli byte locali; per un file residente nella MFT qualche byte, che la
    /// stima non conta (`resident`).
    pub alloc: u64,
    /// Numero di nomi del file (`FileStandardInfo.NumberOfLinks`).
    pub links: u32,
    /// Identità a 128 bit (`FileIdInfo`); 0 se il file system non la dà (exFAT, FAT32).
    pub file_id: u128,
    pub volume_serial: u64,
    /// Data di modifica (`FILETIME`).
    pub modified: i64,
    /// `FILE_ATTRIBUTE_*`.
    pub attributes: u32,
    pub directory: bool,
    /// Collegamento (symlink, junction, punto di montaggio): conta da solo, non si attraversa.
    pub link: bool,
    /// Segnaposto cloud (offline, richiamo all'apertura o all'accesso ai dati, tag cloud).
    pub placeholder: bool,
    /// Dati residenti nella MFT: nella stima valgono 0, la MFT non si restringe (03 §6.3). Windows
    /// riporta comunque un'allocata piccola (16 byte per un file da 10).
    pub resident: bool,
}

/// Il Cestino del volume che contiene un percorso, letto ora.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VolumeBin {
    /// Radice del volume (`C:\`), come la restituisce `GetVolumePathNameW`.
    pub root: String,
    /// Il volume ha un Cestino: disco fisso locale. Unità rimovibili e di rete no (F-AC-1).
    pub has_bin: bool,
    /// `NukeOnDelete` = 1: Windows elimina senza passare dal Cestino (F-AC-1, F-AC-7).
    pub nuke_on_delete: bool,
    /// `MaxCapacity` del volume in byte (chiave `BitBucket`); `None` se non registrata.
    pub capacity: Option<u64>,
    /// Contenuto secondo Windows (`SHQueryRecycleBinW`); `None` se non letto.
    pub content: Option<BinCount>,
}

/// Una voce di una cartella letta da [`Probe::entries`]: nome e attributi, o il codice dell'errore della
/// loro lettura.
pub type Entry = (String, Result<Attrs, u32>);

/// Perché [`Probe::entries`] non ha dato le voci.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ListError {
    /// L'elenco non si legge: codice di Windows (5 per una cartella negata).
    Code(u32),
    /// `step` ha chiesto di fermarsi (annullamento).
    Stopped,
}

/// [`Probe::entries`] fatto con le letture una per una: [`Probe::children`], poi [`Probe::attributes`]
/// di ogni voce nell'ordine dell'elenco, con `step` prima di ciascuna.
pub fn entries_one_by_one<P: Probe + ?Sized>(probe: &P, dir: &str, step: &dyn Fn() -> bool) -> Result<Vec<Entry>, ListError> {
    let names = probe.children(dir).map_err(ListError::Code)?;
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        if !step() {
            return Err(ListError::Stopped);
        }
        let a = probe.attributes(&crate::simulate::join(dir, &name));
        out.push((name, a));
    }
    Ok(out)
}

/// Le letture di simulazione, esecuzione e misure. Ogni metodo restituisce codici di errore di Windows,
/// mai frasi (05 D13). Le implementazioni non scrivono niente.
pub trait Probe: Send + Sync {
    /// Attributi di `path` (file o cartella), senza seguire i collegamenti.
    fn attributes(&self, path: &str) -> Result<Attrs, u32>;
    /// Nomi dei figli diretti di una cartella (senza `.` e `..`). `Err(5)` per una cartella negata.
    fn children(&self, dir: &str) -> Result<Vec<String>, u32>;
    /// Voci di una cartella con i loro attributi: lo stesso risultato di [`Probe::children`] e poi
    /// [`Probe::attributes`] di ogni voce ([`entries_one_by_one`], il comportamento predefinito), che
    /// un'implementazione può ottenere in un altro modo. `step` si chiama prima di leggere ogni voce: se dà
    /// falso, la lettura si ferma con [`ListError::Stopped`].
    fn entries(&self, dir: &str, step: &dyn Fn() -> bool) -> Result<Vec<Entry>, ListError> {
        entries_one_by_one(self, dir, step)
    }
    /// Thread con cui la simulazione legge le cartelle degli elementi e chiede l'uso dei file. Con 1 (il
    /// predefinito) le letture vanno una dopo l'altra, nell'ordine fisso che le prove con le letture
    /// simulate controllano.
    fn threads(&self) -> usize {
        1
    }
    /// Processi che usano almeno uno dei file, secondo Restart Manager (F-AC-8).
    fn processes_using(&self, paths: &[String]) -> Result<Vec<ProcessUse>, RmError>;
    /// Riferimenti a uno dei percorsi o a qualcosa al loro interno (F-AC-9).
    fn references(&self, paths: &[String]) -> ReferenceReport;
    /// Il Cestino del volume che contiene `path` (F-AC-1, F-AC-7).
    fn volume_bin(&self, path: &str) -> Result<VolumeBin, u32>;
    /// Giorni dopo cui Sensore memoria svuota il Cestino; `None` se Sensore memoria è spento o non
    /// svuota il Cestino (F-AC-7).
    fn storage_sense_days(&self) -> Result<Option<u32>, u32>;
    /// Provider della cartella di sincronizzazione registrata che contiene `path`, se c'è (F-AC-2).
    fn sync_root(&self, path: &str) -> Option<String>;
    /// Cartella sincronizzata che Windows non conosce (Dropbox, Google Drive, Syncthing, Resilio Sync), da
    /// chiedere solo se [`Probe::sync_root`] non ne trova una: `crate::sync`. Nessuna per default.
    fn sync_marker(&self, _path: &str) -> Option<crate::sync::SyncFolder> {
        None
    }
    /// Byte liberi del volume che contiene `path` (`GetDiskFreeSpaceExW`, F-VE-1).
    fn free_space(&self, path: &str) -> Result<u64, u32>;
    /// Ora corrente (`FILETIME`): le misure e il registro la usano, i test la fissano.
    fn now(&self) -> i64;
}

/// Letture che fa `quado-kb` e che l'interfaccia passa a [`crate::win_probe::WinProbe`]: impostazioni
/// del Cestino e di Sensore memoria dal registro in sola lettura, riferimenti (registro, `.lnk` letti
/// senza `Resolve`, attività pianificate). Lette al momento della simulazione, non dalla scansione.
pub trait SystemConfig: Send + Sync {
    /// `MaxCapacity` in byte (`None` se non registrata) e `NukeOnDelete` del volume con radice `root`.
    fn bin_settings(&self, root: &str) -> Result<(Option<u64>, bool), u32>;
    /// Come [`Probe::storage_sense_days`].
    fn storage_sense_days(&self) -> Result<Option<u32>, u32>;
    /// Come [`Probe::references`].
    fn references(&self, paths: &[String]) -> ReferenceReport;
    /// Cartelle sincronizzate dichiarate dalla configurazione dei programmi (Dropbox, Google Drive),
    /// lette al momento della simulazione. Nessuna per default.
    fn sync_roots(&self) -> Vec<crate::sync::SyncRoot> {
        Vec::new()
    }
}
