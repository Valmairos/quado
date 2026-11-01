// SPDX-License-Identifier: GPL-3.0-or-later
//! Prove sui VHDX di prova (06 §1.3, §1.4). Richiedono i volumi montati da `cargo xtask vhdx crea`:
//! `cargo test -p quado-scan-win --test vhdx -- --ignored --test-threads=1`.

use std::collections::HashMap;

use quado_model::Tree;
use quado_scan_win::{Options, Progress, ScanResult, scan};
use quado_testkit::oracle;
use quado_testkit::trees::{mounted_vhdx, volume_fixture};

fn volumes() -> Vec<(String, std::path::PathBuf)> {
    let v = mounted_vhdx();
    assert_eq!(v.len(), 3, "VHDX non montati: cargo xtask vhdx crea");
    v
}

fn scan_root(root: &std::path::Path, opts: Options) -> ScanResult {
    scan(root.to_str().unwrap(), opts, &Progress::default()).expect("scansione")
}

/// F-SC-10: su NTFS, exFAT e FAT32 i totali coincidono con l'oracolo.
#[test]
#[ignore = "richiede i VHDX di prova"]
fn file_systems_match_the_oracle() {
    for (name, root) in volumes() {
        let ntfs = name == "ntfs";
        volume_fixture(&root, ntfs);
        let r = scan_root(&root, Options::default());
        let o = oracle::walk(&root);
        let t = &r.tree;
        let mut scanned: HashMap<Vec<u16>, (u64, u64)> = HashMap::new();
        for i in 0..t.len() as u32 {
            if !t.is_dir(i) {
                scanned.insert(t.path(i), (t.logical(i), t.alloc(i)));
            }
        }
        let mut mismatches = 0;
        for (p, f) in &o.files {
            if scanned.get(p) != Some(&(f.logical, f.alloc)) {
                if mismatches < 5 {
                    eprintln!("{name}: {} scanner {:?} oracolo {:?}", String::from_utf16_lossy(p), scanned.get(p), (f.logical, f.alloc));
                }
                mismatches += 1;
            }
        }
        eprintln!(
            "{name}: {} file, allocato unico {} (oracolo {}), problemi {:?}",
            scanned.len(),
            t.alloc(Tree::ROOT),
            o.unique_alloc,
            r.problems.iter().map(|(n, p)| (t.path_lossy(*n), *p)).collect::<Vec<_>>()
        );
        assert!(o.files.len() > 2000, "{name}: l'oracolo deve vedere i file dell'albero ({})", o.files.len());
        assert!(r.problems.iter().all(|(n, _)| t.path_lossy(*n).ends_with("System Volume Information")), "{name}: {:?}", r.problems);
        assert_eq!(mismatches, 0, "{name}");
        assert_eq!(scanned.len(), o.files.len(), "{name}");
        assert_eq!(t.alloc(Tree::ROOT), o.unique_alloc, "{name}");
        if !ntfs {
            assert!(r.links.groups.is_empty(), "{name}: niente hard link");
        }
    }
}

/// F-OV-1, "Residuo su volume fermo" e "Errore iniettato" (06 §1.4).
#[test]
#[ignore = "richiede i VHDX di prova"]
fn idle_volume_residual_and_injected_error() {
    let (_, root) = volumes().into_iter().find(|(n, _)| n == "ntfs").unwrap();
    volume_fixture(&root, true);
    let good = scan_root(&root, Options::default());
    let bad = scan_root(&root, Options { inject_no_dedup: true, ..Options::default() });
    let (g, b) = (good.balance.unwrap(), bad.balance.unwrap());
    eprintln!(
        "residuo {} byte, deriva {}, MFT {:?}, indici {}, USN {:?}, hard link {}",
        g.unattributed, g.drift, g.metadata.mft, g.metadata.dir_indexes, g.metadata.usn_journal, good.links.extra_alloc
    );
    // L'errore iniettato sposta il residuo esattamente dei byte degli hard link.
    assert!(good.links.extra_alloc > 0);
    assert_eq!(g.unattributed - b.unattributed, good.links.extra_alloc as i64 + (b.drift - g.drift));
    // Residuo su volume fermo: entro i metafile attesi più la tolleranza, fissati da tre misure (06 §1.4).
    assert_eq!(g.drift, 0, "volume non fermo");
    assert!(g.unattributed.abs_diff(EXPECTED_METAFILES) <= TOLERANCE, "residuo {} contro {EXPECTED_METAFILES} ± {TOLERANCE}", g.unattributed);
}

/// Metafile attesi sul VHDX NTFS di prova da 4 GiB (cluster 4 KiB) e tolleranza, fissati da cinque misure
/// identiche del 13/10/2026 (M1): 21.165.144 byte non visibili all'enumerazione
/// (dedotto: log transazionali `$TxfLog`, `$LogFile`, `$Bitmap`, `$Secure`, `$UpCase`).
const EXPECTED_METAFILES: i64 = 21_165_144;
const TOLERANCE: u64 = 1 << 20;
