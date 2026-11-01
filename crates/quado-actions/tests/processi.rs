// SPDX-License-Identifier: GPL-3.0-or-later
//! Test d'architettura (06 §5.1, §11, F-AC-1, F-AP-5, 05 D7 "Confini nel codice"): Quado non crea
//! processi e non apre pacchetti MSI con `MsiOpenPackage`. Le chiamate che avviano programmi o procedure
//! e quelle che modificano file o svuotano il Cestino stanno solo in `quado-actions`; nei sorgenti
//! `src/` di model, scan-win, kb, ui e dell'eseguibile non compaiono. Fuori dal controllo:
//! `quado-actions`, i test, `quado-testkit` (supporto ai test), `xtask` e i prototipi.
//!
//! In tutto il prodotto, `quado-actions` compreso, non compaiono i flag che tolgono le conferme di
//! Windows (`SHERB_NOCONFIRMATION`, `FOF_NOCONFIRMATION`, `FOF_NO_UI`): restano solo in `xtask` e
//! `quado-testkit`, per le prove sui VHDX.
//!
//! I commenti non contano (un modulo può scrivere "mai `MsiOpenPackage`"); le stringhe sì, perché un
//! nome di funzione in una stringa può servire a caricarla a mano.

use std::path::{Path, PathBuf};

/// Crate controllati: tutto il codice del prodotto fuori da `quado-actions`.
const CRATES: [&str; 5] = ["quado-model", "quado-scan-win", "quado-kb", "quado-ui", "quado"];

/// Nomi vietati fuori da `quado-actions`: creazione di processi, apertura con la shell, MSI con lo
/// stato della macchina; chiamate che modificano file o svuotano il Cestino (06 §5.1). I nomi sono
/// prefissi: `DeleteFile` trova anche `DeleteFileW`, `GENERIC_WRITE` anche `FILE_GENERIC_WRITE`.
const FORBIDDEN: [&str; 14] = [
    "CreateProcess",
    "ShellExecute",
    "WinExec",
    "process::Command",
    "MsiOpenPackage",
    "IFileOperation",
    "MoveFileEx",
    "DeleteFile",
    "RemoveDirectory",
    "SetFileInformationByHandle",
    "SHFileOperation",
    "SHEmptyRecycleBin",
    "GENERIC_WRITE",
    "FILE_GENERIC_WRITE",
];

/// Eccezioni motivate, una per riga: (file relativo alla cartella dei crate, nome vietato, motivo). Oggi
/// nessuna: le chiamate esistenti stanno in `quado-actions` o nei test, e `GENERIC_WRITE` in
/// `quado-scan-win/src/win.rs` è in un commento (maschera dei diritti di scrittura da escludere).
const EXCEPTIONS: [(&str, &str, &str); 0] = [];

/// Flag che tolgono le conferme di Windows: vietati in tutto il prodotto (F-AC-1, F-AC-5, 05 D7).
const SILENT: [&str; 3] = ["SHERB_NOCONFIRMATION", "FOF_NOCONFIRMATION", "FOF_NO_UI"];

/// Eccezioni motivate ai flag silenziosi, una per riga: (file, nome, motivo). Vuota: un'asserzione che
/// ne nomini uno si motiva qui.
const SILENT_EXCEPTIONS: [(&str, &str, &str); 0] = [];

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Il sorgente senza i commenti (di riga, di documentazione e a blocchi, anche annidati). Stringhe e
/// caratteri restano: `"//"` in una stringa non apre un commento, `'"'` non apre una stringa.
fn strip_comments(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < c.len() {
        let next = c.get(i + 1).copied();
        match c[i] {
            '/' if next == Some('/') => {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            }
            '/' if next == Some('*') => {
                let mut depth = 0;
                while i < c.len() {
                    if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                        depth += 1;
                        i += 2;
                    } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                        depth -= 1;
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        if c[i] == '\n' {
                            out.push('\n');
                        }
                        i += 1;
                    }
                }
            }
            'r' if (next == Some('"') || next == Some('#')) && (i == 0 || !(c[i - 1].is_alphanumeric() || c[i - 1] == '_')) => {
                // Stringa grezza r"..." o r#"..."#: finisce al primo `"` seguito dallo stesso numero di `#`.
                let mut j = i + 1;
                let mut hashes = 0;
                while c.get(j) == Some(&'#') {
                    hashes += 1;
                    j += 1;
                }
                if c.get(j) != Some(&'"') {
                    out.push(c[i]);
                    i += 1;
                    continue;
                }
                let start = i;
                j += 1;
                while j < c.len() && !(c[j] == '"' && (0..hashes).all(|k| c.get(j + 1 + k) == Some(&'#'))) {
                    j += 1;
                }
                let end = (j + 1 + hashes).min(c.len());
                out.extend(&c[start..end]);
                i = end;
            }
            '"' => {
                out.push('"');
                i += 1;
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' && i + 1 < c.len() {
                        out.push(c[i]);
                        i += 1;
                    }
                    out.push(c[i]);
                    i += 1;
                }
                if i < c.len() {
                    out.push('"');
                    i += 1;
                }
            }
            '\'' => {
                // Carattere ('a', '\n', '"') oppure lifetime ('a): solo il primo si salta per intero.
                if next == Some('\\') {
                    let mut j = i + 2;
                    while j < c.len() && c[j] != '\'' && j < i + 12 {
                        j += 1;
                    }
                    out.extend(&c[i..(j + 1).min(c.len())]);
                    i = j + 1;
                } else if c.get(i + 2) == Some(&'\'') {
                    out.extend(&c[i..i + 3]);
                    i += 3;
                } else {
                    out.push('\'');
                    i += 1;
                }
            }
            ch => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

