// SPDX-License-Identifier: GPL-3.0-or-later
//! Procedure di Windows e lanciatore (F-AC-1, 05 D7, 06 §5.7): URI di ogni `Launch`, riconoscimento
//! degli URI delle regole solo per costanti note, rifiuti di "Svuota il Cestino".
//!
//! `empty_recycle_bin` non si chiama mai con una radice valida: aprirebbe il dialogo di Windows. Qui si
//! provano solo i rifiuti, che tornano prima di chiamare Windows. `open` non si esegue nei test
//! automatici: aprirebbe Esplora file o Impostazioni.

use std::path::Path;

use quado_actions::launch::{Launch, Pfn, SteamAppId, TEMP_FILES_URI, open};
use quado_actions::procedures::{EMPTY_RECYCLE_BIN, empty_recycle_bin, is_drive_root};
use quado_actions::shell::ShellError;

/// `E_INVALIDARG`.
const E_INVALIDARG: i32 = 0x8007_0057_u32 as i32;

const PFN: &str = "Microsoft.WindowsTerminal_8wekyb3d8bbwe";

fn all() -> Vec<Launch> {
    vec![
        Launch::AppsFeatures,
        Launch::AppsFeaturesApp(Pfn::new(PFN).expect("PFN")),
        Launch::SteamUninstall(SteamAppId::new(3_999_910).expect("appid")),
        Launch::RecycleBin,
        Launch::StorageSense,
        Launch::TempFiles,
    ]
}

#[test]
fn every_launch_has_its_documented_uri() {
    let expected = [
        "ms-settings:appsfeatures",
        "ms-settings:appsfeatures-app?Microsoft.WindowsTerminal_8wekyb3d8bbwe",
        "steam://uninstall/3999910",
        "shell:RecycleBinFolder",
        "ms-settings:storagesense",
        TEMP_FILES_URI,
    ];
    for (l, uri) in all().iter().zip(expected) {
        assert_eq!(l.uri(), uri, "{l:?}");
        // Nessun carattere che la shell possa leggere come parametro o comando.
        assert!(!uri.contains([' ', '"', '\'', '&', '|', '<', '>', '^', '%', '\n']), "{uri}");
    }
    // File temporanei: un URI di Impostazioni (05 D7), verificato su questo PC a mano (07 §1.6).
    assert!(TEMP_FILES_URI.starts_with("ms-settings:"));
}

#[test]
fn procedures_have_codes_uninstall_does_not() {
    assert_eq!(Launch::RecycleBin.procedure_code(), Some("cestino-apri"));
    assert_eq!(Launch::StorageSense.procedure_code(), Some("archiviazione"));
    assert_eq!(Launch::TempFiles.procedure_code(), Some("file-temporanei"));
    for l in &all()[..3] {
        assert_eq!(l.procedure_code(), None, "{l:?}");
    }
    let mut codes: Vec<&str> = all().iter().filter_map(Launch::procedure_code).chain([EMPTY_RECYCLE_BIN]).collect();
    let n = codes.len();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), n, "codici distinti");
}

