// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask m4-misure`: misure sulla lente Download.
//!
//! - Costo della lettura delle risorse di versione (e delle proprietà MSI) degli installatori di una
//!   cartella, con le stesse funzioni dell'app (`installers_win::version_info`, `msi_info`): 5 esecuzioni,
//!   per ciascuna il tempo totale, poi mediana, minimo e massimo. La prima esecuzione dopo l'avvio del PC è
//!   quella a cache fredda: il documento la riporta a parte.
//! - I due banchi dei frame di `quado` sulla stessa cartella: `--banco installatori` e `--banco hash`
//!   (p50, p95, p99 dal JSON del banco in `target/misure/m4`).
//! - Scrive `target/misure/m4-misure-<cartella>.md` con le condizioni della misura (CPU, RAM, Windows, Defender,
//!   data, versione dell'app).
//!
//! ```text
//! cargo xtask m4-misure --cartella DIR [--escludi "a;b"] [--sottocartelle] [--giri-hash N] [--thread N] [--senza-banchi]
//! cargo xtask m4-misure --genera DIR    cartella di prova: installatori PE generati e file da confrontare
//! ```
//!
//! Le parti di percorso di `--escludi` (senza distinzione di maiuscole) non si leggono mai: qui si saltano
//! prima di aprire il file, nei banchi le rifiuta `Readers::excluding`. Il documento ne riporta solo il
//! numero.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use quado_kb::installers::{self, InstallerKind};
use quado_kb::installers_win::{msi_info, version_info};

struct Options {
    folder: Option<String>,
    exclude: Vec<String>,
    subfolders: bool,
    hash_rounds: u32,
    threads: usize,
    benches: bool,
    generate: Option<String>,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options { folder: None, exclude: Vec::new(), subfolders: false, hash_rounds: 2, threads: 0, benches: true, generate: None };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or_else(|| format!("{a}: manca il valore"));
        match a.as_str() {
            "--cartella" => o.folder = Some(value()?),
            "--escludi" => o.exclude = value()?.split(';').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect(),
            "--sottocartelle" => o.subfolders = true,
            "--giri-hash" => o.hash_rounds = value()?.parse().map_err(|_| "--giri-hash: un numero")?,
            "--thread" => o.threads = value()?.parse().map_err(|_| "--thread: un numero")?,
            "--senza-banchi" => o.benches = false,
            "--genera" => o.generate = Some(value()?),
            other => return Err(format!("argomento sconosciuto: {other}")),
        }
    }
    Ok(o)
}