/// (file relativo alla cartella dei crate, riga, nome) trovati nei sorgenti `.rs` sotto `dirs`, senza
/// i commenti.
fn findings_in(dirs: &[PathBuf], names: &[&'static str]) -> Vec<(String, usize, &'static str)> {
    let base = crates_dir();
    let mut out = Vec::new();
    for d in dirs {
        let files = rust_files(d);
        assert!(!files.is_empty(), "{}: nessun sorgente trovato", d.display());
        for f in files {
            let src = std::fs::read_to_string(&f).expect("sorgente");
            let code = strip_comments(&src);
            let rel = f.strip_prefix(&base).unwrap_or(&f).to_string_lossy().replace('\\', "/");
            for (n, line) in code.lines().enumerate() {
                for bad in names {
                    if line.contains(bad) {
                        out.push((rel.clone(), n + 1, *bad));
                    }
                }
            }
        }
    }
    out
}

/// Nomi di [`FORBIDDEN`] nei sorgenti `src/` dei crate indicati.
fn findings(crates: &[&str]) -> Vec<(String, usize, &'static str)> {
    let dirs: Vec<PathBuf> = crates.iter().map(|c| crates_dir().join(c).join("src")).collect();
    findings_in(&dirs, &FORBIDDEN)
}

/// Cartelle `src/` di tutto il codice del prodotto: ogni crate in `crates/` tranne `quado-testkit`.
fn product_sources() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(crates_dir())
        .expect("cartella dei crate")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("Cargo.toml").is_file() && p.file_name().is_some_and(|n| n != "quado-testkit"))
        .map(|p| p.join("src"))
        .collect();
    dirs.sort();
    dirs
}

/// Problemi (trovati senza eccezione) e controllo che ogni eccezione serva ancora.
fn check(found: &[(String, usize, &'static str)], exceptions: &[(&str, &str, &str)], what: &str) {
    let problems: Vec<String> = found
        .iter()
        .filter(|(file, _, bad)| !exceptions.iter().any(|(f, b, _)| file == f && bad == b))
        .map(|(file, line, bad)| format!("{file}:{line}: {bad}"))
        .collect();
    assert!(problems.is_empty(), "{what}:\n{}", problems.join("\n"));
    // Ogni eccezione deve servire ancora: un'eccezione senza riscontro si toglie.
    for (f, b, why) in exceptions {
        assert!(found.iter().any(|(file, _, bad)| file == f && bad == b), "eccezione non più necessaria: {f} {b} ({why})");
    }
}

#[test]
fn product_code_outside_actions_does_not_create_processes_or_modify_files() {
    check(&findings(&CRATES), &EXCEPTIONS, "chiamate vietate fuori da quado-actions");
}

#[test]
fn the_check_sees_the_launcher_and_the_recycle_bin_in_actions() {
    // Controllo del controllo: in `quado-actions` il lanciatore usa ShellExecuteExW e "Svuota il
    // Cestino" SHEmptyRecycleBinW, e li si trova.
    let found = findings(&["quado-actions"]);
    assert!(found.iter().any(|(file, _, bad)| file.ends_with("launch.rs") && *bad == "ShellExecute"), "{found:?}");
    assert!(found.iter().any(|(file, _, bad)| file.ends_with("procedures.rs") && *bad == "SHEmptyRecycleBin"), "{found:?}");
}

#[test]
fn product_code_has_no_silent_flags() {
    let dirs = product_sources();
    let names: Vec<String> = dirs.iter().filter_map(|d| d.parent()?.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
    for c in CRATES.iter().chain(&["quado-actions"]) {
        assert!(names.iter().any(|n| n == c), "{c} manca tra i crate del prodotto: {names:?}");
    }
    check(&findings_in(&dirs, &SILENT), &SILENT_EXCEPTIONS, "flag che tolgono le conferme di Windows nel prodotto");
}

#[test]
fn the_check_sees_the_silent_flags_in_the_test_tools() {
    // Controllo del controllo: le prove di M1 in `xtask` e lo svuotamento dei VHDX di prova in
    // `quado-testkit` li usano, e li si trova.
    let xtask = findings_in(&[crates_dir().join("../xtask/src")], &SILENT);
    for flag in ["SHERB_NOCONFIRMATION", "FOF_NO_UI"] {
        assert!(xtask.iter().any(|(_, _, bad)| *bad == flag), "{flag}: {xtask:?}");
    }
    let testkit = findings_in(&[crates_dir().join("quado-testkit").join("src")], &SILENT);
    assert!(testkit.iter().any(|(file, _, bad)| file.ends_with("bin.rs") && *bad == "SHERB_NOCONFIRMATION"), "{testkit:?}");
}

#[test]
fn comments_are_ignored_strings_are_not() {
    let src = concat!(
        "//! mai `MsiOpenPackage`\n",
        "/* CreateProcess /* annidato */ WinExec */\n",
        "let a = \"// non è un commento\"; let b = '\"'; fn f<'a>(x: &'a str) {}\n",
        "let c = r#\"ShellExecuteW\"#; // CreateProcess\n",
        "let d = \"CreateProcessW\";\n",
    );
    let code = strip_comments(src);
    assert!(!code.contains("MsiOpenPackage"));
    assert!(!code.contains("WinExec"));
    assert!(code.contains("// non è un commento"));
    assert!(code.contains("ShellExecuteW"));
    assert!(code.contains("\"CreateProcessW\""));
    assert_eq!(code.matches("CreateProcess").count(), 1, "{code}");
    assert_eq!(code.lines().count(), src.lines().count(), "le righe restano allineate");
}
