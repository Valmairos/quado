// SPDX-License-Identifier: GPL-3.0-or-later
//! Letture del contenuto su richiesta: identità di un file (dimensione, date, ID) senza leggerne il
//! contenuto, e SHA-256 del contenuto. Mai durante la scansione, mai sui segnaposto cloud; nessun handle in
//! scrittura. La lettura aggiorna la data di ultimo accesso.
//!
//! Ogni lettura apre il file due volte. Prima con i soli attributi (`FILE_READ_ATTRIBUTES`, come la
//! scansione): identità, tipo, segnaposto cloud. Solo se il file si può leggere lo si riapre con
//! `GENERIC_READ` e si ricontrolla tutto sul nuovo handle, prima del primo byte. Anche questo handle ha
//! `FILE_FLAG_OPEN_NO_RECALL`: se il file diventasse un segnaposto tra le due aperture, Windows non lo
//! scarica. I collegamenti (symlink, junction, punti di montaggio) non si seguono: `NotAFile`.
//!
//! L'hash usa SHA-256 di CNG (`bcrypt.dll`), senza dipendenze esterne.

use std::mem::{MaybeUninit, size_of};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_HANDLE_EOF, ERROR_INVALID_FUNCTION, ERROR_INVALID_NAME, ERROR_INVALID_PARAMETER, ERROR_NOT_SUPPORTED, GENERIC_READ,
    GetLastError, HANDLE, INVALID_HANDLE_VALUE, NTSTATUS, RtlNtStatusToDosError,
};
use windows_sys::Win32::Security::Cryptography::{
    BCRYPT_ALG_HANDLE, BCRYPT_HASH_HANDLE, BCRYPT_SHA256_ALGORITHM, BCryptCloseAlgorithmProvider, BCryptCreateHash, BCryptDestroyHash,
    BCryptFinishHash, BCryptHashData, BCryptOpenAlgorithmProvider,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_TAG_INFO, FILE_BASIC_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_NO_RECALL, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_FLAG_SEQUENTIAL_SCAN, FILE_ID_INFO, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO,
    FileAttributeTagInfo, FileBasicInfo, FileIdInfo, FileStandardInfo, GetFileInformationByHandleEx, OPEN_EXISTING, ReadFile, SYNCHRONIZE,
};
use windows_sys::Win32::System::IO::{OVERLAPPED, OVERLAPPED_0, OVERLAPPED_0_0};

use crate::win;

/// Byte letti per blocco dall'hash: tra un blocco e l'altro si controlla l'annullamento.
pub const BLOCK: usize = 1 << 20;

/// Accessi richiesti da questo modulo, per il test che verifica l'assenza di scrittura.
pub const ACCESS: [u32; 2] = [FILE_READ_ATTRIBUTES | SYNCHRONIZE, GENERIC_READ];

const _: () = assert!(ACCESS[0] & win::WRITE_BITS == 0 && ACCESS[1] & win::WRITE_BITS == 0);

// Stessa definizione della scansione (`scan.rs`, correzione dell'allocata): un file è un segnaposto se ha
// un attributo di richiamo o è offline, oppure se il suo reparse point è della Cloud Filter API.
const ATTR_REPARSE: u32 = 0x0400;
const ATTR_OFFLINE: u32 = 0x1000;
const ATTR_RECALL_ON_OPEN: u32 = 0x0004_0000;
const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
const TAG_NAME_SURROGATE: u32 = 0x2000_0000;

pub fn is_cloud_tag(tag: u32) -> bool {
    tag & 0xFFFF_0FFF == 0x9000_001A
}

/// Segnaposto cloud dagli attributi e dal tag di reparse (anche per `quado-actions`).
pub fn is_placeholder(attributes: u32, reparse_tag: u32) -> bool {
    attributes & (ATTR_OFFLINE | ATTR_RECALL_ON_OPEN | ATTR_RECALL_ON_DATA_ACCESS) != 0
        || (attributes & ATTR_REPARSE != 0 && is_cloud_tag(reparse_tag))
}

