// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask vhdx <crea|freddo|elimina|stato>`.
//!
//! I VHDX di prova vivono in `C:\RilievoProve`: un volume NTFS, uno exFAT e uno FAT32, espandibili.
//! Creare, montare, smontare ed eliminare richiede l'amministratore: il comando scrive uno script
//! PowerShell e lo esegue elevato con la richiesta UAC (chi usa il PC la approva a ogni
//! sessione). Lo script riparte dallo stato in cui trova i dischi e scrive tutto il suo output in
//! `C:\RilievoProve\log-<comando>.txt`. Le prove sui volumi montati girano poi da utente normale.

use std::path::{Path, PathBuf};
use std::process::Command;

pub const DIR: &str = r"C:\RilievoProve";

pub struct Spec {
    pub name: &'static str,
    pub fs: &'static str,
    pub label: &'static str,
    pub mb: u32,
}

pub const SPECS: [Spec; 3] = [
    Spec { name: "ntfs", fs: "NTFS", label: "RilievoNTFS", mb: 4096 },
    Spec { name: "exfat", fs: "exFAT", label: "RilievoEXFAT", mb: 2048 },
    Spec { name: "fat32", fs: "FAT32", label: "RILIEVOFAT", mb: 2048 },
];

pub fn vhdx_path(name: &str) -> PathBuf {
    Path::new(DIR).join(format!("{name}.vhdx"))
}

/// Lettere assegnate, da `stato.tsv`: (nome, lettera).
pub fn state() -> Vec<(String, char)> {
    std::fs::read_to_string(Path::new(DIR).join("stato.tsv"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (n, c) = l.split_once('\t')?;
            Some((n.to_owned(), c.trim().chars().next()?))
        })
        .collect()
}

/// Esegue `script` in un PowerShell elevato (richiesta UAC) e restituisce il log.
fn run_elevated(cmd: &str, script: &str) -> Result<String, String> {
    std::fs::create_dir_all(DIR).map_err(|e| e.to_string())?;
    let ps1 = Path::new(DIR).join(format!("{cmd}.ps1"));
    let log = Path::new(DIR).join(format!("log-{cmd}.txt"));
    let _ = std::fs::remove_file(&log);
    // BOM UTF-8: PowerShell 5.1 legge così anche i caratteri non ASCII.
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(script.as_bytes());
    std::fs::write(&ps1, bytes).map_err(|e| e.to_string())?;
    let launcher = format!(
        "$p = Start-Process -FilePath powershell -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','\"{}\"' -Verb RunAs -Wait -PassThru -WindowStyle Hidden; exit $p.ExitCode",
        ps1.display()
    );
    let st = Command::new("powershell").args(["-NoProfile", "-Command", &launcher]).status().map_err(|e| e.to_string())?;
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    if st.success() { Ok(text) } else { Err(format!("{text}\nscript elevato terminato con {st} (richiesta UAC rifiutata o errore)")) }
}

/// Intestazione comune: ogni comando e il suo output finiscono nel log.
fn header(cmd: &str) -> String {
    format!(
        "$ErrorActionPreference = 'Stop'\n$log = '{DIR}\\log-{cmd}.txt'\nStart-Transcript -Path $log -Force | Out-Null\ntry {{\n"
    )
}

const FOOTER: &str = "} catch { Write-Output (\"ERRORE: \" + $_) ; Stop-Transcript | Out-Null; exit 1 }\nStop-Transcript | Out-Null\nexit 0\n";

