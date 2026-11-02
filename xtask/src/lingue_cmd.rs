// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask lingue [nuova <tag>]`: lingue dell'interfaccia.
//!
//! - Senza argomenti: copertura di ogni lingua rispetto all'italiano (chiavi mancanti o in più,
//!   segnaposto diversi, righe con errori, testi ancora uguali all'italiano), per i file incorporati in
//!   `crates/quado-ui/lingue` e per quelli dell'utente in `%APPDATA%\Quado\lingue`. Esce con errore
//!   se una lingua incorporata non è completa.
//! - `nuova <tag>`: scheletro di una lingua nuova in `crates/quado-ui/lingue/<tag>.ftl`, con i commenti
//!   di contesto e i testi italiani da sostituire. Al prossimo `cargo build` la lingua è incorporata.

use std::collections::BTreeMap;
use std::path::PathBuf;

use quado_ui::i18n::{EMBEDDED, REFERENCE, check, embedded};

/// Valori su una riga (`chiave = testo`), per riconoscere i testi rimasti in italiano.
fn single_line_values(src: &str) -> BTreeMap<String, String> {
    src.lines()
        .filter(|l| !l.starts_with([' ', '#', '\t']))
        .filter_map(|l| l.split_once(" = "))
        .filter(|(k, v)| !k.is_empty() && !v.trim().is_empty())
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect()
}

/// `%APPDATA%\Quado\lingue`, quella che legge l'app.
fn user_dir() -> Option<PathBuf> {
    quado_ui::app::settings::data_dir().map(|d| d.join(quado_ui::app::settings::LANG_DIR))
}

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        None | Some("copertura") => coverage(),
        Some("nuova") => new_language(args.get(1).ok_or("lingue nuova <tag>, per esempio: lingue nuova fr")?),
        Some(other) => Err(format!("lingue: comando sconosciuto {other}")),
    }
}

fn coverage() -> Result<(), String> {
    let reference_src = embedded(REFERENCE).ok_or("manca it.ftl")?;
    let reference = check::analyze(reference_src);
    let ref_values = single_line_values(reference_src);
    let mut files: Vec<(String, String, bool)> = EMBEDDED.iter().map(|(t, s)| ((*t).to_owned(), (*s).to_owned(), true)).collect();
    if let Some(dir) = user_dir()
        && let Ok(rd) = std::fs::read_dir(&dir)
    {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "ftl")
                && let (Some(tag), Ok(src)) = (p.file_stem().and_then(|s| s.to_str()), std::fs::read_to_string(&p))
            {
                files.push((format!("{tag} (utente)"), src, false));
            }
        }
    }
    println!("Riferimento: it.ftl, {} chiavi.\n", reference.keys.len());
    let mut failed = false;
    for (tag, src, built_in) in files {
        let r = check::analyze(&src);
        let missing: Vec<&String> = reference.keys.keys().filter(|k| !r.keys.contains_key(*k)).collect();
        let extra: Vec<&String> = r.keys.keys().filter(|k| !reference.keys.contains_key(*k)).collect();
        let vars: Vec<&String> = r.keys.iter().filter(|(k, v)| reference.keys.get(*k).is_some_and(|rv| rv.vars != v.vars)).map(|(k, _)| k).collect();
        let values = single_line_values(&src);
        let same: Vec<&String> = if tag == REFERENCE {
            Vec::new()
        } else {
            values.iter().filter(|(k, v)| ref_values.get(*k) == Some(v) && v.chars().filter(|c| c.is_alphabetic()).count() > 3).map(|(k, _)| k).collect()
        };
        let complete = missing.is_empty() && vars.is_empty() && r.errors.is_empty();
        println!(
            "{tag}: {} chiavi, mancanti {}, in più {}, segnaposto diversi {}, righe con errori {}, uguali all'italiano {}{}",
            r.keys.len(),
            missing.len(),
            extra.len(),
            vars.len(),
            r.errors.len(),
            same.len(),
            if complete { "" } else { "  (incompleta)" }
        );
        for (label, list) in [("mancanti", &missing), ("in più", &extra), ("segnaposto diversi", &vars)] {
            if !list.is_empty() {
                println!("  {label}: {}", list.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            }
        }
        for (line, e) in &r.errors {
            println!("  riga {line}: {e}");
        }
        if built_in && !complete {
            failed = true;
        }
    }
    if failed { Err("una lingua incorporata non è completa".to_owned()) } else { Ok(()) }
}

fn new_language(tag: &str) -> Result<(), String> {
    if tag.parse::<unic_langid::LanguageIdentifier>().is_err() || tag.contains(['/', '\\', '.']) {
        return Err(format!("{tag}: non è un tag di lingua (esempi: fr, de, es, pt-BR)"));
    }
    let path = PathBuf::from("crates/quado-ui/lingue").join(format!("{tag}.ftl"));
    if path.exists() {
        return Err(format!("{} esiste già", path.display()));
    }
    let src = embedded(REFERENCE).ok_or("manca it.ftl")?;
    let mut out = format!(
        "### Quado: testi dell'interfaccia, lingua «{tag}».\n### Scheletro generato da `cargo xtask lingue nuova {tag}` a partire da it.ftl: sostituisci ogni testo\n### italiano con la traduzione, senza cambiare le chiavi né i nomi dei segnaposto ($nome). I commenti\n### spiegano dove compare ogni testo. Poi: `cargo xtask lingue` per la copertura, `cargo build` per incorporarla.\n\n"
    );
    for line in src.lines() {
        if line.starts_with("### ") {
            continue;
        }
        if let Some(v) = line.strip_prefix("lingua-nome = ") {
            out.push_str(&format!("lingua-nome = {tag}\n"));
            let _ = v;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    std::fs::write(&path, out).map_err(|e| e.to_string())?;
    println!("creato {}: traduci i testi, poi cargo xtask lingue", path.display());
    Ok(())
}
