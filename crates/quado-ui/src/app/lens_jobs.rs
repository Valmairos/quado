// SPDX-License-Identifier: GPL-3.0-or-later
//! Lavori della lente Download: letture su richiesta in thread separati, annullabili, con l'avanzamento in
//! file e byte e i risultati per canale. Nessun lavoro tiene fermo il thread dell'interfaccia: il frame
//! raccoglie solo i risultati già pronti.
//!
//! Le letture vere passano da [`Readers`]: l'app usa [`Readers::live`]; i test una versione finta che non
//! tocca il disco (`synthetic::FakeReaders`); `Env::fixed` una che non legge niente ([`Readers::inert`]);
//! il banco di misura una che rifiuta i percorsi esclusi ([`Readers::excluding`]), così nessuna lettura
//! raggiunge quei file qualunque sia il comando.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::Instant;

use quado_kb::extract::{ExtractError, FormatIssue};
use quado_kb::installers::{InstallerInfo, InstallerKind};
use quado_model::downloads::{FileUse, Hashed};
use quado_kb::lens::UseCheck;
use quado_scan_win::content::{ContentError, FileIdentity};
use quado_scan_win::zip::{ZipError, ZipIndex};

/// Thread dell'impronta SHA-256. Banco `hash` su un portatile a 6 core con SSD NVMe, 1,11 GiB di file della
/// stessa dimensione per giro, il 14/10/2026: 1 thread 7,9 s e frame p95 1,65 ms; 2 thread 4,5-4,9 s e
/// p95 1,7 ms; 3 thread 3,2 s e p95 1,9 ms. Il frame non cambia, il confronto va 2,5 volte più veloce.
/// Il banco ne può chiedere un altro numero (`--thread`).
pub const HASH_THREADS: usize = 3;

/// Thread della lettura dei dati degli installatori. La prima lettura di un installatore dopo un po' è
/// quasi tutta l'attesa di Defender, che controlla il file all'apertura (misura del 26/10/2026: 187 file
/// in 9,8-12,5 s con un thread, 0,34 s a distanza di minuti): con più file aperti insieme Defender li
/// controlla in parallelo (`cargo xtask lente-installatori`).
pub const INSTALLER_THREADS: usize = 4;

/// Codice di Windows restituito per i percorsi esclusi dal banco (`ERROR_ACCESS_DISABLED_BY_POLICY`).
pub const EXCLUDED: u32 = 1260;

/// Codice di Windows delle letture che [`Readers::inert`] non fa (`ERROR_NOT_SUPPORTED`).
pub const NOT_READ: u32 = 50;

/// Le letture che la lente fa su richiesta. Ognuna riguarda un file, o un elenco per Restart Manager, e
/// non tocca il modello.
pub trait LensReaders: Send + Sync {
    /// Identità dai soli attributi (dimensione, date, ID), per la chiave della cache degli installatori. Non
    /// legge il contenuto, non cambia la data di ultimo accesso e non scarica i segnaposto cloud.
    fn identity(&self, path: &str) -> Result<FileIdentity, ContentError>;
    /// SHA-256 del contenuto: `progress` cresce dei byte letti, `cancel` ferma la lettura.
    fn sha256(&self, path: &str, cancel: &AtomicBool, progress: &AtomicU64) -> Result<(FileIdentity, [u8; 32]), ContentError>;
    /// Dati di versione di un installatore: risorsa di versione (exe) o tabella Property (MSI).
    fn installer(&self, path: &str, kind: InstallerKind) -> Result<InstallerInfo, ExtractError>;
    /// Directory centrale di uno zip.
    fn zip_index(&self, path: &str) -> Result<ZipIndex, ZipError>;
    /// Nomi dei processi che usano almeno uno dei file, secondo Restart Manager. Errore: il codice
    /// di Windows. Non legge il contenuto.
    fn processes(&self, paths: &[String]) -> Result<Vec<String>, u32>;
    /// Ora attuale (`FILETIME`), per l'ora dei controlli.
    fn now(&self) -> i64;
}

/// Insieme di letture condiviso tra l'interfaccia e i lavori.
#[derive(Clone)]
pub struct Readers(pub Arc<dyn LensReaders>);

impl std::fmt::Debug for Readers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Readers")
    }
}

