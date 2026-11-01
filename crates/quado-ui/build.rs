// SPDX-License-Identifier: GPL-3.0-or-later
//! Incorpora ogni file `lingue/<tag>.ftl` (05 D13): aggiungere una lingua è aggiungere un file. A ogni
//! lingua si aggiungono i testi delle regole, `../quado-kb/regole/<tag>.ftl`, se ci sono (M3).

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let dir = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("lingue");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut tags: Vec<String> = std::fs::read_dir(&dir)
        .expect("cartella lingue")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "ftl"))
        .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(str::to_owned))
        .collect();
    tags.sort();
    let mut out = String::from("/// File di lingua incorporati, `(tag, testo)`, in ordine di tag (generato da build.rs).\npub const EMBEDDED: &[(&str, &str)] = &[\n");
    let rules = dir.join("..").join("..").join("quado-kb").join("regole");
    for t in &tags {
        let path = dir.join(format!("{t}.ftl"));
        println!("cargo:rerun-if-changed={}", path.display());
        let r = rules.join(format!("{t}.ftl"));
        if r.is_file() {
            println!("cargo:rerun-if-changed={}", r.display());
            let _ = writeln!(out, "    ({t:?}, concat!(include_str!({:?}), \"\\n\", include_str!({:?}))),", path.display().to_string(), r.display().to_string());
        } else {
            let _ = writeln!(out, "    ({t:?}, include_str!({:?})),", path.display().to_string());
        }
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("lingue.rs");
    std::fs::write(dest, out).expect("lingue.rs");
}
