// SPDX-License-Identifier: GPL-3.0-or-later
//! Letture e spostamenti simulati, in memoria, per i test della logica e dell'interfaccia: la
//! logica si prova con dati simulati. Non tocca il disco.
//!
//! [`FakeProbe`] implementa `probe::Probe` su un albero in memoria; i suoi cloni condividono lo stato,
//! così [`FakeMover`] (implementa `execute::Mover`) toglie dalle letture ciò che sposta e lo aggiunge al
//! Cestino del volume. I metodi di costruzione e di modifica prendono `&self` e restituiscono un clone,
//! per scriverli in catena: `FakeProbe::new().volume("R:\\").file("R:\\a.bin", 10, 4096)`. Valgono
//! anche a runtime, tra simulazione ed esecuzione.
//!
//! Il Cestino simulato si comporta come quello misurato su un VHDX di prova
//! (`docs/m1/cestino-verifiche.md`): conta la dimensione logica; uno spostamento su un volume senza Cestino,
//! con `NukeOnDelete` o di un elemento più grande della capacità elimina per sempre senza arrivare nel
//! Cestino (`NotRecycled(0)`); lo spazio libero cresce solo quando il Cestino si svuota
//! ([`FakeProbe::empty_bin`]).
//!
//! I percorsi si confrontano normalizzati (senza maiuscole, `/` come `\`, senza `\` finale), come nella
//! simulazione.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use quado_model::intervention::{BinCount, Reference, ReferenceKind, ReferenceReport};
use quado_model::record::{
    Action, Check, CheckKind, CheckOutcome, Criterion, Date, Estimate, Expiry, Fact, NotEstimable, Origin, Privileges,
    ProvaDiGestibilita, ProvaDiIdentita, Provenance, Record, RuleRef, Source, State,
};

use crate::execute::{MoveResult, Mover};
use crate::probe::{Attrs, Probe, VolumeBin};
use crate::restart::{ProcessUse, RmError};
use crate::simulate::{is_within, norm, parent};

/// Ora iniziale delle letture simulate: 14/10/2026 00:00 UTC (`FILETIME`).
pub const NOW: i64 = 134_364_096_000_000_000;
/// Capacità predefinita del Cestino di un volume simulato: 16 GiB.
pub const CAPACITY: u64 = 16 << 30;
/// Spazio libero predefinito di un volume simulato: 64 GiB.
pub const FREE: u64 = 64 << 30;
/// HRESULT di un elemento che non esiste (`HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND)`).
pub const HR_NOT_FOUND: i32 = 0x8007_0002_u32 as i32;

const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
/// `ERROR_DIRECTORY`: il percorso non è una cartella.
const ERROR_DIRECTORY: u32 = 267;

#[derive(Clone, Debug)]
struct Entry {
    /// Percorso come è stato scritto (maiuscole comprese), per i nomi restituiti da `children`.
    path: String,
    attrs: Attrs,
}

#[derive(Clone, Debug)]
struct Volume {
    /// Radice normalizzata.
    key: String,
    bin: VolumeBin,
    serial: u64,
    free: u64,
    error: Option<u32>,
    /// Voci spostate nel Cestino dal mover, e i byte logici aggiunti al contenuto.
    moved: Vec<Attrs>,
    moved_logical: u64,
}

#[derive(Debug)]
struct World {
    /// Chiave: percorso normalizzato.
    entries: BTreeMap<String, Entry>,
    volumes: Vec<Volume>,
    children_errors: HashMap<String, u32>,
    attr_errors: HashMap<String, u32>,
    in_use: HashMap<String, Vec<ProcessUse>>,
    rm_errors: HashMap<String, RmError>,
    references: Vec<Reference>,
    partial: Vec<(ReferenceKind, u32)>,
    storage_sense: Result<Option<u32>, u32>,
    /// Cartelle di sincronizzazione: prefisso normalizzato e provider.
    synced: Vec<(String, String)>,
    /// Cartelle sincronizzate che Windows non conosce: prefisso normalizzato e cartella (`crate::sync`).
    markers: Vec<(String, crate::sync::SyncFolder)>,
    now: i64,
    next_id: u128,
    reads: Vec<String>,
    rm_calls: Vec<Vec<String>>,
    reference_calls: Vec<Vec<String>>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            volumes: Vec::new(),
            children_errors: HashMap::new(),
            attr_errors: HashMap::new(),
            in_use: HashMap::new(),
            rm_errors: HashMap::new(),
            references: Vec::new(),
            partial: Vec::new(),
            storage_sense: Ok(None),
            synced: Vec::new(),
            markers: Vec::new(),
            now: NOW,
            next_id: 0x1000,
            reads: Vec::new(),
            rm_calls: Vec::new(),
            reference_calls: Vec::new(),
        }
    }
}