impl Readers {
    /// Le letture vere: `quado_scan_win::content` e `zip`, `quado_kb::installers_win`,
    /// `quado_actions::restart`.
    pub fn live() -> Self {
        Self(Arc::new(Live))
    }

    /// Nessuna lettura: ogni richiesta risponde con un errore senza aprire file. Per i test che non
    /// iniettano letture finte.
    pub fn inert() -> Self {
        Self(Arc::new(Inert))
    }

    /// Le stesse letture, con i percorsi che contengono uno dei `fragments` (senza distinzione di maiuscole)
    /// rifiutati prima di aprirli, con [`EXCLUDED`]. Per il banco di misura.
    pub fn excluding(self, fragments: &[String]) -> Self {
        let fragments: Vec<String> = fragments.iter().map(|f| f.trim().to_lowercase()).filter(|f| !f.is_empty()).collect();
        if fragments.is_empty() {
            return self;
        }
        Self(Arc::new(Excluding { inner: self.0, fragments, refused: AtomicUsize::new(0) }))
    }
}

struct Live;

impl LensReaders for Live {
    fn identity(&self, path: &str) -> Result<FileIdentity, ContentError> {
        quado_scan_win::content::identity(path)
    }

    fn sha256(&self, path: &str, cancel: &AtomicBool, progress: &AtomicU64) -> Result<(FileIdentity, [u8; 32]), ContentError> {
        quado_scan_win::content::sha256(path, cancel, progress)
    }

    fn installer(&self, path: &str, kind: InstallerKind) -> Result<InstallerInfo, ExtractError> {
        match kind {
            InstallerKind::Msi => quado_kb::installers_win::msi_info(path),
            InstallerKind::Exe => quado_kb::installers_win::version_info(path),
        }
    }

    fn zip_index(&self, path: &str) -> Result<ZipIndex, ZipError> {
        quado_scan_win::zip::zip_index(path)
    }

    fn processes(&self, paths: &[String]) -> Result<Vec<String>, u32> {
        let used = quado_actions::restart::processes_using(paths).map_err(|e| e.code())?;
        Ok(used.into_iter().map(|p| if p.app_name.is_empty() { p.service.unwrap_or_default() } else { p.app_name }).collect())
    }

    fn now(&self) -> i64 {
        quado_scan_win::system::now_filetime()
    }
}

struct Inert;

impl LensReaders for Inert {
    fn identity(&self, _: &str) -> Result<FileIdentity, ContentError> {
        Err(ContentError::Io(NOT_READ))
    }

    fn sha256(&self, _: &str, _: &AtomicBool, _: &AtomicU64) -> Result<(FileIdentity, [u8; 32]), ContentError> {
        Err(ContentError::Io(NOT_READ))
    }

    fn installer(&self, _: &str, _: InstallerKind) -> Result<InstallerInfo, ExtractError> {
        Err(ExtractError::Io(NOT_READ))
    }

    fn zip_index(&self, _: &str) -> Result<ZipIndex, ZipError> {
        Err(ZipError::Io(NOT_READ))
    }

    fn processes(&self, _: &[String]) -> Result<Vec<String>, u32> {
        Err(NOT_READ)
    }

    fn now(&self) -> i64 {
        quado_scan_win::system::now_filetime()
    }
}

struct Excluding {
    inner: Arc<dyn LensReaders>,
    fragments: Vec<String>,
    refused: AtomicUsize,
}

impl Excluding {
    fn refuses(&self, path: &str) -> bool {
        let p = path.to_lowercase();
        let hit = self.fragments.iter().any(|f| p.contains(f.as_str()));
        if hit {
            self.refused.fetch_add(1, Relaxed);
        }
        hit
    }
}

impl LensReaders for Excluding {
    fn identity(&self, path: &str) -> Result<FileIdentity, ContentError> {
        if self.refuses(path) { Err(ContentError::Io(EXCLUDED)) } else { self.inner.identity(path) }
    }

    fn sha256(&self, path: &str, cancel: &AtomicBool, progress: &AtomicU64) -> Result<(FileIdentity, [u8; 32]), ContentError> {
        if self.refuses(path) { Err(ContentError::Io(EXCLUDED)) } else { self.inner.sha256(path, cancel, progress) }
    }

