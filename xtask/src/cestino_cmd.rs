// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask cestino`: verifiche del Cestino sul VHDX NTFS di prova (07 §3 righe 1-3).
//!
//! Lavora solo su `R:\` (il volume di prova): crea file propri, li sposta nel Cestino di quel volume,
//! riduce per la durata della prova la capacità del Cestino di quel solo volume e alla fine la
//! ripristina. Scrive il resoconto su standard output (Markdown).
//!
//! Spostamento senza finestre e svuotamento senza conferme stanno nel sottomodulo `win`, fuori dal
//! prodotto; dal prodotto viene solo la lettura del Cestino (`quado_actions::recycle::query`).

mod win;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use quado_actions::recycle::TSF_DELETE_RECYCLE_IF_POSSIBLE;
use quado_testkit::trees;
use win::{self as recycle, Event, Guard, Outcome};

const CAPACITY_MB: u32 = 20;
const MB: usize = 1 << 20;

fn volume_guid(root: &str) -> Result<String, String> {
    let out = Command::new("mountvol").arg(root.trim_end_matches('\\')).arg("/L").output().map_err(|e| e.to_string())?;
    let s = String::from_utf8_lossy(&out.stdout);
    let start = s.find('{').ok_or(format!("GUID del volume non trovato: {s}"))?;
    let end = s[start..].find('}').ok_or("GUID incompleto")? + start;
    Ok(s[start..=end].to_owned())
}

fn bitbucket_key(guid: &str) -> String {
    format!(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume\{guid}")
}

fn reg_query_dword(key: &str, value: &str) -> Option<u32> {
    let out = Command::new("reg").args(["query", key, "/v", value]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let hex = s.split_whitespace().find(|w| w.starts_with("0x"))?;
    u32::from_str_radix(hex.trim_start_matches("0x"), 16).ok()
}

fn reg_set_dword(key: &str, value: &str, v: u32) -> bool {
    Command::new("reg").args(["add", key, "/v", value, "/t", "REG_DWORD", "/d", &v.to_string(), "/f"]).output().is_ok_and(|o| o.status.success())
}

fn reg_delete_value(key: &str, value: &str) {
    let _ = Command::new("reg").args(["delete", key, "/v", value, "/f"]).output();
}

/// Cartella del Cestino dell'utente su `root` (la sola leggibile tra quelle per SID).
fn user_bin(root: &Path) -> Option<PathBuf> {
    std::fs::read_dir(root.join("$Recycle.Bin")).ok()?.flatten().map(|e| e.path()).find(|p| std::fs::read_dir(p).is_ok())
}

fn size_of_path(p: &Path) -> u64 {
    if p.is_dir() {
        std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| size_of_path(&e.path())).sum()).unwrap_or(0)
    } else {
        p.metadata().map(|m| m.len()).unwrap_or(0)
    }
}