pub fn run(args: &[String]) -> Result<(), String> {
    let o = parse(args)?;
    if let Some(dir) = &o.generate {
        return generate(Path::new(dir));
    }
    let folder = o.folder.clone().ok_or("uso: cargo xtask m4-misure --cartella DIR [--escludi \"a;b\"] | --genera DIR")?;
    if !Path::new(&folder).is_dir() {
        return Err(format!("{folder}: non è una cartella"));
    }
    let mut out = String::new();
    let name = folder_name(&folder);
    let _ = writeln!(out, "# Misure della lente Download su `{name}`\n");
    let _ = writeln!(out, "Generato da `cargo xtask m4-misure`.\n");
    let _ = writeln!(out, "| Condizione | Valore |\n|---|---|");
    for (k, v) in crate::measure_cmd::environment() {
        let _ = writeln!(out, "| {k} | {v} |");
    }
    let _ = writeln!(out, "| Cartella | `{folder}`{} |", if o.subfolders { ", con un livello di sottocartelle" } else { "" });
    let _ = writeln!(out, "| Esclusioni | {} parti di percorso, non lette (non scritte qui) |\n", o.exclude.len());

    eprintln!("risorse di versione: 5 esecuzioni…");
    versions(&mut out, &folder, o.subfolders, &o.exclude);

    if o.benches {
        let _ = writeln!(out, "\n## Frame durante le letture\n");
        let _ = writeln!(out, "Banchi di `quado` con le letture vere, finestra 1920×1080 senza sincronizzazione verticale. Frame della fase «lettura»: dalla richiesta alla fine dell'ultimo giro, comprese la conferma e la ricostruzione della lente.\n");
        let _ = writeln!(out, "| Banco | Frame | p50 | p95 | p99 | Massimo | Criterio p95 ≤ 16 ms | Giri, file e byte letti, durata dei giri |\n|---|---|---|---|---|---|---|---|");
        let exe = build_app()?;
        let dir = PathBuf::from("target/misure/m4");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let threads = if o.threads > 0 { o.threads.to_string() } else { "predefiniti".to_owned() };
        for (kind, rounds, label) in [("installatori", 5, "installatori".to_owned()), ("hash", o.hash_rounds, format!("hash (thread: {threads})"))] {
            eprintln!("banco {kind}…");
            match bench(&exe, kind, &folder, &o.exclude, rounds, o.threads, &dir) {
                Ok(b) => {
                    let _ = writeln!(
                        out,
                        "| {label} | {:.0} | {:.1} ms | {:.1} ms | {:.1} ms | {:.1} ms | {} | {} |",
                        b.n,
                        b.p50,
                        b.p95,
                        b.p99,
                        b.max,
                        if b.passed { "superato" } else { "**non superato**" },
                        b.effects
                    );
                }
                Err(e) => {
                    let _ = writeln!(out, "| {label} | | | | | | non misurato: {e} | |");
                }
            }
        }
        let _ = writeln!(out, "\nI file JSON dei banchi sono in `target/misure/m4`.");
    }
    std::fs::create_dir_all("target/misure").map_err(|e| e.to_string())?;
    let path = format!("target/misure/m4-misure-{name}.md");
    std::fs::write(&path, &out).map_err(|e| e.to_string())?;
    println!("{out}");
    eprintln!("scritto {path}");
    Ok(())
}

/// Ultimo componente del percorso, con i soli caratteri adatti a un nome di file.
fn folder_name(folder: &str) -> String {
    let last = folder.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or("cartella");
    let s: String = last.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    if s.is_empty() { "cartella".to_owned() } else { s }
}

fn excluded(path: &str, exclude: &[String]) -> bool {
    let p = path.to_lowercase();
    exclude.iter().any(|x| p.contains(x.as_str()))
}

/// Installatori exe e msi della cartella (primo livello; con `subfolders` anche un livello sotto), senza
/// gli esclusi. Restituisce anche quanti sono stati esclusi.
fn installer_files(folder: &str, subfolders: bool, exclude: &[String]) -> (Vec<String>, usize) {
    let list = |d: &Path| -> Vec<PathBuf> { std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default() };
    let mut paths = list(Path::new(folder));
    if subfolders {
        let subs: Vec<PathBuf> = paths.iter().filter(|p| p.is_dir()).cloned().collect();
        paths.extend(subs.iter().flat_map(|s| list(s)));
    }
    let all: Vec<String> =
        paths.iter().filter(|p| p.is_file()).map(|p| p.to_string_lossy().into_owned()).filter(|p| installers::installer_kind(p).is_some()).collect();
    let skipped = all.iter().filter(|p| excluded(p, exclude)).count();
    let mut files: Vec<String> = all.into_iter().filter(|p| !excluded(p, exclude)).collect();
    files.sort();
    (files, skipped)
}

fn stats(v: &[f64]) -> (f64, f64, f64) {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    if s.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let med = if s.len() % 2 == 1 { s[s.len() / 2] } else { (s[s.len() / 2 - 1] + s[s.len() / 2]) / 2.0 };
    (med, s[0], s[s.len() - 1])
}

fn quantile(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    if s.is_empty() { 0.0 } else { s[((s.len() - 1) as f64 * q).round() as usize] }
}