/// Percorso scritto in un test, ripulito senza cambiare le maiuscole: `\` al posto di `/`, senza `\`
/// finale salvo la radice.
fn clean(path: &str) -> String {
    let mut p = path.replace('/', "\\");
    while p.len() > 3 && p.ends_with('\\') {
        p.pop();
    }
    p
}

/// Byte che un file libera quando sparisce: la MFT non si restringe.
fn freed_by(a: &Attrs) -> u64 {
    if a.resident { 0 } else { a.alloc }
}

/// Ultimo componente di un percorso.
fn name_of(path: &str) -> String {
    path.rsplit('\\').next().unwrap_or(path).to_owned()
}

impl World {
    /// Volume con la radice più lunga che contiene `key` (normalizzato).
    fn volume_of(&self, key: &str) -> Option<usize> {
        (0..self.volumes.len()).filter(|&i| is_within(key, &self.volumes[i].key)).max_by_key(|&i| self.volumes[i].key.len())
    }

    fn serial_of(&self, key: &str) -> u64 {
        self.volume_of(key).map_or(0, |i| self.volumes[i].serial)
    }

    fn next_id(&mut self) -> u128 {
        self.next_id += 1;
        self.next_id
    }

    fn dir_attrs(&mut self, key: &str) -> Attrs {
        Attrs {
            links: 1,
            file_id: self.next_id(),
            volume_serial: self.serial_of(key),
            modified: self.now,
            attributes: FILE_ATTRIBUTE_DIRECTORY,
            directory: true,
            ..Attrs::default()
        }
    }

    fn file_attrs(&mut self, key: &str, logical: u64, alloc: u64) -> Attrs {
        Attrs {
            logical,
            alloc,
            links: 1,
            file_id: self.next_id(),
            volume_serial: self.serial_of(key),
            modified: self.now,
            attributes: FILE_ATTRIBUTE_ARCHIVE,
            ..Attrs::default()
        }
    }

    /// Crea la cartella `path` e le cartelle sopra, se mancano.
    fn ensure_dir(&mut self, path: &str) {
        let path = clean(path);
        let key = norm(&path);
        if self.entries.contains_key(&key) {
            return;
        }
        if let Some(p) = parent(&path) {
            self.ensure_dir(p);
        }
        let attrs = self.dir_attrs(&key);
        self.entries.insert(key, Entry { path, attrs });
    }

    /// Inserisce (o sostituisce) una voce, con le cartelle sopra.
    fn insert(&mut self, path: &str, attrs: Attrs) {
        let path = clean(path);
        if let Some(p) = parent(&path) {
            self.ensure_dir(p);
        }
        self.entries.insert(norm(&path), Entry { path, attrs });
    }

    fn entry_mut(&mut self, path: &str) -> &mut Entry {
        self.entries.get_mut(&norm(path)).unwrap_or_else(|| panic!("FakeProbe: {path} non esiste"))
    }

