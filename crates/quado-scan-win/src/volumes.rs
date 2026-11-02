// SPDX-License-Identifier: GPL-3.0-or-later
//! Elenco dei volumi: lettera o cartella di montaggio, etichetta, file system,
//! tipo di unità, capacità e spazio libero. Il volume di sistema è il primo.

use quado_model::Space;
use windows_sys::Win32::Storage::FileSystem::{
    FindFirstVolumeW, FindNextVolumeW, FindVolumeClose, GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW,
    GetVolumePathNamesForVolumeNameW,
};
use windows_sys::Win32::System::SystemInformation::GetSystemWindowsDirectoryW;

use crate::win;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriveKind {
    Fixed,
    Removable,
    Network,
    Optical,
    Ram,
    Unknown,
}

impl DriveKind {
    fn from_win(t: u32) -> Self {
        match t {
            2 => Self::Removable,
            3 => Self::Fixed,
            4 => Self::Network,
            5 => Self::Optical,
            6 => Self::Ram,
            _ => Self::Unknown,
        }
    }
}

/// Quanto Quado garantisce su un file system. La UI lo traduce in parole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    /// NTFS: tutto.
    Full,
    /// exFAT e FAT32: allocata arrotondata al cluster, niente hard link, compressione né Cestino garantito.
    Fat,
    /// ReFS, unità di rete e il resto: "senza garanzie".
    NoGuarantees,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    /// `\\?\Volume{GUID}\`, vuoto per le unità di rete.
    pub guid_path: String,
    /// Lettere e cartelle di montaggio (`C:\`, `D:\Dati\`). Vuoto se il volume non ha percorso.
    pub paths: Vec<String>,
    pub label: String,
    pub file_system: String,
    pub kind: DriveKind,
    pub space: Option<Space>,
    pub is_system: bool,
}

impl Volume {
    pub fn support(&self) -> Support {
        match self.file_system.to_ascii_uppercase().as_str() {
            "NTFS" if self.kind != DriveKind::Network => Support::Full,
            "EXFAT" | "FAT32" | "FAT" if self.kind != DriveKind::Network => Support::Fat,
            _ => Support::NoGuarantees,
        }
    }

    /// Si può scansionare per percorso.
    pub fn scannable(&self) -> bool {
        !self.paths.is_empty()
    }
}

fn from_wide_z(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn multi_sz(buf: &[u16]) -> Vec<String> {
    buf.split(|&c| c == 0).take_while(|s| !s.is_empty()).map(String::from_utf16_lossy).collect()
}

fn info(root: &str) -> (String, String, u32) {
    let z = win::wide_z(&root.encode_utf16().collect::<Vec<_>>());
    let mut label = [0u16; 261];
    let mut fs = [0u16; 261];
    let (mut serial, mut maxlen, mut flags) = (0u32, 0u32, 0u32);
    // SAFETY: `z` terminata da zero; i buffer hanno la lunghezza dichiarata (261 caratteri).
    let ok = unsafe {
        GetVolumeInformationW(z.as_ptr(), label.as_mut_ptr(), 261, &mut serial, &mut maxlen, &mut flags, fs.as_mut_ptr(), 261)
    };
    if ok == 0 { (String::new(), String::new(), 0) } else { (from_wide_z(&label), from_wide_z(&fs), flags) }
}

fn drive_type(root: &str) -> u32 {
    let z = win::wide_z(&root.encode_utf16().collect::<Vec<_>>());
    // SAFETY: `z` terminata da zero.
    unsafe { GetDriveTypeW(z.as_ptr()) }
}

fn system_drive() -> String {
    let mut buf = [0u16; 261];
    // SAFETY: buffer di 261 caratteri, dichiarato alla chiamata.
    let n = unsafe { GetSystemWindowsDirectoryW(buf.as_mut_ptr(), 261) } as usize;
    String::from_utf16_lossy(&buf[..n.min(3)]).to_ascii_uppercase()
}

fn paths_of(guid: &[u16]) -> Vec<String> {
    let mut buf = vec![0u16; 1024];
    let mut needed = 0u32;
    // SAFETY: `guid` terminata da zero; il buffer ha la lunghezza dichiarata; se è piccolo la funzione
    // restituisce la dimensione necessaria e si riprova.
    let mut ok = unsafe { GetVolumePathNamesForVolumeNameW(guid.as_ptr(), buf.as_mut_ptr(), buf.len() as u32, &mut needed) };
    if ok == 0 && needed as usize > buf.len() {
        buf = vec![0u16; needed as usize];
        // SAFETY: come sopra, con il buffer della dimensione richiesta.
        ok = unsafe { GetVolumePathNamesForVolumeNameW(guid.as_ptr(), buf.as_mut_ptr(), buf.len() as u32, &mut needed) };
    }
    if ok == 0 { Vec::new() } else { multi_sz(&buf) }
}

/// Volumi locali (con o senza lettera) e unità di rete mappate.
pub fn list() -> Vec<Volume> {
    let system = system_drive();
    let mut out = Vec::new();
    let mut name = [0u16; 64];
    // SAFETY: buffer di 64 caratteri dichiarato; l'handle di ricerca si chiude con FindVolumeClose.
    let find = unsafe { FindFirstVolumeW(name.as_mut_ptr(), name.len() as u32) };
    if find != windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        loop {
            let guid_path = from_wide_z(&name);
            let paths = paths_of(&name);
            let (label, file_system, _) = info(&guid_path);
            let kind = DriveKind::from_win(drive_type(&guid_path));
            let space = win::disk_space(&guid_path.encode_utf16().collect::<Vec<_>>()).ok();
            let is_system = paths.iter().any(|p| p.eq_ignore_ascii_case(&system));
            out.push(Volume { guid_path, paths, label, file_system, kind, space, is_system });
            name = [0u16; 64];
            // SAFETY: handle di ricerca valido; buffer di 64 caratteri dichiarato.
            if unsafe { FindNextVolumeW(find, name.as_mut_ptr(), name.len() as u32) } == 0 {
                break;
            }
        }
        // SAFETY: handle restituito da FindFirstVolumeW, chiuso una volta.
        unsafe { FindVolumeClose(find) };
    }
    // Unità di rete mappate: non compaiono tra i volumi locali.
    let mut drives = [0u16; 512];
    // SAFETY: buffer di 512 caratteri dichiarato.
    let n = unsafe { GetLogicalDriveStringsW(drives.len() as u32, drives.as_mut_ptr()) } as usize;
    for d in multi_sz(&drives[..n.min(drives.len())]) {
        if drive_type(&d) == 4 {
            let (label, file_system, _) = info(&d);
            let space = win::disk_space(&d.encode_utf16().collect::<Vec<_>>()).ok();
            out.push(Volume { guid_path: String::new(), paths: vec![d], label, file_system, kind: DriveKind::Network, space, is_system: false });
        }
    }
    out.sort_by_key(|v| (!v.is_system, v.paths.is_empty(), v.paths.first().cloned().unwrap_or_default()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_volume_is_first_and_ntfs() {
        let v = list();
        assert!(!v.is_empty());
        assert!(v[0].is_system, "{v:#?}");
        assert_eq!(v[0].support(), Support::Full);
        assert_eq!(v[0].kind, DriveKind::Fixed);
        assert!(v[0].space.is_some_and(|s| s.capacity > 0));
    }
}
