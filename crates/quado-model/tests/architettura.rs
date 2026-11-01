// SPDX-License-Identifier: GPL-3.0-or-later
//! Test d'architettura (06 §7, 05 D13): il core non produce frasi. Gli errori e gli stati di
//! `quado-model`, `quado-scan-win` e `quado-actions` sono codici e valori; i testi li
//! scrive solo l'interfaccia, dai file di lingua.

use std::path::{Path, PathBuf};

fn sources() -> Vec<PathBuf> {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut out = Vec::new();
    for c in ["quado-model", "quado-scan-win", "quado-actions"] {
        let mut stack = vec![crates.join(c).join("src")];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).expect("cartella dei sorgenti").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// Corpo di una definizione `pub struct`/`pub enum` che comincia a `start`, fino alla graffa chiusa.
fn body(src: &str, start: usize) -> &str {
    let open = src[start..].find(['{', ';', '(']).map_or(src.len(), |k| start + k);
    if src[open..].starts_with(';') {
        return &src[start..open];
    }
    let (o, c) = if src[open..].starts_with('(') { ('(', ')') } else { ('{', '}') };
    let mut depth = 0;
    for (k, ch) in src[open..].char_indices() {
        if ch == o {
            depth += 1;
        } else if ch == c {
            depth -= 1;
            if depth == 0 {
                return &src[start..open + k + 1];
            }
        }
    }
    &src[start..]
}

#[test]
fn core_types_carry_codes_not_sentences() {
    let files = sources();
    assert!(files.len() > 10, "sorgenti trovati: {}", files.len());
    let mut problems = Vec::new();
    for f in files {
        let src = std::fs::read_to_string(&f).expect("sorgente");
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        for bad in ["impl std::fmt::Display", "impl fmt::Display", "impl Display", "impl std::error::Error", "impl Error for"] {
            if src.contains(bad) {
                problems.push(format!("{name}: {bad}"));
            }
        }
        // Tipi di errore e di stato: niente testo dentro.
        for kw in ["pub enum ", "pub struct "] {
            let mut from = 0;
            while let Some(k) = src[from..].find(kw) {
                let start = from + k;
                let ident: String = src[start + kw.len()..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                from = start + kw.len();
                let is_status = ident.ends_with("Error") || ident == "Problem" || ident == "Support" || ident == "Outcome" || ident == "Event";
                if !is_status {
                    continue;
                }
                let b = body(&src, start);
                if b.contains("String") || b.contains("&str") || b.contains("&'static str") {
                    // `Event` di recycle.rs porta il percorso del file: è un dato, non una frase.
                    let only_paths = b.lines().filter(|l| l.contains("String")).all(|l| l.contains("path"));
                    if !only_paths {
                        problems.push(format!("{name}: {ident} contiene testo"));
                    }
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