/// Costo della lettura delle risorse di versione: 5 esecuzioni sugli stessi file.
fn versions(out: &mut String, folder: &str, subfolders: bool, exclude: &[String]) {
    let (files, skipped) = installer_files(folder, subfolders, exclude);
    let msi = files.iter().filter(|p| installers::installer_kind(p) == Some(InstallerKind::Msi)).count();
    let _ = writeln!(out, "## Costo della lettura delle risorse di versione\n");
    let _ = writeln!(
        out,
        "Installatori letti: {} ({msi} MSI con la tabella Property, gli altri con la risorsa di versione); esclusi: {skipped}. Stesse funzioni dell'app, un file alla volta, nessuna cache.\n",
        files.len()
    );
    if files.is_empty() {
        let _ = writeln!(out, "Nessun installatore da leggere: misura non fatta.");
        return;
    }
    let mut totals = Vec::new();
    let mut per_file: Vec<Vec<f64>> = vec![Vec::new(); 5];
    let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
    for round in 0..5 {
        let start = Instant::now();
        for p in &files {
            let t = Instant::now();
            let r = match installers::installer_kind(p) {
                Some(InstallerKind::Msi) => msi_info(p),
                _ => version_info(p),
            };
            per_file[round].push(t.elapsed().as_secs_f64() * 1000.0);
            if round == 0 {
                let what = match &r {
                    Ok(i) if i.product_name.is_some() => "con nome del prodotto".to_owned(),
                    Ok(_) => "senza nome del prodotto".to_owned(),
                    Err(e) => format!("errore {e:?}"),
                };
                *outcomes.entry(what).or_default() += 1;
            }
        }
        totals.push(start.elapsed().as_secs_f64() * 1000.0);
        eprintln!("esecuzione {}: {:.0} ms", round + 1, totals[round]);
    }
    let _ = writeln!(out, "| Esecuzione | Tempo totale | Per file: mediana | Per file: p95 | Per file: massimo |\n|---|---|---|---|---|");
    for (k, t) in totals.iter().enumerate() {
        let note = if k == 0 { " (la più fredda)" } else { "" };
        let v = &per_file[k];
        let _ = writeln!(out, "| {}{note} | {t:.1} ms | {:.2} ms | {:.2} ms | {:.2} ms |", k + 1, quantile(v, 0.5), quantile(v, 0.95), stats(v).2);
    }
    let (med, min, max) = stats(&totals);
    let warm: Vec<f64> = per_file[1..].iter().flatten().copied().collect();
    let _ = writeln!(out, "\nTempo totale, 5 esecuzioni: mediana {med:.1} ms, minimo {min:.1} ms, massimo {max:.1} ms.");
    let _ = writeln!(out, "Per file, esecuzioni 2-5: mediana {:.2} ms, p95 {:.2} ms.", quantile(&warm, 0.5), quantile(&warm, 0.95));
    let _ = writeln!(out, "La prima esecuzione è a cache fredda solo se è la prima lettura di questi file dopo l'avvio del PC.");
    let list: Vec<String> = outcomes.iter().map(|(k, n)| format!("{k}: {n}")).collect();
    let _ = writeln!(out, "Esiti della prima esecuzione: {}.", list.join("; "));
}

/// Compila `quado` in release (come `verifica-dll`).
fn build_app() -> Result<PathBuf, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let s = Command::new(&cargo).args(["build", "--release", "--quiet", "-p", "quado", "--features", "misure"]).status().map_err(|e| e.to_string())?;
    if !s.success() {
        return Err("build di quado fallita".into());
    }
    Ok(PathBuf::from(r"target\release\quado.exe"))
}

struct BenchResult {
    n: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    max: f64,
    passed: bool,
    effects: String,
}

/// Numero dopo `"chiave": ` a partire da `from`.
fn number_after(s: &str, key: &str) -> Option<f64> {
    let k = format!("\"{key}\": ");
    let at = s.find(&k)? + k.len();
    let num: String = s[at..].chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    num.parse().ok()
}

