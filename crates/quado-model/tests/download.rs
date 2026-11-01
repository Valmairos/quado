// SPDX-License-Identifier: GPL-3.0-or-later
//! Lente Download nel modello (06 §3.1-3.3, F-DL-1, F-DL-3..5): famiglie, suffisso "(N)", file della
//! stessa dimensione, copie identiche, archivi con la cartella estratta. Alberi costruiti in memoria:
//! nessun file reale viene letto.

use std::time::Instant;

use proptest::prelude::*;
use quado_model::downloads::{
    self, ArchiveCheck, ArchiveEntry, ArchivePair, Hashed, IdenticalGroup, Lens, LensFamily, SizeGroup, compare_archive, copy_suffix,
    identical_groups, same_size_groups, with_keep,
};
use quado_model::tree::w;
use quado_model::{Flags, Name, NodeData, Tree};

fn dir() -> NodeData {
    NodeData { flags: Flags::DIR, ..Default::default() }
}

fn file(logical: u64) -> NodeData {
    NodeData { logical, alloc: logical.div_ceil(4096) * 4096, ..Default::default() }
}

fn with_flags(mut d: NodeData, f: Flags) -> NodeData {
    d.flags |= f;
    d
}

fn sealed(mut t: Tree) -> Tree {
    t.aggregate();
    t.seal();
    t
}

/// `C:\Users\demo` con `Downloads` e `Documents\Progetto`.
struct Profile {
    tree: Tree,
    downloads: u32,
    other: u32,
}

/// Un file per ogni famiglia, più un file che non è in nessuna famiglia mostrata, con dimensioni
/// diverse per famiglia (attese: tabella `EXPECTED`). Lo stesso insieme sta sotto `Downloads` e sotto
/// un'altra cartella dell'utente, con una sottocartella.
const FILES: [(&str, u64); 16] = [
    ("setup.exe", 1_000),
    ("pacchetto.MSI", 2_000),
    ("archivio.zip", 300),
    ("dati.tar.gz", 400),
    ("ubuntu.iso", 50_000),
    ("disco.vhdx", 60_000),
    ("film.mp4", 7_000),
    ("canzone.flac", 800),
    ("relazione.pdf", 90),
    ("foto.JPG", 11),
    ("Non confermato 123.crdownload", 12_000),
    ("video.mp4.part", 13_000),
    ("note.xyz", 7),
    ("LEGGIMI", 5),
    ("libreria.dll", 3),
    ("sottocartella/vecchio setup.exe", 100),
];

/// (famiglia, file, byte logici) attesi per l'insieme `FILES`.
const EXPECTED: [(LensFamily, u64, u64); 7] = [
    (LensFamily::Installers, 3, 3_100),
    (LensFamily::Archives, 2, 700),
    (LensFamily::DiskImages, 2, 110_000),
    (LensFamily::Media, 2, 7_800),
    (LensFamily::DocsImages, 2, 101),
    (LensFamily::Incomplete, 2, 25_000),
    (LensFamily::Other, 3, 15),
];

fn fill(t: &mut Tree, at: u32) {
    let mut sub = None;
    for (name, size) in FILES {
        match name.split_once('/') {
            Some((d, f)) => {
                let s = *sub.get_or_insert_with(|| t.push(at, &w(d), dir()));
                t.push(s, &w(f), file(size));
            }
            None => {
                t.push(at, &w(name), file(size));
            }
        }
    }
}

fn profile() -> Profile {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let users = t.push(Tree::ROOT, &w("Users"), dir());
    let u = t.push(users, &w("demo"), dir());
    let downloads = t.push(u, &w("Downloads"), dir());
    fill(&mut t, downloads);
    let docs = t.push(u, &w("Documents"), dir());
    let other = t.push(docs, &w("Progetto"), dir());
    fill(&mut t, other);
    Profile { tree: sealed(t), downloads, other }
}

fn assert_totals_consistent(t: &Tree, lens: &Lens) {
    assert_eq!(lens.totals.len(), 7);
    for (k, f) in LensFamily::ALL.iter().enumerate() {
        assert_eq!(lens.totals[k].family, *f);
    }
    assert_eq!(lens.totals.iter().map(|x| x.files).sum::<u64>(), lens.files, "file");
    assert_eq!(lens.totals.iter().map(|x| x.logical).sum::<u64>(), lens.logical, "logica");
    assert_eq!(lens.totals.iter().map(|x| x.alloc).sum::<u64>(), lens.alloc, "allocata");
    // Ogni file contato una volta, come nell'albero.
    assert_eq!((lens.files, lens.logical, lens.alloc), (u64::from(t.files(lens.root)), t.logical(lens.root), t.alloc(lens.root)));
}

