// SPDX-License-Identifier: GPL-3.0-or-later
//! Controllo del catalogo: `--volumes` (Docker, elimina dati) non compare in nessun file di lingua, in
//! nessuna regola né nei sorgenti del prodotto. `/ResetBase` non compare in nessun testo che l'utente vede:
//! dopo quel comando gli aggiornamenti installati non si possono più disinstallare, e WinSxS è solo
//! spiegazione. Resta come nota nei commenti della regola `winsxs`.
//!
//! Il controllo segue la forma attuale dei file: nelle regole TOML i commenti cominciano con `#`; nei file
//! Fluent un messaggio comincia a inizio riga con `chiave =` e continua sulle righe rientrate. I commenti
//! delle regole e dei loro testi non sono testo visibile e non contano; nei sorgenti del prodotto e nei file
//! di lingua dell'interfaccia invece conta tutto, commenti compresi.

use std::path::{Path, PathBuf};

const VOLUMES: &str = "--volumes";
const RESET_BASE: &str = "/ResetBase";

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// File con estensione `ext` in `dir` (non ricorsivo), ordinati.
fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> =
        std::fs::read_dir(dir).expect("cartella").flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == ext)).collect();
    v.sort();
    assert!(!v.is_empty(), "nessun .{ext} in {}", dir.display());
    v
}

/// Tutti i file sotto `src/` dei crate del prodotto (ogni crate in `crates/` tranne `quado-testkit`).
fn product_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(crates_dir()).expect("crate").flatten() {
        let c = e.path();
        if !c.join("Cargo.toml").is_file() || c.file_name().is_some_and(|n| n == "quado-testkit") {
            continue;
        }
        let mut stack = vec![c.join("src")];
        while let Some(d) = stack.pop() {
            for f in std::fs::read_dir(&d).expect("sorgenti").flatten() {
                let p = f.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    assert!(out.len() > 20, "sorgenti trovati: {}", out.len());
    out
}

fn ui_languages() -> Vec<PathBuf> {
    files(&crates_dir().join("quado-ui/lingue"), "ftl")
}

fn rule_languages() -> Vec<PathBuf> {
    files(&crates_dir().join("quado-kb/regole"), "ftl")
}

fn rules() -> Vec<PathBuf> {
    files(&crates_dir().join("quado-kb/regole"), "toml")
}

fn read(p: &Path) -> String {
    String::from_utf8_lossy(&std::fs::read(p).expect("file")).into_owned()
}

// ---------------------------------------------------------------------------------------------
// Regole TOML e file Fluent

/// Il testo `line` (dall'inizio della riga fino a un'occorrenza) contiene un `#` fuori dalle stringhe:
/// l'occorrenza sta in un commento.
fn in_toml_comment(line: &str) -> bool {
    let mut quote: Option<char> = None;
    for ch in line.chars() {
        match (quote, ch) {
            (None, '#') => return true,
            (None, '"' | '\'') => quote = Some(ch),
            (Some(q), c) if c == q => quote = None,
            _ => {}
        }
    }
    false
}

/// Occorrenze di `needle` fuori dai commenti di una regola TOML.
fn toml_visible(text: &str, needle: &str) -> usize {
    text.match_indices(needle)
        .filter(|(pos, _)| {
            let line_start = text[..*pos].rfind('\n').map_or(0, |k| k + 1);
            !in_toml_comment(&text[line_start..*pos])
        })
        .count()
}

/// Messaggi di un file Fluent: (chiave, testo con le righe di continuazione). I commenti non contano.
fn ftl_messages(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut open = false;
    for l in text.lines() {
        if l.starts_with('#') {
            open = false;
        } else if l.starts_with(|c: char| c.is_ascii_alphanumeric()) && l.contains('=') {
            let (k, v) = l.split_once('=').unwrap_or_default();
            out.push((k.trim().to_owned(), v.to_owned()));
            open = true;
        } else if open && let Some(last) = out.last_mut() {
            last.1.push('\n');
            last.1.push_str(l);
        }
    }
    out
}

/// Chiavi dei messaggi di un file Fluent che nominano `needle`.
fn ftl_visible(text: &str, needle: &str) -> Vec<String> {
    ftl_messages(text).into_iter().filter(|(_, v)| v.contains(needle)).map(|(k, _)| k).collect()
}

// ---------------------------------------------------------------------------------------------
// Test

#[test]
fn volumes_appears_nowhere() {
    let mut all = ui_languages();
    all.extend(rule_languages());
    all.extend(rules());
    all.extend(product_sources());
    let hits: Vec<String> = all.iter().filter(|p| read(p).contains(VOLUMES)).map(|p| p.display().to_string()).collect();
    assert!(hits.is_empty(), "{VOLUMES} in:\n{}", hits.join("\n"));
}

#[test]
fn reset_base_appears_in_no_visible_text() {
    let mut problems = Vec::new();
    for p in rules() {
        if toml_visible(&read(&p), RESET_BASE) > 0 {
            problems.push(format!("{}: {RESET_BASE} fuori dai commenti", p.display()));
        }
    }
    for p in rule_languages() {
        for k in ftl_visible(&read(&p), RESET_BASE) {
            problems.push(format!("{}: {k} nomina {RESET_BASE}", p.display()));
        }
    }
    // Testi dell'interfaccia e sorgenti del prodotto: mai, nemmeno nei commenti.
    for p in ui_languages().into_iter().chain(product_sources()) {
        if read(&p).contains(RESET_BASE) {
            problems.push(format!("{}: {RESET_BASE}", p.display()));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // La nota resta nei commenti della regola: il controllo guarda i file giusti.
    let winsxs = rules().into_iter().find(|p| p.file_name().is_some_and(|n| n == "winsxs.toml")).expect("winsxs.toml");
    assert!(read(&winsxs).contains(RESET_BASE), "la nota su {RESET_BASE} nei commenti di winsxs.toml");
}

#[test]
fn the_check_finds_what_it_must() {
    // Controllo del controllo, su testi scritti qui.
    let rule = "# commento con /ResetBase\nid = \"winsxs\"\nmetodo = { comando = \"Dism\" }\n";
    assert_eq!(toml_visible(rule, RESET_BASE), 0);
    assert_eq!(toml_visible(&format!("{rule}alternative = [{{ comando = \"Dism /ResetBase\" }}]\n"), RESET_BASE), 1);
    assert_eq!(toml_visible("estratto = '''con # e /ResetBase'''\n", RESET_BASE), 1, "il # dentro una stringa non è un commento");
    let ftl = "# commento: /ResetBase\nregola-winsxs-riga = Windows lo riduce da solo.\n";
    assert!(ftl_visible(ftl, RESET_BASE).is_empty());
    let multi = "altra-chiave = { $n ->\n    [one] uno\n   *[other] /ResetBase\n}\n";
    assert_eq!(ftl_visible(multi, RESET_BASE), vec!["altra-chiave".to_owned()]);
}
