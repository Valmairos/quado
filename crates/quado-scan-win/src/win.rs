// SPDX-License-Identifier: GPL-3.0-or-later
//! Chiamate Win32 di sola lettura, avvolte in funzioni sicure.
//!
//! Regola del crate: nessun handle con accesso in scrittura, nessuna lettura del
//! contenuto dei file. Le cartelle si aprono con `FILE_LIST_DIRECTORY`, i file solo con
//! `FILE_READ_ATTRIBUTES`, sempre con `FILE_FLAG_OPEN_REPARSE_POINT` (non si seguono i collegamenti)
//! e `FILE_FLAG_OPEN_NO_RECALL` (non si scaricano i file dal cloud).

use std::mem::{MaybeUninit, size_of};
use std::ptr::{null, null_mut};

use quado_model::Space;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_HANDLE_EOF, ERROR_NO_MORE_FILES, GENERIC_READ, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_NO_RECALL, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_ATTRIBUTE_TAG_INFO, FILE_ID_EXTD_DIR_INFO, FILE_ID_INFO, FILE_LIST_DIRECTORY, FileAttributeTagInfo, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, FILE_STANDARD_INFO, FileIdExtdDirectoryInfo, FileIdExtdDirectoryRestartInfo, FileIdInfo, FileStandardInfo,
    DISK_SPACE_INFORMATION, GetDiskFreeSpaceExW, GetDiskSpaceInformationW, GetFileInformationByHandleEx, OPEN_EXISTING,
    SYNCHRONIZE,
};
use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::{
    FSCTL_GET_NTFS_VOLUME_DATA, FSCTL_QUERY_USN_JOURNAL, NTFS_VOLUME_DATA_BUFFER, USN_JOURNAL_DATA_V0,
};

/// Stringa UTF-16 terminata da zero.
pub fn wide_z(s: &[u16]) -> Vec<u16> {
    let mut v = Vec::with_capacity(s.len() + 1);
    v.extend_from_slice(s);
    v.push(0);
    v
}

/// Percorso per le API: aggiunge `\\?\` ai percorsi assoluti con lettera, per superare i 260 caratteri.
pub fn long_path(p: &[u16]) -> Vec<u16> {
    let prefix: Vec<u16> = r"\\?\".encode_utf16().collect();
    if p.starts_with(&prefix) || p.starts_with(&[b'\\' as u16, b'\\' as u16]) {
        return p.to_vec();
    }
    let mut v = prefix;
    v.extend_from_slice(p);
    v
}

/// `parent\name`, senza raddoppiare la barra.
pub fn join(parent: &[u16], name: &[u16]) -> Vec<u16> {
    let mut v = Vec::with_capacity(parent.len() + name.len() + 1);
    v.extend_from_slice(parent);
    if v.last() != Some(&(b'\\' as u16)) {
        v.push(b'\\' as u16);
    }
    v.extend_from_slice(name);
    v
}

/// Handle chiuso al drop.
pub struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: l'handle è valido (costruito solo da `open_*` dopo il controllo di
        // INVALID_HANDLE_VALUE) e viene chiuso una sola volta, qui.
        unsafe { CloseHandle(self.0) };
    }
}

fn last_error() -> u32 {
    // SAFETY: nessun prerequisito.
    unsafe { GetLastError() }
}

