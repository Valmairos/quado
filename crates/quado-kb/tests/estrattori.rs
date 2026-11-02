// SPDX-License-Identifier: GPL-3.0-or-later
//! Estrattori: per ognuno un caso valido (fixture sintetica nel formato vero), uno troncato e uno con
//! il formato cambiato. Un formato inatteso dà "estrattore non riuscito" con il motivo, mai un dato
//! ricostruito. I lettori di formati non fidati ricevono ingressi casuali senza andare in panic.

mod common;

use proptest::prelude::*;
use quado_kb::extract::{self, ExtractError, Field, FormatIssue, TrackProgs, UserAssist};
use quado_kb::reg::{RegFile, RegValue, RegistrySource, Root};
use quado_kb::vdf;

fn reg_with(extra: &str) -> RegFile {
    let base = std::fs::read_to_string(common::dir().join("registro.reg")).expect("registro");
    RegFile::parse(&format!("{base}\r\n{extra}")).expect("registro")
}

#[test]
fn uninstall_entries() {
    // Valido: le voci della fixture, con e senza InstallLocation.
    let u = extract::uninstall(&common::registry()).expect("voci");
    let capcut = u.entries.iter().find(|e| e.display_name == "CapCut").expect("CapCut");
    assert_eq!(capcut.hive, quado_model::record::Hive::User);
    assert_eq!(capcut.version.as_deref(), Some("9.4.0.4015"));
    assert_eq!(capcut.location(), None, "CapCut non dichiara InstallLocation");
    assert_eq!(capcut.path_hint().as_deref(), Some(r"C:\Users\demo\AppData\Local\CapCut\Apps"));
    let with_location = u.entries.iter().filter(|e| e.location().is_some()).count();
    assert!(with_location >= 2, "voci con InstallLocation: {with_location}");
    assert!(u.skipped.is_empty());
    // Troncato: il file di registro finisce a metà di una stringa.
    let text = std::fs::read_to_string(common::dir().join("registro.reg")).expect("registro");
    let cut = &text[..text.find("\"DisplayName\"=\"CapCut").expect("voce") + 18];
    assert!(RegFile::parse(cut).is_err());
    // Formato cambiato: DisplayVersion come numero. La voce si scarta con il motivo, senza ricostruirla.
    let r = reg_with("[HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Strana]\r\n\"DisplayName\"=\"Strana\"\r\n\"DisplayVersion\"=dword:00000001\r\n");
    let u = extract::uninstall(&r).expect("voci");
    assert!(!u.entries.iter().any(|e| e.display_name == "Strana"));
    assert!(u.skipped.iter().any(|(_, k, i)| k == "Strana" && *i == FormatIssue::WrongType(Field::DisplayVersion)));
}

#[test]
fn msix_list() {
    let text = std::fs::read_to_string(common::dir().join("msix.txt")).expect("msix");
    let pkgs = extract::parse_msix_list(&text).expect("valido");
    assert!(pkgs.len() >= 2 && pkgs.iter().any(|p| p.framework));
    let first = text.lines().find(|l| !l.starts_with('#')).expect("riga");
    assert_eq!(extract::parse_msix_list(&first[..first.find('|').expect("campo")]), Err(FormatIssue::Truncated));
    assert_eq!(extract::parse_msix_list("Senza.Editore|C:\\x|0"), Err(FormatIssue::BadValue(Field::PackageFamilyName)));
    assert_eq!(extract::parse_msix_list("App_8wekyb3d8bbwe|relativo|0"), Err(FormatIssue::BadValue(Field::InstalledPath)));
}

