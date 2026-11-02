// SPDX-License-Identifier: GPL-3.0-or-later
//! Viste dell'esplorazione sui dati di una scansione vera, contro l'oracolo e contro query di riferimento
//! scritte a parte: File grandi e filtri della ricerca.

use std::collections::{BTreeSet, HashMap};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use quado_model::view::{self, Filter, Needle};
use quado_model::{Family, Tree};
use quado_scan_win::{Options, Progress, scan};
use quado_testkit::oracle;
use quado_testkit::trees::{self, TestDir};

fn filetime(t: SystemTime) -> i64 {
    let d = t.duration_since(UNIX_EPOCH).expect("dopo il 1970");
    (d.as_nanos() / 100) as i64 + 116_444_736_000_000_000
}

fn wide(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().collect()
}

#[test]
fn largest_files_match_the_oracle_order() {
    let d = TestDir::new("file-grandi");
    trees::small_files(&d.join("piccoli"), 300, 9000);
    for (k, mb) in [7usize, 3, 3, 12, 1, 5, 3].into_iter().enumerate() {
        std::fs::create_dir_all(d.join(&format!("g{k}"))).expect("cartella");
        trees::write_file(&d.join(&format!("g{k}/grande-{k}.bin")), mb << 20, k as u8);
    }
    trees::write_compressible(&d.join("comprimibile.txt"), 6 << 20);
    trees::ntfs_compress(&d.join("comprimibile.txt"));
    trees::sparse_file(&d.join("sparso.bin"), 64 << 20);
    let root = d.path.to_str().expect("percorso");
    let r = scan(root, Options::default(), &Progress::default()).expect("scansione");
    let o = oracle::walk(&d.path);
    // Riferimento: l'oracolo ordinato per allocata decrescente, poi per percorso in unità UTF-16.
    let mut expected: Vec<(u64, Vec<u16>)> = o.files.iter().map(|(p, f)| (f.alloc, p.clone())).collect();
    expected.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    for limit in [1, 10, 50, expected.len()] {
        let got: Vec<(u64, Vec<u16>)> = view::largest_files(&r.tree, &Filter::default(), limit, None).into_iter().map(|n| (r.tree.alloc(n), r.tree.path(n))).collect();
        assert_eq!(got, expected[..limit.min(expected.len())].to_vec(), "primi {limit}");
    }
}

/// Famiglia secondo l'estensione, scritta qui a parte per le sole estensioni del test.
fn reference_family(name: &str) -> Option<Family> {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())?;
    Some(match ext.as_str() {
        "jpg" | "png" => Family::Images,
        "mp4" => Family::Video,
        "pdf" | "txt" => Family::Documents,
        "zip" => Family::Archives,
        _ => Family::Other,
    })
}

#[test]
fn filters_match_a_reference_query() {
    let d = TestDir::new("filtri");
    let now = SystemTime::now();
    let day = Duration::from_secs(86_400);
    let files: [(&str, usize, u64); 10] = [
        ("foto/Foto mare.jpg", 3 << 20, 10),
        ("foto/foto montagna.png", 200 << 10, 400),
        ("foto/scansione.pdf", 1 << 20, 900),
        ("video/Viaggio.mp4", 9 << 20, 40),
        ("video/viaggio-corto.mp4", 300 << 10, 2000),
        ("documenti/Relazione FOTO.txt", 5 << 10, 5),
        ("documenti/archivio.zip", 2 << 20, 1200),
        ("documenti/note.txt", 900, 1),
        ("altro/dati.xyz", 4 << 20, 100),
        ("altro/foto.bak", 10 << 10, 30),
    ];
    for (rel, bytes, age_days) in files {
        let p = d.join(rel);
        std::fs::create_dir_all(p.parent().expect("cartella")).expect("cartella");
        trees::write_file(&p, bytes, 7);
        let f = std::fs::OpenOptions::new().write(true).open(&p).expect("apertura");
        f.set_modified(now - day * age_days as u32).expect("data");
    }
    let root = d.path.to_str().expect("percorso");
    let r = scan(root, Options::default(), &Progress::default()).expect("scansione");
    let t = &r.tree;
    let o = oracle::walk(&d.path);

    // Elementi di riferimento: file dall'oracolo, cartelle dalla somma dei file sotto di esse.
    struct Item {
        path: Vec<u16>,
        name: String,
        alloc: u64,
        mtime: i64,
        dir: bool,
    }
    let mut items: Vec<Item> = Vec::new();
    for (p, f) in &o.files {
        let pb = oracle::path_from_utf16(p);
        let name = pb.file_name().expect("nome").to_string_lossy().into_owned();
        let mtime = filetime(std::fs::metadata(&pb).expect("metadati").modified().expect("data"));
        items.push(Item { path: p.clone(), name, alloc: f.alloc, mtime, dir: false });
    }
    for e in walkdir(&d.path) {
        let w = wide(&e);
        let mut prefix = w.clone();
        prefix.push(u16::from(b'\\'));
        let alloc: u64 = o.files.iter().filter(|(p, _)| p.starts_with(&prefix)).map(|(_, f)| f.alloc).sum();
        let mtime = filetime(std::fs::metadata(&e).expect("metadati").modified().expect("data"));
        let name = e.file_name().expect("nome").to_string_lossy().into_owned();
        items.push(Item { path: w, name, alloc, mtime, dir: true });
    }

    let now_ft = filetime(now);
    let year = 365 * 864_000_000_000i64;
    let cases: Vec<(&str, Filter)> = vec![
        ("", Filter::default()),
        ("foto", Filter::default()),
        ("", Filter { min_alloc: Some(1 << 20), ..Default::default() }),
        ("", Filter { family: Some(Family::Video), ..Default::default() }),
        ("", Filter { modified_before: Some(now_ft - year), ..Default::default() }),
        ("", Filter { modified_after: Some(now_ft - 60 * 864_000_000_000), ..Default::default() }),
        ("foto", Filter { min_alloc: Some(100 << 10), family: Some(Family::Images), ..Default::default() }),
        ("viaggio", Filter { family: Some(Family::Video), modified_after: Some(now_ft - year), ..Default::default() }),
        ("", Filter { min_alloc: Some(2 << 20), modified_before: Some(now_ft - 50 * 864_000_000_000), ..Default::default() }),
    ];
    for (text, f) in cases {
        let got: BTreeSet<Vec<u16>> = view::search(t, Tree::ROOT, &Needle::new(text), &f, None).into_iter().map(|n| t.path(n)).collect();
        let want: BTreeSet<Vec<u16>> = items
            .iter()
            .filter(|i| text.is_empty() || i.name.to_lowercase().contains(text))
            .filter(|i| f.min_alloc.is_none_or(|m| i.alloc >= m))
            .filter(|i| f.family.is_none_or(|fam| !i.dir && reference_family(&i.name) == Some(fam)))
            .filter(|i| f.modified_after.is_none_or(|a| i.mtime >= a))
            .filter(|i| f.modified_before.is_none_or(|b| i.mtime < b))
            .map(|i| i.path.clone())
            .collect();
        let show = |s: &BTreeSet<Vec<u16>>| s.iter().map(|p| String::from_utf16_lossy(p)).collect::<Vec<_>>();
        assert_eq!(show(&got), show(&want), "testo {text:?}, filtro {f:?}");
    }
    let _ = HashMap::<u8, u8>::new();
}

/// Cartelle sotto `root`, esclusa la radice.
fn walkdir(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).expect("elenco").flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                out.push(e.path());
                stack.push(e.path());
            }
        }
    }
    out
}
