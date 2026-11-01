// SPDX-License-Identifier: GPL-3.0-or-later
//! Attributi completi di un file o di una cartella senza aprirne il contenuto (03 §6.3, 05 D7 controllo
//! 7): `FileBasicInfo`, `FileStandardInfo`, `FileIdInfo` e il tag di reparse, con un handle che chiede
//! solo `FILE_READ_ATTRIBUTES`, `FILE_FLAG_OPEN_REPARSE_POINT` e `FILE_FLAG_OPEN_NO_RECALL`. Servono alla
//! simulazione e al ricontrollo prima dello spostamento (`quado-actions`).
//!
//! L'handle è lo stesso di [`win::open_attributes`]: nessun bit di scrittura, i collegamenti non si
//! seguono, i segnaposto cloud non si scaricano e la data di ultimo accesso non cambia. Funziona anche
//! sulle cartelle (`FILE_FLAG_BACKUP_SEMANTICS`) e oltre i 260 caratteri (`\\?\`, [`win::long_path`]).
//! Codici di Windows, mai frasi (05 D13).
//!
//! Il numero di nomi (`NumberOfLinks`) non c'è nell'elenco delle cartelle (`FileIdExtdDirectoryInfo`), e
//! data e attributi dell'elenco possono essere vecchi per un file con più nomi o aperto in scrittura: per
//! questo ogni voce si apre comunque, una volta. Aprirla rispetto all'handle della cartella (`NtOpenFile`
//! con `RootDirectory`) o chiederla per nome (`NtQueryInformationByName`) costa quanto `CreateFileW` con il
//! percorso intero: 155-220 µs per voce su questo PC (`cargo xtask attributi-fasi --aperture`, 26/10/2026),
//! il tempo è dell'apertura nel kernel e nei filtri, non del percorso.

use std::mem::{MaybeUninit, size_of};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_DIRECTORY, ERROR_INVALID_FUNCTION, ERROR_INVALID_NAME, ERROR_INVALID_PARAMETER, ERROR_NOT_SUPPORTED, GetLastError,
    HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_TAG_INFO, FILE_BASIC_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_NO_RECALL, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_ID_INFO, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO, FileAttributeTagInfo,
    FileBasicInfo, FileIdInfo, FileStandardInfo, GetFileInformationByHandleEx, OPEN_EXISTING, SYNCHRONIZE,
};

use crate::{content, win};

/// Accesso richiesto da [`read`]: lo stesso di [`win::open_attributes`] (uno dei valori di
/// [`win::READ_ONLY_ACCESS`]), per il controllo dell'assenza di scrittura (06 §11).
pub const ACCESS: u32 = FILE_READ_ATTRIBUTES | SYNCHRONIZE;

const _: () = assert!(ACCESS & win::WRITE_BITS == 0);
const _: () = assert!(ACCESS == win::READ_ONLY_ACCESS[1]);

const ATTR_REPARSE: u32 = 0x0400;
const ATTR_DIRECTORY: u32 = 0x0010;
/// Bit "name surrogate" del tag: symlink, junction, punto di montaggio (il reparse point è un altro nome
/// per qualcosa che sta altrove).
const TAG_NAME_SURROGATE: u32 = 0x2000_0000;

/// Attributi di un file o di una cartella, letti dall'handle senza aprire il contenuto.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FullAttrs {
    /// Dimensione logica (`FileStandardInfo.EndOfFile`).
    pub logical: u64,
    /// Dimensione su disco (`FileStandardInfo.AllocationSize`).
    pub alloc: u64,
    /// Numero di nomi (`FileStandardInfo.NumberOfLinks`).
    pub links: u32,
    /// Identità a 128 bit (`FileIdInfo`); 0 se il file system non la dà (exFAT, FAT32).
    pub file_id: u128,
    /// Numero di serie del volume (`FileIdInfo`); 0 come `file_id`.
    pub volume_serial: u64,
    /// Data di modifica (`FileBasicInfo.LastWriteTime`, `FILETIME`).
    pub modified: i64,
    /// `FILE_ATTRIBUTE_*` (`FileBasicInfo.FileAttributes`).
    pub attributes: u32,
    pub directory: bool,
    /// Tag del reparse point (`FileAttributeTagInfo`); 0 se non è un reparse point.
    pub reparse_tag: u32,
}

impl FullAttrs {
    /// Collegamento (symlink, junction, punto di montaggio): reparse point con il tag "name surrogate".
    /// Come la scansione, che non li attraversa (`Flags::NOT_FOLLOWED`).
    pub fn link(&self) -> bool {
        self.attributes & ATTR_REPARSE != 0 && self.reparse_tag & TAG_NAME_SURROGATE != 0
    }

    /// Segnaposto cloud, con la definizione della scansione e delle letture del contenuto: offline,
    /// richiamo all'apertura o all'accesso ai dati, oppure reparse point della Cloud Filter API.
    pub fn placeholder(&self) -> bool {
        content::is_placeholder(self.attributes, self.reparse_tag)
    }
}