fn script_create() -> String {
    let user = format!("{}\\{}", std::env::var("USERDOMAIN").unwrap_or_default(), std::env::var("USERNAME").unwrap_or_default());
    let mut s = header("crea");
    s.push_str("$stato = @()\n");
    for spec in &SPECS {
        let f = vhdx_path(spec.name);
        s.push_str(&format!(
            r#"
$f = '{f}'
if (-not (Test-Path $f)) {{
  $dp = '{DIR}\diskpart.txt'
  Set-Content -Path $dp -Encoding ASCII -Value @("create vdisk file=""$f"" maximum={mb} type=expandable")
  diskpart /s $dp | Write-Output
}}
# Get-DiskImage e Mount-DiskImage aprono il VHDX prima in sola lettura: con un registro da riapplicare
# (disco montato durante un arresto improvviso) Windows risponde accesso negato, anche all'amministratore.
# diskpart lo apre in scrittura, e Windows riapplica il registro.
try {{ $img = Get-DiskImage -ImagePath $f }} catch {{
  Write-Output "Get-DiskImage: $($_.Exception.Message) Lo monto con diskpart."
  $dp = '{DIR}\diskpart.txt'
  Set-Content -Path $dp -Encoding ASCII -Value @("select vdisk file=""$f""", "attach vdisk")
  diskpart /s $dp | Write-Output
  Start-Sleep -Seconds 2
  $img = Get-DiskImage -ImagePath $f
}}
if (-not $img.Attached) {{ $img = Mount-DiskImage -ImagePath $f -PassThru }}
$disk = $img | Get-Disk
if ($disk.PartitionStyle -eq 'RAW') {{ Initialize-Disk -Number $disk.Number -PartitionStyle GPT }}
$part = Get-Partition -DiskNumber $disk.Number | Where-Object Type -eq 'Basic' | Select-Object -First 1
if (-not $part) {{ $part = New-Partition -DiskNumber $disk.Number -UseMaximumSize }}
$vol = $part | Get-Volume
if ($vol.FileSystem -ne '{fs}') {{ Format-Volume -Partition $part -FileSystem {fs} -NewFileSystemLabel '{label}' -Confirm:$false -Force | Out-Null }}
$part = Get-Partition -DiskNumber $disk.Number -PartitionNumber $part.PartitionNumber
if (-not $part.DriveLetter -or [int][char]$part.DriveLetter -eq 0) {{
  $part | Add-PartitionAccessPath -AssignDriveLetter
  Start-Sleep -Seconds 2
  $part = Get-Partition -DiskNumber $disk.Number -PartitionNumber $part.PartitionNumber
}}
$letter = $part.DriveLetter
Write-Output ("{name}: disco $($disk.Number), lettera $letter, file system $((Get-Volume -DriveLetter $letter).FileSystem)")
$stato += "{name}`t$letter"
"#,
            f = f.display(),
            mb = spec.mb,
            fs = spec.fs,
            label = spec.label,
            name = spec.name,
        ));
        if spec.fs == "NTFS" {
            // Le prove girano da utente normale: la radice del volume NTFS deve essere sua.
            s.push_str(&format!("icacls \"$($letter):\\\" /grant \"{user}:(OI)(CI)F\" | Write-Output\n"));
        }
    }
    s.push_str(&format!("Set-Content -Path '{DIR}\\stato.tsv' -Encoding ASCII -Value $stato\n"));
    // Candidati per lo stato e la dimensione dell'archiviazione riservata.
    s.push_str(
        r#"
Write-Output '=== DISM /Online /Get-ReservedStorageState'
$ErrorActionPreference = 'Continue'
DISM /Online /Get-ReservedStorageState 2>&1 | Write-Output
Write-Output '=== fsutil storagereserve query C:'
fsutil storagereserve query C: 2>&1 | Write-Output
$ErrorActionPreference = 'Stop'
"#,
    );
    s.push_str(FOOTER);
    s
}

fn script_cold(runs: usize) -> String {
    let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default();
    let mut s = header("freddo");
    for (name, letter) in state() {
        let f = vhdx_path(&name);
        s.push_str(&format!(
            r#"
for ($i = 1; $i -le {runs}; $i++) {{
  Dismount-DiskImage -ImagePath '{f}' | Out-Null
  Mount-DiskImage -ImagePath '{f}' | Out-Null
  for ($k = 0; $k -lt 50 -and -not (Test-Path '{letter}:\'); $k++) {{ Start-Sleep -Milliseconds 200 }}
  $r = & '{exe}' scan-riga '{letter}:\'
  Write-Output ("FREDDO`t{name}`t$i`t$r")
}}
"#,
            f = f.display()
        ));
    }
    s.push_str(FOOTER);
    s
}

fn script_delete() -> String {
    let mut s = header("elimina");
    for spec in &SPECS {
        let f = vhdx_path(spec.name);
        s.push_str(&format!(
            "if (Test-Path '{f}') {{ $img = Get-DiskImage -ImagePath '{f}'; if ($img.Attached) {{ Dismount-DiskImage -ImagePath '{f}' | Out-Null }}; Remove-Item '{f}' -Force; Write-Output 'eliminato {f}' }}\n",
            f = f.display()
        ));
    }
    s.push_str(&format!("Remove-Item '{DIR}\\stato.tsv' -ErrorAction SilentlyContinue\n"));
    s.push_str(FOOTER);
    s
}

pub fn run(args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(String::as_str).ok_or("vhdx <crea|freddo [N]|elimina|stato>")?;
    let log = match cmd {
        "crea" => run_elevated("crea", &script_create()),
        "freddo" => run_elevated("freddo", &script_cold(args.get(1).and_then(|a| a.parse().ok()).unwrap_or(5))),
        "elimina" => run_elevated("elimina", &script_delete()),
        "stato" => {
            for (n, c) in state() {
                println!("{n}\t{c}:\\");
            }
            return Ok(());
        }
        other => return Err(format!("comando sconosciuto: {other}")),
    };
    match log {
        Ok(text) => {
            println!("{text}");
            Ok(())
        }
        Err(e) => Err(e),
    }
}