#[test]
fn families_in_downloads_and_in_another_user_folder() {
    let p = profile();
    for root in [p.downloads, p.other] {
        let lens = downloads::build(&p.tree, root);
        assert_totals_consistent(&p.tree, &lens);
        for (f, files, logical) in EXPECTED {
            let got = lens.total(f);
            assert_eq!((got.files, got.logical), (files, logical), "{} in {}", f.code(), p.tree.path_lossy(root));
        }
        assert_eq!(lens.items.len(), FILES.len());
        assert!(lens.items.windows(2).all(|x| x[0].node < x[1].node), "in ordine di nodo");
        assert_eq!(lens.unread_dirs, 0);
    }
    // Le sei famiglie dei riquadri, nell'ordine di 04 §6.6, e i codici stabili.
    assert_eq!(LensFamily::SHOWN.map(LensFamily::code), ["installatori", "archivi", "immagini-disco", "video-audio", "documenti-immagini", "download-incompleti"]);
    for f in LensFamily::ALL {
        assert_eq!(LensFamily::from_code(f.code()), Some(f));
    }
}

#[test]
fn incomplete_downloads_are_only_crdownload_and_part() {
    let fam = |s: &str| LensFamily::of_name(Name::Ascii(s.as_bytes()));
    assert_eq!(fam("setup.exe.crdownload"), LensFamily::Incomplete);
    assert_eq!(fam("Non confermato 1.CRDOWNLOAD"), LensFamily::Incomplete);
    assert_eq!(fam("film.mkv.Part"), LensFamily::Incomplete);
    assert_eq!(fam("file.partial"), LensFamily::Other);
    assert_eq!(fam("file.download"), LensFamily::Other);
    assert_eq!(fam("file.tmp"), LensFamily::Other);
    assert_eq!(fam(".part"), LensFamily::Other);
    assert_eq!(fam("canzone.opus"), LensFamily::Media);
    assert_eq!(fam("scansione.png"), LensFamily::DocsImages);
    let wide: Vec<u16> = "Ünbestätigt.crdownload".encode_utf16().collect();
    assert_eq!(LensFamily::of_name(Name::Wide(&wide)), LensFamily::Incomplete);
}

#[test]
fn copy_suffix_cases() {
    let cases: [(&str, Option<u32>); 23] = [
        ("setup (1).exe", Some(1)),
        ("foto (12).jpg", Some(12)),
        ("archivio (2).tar.gz", Some(2)),
        ("archivio.tar (2).gz", Some(2)),
        ("archivio (2).tar.bz2", Some(2)),
        ("dati.abcdef (3).gz", Some(3)),
        ("script (4).user.js", Some(4)),
        ("LEGGIMI (3)", Some(3)),
        ("Copia di relazione (2).pdf", Some(2)),
        ("relazione (1) (2).pdf", Some(2)),
        ("Università è (7).docx", Some(7)),
        ("setup.exe", None),
        ("setup(1).exe", None),
        (" (1).exe", None),
        ("(1).exe", None),
        ("setup (0).exe", None),
        ("setup (01).exe", None),
        ("setup ().exe", None),
        ("setup (1a).exe", None),
        ("setup (-1).exe", None),
        ("setup (4294967295).exe", Some(u32::MAX)),
        ("setup (4294967296).exe", None),
        ("setup (99999999999999999999999).exe", None),
    ];
    for (name, want) in cases {
        assert_eq!(copy_suffix(name), want, "{name}");
    }
    assert_eq!(copy_suffix(""), None);
    assert_eq!(copy_suffix(".bashrc"), None);
}

#[test]
fn copy_suffix_is_only_a_filter() {
    // `setup (1).exe` con una dimensione diversa da `setup.exe` (F-DL-3, 06 §3.1).
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let dl = t.push(Tree::ROOT, &w("Downloads"), dir());
    let setup = t.push(dl, &w("setup.exe"), file(5_000_000));
    let copy = t.push(dl, &w("setup (1).exe"), file(5_000_123));
    let t = sealed(t);
    let lens = downloads::build(&t, dl);
    let filtered: Vec<u32> = lens.items.iter().filter(|i| i.copy_suffix.is_some()).map(|i| i.node).collect();
    assert_eq!(filtered, vec![copy], "il filtro lo trova");
    assert_eq!(lens.items.iter().find(|i| i.node == setup).and_then(|i| i.copy_suffix), None);
    // Dimensioni diverse: nessun gruppo, nemmeno con la stessa impronta.
    assert!(same_size_groups(&lens).is_empty());
    let h = |node, size| Hashed { node, size, digest: [7; 32], created: 1 };
    assert_eq!(identical_groups(&same_size_groups(&lens), &[h(setup, 5_000_000), h(copy, 5_000_123)]), (Vec::new(), true));
}

