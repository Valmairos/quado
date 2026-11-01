// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask prestazioni [radice] [--runs N]` e `cargo xtask riferimento [radice]` (06 §9, 05 D10).
//!
//! Ogni esecuzione è un processo a sé (`xtask scan-riga`), così il picco di memoria è quello di una
//! scansione sola. Il protocollo annota CPU, RAM, versione di Windows, stato di Defender, versione
//! dell'app e data; si riportano mediana, minimo e massimo.

use std::process::Command;

use quado_model::{Flags, Tree};
use quado_scan_win::{Options, Progress, scan, win};

use crate::scan_cmd::{gib, gib_i};

/// Una riga di misure per esecuzione, separata da tabulazioni.
pub fn scan_line(args: &[String]) -> Result<(), String> {
    let root = args.first().ok_or("radice")?;
    let mut opts = Options::default();
    if let Some(k) = args.iter().position(|a| a == "--threads") {
        opts.threads = args.get(k + 1).and_then(|v| v.parse().ok()).ok_or("--threads N")?;
    }
    let r = scan(root, opts, &Progress::default()).map_err(|e| format!("errore {}", e.0))?;
    let m = win::process_metrics();
    let total = r.stats.enumeration + r.stats.finish;
    println!(
        "{:.3}\t{:.3}\t{}\t{:.1}\t{:.1}\t{}\t{}\t{}",
        total.as_secs_f64(),
        r.stats.enumeration.as_secs_f64(),
        m.peak_working_set,
        m.user.as_secs_f64(),
        m.kernel.as_secs_f64(),
        m.handles,
        r.tree.len(),
        r.stats.files
    );
    Ok(())
}

fn powershell(cmd: &str) -> String {
    Command::new("powershell")
        .args(["-NoProfile", "-Command", cmd])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}

/// Condizioni della misura (06 §9).
pub fn environment() -> Vec<(&'static str, String)> {
    let git = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned()).unwrap_or_default();
    vec![
        ("Data", powershell("Get-Date -Format 'dd/MM/yyyy HH:mm'")),
        ("CPU", powershell("(Get-CimInstance Win32_Processor).Name")),
        ("RAM", powershell("'{0:N0} GiB' -f ((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)")),
        ("Disco di C:", powershell("Get-Partition -DriveLetter C | Get-Disk | ForEach-Object { $_.FriendlyName + ', ' + $_.BusType + ', ' + [math]::Round($_.Size / 1GB) + ' GB' }")),
        ("Windows", powershell("$v = Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion'; \"$($v.ProductName) $($v.DisplayVersion) build $($v.CurrentBuild).$($v.UBR)\"")),
        ("Defender in tempo reale", powershell("try { (Get-MpComputerStatus).RealTimeProtectionEnabled } catch { 'non leggibile' }")),
        ("Versione di Quado", format!("commit {git}")),
    ]
}

fn stats(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(f64::total_cmp);
    let med = if v.len() % 2 == 1 { v[v.len() / 2] } else { (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0 };
    (med, v[0], v[v.len() - 1])
}

pub fn perf(args: &[String]) -> Result<(), String> {
    let mut root = "C:\\".to_owned();
    let mut runs = 5usize;
    let mut threads: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--runs" => runs = it.next().and_then(|v| v.parse().ok()).ok_or("--runs N")?,
            "--threads" => threads = Some(it.next().cloned().ok_or("--threads N")?),
            r => root = r.to_owned(),
        }
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut rows: Vec<Vec<f64>> = Vec::new();
    for i in 0..runs {
        let mut c = Command::new(&exe);
        c.args(["scan-riga", &root]);
        if let Some(t) = &threads {
            c.args(["--threads", t]);
        }
        let out = c.output().map_err(|e| e.to_string())?;
        let line = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        let cells: Vec<f64> = line.split('\t').filter_map(|c| c.parse().ok()).collect();
        if cells.len() < 8 {
            return Err(format!("esecuzione {}: {line} {}", i + 1, String::from_utf8_lossy(&out.stderr)));
        }
        eprintln!("esecuzione {}: {:.2} s, picco {} MB", i + 1, cells[0], cells[2] as u64 >> 20);
        rows.push(cells);
    }
    let col = |k: usize| -> Vec<f64> { rows.iter().map(|r| r[k]).collect() };
    let (t_med, t_min, t_max) = stats(&mut col(0));
    let (m_med, m_min, m_max) = stats(&mut col(2));
    let (u_med, _, _) = stats(&mut col(3));
    let (k_med, _, _) = stats(&mut col(4));
    let (h_med, _, h_max) = stats(&mut col(5));
    let th = threads.as_deref().map_or_else(String::new, |t| format!(", {t} thread"));
    println!("## Prestazioni della scansione di {root} ({runs} esecuzioni, cache calda{th})\n");
    println!("| Condizione | Valore |\n|---|---|");
    for (k, v) in environment() {
        println!("| {k} | {v} |");
    }
    println!("\n| Misura | Mediana | Minimo | Massimo | Obiettivo |\n|---|---|---|---|---|");
    println!("| Tempo totale | {t_med:.1} s | {t_min:.1} s | {t_max:.1} s | ≤ 22 s |");
    println!("| Picco di working set | {} MB | {} MB | {} MB | ≤ 400 MB |", m_med as u64 >> 20, m_min as u64 >> 20, m_max as u64 >> 20);
    println!("| CPU utente / kernel | {u_med:.1} s / {k_med:.1} s | | | |");
    println!("| Handle aperti a fine scansione | {h_med:.0} | | {h_max:.0} | |");
    println!("| Nodi | {} | | | |", rows[0][6] as u64);
    Ok(())
}