/// Identità di un file letta dai soli attributi. Le date sono `FILETIME` (100 ns dal 1601, UTC).
/// Se il file system non dà l'identità a 128 bit (`FileIdInfo` non supportato), `file_id` e
/// `volume_serial` valgono 0, come l'identità della scansione su exFAT e FAT32.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileIdentity {
    /// Dimensione logica.
    pub size: u64,
    pub created: i64,
    pub modified: i64,
    pub accessed: i64,
    pub file_id: u128,
    pub volume_serial: u64,
    /// Attributi di Windows (`FILE_ATTRIBUTE_*`).
    pub attributes: u32,
}

/// Perché una lettura non è riuscita. Codici, non frasi.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ContentError {
    /// Errore di Windows (per esempio 2 file non trovato, 5 accesso negato, 32 file in uso).
    Io(u32),
    /// Segnaposto cloud: il contenuto non si legge, perché leggerlo lo scaricherebbe.
    CloudPlaceholder,
    /// Annullata da chi ha chiesto la lettura.
    Cancelled,
    /// Dimensione, data di modifica o identità cambiate durante la lettura: l'hash non vale.
    Changed,
    /// Cartella o collegamento (symlink, junction, punto di montaggio): non si legge.
    NotAFile,
}

/// Handle di file chiuso al drop.
struct File(HANDLE);

impl Drop for File {
    fn drop(&mut self) {
        // SAFETY: l'handle è valido (costruito solo da `open` dopo il controllo di INVALID_HANDLE_VALUE)
        // e viene chiuso una sola volta, qui.
        unsafe { CloseHandle(self.0) };
    }
}

fn last_error() -> u32 {
    // SAFETY: nessun prerequisito.
    unsafe { GetLastError() }
}

fn open(path: &[u16], access: u32, flags: u32) -> Result<File, u32> {
    // Un carattere nullo troncherebbe il percorso: si aprirebbe un altro file.
    if path.contains(&0) {
        return Err(ERROR_INVALID_NAME);
    }
    let z = win::wide_z(&win::long_path(path));
    // SAFETY: `z` è terminata da zero e vive per tutta la chiamata; gli altri argomenti sono costanti
    // o puntatori nulli ammessi. `access` è uno dei valori di `ACCESS`, senza bit di scrittura.
    let h = unsafe {
        CreateFileW(z.as_ptr(), access, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, null(), OPEN_EXISTING, flags, null_mut())
    };
    if h == INVALID_HANDLE_VALUE { Err(last_error()) } else { Ok(File(h)) }
}

fn query<T>(h: &File, class: i32) -> Result<T, u32> {
    let mut out = MaybeUninit::<T>::zeroed();
    // SAFETY: il buffer è grande `size_of::<T>()` e allineato per `T`, che per le classi usate qui
    // (FileBasicInfo, FileStandardInfo, FileIdInfo, FileAttributeTagInfo) è la struttura documentata
    // a dimensione fissa.
    let ok = unsafe { GetFileInformationByHandleEx(h.0, class, out.as_mut_ptr().cast(), size_of::<T>() as u32) };
    if ok == 0 {
        return Err(last_error());
    }
    // SAFETY: la chiamata è riuscita e ha scritto la struttura; partiva comunque azzerata.
    Ok(unsafe { out.assume_init() })
}

/// Identità più quello che serve a decidere se il file si può leggere.
#[derive(Clone, Copy, Debug)]
struct Probe {
    id: FileIdentity,
    reparse_tag: u32,
    directory: bool,
}

impl Probe {
    fn read(h: &File) -> Result<Self, u32> {
        let basic: FILE_BASIC_INFO = query(h, FileBasicInfo)?;
        let standard: FILE_STANDARD_INFO = query(h, FileStandardInfo)?;
        let (volume_serial, file_id) = match query::<FILE_ID_INFO>(h, FileIdInfo) {
            Ok(i) => (i.VolumeSerialNumber, u128::from_le_bytes(i.FileId.Identifier)),
            // File system senza FileIdInfo: identità 0, come la scansione su exFAT e FAT32.
            Err(ERROR_INVALID_PARAMETER | ERROR_INVALID_FUNCTION | ERROR_NOT_SUPPORTED) => (0, 0),
            Err(e) => return Err(e),
        };
        let reparse_tag =
            if basic.FileAttributes & ATTR_REPARSE != 0 { query::<FILE_ATTRIBUTE_TAG_INFO>(h, FileAttributeTagInfo)?.ReparseTag } else { 0 };
        Ok(Self {
            id: FileIdentity {
                size: standard.EndOfFile.max(0) as u64,
                created: basic.CreationTime,
                modified: basic.LastWriteTime,
                accessed: basic.LastAccessTime,
                file_id,
                volume_serial,
                attributes: basic.FileAttributes,
            },
            reparse_tag,
            directory: standard.Directory,
        })
    }

