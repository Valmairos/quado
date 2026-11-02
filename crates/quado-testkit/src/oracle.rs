// SPDX-License-Identifier: GPL-3.0-or-later
//! Oracolo indipendente: un secondo percorso di misura, usato solo nei test.
//!
//! Elenca le cartelle con `FindFirstFileW` (attraverso `std::fs::read_dir`), non con
//! `FileIdExtdDirectoryInfo`, e apre ogni file con soli attributi per leggere `FileStandardInfo`
//! (allocata, logica, numero di link) e `FileIdInfo` (identità). Le regole sono le stesse dello
//! scanner: niente reparse con il bit "name surrogate", niente cartelle cloud `RECALL_ON_DATA_ACCESS`,
//! ogni identità contata una volta.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::mem::{MaybeUninit, size_of};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_NO_RECALL, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_ID_INFO, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO,
    FileAttributeTagInfo, FileIdInfo, FileStandardInfo, GetFileInformationByHandleEx, OPEN_EXISTING, SYNCHRONIZE,
};

const ATTR_DIRECTORY: u32 = 0x10;
const ATTR_REPARSE: u32 = 0x400;
const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OracleFile {
    pub logical: u64,
    pub alloc: u64,
    pub links: u32,
    pub id: u128,
}

#[derive(Clone, Debug, Default)]
pub struct Oracle {
    /// File per percorso completo esatto (UTF-16).
    pub files: HashMap<Vec<u16>, OracleFile>,
    pub dirs: u64,
    /// Somma delle allocate degli indici delle cartelle elencate.
    pub dir_index_alloc: u64,
    /// Ogni identità contata una volta.
    pub unique_alloc: u64,
    pub unique_logical: u64,
    /// Cartelle che non si sono potute elencare.
    pub unreadable: Vec<PathBuf>,
    /// Reparse con "name surrogate" non seguiti.
    pub not_followed: u64,
}

struct H(HANDLE);

impl Drop for H {
    fn drop(&mut self) {
        // SAFETY: handle valido, chiuso una volta sola.
        unsafe { CloseHandle(self.0) };
    }
}

fn wide_long(p: &Path) -> Vec<u16> {
    let mut v: Vec<u16> = p.as_os_str().encode_wide().collect();
    let prefix: Vec<u16> = r"\\?\".encode_utf16().collect();
    if !v.starts_with(&prefix) {
        let mut x = prefix;
        x.append(&mut v);
        v = x;
    }
    v.push(0);
    v
}

fn open_attrs(p: &Path) -> Option<H> {
    let z = wide_long(p);
    // SAFETY: `z` terminata da zero; accesso di sola lettura degli attributi.
    let h = unsafe {
        CreateFileW(
            z.as_ptr(),
            FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_OPEN_NO_RECALL,
            null_mut(),
        )
    };
    (h != INVALID_HANDLE_VALUE).then_some(H(h))
}

fn query<T>(h: &H, class: i32) -> Option<T> {
    let mut out = MaybeUninit::<T>::zeroed();
    // SAFETY: buffer della dimensione e dell'allineamento di `T`, struttura a dimensione fissa.
    let ok = unsafe { GetFileInformationByHandleEx(h.0, class, out.as_mut_ptr().cast(), size_of::<T>() as u32) };
    // SAFETY: la chiamata è riuscita oppure il valore azzerato non viene usato.
    (ok != 0).then(|| unsafe { out.assume_init() })
}

/// Percorre `root` e misura ogni file.
pub fn walk(root: &Path) -> Oracle {
    let mut o = Oracle::default();
    let mut seen: HashSet<u128> = HashSet::new();
    let mut synthetic: u128 = 0;
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        o.dirs += 1;
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => {
                o.unreadable.push(dir);
                continue;
            }
        };
        // Come lo scanner: l'indice conta solo per le cartelle che si sono potute elencare.
        if let Some(h) = open_attrs(&dir)
            && let Some(si) = query::<FILE_STANDARD_INFO>(&h, FileStandardInfo)
        {
            o.dir_index_alloc += si.AllocationSize as u64;
        }
        for entry in rd.flatten() {
            let path = entry.path();
            let Some(h) = open_attrs(&path) else {
                // Un'ACL può negare anche la lettura degli attributi: una cartella così non è elencabile.
                if entry.file_type().is_ok_and(|t| t.is_dir()) {
                    o.dirs += 1;
                    o.unreadable.push(path);
                }
                continue;
            };
            // exFAT e FAT32 non hanno reparse point: lì `FileAttributeTagInfo` fallisce e valgono gli attributi della voce.
            let (attrs, reparse_tag) = match query::<FILE_ATTRIBUTE_TAG_INFO>(&h, FileAttributeTagInfo) {
                Some(t) => (t.FileAttributes, t.ReparseTag),
                None => {
                    use std::os::windows::fs::MetadataExt;
                    let Ok(m) = entry.metadata() else { continue };
                    (m.file_attributes(), 0)
                }
            };
            let surrogate = attrs & ATTR_REPARSE != 0 && reparse_tag & 0x2000_0000 != 0;
            if attrs & ATTR_DIRECTORY != 0 {
                if surrogate {
                    o.not_followed += 1;
                } else if attrs & ATTR_RECALL_ON_DATA_ACCESS == 0 {
                    stack.push(path);
                }
                continue;
            }
            if surrogate {
                o.not_followed += 1;
            }
            let Some(si) = query::<FILE_STANDARD_INFO>(&h, FileStandardInfo) else { continue };
            // exFAT e FAT32 non hanno `FileIdInfo` né hard link: ogni file ha un'identità propria.
            let id = query::<FILE_ID_INFO>(&h, FileIdInfo).map_or_else(|| { synthetic += 1; u128::MAX - synthetic }, |i| u128::from_le_bytes(i.FileId.Identifier));
            let f = OracleFile {
                logical: si.EndOfFile as u64,
                alloc: si.AllocationSize as u64,
                links: si.NumberOfLinks,
                id,
            };
            if seen.insert(f.id) {
                o.unique_alloc += f.alloc;
                o.unique_logical += f.logical;
            }
            o.files.insert(path.as_os_str().encode_wide().collect(), f);
        }
    }
    o
}

/// Percorso UTF-16 esatto come `PathBuf`.
pub fn path_from_utf16(p: &[u16]) -> PathBuf {
    PathBuf::from(OsString::from_wide(p))
}