/// Contenuto del Cestino su disco: (nome, byte logici) per ogni `$I` e `$R`.
fn bin_contents(root: &Path) -> Vec<(String, u64)> {
    let Some(bin) = user_bin(root) else { return Vec::new() };
    let mut v: Vec<(String, u64)> = std::fs::read_dir(bin)
        .map(|rd| rd.flatten().filter(|e| e.file_name() != "desktop.ini").map(|e| (e.file_name().to_string_lossy().into_owned(), size_of_path(&e.path()))).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn describe(o: &Outcome, out: &mut String) {
    for e in &o.events {
        match e {
            Event::Pre { path, flags, rejected } => {
                let _ = writeln!(
                    out,
                    "  - PreDeleteItem `{}`: flag {flags:#x} ({}){}",
                    Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    if flags & TSF_DELETE_RECYCLE_IF_POSSIBLE != 0 { "nel Cestino" } else { "eliminazione definitiva" },
                    if *rejected { ", rifiutato" } else { "" }
                );
            }
            Event::Post { path, flags, hr, recycled } => {
                let _ = writeln!(
                    out,
                    "  - PostDeleteItem `{}`: flag {flags:#x}, esito {:#010x}, nel Cestino: {recycled}",
                    Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                    hr.0 as u32
                );
            }
        }
    }
    let _ = writeln!(out, "  - PerformOperations {:#010x}, operazioni annullate: {}", o.perform.0 as u32, o.aborted);
}

fn exists(paths: &[PathBuf]) -> String {
    paths.iter().map(|p| format!("{} {}", p.file_name().unwrap().to_string_lossy(), if p.exists() { "presente" } else { "assente" })).collect::<Vec<_>>().join(", ")
}

pub fn run() -> Result<(), String> {
    let (_, root) = trees::mounted_vhdx().into_iter().find(|(n, _)| n == "ntfs").ok_or("VHDX NTFS non montato: cargo xtask vhdx crea")?;
    let root_s = root.to_string_lossy().into_owned();
    let guid = volume_guid(&root_s)?;
    let key = bitbucket_key(&guid);
    let original = reg_query_dword(&key, "MaxCapacity");
    if !reg_set_dword(&key, "MaxCapacity", CAPACITY_MB) {
        return Err("impossibile impostare MaxCapacity per il volume di prova".into());
    }
    let work = root.join("cestino-prove");
    let mut out = String::new();
    let result = (|| -> Result<(), String> {
        let _ = writeln!(out, "## Verifiche del Cestino su {root_s} (volume {guid}, Cestino ridotto a {CAPACITY_MB} MB)\n");
        let reset = |work: &Path| {
            let _ = recycle::empty(&root);
            let _ = std::fs::remove_dir_all(work);
            std::fs::create_dir_all(work).unwrap();
        };

        // --- 07 §3 riga 2: PreDeleteItem e codice di rifiuto del singolo elemento.
        let _ = writeln!(out, "### Riga 2: `PreDeleteItem` e rifiuto del singolo elemento\n");
        let _ = writeln!(out, "Tre elementi in un'unica operazione: `a.bin` (2 MB), `grande.bin` (30 MB, oltre la capacità), `b.bin` (2 MB).\n");
        let mut runs: Vec<(&str, Guard)> = vec![("nessuna protezione (controllo)", Guard::None)];
        runs.extend(recycle::candidate_codes().into_iter().map(|(n, hr)| (n, Guard::Reject(hr))));
        for (name, guard) in runs {
            reset(&work);
            let files: Vec<PathBuf> = [("a.bin", 2), ("grande.bin", 30), ("b.bin", 2)]
                .iter()
                .map(|(n, mb)| {
                    let p = work.join(n);
                    trees::write_file(&p, mb * MB, 1);
                    p
                })
                .collect();
            let o = recycle::recycle(&files, guard).map_err(|e| e.to_string())?;
            let _ = writeln!(out, "- **{name}**: dopo l'operazione {}", exists(&files));
            describe(&o, &mut out);
            let (items, bytes) = recycle::query(&root).unwrap_or((-1, -1));
            let _ = writeln!(out, "  - Cestino secondo Windows: {items} elementi, {bytes} byte\n");
        }

        // --- 07 §3 riga 3a: dimensione logica o allocata per decidere "troppo grande".
        let _ = writeln!(out, "### Riga 3: come Windows conta la capacità\n");
        reset(&work);
        let sparse = work.join("sparso.bin");
        trees::sparse_file(&sparse, 50 * MB as u64);
        let comp = work.join("compresso.txt");
        trees::write_compressible(&comp, 30 * MB);
        trees::ntfs_compress(&comp);
        let alloc = |p: &Path| -> String {
            let out = Command::new("powershell")
                .args(["-NoProfile", "-Command", &format!("(Get-Item '{}').Length; (compact '{}' | Select-String 'ratio')", p.display(), p.display())])
                .output();
            out.map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().take(6).collect::<Vec<_>>().join(" ")).unwrap_or_default()
        };
        let _ = writeln!(out, "- `sparso.bin`: logica 50 MB, quasi nessun cluster allocato; `compresso.txt`: logica 30 MB, compresso NTFS ({})", alloc(&comp));
        for p in [&sparse, &comp] {
            let o = recycle::recycle(std::slice::from_ref(p), Guard::Reject(recycle::candidate_codes()[0].1)).map_err(|e| e.to_string())?;
            let _ = writeln!(out, "- `{}`:", p.file_name().unwrap().to_string_lossy());
            describe(&o, &mut out);
        }

        // --- 07 §3 riga 3b: cosa elimina Windows per fare spazio.
        reset(&work);
        let _ = writeln!(out, "\n- Riempimento: tre file da 8 MB spostati uno alla volta in un Cestino da {CAPACITY_MB} MB.");
        for k in 1..=3 {
            let p = work.join(format!("f{k}.bin"));
            trees::write_file(&p, 8 * MB, k as u8);
            let o = recycle::recycle(&[p], Guard::Reject(recycle::candidate_codes()[0].1)).map_err(|e| e.to_string())?;
            describe(&o, &mut out);
            let (items, bytes) = recycle::query(&root).unwrap_or((-1, -1));
            let _ = writeln!(out, "  - dopo f{k}: Cestino secondo Windows {items} elementi, {bytes} byte; su disco {:?}", bin_contents(&root));
        }

        // --- Cartella orfana (contata o no) e svuotamento (07 §3 righe 3 e 1).
        let _ = writeln!(out, "\n### Riga 3 e riga 1: cartella orfana\n");
        reset(&work);
        let folder = work.join("cartella12");
        for k in 0..3 {
            trees::write_file(&folder.join(format!("p{k}.bin")), 4 * MB, 9);
        }
        let o = recycle::recycle(&[folder], Guard::Reject(recycle::candidate_codes()[0].1)).map_err(|e| e.to_string())?;
        describe(&o, &mut out);
        let bin = user_bin(&root).ok_or("Cestino dell'utente non trovato")?;
        let info_file = std::fs::read_dir(&bin)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("$I")))
            .ok_or("$I non trovato")?;
        std::fs::remove_file(&info_file).map_err(|e| e.to_string())?;
        let (items, bytes) = recycle::query(&root).unwrap_or((-1, -1));
        let _ = writeln!(out, "- Tolto `{}`: ora la cartella è orfana. Windows vede {items} elementi, {bytes} byte; su disco {:?}", info_file.file_name().unwrap().to_string_lossy(), bin_contents(&root));
        let f4 = work.join("f4.bin");
        trees::write_file(&f4, 10 * MB, 4);
        let o = recycle::recycle(&[f4], Guard::Reject(recycle::candidate_codes()[0].1)).map_err(|e| e.to_string())?;
        let _ = writeln!(out, "- Spostato `f4.bin` (10 MB): con l'orfana da 12 MB il totale supererebbe {CAPACITY_MB} MB se l'orfana contasse.");
        describe(&o, &mut out);
        let (items, bytes) = recycle::query(&root).unwrap_or((-1, -1));
        let _ = writeln!(out, "  - Windows vede {items} elementi, {bytes} byte; su disco {:?}", bin_contents(&root));
        let empty = recycle::empty(&root);
        let _ = writeln!(out, "- Svuotamento del Cestino (`SHEmptyRecycleBinW`, lo stesso dell'opzione di Windows): esito {:?}; su disco dopo: {:?}", empty.map_err(|e| e.code().0 as u32), bin_contents(&root));
        Ok(())
    })();
    // Ripristino della capacità originale e pulizia, sempre.
    match original {
        Some(v) => {
            reg_set_dword(&key, "MaxCapacity", v);
        }
        None => reg_delete_value(&key, "MaxCapacity"),
    }
    let _ = recycle::empty(&root);
    let _ = std::fs::remove_dir_all(&work);
    let _ = writeln!(out, "\nCapacità ripristinata: {}", original.map_or("valore assente, come prima".to_owned(), |v| format!("{v} MB")));
    println!("{out}");
    result
}

