// SPDX-License-Identifier: GPL-3.0-or-later
//! F-SC-1: l'elenco dei volumi coincide con `Get-Volume` nella stessa sessione (06 §1.3).

use quado_scan_win::volumes::{self, DriveKind};

#[test]
fn matches_get_volume() {
    let ours = volumes::list();
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-Volume | Where-Object DriveLetter | ForEach-Object { \"$($_.DriveLetter)`t$($_.FileSystemLabel)`t$($_.FileSystem)`t$($_.DriveType)`t$($_.Size)\" }",
        ])
        .output()
        .expect("powershell");
    let text = String::from_utf8_lossy(&out.stdout);
    let mut checked = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let c: Vec<&str> = line.trim_end().split('\t').collect();
        let letter = format!("{}:\\", c[0]);
        let v = ours.iter().find(|v| v.paths.iter().any(|p| p.eq_ignore_ascii_case(&letter))).unwrap_or_else(|| panic!("manca {letter}"));
        assert_eq!(v.label, c[1], "etichetta di {letter}");
        assert_eq!(v.file_system, c[2], "file system di {letter}");
        let kind = match c[3] {
            "Fixed" => DriveKind::Fixed,
            "Removable" => DriveKind::Removable,
            "CD-ROM" => DriveKind::Optical,
            "Network" => DriveKind::Network,
            _ => DriveKind::Unknown,
        };
        assert_eq!(v.kind, kind, "tipo di {letter}");
        assert_eq!(v.space.map(|s| s.capacity.to_string()), Some(c[4].to_owned()), "capacità di {letter}");
        checked += 1;
    }
    assert!(checked >= 1);
}