/// Avvia il banco e legge il suo JSON (il file nuovo in `dir`).
fn bench(exe: &Path, kind: &str, folder: &str, exclude: &[String], rounds: u32, threads: usize, dir: &Path) -> Result<BenchResult, String> {
    let names = |d: &Path| -> HashSet<PathBuf> { std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default() };
    let before = names(dir);
    let mut c = Command::new(exe);
    c.args(["--banco", kind, "--cartella", folder, "--giri", &rounds.to_string(), "--out"]).arg(dir);
    if !exclude.is_empty() {
        c.args(["--escludi", &exclude.join(";")]);
    }
    if threads > 0 {
        c.args(["--thread", &threads.to_string()]);
    }
    let s = c.status().map_err(|e| e.to_string())?;
    if !s.success() {
        return Err(format!("quado è uscito con {s}"));
    }
    let prefix = format!("m4-{kind}-");
    let file = names(dir)
        .into_iter()
        .filter(|p| !before.contains(p) && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&prefix)))
        .max()
        .ok_or("il banco non ha scritto il JSON")?;
    let json = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let crit = &json[json.find("\"criterio\": {\"fasi\"").ok_or("JSON senza criterio")?..];
    let ms = &crit[crit.find("\"intervallo_ms\"").ok_or("JSON senza intervalli")?..];
    let read = json.find("\"nome\": \"lettura\"").map(|k| &json[k..]).unwrap_or_default();
    let effects = read.find("\"effetti\": {").map_or(String::new(), |k| {
        let rest = &read[k + 12..];
        rest[..rest.find('}').unwrap_or(rest.len())].replace('"', "")
    });
    Ok(BenchResult {
        n: number_after(ms, "n").unwrap_or(0.0),
        p50: number_after(ms, "p50").unwrap_or(0.0),
        p95: number_after(ms, "p95").unwrap_or(0.0),
        p99: number_after(ms, "p99").unwrap_or(0.0),
        max: number_after(ms, "max").unwrap_or(0.0),
        passed: crit.contains("\"superato\": true"),
        effects,
    })
}

// ---------------------------------------------------------------------------------------------
// Cartella di prova: installatori PE generati (come in `quado-kb/tests/installatori.rs`, risorsa di
// versione scritta qui, nessun codice) e file da confrontare. Mai sui file dell'utente.

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
}

fn pad4(v: &mut Vec<u8>) {
    while !v.len().is_multiple_of(4) {
        v.push(0);
    }
}

fn block(key: &str, text: bool, value: &[u8], value_len: u16, children: &[Vec<u8>]) -> Vec<u8> {
    let mut b = vec![0, 0];
    b.extend(value_len.to_le_bytes());
    b.extend(u16::from(text).to_le_bytes());
    b.extend(utf16z(key));
    pad4(&mut b);
    b.extend(value);
    for c in children {
        pad4(&mut b);
        b.extend(c);
    }
    let len = b.len().min(u16::MAX as usize) as u16;
    b[0..2].copy_from_slice(&len.to_le_bytes());
    b
}

/// Risorsa di versione con nome, versione ed editore.
fn version_resource(product: &str, version: [u16; 4]) -> Vec<u8> {
    let v = format!("{}.{}.{}.{}", version[0], version[1], version[2], version[3]);
    let string = |k: &str, val: &str| {
        let x = utf16z(val);
        block(k, true, &x, (x.len() / 2) as u16, &[])
    };
    let strings = vec![string("CompanyName", "Editore di prova"), string("FileVersion", &v), string("ProductName", product), string("ProductVersion", &v)];
    let table = block("040904B0", true, &[], 0, &strings);
    let var = block("Translation", false, &[0x0409u16.to_le_bytes(), 0x04B0u16.to_le_bytes()].concat(), 4, &[]);
    let ms = (u32::from(version[0]) << 16) | u32::from(version[1]);
    let ls = (u32::from(version[2]) << 16) | u32::from(version[3]);
    let fixed: Vec<u8> = [0xFEEF_04BD_u32, 0x0001_0000, ms, ls, ms, ls, 0x3F, 0, 0x0004_0004, 1, 0, 0, 0].iter().flat_map(|w| w.to_le_bytes()).collect();
    block("VS_VERSION_INFO", false, &fixed, fixed.len() as u16, &[block("StringFileInfo", true, &[], 0, &[table]), block("VarFileInfo", true, &[], 0, &[var])])
}

