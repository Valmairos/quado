// SPDX-License-Identifier: GPL-3.0-or-later
//! Cartelle sincronizzate dichiarate dalla configurazione dei programmi (R10, parere sulle avvertenze del
//! 01/11/2026), lette in sola lettura e senza rete. Le usa la conferma dello spostamento nel Cestino
//! (`quado_actions::sync`) quando la cartella non è registrata in Windows.
//!
//! - Dropbox: `info.json` in `%LOCALAPPDATA%\Dropbox` o `%APPDATA%\Dropbox`, un oggetto per conto
//!   (`personal`, `business`) con il campo `path` (Dropbox, "How to programmatically find the Dropbox folder
//!   paths", https://help.dropbox.com/installs/locate-dropbox-folder).
//! - Google Drive per desktop: `DefaultMountPoint`, lettera di unità o cartella dove Drive monta i file, nelle
//!   chiavi `HKLM\Software\Policies\Google\DriveFS` (vince), `HKCU\Software\Google\DriveFS` e
//!   `HKLM\Software\Google\DriveFS` (Google, "Advanced Drive for desktop configuration"). Il valore accetta
//!   le variabili d'ambiente (`%USERNAME%`). Senza il valore, la lettera scelta dall'app non è documentata e
//!   non si indovina.

use crate::extract::{ExtractError, FileSource, FormatIssue, is_absolute, join, normalize};
use crate::reg::{RegistrySource, Root};

/// Chiavi di Google Drive per desktop, nell'ordine in cui valgono.
pub const DRIVEFS_KEYS: [(Root, &str); 3] =
    [(Root::LocalMachine, r"Software\Policies\Google\DriveFS"), (Root::CurrentUser, r"Software\Google\DriveFS"), (Root::LocalMachine, r"Software\Google\DriveFS")];

/// Il `path` di ogni conto di `info.json` di Dropbox, senza doppioni. Un `info.json` che non è un oggetto
/// JSON è un formato inatteso.
pub fn parse_dropbox_info(text: &str) -> Result<Vec<String>, FormatIssue> {
    if text.trim().is_empty() {
        return Err(FormatIssue::Empty);
    }
    let v: serde_json::Value = serde_json::from_str(text).map_err(|_| FormatIssue::Unexpected)?;
    let obj = v.as_object().ok_or(FormatIssue::Unexpected)?;
    let mut out: Vec<String> = Vec::new();
    for account in obj.values() {
        let Some(p) = account.get("path").and_then(serde_json::Value::as_str) else { continue };
        let p = normalize(p);
        if is_absolute(&p) && !out.iter().any(|x| x.eq_ignore_ascii_case(&p)) {
            out.push(p);
        }
    }
    Ok(out)
}

/// Cartelle di Dropbox dai due `info.json` possibili (`local` e `roaming` sono le cartelle note
/// `%LOCALAPPDATA%` e `%APPDATA%`). Un file assente o illeggibile non dà cartelle.
pub fn dropbox_roots(files: &dyn FileSource, local: Option<&str>, roaming: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for base in [local, roaming].into_iter().flatten() {
        let text = match files.read(&join(&join(base, "Dropbox"), "info.json")) {
            Ok(t) => t,
            Err(ExtractError::NotPresent | ExtractError::Io(_) | ExtractError::Format(_) | ExtractError::CloudPlaceholder) => continue,
        };
        for p in parse_dropbox_info(&text).unwrap_or_default() {
            if !out.iter().any(|x| x.eq_ignore_ascii_case(&p)) {
                out.push(p);
            }
        }
    }
    out
}