    fn installer(&self, path: &str, kind: InstallerKind) -> Result<InstallerInfo, ExtractError> {
        if self.refuses(path) { Err(ExtractError::Io(EXCLUDED)) } else { self.inner.installer(path, kind) }
    }

    fn zip_index(&self, path: &str) -> Result<ZipIndex, ZipError> {
        if self.refuses(path) { Err(ZipError::Io(EXCLUDED)) } else { self.inner.zip_index(path) }
    }

    fn processes(&self, paths: &[String]) -> Result<Vec<String>, u32> {
        // Un elenco con un percorso escluso non si registra: chi chiama ripiega sui file uno per uno.
        if paths.iter().any(|p| self.refuses(p)) { Err(EXCLUDED) } else { self.inner.processes(paths) }
    }

    fn now(&self) -> i64 {
        self.inner.now()
    }
}

// ---------------------------------------------------------------------------------------------
// Lavori

/// Avanzamento condiviso con i thread: file finiti (anche con un errore) e byte letti.
#[derive(Default)]
pub struct Progress {
    pub files: AtomicUsize,
    pub bytes: AtomicU64,
}

/// Lavoro su un elenco di elementi, con uno o più thread che prendono l'elemento successivo finché ce ne
/// sono o finché non arriva l'annullamento (controllato prima di ogni elemento; dentro un elemento lo
/// controlla la lettura, come l'impronta a blocchi da 1 MiB). Ogni risultato arriva con l'indice
/// dell'elemento. Un thread che non parte lascia i suoi elementi non letti, senza fermare gli altri.
///
/// Con [`Batch::spawn_in_order`] i risultati arrivano nell'ordine degli elementi, come con un thread solo:
/// un risultato arrivato prima del suo turno aspetta quelli prima di lui (che sono già in lettura: i thread
/// prendono gli elementi in ordine e finiscono quello che hanno preso).
pub struct Batch<O> {
    cancel: Arc<AtomicBool>,
    pub progress: Arc<Progress>,
    rx: Receiver<(usize, O)>,
    threads: Vec<JoinHandle<()>>,
    pub total_files: usize,
    pub total_bytes: u64,
    pub started: Instant,
    finished: bool,
    /// Risultati arrivati prima del loro turno, per indice; `None` se l'ordine non conta.
    early: Option<std::collections::BTreeMap<usize, O>>,
    /// Indice del prossimo risultato da consegnare, con l'ordine.
    turn: usize,
}