#[test]
fn steam_manifests() {
    let files = common::FakeFiles::load();
    let s = extract::steam(&common::registry(), &files, "C:\\").expect("steam");
    assert_eq!(s.libraries, vec![r"C:\Program Files (x86)\Steam".to_owned()]);
    let game = s.apps.iter().find(|a| a.appid == 3_999_910).expect("Hollow Peaks");
    assert_eq!(game.installdir, "hollow peaks");
    assert!(game.size_on_disk.is_some());
    // LastPlayed di localconfig.vdf e dei manifest concordano entro un giorno.
    for a in &s.apps {
        if let (Some(m), Some(&(_, l))) = (a.last_played, s.local_last_played.iter().find(|(id, _)| *id == a.appid)) {
            assert!((m - l).abs() <= 86_400, "{}: {m} contro {l}", a.name);
        }
    }
    // Troncato.
    let lf = files.read_text(r"C:\Program Files (x86)\Steam\steamapps\libraryfolders.vdf");
    assert_eq!(extract::parse_libraryfolders(&lf[..lf.len() / 2]), Err(FormatIssue::Truncated));
    let acf = files.read_text(r"C:\Program Files (x86)\Steam\steamapps\appmanifest_3999910.acf");
    assert_eq!(extract::parse_appmanifest(&acf[..acf.len() - 3], "C:\\x").map(|_| ()), Err(FormatIssue::Truncated));
    // Formato cambiato: radice diversa, installdir che esce dalla libreria, LastPlayed non numerico.
    assert_eq!(extract::parse_appmanifest(&acf.replacen("\"AppState\"", "\"AppStato\"", 1), "C:\\x").map(|_| ()), Err(FormatIssue::MissingField(Field::Root)));
    assert_eq!(
        extract::parse_appmanifest(&acf.replacen("\"hollow peaks\"", "\"..\\\\..\\\\Windows\"", 1), "C:\\x").map(|_| ()),
        Err(FormatIssue::BadValue(Field::InstallDir))
    );
    let lc = files.read_text(r"C:\Program Files (x86)\Steam\userdata\1000\config\localconfig.vdf");
    let bad = lc.replacen("\"LastPlayed\"\t\t\"", "\"LastPlayed\"\t\t\"ieri", 1);
    assert_eq!(extract::parse_localconfig(&bad), Err(FormatIssue::BadValue(Field::LastPlayed)));
}

#[test]
fn epic_manifests() {
    let files = common::FakeFiles::load();
    let e = extract::epic(&files, r"C:\ProgramData").expect("epic");
    assert_eq!(e.items.len(), 3);
    assert!(e.items.iter().all(|i| i.install_location == r"C:\Program Files\Epic Games\UE_5.5"), "stessa cartella per i tre prodotti");
    let one = files.read_text(&format!(r"C:\ProgramData\{}\{}", extract::EPIC_MANIFESTS, "A0000000000000000000000000000002.item"));
    assert_eq!(extract::parse_epic_item(&one[..one.len() / 2]).map(|_| ()), Err(FormatIssue::Truncated));
    let changed = one.replacen("\"InstallLocation\": \"", "\"InstallLocation\": 1, \"Vecchio\": \"", 1);
    assert_eq!(extract::parse_epic_item(&changed).map(|_| ()), Err(FormatIssue::WrongType(Field::InstallLocation)));
}

#[test]
fn capcut_files() {
    let files = common::FakeFiles::load();
    let local = r"C:\Users\demo\AppData\Local";
    assert_eq!(extract::capcut(&files, local), Ok("9.4.0.4015".into()));
    assert_eq!(extract::capcut_drafts(&files, local), Ok(r"C:\Users\demo\AppData\Local\CapCut\User Data\Projects\com.lveditor.draft".into()));
    let xml = files.read_text(&format!(r"{local}\{}", extract::CAPCUT_PRODUCT_INFO));
    assert_eq!(extract::parse_product_info(&xml[..xml.len() - 12]), Err(FormatIssue::Truncated));
    assert_eq!(extract::parse_product_info(&xml.replace("full_appver", "versione_completa")), Err(FormatIssue::MissingField(Field::Version)));
    let gs = files.read_text(&format!(r"{local}\{}", extract::CAPCUT_SETTINGS));
    assert_eq!(extract::parse_capcut_drafts(""), Err(FormatIssue::Empty));
    assert_eq!(extract::parse_capcut_drafts(&gs.replace("currentCustomDraftPath", "draftPath")), Err(FormatIssue::MissingField(Field::InstallLocation)));
    assert_eq!(extract::parse_capcut_drafts("currentCustomDraftPath=Projects"), Err(FormatIssue::BadValue(Field::InstallLocation)));
}

#[test]
fn userassist_only_with_tracking() {
    // Questo PC: Start_TrackProgs = 0 e la chiave con dati vecchi. Nessun valore si usa.
    let reg = common::registry();
    assert_eq!(extract::track_progs(&reg), TrackProgs::Off);
    assert_eq!(extract::userassist(&reg), Ok(UserAssist::NotTracked(TrackProgs::Off)));
    // Valore assente: nessun dato d'uso.
    let none = RegFile::parse("Windows Registry Editor Version 5.00\r\n[HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced]\r\n").expect("vuoto");
    assert_eq!(extract::track_progs(&none), TrackProgs::Absent);
    assert!(matches!(extract::userassist(&none), Ok(UserAssist::NotTracked(TrackProgs::Absent))));
    // Con il tracciamento attivo le voci della fixture si leggono.
    let on = reg_with("[HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced]\r\n\"Start_TrackProgs\"=dword:00000001\r\n");
    let on = merged(&on);
    match extract::userassist(&on) {
        Ok(UserAssist::Entries(v)) => assert!(!v.is_empty() && v.iter().all(|e| e.name.starts_with("Microsoft.Windows"))),
        other => panic!("{other:?}"),
    }
    // Formato cambiato: un valore di 10 byte invece di 72.
    let bad = reg_with(
        "[HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced]\r\n\"Start_TrackProgs\"=dword:00000001\r\n\r\n[HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\UserAssist\\{X}\\Count]\r\n\"Cebtenz\"=hex:01,02,03,04,05,06,07,08,09,0a\r\n",
    );
    assert_eq!(extract::userassist(&merged(&bad)), Err(ExtractError::Format(FormatIssue::BadValue(Field::Data))));
}

