// SPDX-License-Identifier: GPL-3.0-or-later
//! Elementi protetti (03 F-AC-4, 05 D7 controllo 1, 06 §5.2): radici dalle prove di prova
//! (`tests/common`, fixture sintetiche con la forma di un PC vero) e dalla vista Programmi costruita sulle
//! stesse fixture, più voci di disinstallazione aggiunte per i casi limite.

mod common;

use std::collections::HashSet;

use quado_kb::evidence::Evidence;
use quado_kb::extract::UninstallEntry;
use quado_kb::programs::{self, Location, Programs};
use quado_kb::protection::protected_roots;
use quado_kb::{Kb, classify};
use quado_model::intervention::{ProtectedKind, ProtectedRoot};
use quado_model::record::{Date, Hive};

const TODAY: Date = Date { year: 2026, month: 10, day: 13 };
const ALBERI: [&str; 4] = ["radice.albero", "profilo.albero", "capcut.albero", "giochi.albero"];

fn programs_for(ev: &Evidence) -> Programs {
    let kb = Kb::embedded().expect("regole");
    let tree = common::tree(&ALBERI);
    let cls = classify(&kb, &tree, ev, TODAY);
    programs::build(&tree, ev, &kb, &cls)
}

fn entry(name: &str, location: &str, system_component: bool) -> UninstallEntry {
    UninstallEntry {
        hive: Hive::Machine,
        key: name.into(),
        display_name: name.into(),
        version: None,
        publisher: None,
        install_location: Some(location.into()),
        uninstall_string: None,
        display_icon: None,
        install_date: None,
        estimated_kb: None,
        system_component,
    }
}

/// Prove del PC delle fixture con voci in più: una cartella dichiarata ma assente dalla scansione, una voce
/// nascosta, una su un altro volume, una che indica il profilo (non plausibile) e una che coincide con
/// `Windows\Installer` scritta in minuscolo.
fn evidence() -> Evidence {
    let mut ev = common::evidence();
    let u = ev.uninstall.as_mut().expect("voci di disinstallazione");
    u.entries.push(entry("Assente", r"C:\Program Files\Assente", false));
    u.entries.push(entry("Nascosto", r"C:\Program Files\Nascosto\", true));
    u.entries.push(entry("Altro volume", r"D:\Giochi\Gioco", false));
    u.entries.push(entry("Profilo", r"C:\Users\demo", false));
    u.entries.push(entry("Installer", r"c:\windows\installer", false));
    ev
}

fn root(path: &str, kind: ProtectedKind) -> ProtectedRoot {
    ProtectedRoot { path: path.into(), kind }
}

fn find<'a>(roots: &'a [ProtectedRoot], path: &str) -> Vec<&'a ProtectedRoot> {
    roots.iter().filter(|r| r.path.eq_ignore_ascii_case(path)).collect()
}

fn assert_well_formed(roots: &[ProtectedRoot], volume: &str) {
    let mut seen = HashSet::new();
    for r in roots {
        assert!(r.path.to_lowercase().starts_with(&volume.to_lowercase()) && r.path.len() > volume.len(), "{} fuori dal volume {volume}", r.path);
        assert!(!r.path.ends_with('\\'), "{} con la barra finale", r.path);
        assert!(!r.path.contains('/'), "{}", r.path);
        assert!(seen.insert(r.path.to_lowercase()), "{} doppio", r.path);
    }
}

#[test]
fn system_roots_come_first_with_windows_paths() {
    let ev = evidence();
    let roots = protected_roots(&ev, &programs_for(&ev));
    assert_eq!(
        roots[..7],
        [
            root(r"C:\Windows", ProtectedKind::WindowsDir),
            root(r"C:\Windows\WinSxS", ProtectedKind::WinSxS),
            root(r"C:\Windows\Installer", ProtectedKind::WindowsInstaller),
            root(r"C:\System Volume Information", ProtectedKind::SystemVolumeInformation),
            root(r"C:\pagefile.sys", ProtectedKind::PagingFile),
            root(r"C:\hiberfil.sys", ProtectedKind::Hibernation),
            root(r"C:\swapfile.sys", ProtectedKind::SwapFile),
        ]
    );
    assert!(roots[7..].iter().all(|r| r.kind == ProtectedKind::RegisteredProgram));
    assert_well_formed(&roots, "C:\\");
}