/// Handle chiuso al drop.
struct Handle(HANDLE);

impl Drop for Handle {
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

fn open(path: &[u16]) -> Result<Handle, u32> {
    // Un carattere nullo troncherebbe il percorso: si leggerebbe un altro file.
    if path.contains(&0) {
        return Err(ERROR_INVALID_NAME);
    }
    let z = win::wide_z(&win::long_path(path));
    // SAFETY: `z` è terminata da zero e vive per tutta la chiamata; gli altri argomenti sono costanti o
    // puntatori nulli ammessi. L'accesso è `ACCESS`, senza bit di scrittura.
    let h = unsafe {
        CreateFileW(
            z.as_ptr(),
            ACCESS,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_OPEN_NO_RECALL,
            null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE { Err(last_error()) } else { Ok(Handle(h)) }
}

fn query<T>(h: &Handle, class: i32) -> Result<T, u32> {
    let mut out = MaybeUninit::<T>::zeroed();
    // SAFETY: il buffer è grande `size_of::<T>()` e allineato per `T`, che per le classi usate qui
    // (FileBasicInfo, FileStandardInfo, FileIdInfo, FileAttributeTagInfo) è la struttura documentata a
    // dimensione fissa; l'handle è valido per tutta la chiamata.
    let ok = unsafe { GetFileInformationByHandleEx(h.0, class, out.as_mut_ptr().cast(), size_of::<T>() as u32) };
    if ok == 0 {
        return Err(last_error());
    }
    // SAFETY: la chiamata è riuscita e ha scritto la struttura; partiva comunque azzerata.
    Ok(unsafe { out.assume_init() })
}

/// Attributi di `path` (percorso assoluto, con o senza `\\?\`), file o cartella, senza seguire i
/// collegamenti. Errore: codice di Windows (2 file non trovato, 3 percorso non trovato, 5 accesso negato,
/// 123 nome non valido, anche per un carattere nullo).
pub fn read(path: &str) -> Result<FullAttrs, u32> {
    let w: Vec<u16> = path.encode_utf16().collect();
    let h = open(&w)?;
    let basic: FILE_BASIC_INFO = query(&h, FileBasicInfo)?;
    let standard: FILE_STANDARD_INFO = query(&h, FileStandardInfo)?;
    let (volume_serial, file_id) = match query::<FILE_ID_INFO>(&h, FileIdInfo) {
        Ok(i) => (i.VolumeSerialNumber, u128::from_le_bytes(i.FileId.Identifier)),
        // File system senza FileIdInfo: identità 0, come la scansione su exFAT e FAT32.
        Err(ERROR_INVALID_PARAMETER | ERROR_INVALID_FUNCTION | ERROR_NOT_SUPPORTED) => (0, 0),
        Err(e) => return Err(e),
    };
    let reparse_tag =
        if basic.FileAttributes & ATTR_REPARSE != 0 { query::<FILE_ATTRIBUTE_TAG_INFO>(&h, FileAttributeTagInfo)?.ReparseTag } else { 0 };
    Ok(FullAttrs {
        logical: standard.EndOfFile.max(0) as u64,
        alloc: standard.AllocationSize.max(0) as u64,
        links: standard.NumberOfLinks,
        file_id,
        volume_serial,
        modified: basic.LastWriteTime,
        attributes: basic.FileAttributes,
        directory: standard.Directory,
        reparse_tag,
    })
}

/// Nomi dei figli diretti di `dir`, senza `.` e `..`, con l'enumerazione della scansione
/// ([`win::open_dir`] e [`win::enumerate`]): non apre i figli, non scarica i segnaposto, regge exFAT e
/// FAT32. Prima si leggono gli attributi con [`read`]: un file non si apre con l'accesso dell'elenco (che
/// per un file vuol dire lettura dei dati) e dà `Err(267)` (`ERROR_DIRECTORY`). Un nome che non è UTF-16
/// valido (surrogato spaiato) non si può restituire come `String` senza cambiarlo, e il nome cambiato
/// indicherebbe un altro file: l'elenco intero dà `Err(123)` (`ERROR_INVALID_NAME`), come un elenco che
/// non si conosce. Altri errori: codice di Windows (5 per una cartella negata).
pub fn list(dir: &str) -> Result<Vec<String>, u32> {
    if !read(dir)?.directory {
        return Err(ERROR_DIRECTORY);
    }
    let w: Vec<u16> = dir.encode_utf16().collect();
    let h = win::open_dir(&w)?;
    names(&h)
}

/// Come `read(dir)?.link()` seguito da [`list`], con una sola apertura invece di tre: per chi ha appena
/// letto gli attributi di `dir` con [`read`] e sa che è una cartella e non un collegamento (la visita
/// della simulazione, `WinProbe::entries`). Si apre con l'handle dell'elenco ([`win::open_dir`]) e si
/// controllano subito gli attributi dall'handle (`FileAttributeTagInfo`), prima di elencare: un
/// collegamento non ha voci (`Ok` vuoto, come `WinProbe::children`); un file comparso nel frattempo al
/// posto della cartella dà `Err(267)` come [`list`], senza leggere niente (il controllo di [`list`] prima
/// dell'apertura non chiude questa finestra: la sposta). Stessi codici di errore di [`list`].
pub fn list_dir(dir: &str) -> Result<Vec<String>, u32> {
    let w: Vec<u16> = dir.encode_utf16().collect();
    // Un carattere nullo troncherebbe il percorso: si elencherebbe un'altra cartella.
    if w.contains(&0) {
        return Err(ERROR_INVALID_NAME);
    }
    let h = win::open_dir(&w)?;
    let (attributes, tag) = win::attribute_tag(&h)?;
    if attributes & ATTR_REPARSE != 0 && tag & TAG_NAME_SURROGATE != 0 {
        return Ok(Vec::new());
    }
    if attributes & ATTR_DIRECTORY == 0 {
        return Err(ERROR_DIRECTORY);
    }
    names(&h)
}

/// Nomi delle voci di una cartella aperta per l'elenco; un nome che non è UTF-16 valido dà `Err(123)`.
fn names(h: &win::Handle) -> Result<Vec<String>, u32> {
    let mut buf = win::EnumBuffer::default();
    let mut names = Vec::new();
    let mut invalid = false;
    win::enumerate(h, &mut buf, |e| match String::from_utf16(e.name) {
        Ok(n) => names.push(n),
        Err(_) => invalid = true,
    })?;
    if invalid { Err(ERROR_INVALID_NAME) } else { Ok(names) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_and_placeholders() {
        let a = |attributes: u32, reparse_tag: u32| FullAttrs { attributes, reparse_tag, ..Default::default() };
        assert!(a(ATTR_REPARSE | 0x10, 0xA000_0003).link(), "junction");
        assert!(a(ATTR_REPARSE, 0xA000_000C).link(), "symlink");
        assert!(!a(ATTR_REPARSE, 0x8000_0017).link(), "WOF");
        assert!(!a(ATTR_REPARSE, 0x9000_001A).link(), "cloud");
        assert!(!a(0x20, 0xA000_000C).link(), "il tag conta solo con l'attributo di reparse point");
        assert!(a(ATTR_REPARSE, 0x9000_001A).placeholder());
        assert!(a(0x0040_0000, 0).placeholder());
        assert!(!a(ATTR_REPARSE, 0xA000_000C).placeholder());
    }

    #[test]
    fn nul_is_rejected() {
        assert_eq!(read("C:\\Windows\0\\x"), Err(ERROR_INVALID_NAME));
        assert_eq!(list("C:\\Windows\0"), Err(ERROR_INVALID_NAME));
        assert_eq!(list_dir("C:\\Windows\0"), Err(ERROR_INVALID_NAME));
        assert_eq!(list_dir("C:\\Windows\0\\System32"), Err(ERROR_INVALID_NAME));
    }

    #[test]
    fn list_dir_lists_like_list() {
        let d = r"C:\Windows\System32";
        assert_eq!(list_dir(d), list(d));
        assert_eq!(list_dir(&format!(r"\\?\{d}")), list(d));
        assert_eq!(list_dir(r"C:\Windows\System32\kernel32.dll"), Err(ERROR_DIRECTORY));
        assert!(matches!(list_dir(r"C:\Windows\nessuna-cartella-così"), Err(2 | 3)));
    }

    #[test]
    fn the_handle_has_no_write_access() {
        use crate::process::own_handles;
        let w: Vec<u16> = r"C:\Windows\System32\kernel32.dll".encode_utf16().collect();
        let h = open(&w).unwrap();
        let handle = own_handles().into_iter().find(|o| o.value == h.0 as usize).expect("handle visibile");
        assert_eq!(handle.granted_access & win::WRITE_BITS, 0, "accesso {:#x}", handle.granted_access);
        assert_eq!(handle.granted_access, ACCESS, "accesso {:#x}", handle.granted_access);
    }

    #[test]
    fn reads_a_directory_and_a_file() {
        let d = read(r"C:\Windows").unwrap();
        assert!(d.directory && d.file_id != 0 && d.volume_serial != 0);
        let f = read(r"C:\Windows\System32\kernel32.dll").unwrap();
        assert!(!f.directory && f.logical > 0 && f.alloc > 0 && f.links >= 1 && f.modified > 0);
        let names = list(r"C:\Windows\System32").unwrap();
        assert!(names.iter().any(|n| n.eq_ignore_ascii_case("kernel32.dll")));
        assert!(!names.iter().any(|n| n == "." || n == ".."));
        assert_eq!(list(r"C:\Windows\System32\kernel32.dll"), Err(ERROR_DIRECTORY));
    }
}
