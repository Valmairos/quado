// SPDX-License-Identifier: GPL-3.0-or-later
//! Prove dell'interfaccia contro Windows vero:
//! - "Rileggi dal disco" su un file aperto in scrittura che cresce, senza cambiare la data di ultimo accesso;
//! - dimensioni scritte come Esplora file (`StrFormatByteSizeEx`) nel formato di questo PC;
//! - percorsi oltre 260 caratteri, con il nome breve come ripiego per la shell.

use std::io::Write as _;
use std::os::windows::fs::OpenOptionsExt;
use std::time::Duration;

use quado_actions::shell;
use quado_model::{Flags, Tree};
use quado_scan_win::{Options, Progress, scan, system};
use quado_testkit::oracle;
use quado_testkit::trees::{self, TestDir};
use quado_ui::app::data::{self, Loaded, VolumeInfo, Verified};
use quado_ui::format::{Formatter, UnitLabels, Units};
use quado_ui::i18n::Lingue;

#[test]
fn verify_reads_a_file_that_grew_while_open() {
    let d = TestDir::new("verifica");
    let path = d.join("registro.log");
    // Aperto in scrittura e lasciato aperto, con la condivisione che lascia leggere gli attributi.
    let mut f = std::fs::OpenOptions::new().create(true).truncate(true).write(true).share_mode(7).open(&path).expect("apertura");
    f.write_all(&vec![1u8; 300_000]).expect("scrittura");
    f.flush().expect("flush");
    trees::small_files(&d.join("altro"), 20, 5000);
    let root = d.path.to_str().expect("percorso").to_owned();
    let r = scan(&root, Options::default(), &Progress::default()).expect("scansione");
    let volume = VolumeInfo { root: root.clone(), label: String::new(), file_system: "NTFS".into(), kind: quado_scan_win::volumes::DriveKind::Fixed, support: quado_scan_win::volumes::Support::Full, is_system: false };
    let mut loaded = Loaded::new(volume, r, Duration::ZERO, None);
    let node = (0..loaded.tree().len() as u32).find(|&i| loaded.tree().name_lossy(i) == "registro.log").expect("nodo");
    // Il file cresce dopo la scansione, mentre è ancora aperto.
    f.write_all(&vec![2u8; 5_000_000]).expect("scrittura");
    f.flush().expect("flush");
    let accessed_before = std::fs::metadata(&path).expect("metadati").accessed().expect("accesso");
    std::thread::sleep(Duration::from_millis(50));
    let v = data::spawn_verify(path.to_str().expect("percorso").to_owned(), node, false).wait().expect("esito");
    assert!(matches!(v, Verified::File { .. }));
    let report = data::apply_verified(&mut loaded, v, None);
    let o = oracle::walk(&d.path);
    let wanted = o.files.iter().find(|(p, _)| String::from_utf16_lossy(p).ends_with("registro.log")).map(|(_, f)| f.clone()).expect("oracolo");
    let t = loaded.tree();
    assert_eq!((t.logical(report.node), t.alloc(report.node)), (wanted.logical, wanted.alloc), "dopo la verifica coincide con l'oracolo");
    assert_eq!(t.logical(report.node), 5_300_000);
    // Gli antenati sono aggiornati: la radice vale la somma dell'oracolo.
    assert_eq!(t.alloc(Tree::ROOT), o.unique_alloc);
    let accessed_after = std::fs::metadata(&path).expect("metadati").accessed().expect("accesso");
    assert_eq!(accessed_before, accessed_after, "la verifica non cambia la data di ultimo accesso");
    drop(f);
}