impl<O: Send + 'static> Batch<O> {
    pub fn spawn<I, F>(name: &str, items: Vec<I>, threads: usize, total_bytes: u64, work: F) -> Self
    where
        I: Send + Sync + 'static,
        F: Fn(&I, &AtomicBool, &AtomicU64) -> O + Send + Sync + 'static,
    {
        Self::start(name, items, threads, total_bytes, work, false)
    }

    /// Come [`Batch::spawn`], con i risultati consegnati nell'ordine degli elementi.
    pub fn spawn_in_order<I, F>(name: &str, items: Vec<I>, threads: usize, total_bytes: u64, work: F) -> Self
    where
        I: Send + Sync + 'static,
        F: Fn(&I, &AtomicBool, &AtomicU64) -> O + Send + Sync + 'static,
    {
        Self::start(name, items, threads, total_bytes, work, true)
    }

    fn start<I, F>(name: &str, items: Vec<I>, threads: usize, total_bytes: u64, work: F, in_order: bool) -> Self
    where
        I: Send + Sync + 'static,
        F: Fn(&I, &AtomicBool, &AtomicU64) -> O + Send + Sync + 'static,
    {
        let total_files = items.len();
        let items = Arc::new(items);
        let work = Arc::new(work);
        let next = Arc::new(AtomicUsize::new(0));
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Progress::default());
        let (tx, rx) = channel();
        let mut handles = Vec::new();
        for k in 0..threads.clamp(1, total_files.max(1)) {
            let (items, work, next, cancel, progress, tx) =
                (Arc::clone(&items), Arc::clone(&work), Arc::clone(&next), Arc::clone(&cancel), Arc::clone(&progress), tx.clone());
            let spawned = std::thread::Builder::new().name(format!("quado-lente-{name}-{k}")).spawn(move || {
                while !cancel.load(Relaxed) {
                    let i = next.fetch_add(1, Relaxed);
                    let Some(item) = items.get(i) else { break };
                    let out = work(item, &cancel, &progress.bytes);
                    progress.files.fetch_add(1, Relaxed);
                    if tx.send((i, out)).is_err() {
                        break;
                    }
                }
            });
            match spawned {
                Ok(h) => handles.push(h),
                Err(e) => log::warn!("lavoro della lente {name}: thread non avviato ({e})"),
            }
        }
        Self {
            cancel,
            progress,
            rx,
            threads: handles,
            total_files,
            total_bytes,
            started: Instant::now(),
            finished: false,
            early: in_order.then(Default::default),
            turn: 0,
        }
    }

    /// Risultati arrivati, senza attendere.
    pub fn drain(&mut self) -> Vec<(usize, O)> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(x) => out.push(x),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.finished = true;
                    break;
                }
            }
        }
        self.in_turn(out)
    }

    /// Con l'ordine, i risultati che toccano ora: quelli in fila dal prossimo indice, gli altri restano da
    /// parte. Alla fine del lavoro escono tutti (un elemento mai finito, per un thread fermato da un panico,
    /// non trattiene gli altri).
    fn in_turn(&mut self, arrived: Vec<(usize, O)>) -> Vec<(usize, O)> {
        let Some(early) = self.early.as_mut() else { return arrived };
        early.extend(arrived);
        let mut out = Vec::new();
        while let Some(x) = early.remove(&self.turn) {
            out.push((self.turn, x));
            self.turn += 1;
        }
        if self.finished {
            out.extend(std::mem::take(early));
        }
        out
    }

    /// Tutti i thread sono finiti e i risultati sono stati raccolti.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Relaxed)
    }

    /// File finiti e byte letti finora.
    pub fn done(&self) -> (usize, u64) {
        (self.progress.files.load(Relaxed), self.progress.bytes.load(Relaxed))
    }

    /// Attende la fine dei thread (dopo [`Self::cancel`], al più la fine dell'elemento in corso) e
    /// restituisce i risultati non ancora raccolti. Dopo, nessun thread tiene più ciò che il lavoro ha
    /// catturato (per esempio il modello, che si può di nuovo modificare).
    pub fn wait(&mut self) -> Vec<(usize, O)> {
        for h in std::mem::take(&mut self.threads) {
            let _ = h.join();
        }
        // Tutti i thread sono finiti: con l'ordine escono anche i risultati rimasti da parte.
        self.finished = true;
        self.drain()
    }
}

/// Un file da leggere: nodo e percorso, con la dimensione della scansione.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub node: u32,
    pub path: String,
    pub size: u64,
}

/// Impronta di un file letto, come la vuole `downloads::compare_groups`.
pub fn hashed(node: u32, id: &FileIdentity, digest: [u8; 32]) -> Hashed {
    Hashed { node, size: id.size, digest, created: id.created }
}

/// Lavoro dell'impronta SHA-256 sui file della stessa dimensione, con `threads` thread.
pub fn spawn_hash(readers: &Readers, targets: Vec<Target>, threads: usize) -> Batch<Result<(FileIdentity, [u8; 32]), ContentError>> {
    let bytes = targets.iter().map(|t| t.size).sum();
    let r = readers.clone();
    Batch::spawn("impronte", targets, threads, bytes, move |t: &Target, cancel, progress| r.0.sha256(&t.path, cancel, progress))
}

/// Come `spawn_hash`, con i percorsi costruiti dai thread del lavoro a partire dall'albero (bersagli con
/// `path` vuoto): con centinaia di migliaia di file costruirli nel frame costa secondi (misurato con
/// il banco su una cartella Download con quasi duecentomila file: un frame da 2 s).
pub fn spawn_hash_in(readers: &Readers, model: Arc<crate::app::data::Model>, targets: Vec<Target>, threads: usize) -> Batch<Result<(FileIdentity, [u8; 32]), ContentError>> {
    let bytes = targets.iter().map(|t| t.size).sum();
    let r = readers.clone();
    Batch::spawn("impronte", targets, threads, bytes, move |t: &Target, cancel, progress| {
        if t.path.is_empty() { r.0.sha256(&model.tree.path_lossy(t.node), cancel, progress) } else { r.0.sha256(&t.path, cancel, progress) }
    })
}