    /// Il contenuto si può leggere: file, non collegamento, non segnaposto.
    fn readable(&self) -> Result<(), OpenError> {
        let a = self.id.attributes;
        if self.directory || (a & ATTR_REPARSE != 0 && self.reparse_tag & TAG_NAME_SURROGATE != 0) {
            Err(OpenError::NotAFile)
        } else if is_placeholder(a, self.reparse_tag) {
            Err(OpenError::CloudPlaceholder)
        } else {
            Ok(())
        }
    }
}

fn wide(path: &str) -> Vec<u16> {
    path.encode_utf16().collect()
}

fn open_attributes(path: &[u16]) -> Result<File, u32> {
    open(path, ACCESS[0], FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_OPEN_NO_RECALL)
}

/// Identità di `path` senza leggerne il contenuto: handle con il solo `FILE_READ_ATTRIBUTES`,
/// `FILE_FLAG_OPEN_REPARSE_POINT` e `FILE_FLAG_OPEN_NO_RECALL`. Non aggiorna la data di ultimo accesso
/// e non richiama i segnaposto cloud (che restituisce come gli altri file, con i loro attributi).
/// Di un collegamento restituisce l'identità del collegamento. Una cartella dà `NotAFile`.
pub fn identity(path: &str) -> Result<FileIdentity, ContentError> {
    let h = open_attributes(&wide(path)).map_err(ContentError::Io)?;
    let p = Probe::read(&h).map_err(ContentError::Io)?;
    if p.directory { Err(ContentError::NotAFile) } else { Ok(p.id) }
}

/// Esito negativo dell'apertura per la lettura del contenuto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenError {
    Io(u32),
    CloudPlaceholder,
    NotAFile,
}

impl From<OpenError> for ContentError {
    fn from(e: OpenError) -> Self {
        match e {
            OpenError::Io(c) => Self::Io(c),
            OpenError::CloudPlaceholder => Self::CloudPlaceholder,
            OpenError::NotAFile => Self::NotAFile,
        }
    }
}

/// File aperto per leggerne il contenuto, in sola lettura, dopo i controlli.
pub(crate) struct ContentFile {
    file: File,
    /// Identità dall'handle degli attributi, prima dell'apertura in lettura.
    pub before: FileIdentity,
    /// Identità dall'handle di lettura, prima di leggere il primo byte.
    pub opened: FileIdentity,
}

impl ContentFile {
    /// Controlla `path` con i soli attributi e, se si può leggere, lo apre con `GENERIC_READ`.
    /// `sequential` chiede `FILE_FLAG_SEQUENTIAL_SCAN` (lettura di tutto il file).
    pub fn open(path: &str, sequential: bool) -> Result<Self, OpenError> {
        let w = wide(path);
        let before = {
            let h = open_attributes(&w).map_err(OpenError::Io)?;
            let p = Probe::read(&h).map_err(OpenError::Io)?;
            p.readable()?;
            p
        };
        let mut flags = FILE_FLAG_OPEN_NO_RECALL;
        if sequential {
            flags |= FILE_FLAG_SEQUENTIAL_SCAN;
        }
        // Senza reparse point si apre il file stesso, mai un collegamento comparso nel frattempo. Con un
        // reparse point che non è un collegamento né cloud (per esempio la deduplicazione) il contenuto lo
        // fornisce il filtro del file system: si apre senza OPEN_REPARSE_POINT, e per l'hash l'identità
        // ricontrollata dice se il file è ancora lo stesso. La compressione WOF non mostra il reparse point
        // negli attributi (misurato il 14/10/2026) e si legge correttamente anche con OPEN_REPARSE_POINT.
        if before.id.attributes & ATTR_REPARSE == 0 {
            flags |= FILE_FLAG_OPEN_REPARSE_POINT;
        }
        let file = open(&w, ACCESS[1], flags).map_err(OpenError::Io)?;
        let opened = Probe::read(&file).map_err(OpenError::Io)?;
        opened.readable()?;
        Ok(Self { file, before: before.id, opened: opened.id })
    }

