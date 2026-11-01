// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask rustflags-dist`: stampa la stringa RUSTFLAGS completa per l'eseguibile pubblicato (profilo
//! `dist`), la stessa che usa `cargo xtask installa`.
//!
//! Con RUSTFLAGS (o CARGO_ENCODED_RUSTFLAGS) cargo ignora i `rustflags` di `.cargo/config.toml`: la stringa
//! li ripete (oggi `-C target-feature=+crt-static`, 05 D9) e aggiunge un `--remap-path-prefix` per ogni
//! cartella che finirebbe nell'eseguibile con i messaggi di panic delle dipendenze (`file!()`,
//! `Location::caller`) e con i file generati in `OUT_DIR`:
//! - la cartella di cargo (`CARGO_HOME`, con il registro dei sorgenti) diventa `cargo`;
//! - la radice del repository (e i file generati sotto `target`) diventa `quado`;
//! - la cartella di rustup (`RUSTUP_HOME`), se esiste, diventa `rustup`;
//! - `CARGO_TARGET_DIR`, se è fuori dal repository, diventa `target`.
//!
//! I prefissi si calcolano a ogni build, così nessun percorso di una macchina finisce nel repository. La
//! CI di rilascio usa lo stesso comando: `RUSTFLAGS=$(cargo xtask rustflags-dist)`.

use std::path::{Path, PathBuf};

/// Target dei `rustflags` di `.cargo/config.toml`.
const TARGET_SECTION: &str = "[target.x86_64-pc-windows-msvc]";

/// Radice del repository (la cartella sopra `xtask`), con le maiuscole che usa cargo.
pub fn repo_root() -> PathBuf {
    let xtask = Path::new(env!("CARGO_MANIFEST_DIR"));
    xtask.parent().unwrap_or(xtask).to_path_buf()
}

/// Le stringhe di `rustflags = [...]` nella sezione del target Windows di `.cargo/config.toml`.
pub fn config_rustflags(text: &str) -> Result<Vec<String>, String> {
    let mut in_section = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_section = l == TARGET_SECTION;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some(rest) = l.strip_prefix("rustflags") else { continue };
        let rest = rest.trim_start().strip_prefix('=').ok_or("rustflags senza `=`")?.trim();
        let inner = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')).ok_or("rustflags su più righe o non è un elenco")?;
        let mut out = Vec::new();
        for part in inner.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let s = part.strip_prefix('"').and_then(|p| p.strip_suffix('"')).ok_or_else(|| format!("valore non tra virgolette: {part}"))?;
            out.push(s.to_owned());
        }
        return Ok(out);
    }
    Err(format!("nessun rustflags in {TARGET_SECTION}"))
}

/// Coppie (cartella, nome) da rimappare, dalla più corta alla più lunga: rustc applica l'ultima che
/// corrisponde, quindi una cartella dentro un'altra vince.
pub fn remaps() -> Result<Vec<(String, &'static str)>, String> {
    let root = repo_root();
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let cargo_home = std::env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|h| h.join(".cargo"))).ok_or("né CARGO_HOME né USERPROFILE")?;
    let mut out = vec![(cargo_home, "cargo"), (root.clone(), "quado")];
    let rustup = std::env::var_os("RUSTUP_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|h| h.join(".rustup")));
    if let Some(r) = rustup.filter(|r| r.is_dir()) {
        out.push((r, "rustup"));
    }
    if let Some(t) = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from) {
        let t = if t.is_absolute() { t } else { root.join(t) };
        if !t.starts_with(&root) {
            out.push((t, "target"));
        }
    }
    let mut out: Vec<(String, &'static str)> = out.into_iter().map(|(p, n)| (p.to_string_lossy().trim_end_matches(['\\', '/']).to_owned(), n)).collect();
    out.sort_by_key(|(p, _)| p.len());
    out.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
    Ok(out)
}

/// Flag completi per rustc: quelli di `.cargo/config.toml`, poi i `--remap-path-prefix`.
pub fn rustflags() -> Result<Vec<String>, String> {
    let config = repo_root().join(".cargo").join("config.toml");
    let text = std::fs::read_to_string(&config).map_err(|e| format!("{}: {e}", config.display()))?;
    let mut flags = config_rustflags(&text)?;
    for (from, to) in remaps()? {
        flags.push(format!("--remap-path-prefix={from}={to}"));
    }
    Ok(flags)
}

/// La stringa per RUSTFLAGS: i flag separati da spazi. Un flag con uno spazio non si può scrivere così
/// (cargo divide RUSTFLAGS sugli spazi): in quel caso serve CARGO_ENCODED_RUSTFLAGS, come fa `installa`.
pub fn rustflags_string(flags: &[String]) -> Result<String, String> {
    if let Some(f) = flags.iter().find(|f| f.contains(char::is_whitespace)) {
        return Err(format!("flag con uno spazio, non rappresentabile in RUSTFLAGS: {f}\nusa CARGO_ENCODED_RUSTFLAGS (flag separati da \\x1f)"));
    }
    Ok(flags.join(" "))
}

pub fn run(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("uso: cargo xtask rustflags-dist".into());
    }
    println!("{}", rustflags_string(&rustflags()?)?);
    Ok(())
}