#[test]
fn different_names_with_identical_content_group_only_after_the_comparison() {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let dl = t.push(Tree::ROOT, &w("Downloads"), dir());
    let a = t.push(dl, &w("fattura marzo.pdf"), file(70_000));
    let b = t.push(dl, &w("scaricato.pdf"), file(70_000));
    let c = t.push(dl, &w("altro.pdf"), file(70_001));
    let t = sealed(t);
    let lens = downloads::build(&t, dl);
    let groups = same_size_groups(&lens);
    assert_eq!(groups, vec![SizeGroup { logical: 70_000, nodes: vec![a, b] }], "solo la stessa dimensione: {c} è fuori");
    // Prima del confronto: nessuna copia.
    assert_eq!(identical_groups(&groups, &[]), (Vec::new(), false));
    // Stessa impronta: un gruppo, si tiene il più vecchio per data di creazione.
    let same = [Hashed { node: a, size: 70_000, digest: [1; 32], created: 300 }, Hashed { node: b, size: 70_000, digest: [1; 32], created: 200 }];
    let (got, complete) = identical_groups(&groups, &same);
    assert!(complete);
    assert_eq!(got, vec![IdenticalGroup { logical: 70_000, nodes: vec![b, a], keep: b }]);
    // Impronte diverse: nessun gruppo, confronto completo.
    let diff = [Hashed { digest: [2; 32], ..same[0] }, same[1]];
    assert_eq!(identical_groups(&groups, &diff), (Vec::new(), true));
}

#[test]
fn keep_the_oldest_then_the_lowest_node_and_let_the_user_choose() {
    let g = SizeGroup { logical: 10, nodes: vec![4, 5, 6, 9] };
    let h = |node, created, d| Hashed { node, size: 10, digest: [d; 32], created };
    let hashed = [h(4, 50, 1), h(5, 10, 1), h(6, 10, 1), h(9, 5, 2)];
    let (got, complete) = identical_groups(std::slice::from_ref(&g), &hashed);
    assert!(complete);
    assert_eq!(got, vec![IdenticalGroup { logical: 10, nodes: vec![5, 6, 4], keep: 5 }], "a parità di data il nodo più basso");
    let chosen = with_keep(&got[0], 4).expect("nel gruppo");
    assert_eq!((chosen.keep, &chosen.nodes), (4, &got[0].nodes));
    assert_eq!(with_keep(&got[0], 9), None, "9 ha un altro contenuto");
    // Con più letture dello stesso file vale l'ultima.
    let reread = [h(4, 50, 1), h(5, 10, 1), h(6, 10, 1), h(9, 5, 2), h(9, 5, 1)];
    let (got, _) = identical_groups(&[g], &reread);
    assert_eq!(got[0].nodes, vec![9, 5, 6, 4]);
}

#[test]
fn a_comparison_cancelled_halfway_never_shows_a_partial_group() {
    let g1 = SizeGroup { logical: 1_000, nodes: vec![1, 2] };
    let g2 = SizeGroup { logical: 500, nodes: vec![3, 4, 5] };
    let h = |node, size| Hashed { node, size, digest: [0; 32], created: i64::from(node) };
    // Il primo gruppo letto per intero, il secondo a metà: tre file su cinque.
    let (got, complete) = identical_groups(&[g1.clone(), g2.clone()], &[h(1, 1_000), h(2, 1_000), h(3, 500)]);
    assert!(!complete);
    assert_eq!(got, vec![IdenticalGroup { logical: 1_000, nodes: vec![1, 2], keep: 1 }]);
    let cmp = downloads::compare_groups(&[g1.clone(), g2.clone()], &[h(1, 1_000), h(2, 1_000), h(3, 500)]);
    assert_eq!(cmp[1], downloads::GroupComparison::NotRead { read: 1 });
    // Un file cambiato dopo la scansione: il gruppo non conta, anche se tutto è stato letto.
    let (got, complete) = identical_groups(&[g1, g2], &[h(1, 1_000), h(2, 1_001), h(3, 500), h(4, 500), h(5, 500)]);
    assert!(complete);
    assert_eq!(got, vec![IdenticalGroup { logical: 500, nodes: vec![3, 4, 5], keep: 3 }]);
}

