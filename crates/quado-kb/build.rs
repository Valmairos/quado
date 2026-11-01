// SPDX-License-Identifier: GPL-3.0-or-later
//! Incorpora le regole: ogni file `regole/*.toml` diventa una voce di `RULE_FILES` (05 D6). Aggiungere
//! una regola è aggiungere un file, con i testi in `regole/it.ftl` ed `regole/en.ftl`.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("regole");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("cartella regole")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    files.sort();
    let mut out = String::from("/// File delle regole incorporati: (nome del file, testo).\npub static RULE_FILES: &[(&str, &str)] = &[\n");
    for f in files {
        println!("cargo:rerun-if-changed={}", f.display());
        let name = f.file_name().and_then(|n| n.to_str()).expect("nome");
        let _ = writeln!(out, "    ({name:?}, include_str!({:?})),", f.display().to_string());
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("regole.rs");
    std::fs::write(dest, out).expect("regole.rs");
}