    /// Identità riletta adesso dall'handle di lettura.
    pub fn current(&self) -> Result<FileIdentity, u32> {
        Probe::read(&self.file).map(|p| p.id)
    }

    /// Lettura sequenziale: byte letti, 0 alla fine del file.
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, u32> {
        let len = buf.len().min(u32::MAX as usize) as u32;
        let mut n = 0u32;
        // SAFETY: handle valido aperto in lettura sincrona; il buffer è valido per `len` byte e `n` è una
        // variabile locale; nessuna struttura OVERLAPPED.
        let ok = unsafe { ReadFile(self.file.0, buf.as_mut_ptr(), len, &mut n, null_mut()) };
        if ok == 0 {
            let e = last_error();
            return if e == ERROR_HANDLE_EOF { Ok(0) } else { Err(e) };
        }
        Ok(n as usize)
    }

    /// Lettura a una posizione: byte letti, 0 oltre la fine del file.
    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize, u32> {
        let len = buf.len().min(u32::MAX as usize) as u32;
        let mut n = 0u32;
        let mut ov = OVERLAPPED {
            Anonymous: OVERLAPPED_0 { Anonymous: OVERLAPPED_0_0 { Offset: offset as u32, OffsetHigh: (offset >> 32) as u32 } },
            ..Default::default()
        };
        // SAFETY: handle valido aperto senza FILE_FLAG_OVERLAPPED: con una OVERLAPPED la lettura resta
        // sincrona e parte dalla posizione indicata. Buffer valido per `len` byte; `n` e `ov` sono locali
        // e vivono fino al ritorno della chiamata.
        let ok = unsafe { ReadFile(self.file.0, buf.as_mut_ptr(), len, &mut n, &mut ov) };
        if ok == 0 {
            let e = last_error();
            return if e == ERROR_HANDLE_EOF { Ok(0) } else { Err(e) };
        }
        Ok(n as usize)
    }
}

/// Codice di Windows di un NTSTATUS di CNG.
fn nt_error(status: NTSTATUS) -> u32 {
    // SAFETY: conversione pura di un codice, nessun prerequisito.
    unsafe { RtlNtStatusToDosError(status) }
}

/// Provider SHA-256 di CNG, chiuso al drop. Si apre a ogni lettura: circa 3 µs in più di un provider
/// riusato, su circa 130 µs per l'hash di un file vuoto (aperture e controlli; misure del 14/10/2026,
/// `measure_provider_cost` e `tests/content.rs`), e così tra una chiamata e l'altra non resta nessuno stato.
struct Algorithm(BCRYPT_ALG_HANDLE);

impl Algorithm {
    fn sha256() -> Result<Self, u32> {
        let mut h: BCRYPT_ALG_HANDLE = null_mut();
        // SAFETY: `h` è una variabile locale; il nome dell'algoritmo è una costante terminata da zero;
        // implementazione predefinita (puntatore nullo), nessun flag.
        let st = unsafe { BCryptOpenAlgorithmProvider(&mut h, BCRYPT_SHA256_ALGORITHM, null(), 0) };
        if st < 0 { Err(nt_error(st)) } else { Ok(Self(h)) }
    }
}

impl Drop for Algorithm {
    fn drop(&mut self) {
        // SAFETY: handle aperto da BCryptOpenAlgorithmProvider, chiuso una sola volta, dopo che tutti gli
        // hash creati da `Hash::new` (che lo prendono in prestito) sono stati distrutti.
        unsafe { BCryptCloseAlgorithmProvider(self.0, 0) };
    }
}

/// Hash SHA-256 in corso, distrutto al drop.
struct Hash<'a> {
    h: BCRYPT_HASH_HANDLE,
    _alg: &'a Algorithm,
}