    /// Voci strettamente sotto `key` (normalizzato), con la parte del percorso dopo `key`.
    fn below<'a>(&'a self, key: &str) -> impl Iterator<Item = (&'a str, &'a Entry)> + 'a {
        let prefix = if key.ends_with('\\') { key.to_owned() } else { format!("{key}\\") };
        let n = prefix.len();
        // Per la radice di un'unità (`c:\`) la chiave stessa è il prefisso: non è sotto di sé.
        self.entries.range(prefix.clone()..).take_while(move |(k, _)| k.starts_with(&prefix)).filter(move |(k, _)| k.len() > n).map(move |(k, e)| (&k[n..], e))
    }

    /// Chiavi di `key` e di tutto ciò che sta sotto.
    fn subtree(&self, key: &str) -> Vec<String> {
        let mut keys: Vec<String> = self.below(key).map(|(_, e)| norm(&e.path)).collect();
        if self.entries.contains_key(key) {
            keys.push(key.to_owned());
        }
        keys
    }

    /// Toglie `key` e il suo contenuto dalle letture; restituisce gli attributi tolti.
    fn take_subtree(&mut self, key: &str) -> Vec<Attrs> {
        self.subtree(key).into_iter().filter_map(|k| self.entries.remove(&k)).map(|e| e.attrs).collect()
    }

    /// Byte liberati eliminando per sempre `removed` (già tolti dalle letture): le allocate dei file, una
    /// volta per identità; un file con altri nomi ancora presenti non libera niente e perde quei nomi; un
    /// file residente nella MFT non libera niente.
    fn release(&mut self, removed: &[Attrs]) -> u64 {
        let mut names: HashMap<(u64, u128), u32> = HashMap::new();
        let mut total = 0;
        for a in removed.iter().filter(|a| !a.directory) {
            if a.links > 1 && a.file_id != 0 {
                *names.entry((a.volume_serial, a.file_id)).or_default() += 1;
            } else {
                total += freed_by(a);
            }
        }
        let mut seen = HashSet::new();
        for a in removed.iter().filter(|a| !a.directory && a.links > 1 && a.file_id != 0) {
            let key = (a.volume_serial, a.file_id);
            if !seen.insert(key) {
                continue;
            }
            let gone = names[&key];
            let mut remaining = false;
            for e in self.entries.values_mut().filter(|e| (e.attrs.volume_serial, e.attrs.file_id) == key) {
                e.attrs.links = e.attrs.links.saturating_sub(gone).max(1);
                remaining = true;
            }
            if !remaining {
                total += freed_by(a);
            }
        }
        total
    }

    /// Elimina per sempre `path`: lo spazio si libera subito.
    fn destroy(&mut self, path: &str) {
        let key = norm(path);
        let removed = self.take_subtree(&key);
        let freed = self.release(&removed);
        if let Some(i) = self.volume_of(&key) {
            self.volumes[i].free += freed;
        }
    }

    /// Spostamento nel Cestino come lo fa Windows (vedi la documentazione del modulo).
    fn recycle(&mut self, path: &str) -> MoveResult {
        let key = norm(path);
        if !self.entries.contains_key(&key) {
            return MoveResult::Failed(HR_NOT_FOUND);
        }
        let Some(vi) = self.volume_of(&key) else {
            return MoveResult::Failed(HR_NOT_FOUND);
        };
        let removed = self.take_subtree(&key);
        let logical: u64 = removed.iter().filter(|a| !a.directory).map(|a| a.logical).sum();
        let bin = &self.volumes[vi].bin;
        if !bin.has_bin || bin.nuke_on_delete || bin.capacity.is_some_and(|c| logical > c) {
            let freed = self.release(&removed);
            self.volumes[vi].free += freed;
            return MoveResult::NotRecycled(0);
        }
        let v = &mut self.volumes[vi];
        v.moved.extend(removed);
        v.moved_logical += logical;
        if let Some(c) = &mut v.bin.content {
            c.items += 1;
            c.bytes += logical;
        }
        MoveResult::Recycled
    }
}

/// Letture simulate (vedi la documentazione del modulo). Clonabile: i cloni condividono lo stato.
#[derive(Clone, Debug, Default)]
pub struct FakeProbe {
    world: Arc<Mutex<World>>,
}

impl FakeProbe {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, World> {
        self.world.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn with(&self, f: impl FnOnce(&mut World)) -> Self {
        f(&mut self.lock());
        self.clone()
    }

    // -----------------------------------------------------------------------------------------
    // Volumi e Cestino