pub fn reference(args: &[String]) -> Result<(), String> {
    let root = args.first().cloned().unwrap_or_else(|| "C:\\".to_owned());
    let r = scan(&root, Options::default(), &Progress::default()).map_err(|e| format!("errore {}", e.0))?;
    let t = &r.tree;
    let b = r.balance.ok_or("il bilancio richiede la radice di un volume")?;
    let row = |k: &str, bytes: u64, note: &str| println!("| {k} | {bytes} | {} | {note} |", gib(bytes));
    let date = environment().into_iter().next().map(|(_, v)| v).unwrap_or_default();
    println!("## Misure di Quado su {root} ({date}, `cargo xtask riferimento`)\n");
    println!("| Grandezza | Byte | GiB | Note |\n|---|---|---|---|");
    row("Capacità", b.capacity, "GetDiskFreeSpaceExW");
    row("Usato a inizio scansione (U0)", b.used_start, "");
    row("Usato a fine scansione (U)", b.used, "");
    println!("| Deriva (U - U0) | {} | {} | incertezza del non attribuito |", b.drift, gib_i(b.drift));
    row("Allocato unico letto (A)", b.allocated, "ogni file con più nomi contato una volta");
    row("`$MFT`", b.metadata.mft.unwrap_or(0), "FSCTL_GET_NTFS_VOLUME_DATA, senza privilegi");
    row("Indici delle cartelle", b.metadata.dir_indexes, "FileStandardInfo sugli handle delle cartelle");
    row("`$UsnJrnl` (intervallo attivo)", b.metadata.usn_journal.unwrap_or(0), "FSCTL_QUERY_USN_JOURNAL, senza privilegi");
    println!("| Non attribuito (N = U - A - M) | {} | {} | |", b.unattributed, gib_i(b.unattributed));
    row("Hard link: byte contati una volta", b.hardlinks_counted_once, &format!("{} gruppi, {} nomi in più", r.links.groups.len(), r.links.extra_names));
    row("Logico totale", t.logical(Tree::ROOT), "");
    println!("| File | {} | | |", r.stats.files);
    println!("| Cartelle | {} | | |", r.stats.dirs);
    println!("| Cartelle negate | {} | | elencate con il motivo (F-SC-6) |", r.stats.denied);
    println!("| Junction, symlink e montaggi non seguiti | {} | | |", r.stats.not_followed);
    println!("| File con allocata corretta (WOF, sparse) | {} | | |", r.stats.alloc_fixed);
    println!("| Segnaposto cloud | {} | | {} cartelle non elencate |", r.stats.cloud_files, r.stats.cloud_dirs_not_listed);
    let (units, wide) = t.name_units();
    println!("| Unità di nome | {units} | | di cui {wide} in UTF-16 |");
    let denied_paths: Vec<String> = r
        .problems
        .iter()
        .filter(|(n, _)| t.flags(*n).has(Flags::DENIED))
        .take(5)
        .map(|(n, _)| format!("`{}`", t.path_lossy(*n)))
        .collect();
    println!("\nPrime cartelle negate: {}.", denied_paths.join(", "));
    Ok(())
}

/// `cargo xtask freddo-c`: prima scansione di C: dopo un riavvio (07 §3 riga 8, 06 §9).
/// Da lanciare subito dopo l'accesso; aggiunge una riga a `target/misure/freddo-C.tsv`.
pub fn cold_c() -> Result<(), String> {
    let uptime_min: f64 = powershell("[math]::Round(((Get-Date) - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime).TotalMinutes, 1)")
        .replace(',', ".")
        .parse()
        .unwrap_or(-1.0);
    // Prima il primo frame interattivo di Quado (06 §9, criterio 5 di M2): l'eseguibile si apre,
    // scrive il tempo dalla creazione del processo e si chiude da solo. Poi la scansione a freddo.
    let app = std::path::Path::new("target/release/quado.exe");
    if app.exists() {
        let _ = std::fs::create_dir_all("target/misure");
        match Command::new(app).args(["--misura-avvio", "target/misure/avvio.tsv"]).output() {
            Ok(o) => {
                let s = String::from_utf8_lossy(&o.stdout);
                let ms = s.lines().find_map(|l| l.strip_prefix("primo-frame-ms\t")).unwrap_or("?").trim().to_owned();
                println!("primo frame interattivo di Quado: {ms} ms");
            }
            Err(e) => println!("misura dell'avvio non riuscita: {e}"),
        }
    } else {
        println!("target/release/quado.exe non c'è: avvio non misurato (cargo build --release -p quado --features misure)");
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let out = Command::new(&exe).args(["scan-riga", "C:\\"]).output().map_err(|e| e.to_string())?;
    let line = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let cells: Vec<&str> = line.split('\t').collect();
    if cells.len() < 8 {
        return Err(format!("scansione non riuscita: {line}"));
    }
    let date = powershell("Get-Date -Format 'dd/MM/yyyy HH:mm'");
    let _ = std::fs::create_dir_all("target/misure");
    let path = std::path::Path::new("target/misure/freddo-C.tsv");
    let mut text = std::fs::read_to_string(path).unwrap_or_else(|_| "data\tminuti_dall_avvio\ttempo_s\tenumerazione_s\tpicco_byte\tcpu_utente_s\tcpu_kernel_s\thandle\tnodi\tfile\n".to_owned());
    text.push_str(&format!("{date}\t{uptime_min}\t{line}\n"));
    std::fs::write(path, text).map_err(|e| e.to_string())?;
    println!("prima scansione dopo il riavvio ({uptime_min} minuti dall'avvio): {} s, picco {} MB", cells[0], cells[2].parse::<u64>().unwrap_or(0) >> 20);
    if uptime_min > 15.0 {
        println!("nota: il PC è acceso da più di 15 minuti, la cache potrebbe non essere più fredda");
    }
    Ok(())
}