/// Il file di fixture ripete la chiave `Advanced`: l'ultima vince, come in `reg import`.
fn merged(r: &RegFile) -> impl RegistrySource + '_ {
    struct Last<'a>(&'a RegFile);
    impl RegistrySource for Last<'_> {
        fn subkeys(&self, root: Root, path: &str) -> Result<Vec<String>, u32> {
            self.0.subkeys(root, path)
        }
        fn values(&self, root: Root, path: &str) -> Result<Vec<(String, RegValue)>, u32> {
            let mut v = self.0.values(root, path)?;
            v.reverse();
            Ok(v)
        }
    }
    Last(r)
}

#[test]
fn configuration_readings() {
    let reg = common::registry();
    // Sensore memoria della fixture: attivo, ogni mese, Cestino oltre 30 giorni, Download spento.
    let s = extract::storage_sense(&reg).expect("sensore");
    assert!(s.configured && s.enabled);
    assert_eq!((s.frequency_days, s.recycle_days, s.downloads_days), (Some(30), Some(30), None));
    let b = extract::bin_settings(&reg, "{00000000-0000-4000-8000-000000000c01}").expect("cestino");
    assert_eq!((b.max_capacity_mib, b.nuke_on_delete), (Some(24_380), false));
    assert_eq!(extract::bin_settings(&reg, "{altro}"), Err(ExtractError::NotPresent));
    // Formato cambiato: un valore di Sensore memoria come testo.
    let bad = reg_with("[HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\StorageSense\\Parameters\\StoragePolicy]\r\n\"256\"=\"trenta\"\r\n");
    assert_eq!(extract::storage_sense(&merged(&bad)), Err(ExtractError::Format(FormatIssue::WrongType(Field::Data))));
    // Chiave assente: mai configurato.
    let empty = RegFile::parse("Windows Registry Editor Version 5.00\r\n").expect("vuoto");
    assert_eq!(extract::storage_sense(&empty).map(|s| s.configured), Ok(false));
}

trait ReadText {
    fn read_text(&self, p: &str) -> String;
}

impl ReadText for common::FakeFiles {
    fn read_text(&self, p: &str) -> String {
        quado_kb::extract::FileSource::read(self, p).unwrap_or_else(|e| panic!("{p}: {e:?}"))
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn untrusted_readers_never_panic(s in "\\PC{0,400}") {
        let _ = vdf::parse(&s);
        let _ = extract::parse_epic_item(&s);
        let _ = extract::parse_product_info(&s);
        let _ = extract::parse_capcut_drafts(&s);
        let _ = extract::parse_msix_list(&s);
        let _ = extract::parse_libraryfolders(&s);
        let _ = extract::parse_appmanifest(&s, "C:\\x");
        let _ = extract::parse_localconfig(&s);
        let _ = RegFile::parse(&s);
    }

    #[test]
    fn readers_survive_mutated_fixtures(cut in 0usize..2000, byte in any::<u8>()) {
        let files = common::FakeFiles::load();
        for p in [r"C:\Program Files (x86)\Steam\steamapps\libraryfolders.vdf", r"C:\Program Files (x86)\Steam\steamapps\appmanifest_3999910.acf"] {
            let mut t = files.read_text(p).into_bytes();
            let k = cut.min(t.len().saturating_sub(1));
            t[k] = byte;
            let s = String::from_utf8_lossy(&t);
            let _ = vdf::parse(&s);
            let _ = extract::parse_appmanifest(&s, "C:\\x");
        }
        let reg = std::fs::read_to_string(common::dir().join("registro.reg")).expect("registro");
        let mut b = reg.into_bytes();
        let k = cut.min(b.len().saturating_sub(1));
        b[k] = byte;
        let _ = RegFile::parse(&String::from_utf8_lossy(&b));
    }
}