/// Seconda serie, mirata (07 §3 righe 2 e 3): esito di ogni elemento letto anche dal contenuto del Cestino.
pub fn run_followup() -> Result<(), String> {
    let (_, root) = trees::mounted_vhdx().into_iter().find(|(n, _)| n == "ntfs").ok_or("VHDX NTFS non montato")?;
    let guid = volume_guid(&root.to_string_lossy())?;
    let key = bitbucket_key(&guid);
    let original = reg_query_dword(&key, "MaxCapacity");
    reg_set_dword(&key, "MaxCapacity", CAPACITY_MB);
    let work = root.join("cestino-prove");
    let mut out = String::from("## Seconda serie: esito per elemento, letto dal Cestino su disco\n\n");
    let result = (|| -> Result<(), String> {
        type Make = fn(&Path) -> PathBuf;
        let small: Make = |w| {
            let p = w.join("a.bin");
            trees::write_file(&p, 2 * MB, 1);
            p
        };
        let small_b: Make = |w| {
            let p = w.join("b.bin");
            trees::write_file(&p, 2 * MB, 2);
            p
        };
        let big: Make = |w| {
            let p = w.join("grande.bin");
            trees::write_file(&p, 30 * MB, 3);
            p
        };
        let sparse: Make = |w| {
            let p = w.join("sparso.bin");
            trees::sparse_file(&p, 50 * MB as u64);
            p
        };
        let comp: Make = |w| {
            let p = w.join("compresso.txt");
            trees::write_compressible(&p, 30 * MB);
            trees::ntfs_compress(&p);
            p
        };
        let cases: Vec<(&str, Vec<Make>)> = vec![
            ("A: solo a.bin (2 MB)", vec![small]),
            ("B: a.bin e b.bin (2 + 2 MB) insieme", vec![small, small_b]),
            ("C: a.bin poi grande.bin (2 + 30 MB) insieme", vec![small, big]),
            ("D: grande.bin poi a.bin insieme", vec![big, small]),
            ("E: solo sparso.bin (50 MB logici, quasi nulla allocato)", vec![sparse]),
            ("F: solo compresso.txt (30 MB logici, circa 4 MB allocati)", vec![comp]),
        ];
        for (name, makers) in cases {
            let _ = recycle::empty(&root);
            let _ = std::fs::remove_dir_all(&work);
            std::fs::create_dir_all(&work).unwrap();
            let files: Vec<PathBuf> = makers.iter().map(|m| m(&work)).collect();
            let o = recycle::recycle(&files, Guard::None).map_err(|e| e.to_string())?;
            let _ = writeln!(out, "- **{name}**: dopo {}", exists(&files));
            describe(&o, &mut out);
            let (items, bytes) = recycle::query(&root).unwrap_or((-1, -1));
            let _ = writeln!(out, "  - Cestino: Windows {items} elementi, {bytes} byte; su disco {:?}\n", bin_contents(&root));
        }
        Ok(())
    })();
    match original {
        Some(v) => {
            reg_set_dword(&key, "MaxCapacity", v);
        }
        None => reg_delete_value(&key, "MaxCapacity"),
    }
    let _ = recycle::empty(&root);
    let _ = std::fs::remove_dir_all(&work);
    println!("{out}");
    result
}