/// Identità dei soli attributi degli installatori, prima di chiedere conferma: dice quali file sono nuovi
/// o cambiati rispetto alla cache.
pub fn spawn_identity(readers: &Readers, targets: Vec<Target>) -> Batch<Result<FileIdentity, ContentError>> {
    let r = readers.clone();
    Batch::spawn("attributi", targets, 1, 0, move |t: &Target, _, _| r.0.identity(&t.path))
}

/// Lettura dei dati di versione dei file chiesti, con [`INSTALLER_THREADS`] thread e i risultati
/// nell'ordine dei file.
pub fn spawn_installers(readers: &Readers, targets: Vec<(Target, InstallerKind)>) -> Batch<Result<InstallerInfo, ExtractError>> {
    spawn_installers_with(readers, targets, INSTALLER_THREADS)
}

/// Come [`spawn_installers`], con `threads` thread (per le misure).
pub fn spawn_installers_with(readers: &Readers, targets: Vec<(Target, InstallerKind)>, threads: usize) -> Batch<Result<InstallerInfo, ExtractError>> {
    let bytes = targets.iter().map(|(t, _)| t.size).sum();
    let r = readers.clone();
    Batch::spawn_in_order("installatori", targets, threads, bytes, move |(t, kind): &(Target, InstallerKind), _, progress| {
        let out = r.0.installer(&t.path, *kind);
        progress.fetch_add(t.size, Relaxed);
        out
    })
}

/// Lettura degli indici degli zip. Il confronto con la cartella lo fa `compare` nello stesso thread,
/// perché un indice può avere fino a un milione di voci.
pub fn spawn_zip<C>(readers: &Readers, targets: Vec<Target>, compare: C) -> Batch<Result<quado_model::downloads::ArchiveCheck, ExtractError>>
where
    C: Fn(u32, &ZipIndex) -> quado_model::downloads::ArchiveCheck + Send + Sync + 'static,
{
    let bytes = targets.iter().map(|t| t.size).sum();
    let r = readers.clone();
    Batch::spawn("archivi", targets, 1, bytes, move |t: &Target, _, progress| {
        let out = r.0.zip_index(&t.path).map(|ix| compare(t.node, &ix)).map_err(zip_error);
        progress.fetch_add(t.size, Relaxed);
        out
    })
}

/// Restart Manager sui download incompleti: una sessione per tutti; se qualcuno li usa, o se
/// l'elenco intero non si registra, un'interrogazione per file, così ogni file ha il suo esito. Un file
/// senza risposta resta "uso non noto", mai "non in uso".
pub fn spawn_usage(readers: &Readers, targets: Vec<Target>) -> Batch<Vec<(u32, UseCheck)>> {
    let r = readers.clone();
    Batch::spawn("uso", vec![targets], 1, 0, move |targets: &Vec<Target>, _, _| usage(&r, targets))
}

fn usage(r: &Readers, targets: &[Target]) -> Vec<(u32, UseCheck)> {
    let paths: Vec<String> = targets.iter().map(|t| t.path.clone()).collect();
    let at = r.0.now();
    if let Ok(names) = r.0.processes(&paths)
        && names.is_empty()
    {
        return targets.iter().map(|t| (t.node, UseCheck { usage: FileUse::Free, at })).collect();
    }
    targets
        .iter()
        .map(|t| {
            let usage = match r.0.processes(std::slice::from_ref(&t.path)) {
                Ok(names) if names.is_empty() => FileUse::Free,
                Ok(names) => FileUse::Used(names),
                Err(code) => FileUse::Unknown(code),
            };
            (t.node, UseCheck { usage, at: r.0.now() })
        })
        .collect()
}

/// Errore del lettore degli zip come errore di estrattore: "non leggibile" con il motivo.
pub fn zip_error(e: ZipError) -> ExtractError {
    match e {
        ZipError::Io(2 | 3) => ExtractError::NotPresent,
        ZipError::Io(c) => ExtractError::Io(c),
        ZipError::CloudPlaceholder => ExtractError::CloudPlaceholder,
        ZipError::NotZip | ZipError::Corrupt => ExtractError::Format(FormatIssue::Unexpected),
        ZipError::Truncated => ExtractError::Format(FormatIssue::Truncated),
        ZipError::TooLarge => ExtractError::Format(FormatIssue::TooLarge),
    }
}