fn open(path: &[u16], access: u32) -> Result<Handle, u32> {
    let z = wide_z(&long_path(path));
    // SAFETY: `z` è terminata da zero e vive per tutta la chiamata; gli altri argomenti sono costanti
    // o puntatori nulli ammessi. L'accesso richiesto non comprende mai la scrittura.
    let h = unsafe {
        CreateFileW(
            z.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_OPEN_NO_RECALL,
            null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE { Err(last_error()) } else { Ok(Handle(h)) }
}

/// Apre una cartella per elencarla.
pub fn open_dir(path: &[u16]) -> Result<Handle, u32> {
    open(path, FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
}

/// Apre un file o una cartella per leggerne solo gli attributi.
pub fn open_attributes(path: &[u16]) -> Result<Handle, u32> {
    open(path, FILE_READ_ATTRIBUTES | SYNCHRONIZE)
}

/// Accessi richiesti da `open_dir` e `open_attributes`: usati dal test che verifica l'assenza di scrittura.
pub const READ_ONLY_ACCESS: [u32; 2] = [FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE, FILE_READ_ATTRIBUTES | SYNCHRONIZE];

/// Bit di accesso che non devono mai comparire.
pub const WRITE_BITS: u32 = 0x0002 /* FILE_WRITE_DATA */
    | 0x0004 /* FILE_APPEND_DATA */
    | 0x0010 /* FILE_WRITE_EA */
    | 0x0100 /* FILE_WRITE_ATTRIBUTES */
    | 0x0001_0000 /* DELETE */
    | 0x0004_0000 /* WRITE_DAC */
    | 0x0008_0000 /* WRITE_OWNER */
    | 0x4000_0000 /* GENERIC_WRITE */
    | 0x1000_0000 /* GENERIC_ALL */;

const _: () = assert!(GENERIC_READ & WRITE_BITS == 0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StandardInfo {
    pub alloc: u64,
    pub logical: u64,
    pub links: u32,
    pub directory: bool,
}

fn query<T>(h: &Handle, class: i32) -> Result<T, u32> {
    let mut out = MaybeUninit::<T>::zeroed();
    // SAFETY: il buffer è grande `size_of::<T>()` e allineato per `T`, che per le classi usate
    // (FileStandardInfo, FileIdInfo, FileAttributeTagInfo) è la struttura documentata a dimensione fissa.
    let ok = unsafe { GetFileInformationByHandleEx(h.0, class, out.as_mut_ptr().cast(), size_of::<T>() as u32) };
    if ok == 0 {
        return Err(last_error());
    }
    // SAFETY: la chiamata è riuscita e ha scritto la struttura; partiva comunque azzerata.
    Ok(unsafe { out.assume_init() })
}

pub fn standard_info(h: &Handle) -> Result<StandardInfo, u32> {
    let s: FILE_STANDARD_INFO = query(h, FileStandardInfo)?;
    Ok(StandardInfo {
        alloc: s.AllocationSize.max(0) as u64,
        logical: s.EndOfFile.max(0) as u64,
        links: s.NumberOfLinks,
        directory: s.Directory,
    })
}

/// Attributi (`FILE_ATTRIBUTE_*`) e tag di reparse (0 se non è un reparse point) dall'handle, con una
/// sola domanda (`FileAttributeTagInfo`).
pub fn attribute_tag(h: &Handle) -> Result<(u32, u32), u32> {
    let s: FILE_ATTRIBUTE_TAG_INFO = query(h, FileAttributeTagInfo)?;
    Ok((s.FileAttributes, s.ReparseTag))
}

/// Identità a 128 bit del file (`FileIdInfo`) e numero di serie del volume.
pub fn id_info(h: &Handle) -> Result<(u64, u128), u32> {
    let s: FILE_ID_INFO = query(h, FileIdInfo)?;
    Ok((s.VolumeSerialNumber, u128::from_le_bytes(s.FileId.Identifier)))
}

/// Una voce di cartella, come la restituisce `FileIdExtdDirectoryInfo`.
#[derive(Clone, Copy, Debug)]
pub struct DirEntry<'a> {
    pub name: &'a [u16],
    pub attrs: u32,
    pub reparse_tag: u32,
    pub logical: u64,
    pub alloc: u64,
    pub mtime: i64,
    pub file_id: u128,
}

impl DirEntry<'_> {
    pub fn is_dir(&self) -> bool {
        self.attrs & FILE_ATTRIBUTE_DIRECTORY != 0
    }
}

/// Buffer da 64 KiB allineato a 8 byte per l'enumerazione.
pub struct EnumBuffer(Vec<u64>);

impl Default for EnumBuffer {
    fn default() -> Self {
        Self(vec![0; 64 * 1024 / 8])
    }
}

/// Elenca una cartella aperta con [`open_dir`]. Restituisce `Ok(())` alla fine dell'elenco, oppure
/// l'errore se l'elenco si interrompe (le voci già passate a `f` restano valide).
///
/// exFAT e FAT32 non supportano `FileIdExtdDirectoryInfo` (errore 87, misurato sui VHDX di prova): lì si usa
/// `FileFullDirectoryInfo`, che non ha l'identità dei file. Quei file system non hanno hard link,
/// quindi l'identità resta 0 e la deduplica li ignora.
pub fn enumerate(h: &Handle, buf: &mut EnumBuffer, mut f: impl FnMut(DirEntry<'_>)) -> Result<(), u32> {
    let mut emitted = false;
    match enumerate_extd(h, buf, |e| {
        emitted = true;
        f(e)
    }) {
        Err(87) if !emitted => enumerate_full(h, buf, f),
        other => other,
    }
}

fn enumerate_full(h: &Handle, buf: &mut EnumBuffer, mut f: impl FnMut(DirEntry<'_>)) -> Result<(), u32> {
    use windows_sys::Win32::Storage::FileSystem::{FILE_FULL_DIR_INFO, FileFullDirectoryInfo, FileFullDirectoryRestartInfo};
    let mut first = true;
    loop {
        let class = if first { FileFullDirectoryRestartInfo } else { FileFullDirectoryInfo };
        first = false;
        let bytes = (buf.0.len() * 8) as u32;
        // SAFETY: come in `enumerate_extd`, con la catena di FILE_FULL_DIR_INFO.
        let ok = unsafe { GetFileInformationByHandleEx(h.0, class, buf.0.as_mut_ptr().cast(), bytes) };
        if ok == 0 {
            let e = last_error();
            return if e == ERROR_NO_MORE_FILES || e == ERROR_HANDLE_EOF { Ok(()) } else { Err(e) };
        }
        let base = buf.0.as_ptr().cast::<u8>();
        let mut off = 0usize;
        loop {
            // SAFETY: catena di FILE_FULL_DIR_INFO scritta dal sistema nel buffer; `off` segue
            // `NextEntryOffset`; il nome è lungo `FileNameLength` byte dentro il buffer.
            let (info, name) = unsafe {
                let info = &*base.add(off).cast::<FILE_FULL_DIR_INFO>();
                let name = std::slice::from_raw_parts(info.FileName.as_ptr(), (info.FileNameLength / 2) as usize);
                (info, name)
            };
            let dot = name == [b'.' as u16] || name == [b'.' as u16, b'.' as u16];
            if !dot {
                let reparse = info.FileAttributes & 0x400 != 0;
                f(DirEntry {
                    name,
                    attrs: info.FileAttributes,
                    reparse_tag: if reparse { info.EaSize } else { 0 },
                    logical: info.EndOfFile.max(0) as u64,
                    alloc: info.AllocationSize.max(0) as u64,
                    mtime: info.LastWriteTime,
                    file_id: 0,
                });
            }
            if info.NextEntryOffset == 0 {
                break;
            }
            off += info.NextEntryOffset as usize;
        }
    }
}

fn enumerate_extd(h: &Handle, buf: &mut EnumBuffer, mut f: impl FnMut(DirEntry<'_>)) -> Result<(), u32> {
    let mut first = true;
    loop {
        let class = if first { FileIdExtdDirectoryRestartInfo } else { FileIdExtdDirectoryInfo };
        first = false;
        let bytes = (buf.0.len() * 8) as u32;
        // SAFETY: il buffer è valido per `bytes` byte, allineato a 8 come richiesto dalle strutture
        // FILE_ID_EXTD_DIR_INFO, e resta in vita durante la chiamata.
        let ok = unsafe { GetFileInformationByHandleEx(h.0, class, buf.0.as_mut_ptr().cast(), bytes) };
        if ok == 0 {
            let e = last_error();
            return if e == ERROR_NO_MORE_FILES || e == ERROR_HANDLE_EOF { Ok(()) } else { Err(e) };
        }
        let base = buf.0.as_ptr().cast::<u8>();
        let mut off = 0usize;
        loop {
            // SAFETY: il sistema ha scritto una catena di FILE_ID_EXTD_DIR_INFO dentro il buffer;
            // `off` segue `NextEntryOffset`, che resta nei limiti ed è allineato a 8 per contratto
            // dell'API. Il nome è lungo `FileNameLength` byte a partire da `FileName`, dentro il buffer.
            let (info, name) = unsafe {
                let info = &*base.add(off).cast::<FILE_ID_EXTD_DIR_INFO>();
                let name = std::slice::from_raw_parts(info.FileName.as_ptr(), (info.FileNameLength / 2) as usize);
                (info, name)
            };
            let dot = name == [b'.' as u16] || name == [b'.' as u16, b'.' as u16];
            if !dot {
                f(DirEntry {
                    name,
                    attrs: info.FileAttributes,
                    reparse_tag: info.ReparsePointTag,
                    logical: info.EndOfFile.max(0) as u64,
                    alloc: info.AllocationSize.max(0) as u64,
                    mtime: info.LastWriteTime,
                    file_id: u128::from_le_bytes(info.FileId.Identifier),
                });
            }
            if info.NextEntryOffset == 0 {
                break;
            }
            off += info.NextEntryOffset as usize;
            debug_assert!(off < bytes as usize);
        }
    }
}

/// Dati NTFS del volume dall'handle di un qualunque file o cartella (senza privilegi).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NtfsVolumeData {
    pub serial: u64,
    pub bytes_per_cluster: u32,
    pub bytes_per_record: u32,
    pub mft_valid_data_length: u64,
}

impl NtfsVolumeData {
    /// Record della MFT: limite superiore dei nomi da aspettarsi (stima per riservare memoria).
    pub fn mft_records(&self) -> u64 {
        if self.bytes_per_record == 0 { 0 } else { self.mft_valid_data_length / u64::from(self.bytes_per_record) }
    }
}

fn ioctl_out<T>(h: &Handle, code: u32) -> Result<T, u32> {
    let mut out = MaybeUninit::<T>::zeroed();
    let mut returned = 0u32;
    // SAFETY: nessun buffer d'ingresso; il buffer d'uscita è grande `size_of::<T>()` e resta in vita
    // durante la chiamata sincrona (overlapped nullo). I codici usati sono di sola lettura.
    let ok = unsafe { DeviceIoControl(h.0, code, null(), 0, out.as_mut_ptr().cast(), size_of::<T>() as u32, &mut returned, null_mut()) };
    if ok == 0 {
        return Err(last_error());
    }
    // SAFETY: la chiamata è riuscita; la struttura partiva azzerata.
    Ok(unsafe { out.assume_init() })
}

pub fn ntfs_volume_data(h: &Handle) -> Result<NtfsVolumeData, u32> {
    let d: NTFS_VOLUME_DATA_BUFFER = ioctl_out(h, FSCTL_GET_NTFS_VOLUME_DATA)?;
    Ok(NtfsVolumeData {
        serial: d.VolumeSerialNumber as u64,
        bytes_per_cluster: d.BytesPerCluster,
        bytes_per_record: d.BytesPerFileRecordSegment,
        mft_valid_data_length: d.MftValidDataLength.max(0) as u64,
    })
}

/// Journal USN: intervallo attivo `NextUsn - FirstUsn` e dimensione massima configurata.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UsnJournal {
    pub active_bytes: u64,
    pub maximum_size: u64,
}

pub fn usn_journal(h: &Handle) -> Result<UsnJournal, u32> {
    let d: USN_JOURNAL_DATA_V0 = ioctl_out(h, FSCTL_QUERY_USN_JOURNAL)?;
    Ok(UsnJournal { active_bytes: (d.NextUsn - d.FirstUsn).max(0) as u64, maximum_size: d.MaximumSize })
}

/// Spazio del volume che contiene `path`.
pub fn disk_space(path: &[u16]) -> Result<Space, u32> {
    let z = wide_z(path);
    let (mut caller, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: `z` è terminata da zero; i tre puntatori sono a variabili locali valide.
    let ok = unsafe { GetDiskFreeSpaceExW(z.as_ptr(), &mut caller, &mut total, &mut free) };
    if ok == 0 {
        return Err(last_error());
    }
    Ok(Space { capacity: total, free, free_to_caller: caller })
}

/// Spazio riservato e non usato del volume (`TotalReservedAllocationUnits`), archiviazione riservata
/// compresa. Non richiede privilegi, a differenza di `fsutil storagereserve`: su C: la
/// parte dell'archiviazione riservata coincide con la garanzia letta da `fsutil` (5.605 MB sul PC di sviluppo).
/// `path` è la radice del volume (`X:\`).
pub fn reserved_space(path: &[u16]) -> Result<u64, u32> {
    let z = wide_z(path);
    let mut info = MaybeUninit::<DISK_SPACE_INFORMATION>::zeroed();
    // SAFETY: `z` è terminata da zero; `info` è un buffer della dimensione della struttura, scritto
    // dalla funzione solo se restituisce S_OK.
    let hr = unsafe { GetDiskSpaceInformationW(z.as_ptr(), info.as_mut_ptr()) };
    if hr < 0 {
        return Err(hr as u32);
    }
    // SAFETY: S_OK, quindi la struttura è stata riempita (ed era comunque azzerata).
    let i = unsafe { info.assume_init() };
    let cluster = u64::from(i.SectorsPerAllocationUnit) * u64::from(i.BytesPerSector);
    Ok(i.TotalReservedAllocationUnits.saturating_mul(cluster))
}

/// Mostra i segnaposto cloud come tali: senza, Windows li presenta come file normali
/// ad alcuni processi. Vale per tutto il processo; si chiama prima di scansionare.
pub fn expose_placeholders() {
    const PHCM_EXPOSE_PLACEHOLDERS: i8 = 2;
    // SAFETY: nessun prerequisito; cambia solo una modalità del processo corrente.
    unsafe { windows_sys::Wdk::Storage::FileSystem::RtlSetProcessPlaceholderCompatibilityMode(PHCM_EXPOSE_PLACEHOLDERS) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn paths() {
        assert_eq!(String::from_utf16(&long_path(&w(r"C:\a"))).unwrap(), r"\\?\C:\a");
        assert_eq!(String::from_utf16(&long_path(&w(r"\\?\C:\a"))).unwrap(), r"\\?\C:\a");
        assert_eq!(String::from_utf16(&join(&w(r"C:\"), &w("x"))).unwrap(), r"C:\x");
        assert_eq!(String::from_utf16(&join(&w(r"C:\a"), &w("x"))).unwrap(), r"C:\a\x");
    }

    #[test]
    fn read_only_access_has_no_write_bits() {
        for a in READ_ONLY_ACCESS {
            assert_eq!(a & WRITE_BITS, 0, "{a:#x}");
        }
    }

    #[test]
    fn reads_the_system_volume_without_privileges() {
        let root = w(r"C:\");
        let h = open_dir(&root).expect("radice di C:");
        let nv = ntfs_volume_data(&h).expect("FSCTL_GET_NTFS_VOLUME_DATA senza privilegi");
        assert!(nv.mft_valid_data_length > 0 && nv.bytes_per_cluster > 0);
        let mut n = 0;
        enumerate(&h, &mut EnumBuffer::default(), |_| n += 1).unwrap();
        assert!(n > 3);
        let s = disk_space(&root).unwrap();
        assert!(s.capacity > s.free);
    }
}

/// Metriche del processo corrente, per le misure delle prestazioni.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessMetrics {
    pub peak_working_set: u64,
    pub working_set: u64,
    /// Memoria privata impegnata (`PagefileUsage`).
    pub private: u64,
    pub user: std::time::Duration,
    pub kernel: std::time::Duration,
    pub handles: u32,
}

pub fn process_metrics() -> ProcessMetrics {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount, GetProcessTimes};
    let mut m = ProcessMetrics::default();
    // SAFETY: lo pseudo-handle del processo corrente è sempre valido e non va chiuso; le strutture
    // d'uscita sono variabili locali della dimensione dichiarata.
    unsafe {
        let p = GetCurrentProcess();
        let mut pmc: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        pmc.cb = size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(p, &mut pmc, pmc.cb) != 0 {
            m.peak_working_set = pmc.PeakWorkingSetSize as u64;
            m.working_set = pmc.WorkingSetSize as u64;
            m.private = pmc.PagefileUsage as u64;
        }
        let mut t: [FILETIME; 4] = std::mem::zeroed();
        if GetProcessTimes(p, &mut t[0], &mut t[1], &mut t[2], &mut t[3]) != 0 {
            let d = |f: FILETIME| std::time::Duration::from_nanos(((u64::from(f.dwHighDateTime) << 32) | u64::from(f.dwLowDateTime)) * 100);
            m.kernel = d(t[2]);
            m.user = d(t[3]);
        }
        let mut h = 0u32;
        if GetProcessHandleCount(p, &mut h) != 0 {
            m.handles = h;
        }
    }
    m
}