    /// Volume con radice `root` (`R:\`): Cestino presente, `NukeOnDelete` spento, capacità [`CAPACITY`],
    /// Cestino vuoto, spazio libero [`FREE`]. La radice è una cartella.
    pub fn volume(&self, root: &str) -> Self {
        self.with(|w| {
            let serial = 0x5000 + w.volumes.len() as u64;
            let key = norm(root);
            let bin = VolumeBin { root: root.to_owned(), has_bin: true, nuke_on_delete: false, capacity: Some(CAPACITY), content: Some(BinCount::default()) };
            w.volumes.push(Volume { key, bin, serial, free: FREE, error: None, moved: Vec::new(), moved_logical: 0 });
            w.ensure_dir(root);
        })
    }

    fn volume_mut<'a>(w: &'a mut World, root: &str) -> &'a mut Volume {
        let key = norm(root);
        w.volumes.iter_mut().find(|v| v.key == key).unwrap_or_else(|| panic!("FakeProbe: volume {root} non dichiarato"))
    }

    /// Cambia il Cestino del volume `root` (presenza, `NukeOnDelete`, capacità, contenuto).
    pub fn bin(&self, root: &str, f: impl FnOnce(&mut VolumeBin)) -> Self {
        self.with(|w| f(&mut Self::volume_mut(w, root).bin))
    }

    /// `volume_bin` sul volume `root` fallisce con `code`.
    pub fn volume_error(&self, root: &str, code: u32) -> Self {
        self.with(|w| Self::volume_mut(w, root).error = Some(code))
    }

    pub fn set_free(&self, root: &str, bytes: u64) -> Self {
        self.with(|w| Self::volume_mut(w, root).free = bytes)
    }

    /// Svuota il Cestino del volume `root`: lo spazio libero cresce delle allocate di ciò che il mover ha
    /// spostato (una volta per identità, e solo se nessun altro nome del file è rimasto) più i byte degli
    /// altri elementi che il Cestino conteneva (contati alla dimensione logica).
    pub fn empty_bin(&self, root: &str) -> Self {
        self.with(|w| {
            let v = Self::volume_mut(w, root);
            let moved = std::mem::take(&mut v.moved);
            let moved_logical = std::mem::take(&mut v.moved_logical);
            let other = v.bin.content.map_or(0, |c| c.bytes.saturating_sub(moved_logical));
            if let Some(c) = &mut v.bin.content {
                *c = BinCount::default();
            }
            let freed = w.release(&moved) + other;
            Self::volume_mut(w, root).free += freed;
        })
    }

    // -----------------------------------------------------------------------------------------
    // Albero

    /// Cartella (con quelle sopra).
    pub fn dir(&self, path: &str) -> Self {
        self.with(|w| w.ensure_dir(path))
    }

    /// File con un solo nome, identità nuova, data di modifica uguale all'ora corrente.
    pub fn file(&self, path: &str, logical: u64, alloc: u64) -> Self {
        self.with(|w| {
            let a = w.file_attrs(&norm(path), logical, alloc);
            w.insert(path, a);
        })
    }

    /// Voce con attributi scritti dal test (per esempio senza identità, come su exFAT).
    pub fn entry(&self, path: &str, attrs: Attrs) -> Self {
        self.with(|w| w.insert(path, attrs))
    }

    /// File residente nella MFT: Windows riporta un'allocata piccola (la logica arrotondata a 8 byte, 16
    /// per un file da 10), ma nella stima vale 0.
    pub fn resident(&self, path: &str, logical: u64) -> Self {
        self.with(|w| {
            let mut a = w.file_attrs(&norm(path), logical, logical.div_ceil(8) * 8);
            a.resident = true;
            w.insert(path, a);
        })
    }

    /// Segnaposto cloud: logica `logical`, allocata solo i byte locali `local`.
    pub fn placeholder(&self, path: &str, logical: u64, local: u64) -> Self {
        self.with(|w| {
            let mut a = w.file_attrs(&norm(path), logical, local);
            a.placeholder = true;
            a.attributes |= FILE_ATTRIBUTE_OFFLINE;
            w.insert(path, a);
        })
    }

    /// Nuovo nome `path` del file `existing` (hard link): stessa identità, e il numero di nomi cresce su
    /// tutti i nomi del file.
    pub fn hard_link(&self, path: &str, existing: &str) -> Self {
        self.with(|w| {
            let a = w.entry_mut(existing).attrs;
            let key = (a.volume_serial, a.file_id);
            let links = a.links + 1;
            for e in w.entries.values_mut().filter(|e| (e.attrs.volume_serial, e.attrs.file_id) == key) {
                e.attrs.links = links;
            }
            w.insert(path, Attrs { links, ..a });
        })
    }

    /// Collegamento (symlink o junction): conta da solo e non si attraversa. Le voci che il test mette
    /// sotto un collegamento a una cartella fanno da bersaglio: `children` le elenca.
    pub fn link(&self, path: &str, directory: bool) -> Self {
        self.with(|w| {
            let mut a = if directory { w.dir_attrs(&norm(path)) } else { w.file_attrs(&norm(path), 0, 0) };
            a.link = true;
            a.attributes |= FILE_ATTRIBUTE_REPARSE_POINT;
            w.insert(path, a);
        })
    }

    /// Cartella negata: l'elenco dei figli fallisce con `ERROR_ACCESS_DENIED`.
    pub fn denied(&self, dir: &str) -> Self {
        self.children_error(dir, 5)
    }

    pub fn children_error(&self, dir: &str, code: u32) -> Self {
        self.with(|w| {
            w.ensure_dir(dir);
            w.children_errors.insert(norm(dir), code);
        })
    }

    /// Gli attributi di `path` non si leggono (`code`).
    pub fn attributes_error(&self, path: &str, code: u32) -> Self {
        self.with(|w| {
            w.attr_errors.insert(norm(path), code);
        })
    }

    /// Cambia gli attributi di una voce esistente.
    pub fn set_attrs(&self, path: &str, f: impl FnOnce(&mut Attrs)) -> Self {
        self.with(|w| f(&mut w.entry_mut(path).attrs))
    }

    /// Toglie `path` e il suo contenuto, senza Cestino e senza liberare spazio (lo ha tolto qualcun
    /// altro).
    pub fn remove(&self, path: &str) -> Self {
        self.with(|w| {
            w.take_subtree(&norm(path));
        })
    }

    // -----------------------------------------------------------------------------------------
    // Controlli

    /// Un processo usa il file `path` (Restart Manager).
    pub fn in_use(&self, path: &str, pid: u32, app: &str) -> Self {
        self.with(|w| w.in_use.entry(norm(path)).or_default().push(ProcessUse { pid, app_name: app.to_owned(), service: None }))
    }

    /// Restart Manager fallisce se tra i file registrati c'è `path`.
    pub fn rm_error(&self, path: &str, e: RmError) -> Self {
        self.with(|w| {
            w.rm_errors.insert(norm(path), e);
        })
    }

    /// Riferimento a `target` (o a qualcosa che lo contiene, se `target` sta sotto un percorso chiesto).
    pub fn reference(&self, kind: ReferenceKind, location: &str, target: &str) -> Self {
        self.with(|w| w.references.push(Reference { kind, location: location.to_owned(), target: target.to_owned() }))
    }

    /// Fonte di riferimenti non letta per intero: il controllo è "parziale".
    pub fn partial(&self, kind: ReferenceKind, code: u32) -> Self {
        self.with(|w| w.partial.push((kind, code)))
    }

    /// Giorni di Sensore memoria, o il codice di errore della lettura.
    pub fn storage_sense(&self, days: Result<Option<u32>, u32>) -> Self {
        self.with(|w| w.storage_sense = days)
    }

    /// Cartella di sincronizzazione registrata con radice `prefix`.
    pub fn synced(&self, prefix: &str, provider: &str) -> Self {
        self.with(|w| w.synced.push((norm(prefix), provider.to_owned())))
    }

    /// Cartella sincronizzata che Windows non conosce, con radice `prefix` (Dropbox, Syncthing...).
    pub fn synced_by(&self, prefix: &str, provider: &str, kind: crate::sync::SyncKind) -> Self {
        self.with(|w| w.markers.push((norm(prefix), crate::sync::SyncFolder { provider: provider.to_owned(), kind })))
    }

    pub fn set_now(&self, t: i64) -> Self {
        self.with(|w| w.now = t)
    }

    // -----------------------------------------------------------------------------------------
    // Letture per i test

    pub fn exists(&self, path: &str) -> bool {
        self.lock().entries.contains_key(&norm(path))
    }

    /// Percorsi passati a `attributes`, `children`, `volume_bin`, `sync_root` e `free_space`, in ordine.
    pub fn reads(&self) -> Vec<String> {
        self.lock().reads.clone()
    }

    /// Elenchi passati a `processes_using`, una voce per chiamata.
    pub fn rm_calls(&self) -> Vec<Vec<String>> {
        self.lock().rm_calls.clone()
    }

    /// Elenchi passati a `references`, una voce per chiamata.
    pub fn reference_calls(&self) -> Vec<Vec<String>> {
        self.lock().reference_calls.clone()
    }
}

