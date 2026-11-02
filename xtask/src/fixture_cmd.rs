// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask fixture-regole [--progetti "a;b"] [--uscita DIR]`: bozza delle fixture delle regole
//! letta da questo PC.
//!
//! Legge in sola lettura: una scansione di C:, le chiavi di registro e i file nominati dalle regole.
//! Il profilo diventa `C:\Users\demo`, il SID uno finto. Le cartelle oltre la profondità scelta si
//! riducono a una voce `(contenuto)` con la loro allocata, così la bozza resta piccola e la forma
//! dell'albero è quella vera. `--progetti` nomina le cartelle di Documenti da leggere (progetti con
//! dipendenze e lockfile); senza, nessuna.
//!
//! Scrive in `target/fixture-regole` (o `--uscita`), mai in `crates/quado-kb/fixture`: la bozza contiene
//! ancora i nomi veri di programmi, giochi, progetti e file. Le fixture del repository sono sintetiche:
//! dalla bozza si copia solo la forma che serve a una regola, con nomi inventati.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use quado_kb::extract::{self, FileSource};
use quado_kb::reg::{RegValue, RegistrySource, Root};
use quado_kb::win::{LiveFiles, LiveRegistry};
use quado_model::{Flags, Tree};
use quado_scan_win::{Options, Progress, scan, system};

pub const FAKE_SID: &str = "S-1-5-21-1000-1000-1000-1001";

struct Anon {
    profile: String,
    user: String,
    sid: Option<String>,
}

impl Anon {
    fn apply(&self, s: &str) -> String {
        let mut out = replace_ci(s, &self.profile, r"C:\Users\demo");
        out = replace_ci(&out, &self.profile.replace('\\', "\\\\"), r"C:\\Users\\demo");
        out = replace_ci(&out, &self.profile.replace('\\', "/"), "C:/Users/demo");
        if let Some(sid) = &self.sid {
            out = out.replace(sid.as_str(), FAKE_SID);
        }
        // Il nome da solo (per esempio in un percorso con barre diverse).
        replace_ci(&out, &format!(r"\{}\", self.user), r"\demo\")
    }
}

fn replace_ci(s: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        return s.to_owned();
    }
    let lower = s.to_lowercase();
    let f = from.to_lowercase();
    let mut out = String::new();
    let mut i = 0;
    while let Some(k) = lower[i..].find(&f) {
        out.push_str(&s[i..i + k]);
        out.push_str(to);
        i += k + f.len();
    }
    out.push_str(&s[i..]);
    out
}

fn find(tree: &Tree, path: &str) -> Option<u32> {
    quado_kb::engine::find(tree, path)
}

/// Righe della fixture di un albero: `d <percorso>`, `n <percorso>` (cartella negata),
/// `f <allocata> <logica> <percorso>`.
fn dump(tree: &Tree, specs: &[(String, usize)], anon: &Anon) -> String {
    let mut lines: BTreeMap<String, String> = BTreeMap::new();
    let add_dir = |lines: &mut BTreeMap<String, String>, n: u32| {
        let p = anon.apply(&tree.path_lossy(n));
        let kind = if tree.flags(n).has(Flags::DENIED) { "n" } else { "d" };
        lines.entry(p.to_lowercase()).or_insert_with(|| format!("{kind} {p}"));
    };
    for (path, depth) in specs {
        let Some(root) = find(tree, path) else {
            eprintln!("non trovato: {path}");
            continue;
        };
        // Antenati.
        let mut a = tree.parent(root);
        while a != quado_model::NONE {
            add_dir(&mut lines, a);
            a = tree.parent(a);
        }
        let mut stack = vec![(root, 0usize)];
        while let Some((n, d)) = stack.pop() {
            if tree.flags(n).has(Flags::DETACHED) {
                continue;
            }
            if tree.is_dir(n) {
                add_dir(&mut lines, n);
                if d >= *depth {
                    if tree.alloc(n) > 0 {
                        let p = anon.apply(&format!(r"{}\(contenuto)", tree.path_lossy(n)));
                        lines.insert(p.to_lowercase(), format!("f {} {} {p}", tree.alloc(n), tree.logical(n)));
                    }
                } else {
                    for c in tree.children(n) {
                        stack.push((c, d + 1));
                    }
                }
            } else {
                let p = anon.apply(&tree.path_lossy(n));
                lines.insert(p.to_lowercase(), format!("f {} {} {p}", tree.alloc(n), tree.logical(n)));
            }
        }
    }
    let mut out = String::from("# Bozza letta da questo PC (cargo xtask fixture-regole): nomi veri, da ridurre e rinominare.\n");
    for l in lines.values() {
        out.push_str(l);
        out.push('\n');
    }
    out
}

fn reg_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(",")
}

