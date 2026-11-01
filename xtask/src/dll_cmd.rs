// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask verifica-dll` (07 §3 riga 5, 06 §11, RS11).
//!
//! 1. Il load config di `quado.exe` ha `DependentLoadFlags` = 0x800 (letto dall'intestazione PE).
//! 2. Prova dell'esca: una copia di `version.dll` di System32 messa accanto all'eseguibile non viene
//!    caricata. Come controllo, una build senza il flag deve invece caricarla: così la prova sa
//!    distinguere il caso sicuro da quello vulnerabile.

use std::path::{Path, PathBuf};
use std::process::Command;

/// `DependentLoadFlags` dal load config di un PE a 64 bit.
pub fn dependent_load_flags(exe: &Path) -> Result<u16, String> {
    let b = std::fs::read(exe).map_err(|e| e.to_string())?;
    let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let pe = u32_at(0x3c) as usize;
    if &b[pe..pe + 4] != b"PE\0\0" {
        return Err("non è un PE".into());
    }
    let coff = pe + 4;
    let sections = u16_at(coff + 2) as usize;
    let opt_size = u16_at(coff + 16) as usize;
    let opt = coff + 20;
    if u16_at(opt) != 0x20b {
        return Err("non è un PE32+".into());
    }
    // Directory 10 = load config.
    let dir = opt + 112 + 10 * 8;
    let rva = u32_at(dir) as usize;
    if rva == 0 {
        return Err("nessun load config".into());
    }
    let sec = opt + opt_size;
    for i in 0..sections {
        let s = sec + i * 40;
        let va = u32_at(s + 12) as usize;
        let vsize = u32_at(s + 8) as usize;
        let raw = u32_at(s + 20) as usize;
        if rva >= va && rva < va + vsize {
            let off = raw + (rva - va);
            // IMAGE_LOAD_CONFIG_DIRECTORY64: DependentLoadFlags all'offset 78 (dopo ProcessHeapFlags e CSDVersion).
            return Ok(u16_at(off + 78));
        }
    }
    Err("load config fuori dalle sezioni".into())
}

fn bait_loaded(exe: &Path, label: &str) -> Result<bool, String> {
    let dir = std::env::temp_dir().join(format!("quado-esca-dll-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let copy = dir.join("quado.exe");
    std::fs::copy(exe, &copy).map_err(|e| e.to_string())?;
    let sys = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join(r"System32\version.dll");
    std::fs::copy(&sys, dir.join("version.dll")).map_err(|e| e.to_string())?;
    let out = Command::new(&copy).arg("--diagnostica-moduli").output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    let version_lines: Vec<&str> = text.lines().filter(|l| l.to_ascii_lowercase().ends_with("version.dll")).collect();
    if version_lines.is_empty() {
        return Err(format!("version.dll non caricata ({label}):\n{text}"));
    }
    for l in &version_lines {
        println!("  {label}: {}", l.trim_start_matches("modulo\t"));
    }
    Ok(version_lines.iter().any(|l| !l.to_ascii_lowercase().contains(r"\system32\")))
}

pub fn run() -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let build = |extra_env: Option<(&str, &str)>, target: &str| -> Result<PathBuf, String> {
        let mut c = Command::new(&cargo);
        c.args(["build", "--release", "--quiet", "-p", "quado", "--features", "misure", "--target-dir", target]);
        if let Some((k, v)) = extra_env {
            c.env(k, v);
        }
        let s = c.status().map_err(|e| e.to_string())?;
        if !s.success() {
            return Err("build fallita".into());
        }
        Ok(PathBuf::from(target).join(r"release\quado.exe"))
    };
    let safe = build(None, "target")?;
    let flags = dependent_load_flags(&safe)?;
    println!("DependentLoadFlags di quado.exe: {flags:#06x}");
    if flags != 0x800 {
        return Err("manca /DEPENDENTLOADFLAG:0x800".into());
    }
    let control = build(Some(("QUADO_SENZA_DEPENDENTLOADFLAG", "1")), r"target\controllo-dll")?;
    println!("DependentLoadFlags della build di controllo: {:#06x}", dependent_load_flags(&control)?);
    let control_vulnerable = bait_loaded(&control, "controllo")?;
    let safe_vulnerable = bait_loaded(&safe, "quado")?;
    println!("esca caricata: controllo {control_vulnerable}, quado {safe_vulnerable}");
    if !control_vulnerable {
        return Err("la build di controllo non carica l'esca: la prova non distingue i due casi".into());
    }
    if safe_vulnerable {
        return Err("quado.exe carica la DLL esca dalla sua cartella".into());
    }
    println!("verifica superata: l'esca accanto all'eseguibile non viene caricata");
    Ok(())
}
