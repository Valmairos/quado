// SPDX-License-Identifier: GPL-3.0-or-later
//! Lanciatore della disinstallazione: solo valori tipizzati. I test di compilazione
//! (`compile_fail`) sono nella documentazione di `launch.rs`; qui la validazione del PFN e gli URI.
//! `open` non si esegue nei test automatici: aprirebbe Impostazioni o Steam.

use quado_actions::launch::{Launch, Pfn, SteamAppId, open};

const VALID: &str = "Microsoft.WindowsTerminal_8wekyb3d8bbwe";

#[test]
fn real_family_names_are_accepted() {
    // Elenco MSIX della fixture della base delle regole (pacchetti di Windows).
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../quado-kb/fixture/msix.txt");
    let text = std::fs::read_to_string(path).expect("fixture MSIX");
    let mut n = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let pfn = line.split('|').next().expect("nome di famiglia");
        let p = Pfn::new(pfn).unwrap_or_else(|| panic!("{pfn} rifiutato"));
        assert_eq!(p.as_str(), pfn);
        n += 1;
    }
    assert!(n >= 5, "voci lette: {n}");
    for ok in [VALID, "NcsiUwpApp_8wekyb3d8bbwe", "1527c705-839a-4832-9118-54d4Bd6a0c89_cw5n1h2txyewy", "abc_0123456789abc"] {
        assert!(Pfn::new(ok).is_some(), "{ok}");
    }
}

#[test]
fn pfn_rejects_command_line_characters_spaces_and_uris() {
    let bad = [
        "",
        "_",
        "abc_",
        "_8wekyb3d8bbwe",
        "8wekyb3d8bbwe",
        "ab_8wekyb3d8bbwe",
        "Microsoft Terminal_8wekyb3d8bbwe",
        " Microsoft.WindowsTerminal_8wekyb3d8bbwe",
        "Microsoft.WindowsTerminal_8wekyb3d8bbwe ",
        "Microsoft.WindowsTerminal_8wekyb3d8bbw",
        "Microsoft.WindowsTerminal_8wekyb3d8bbwe1",
        "Microsoft.WindowsTerminal_8wekyb3d8bbw-",
        "Microsoft.WindowsTerminal_8wekyb3d8bbwe&x=1",
        "Microsoft.WindowsTerminal_8wekyb3d8bbwe?x",
        "ms-settings:appsfeatures",
        "ms-settings:appsfeatures-app?Microsoft.WindowsTerminal_8wekyb3d8bbwe",
        "steam://uninstall/3999910",
        "https://example.com/a_8wekyb3d8bbwe",
        "file:///C:/Windows_8wekyb3d8bbwe",
        r"C:\Windows\System32\cmd.exe_8wekyb3d8bbwe",
        "C:/Windows/notepad_8wekyb3d8bbwe",
        "\"Microsoft\"_8wekyb3d8bbwe",
        "'Microsoft'_8wekyb3d8bbwe",
        "Micro#soft_8wekyb3d8bbwe",
        "Micro&soft_8wekyb3d8bbwe",
        "Micro|soft_8wekyb3d8bbwe",
        "Micro;soft_8wekyb3d8bbwe",
        "Micro%soft_8wekyb3d8bbwe",
        "Micro^soft_8wekyb3d8bbwe",
        "Micro>soft_8wekyb3d8bbwe",
        "Micro<soft_8wekyb3d8bbwe",
        "Micro`soft_8wekyb3d8bbwe",
        "Micro$soft_8wekyb3d8bbwe",
        "Micro(soft)_8wekyb3d8bbwe",
        "Micro\nsoft_8wekyb3d8bbwe",
        "Micro\tsoft_8wekyb3d8bbwe",
        "Micro\0soft_8wekyb3d8bbwe",
        "Micro_soft_8wekyb3d8bbwe",
        "Microsoftè.Terminal_8wekyb3d8bbwe",
        "Microsoft.WindowsTerminal_8wekyb3d8bbwé",
    ];
    for b in bad {
        assert!(Pfn::new(b).is_none(), "accettato: {b:?}");
    }
    // Nome troppo lungo (lo schema del manifest ammette fino a 50 caratteri) e stringhe enormi.
    assert!(Pfn::new(&format!("{}_8wekyb3d8bbwe", "a".repeat(50))).is_some());
    assert!(Pfn::new(&format!("{}_8wekyb3d8bbwe", "a".repeat(51))).is_none());
    assert!(Pfn::new(&"a".repeat(100_000)).is_none());
    assert!(Pfn::new(&format!("{}_8wekyb3d8bbwe", "a.".repeat(50_000))).is_none());
}

#[test]
fn every_ascii_character_outside_the_allowed_set_is_rejected() {
    // Ogni carattere ASCII, messo nel nome o nell'identificativo di un PFN valido.
    for b in 0u8..128 {
        let c = b as char;
        let in_name = format!("Micro{c}soft_8wekyb3d8bbwe");
        let allowed_in_name = c.is_ascii_alphanumeric() || c == '.' || c == '-';
        assert_eq!(Pfn::new(&in_name).is_some(), allowed_in_name, "nel nome: {c:?}");
        let in_id = format!("Microsoft_8wekyb{c}d8bbwe");
        assert_eq!(Pfn::new(&in_id).is_some(), c.is_ascii_alphanumeric(), "nell'identificativo: {c:?}");
    }
}

#[test]
fn steam_app_id_is_never_zero() {
    assert!(SteamAppId::new(0).is_none());
    assert_eq!(SteamAppId::new(1).map(SteamAppId::get), Some(1));
    assert_eq!(SteamAppId::new(u32::MAX).map(SteamAppId::get), Some(u32::MAX));
}

#[test]
fn uris_have_the_documented_form() {
    assert_eq!(Launch::AppsFeatures.uri(), "ms-settings:appsfeatures");
    let pfn = Pfn::new(VALID).expect("PFN");
    assert_eq!(Launch::AppsFeaturesApp(pfn).uri(), format!("ms-settings:appsfeatures-app?{VALID}"));
    let id = SteamAppId::new(3_999_920).expect("appid");
    assert_eq!(Launch::SteamUninstall(id).uri(), "steam://uninstall/3999920");
    // L'URI non ha mai spazi né virgolette: la shell non lo spezza in parametri.
    for l in [Launch::AppsFeatures, Launch::AppsFeaturesApp(Pfn::new(VALID).expect("PFN")), Launch::SteamUninstall(SteamAppId::new(7).expect("appid"))] {
        let u = l.uri();
        assert!(!u.contains([' ', '"', '\'', '&', '|', '<', '>', '^', '%']), "{u}");
    }
}

#[test]
#[ignore = "apre Impostazioni ▸ App su questo PC: controllo manuale con data"]
fn opens_apps_and_features() {
    open(&Launch::AppsFeatures).expect("Impostazioni ▸ App");
}