/// Sostituisce `%NOME%` con la variabile d'ambiente; `None` se una variabile non c'è.
fn expand(value: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find('%')?;
        out.push_str(&env(&after[..end])?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// L'unità o la cartella di Google Drive per desktop da `DefaultMountPoint` (vedi il modulo). `G`, `G:` e
/// `G:\` danno `G:\`.
pub fn google_drive_mount(reg: &dyn RegistrySource, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let raw = DRIVEFS_KEYS.iter().find_map(|(root, key)| match reg.value(*root, key, "DefaultMountPoint") {
        Ok(Some(v)) => v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned),
        _ => None,
    })?;
    let v = expand(&raw, env)?;
    let v = v.trim().trim_matches('"');
    let b = v.as_bytes();
    let path = if b.len() == 1 && b[0].is_ascii_alphabetic() || b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        format!("{}:\\", v[..1].to_ascii_uppercase())
    } else {
        normalize(v)
    };
    is_absolute(&path).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reg::RegFile;

    #[test]
    fn dropbox_info_gives_the_path_of_every_account() {
        let text = r#"{"personal": {"path": "C:\\Users\\demo\\Dropbox", "host": 123, "is_team": false, "subscription_type": "Basic"},
            "business": {"path": "C:\\Users\\demo\\Dropbox (Ditta)", "host": 456, "is_team": true, "subscription_type": "Business"}}"#;
        // Un percorso per conto, nell'ordine dei nomi dei conti.
        assert_eq!(parse_dropbox_info(text), Ok(vec![r"C:\Users\demo\Dropbox (Ditta)".to_owned(), r"C:\Users\demo\Dropbox".to_owned()]));
        assert_eq!(parse_dropbox_info(r#"{"personal": {"host": 1}}"#), Ok(Vec::new()), "conto senza path");
        assert_eq!(parse_dropbox_info(r#"{"personal": {"path": "Dropbox"}}"#), Ok(Vec::new()), "percorso relativo");
        assert_eq!(parse_dropbox_info("[1, 2]"), Err(FormatIssue::Unexpected));
        assert_eq!(parse_dropbox_info("{\"personal\": "), Err(FormatIssue::Unexpected));
        assert_eq!(parse_dropbox_info("  "), Err(FormatIssue::Empty));
    }

    struct Files(Vec<(&'static str, &'static str)>);

    impl FileSource for Files {
        fn read(&self, path: &str) -> Result<String, ExtractError> {
            self.0.iter().find(|(p, _)| p.eq_ignore_ascii_case(path)).map(|(_, t)| (*t).to_owned()).ok_or(ExtractError::NotPresent)
        }
        fn list(&self, _: &str) -> Result<Vec<String>, ExtractError> {
            Err(ExtractError::NotPresent)
        }
    }

    #[test]
    fn dropbox_roots_from_local_and_roaming() {
        let files = Files(vec![
            (r"C:\Users\demo\AppData\Local\Dropbox\info.json", r#"{"personal": {"path": "D:\\Dropbox"}}"#),
            (r"C:\Users\demo\AppData\Roaming\Dropbox\info.json", r#"{"personal": {"path": "D:\\Dropbox\\"}, "business": {"path": "E:\\Lavoro"}}"#),
        ]);
        let roots = dropbox_roots(&files, Some(r"C:\Users\demo\AppData\Local"), Some(r"C:\Users\demo\AppData\Roaming"));
        assert_eq!(roots, vec![r"D:\Dropbox".to_owned(), r"E:\Lavoro".to_owned()]);
        assert!(dropbox_roots(&Files(Vec::new()), Some(r"C:\x"), None).is_empty());
        let bad = Files(vec![(r"C:\x\Dropbox\info.json", "non json")]);
        assert!(dropbox_roots(&bad, Some(r"C:\x"), None).is_empty());
    }

    #[test]
    fn google_drive_mount_point_from_the_documented_keys() {
        let env = |v: &str| (v.eq_ignore_ascii_case("USERNAME")).then(|| "demo".to_owned());
        let reg = |text: &str| RegFile::parse(&format!("Windows Registry Editor Version 5.00\n\n{text}")).expect("registro");
        let user = reg("[HKEY_CURRENT_USER\\Software\\Google\\DriveFS]\n\"DefaultMountPoint\"=\"H\"\n");
        assert_eq!(google_drive_mount(&user, &env).as_deref(), Some("H:\\"));
        let folder = reg("[HKEY_LOCAL_MACHINE\\Software\\Google\\DriveFS]\n\"DefaultMountPoint\"=\"C:\\\\Users\\\\%USERNAME%\\\\GoogleDrive\"\n");
        assert_eq!(google_drive_mount(&folder, &env).as_deref(), Some(r"C:\Users\demo\GoogleDrive"));
        // La chiave delle policy vince sulle altre.
        let both = reg(
            "[HKEY_LOCAL_MACHINE\\Software\\Policies\\Google\\DriveFS]\n\"DefaultMountPoint\"=\"P:\"\n\n[HKEY_CURRENT_USER\\Software\\Google\\DriveFS]\n\"DefaultMountPoint\"=\"H\"\n",
        );
        assert_eq!(google_drive_mount(&both, &env).as_deref(), Some("P:\\"));
        // Niente valore, una variabile che non c'è, un percorso relativo: nessuna unità.
        assert_eq!(google_drive_mount(&reg(""), &env), None);
        let unknown = reg("[HKEY_CURRENT_USER\\Software\\Google\\DriveFS]\n\"DefaultMountPoint\"=\"%NONCE%\\\\Drive\"\n");
        assert_eq!(google_drive_mount(&unknown, &env), None);
        let relative = reg("[HKEY_CURRENT_USER\\Software\\Google\\DriveFS]\n\"DefaultMountPoint\"=\"Drive\"\n");
        assert_eq!(google_drive_mount(&relative, &env), None);
    }
}