impl Probe for FakeProbe {
    fn attributes(&self, path: &str) -> Result<Attrs, u32> {
        let mut w = self.lock();
        w.reads.push(path.to_owned());
        let key = norm(path);
        if let Some(&e) = w.attr_errors.get(&key) {
            return Err(e);
        }
        w.entries.get(&key).map(|e| e.attrs).ok_or(2)
    }

    fn children(&self, dir: &str) -> Result<Vec<String>, u32> {
        let mut w = self.lock();
        w.reads.push(dir.to_owned());
        let key = norm(dir);
        if let Some(&e) = w.children_errors.get(&key) {
            return Err(e);
        }
        // Un collegamento a una cartella elenca ciò che il test ha messo sotto di lui (il suo bersaglio):
        // così si vede se qualcuno lo attraversa.
        match w.entries.get(&key) {
            None => return Err(3),
            Some(e) if !e.attrs.directory => return Err(ERROR_DIRECTORY),
            Some(_) => {}
        }
        Ok(w.below(&key).filter(|(rest, _)| !rest.contains('\\')).map(|(_, e)| name_of(&e.path)).collect())
    }

    fn processes_using(&self, paths: &[String]) -> Result<Vec<ProcessUse>, RmError> {
        let mut w = self.lock();
        w.rm_calls.push(paths.to_vec());
        let keys: Vec<String> = paths.iter().map(|p| norm(p)).collect();
        if let Some(e) = keys.iter().find_map(|k| w.rm_errors.get(k)) {
            return Err(*e);
        }
        let mut out: Vec<ProcessUse> = Vec::new();
        for p in keys.iter().filter_map(|k| w.in_use.get(k)).flatten() {
            if !out.iter().any(|o| o.pid == p.pid) {
                out.push(p.clone());
            }
        }
        Ok(out)
    }