#[test]
fn size_groups_skip_empty_files_placeholders_and_incomplete_downloads() {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let dl = t.push(Tree::ROOT, &w("Downloads"), dir());
    let a = t.push(dl, &w("a.bin"), file(4096));
    let b = t.push(dl, &w("b.bin"), file(4096));
    t.push(dl, &w("cloud.bin"), with_flags(file(4096), Flags::CLOUD));
    t.push(dl, &w("x.crdownload"), file(4096));
    t.push(dl, &w("vuoto1.txt"), file(0));
    t.push(dl, &w("vuoto2.txt"), file(0));
    t.push(dl, &w("unico.bin"), file(123));
    let big1 = t.push(dl, &w("grande1.iso"), file(1 << 30));
    let big2 = t.push(dl, &w("grande2.iso"), file(1 << 30));
    let t = sealed(t);
    let lens = downloads::build(&t, dl);
    assert!(lens.items.iter().any(|i| i.placeholder));
    // Ordine: byte potenzialmente recuperabili decrescenti.
    assert_eq!(same_size_groups(&lens), vec![SizeGroup { logical: 1 << 30, nodes: vec![big1, big2] }, SizeGroup { logical: 4096, nodes: vec![a, b] }]);
}

#[test]
fn detached_nodes_and_links_not_followed_stay_out() {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let dl = t.push(Tree::ROOT, &w("Downloads"), dir());
    t.push(dl, &w("tenuto.exe"), file(10));
    let gone = t.push(dl, &w("riscansione"), dir());
    t.push(gone, &w("vecchio.exe"), file(20));
    // Junction con figli costruiti in memoria: la scansione vera non li avrebbe.
    let j = t.push(dl, &w("junction"), with_flags(dir(), Flags::REPARSE | Flags::NOT_FOLLOWED));
    t.push(j, &w("altrove.iso"), file(30));
    t.push(dl, &w("collegamento.exe"), with_flags(file(0), Flags::REPARSE | Flags::NOT_FOLLOWED));
    // Due nomi dello stesso file: uno solo conta nei totali, tutti e due sono voci.
    let primary = t.push(dl, &w("a.zip"), with_flags(file(500), Flags::MULTI_LINK));
    let secondary = t.push(dl, &w("b.zip"), with_flags(file(500), Flags::MULTI_LINK | Flags::SECONDARY_LINK));
    let denied = t.push(dl, &w("negata"), with_flags(dir(), Flags::DENIED));
    let mut t = sealed(t);
    t.detach(gone);
    let lens = downloads::build(&t, dl);
    let nodes: Vec<String> = lens.items.iter().map(|i| t.name_lossy(i.node)).collect();
    assert_eq!(nodes, ["tenuto.exe", "a.zip", "b.zip"]);
    assert_eq!(lens.total(LensFamily::Archives).files, 1);
    assert_eq!(lens.total(LensFamily::Archives).logical, 500);
    assert_eq!((lens.files, lens.logical), (2, 510));
    assert_eq!(lens.unread_dirs, 1, "{}", t.name_lossy(denied));
    // Lo stesso file con due nomi non è una copia: il nome secondario non entra nei gruppi da confrontare
    // (toglierne uno non libera spazio), quindi il primario resta solo e non c'è nessun gruppo.
    assert!(same_size_groups(&lens).is_empty());
    assert!(lens.items.iter().any(|i| i.node == secondary && i.secondary));
    assert!(lens.items.iter().any(|i| i.node == primary && !i.secondary));
    // Una lente su un nodo fuori dall'albero o staccato è vuota, senza panic.
    assert!(downloads::build(&t, 9_999).items.is_empty());
    assert!(downloads::build(&t, gone).items.is_empty());
}

// ---------------------------------------------------------------------------------------------
// Archivi (F-DL-5, 06 §3.3)

fn entry(path: &str, size: u64) -> ArchiveEntry {
    ArchiveEntry { path: path.into(), size, dir: false }
}

fn dir_entry(path: &str) -> ArchiveEntry {
    ArchiveEntry { path: path.into(), size: 0, dir: true }
}

/// `Downloads` con `Progetto.zip` e la cartella `Progetto` estratta: `a.txt`, `src\main.rs`,
/// `src\lib.rs`, `img\logo.png`.
fn extracted() -> (Tree, u32, u32, u32) {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let dl = t.push(Tree::ROOT, &w("Downloads"), dir());
    let zip = t.push(dl, &w("Progetto.zip"), file(9_000));
    let folder = t.push(dl, &w("progetto"), dir());
    t.push(folder, &w("a.txt"), file(10));
    let src = t.push(folder, &w("src"), dir());
    t.push(src, &w("main.rs"), file(200));
    t.push(src, &w("lib.rs"), file(300));
    let img = t.push(folder, &w("IMG"), dir());
    t.push(img, &w("logo.png"), file(4_000));
    (sealed(t), dl, zip, folder)
}