#[test]
fn rule_uris_are_recognized_only_as_known_constants() {
    assert_eq!(Launch::for_rule_uri("shell:RecycleBinFolder"), Some(Launch::RecycleBin));
    assert_eq!(Launch::for_rule_uri("ms-settings:storagesense"), Some(Launch::StorageSense));
    assert_eq!(Launch::for_rule_uri("ms-settings:appsfeatures"), Some(Launch::AppsFeatures));
    // Ogni costante si riconosce e riapre lo stesso URI.
    for l in [Launch::AppsFeatures, Launch::RecycleBin, Launch::StorageSense, Launch::TempFiles] {
        let back = Launch::for_rule_uri(&l.uri()).unwrap_or_else(|| panic!("{l:?} non riconosciuto"));
        assert_eq!(back.uri(), l.uri());
    }
    let foreign = [
        "",
        " ",
        "shell:recyclebinfolder",
        "SHELL:RecycleBinFolder",
        "shell:RecycleBinFolder ",
        " shell:RecycleBinFolder",
        "shell:RecycleBinFolder\0",
        "shell:RecycleBinFolder\\..",
        "shell:::{645FF040-5081-101B-9F08-00AA002F954E}",
        "shell:Downloads",
        "ms-settings:",
        "ms-settings:storagesense?",
        "ms-settings:storagesense?x=1",
        "ms-settings:storagesense&x",
        "ms-settings:storagesense\"",
        "ms-settings:Storagesense",
        "ms-settings:appsfeatures-app?Microsoft.WindowsTerminal_8wekyb3d8bbwe",
        "ms-settings:appsfeatures-app",
        "ms-settings:privacy-webcam",
        "steam://uninstall/3999910",
        "steam://run/3999910",
        "file:///C:/Windows/System32/cmd.exe",
        r"C:\Windows\System32\cmd.exe",
        "cmd.exe /c del",
        "cleanmgr",
        "https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings",
    ];
    for s in foreign {
        assert_eq!(Launch::for_rule_uri(s), None, "{s:?}");
    }
}

#[test]
fn every_uri_in_the_rules_is_a_known_constant() {
    // Gli URI dei metodi e delle alternative delle regole (`uri = "…"`): tutti apribili dal lanciatore.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../quado-kb/regole");
    let mut found = 0;
    for e in std::fs::read_dir(&dir).expect("regole").flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "toml") {
            continue;
        }
        let text = std::fs::read_to_string(&p).expect("regola");
        for (k, _) in text.match_indices("uri = \"") {
            let rest = &text[k + "uri = \"".len()..];
            let uri = &rest[..rest.find('"').expect("stringa chiusa")];
            assert!(Launch::for_rule_uri(uri).is_some(), "{}: {uri}", p.display());
            found += 1;
        }
    }
    assert!(found >= 3, "URI trovati nelle regole: {found}");
}

#[test]
fn empty_recycle_bin_rejects_anything_but_a_drive_root_without_calling_windows() {
    let rejected = [
        "",
        "C:",
        "c:",
        "C",
        r"\",
        "C:/",
        r"C:\Windows",
        r"C:\Windows\",
        r"C:\$Recycle.Bin",
        r"C:\\",
        r"C:\ ",
        r" C:\",
        "C:\\\0",
        r"\\server\share",
        r"\\server\share\",
        r"\\?\C:\",
        r"\\.\C:\",
        r"\\?\Volume{00000000-0000-0000-0000-000000000000}\",
        r"1:\",
        r"_:\",
        r"é:\",
        r"CC:\",
        r"C::\",
        "shell:RecycleBinFolder",
    ];
    for root in rejected {
        assert!(!is_drive_root(root), "{root:?}");
        // Rifiutato prima di Windows: nessun dialogo, nessun Cestino toccato.
        assert_eq!(empty_recycle_bin(root, None), Err(ShellError(E_INVALIDARG)), "{root:?}");
        assert_eq!(empty_recycle_bin(root, Some(0)), Err(ShellError(E_INVALIDARG)), "{root:?}");
    }
    // Le radici ammesse si controllano solo con il predicato, senza chiamare Windows.
    for root in [r"C:\", r"r:\", r"Z:\"] {
        assert!(is_drive_root(root), "{root}");
    }
}

#[test]
#[ignore = "apre Esplora file sul Cestino di questo PC: controllo manuale con data (06 §5.7)"]
fn opens_the_recycle_bin() {
    open(&Launch::RecycleBin).expect("Apri il Cestino");
}

#[test]
#[ignore = "apre Impostazioni › Archiviazione su questo PC: controllo manuale con data (06 §5.7)"]
fn opens_storage_settings() {
    open(&Launch::StorageSense).expect("Archiviazione");
}

#[test]
#[ignore = "apre la pagina dei file temporanei su questo PC: controllo manuale con data (07 §1.6, criterio 5)"]
fn opens_temporary_files() {
    open(&Launch::TempFiles).expect("File temporanei");
}