fn utf16_hex(s: &str) -> Vec<u8> {
    s.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
}

fn reg_value(v: &RegValue, anon: &Anon) -> Option<String> {
    Some(match v {
        RegValue::Sz(s) => reg_string(&anon.apply(s)),
        RegValue::ExpandSz(s) => format!("hex(2):{}", hex(&utf16_hex(&anon.apply(s)))),
        RegValue::MultiSz(list) => {
            let mut b = Vec::new();
            for s in list {
                b.extend(utf16_hex(&anon.apply(s)));
            }
            b.extend([0, 0]);
            format!("hex(7):{}", hex(&b))
        }
        RegValue::Dword(d) => format!("dword:{d:08x}"),
        RegValue::Qword(q) => format!("hex(b):{}", hex(&q.to_le_bytes())),
        RegValue::Binary(b) => format!("hex:{}", hex(b)),
        RegValue::Other(_) => return None,
    })
}

struct RegOut {
    text: String,
}

impl RegOut {
    fn key(&mut self, reg: &LiveRegistry, root: Root, path: &str, only: Option<&[&str]>, anon: &Anon) {
        let Ok(values) = reg.values(root, path) else {
            eprintln!("chiave non leggibile: {path}");
            return;
        };
        let prefix = match root {
            Root::LocalMachine => "HKEY_LOCAL_MACHINE",
            Root::CurrentUser => "HKEY_CURRENT_USER",
        };
        let _ = writeln!(self.text, "\r\n[{prefix}\\{}]\r", anon.apply(path));
        for (n, v) in values {
            if only.is_some_and(|o| !o.iter().any(|x| x.eq_ignore_ascii_case(&n))) {
                continue;
            }
            if let Some(s) = reg_value(&v, anon) {
                let name = if n.is_empty() { "@".to_owned() } else { reg_string(&n) };
                let _ = writeln!(self.text, "{name}={s}\r");
            }
        }
    }
}

fn write(dir: &Path, rel: &str, text: &str) -> Result<(), String> {
    let p = dir.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
    println!("scritto {}", p.display());
    Ok(())
}