impl<'a> Hash<'a> {
    fn new(alg: &'a Algorithm) -> Result<Self, u32> {
        let mut h: BCRYPT_HASH_HANDLE = null_mut();
        // SAFETY: provider valido per tutta la vita dell'hash (prestito `'a`); oggetto dell'hash allocato
        // da CNG (puntatore nullo e lunghezza 0, ammessi da Windows 7); nessun segreto.
        let st = unsafe { BCryptCreateHash(alg.0, &mut h, null_mut(), 0, null(), 0, 0) };
        if st < 0 { Err(nt_error(st)) } else { Ok(Self { h, _alg: alg }) }
    }

    fn update(&mut self, data: &[u8]) -> Result<(), u32> {
        for chunk in data.chunks(u32::MAX as usize) {
            // SAFETY: hash valido; il blocco è valido in lettura per `chunk.len()` byte, che sta in un u32.
            let st = unsafe { BCryptHashData(self.h, chunk.as_ptr(), chunk.len() as u32, 0) };
            if st < 0 {
                return Err(nt_error(st));
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<[u8; 32], u32> {
        let mut out = [0u8; 32];
        // SAFETY: hash valido; l'uscita è grande 32 byte, la lunghezza esatta di SHA-256.
        let st = unsafe { BCryptFinishHash(self.h, out.as_mut_ptr(), out.len() as u32, 0) };
        if st < 0 { Err(nt_error(st)) } else { Ok(out) }
    }
}

impl Drop for Hash<'_> {
    fn drop(&mut self) {
        // SAFETY: handle creato da BCryptCreateHash e distrutto una sola volta, qui; il provider è ancora
        // aperto (prestito).
        unsafe { BCryptDestroyHash(self.h) };
    }
}

/// SHA-256 di `data` con CNG, per chi ha già i byte in memoria (e per i test). Errore: codice di Windows.
pub fn sha256_bytes(data: &[u8]) -> Result<[u8; 32], u32> {
    let alg = Algorithm::sha256()?;
    let mut h = Hash::new(&alg)?;
    h.update(data)?;
    h.finish()
}

/// SHA-256 del contenuto di `path`, a blocchi di al più [`BLOCK`] byte.
///
/// Prima legge l'identità con i soli attributi e rifiuta cartelle, collegamenti e segnaposto cloud; poi
/// legge con `GENERIC_READ` e `FILE_FLAG_SEQUENTIAL_SCAN`, condivisione lettura, scrittura ed
/// eliminazione, mai accesso in scrittura. `progress` cresce dei byte letti (si può condividere tra più
/// file); `cancel` si controlla prima di aprire e tra un blocco e l'altro. Se identità, dimensione o data
/// di modifica cambiano tra il controllo e la fine della lettura: `Changed`.
///
/// Restituisce l'identità letta prima della lettura (con la data di ultimo accesso precedente) e l'hash.
/// La lettura aggiorna la data di ultimo accesso del file.
pub fn sha256(path: &str, cancel: &AtomicBool, progress: &AtomicU64) -> Result<(FileIdentity, [u8; 32]), ContentError> {
    if cancel.load(Relaxed) {
        return Err(ContentError::Cancelled);
    }
    let f = ContentFile::open(path, true)?;
    let expected = f.before.size;
    if !same_version(&f.before, &f.opened) {
        return Err(ContentError::Changed);
    }
    let alg = Algorithm::sha256().map_err(ContentError::Io)?;
    let mut hash = Hash::new(&alg).map_err(ContentError::Io)?;
    // Un byte in più della dimensione attesa: un file cresciuto si vede già alla prima lettura.
    let mut buf = vec![0u8; expected.saturating_add(1).clamp(4096, BLOCK as u64) as usize];
    let mut total = 0u64;
    loop {
        if cancel.load(Relaxed) {
            return Err(ContentError::Cancelled);
        }
        let n = f.read(&mut buf).map_err(ContentError::Io)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > expected {
            return Err(ContentError::Changed);
        }
        hash.update(&buf[..n]).map_err(ContentError::Io)?;
        progress.fetch_add(n as u64, Relaxed);
    }
    let after = f.current().map_err(ContentError::Io)?;
    if total != expected || !same_version(&f.before, &after) {
        return Err(ContentError::Changed);
    }
    let digest = hash.finish().map_err(ContentError::Io)?;
    Ok((f.before, digest))
}

/// Stesso file nella stessa versione: identità, dimensione e data di modifica.
fn same_version(a: &FileIdentity, b: &FileIdentity) -> bool {
    a.file_id == b.file_id && a.volume_serial == b.volume_serial && a.size == b.size && a.modified == b.modified
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_definition_matches_the_scan() {
        assert!(is_placeholder(ATTR_RECALL_ON_DATA_ACCESS, 0));
        assert!(is_placeholder(ATTR_RECALL_ON_OPEN, 0));
        assert!(is_placeholder(ATTR_OFFLINE, 0));
        // Tag della Cloud Filter API (IO_REPARSE_TAG_CLOUD e le sue varianti 0x9000x01A).
        assert!(is_placeholder(ATTR_REPARSE, 0x9000_001A));
        assert!(is_placeholder(ATTR_REPARSE, 0x9000_F01A));
        assert!(!is_placeholder(0x20, 0x9000_001A), "il tag conta solo con l'attributo di reparse point");
        // WOF, deduplicazione, symlink: non sono segnaposto.
        assert!(!is_placeholder(ATTR_REPARSE, 0x8000_0017));
        assert!(!is_placeholder(ATTR_REPARSE, 0x8000_0013));
        assert!(!is_placeholder(ATTR_REPARSE, 0xA000_000C));
    }

    #[test]
    fn readable_rejects_directories_links_and_placeholders() {
        let id = FileIdentity { size: 0, created: 0, modified: 0, accessed: 0, file_id: 0, volume_serial: 0, attributes: 0x20 };
        let p = |attributes: u32, reparse_tag: u32, directory: bool| Probe { id: FileIdentity { attributes, ..id }, reparse_tag, directory };
        assert_eq!(p(0x20, 0, false).readable(), Ok(()));
        assert_eq!(p(0x10, 0, true).readable(), Err(OpenError::NotAFile));
        assert_eq!(p(ATTR_REPARSE, 0xA000_000C, false).readable(), Err(OpenError::NotAFile), "symlink");
        assert_eq!(p(ATTR_REPARSE, 0xA000_0003, false).readable(), Err(OpenError::NotAFile), "junction");
        assert_eq!(p(ATTR_REPARSE, 0x8000_0017, false).readable(), Ok(()), "WOF");
        assert_eq!(p(ATTR_REPARSE | ATTR_RECALL_ON_DATA_ACCESS, 0x9000_001A, false).readable(), Err(OpenError::CloudPlaceholder));
        assert_eq!(p(ATTR_OFFLINE, 0, false).readable(), Err(OpenError::CloudPlaceholder));
    }

    #[test]
    fn cng_errors_become_windows_codes() {
        // STATUS_NO_MEMORY e STATUS_INVALID_PARAMETER.
        assert_eq!(nt_error(0xC000_0017_u32 as i32), 8);
        assert_eq!(nt_error(0xC000_000D_u32 as i32), 87);
    }

    #[test]
    fn cng_known_vectors() {
        let hex = |d: [u8; 32]| d.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(sha256_bytes(b"").unwrap()), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(sha256_bytes(b"abc").unwrap()), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    /// Misura il costo del provider CNG: aperto a ogni hash o riusato (per esempio uno per thread).
    #[test]
    #[ignore = "misura, non verifica"]
    fn measure_provider_cost() {
        let n = 2000;
        let t = std::time::Instant::now();
        for _ in 0..n {
            sha256_bytes(b"abc").unwrap();
        }
        let per_call = t.elapsed() / n;
        let alg = Algorithm::sha256().unwrap();
        let t = std::time::Instant::now();
        for _ in 0..n {
            let mut h = Hash::new(&alg).unwrap();
            h.update(b"abc").unwrap();
            h.finish().unwrap();
        }
        let reused = t.elapsed() / n;
        eprintln!("provider aperto a ogni hash: {per_call:?}; provider riusato: {reused:?}");
    }
}
