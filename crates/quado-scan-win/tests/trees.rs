// SPDX-License-Identifier: GPL-3.0-or-later
//! Alberi di prova contro l'oracolo indipendente (06 §1.1, §1.2): F-SC-2, F-SC-6, F-SC-7, F-SC-8.
//! Ogni test crea la sua cartella in `%TEMP%` e la cancella alla fine.

use std::collections::HashMap;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::Ordering::Relaxed;
use std::time::{Duration, Instant};

use quado_model::{Flags, Tree};
use quado_scan_win::{Options, Problem, Progress, ScanResult, scan};
use quado_testkit::oracle::{self, Oracle};
use quado_testkit::trees::{self, TestDir};

fn scan_dir(root: &Path, opts: Options) -> ScanResult {
    scan(root.to_str().unwrap(), opts, &Progress::default()).expect("scansione")
}

fn wide(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().collect()
}

fn node_of(t: &Tree, path: &Path) -> u32 {
    let want = wide(path);
    (0..t.len() as u32).find(|&i| t.path(i) == want).unwrap_or_else(|| panic!("nodo non trovato: {}", path.display()))
}

/// Confronto al byte, file per file e nei totali.
fn compare(r: &ScanResult, o: &Oracle) {
    let t = &r.tree;
    let mut scanned: HashMap<Vec<u16>, (u64, u64)> = HashMap::new();
    for i in 0..t.len() as u32 {
        if !t.is_dir(i) {
            scanned.insert(t.path(i), (t.logical(i), t.alloc(i)));
        }
    }
    for (p, f) in &o.files {
        let s = scanned.get(p).unwrap_or_else(|| panic!("lo scanner non ha {}", String::from_utf16_lossy(p)));
        assert_eq!(*s, (f.logical, f.alloc), "{}", String::from_utf16_lossy(p));
    }
    assert_eq!(scanned.len(), o.files.len(), "numero di file");
    assert_eq!(t.alloc(Tree::ROOT), o.unique_alloc, "allocato unico");
    assert_eq!(t.logical(Tree::ROOT), o.unique_logical, "logico unico");
    assert_eq!(t.dir_index_total(), o.dir_index_alloc, "indici delle cartelle");
}

#[test]
fn full_tree_matches_the_oracle() {
    let d = TestDir::new("completo");
    let root = &d.path;
    trees::small_files(&root.join("piccoli"), 3000, 5000);
    for k in 0..3 {
        trees::sparse_file(&root.join("sparse").join(format!("s{k}.bin")).tap_parent(), (k + 1) * (1 << 30));
    }
    // Hard link tra cartelle lontane.
    let big = root.join("a\\originale.bin");
    trees::write_file(&big, 300_000, 3);
    std::fs::create_dir_all(root.join("b\\c")).unwrap();
    std::fs::create_dir_all(root.join("z")).unwrap();
    std::fs::hard_link(&big, root.join("b\\c\\link1.bin")).unwrap();
    std::fs::hard_link(&big, root.join("z\\link2.bin")).unwrap();
    // Junction che crea un ciclo verso la radice.
    assert!(trees::junction(&root.join("b\\ciclo"), root), "mklink /J");
    let symlink = trees::dir_symlink(&root.join("simbolico"), &root.join("a"));
    // Compressione NTFS e WOF.
    let ntfs = root.join("compressi\\ntfs.txt");
    trees::write_compressible(&ntfs, 2_000_000);
    assert!(trees::ntfs_compress(&ntfs), "compact /c");
    let wof = root.join("compressi\\wof.txt");
    trees::write_compressible(&wof, 2_000_000);
    let wof_ok = trees::wof_compress(&wof);
    // Flusso alternativo, nome non valido, percorso lungo, profondità, file in uso.
    let with_ads = root.join("flussi\\base.txt");
    trees::write_file(&with_ads, 1000, 1);
    trees::alternate_stream(&with_ads, "nascosto", 500_000);
    let bad_name = trees::invalid_utf16_file(root);
    let long = trees::long_path(&root.join("lungo"), 400);
    std::fs::write(long.join("in-fondo.txt"), b"percorso lungo").unwrap();
    let deepest = trees::deep(&root.join("profondo"), 2000);
    let busy = root.join("in-uso.log");
    trees::write_file(&busy, 70_000, 9);
    let _hold = trees::hold_exclusive(&busy);

    let r = scan_dir(root, Options::default());
    let o = oracle::walk(root);
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    compare(&r, &o);
    let t = &r.tree;

    // F-SC-7: hard link contati una volta, junction e symlink non seguiti.
    assert_eq!(r.links.groups.len(), 1);
    assert_eq!(r.links.extra_names, 2);
    let secondaries = [root.join("b\\c\\link1.bin"), root.join("z\\link2.bin"), big.clone()]
        .iter()
        .filter(|p| t.flags(node_of(t, p)).has(Flags::SECONDARY_LINK))
        .count();
    assert_eq!(secondaries, 2);
    assert!(t.flags(node_of(t, &root.join("b\\ciclo"))).has(Flags::NOT_FOLLOWED));
    if symlink {
        assert!(t.flags(node_of(t, &root.join("simbolico"))).has(Flags::NOT_FOLLOWED));
    } else {
        eprintln!("nota: symlink non creato (Modalità sviluppatore spenta); caso non provato");
    }
    // F-SC-8: sparse e compressi con allocata minore della logica; WOF corretto.
    for k in 0..3 {
        let n = node_of(t, &root.join("sparse").join(format!("s{k}.bin")));
        assert!(t.alloc(n) < t.logical(n) / 100);
    }
    let n = node_of(t, &ntfs);
    assert!(t.alloc(n) < t.logical(n));
    if wof_ok {
        let n = node_of(t, &wof);
        assert!(t.flags(n).has(Flags::ALLOC_FIXED), "WOF: la voce di cartella riporta 0 e va corretta");
        assert!(t.alloc(n) > 0 && t.alloc(n) < t.logical(n));
    } else {
        eprintln!("nota: compressione WOF non applicata; caso non provato");
    }
    // Flusso alternativo: non contato.
    assert_eq!(t.logical(node_of(t, &with_ads)), 1000);
    // F-SC-2: nome non valido esatto, percorso lungo, profondità, file in uso misurato.
    assert_eq!(t.name(node_of(t, &bad_name)).to_utf16(), wide(Path::new(bad_name.file_name().unwrap())));
    node_of(t, &long.join("in-fondo.txt"));
    assert_eq!(t.depth(node_of(t, &deepest.join("fondo.txt"))), 2002);
    assert_eq!(t.logical(node_of(t, &busy)), 70_000);
}