/// Percorso di un file di fixture: `file/C/Program Files (x86)/...`.
fn file_rel(path: &str) -> String {
    let p = path.replace(':', "").replace('\\', "/");
    format!("file/{p}")
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut dir: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("target").join("fixture-regole");
    let mut projects: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--progetti" => projects = it.next().ok_or("--progetti senza elenco")?.split(';').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect(),
            "--uscita" => dir = PathBuf::from(it.next().ok_or("--uscita senza cartella")?),
            other => return Err(format!("argomento sconosciuto: {other}")),
        }
    }
    let sid = system::user_sid();
    let folders = quado_kb::win::known_folders();
    let profile = folders.profile.clone().ok_or("profilo non trovato")?;
    let user = profile.rsplit('\\').next().unwrap_or_default().to_owned();
    let anon = Anon { profile: profile.clone(), user, sid: sid.clone() };
    let local = folders.local.clone().ok_or("Local non trovata")?;
    let docs = folders.documents.clone().ok_or("Documenti non trovati")?;

    println!("scansione di C: (sola lettura)...");
    let r = scan("C:\\", Options::default(), &Progress::default()).map_err(|e| format!("scansione: {e:?}"))?;
    let t = &r.tree;
    let j = |a: &str, b: &str| extract::join(a, b);
    let bin = sid.as_deref().map(|s| format!(r"C:\$Recycle.Bin\{s}")).unwrap_or_default();

    let mut dev = vec![(j(&local, "npm-cache"), 1), (j(&local, "pnpm"), 2), (j(&profile, r".android\avd"), 1), (j(&profile, r".vscode\extensions"), 1)];
    dev.extend(projects.iter().map(|p| (j(&docs, p), 1)));
    let trees: Vec<(&str, Vec<(String, usize)>)> = vec![
        ("capcut.albero", vec![(j(&local, "CapCut"), 2), (j(&local, r"CapCut\User Data"), 1)]),
        (
            "radice.albero",
            vec![
                (r"C:\".into(), 1),
                (bin.clone(), 1),
                (r"C:\Windows".into(), 1),
                (r"C:\Windows\SoftwareDistribution".into(), 1),
                (r"C:\Windows\System32\DriverStore\FileRepository".into(), 0),
                (j(&local, "Temp"), 0),
                (j(&local, r"Microsoft\Windows\Explorer"), 1),
            ],
        ),
        ("sviluppo.albero", dev),
        ("giochi.albero", vec![(r"C:\Program Files (x86)\Steam\steamapps\common".into(), 1), (r"C:\Program Files\Epic Games".into(), 1)]),
        (
            "browser.albero",
            vec![
                (j(&local, r"Google\Chrome\User Data"), 2),
                (j(&local, r"BraveSoftware\Brave-Browser\User Data"), 2),
                (j(&local, r"Microsoft\Edge\User Data"), 2),
            ],
        ),
        ("profilo.albero", vec![(profile.clone(), 1), (local.clone(), 1), (j(&local, "Packages"), 1), (r"C:\Program Files".into(), 1), (r"C:\Program Files (x86)".into(), 1), (r"C:\ProgramData".into(), 1), (r"C:\Program Files (x86)\Steam".into(), 1), (r"C:\Program Files\NVIDIA Corporation\Installer2".into(), 1)]),
    ];
    for (name, specs) in &trees {
        write(&dir, name, &dump(t, specs, &anon))?;
    }

    // Registro.
    let reg = LiveRegistry;
    let mut out = RegOut { text: "Windows Registry Editor Version 5.00\r\n".into() };
    let u = extract::uninstall(&reg).map_err(|e| format!("disinstallazione: {e:?}"))?;
    let mut chosen: Vec<&extract::UninstallEntry> = Vec::new();
    let mut pick = |f: &dyn Fn(&extract::UninstallEntry) -> bool| {
        if let Some(e) = u.entries.iter().find(|e| f(e) && !chosen.iter().any(|c| c.key == e.key && c.hive == e.hive)) {
            chosen.push(e);
        }
    };
    pick(&|e| e.display_name == "CapCut");
    pick(&|e| e.display_name == "Steam");
    pick(&|e| e.display_name.starts_with("Steam App") || e.key.starts_with("Steam App"));
    pick(&|e| e.display_name.starts_with("NVIDIA"));
    pick(&|e| e.display_name.starts_with("Autodesk"));
    pick(&|e| e.display_name.starts_with("7-Zip"));
    pick(&|e| e.install_location.is_none() && e.uninstall_string.as_deref().is_some_and(|s| s.to_ascii_lowercase().starts_with("msiexec")));
    pick(&|e| e.install_location.as_deref().is_some_and(|l| l.eq_ignore_ascii_case(r"C:\") || l.eq_ignore_ascii_case(r"C:\Program Files")));
    for e in &chosen {
        let (root, base) = match e.hive {
            quado_model::record::Hive::Machine => (Root::LocalMachine, extract::UNINSTALL_KEYS[0].1),
            quado_model::record::Hive::Machine32 => (Root::LocalMachine, extract::UNINSTALL_KEYS[1].1),
            quado_model::record::Hive::User => (Root::CurrentUser, extract::UNINSTALL_KEYS[2].1),
        };
        out.key(&reg, root, &format!(r"{base}\{}", e.key), None, &anon);
    }
    out.key(&reg, Root::CurrentUser, r"Software\Valve\Steam", Some(&["SteamPath"]), &anon);
    out.key(&reg, Root::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\StorageSense\Parameters\StoragePolicy", Some(&["01", "2048", "04", "08", "256", "32", "512"]), &anon);
    if let Some(g) = quado_kb::win::volume_guid("C:\\") {
        out.key(&reg, Root::CurrentUser, &format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume\{g}"), None, &anon);
        println!("GUID del volume C: {g}");
    }
    out.key(&reg, Root::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced", Some(&["Start_TrackProgs"]), &anon);
    out.key(&reg, Root::LocalMachine, r"SYSTEM\CurrentControlSet\Control\Session Manager\Power", Some(&["HiberbootEnabled"]), &anon);
    // UserAssist: due voci di app di Windows, per il formato (i dati sono fermi: Start_TrackProgs = 0).
    let ua = r"Software\Microsoft\Windows\CurrentVersion\Explorer\UserAssist";
    if let Ok(guids) = reg.subkeys(Root::CurrentUser, ua) {
        for g in guids {
            let path = format!(r"{ua}\{g}\Count");
            let Ok(values) = reg.values(Root::CurrentUser, &path) else { continue };
            let names: Vec<String> = values.iter().filter(|(n, v)| matches!(v, RegValue::Binary(b) if b.len() == 72) && n.starts_with("Zvpebfbsg.Jvaqbjf")).map(|(n, _)| n.clone()).take(2).collect();
            if !names.is_empty() {
                let refs: Vec<&str> = names.iter().map(String::as_str).collect();
                out.key(&reg, Root::CurrentUser, &path, Some(&refs), &anon);
                break;
            }
        }
    }
    write(&dir, "registro.reg", &out.text)?;

    // File nominati dalle regole.
    let files = LiveFiles;
    let copy = |path: &str| -> Result<(), String> {
        match files.read(path) {
            Ok(t) => write(&dir, &file_rel(&anon.apply(path)), &anon.apply(&t)),
            Err(e) => {
                eprintln!("non letto {path}: {e:?}");
                Ok(())
            }
        }
    };
    copy(&j(&local, extract::CAPCUT_PRODUCT_INFO))?;
    let steam = extract::steam(&reg, &files, "C:\\").map_err(|e| format!("steam: {e:?}"))?;
    copy(&j(&j(&steam.root, "steamapps"), "libraryfolders.vdf"))?;
    let mut apps = steam.apps.clone();
    apps.sort_by_key(|a| std::cmp::Reverse(a.size_on_disk.unwrap_or(0)));
    let keep: Vec<_> = apps.iter().take(3).collect();
    for a in &keep {
        copy(&j(&j(&a.library, "steamapps"), &format!("appmanifest_{}.acf", a.appid)))?;
    }
    // localconfig.vdf: solo LastPlayed dei giochi scelti (il file contiene altro, che non serve).
    let mut lc = String::from("\"UserLocalConfigStore\"\n{\n\t\"Software\"\n\t{\n\t\t\"Valve\"\n\t\t{\n\t\t\t\"Steam\"\n\t\t\t{\n\t\t\t\t\"apps\"\n\t\t\t\t{\n");
    for a in &keep {
        if let Some((_, t)) = steam.local_last_played.iter().find(|(id, _)| *id == a.appid) {
            let _ = write!(lc, "\t\t\t\t\t\"{}\"\n\t\t\t\t\t{{\n\t\t\t\t\t\t\"LastPlayed\"\t\t\"{t}\"\n\t\t\t\t\t}}\n", a.appid);
        }
    }
    lc.push_str("\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n}\n");
    write(&dir, &file_rel(&j(&j(&j(&j(&steam.root, "userdata"), "1000"), "config"), "localconfig.vdf")), &lc)?;
    let pd = folders.program_data.clone().unwrap_or_else(|| r"C:\ProgramData".into());
    let mdir = j(&pd, extract::EPIC_MANIFESTS);
    if let Ok(names) = files.list(&mdir) {
        for n in names.iter().filter(|n| n.ends_with(".item")) {
            copy(&j(&mdir, n))?;
        }
    }
    // globalSetting di CapCut: solo le righe dei percorsi.
    if let Ok(t) = files.read(&j(&local, extract::CAPCUT_SETTINGS)) {
        let lines: String = t.lines().filter(|l| l.contains("Path=")).map(|l| format!("{}\n", anon.apply(l))).collect();
        write(&dir, &file_rel(&anon.apply(&j(&local, extract::CAPCUT_SETTINGS))), &lines)?;
    }
    // Pacchetti MSIX con dati in Packages, più un framework.
    if let Ok(pkgs) = quado_kb::win::msix() {
        let mut text = String::from("# nome di famiglia|percorso di installazione|framework (bozza di cargo xtask fixture-regole)\n");
        let (mut n, mut fw) = (0, false);
        for p in &pkgs {
            let has_data = Path::new(&j(&j(&local, "Packages"), &p.family_name)).is_dir();
            if (p.framework && !fw) || (!p.framework && has_data && n < 6) {
                fw |= p.framework;
                let _ = writeln!(text, "{}|{}|{}", p.family_name, p.installed_path.clone().unwrap_or_default(), u8::from(p.framework));
                n += usize::from(!p.framework);
            }
        }
        write(&dir, "msix.txt", &text)?;
    }
    Ok(())
}