/// PE32+ minimo con una sola sezione (`.rsrc` con la risorsa di versione, oppure `.rdata` vuota) e `extra`
/// byte di dati aggiunti in fondo, come un installatore con l'archivio dopo le sezioni.
fn pe(resource: Option<&[u8]>, extra: usize) -> Vec<u8> {
    const RVA: u32 = 0x1000;
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend(x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend(x.to_le_bytes());
    let le64 = |v: &mut Vec<u8>, x: u64| v.extend(x.to_le_bytes());
    let section = match resource {
        Some(data) => {
            let mut s = Vec::new();
            let dir = |s: &mut Vec<u8>, id: u32, target: u32| {
                for _ in 0..2 {
                    le32(s, 0);
                }
                for x in [0, 0, 0, 1] {
                    le16(s, x);
                }
                le32(s, id);
                le32(s, target);
            };
            dir(&mut s, 16, 0x8000_0000 | 24);
            dir(&mut s, 1, 0x8000_0000 | 48);
            dir(&mut s, 0x0409, 72);
            le32(&mut s, RVA + 88);
            le32(&mut s, data.len() as u32);
            le32(&mut s, 0);
            le32(&mut s, 0);
            s.extend(data);
            s
        }
        None => vec![0u8; 16],
    };
    let virt = section.len();
    let raw = virt.div_ceil(0x200) * 0x200;
    let mut f = vec![0u8; 0x40];
    f[0..2].copy_from_slice(b"MZ");
    f[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    f.extend(b"PE\0\0");
    le16(&mut f, 0x8664);
    le16(&mut f, 1);
    for _ in 0..3 {
        le32(&mut f, 0);
    }
    le16(&mut f, 240);
    le16(&mut f, 0x0022);
    le16(&mut f, 0x20B);
    f.extend([14, 0]);
    le32(&mut f, 0);
    le32(&mut f, raw as u32);
    le32(&mut f, 0);
    le32(&mut f, 0);
    le32(&mut f, RVA);
    le64(&mut f, 0x1_4000_0000);
    le32(&mut f, 0x1000);
    le32(&mut f, 0x200);
    for v in [6, 0, 0, 0, 6, 0] {
        le16(&mut f, v);
    }
    le32(&mut f, 0);
    le32(&mut f, RVA + (virt.div_ceil(0x1000) * 0x1000) as u32);
    le32(&mut f, 0x200);
    le32(&mut f, 0);
    le16(&mut f, 2);
    le16(&mut f, 0x8160);
    for v in [0x10_0000u64, 0x1000, 0x10_0000, 0x1000] {
        le64(&mut f, v);
    }
    le32(&mut f, 0);
    le32(&mut f, 16);
    for k in 0..16 {
        let (rva, size) = if k == 2 && resource.is_some() { (RVA, virt as u32) } else { (0, 0) };
        le32(&mut f, rva);
        le32(&mut f, size);
    }
    f.extend(if resource.is_some() { *b".rsrc\0\0\0" } else { *b".rdata\0\0" });
    le32(&mut f, virt as u32);
    le32(&mut f, RVA);
    le32(&mut f, raw as u32);
    le32(&mut f, 0x200);
    for _ in 0..2 {
        le32(&mut f, 0);
    }
    le16(&mut f, 0);
    le16(&mut f, 0);
    le32(&mut f, 0x4000_0040);
    f.resize(0x200, 0);
    f.extend(&section);
    f.resize(0x200 + raw + extra, 0);
    f
}

/// File di `size` byte con contenuto pseudocasuale deterministico da `seed`, scritto a blocchi.
fn write_random(path: &Path, size: u64, seed: u64) -> Result<(), String> {
    use std::io::Write as _;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path).map_err(|e| e.to_string())?);
    let mut x = seed | 1;
    let mut buf = vec![0u8; 1 << 20];
    let mut left = size;
    while left > 0 {
        for chunk in buf.chunks_mut(8) {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let b = x.to_le_bytes();
            chunk.copy_from_slice(&b[..chunk.len()]);
        }
        let n = left.min(buf.len() as u64) as usize;
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        left -= n as u64;
    }
    f.flush().map_err(|e| e.to_string())
}