fn full_entries() -> Vec<ArchiveEntry> {
    vec![dir_entry("src/"), entry("a.txt", 10), entry("src/main.rs", 200), entry("src/lib.rs", 300), dir_entry("img/"), entry("img/logo.png", 4_000)]
}

fn check(total: u64, present: u64, different_size: u64, missing: u64, unreadable: u64) -> ArchiveCheck {
    ArchiveCheck { total, present, different_size, missing, unreadable }
}

#[test]
fn archive_pairs_are_only_zips_with_a_sibling_folder() {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let dl = t.push(Tree::ROOT, &w("Downloads"), dir());
    let zip = t.push(dl, &w("Foto Vacanze.ZIP"), file(100));
    let folder = t.push(dl, &w("foto vacanze"), dir());
    // Altri archivi con la loro cartella: restano nella famiglia, senza confronto.
    t.push(dl, &w("sorgenti.7z"), file(100));
    t.push(dl, &w("sorgenti"), dir());
    t.push(dl, &w("dati.tar.gz"), file(100));
    t.push(dl, &w("dati.tar"), dir());
    // Zip senza cartella, zip con un file (non una cartella) dello stesso nome, cartella altrove.
    t.push(dl, &w("solo.zip"), file(100));
    t.push(dl, &w("file.zip"), file(100));
    t.push(dl, &w("file"), file(1));
    let sub = t.push(dl, &w("sotto"), dir());
    t.push(sub, &w("lontano"), dir());
    t.push(dl, &w("lontano.zip"), file(100));
    // Junction con il nome dello zip: il suo contenuto non è nella scansione.
    t.push(dl, &w("collegato.zip"), file(100));
    t.push(dl, &w("collegato"), with_flags(dir(), Flags::REPARSE | Flags::NOT_FOLLOWED));
    // Nomi non ASCII, senza distinzione di maiuscole.
    let zw = t.push(sub, &w("Università.zip"), file(100));
    let fw = t.push(sub, &w("UNIVERSITÀ"), dir());
    let t = sealed(t);
    let lens = downloads::build(&t, dl);
    assert_eq!(downloads::archive_pairs(&t, &lens), vec![ArchivePair { archive: zip, folder }, ArchivePair { archive: zw, folder: fw }]);
}

#[test]
fn archive_against_complete_partial_and_different_folders() {
    let (t, _, _, folder) = extracted();
    // Completa, con sottocartelle, maiuscole diverse e voci di cartella che non contano.
    assert_eq!(compare_archive(&t, folder, &full_entries()), check(4, 4, 0, 0, 0));
    // Parziale: due voci in più nell'archivio.
    let mut more = full_entries();
    more.push(entry("src/extra.rs", 5));
    more.push(entry("docs/guida.pdf", 6));
    assert_eq!(compare_archive(&t, folder, &more), check(6, 4, 0, 2, 0));
    // Dimensioni diverse.
    let mut diff = full_entries();
    diff[2].size = 201;
    assert_eq!(compare_archive(&t, folder, &diff), check(4, 3, 1, 0, 0));
    // `\` come separatore.
    assert_eq!(compare_archive(&t, folder, &[entry(r"src\MAIN.RS", 200)]), check(1, 1, 0, 0, 0));
    // Un file al posto di una cartella e una cartella al posto di un file.
    assert_eq!(compare_archive(&t, folder, &[entry("a.txt/x", 1), entry("src", 1)]), check(2, 0, 0, 2, 0));
    // Archivio vuoto o con sole cartelle.
    assert_eq!(compare_archive(&t, folder, &[]), ArchiveCheck::default());
    assert_eq!(compare_archive(&t, folder, &[dir_entry("src/"), entry("img/", 0)]), ArchiveCheck::default());
}