#[test]
fn verify_rescans_a_folder_and_keeps_totals() {
    let d = TestDir::new("verifica-cartella");
    trees::small_files(&d.join("a"), 30, 8000);
    trees::small_files(&d.join("b"), 30, 8000);
    let root = d.path.to_str().expect("percorso").to_owned();
    let r = scan(&root, Options::default(), &Progress::default()).expect("scansione");
    let volume = VolumeInfo { root: root.clone(), label: String::new(), file_system: "NTFS".into(), kind: quado_scan_win::volumes::DriveKind::Fixed, support: quado_scan_win::volumes::Support::Full, is_system: false };
    let mut loaded = Loaded::new(volume, r, Duration::ZERO, None);
    let a = (0..loaded.tree().len() as u32).find(|&i| loaded.tree().name_lossy(i) == "a").expect("cartella a");
    // Cambia il contenuto di "a" dopo la scansione.
    trees::small_files(&d.join("a/nuova"), 25, 20_000);
    let v = data::spawn_verify(d.join("a").to_str().expect("percorso").to_owned(), a, true).wait().expect("esito");
    let report = data::apply_verified(&mut loaded, v, None);
    assert!(loaded.tree().flags(a).has(Flags::DETACHED));
    let o = oracle::walk(&d.path);
    assert_eq!(loaded.tree().alloc(Tree::ROOT), o.unique_alloc, "totale come una scansione nuova");
    assert_eq!(loaded.tree().name_lossy(report.node), "a");
    // Gli indici delle viste seguono il modello.
    assert!(loaded.ord().children(Tree::ROOT).contains(&report.node));
}

#[test]
fn sizes_are_written_like_file_explorer() {
    let lingue = Lingue::load("it", None);
    let f = Formatter { region: system::region_format(), units: Units::Explorer, labels: UnitLabels::from(&lingue) };
    let mut sizes: Vec<u64> = vec![1024, 1536, 4096, 10_240, 1_000_000, 1_048_575, 1_048_576, 14_806_917_939, 476 << 30, 2 << 40, 1_234_567_890];
    let mut x = 1023u64;
    while x < 1 << 50 {
        sizes.push(x);
        sizes.push(x + x / 3);
        x = x * 7 / 3;
    }
    let mut different = Vec::new();
    for s in sizes {
        let Some(shell) = shell::explorer_size_text(s) else { continue };
        if s < 1024 {
            continue;
        }
        let ours = f.size(s);
        if ours != shell {
            different.push(format!("{s}: Quado «{ours}», Esplora «{shell}»"));
        }
    }
    assert!(different.is_empty(), "{}", different.join("\n"));
}

#[test]
fn long_paths_have_a_shell_candidate() {
    let d = TestDir::new("percorso-lungo");
    let dir = trees::long_path(&d.path, 300);
    let file = dir.join("documento.txt");
    std::fs::write(&file, b"prova").expect("file");
    let p = file.to_str().expect("percorso");
    assert!(p.len() > 260);
    let c = shell::path_candidates(p);
    assert_eq!(c[0], p);
    // Con i nomi brevi 8.3 attivi sul volume c'è un secondo candidato sotto i 260 caratteri.
    if c.len() > 1 {
        assert!(c[1].len() < 260, "{}", c[1]);
        assert!(std::path::Path::new(&c[1]).exists());
    } else {
        eprintln!("nomi brevi 8.3 non disponibili su questo volume: resta solo il percorso lungo");
    }
}

/// Controllo manuale con data: apre davvero Esplora file e Proprietà su un percorso oltre i
/// 260 caratteri, poi chiude la finestra di Esplora. `cargo test -p quado-ui --test windows -- --ignored`.
#[test]
#[ignore = "apre finestre di Windows: controllo manuale"]
fn reveal_and_properties_on_a_long_path() {
    let d = TestDir::new("apri-percorso-lungo");
    let dir = trees::long_path(&d.path, 300);
    let file = dir.join("documento.txt");
    std::fs::write(&file, b"prova").expect("file");
    let p = file.to_str().expect("percorso").to_owned();
    println!("percorso di {} caratteri", p.len());
    println!("Apri in Esplora file: {:?}", shell::reveal(&p));
    println!("Proprietà: {:?}", shell::properties(&p));
    std::thread::sleep(Duration::from_secs(4));
    // Chiude le finestre di Esplora aperte sulla cartella di prova.
    let close = "(New-Object -ComObject Shell.Application).Windows() | Where-Object { $_.LocationURL -like '*apri-percorso-lungo*' -or $_.LocationName -like 'cartella-con-un-nome*' } | ForEach-Object { $_.Quit() }";
    let _ = std::process::Command::new("powershell").args(["-NoProfile", "-Command", close]).status();
}