/// Errore dell'identità come errore di estrattore: un installatore che non si è potuto controllare.
pub fn identity_error(e: ContentError) -> ExtractError {
    match e {
        ContentError::Io(2 | 3) => ExtractError::NotPresent,
        ContentError::Io(c) => ExtractError::Io(c),
        ContentError::CloudPlaceholder => ExtractError::CloudPlaceholder,
        ContentError::NotAFile | ContentError::Changed | ContentError::Cancelled => ExtractError::Format(FormatIssue::Unexpected),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Letture che contano le chiamate e rispondono subito.
    #[derive(Default)]
    struct Counting {
        calls: Mutex<Vec<String>>,
        busy: Mutex<Vec<String>>,
    }

    impl LensReaders for Counting {
        fn identity(&self, path: &str) -> Result<FileIdentity, ContentError> {
            self.calls.lock().map(|mut c| c.push(format!("id {path}"))).ok();
            Ok(FileIdentity { size: 1, created: 0, modified: 0, accessed: 0, file_id: 1, volume_serial: 1, attributes: 0x20 })
        }
        fn sha256(&self, path: &str, cancel: &AtomicBool, progress: &AtomicU64) -> Result<(FileIdentity, [u8; 32]), ContentError> {
            if cancel.load(Relaxed) {
                return Err(ContentError::Cancelled);
            }
            progress.fetch_add(10, Relaxed);
            self.calls.lock().map(|mut c| c.push(format!("hash {path}"))).ok();
            Ok((FileIdentity { size: 10, created: 0, modified: 0, accessed: 0, file_id: 1, volume_serial: 1, attributes: 0x20 }, [0; 32]))
        }
        fn installer(&self, _: &str, _: InstallerKind) -> Result<InstallerInfo, ExtractError> {
            Ok(InstallerInfo::default())
        }
        fn zip_index(&self, _: &str) -> Result<ZipIndex, ZipError> {
            Err(ZipError::Corrupt)
        }
        fn processes(&self, paths: &[String]) -> Result<Vec<String>, u32> {
            let busy = self.busy.lock().map(|b| b.clone()).unwrap_or_default();
            if paths.len() > 1 && paths.iter().any(|p| p.contains("lungo")) {
                return Err(29);
            }
            Ok(paths.iter().filter(|p| busy.contains(p)).map(|_| "prova.exe".to_owned()).collect())
        }
        fn now(&self) -> i64 {
            7
        }
    }

    fn target(node: u32, path: &str) -> Target {
        Target { node, path: path.into(), size: 10 }
    }

    #[test]
    fn batches_deliver_every_result_with_progress() {
        let c = Arc::new(Counting::default());
        let r = Readers(c.clone());
        let targets: Vec<Target> = (0..20).map(|k| target(k, &format!("f{k}"))).collect();
        let mut b = spawn_hash(&r, targets, 3);
        assert_eq!((b.total_files, b.total_bytes), (20, 200));
        let mut got = b.wait();
        got.sort_by_key(|x| x.0);
        assert_eq!(got.iter().map(|x| x.0).collect::<Vec<_>>(), (0..20).collect::<Vec<_>>());
        assert!(got.iter().all(|x| x.1.is_ok()));
    }

    /// Letture degli installatori che durano di più per i primi file: con più thread finiscono in disordine.
    struct Uneven;

    impl LensReaders for Uneven {
        fn identity(&self, _: &str) -> Result<FileIdentity, ContentError> {
            Err(ContentError::Io(NOT_READ))
        }
        fn sha256(&self, _: &str, _: &AtomicBool, _: &AtomicU64) -> Result<(FileIdentity, [u8; 32]), ContentError> {
            Err(ContentError::Io(NOT_READ))
        }
        fn installer(&self, path: &str, kind: InstallerKind) -> Result<InstallerInfo, ExtractError> {
            let k: u64 = path.trim_start_matches('f').parse().unwrap_or(0);
            std::thread::sleep(std::time::Duration::from_millis((40 - k % 40) / 4));
            if k.is_multiple_of(5) {
                return Err(ExtractError::Io(5));
            }
            Ok(InstallerInfo { kind: Some(kind), product_name: Some(path.to_owned()), ..Default::default() })
        }
        fn zip_index(&self, _: &str) -> Result<ZipIndex, ZipError> {
            Err(ZipError::Io(NOT_READ))
        }
        fn processes(&self, _: &[String]) -> Result<Vec<String>, u32> {
            Err(NOT_READ)
        }
        fn now(&self) -> i64 {
            0
        }
    }

    #[test]
    fn installers_arrive_in_file_order_whatever_the_threads() {
        let r = Readers(Arc::new(Uneven));
        let targets: Vec<(Target, InstallerKind)> = (0..60).map(|k| (target(k, &format!("f{k}")), InstallerKind::Exe)).collect();
        let one = spawn_installers_with(&r, targets.clone(), 1).wait();
        assert_eq!(one.iter().map(|x| x.0).collect::<Vec<_>>(), (0..60).collect::<Vec<_>>());
        // Raccolti a pezzi durante il lavoro, come fa il frame.
        let mut b = spawn_installers(&r, targets);
        let mut many = Vec::new();
        while !b.is_finished() {
            many.extend(b.drain());
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        many.extend(b.wait());
        assert_eq!(many, one);
        assert_eq!(INSTALLER_THREADS, 4);
    }

    #[test]
    fn a_cancelled_ordered_batch_delivers_what_it_read_in_order() {
        let r = Readers(Arc::new(Uneven));
        let targets: Vec<(Target, InstallerKind)> = (0..200).map(|k| (target(k, &format!("f{k}")), InstallerKind::Exe)).collect();
        let mut b = spawn_installers_with(&r, targets, 4);
        std::thread::sleep(std::time::Duration::from_millis(30));
        b.cancel();
        let got = b.wait();
        assert!(got.len() < 200, "annullato: {} file", got.len());
        assert_eq!(got.iter().map(|x| x.0).collect::<Vec<_>>(), (0..got.len()).collect::<Vec<_>>());
    }

    #[test]
    fn a_cancelled_batch_stops_before_the_next_file() {
        let r = Readers(Arc::new(Counting::default()));
        let mut b = spawn_hash(&r, (0..1000).map(|k| target(k, "f")).collect(), 1);
        b.cancel();
        let got = b.wait();
        assert!(got.len() < 1000);
    }

    #[test]
    fn usage_is_per_file_when_someone_uses_them() {
        let c = Arc::new(Counting::default());
        let r = Readers(c.clone());
        let free = usage(&r, &[target(1, "a"), target(2, "b")]);
        assert!(free.iter().all(|(_, u)| u.usage == FileUse::Free && u.at == 7));
        c.busy.lock().map(|mut b| b.push("b".into())).ok();
        let one = usage(&r, &[target(1, "a"), target(2, "b")]);
        assert_eq!(one[0].1.usage, FileUse::Free);
        assert_eq!(one[1].1.usage, FileUse::Used(vec!["prova.exe".into()]));
        // L'elenco intero non si registra: si interroga file per file.
        let long = usage(&r, &[target(1, "a"), target(3, "lungo")]);
        assert_eq!(long.iter().map(|x| &x.1.usage).collect::<Vec<_>>(), [&FileUse::Free, &FileUse::Free]);
    }

    #[test]
    fn excluded_paths_never_reach_the_reader() {
        let c = Arc::new(Counting::default());
        let r = Readers(c.clone()).excluding(&["\\Escluso\\".into()]);
        assert_eq!(r.0.identity(r"C:\Dati\escluso\a.exe").err(), Some(ContentError::Io(EXCLUDED)));
        assert_eq!(r.0.installer(r"C:\Dati\ESCLUSO\a.exe", InstallerKind::Exe).err(), Some(ExtractError::Io(EXCLUDED)));
        assert!(r.0.identity(r"C:\Dati\altro\a.exe").is_ok());
        let calls = c.calls.lock().map(|c| c.clone()).unwrap_or_default();
        assert_eq!(calls, [r"id C:\Dati\altro\a.exe"]);
    }
}