#[test]
fn invalid_paths_count_as_missing_and_never_leave_the_folder() {
    let (t, _, zip, folder) = extracted();
    let bad = [
        entry("../Progetto.zip", 9_000),
        entry("src/../a.txt", 10),
        entry("./a.txt", 10),
        entry("/a.txt", 10),
        entry(r"\a.txt", 10),
        entry("C:/Users/demo/Downloads/Progetto.zip", 9_000),
        entry("C:a.txt", 10),
        entry("src//main.rs", 200),
        entry("", 0),
        entry("a.txt:stream", 10),
    ];
    assert_eq!(compare_archive(&t, folder, &bad), check(10, 0, 0, 10, 0), "{}", t.name_lossy(zip));
    // Una cartella che non è nell'albero o che non è una cartella: tutto mancante, senza panic.
    assert_eq!(compare_archive(&t, 9_999, &full_entries()), check(4, 0, 0, 4, 0));
    assert_eq!(compare_archive(&t, zip, &full_entries()), check(4, 0, 0, 4, 0));
}

#[test]
fn extract_all_prefix_is_removed_only_when_the_folder_lacks_it() {
    // Archivio con tutto sotto `Progetto/`, estratto accanto: la cartella non ha `Progetto` dentro.
    let (t, _, _, folder) = extracted();
    let prefixed: Vec<ArchiveEntry> = std::iter::once(dir_entry("Progetto/"))
        .chain(full_entries().into_iter().map(|e| ArchiveEntry { path: format!("Progetto/{}", e.path), ..e }))
        .collect();
    assert_eq!(compare_archive(&t, folder, &prefixed), check(4, 4, 0, 0, 0));
    // "Estrai tutto": la cartella doppia `progetto\Progetto\...`; il prefisso resta.
    let mut t2 = Tree::with_root(&w("C:\\"), 0);
    let dl = t2.push(Tree::ROOT, &w("Downloads"), dir());
    t2.push(dl, &w("Progetto.zip"), file(9_000));
    let outer = t2.push(dl, &w("Progetto"), dir());
    let inner = t2.push(outer, &w("PROGETTO"), dir());
    t2.push(inner, &w("a.txt"), file(10));
    let src = t2.push(inner, &w("src"), dir());
    t2.push(src, &w("main.rs"), file(200));
    let t2 = sealed(t2);
    let entries = [dir_entry("Progetto/"), entry("Progetto/a.txt", 10), entry("Progetto/src/main.rs", 200), entry("Progetto/src/lib.rs", 300)];
    assert_eq!(compare_archive(&t2, outer, &entries), check(3, 2, 0, 1, 0));
    // Non tutte le voci sotto il prefisso: niente da togliere.
    let mixed = [entry("Progetto/a.txt", 10), entry("a.txt", 10)];
    assert_eq!(compare_archive(&t, folder, &mixed), check(2, 1, 0, 1, 0));
    // Un file con il nome del prefisso non è "sotto" il prefisso.
    assert_eq!(compare_archive(&t, folder, &[entry("Progetto", 1), entry("Progetto/a.txt", 10)]), check(2, 0, 0, 2, 0));
}

#[test]
fn entries_in_unreadable_folders_are_neither_present_nor_missing() {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let dl = t.push(Tree::ROOT, &w("Downloads"), dir());
    let folder = t.push(dl, &w("estratto"), dir());
    t.push(folder, &w("a.txt"), file(10));
    t.push(folder, &w("negata"), with_flags(dir(), Flags::DENIED));
    let partial = t.push(folder, &w("parziale"), with_flags(dir(), Flags::PARTIAL));
    t.push(partial, &w("letto.txt"), file(1));
    t.push(folder, &w("junction"), with_flags(dir(), Flags::REPARSE | Flags::NOT_FOLLOWED));
    t.push(folder, &w("nuvola"), with_flags(dir(), Flags::CLOUD | Flags::CLOUD_NOT_LISTED));
    let t = sealed(t);
    let entries = [
        entry("a.txt", 10),
        entry("negata/x.txt", 1),
        entry("parziale/letto.txt", 1),
        entry("parziale/non-letto.txt", 1),
        entry("junction/y.txt", 1),
        entry("nuvola/z.txt", 1),
        entry("assente/w.txt", 1),
    ];
    assert_eq!(compare_archive(&t, folder, &entries), check(7, 2, 0, 1, 4));
}

#[test]
fn many_entries_use_the_child_index() {
    // 50.000 voci contro una cartella con 50.000 file: un indice dei figli, non ricerche lineari.
    let n = 50_000u64;
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let dl = t.push(Tree::ROOT, &w("Downloads"), dir());
    let folder = t.push(dl, &w("grande"), dir());
    for k in 0..n {
        t.push(folder, &w(&format!("file{k}.dat")), file(k));
    }
    let t = sealed(t);
    let entries: Vec<ArchiveEntry> = (0..n).map(|k| entry(&format!("FILE{k}.DAT"), k)).collect();
    let start = Instant::now();
    let got = compare_archive(&t, folder, &entries);
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    println!("compare_archive con {n} voci: {ms:.1} ms");
    assert_eq!(got, check(n, n, 0, 0, 0));
    assert!(ms < 5_000.0, "{ms} ms");
}