#[test]
fn registered_program_folders_on_the_volume() {
    let ev = evidence();
    let p = programs_for(&ev);
    let roots = protected_roots(&ev, &p);
    // Ogni cartella di programma sul volume è una radice (di programma, o già di sistema).
    let mut folders = 0;
    for item in &p.items {
        if let Location::Folder { path, .. } = &item.location
            && path.to_lowercase().starts_with("c:\\")
        {
            folders += 1;
            assert_eq!(find(&roots, path).len(), 1, "{} ({}) non protetta", path, item.name);
        }
    }
    // 20 cartelle, 17 radici: tre prodotti di Epic nella stessa cartella e `Windows\Installer`, che è già
    // una radice di sistema.
    assert_eq!(folders, 20);
    assert_eq!(roots.iter().filter(|r| r.kind == ProtectedKind::RegisteredProgram).count(), 17, "{roots:#?}");
    // Ogni radice di programma viene da una cartella di programma.
    for r in roots.iter().filter(|r| r.kind == ProtectedKind::RegisteredProgram) {
        assert!(
            p.items.iter().any(|i| matches!(&i.location, Location::Folder { path, .. } if path.eq_ignore_ascii_case(&r.path))),
            "{} senza programma",
            r.path
        );
    }
    let program = |path: &str| assert_eq!(find(&roots, path), [&root(path, ProtectedKind::RegisteredProgram)], "{path}");
    // Indizio di `UninstallString` (CapCut in HKCU), Steam, gioco di Steam, Epic, MSIX, voce nascosta.
    program(r"C:\Users\demo\AppData\Local\CapCut\Apps");
    program(r"C:\Program Files (x86)\Steam");
    program(r"C:\Program Files (x86)\Steam\steamapps\common\Gioco di Prova");
    program(r"C:\Program Files\Epic Games\UE_5.5");
    program(r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_1.23.0.0_x64__8wekyb3d8bbwe");
    program(r"C:\Program Files\Nascosto");
    // Dichiarata ma assente dalla scansione: resta protetta.
    assert!(p.items.iter().any(|i| matches!(&i.location, Location::Folder { path, node: None, .. } if path == r"C:\Program Files\Assente")));
    program(r"C:\Program Files\Assente");
    // Il profilo non è la cartella di un programma (non plausibile): non si protegge.
    assert!(find(&roots, r"C:\Users\demo").is_empty());
    assert!(find(&roots, r"C:\Users").is_empty());
    // Una cartella che è insieme di sistema e di un programma resta una sola radice, di sistema.
    assert_eq!(find(&roots, r"C:\Windows\Installer"), [&root(r"C:\Windows\Installer", ProtectedKind::WindowsInstaller)]);
    // Altro volume: fuori.
    assert!(find(&roots, r"D:\Giochi\Gioco").is_empty());
}

#[test]
fn another_volume_has_only_its_own_roots() {
    let mut ev = evidence();
    ev.volume_root = "D:\\".into();
    let roots = protected_roots(&ev, &programs_for(&ev));
    assert_eq!(
        roots,
        [
            root(r"D:\System Volume Information", ProtectedKind::SystemVolumeInformation),
            root(r"D:\pagefile.sys", ProtectedKind::PagingFile),
            root(r"D:\hiberfil.sys", ProtectedKind::Hibernation),
            root(r"D:\swapfile.sys", ProtectedKind::SwapFile),
            root(r"D:\Giochi\Gioco", ProtectedKind::RegisteredProgram),
        ]
    );
    assert_well_formed(&roots, "D:\\");
}

#[test]
fn without_evidence_the_system_roots_remain() {
    // Prove vuote (nessun estrattore riuscito, cartella di Windows non letta): le radici di sistema ci
    // sono lo stesso, con il nome predefinito della cartella di Windows; la radice senza barra finale va
    // bene.
    let ev = Evidence::empty("R:");
    let p = programs_for(&ev);
    assert!(p.items.is_empty());
    let roots = protected_roots(&ev, &p);
    let kinds: Vec<ProtectedKind> = roots.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        [
            ProtectedKind::WindowsDir,
            ProtectedKind::WinSxS,
            ProtectedKind::WindowsInstaller,
            ProtectedKind::SystemVolumeInformation,
            ProtectedKind::PagingFile,
            ProtectedKind::Hibernation,
            ProtectedKind::SwapFile,
        ]
    );
    assert_eq!(roots[0].path, r"R:\Windows");
    assert_eq!(roots[2].path, r"R:\Windows\Installer");
    assert_eq!(roots[6].path, r"R:\swapfile.sys");
    assert_well_formed(&roots, "R:\\");
}