    fn references(&self, paths: &[String]) -> ReferenceReport {
        let mut w = self.lock();
        w.reference_calls.push(paths.to_vec());
        let keys: Vec<String> = paths.iter().map(|p| norm(p)).collect();
        let found = w.references.iter().filter(|r| keys.iter().any(|k| is_within(&norm(&r.target), k))).cloned().collect();
        ReferenceReport { found, partial: w.partial.clone() }
    }

    fn volume_bin(&self, path: &str) -> Result<VolumeBin, u32> {
        let mut w = self.lock();
        w.reads.push(path.to_owned());
        let i = w.volume_of(&norm(path)).ok_or(3u32)?;
        let v = &w.volumes[i];
        match v.error {
            Some(e) => Err(e),
            None => Ok(v.bin.clone()),
        }
    }

    fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
        self.lock().storage_sense
    }

    fn sync_root(&self, path: &str) -> Option<String> {
        let mut w = self.lock();
        w.reads.push(path.to_owned());
        let key = norm(path);
        w.synced.iter().filter(|(p, _)| is_within(&key, p)).max_by_key(|(p, _)| p.len()).map(|(_, provider)| provider.clone())
    }

    fn sync_marker(&self, path: &str) -> Option<crate::sync::SyncFolder> {
        let w = self.lock();
        let key = norm(path);
        w.markers.iter().filter(|(p, _)| is_within(&key, p)).max_by_key(|(p, _)| p.len()).map(|(_, f)| f.clone())
    }

    fn free_space(&self, path: &str) -> Result<u64, u32> {
        let mut w = self.lock();
        w.reads.push(path.to_owned());
        let i = w.volume_of(&norm(path)).ok_or(3u32)?;
        Ok(w.volumes[i].free)
    }

    fn now(&self) -> i64 {
        self.lock().now
    }
}

