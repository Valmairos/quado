// SPDX-License-Identifier: GPL-3.0-or-later
//! Alberi di prova (06 §1.2), creati in cartelle temporanee su NTFS e cancellati alla fine.
//!
//! Scrivono solo dentro la cartella di prova. Alcuni casi dipendono dal sistema: i collegamenti
//! simbolici richiedono la Modalità sviluppatore; se un caso non si può creare, la funzione lo dice
//! (`Ok(false)`) e il test lo dichiara invece di fallire in silenzio.

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_sys::Win32::System::IO::DeviceIoControl;
use windows_sys::Win32::System::Ioctl::FSCTL_SET_SPARSE;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Cartella di prova in `%TEMP%`, cancellata al drop (anche dopo un'ACL di negazione).
pub struct TestDir {
    pub path: PathBuf,
    denied: Vec<PathBuf>,
}

impl TestDir {
    pub fn new(name: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("quado-prova-{name}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("cartella di prova");
        Self { path, denied: Vec::new() }
    }

    pub fn join(&self, rel: &str) -> PathBuf {
        self.path.join(rel)
    }

    /// Nega l'elenco di `dir` all'utente corrente (ACL di negazione, come in 06 §1.2).
    pub fn deny_listing(&mut self, dir: &Path) -> bool {
        let user = current_user();
        let ok = Command::new("icacls")
            .arg(native(dir))
            .args(["/deny", &format!("{user}:(RX)")])
            .output()
            .is_ok_and(|o| o.status.success());
        if ok {
            self.denied.push(dir.to_path_buf());
        }
        ok
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let user = current_user();
        for d in &self.denied {
            let _ = Command::new("icacls").arg(d).args(["/remove:d", &user]).output();
        }
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn current_user() -> String {
    let domain = std::env::var("USERDOMAIN").unwrap_or_default();
    let user = std::env::var("USERNAME").unwrap_or_default();
    if domain.is_empty() { user } else { format!("{domain}\\{user}") }
}

pub fn write_file(path: &Path, bytes: usize, fill: u8) {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    let mut f = fs::File::create(path).unwrap();
    let chunk = vec![fill; 64 * 1024];
    let mut left = bytes;
    while left > 0 {
        let n = left.min(chunk.len());
        f.write_all(&chunk[..n]).unwrap();
        left -= n;
    }
}

/// Testo comprimibile (per la compressione NTFS e WOF).
pub fn write_compressible(path: &Path, bytes: usize) {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    let line = b"Rilievo albero di prova: testo ripetuto per la compressione. 0123456789\r\n";
    let mut buf = Vec::with_capacity(bytes);
    while buf.len() < bytes {
        buf.extend_from_slice(line);
    }
    buf.truncate(bytes);
    fs::write(path, buf).unwrap();
}

/// `n` file piccoli da 0 a `max` byte in `dir`, distribuiti in sottocartelle da 500.
pub fn small_files(dir: &Path, n: usize, max: usize) {
    for i in 0..n {
        let sub = dir.join(format!("g{:04}", i / 500));
        if i % 500 == 0 {
            fs::create_dir_all(&sub).unwrap();
        }
        let size = (i * 7919) % (max + 1);
        fs::write(sub.join(format!("f{i}.dat")), vec![b'a'; size]).unwrap();
    }
}

/// File sparse con `size` byte logici e quasi nessun cluster allocato.
pub fn sparse_file(path: &Path, size: u64) {
    let f = fs::File::create(path).unwrap();
    let mut ret = 0u32;
    // SAFETY: handle valido del file appena creato; nessun buffer d'ingresso né d'uscita.
    let ok = unsafe {
        DeviceIoControl(f.as_raw_handle(), FSCTL_SET_SPARSE, std::ptr::null(), 0, std::ptr::null_mut(), 0, &mut ret, std::ptr::null_mut())
    };
    assert_ne!(ok, 0, "FSCTL_SET_SPARSE");
    f.set_len(size).unwrap();
    // Pochi byte veri in fondo, così il file ha almeno un cluster.
    drop(f);
    let mut f = fs::OpenOptions::new().write(true).open(path).unwrap();
    use std::io::Seek;
    f.seek(std::io::SeekFrom::Start(size - 4096)).unwrap();
    f.write_all(&[7u8; 4096]).unwrap();
}

/// Percorso con le sole barre rovesciate: `cmd` e `compact` leggono `/` come inizio di un'opzione.
pub fn native(p: &Path) -> PathBuf {
    PathBuf::from(p.as_os_str().to_string_lossy().replace('/', "\\"))
}

fn run(cmd: &str, args: &[&std::ffi::OsStr]) -> bool {
    Command::new(cmd).args(args).output().is_ok_and(|o| o.status.success())
}

/// Compressione NTFS (`compact /c`).
pub fn ntfs_compress(path: &Path) -> bool {
    run("compact", &["/c".as_ref(), "/q".as_ref(), native(path).as_os_str()])
}

/// Compressione WOF (`compact /c /exe:lzx`): la voce di cartella riporterà allocata 0.
pub fn wof_compress(path: &Path) -> bool {
    run("compact", &["/c".as_ref(), "/q".as_ref(), "/exe:lzx".as_ref(), native(path).as_os_str()])
}

/// Junction (`mklink /J`), senza privilegi.
pub fn junction(link: &Path, target: &Path) -> bool {
    run("cmd", &["/c".as_ref(), "mklink".as_ref(), "/J".as_ref(), native(link).as_os_str(), native(target).as_os_str()])
}

/// Collegamento simbolico a una cartella: `false` se manca la Modalità sviluppatore.
pub fn dir_symlink(link: &Path, target: &Path) -> bool {
    std::os::windows::fs::symlink_dir(target, link).is_ok()
}

/// Flusso alternativo: non contato dallo scanner né da `FileStandardInfo` (F-SC-8).
pub fn alternate_stream(file: &Path, stream: &str, bytes: usize) {
    let mut s = file.as_os_str().to_owned();
    s.push(format!(":{stream}"));
    fs::write(PathBuf::from(s), vec![b'z'; bytes]).unwrap();
}

/// File con un nome UTF-16 non valido (surrogato spaiato). Restituisce il percorso esatto.
pub fn invalid_utf16_file(dir: &Path) -> PathBuf {
    let name = OsString::from_wide(&[b'x' as u16, 0xD800, b'y' as u16, b'.' as u16, b't' as u16]);
    let p = dir.join(name);
    fs::write(&p, b"nome non valido").unwrap();
    p
}

/// Cartelle annidate fino a superare `chars` caratteri di percorso.
pub fn long_path(dir: &Path, chars: usize) -> PathBuf {
    let mut p = dir.to_path_buf();
    let seg = "cartella-con-un-nome-piuttosto-lungo-per-il-test";
    while p.as_os_str().len() < chars {
        p.push(seg);
    }
    fs::create_dir_all(&p).unwrap();
    p
}

/// `levels` cartelle annidate con nomi di una lettera.
pub fn deep(dir: &Path, levels: usize) -> PathBuf {
    let mut p = dir.to_path_buf();
    for _ in 0..levels {
        p.push("d");
    }
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("fondo.txt"), b"fondo").unwrap();
    p
}

/// File aperto senza condivisione: resta aperto finché si tiene il valore restituito.
pub fn hold_exclusive(path: &Path) -> fs::File {
    fs::OpenOptions::new().read(true).write(true).share_mode(0).open(path).unwrap()
}

/// Albero standard per i volumi di prova (06 §1.3): file piccoli, file medi, profondità e,
/// solo su NTFS, hard link e compressione. Restituisce la cartella creata.
pub fn volume_fixture(volume_root: &Path, ntfs: bool) -> PathBuf {
    let base = volume_root.join("prova");
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    small_files(&base.join("piccoli"), 2000, 6000);
    for k in 0..5 {
        write_file(&base.join("medi").join(format!("m{k}.bin")), 3_000_000 + k * 12_345, k as u8);
    }
    deep(&base.join("profondo"), 80);
    if ntfs {
        let original = base.join("medi").join("m0.bin");
        fs::create_dir_all(base.join("link")).unwrap();
        for k in 0..3 {
            fs::hard_link(&original, base.join("link").join(format!("l{k}.bin"))).unwrap();
        }
        let c = base.join("compresso.txt");
        write_compressible(&c, 1_500_000);
        ntfs_compress(&c);
    }
    base
}

/// Lettere dei VHDX montati da `cargo xtask vhdx crea`: (nome, radice).
pub fn mounted_vhdx() -> Vec<(String, PathBuf)> {
    fs::read_to_string(r"C:\RilievoProve\stato.tsv")
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (n, c) = l.split_once('\t')?;
            Some((n.to_owned(), PathBuf::from(format!("{c}:\\"))))
        })
        .filter(|(_, p)| p.exists())
        .collect()
}
