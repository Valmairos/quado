// SPDX-License-Identifier: GPL-3.0-or-later
//! Cestino dei VHDX di prova (06 §5.3, §6.1): capacità ridotta per la durata di una prova e
//! svuotamento senza conferme. Solo sui volumi montati da `cargo xtask vhdx crea`: ogni funzione rifiuta
//! qualunque altra radice, quindi il Cestino di C: e degli altri dischi non si tocca mai.
//!
//! Il prodotto non svuota mai il Cestino senza il dialogo di Windows (F-AC-1): lo svuotamento silenzioso
//! esiste solo qui, per preparare e ripulire le prove.

use std::path::{Path, PathBuf};
use std::process::Command;

use windows_sys::Win32::UI::Shell::{SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW};

use crate::trees::mounted_vhdx;

/// `root` se è la radice di un VHDX di prova montato; altrimenti la prova si ferma.
fn test_root(root: &Path) -> PathBuf {
    let r = root.to_string_lossy().to_ascii_uppercase();
    let ok = mounted_vhdx().into_iter().any(|(_, p)| p.to_string_lossy().to_ascii_uppercase() == r);
    assert!(ok, "{} non è un VHDX di prova montato: il Cestino degli altri volumi non si tocca", root.display());
    root.to_path_buf()
}

/// GUID del volume (`{...}`), da `mountvol`.
pub fn volume_guid(root: &Path) -> Option<String> {
    let root = test_root(root);
    let out = Command::new("mountvol").arg(root.to_string_lossy().trim_end_matches('\\')).arg("/L").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let start = s.find('{')?;
    let end = s[start..].find('}')? + start;
    Some(s[start..=end].to_owned())
}

fn bitbucket_key(guid: &str) -> String {
    format!(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume\{guid}")
}

fn reg_query_dword(key: &str, value: &str) -> Option<u32> {
    let out = Command::new("reg").args(["query", key, "/v", value]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let hex = s.split_whitespace().find(|w| w.starts_with("0x"))?;
    u32::from_str_radix(hex.trim_start_matches("0x"), 16).ok()
}

fn reg_set_dword(key: &str, value: &str, v: u32) -> bool {
    Command::new("reg").args(["add", key, "/v", value, "/t", "REG_DWORD", "/d", &v.to_string(), "/f"]).output().is_ok_and(|o| o.status.success())
}

fn reg_delete_value(key: &str, value: &str) {
    let _ = Command::new("reg").args(["delete", key, "/v", value, "/f"]).output();
}

/// Un valore DWORD della chiave `BitBucket` di un VHDX di prova, cambiato per la durata del valore e
/// ripristinato al drop (anche se la prova fallisce): `MaxCapacity` in MB o `NukeOnDelete`.
pub struct BinSetting {
    key: String,
    value: &'static str,
    original: Option<u32>,
}

impl BinSetting {
    pub fn set(root: &Path, value: &'static str, v: u32) -> Self {
        let guid = volume_guid(root).expect("GUID del volume di prova");
        let key = bitbucket_key(&guid);
        let original = reg_query_dword(&key, value);
        assert!(reg_set_dword(&key, value, v), "{value} non impostato per {}", root.display());
        Self { key, value, original }
    }

    /// Capacità del Cestino in MB.
    pub fn capacity(root: &Path, mb: u32) -> Self {
        Self::set(root, "MaxCapacity", mb)
    }

    /// `NukeOnDelete` = 1: Windows elimina senza passare dal Cestino.
    pub fn nuke(root: &Path) -> Self {
        Self::set(root, "NukeOnDelete", 1)
    }
}

impl Drop for BinSetting {
    fn drop(&mut self) {
        match self.original {
            Some(v) => {
                reg_set_dword(&self.key, self.value, v);
            }
            None => reg_delete_value(&self.key, self.value),
        }
    }
}

/// Svuota senza conferme il Cestino di un VHDX di prova.
pub fn empty_test_volume(root: &Path) {
    let root = test_root(root);
    let w: Vec<u16> = root.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `w` è terminata da zero e vive per la chiamata; nessuna finestra proprietaria. La radice è
    // stata controllata: è un VHDX di prova, mai NULL (che svuoterebbe i Cestini di tutti i volumi).
    let _ = unsafe { SHEmptyRecycleBinW(std::ptr::null_mut(), w.as_ptr(), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND) };
}

/// Cartella del Cestino dell'utente corrente su un VHDX di prova (la sola leggibile tra quelle per SID).
pub fn user_bin_dir(root: &Path) -> Option<PathBuf> {
    let root = test_root(root);
    std::fs::read_dir(root.join("$Recycle.Bin")).ok()?.flatten().map(|e| e.path()).find(|p| std::fs::read_dir(p).is_ok())
}