/// Percorsi assoluti di un profilo utente nell'eseguibile (`X:\Users\...`, in ASCII e in UTF-16): dopo il
/// remap non ne deve restare nessuno. Restituisce i testi trovati, al più 20, fino al primo carattere non
/// stampabile.
pub fn embedded_user_paths(bytes: &[u8]) -> Vec<String> {
    let mut found = Vec::new();
    let ascii = br":\Users\";
    for i in 0..bytes.len().saturating_sub(ascii.len()) {
        if bytes[i..].starts_with(ascii) && i > 0 && bytes[i - 1].is_ascii_alphabetic() {
            let end = bytes[i..].iter().position(|b| !(0x20..0x7f).contains(b)).map_or(bytes.len(), |k| i + k);
            found.push(String::from_utf8_lossy(&bytes[i - 1..end.min(i + 120)]).into_owned());
        }
    }
    let wide: Vec<u8> = ":\\Users\\".encode_utf16().flat_map(u16::to_le_bytes).collect();
    for i in 0..bytes.len().saturating_sub(wide.len()) {
        if bytes[i..].starts_with(&wide) && i >= 2 && bytes[i - 2].is_ascii_alphabetic() && bytes[i - 1] == 0 {
            let units: Vec<u16> = bytes[i - 2..].as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).take_while(|&u| (0x20..0x7f).contains(&u)).take(60).collect();
            found.push(String::from_utf16_lossy(&units));
        }
    }
    found.truncate(20);
    found
}

/// Opzioni di misura e di prova di `quado` (feature `misure`): nessuna deve stare nell'eseguibile distribuito.
pub const DEV_OPTIONS: [&str; 11] = [
    "--misura-avvio",
    "--misura-schermate",
    "--misura-riposo",
    "--misura-wgpu",
    "--sessione-prova",
    "--banco",
    "--immagini",
    "--diagnostica-moduli",
    "--wgpu-limiti",
    "--senza-elenco-anticipato",
    "target/misure",
];

/// Le opzioni di misura e di prova presenti nell'eseguibile (testo ASCII, come le scrive Rust).
pub fn embedded_dev_options(bytes: &[u8]) -> Vec<&'static str> {
    DEV_OPTIONS.iter().copied().filter(|o| bytes.windows(o.len()).any(|w| w == o.as_bytes())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_options_are_found_in_the_bytes() {
        let bytes = b"\x00\x01argomento sconosciuto--misura-avvio\x00target/misure\x00";
        assert_eq!(embedded_dev_options(bytes), vec!["--misura-avvio", "target/misure"]);
        assert!(embedded_dev_options(b"--renderer auto --helper").is_empty());
    }

    #[test]
    fn the_config_flags_are_read_from_the_windows_section() {
        let text = std::fs::read_to_string(repo_root().join(".cargo").join("config.toml")).expect("config.toml");
        let flags = config_rustflags(&text).expect("rustflags");
        assert!(flags.windows(2).any(|w| w == ["-C", "target-feature=+crt-static"]), "{flags:?}");
        let other = "[alias]\nx = \"y\"\n\n[target.altro]\nrustflags = [\"-A\"]\n\n[target.x86_64-pc-windows-msvc]\nrustflags = [\"-C\", \"a=b\"]\n";
        assert_eq!(config_rustflags(other).unwrap(), ["-C", "a=b"]);
        assert!(config_rustflags("[target.x86_64-pc-windows-msvc]\n").is_err());
        assert!(config_rustflags("[target.x86_64-pc-windows-msvc]\nrustflags = [-C]\n").is_err());
    }

    #[test]
    fn the_flags_keep_the_config_and_remap_cargo_and_the_repository() {
        let flags = rustflags().expect("flag");
        assert!(flags.iter().any(|f| f == "target-feature=+crt-static"));
        let root = repo_root().to_string_lossy().into_owned();
        assert!(flags.contains(&format!("--remap-path-prefix={root}=quado")), "{flags:?}");
        assert!(flags.iter().any(|f| f.starts_with("--remap-path-prefix=") && f.ends_with("=cargo")));
        // La più lunga per ultima: vince su quelle che la contengono.
        let lens: Vec<usize> = remaps().unwrap().iter().map(|(p, _)| p.len()).collect();
        assert!(lens.windows(2).all(|w| w[0] <= w[1]), "{lens:?}");
        assert!(rustflags_string(&["-C".into(), "a b".into()]).is_err());
        assert_eq!(rustflags_string(&["-C".into(), "x".into()]).unwrap(), "-C x");
    }

    #[test]
    fn user_paths_are_found_in_ascii_and_utf16() {
        let mut exe = b"MZ...C:\\Users\\demo\\.cargo\\registry\\src\\a.rs\0...".to_vec();
        exe.extend("D:\\Users\\demo\\x".encode_utf16().flat_map(u16::to_le_bytes));
        exe.extend(b"\0\0cargo\\registry\\src\\b.rs\0Users\\demo senza unita\0");
        let found = embedded_user_paths(&exe);
        assert_eq!(found, [r"C:\Users\demo\.cargo\registry\src\a.rs", r"D:\Users\demo\x"]);
        assert!(embedded_user_paths(b"quado\\target\\dist\\build\\out.rs").is_empty());
    }
}