#[derive(Debug, Default)]
struct MoverState {
    /// Esito forzato e se l'elemento sparisce (eliminato per sempre), per percorso normalizzato.
    forced: HashMap<String, (MoveResult, bool)>,
    calls: Vec<String>,
}

/// Spostamenti simulati sul [`FakeProbe`] a cui è legato. Per default sposta nel Cestino come Windows
/// (vedi la documentazione del modulo); [`FakeMover::force`] impone un esito per un percorso.
#[derive(Clone, Debug)]
pub struct FakeMover {
    probe: FakeProbe,
    state: Arc<Mutex<MoverState>>,
}

impl FakeMover {
    pub fn new(probe: &FakeProbe) -> Self {
        Self { probe: probe.clone(), state: Arc::default() }
    }

    fn lock(&self) -> MutexGuard<'_, MoverState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Per `path` Windows risponde `result`; con `vanish` l'elemento sparisce senza arrivare nel Cestino
    /// (eliminato per sempre), altrimenti resta al suo posto.
    pub fn force(&self, path: &str, result: MoveResult, vanish: bool) -> Self {
        self.lock().forced.insert(norm(path), (result, vanish));
        self.clone()
    }

    /// Percorsi ricevuti, nell'ordine.
    pub fn calls(&self) -> Vec<String> {
        self.lock().calls.clone()
    }
}

impl Mover for FakeMover {
    fn recycle(&self, path: &str) -> MoveResult {
        let forced = {
            let mut s = self.lock();
            s.calls.push(path.to_owned());
            s.forced.get(&norm(path)).copied()
        };
        match forced {
            Some((result, vanish)) => {
                if vanish {
                    self.probe.lock().destroy(path);
                }
                result
            }
            None => self.probe.lock().recycle(path),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Record di prova

/// Prova di gestibilità di prova (un fatto "da fonte").
pub fn proof() -> ProvaDiGestibilita {
    let f = Fact::new("fatto-prova", Provenance::FromSource, Source::Rule, Vec::new());
    ProvaDiGestibilita::new(vec![f]).expect("un fatto da fonte prova la gestibilità")
}

/// Record di prova che propone lo spostamento nel Cestino, con lo stato dato: regola `prova` verificata il
/// 14/10/2026, nessun controllo, senza eccezione per gli elementi protetti.
pub fn record(state: State) -> Record {
    Record {
        state,
        action: Action::Recycle,
        identity: ProvaDiIdentita::default(),
        hypotheses: Vec::new(),
        reliability: Criterion::OfficialWithStructure,
        risks: Vec::new(),
        preferred: None,
        alternatives: Vec::new(),
        checks: Vec::new(),
        estimate: Estimate::NotEstimable(NotEstimable::AtSimulation),
        reversibility: "reversibilita-prova".into(),
        privileges: Privileges::None,
        on_error: "errore-prova".into(),
        after: "verifica-prova".into(),
        origin: Origin::Rule(RuleRef { id: "prova".into(), verified: Date::new(2026, 10, 14) }),
        expiry: Expiry::Fresh,
        protected_exception: false,
    }
}

/// Controllo automatico che si esegue nella simulazione (`controllo-restart-manager`, ...).
pub fn automatic(key: &str) -> Check {
    Check { key: key.to_owned(), kind: CheckKind::Automatic, outcome: CheckOutcome::AtSimulation }
}

/// Controllo manuale da spuntare.
pub fn manual(key: &str) -> Check {
    Check { key: key.to_owned(), kind: CheckKind::Manual, outcome: CheckOutcome::ToTick }
}
