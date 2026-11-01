// SPDX-License-Identifier: GPL-3.0-or-later
//! 06 §3.6: effetto delle letture sulla data di ultimo accesso, sul VHDX NTFS di prova montato come `R:`
//! (etichetta `RilievoNTFS`, creato da `cargo xtask vhdx crea`). Senza quel volume il test lo dice e
//! termina. Usa solo `R:\m4-ultimo-accesso`, cancellata alla fine anche se il test fallisce.
//!
//! Identità e scansione non devono cambiare l'ultimo accesso; hash e indice degli zip lo cambiano (NTFS
//! lo aggiorna quando la differenza supera un'ora, se l'aggiornamento è attivo). Il setup porta l'ultimo
//! accesso a un anno fa con un handle `FILE_WRITE_ATTRIBUTES`: solo qui, sui file di prova.

mod common;

use std::fs::FileTimes;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::{Duration, SystemTime};

use quado_scan_win::content::{identity, sha256};
use quado_scan_win::system::local_time;
use quado_scan_win::zip::zip_index;
use quado_scan_win::{Options, Progress, scan};

const ROOT: &str = r"R:\";
const LABEL: &str = "RilievoNTFS";
const DIR: &str = r"R:\m4-ultimo-accesso";
/// Un'ora in unità `FILETIME` (100 ns).
const HOUR: i64 = 36_000_000_000;

/// Cancella la cartella di prova al drop, anche dopo un fallimento.
struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Etichetta e file system di `R:\`, letti solo da quel volume.
fn volume_of_r() -> Option<(String, String)> {
    use windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW;
    let root: Vec<u16> = ROOT.encode_utf16().chain(std::iter::once(0)).collect();
    let (mut label, mut fs) = ([0u16; 261], [0u16; 261]);
    // SAFETY: radice terminata da zero; i due buffer locali hanno la lunghezza dichiarata; gli altri
    // puntatori d'uscita sono nulli, ammessi.
    let ok = unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            fs.as_mut_ptr(),
            fs.len() as u32,
        )
    };
    let text = |b: &[u16]| String::from_utf16_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]);
    (ok != 0).then(|| (text(&label), text(&fs)))
}

fn filetime(t: SystemTime) -> i64 {
    let unix = t.duration_since(SystemTime::UNIX_EPOCH).unwrap();
    (unix.as_nanos() / 100) as i64 + 116_444_736_000_000_000
}

fn when(ft: i64) -> String {
    local_time(ft).map_or_else(|| ft.to_string(), |t| format!("{}-{:02}-{:02} {:02}:{:02}", t.year, t.month, t.day, t.hour, t.minute))
}

fn accessed(p: &Path) -> i64 {
    identity(p.to_str().unwrap()).unwrap().accessed
}

#[test]
fn reads_and_last_access_time() {
    if !Path::new(ROOT).exists() {
        eprintln!("test non eseguito: R:\\ non esiste (VHDX di prova: cargo xtask vhdx crea)");
        return;
    }
    match volume_of_r() {
        Some((label, fs)) if label == LABEL && fs == "NTFS" => {}
        other => {
            eprintln!("test non eseguito: R:\\ non è il VHDX NTFS di prova (etichetta e file system: {other:?})");
            return;
        }
    }
    let dir = PathBuf::from(DIR);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir(&dir).unwrap();
    let _cleanup = Cleanup(dir.clone());

    let plain = dir.join("qualsiasi.txt");
    let zip = dir.join("archivio.zip");
    let big = dir.join("grande.bin");
    let control = dir.join("controllo.bin");
    std::fs::write(&plain, b"un file qualsiasi\r\n").unwrap();
    std::fs::write(&zip, common::build_zip(&common::sample_entries(), &Default::default()).bytes).unwrap();
    std::fs::write(&big, common::pseudo_random(6 << 20, 4)).unwrap();
    std::fs::write(&control, common::pseudo_random(64 << 10, 5)).unwrap();
    let files = [&plain, &zip, &big, &control];
    // Lascia finire l'analisi dell'antivirus sui file appena scritti, poi porta l'ultimo accesso a un anno fa.
    std::thread::sleep(Duration::from_secs(1));
    let old = SystemTime::now() - Duration::from_secs(365 * 24 * 3600);
    for f in files {
        let h = std::fs::OpenOptions::new().access_mode(0x0100 /* FILE_WRITE_ATTRIBUTES */).open(f).unwrap();
        h.set_times(FileTimes::new().set_accessed(old)).unwrap();
    }
    let set = filetime(old);
    let base: Vec<i64> = files.iter().map(|f| accessed(f)).collect();
    for (f, b) in files.iter().zip(&base) {
        eprintln!("{}: ultimo accesso impostato {}", f.display(), when(*b));
        assert!((b - set).abs() < 10_000_000, "impostato {set}, letto {b}");
    }
    let unchanged = |what: &str| {
        for (f, b) in files.iter().zip(&base) {
            let now = accessed(f);
            assert_eq!(now, *b, "{what} ha cambiato l'ultimo accesso di {}: {} invece di {}", f.display(), when(now), when(*b));
        }
    };

    // Identità, più volte: solo attributi.
    for _ in 0..3 {
        for f in files {
            identity(f.to_str().unwrap()).unwrap();
        }
    }
    unchanged("l'identità");

    // Scansione del sottoalbero.
    let dir_before = std::fs::metadata(&dir).unwrap().last_access_time();
    let r = scan(DIR, Options::default(), &Progress::default()).unwrap();
    assert!(r.tree.len() >= 5, "nodi {}", r.tree.len());
    unchanged("la scansione");
    let dir_after = std::fs::metadata(&dir).unwrap().last_access_time();
    eprintln!("cartella: ultimo accesso {} prima della scansione, {} dopo", when(dir_before as i64), when(dir_after as i64));

    // Controllo: una lettura qualsiasi aggiorna l'ultimo accesso?
    std::fs::read(&control).unwrap();
    let control_after = accessed(&control);
    let enabled = control_after - base[3] > HOUR;
    eprintln!("controllo (std::fs::read): {} -> {}; aggiornamento attivo: {enabled}", when(base[3]), when(control_after));

    // Letture di contenuto.
    let (cancel, progress) = (AtomicBool::new(false), AtomicU64::new(0));
    sha256(plain.to_str().unwrap(), &cancel, &progress).unwrap();
    sha256(big.to_str().unwrap(), &cancel, &progress).unwrap();
    assert_eq!(zip_index(zip.to_str().unwrap()).unwrap().entries.len(), 7);
    for (f, b) in [(&plain, base[0]), (&zip, base[1]), (&big, base[2])] {
        let now = accessed(f);
        eprintln!("{}: {} -> {}", f.display(), when(b), when(now));
        if enabled {
            assert!(now - b > HOUR, "la lettura non ha aggiornato l'ultimo accesso di {}", f.display());
        }
    }
    if !enabled {
        eprintln!("aggiornamento dell'ultimo accesso disattivato su R:: verificata solo la prima metà (identità e scansione)");
    }
}