#[test]
fn lens_on_100_000_files_is_fast() {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let dl = t.push(Tree::ROOT, &w("Downloads"), dir());
    let exts = ["exe", "zip", "iso", "mp4", "pdf", "jpg", "crdownload", "xyz", "tar.gz"];
    let mut at = dl;
    for k in 0..100_000u64 {
        if k % 10_000 == 0 {
            at = t.push(dl, &w(&format!("cartella {k}")), dir());
        }
        let suffix = if k % 7 == 0 { format!(" ({})", k % 5 + 1) } else { String::new() };
        let name = format!("file {k}{suffix}.{}", exts[(k % exts.len() as u64) as usize]);
        t.push(at, &w(&name), file(k % 3_000));
    }
    let t = sealed(t);
    let start = Instant::now();
    let lens = downloads::build(&t, dl);
    let built = start.elapsed().as_secs_f64() * 1000.0;
    let groups = same_size_groups(&lens);
    let pairs = downloads::archive_pairs(&t, &lens);
    let all = start.elapsed().as_secs_f64() * 1000.0;
    println!("lente su 100.000 file: {built:.1} ms; con gruppi e coppie: {all:.1} ms");
    assert_eq!(lens.items.len(), 100_000);
    assert_totals_consistent(&t, &lens);
    assert!(!groups.is_empty() && pairs.is_empty());
    assert!(built < 3_000.0, "{built} ms");
}

// ---------------------------------------------------------------------------------------------
// Proprietà (06 §3.1, §3.2)

const STEMS: [&str; 6] = ["setup", "foto", "archivio", "film", "Relazione è", "dati"];
const EXTS: [&str; 13] = ["exe", "msi", "zip", "iso", "mp4", "mp3", "pdf", "jpg", "crdownload", "part", "xyz", "", "tar.gz"];

/// Un file generato: cartella (0 = la radice della lente), nome, estensione, dimensione, flag, suffisso.
#[derive(Clone, Debug)]
struct Spec {
    folder: usize,
    stem: usize,
    ext: usize,
    size: u64,
    cloud: bool,
    links: bool,
    suffix: Option<u32>,
}

fn spec() -> impl Strategy<Value = Spec> {
    (0usize..4, 0..STEMS.len(), 0..EXTS.len(), prop_oneof![Just(0u64), Just(1_000), Just(4_096), Just(70_000), 1u64..200], any::<bool>(), 0u8..10, proptest::option::of(1u32..200))
        .prop_map(|(folder, stem, ext, size, cloud, links, suffix)| Spec { folder, stem, ext, size, cloud: cloud && links % 3 == 0, links: links == 1, suffix })
}