#[test]
fn denied_folder_is_listed_and_not_counted() {
    let mut d = TestDir::new("negata");
    let root = d.path.clone();
    trees::write_file(&root.join("visibile.bin"), 10_000, 1);
    let denied = root.join("privata");
    trees::write_file(&denied.join("segreto.bin"), 50_000, 2);
    assert!(d.deny_listing(&denied), "icacls /deny");
    let r = scan_dir(&root, Options::default());
    let t = &r.tree;
    let n = node_of(t, &denied);
    assert!(t.flags(n).has(Flags::DENIED));
    assert!(r.problems.contains(&(n, Problem::AccessDenied)));
    assert_eq!(r.stats.denied, 1);
    // Il contenuto resta fuori dall'allocato letto (finirà nel non attribuito).
    assert_eq!(t.files(Tree::ROOT), 1);
    let o = oracle::walk(&root);
    assert_eq!(o.unreadable.len(), 1);
    compare(&r, &o);
}

#[test]
fn injected_error_counts_hard_links_twice() {
    let d = TestDir::new("iniettato");
    let root = &d.path;
    let f = root.join("uno.bin");
    trees::write_file(&f, 123_456, 4);
    for k in 0..3 {
        std::fs::hard_link(&f, root.join(format!("altro{k}.bin"))).unwrap();
    }
    let good = scan_dir(root, Options::default());
    let bad = scan_dir(root, Options { inject_no_dedup: true, ..Options::default() });
    let diff = bad.tree.alloc(Tree::ROOT) - good.tree.alloc(Tree::ROOT);
    assert_eq!(diff, good.links.extra_alloc);
    assert_eq!(diff, 3 * good.tree.alloc(node_of(&good.tree, &f)));
}

#[test]
fn cancel_stops_within_500_ms() {
    let d = TestDir::new("annulla");
    trees::small_files(&d.path, 60_000, 10);
    let progress = Progress::default();
    let root = d.path.to_str().unwrap().to_owned();
    std::thread::scope(|s| {
        let h = s.spawn(|| scan(&root, Options::default(), &progress).unwrap());
        while progress.files.load(Relaxed) < 2_000 {
            std::thread::sleep(Duration::from_millis(1));
        }
        let t0 = Instant::now();
        progress.cancel();
        let r = h.join().unwrap();
        let took = t0.elapsed();
        assert!(took <= Duration::from_millis(500), "annullamento in {took:?}");
        if r.stats.files < 60_000 {
            assert!(r.cancelled);
            assert!(r.problems.iter().any(|(_, p)| *p == Problem::Cancelled));
        }
    });
}

trait TapParent {
    fn tap_parent(self) -> Self;
}

impl TapParent for std::path::PathBuf {
    fn tap_parent(self) -> Self {
        std::fs::create_dir_all(self.parent().unwrap()).unwrap();
        self
    }
}
