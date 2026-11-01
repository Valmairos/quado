// SPDX-License-Identifier: GPL-3.0-or-later
//! Dati della versione che si chiamava Rilievo (`legacy`): copia al primo avvio e cancellazione della
//! cartella vecchia, provate in cartelle temporanee al posto di `%APPDATA%` (mai quella vera).

use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use quado_actions::legacy::{self, Migration};

/// Una cartella temporanea che fa da `%APPDATA%`, tolta alla fine.
struct AppData(PathBuf);

impl AppData {
    fn new(name: &str) -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!("quado-appdata-{name}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("cartella temporanea");
        AppData(p)
    }

    fn dirs(&self) -> (PathBuf, PathBuf) {
        legacy::dirs(&self.0)
    }
}

impl Drop for AppData {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const NAMES: [&str; 3] = ["impostazioni.txt", "interventi.jsonl", "lingue"];

/// Una cartella di Rilievo come quella vera: impostazioni, registro, una lingua aggiunta e il modello, il log.
fn old_folder(old: &Path) {
    std::fs::create_dir_all(old.join("lingue")).unwrap();
    std::fs::create_dir_all(old.join("log")).unwrap();
    std::fs::write(old.join("impostazioni.txt"), "lingua = it\ntema = scuro\n").unwrap();
    std::fs::write(old.join("interventi.jsonl"), "{\"versione\":1}\n").unwrap();
    std::fs::write(old.join("lingue").join("de.ftl"), "nav-panoramica = Übersicht\n").unwrap();
    std::fs::write(old.join("lingue").join("modello-da-tradurre.txt"), "x").unwrap();
    std::fs::write(old.join("log").join("vecchio.txt"), "y").unwrap();
}

fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((p.strip_prefix(dir).unwrap().display().to_string(), std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_first_start_copies_settings_journal_and_languages_and_leaves_rilievo_as_it_is() {
    let a = AppData::new("copia");
    let (new, old) = a.dirs();
    assert!(new.ends_with("Quado") && old.ends_with("Rilievo"));
    old_folder(&old);
    let before = files(&old);
    assert_eq!(legacy::migrate(&old, &new, &NAMES), Migration::Copied(NAMES.iter().map(|s| s.to_string()).collect()));
    // Gli stessi byte; il log no (non è nell'elenco).
    let copied = files(&new);
    let expected: Vec<_> = before.iter().filter(|(n, _)| !n.starts_with("log")).cloned().collect();
    assert_eq!(copied, expected);
    assert_eq!(files(&old), before, "la cartella di Rilievo resta com'è");
    assert!(!a.0.join(legacy::STAGING_NAME).exists(), "nessuna cartella di copia rimasta");
    // Dal secondo avvio non si copia più, anche se Rilievo cambia.
    std::fs::write(old.join("impostazioni.txt"), "tema = chiaro\n").unwrap();
    assert_eq!(legacy::migrate(&old, &new, &NAMES), Migration::Nothing);
    assert_eq!(std::fs::read_to_string(new.join("impostazioni.txt")).unwrap(), "lingua = it\ntema = scuro\n");
}

#[test]
fn nothing_to_copy_without_rilievo_or_with_quado_already_there() {
    let a = AppData::new("niente");
    let (new, old) = a.dirs();
    assert_eq!(legacy::migrate(&old, &new, &NAMES), Migration::Nothing);
    assert!(!new.exists(), "senza Rilievo la cartella di Quado si crea solo quando serve");
    // Cartella di Quado già presente (anche vuota): niente si copia.
    old_folder(&old);
    std::fs::create_dir_all(&new).unwrap();
    assert_eq!(legacy::migrate(&old, &new, &NAMES), Migration::Nothing);
    assert!(files(&new).is_empty());
    // Solo alcuni elementi: si copiano quelli che ci sono.
    let b = AppData::new("parziale");
    let (new, old) = b.dirs();
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join("impostazioni.txt"), "tema = scuro\n").unwrap();
    assert_eq!(legacy::migrate(&old, &new, &NAMES), Migration::Copied(vec!["impostazioni.txt".into()]));
}

#[test]
fn a_failed_copy_leaves_no_quado_folder_and_is_retried() {
    let a = AppData::new("errore");
    let (new, old) = a.dirs();
    old_folder(&old);
    // Una copia rimasta a metà da un avvio interrotto si rifà da capo.
    std::fs::create_dir_all(a.0.join(legacy::STAGING_NAME).join("lingue")).unwrap();
    std::fs::write(a.0.join(legacy::STAGING_NAME).join("avanzo.txt"), "z").unwrap();
    {
        // Il registro aperto senza condivisione (Rilievo in esecuzione): la copia non riesce.
        let _held = std::fs::OpenOptions::new().read(true).share_mode(0).open(old.join("interventi.jsonl")).unwrap();
        assert_eq!(legacy::migrate(&old, &new, &NAMES), Migration::Failed(32), "violazione di condivisione");
        assert!(!new.exists(), "la cartella di Quado non c'è: al prossimo avvio si riprova");
        assert!(!a.0.join(legacy::STAGING_NAME).exists());
    }
    assert!(matches!(legacy::migrate(&old, &new, &NAMES), Migration::Copied(_)));
    assert!(!new.join("avanzo.txt").exists(), "niente della copia interrotta");
}

#[test]
fn delete_old_removes_only_the_rilievo_folder() {
    let a = AppData::new("cancella");
    let (new, old) = a.dirs();
    old_folder(&old);
    std::fs::create_dir_all(&new).unwrap();
    std::fs::write(new.join("impostazioni.txt"), "tema = scuro\n").unwrap();
    // Un'altra cartella non si cancella da qui.
    assert_eq!(legacy::delete_old(&new), Err(0));
    assert!(new.join("impostazioni.txt").exists());
    let other = a.0.join("Altro");
    std::fs::create_dir_all(&other).unwrap();
    assert_eq!(legacy::delete_old(&other), Err(0));
    assert!(other.exists());
    assert_eq!(legacy::delete_old(&old), Ok(()));
    assert!(!old.exists());
    assert!(new.join("impostazioni.txt").exists(), "i dati di Quado restano");
    // Già cancellata: nessun errore.
    assert_eq!(legacy::delete_old(&old), Ok(()));
}
