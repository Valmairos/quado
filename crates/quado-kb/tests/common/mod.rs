// SPDX-License-Identifier: GPL-3.0-or-later
//! Fixture dei test delle regole: alberi sintetici con la forma di un PC Windows (`fixture/*.albero`), registro
//! (`fixture/registro.reg`) e file (`fixture/file/...`). Utente `demo`, progetti, giochi e identificativi inventati.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use quado_kb::evidence::{Evidence, RecycleQuery};
use quado_kb::extract::{self, ExtractError, FileSource, FormatIssue, Power};
use quado_kb::reg::{RegFile, RegistrySource, Root};
use quado_model::{Flags, NodeData, Tree};

pub const SID: &str = "S-1-5-21-1000-1000-1000-1001";

pub fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture")
}

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Albero da una fixture `.albero`. Più fixture si possono unire nello stesso albero.
pub fn tree(names: &[&str]) -> Tree {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let mut index: HashMap<String, u32> = HashMap::new();
    index.insert("c:\\".into(), Tree::ROOT);
    for name in names {
        let text = std::fs::read_to_string(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let (kind, rest) = line.split_once(' ').expect("riga");
            let (data, path) = match kind {
                "d" => (NodeData { flags: Flags::DIR, ..Default::default() }, rest),
                "n" => (NodeData { flags: Flags::DIR | Flags::DENIED, ..Default::default() }, rest),
                "f" => {
                    let mut it = rest.splitn(3, ' ');
                    let alloc: u64 = it.next().and_then(|x| x.parse().ok()).expect("allocata");
                    let logical: u64 = it.next().and_then(|x| x.parse().ok()).expect("logica");
                    (NodeData { alloc, logical, ..Default::default() }, it.next().expect("percorso"))
                }
                other => panic!("riga sconosciuta {other}"),
            };
            if path == "C:\\" {
                continue;
            }
            let key = path.to_lowercase();
            if index.contains_key(&key) {
                continue;
            }
            let (parent, file) = path.rsplit_once('\\').expect("genitore");
            let parent_key = if parent.len() == 2 { format!("{}\\", parent.to_lowercase()) } else { parent.to_lowercase() };
            let p = *index.get(&parent_key).unwrap_or_else(|| panic!("genitore mancante per {path}"));
            let n = t.push(p, &w(file), data);
            index.insert(key, n);
        }
    }
    t.aggregate();
    t.seal();
    t
}

/// File di `fixture/file/<unità>/<percorso>`, senza distinzione di maiuscole.
pub struct FakeFiles {
    files: HashMap<String, String>,
}

impl FakeFiles {
    pub fn load() -> Self {
        let base = dir().join("file");
        let mut files = HashMap::new();
        let mut stack = vec![base.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).expect("cartella").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    let rel = p.strip_prefix(&base).expect("relativo").to_string_lossy().replace('/', "\\");
                    let (drive, rest) = rel.split_once('\\').expect("unità");
                    let win = format!("{}:\\{}", drive, rest).to_lowercase();
                    files.insert(win, std::fs::read_to_string(&p).expect("testo"));
                }
            }
        }
        Self { files }
    }

    pub fn with(mut self, path: &str, text: &str) -> Self {
        self.files.insert(path.to_lowercase(), text.to_owned());
        self
    }
}

impl FileSource for FakeFiles {
    fn read(&self, path: &str) -> Result<String, ExtractError> {
        let t = self.files.get(&extract::normalize(path).to_lowercase()).ok_or(ExtractError::NotPresent)?;
        if t.len() as u64 > extract::MAX_FILE {
            return Err(FormatIssue::TooLarge.into());
        }
        Ok(t.clone())
    }

    fn list(&self, dir: &str) -> Result<Vec<String>, ExtractError> {
        let prefix = format!("{}\\", extract::normalize(dir).to_lowercase());
        let names: Vec<String> = self.files.keys().filter_map(|k| k.strip_prefix(&prefix)).filter(|r| !r.contains('\\')).map(str::to_owned).collect();
        let dirs = self.files.keys().any(|k| k.starts_with(&prefix));
        if names.is_empty() && !dirs { Err(ExtractError::NotPresent) } else { Ok(names) }
    }
}

pub fn registry() -> RegFile {
    RegFile::parse(&std::fs::read_to_string(dir().join("registro.reg")).expect("registro")).expect("formato del registro")
}

/// Prove del PC delle fixture: registro e file con i lettori veri; alimentazione e Cestino con i
/// valori letti dalle API il 13/10/2026 (`tests/live.rs`).
pub fn evidence_with(reg: &dyn RegistrySource, files: &dyn FileSource) -> Evidence {
    let mut ev = Evidence::empty("C:\\");
    ev.folders = Evidence::default_folders("C:\\", "demo");
    ev.system_volume = true;
    ev.user_sid = Some(SID.into());
    let local = ev.folders.local.clone().expect("local");
    ev.uninstall = extract::uninstall(reg);
    ev.steam = extract::steam(reg, files, "C:\\");
    ev.epic = extract::epic(files, r"C:\ProgramData");
    ev.capcut = extract::capcut(files, &local);
    ev.capcut_drafts = extract::capcut_drafts(files, &local);
    ev.track_progs = extract::track_progs(reg);
    ev.userassist = extract::userassist(reg);
    ev.storage_sense = extract::storage_sense(reg);
    let volumes = reg.subkeys(Root::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume").unwrap_or_default();
    ev.bin = volumes.first().map_or(Err(ExtractError::NotPresent), |g| extract::bin_settings(reg, g));
    let fast = matches!(reg.value(Root::LocalMachine, r"SYSTEM\CurrentControlSet\Control\Session Manager\Power", "HiberbootEnabled"), Ok(Some(quado_kb::reg::RegValue::Dword(1))));
    ev.power = Ok(Power { hibernate: true, hiberfile_present: true, fast_startup: Some(fast), has_battery: true, modern_standby: false, critical_battery_action: Some(2) });
    ev.recycle = Ok(RecycleQuery { bytes: 1_318_890, items: 1 });
    ev.msix = extract::parse_msix_list(&std::fs::read_to_string(dir().join("msix.txt")).expect("msix")).map_err(ExtractError::Format);
    ev.restore = Err(ExtractError::Io(5));
    ev
}

pub fn evidence() -> Evidence {
    evidence_with(&registry(), &FakeFiles::load())
}