/// Cartella di prova per `m4-misure`: 24 installatori con risorsa di versione e 4 senza (dati aggiunti fino
/// a qualche MiB), tre copie identiche da 256 MiB, due file da 128 MiB con la stessa dimensione e contenuto
/// diverso, un file unico, un download incompleto. La cartella deve essere nuova o vuota.
fn generate(dir: &Path) -> Result<(), String> {
    if dir.exists() && std::fs::read_dir(dir).map_err(|e| e.to_string())?.next().is_some() {
        return Err(format!("{}: la cartella esiste e non è vuota", dir.display()));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    const MIB: usize = 1 << 20;
    for k in 0..24u16 {
        let res = version_resource(&format!("Programma di prova {k}"), [1, k, 0, 0]);
        let bytes = pe(Some(&res), (1 + usize::from(k) % 6) * MIB);
        std::fs::write(dir.join(format!("installatore-{k:02}.exe")), bytes).map_err(|e| e.to_string())?;
    }
    for k in 0..4 {
        std::fs::write(dir.join(format!("senza-risorsa-{k}.exe")), pe(None, MIB)).map_err(|e| e.to_string())?;
    }
    let copy = dir.join("copia.bin");
    write_random(&copy, 256 << 20, 7)?;
    for name in ["copia (1).bin", "copia (2).bin"] {
        std::fs::copy(&copy, dir.join(name)).map_err(|e| e.to_string())?;
    }
    write_random(&dir.join("stessa-dimensione-a.bin"), 128 << 20, 11)?;
    write_random(&dir.join("stessa-dimensione-b.bin"), 128 << 20, 13)?;
    write_random(&dir.join("unico.bin"), 64 << 20, 17)?;
    write_random(&dir.join("film.mp4.crdownload"), 1 << 20, 19)?;
    println!("cartella di prova pronta: {}", dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_installers_are_read_like_real_ones() {
        let dir = std::env::temp_dir().join(format!("quado-m4-prova-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("cartella");
        let p = dir.join("prova.exe");
        std::fs::write(&p, pe(Some(&version_resource("Programma di prova 3", [1, 3, 0, 0])), 4096)).expect("scrittura");
        let info = version_info(&p.to_string_lossy()).expect("risorsa di versione");
        assert_eq!(info.product_name.as_deref(), Some("Programma di prova 3"));
        assert_eq!(info.product_version.as_deref(), Some("1.3.0.0"));
        let q = dir.join("vuoto.exe");
        std::fs::write(&q, pe(None, 0)).expect("scrittura");
        assert_eq!(version_info(&q.to_string_lossy()).map(|i| i.product_name), Ok(None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_and_exclusions() {
        assert_eq!(folder_name(r"C:\Users\u\Downloads\"), "Downloads");
        assert_eq!(folder_name(r"C:\Temp\m4 prova"), "m4-prova");
        assert!(excluded(r"C:\Users\u\Downloads\App Uno\x.exe", &["app uno".into()]));
        assert!(!excluded(r"C:\Users\u\Downloads\altro.exe", &["app uno".into()]));
    }
}