/// Albero con `Downloads` e tre sottocartelle; con `suffixed` i nomi portano " (N)" prima
/// dell'estensione come lo aggiunge Chromium. Gli indici dei nodi non cambiano.
fn build_tree(specs: &[Spec], suffixed: bool) -> (Tree, u32) {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let dl = t.push(Tree::ROOT, &w("Downloads"), dir());
    let subs: Vec<u32> = (0..3).map(|k| t.push(dl, &w(&format!("sotto{k}")), dir())).collect();
    for (k, s) in specs.iter().enumerate() {
        let at = if s.folder == 0 { dl } else { subs[s.folder - 1] };
        let sfx = match (suffixed, s.suffix) {
            (true, Some(n)) => format!(" ({n})"),
            _ => String::new(),
        };
        let name = if EXTS[s.ext].is_empty() { format!("{}{k}{sfx}", STEMS[s.stem]) } else { format!("{}{k}{sfx}.{}", STEMS[s.stem], EXTS[s.ext]) };
        let mut flags = Flags::default();
        if s.cloud {
            flags |= Flags::CLOUD;
        }
        if s.links {
            flags |= Flags::MULTI_LINK;
        }
        t.push(at, &w(&name), NodeData { logical: s.size, alloc: s.size.div_ceil(512) * 512, flags, ..Default::default() });
    }
    (sealed(t), dl)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn family_totals_add_up_to_the_lens_and_to_the_tree(specs in proptest::collection::vec(spec(), 0..60)) {
        let (t, dl) = build_tree(&specs, true);
        for root in std::iter::once(dl).chain(t.children(dl)) {
            let lens = downloads::build(&t, root);
            assert_totals_consistent(&t, &lens);
            prop_assert_eq!(lens.items.len() as u64, u64::from(t.files(root)));
        }
    }

    #[test]
    fn the_copy_suffix_never_changes_groups(
        specs in proptest::collection::vec(spec(), 0..60),
        digests in proptest::collection::vec(0u8..3, 60),
        read in proptest::collection::vec(any::<bool>(), 60),
    ) {
        let (plain, dl) = build_tree(&specs, false);
        let (suffixed, _) = build_tree(&specs, true);
        let a = downloads::build(&plain, dl);
        let b = downloads::build(&suffixed, dl);
        // Il filtro vede il suffisso, nient'altro cambia.
        for (x, y) in a.items.iter().zip(&b.items) {
            prop_assert_eq!(x.copy_suffix, None);
            prop_assert_eq!(y.copy_suffix, specs[(y.node - 5) as usize].suffix);
            prop_assert_eq!(downloads::LensItem { copy_suffix: None, ..*y }, *x);
        }
        prop_assert_eq!(&a.totals, &b.totals);
        let ga = same_size_groups(&a);
        prop_assert_eq!(&ga, &same_size_groups(&b));
        let hashed: Vec<Hashed> = a.items.iter().enumerate().filter(|(k, _)| read[k % 60]).map(|(k, i)| Hashed { node: i.node, size: i.logical, digest: [digests[k % 60]; 32], created: i64::from(i.node % 7) }).collect();
        prop_assert_eq!(identical_groups(&ga, &hashed), identical_groups(&same_size_groups(&b), &hashed));
        prop_assert_eq!(downloads::archive_pairs(&plain, &a), downloads::archive_pairs(&suffixed, &b));
    }

    #[test]
    fn identical_groups_never_show_an_incomplete_size_group(
        groups in proptest::collection::vec((1u64..5, 2usize..6), 1..6),
        hashed in proptest::collection::vec((0u32..30, 0u64..6, 0u8..3, 0i64..4), 0..40),
    ) {
        // Gruppi di nodi consecutivi; letture casuali, anche con dimensioni cambiate e ripetute.
        let mut next = 0u32;
        let candidates: Vec<SizeGroup> = groups.iter().map(|&(size, n)| {
            let nodes = (next..next + n as u32).collect();
            next += n as u32;
            SizeGroup { logical: size, nodes }
        }).collect();
        let hashed: Vec<Hashed> = hashed.iter().map(|&(node, size, d, created)| Hashed { node, size, digest: [d; 32], created }).collect();
        let last = |n: u32| hashed.iter().rev().find(|h| h.node == n);
        let (got, complete) = identical_groups(&candidates, &hashed);
        prop_assert_eq!(complete, candidates.iter().all(|g| g.nodes.iter().all(|&n| last(n).is_some())));
        for ig in &got {
            prop_assert!(ig.nodes.len() >= 2);
            prop_assert_eq!(ig.keep, ig.nodes[0]);
            let g = candidates.iter().find(|g| g.nodes.contains(&ig.nodes[0])).expect("gruppo di dimensione");
            // Il gruppo di dimensione da cui viene è stato letto per intero, con la dimensione della scansione.
            for &n in &g.nodes {
                let h = last(n);
                prop_assert!(h.is_some_and(|h| h.size == g.logical), "{n} non letto o cambiato");
            }
            let d = last(ig.nodes[0]).map(|h| h.digest);
            for &n in &ig.nodes {
                prop_assert!(g.nodes.contains(&n));
                prop_assert_eq!(last(n).map(|h| h.digest), d);
            }
            // Per data di creazione, poi per nodo.
            let key: Vec<(i64, u32)> = ig.nodes.iter().map(|&n| (last(n).map_or(0, |h| h.created), n)).collect();
            prop_assert!(key.windows(2).all(|p| p[0] < p[1]));
        }
    }

    #[test]
    fn archive_counts_always_add_up(
        paths in proptest::collection::vec(("[a-cA-C./\\\\:]{0,12}", 0u64..400, any::<bool>()), 0..30),
    ) {
        let (t, _, _, folder) = extracted();
        let entries: Vec<ArchiveEntry> = paths.into_iter().map(|(path, size, dir)| ArchiveEntry { path, size, dir }).collect();
        let c = compare_archive(&t, folder, &entries);
        prop_assert_eq!(c.total, c.present + c.different_size + c.missing + c.unreadable);
        prop_assert_eq!(c.total, entries.iter().filter(|e| !e.dir && !e.path.ends_with(['/', '\\'])).count() as u64);
    }
}
