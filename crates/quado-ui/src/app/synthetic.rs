// SPDX-License-Identifier: GPL-3.0-or-later
//! Dati sintetici per test, immagini e banchi di misura: un volume piccolo e realistico, una tabella da
//! milioni di righe, una cartella con un milione di figli, una mappa da 20.000 aree. Nessun file su disco:
//! solo il modello in memoria.

use quado_model::{Balance, DirExtra, Flags, Links, Metadata, NodeData, Space, Tree};
use quado_scan_win::volumes::{DriveKind, Support};
use quado_scan_win::win::NtfsVolumeData;
use quado_scan_win::{Problem, ScanResult, Stats, VolumeFacts};

use super::data::VolumeInfo;

const GIB: u64 = 1 << 30;
const MIB: u64 = 1 << 20;
/// 13/10/2026 12:00 UTC come `FILETIME`.
pub const T0: i64 = 134_363_664_000_000_000;
const DAY: i64 = 864_000_000_000;

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn dir(mtime: i64) -> NodeData {
    NodeData { flags: Flags::DIR, mtime, ..Default::default() }
}

fn file(bytes: u64, mtime: i64) -> NodeData {
    let alloc = bytes.div_ceil(4096) * 4096;
    NodeData { logical: bytes, alloc, mtime, attrs: 0x20, ..Default::default() }
}

/// Generatore pseudocasuale riproducibile (xorshift).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }
}

pub fn volume_c() -> VolumeInfo {
    VolumeInfo { root: "C:\\".into(), label: String::new(), file_system: "NTFS".into(), kind: DriveKind::Fixed, support: Support::Full, is_system: true }
}

fn result(mut tree: Tree, problems: Vec<(u32, Problem)>, capacity: u64, cancelled: bool) -> ScanResult {
    tree.aggregate();
    tree.seal();
    let allocated = tree.alloc(Tree::ROOT);
    let dirs = (0..tree.len() as u32).filter(|&i| tree.is_dir(i)).count() as u64 - 1;
    let files = u64::from(tree.files(Tree::ROOT));
    let metadata = Metadata { mft: Some(GIB + 740 * MIB), dir_indexes: tree.dir_index_total(), usn_journal: Some(32 * MIB), reserved: Some(4 * GIB + 160 * MIB) };
    let used = allocated + metadata.total() + 9 * GIB + 300 * MIB;
    let free = capacity.saturating_sub(used);
    let end = Space { capacity, free, free_to_caller: free };
    let start = Space { capacity, free: free + 180 * 1024, free_to_caller: free + 180 * 1024 };
    let links = Links { nodes: Vec::new(), groups: Vec::new(), extra_alloc: GIB, extra_logical: GIB, extra_names: 48_210 };
    let balance = Balance::compute(start, end, allocated, metadata, links.extra_alloc);
    let ntfs = NtfsVolumeData { serial: 0x1234, bytes_per_cluster: 4096, bytes_per_record: 1024, mft_valid_data_length: GIB + 740 * MIB };
    ScanResult {
        tree,
        links,
        problems,
        stats: Stats { files, dirs, ..Default::default() },
        volume: Some(VolumeFacts { ntfs: Some(ntfs), usn: None, space_start: start, space_end: end }),
        balance: Some(balance),
        cancelled,
    }
}

/// Un C: piccolo con la forma di uno vero: profilo con AppData e Download, CapCut, Steam, Windows,
/// cartelle negate, un collegamento non seguito, un file cloud, hard link, nomi in altre scritture.
pub fn sample_c(cancelled: bool) -> (VolumeInfo, ScanResult) {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let mut rng = Rng::new(42);
    let mut problems = Vec::new();
    let root = Tree::ROOT;
    let users = t.push(root, &w("Users"), dir(T0 - 40 * DAY));
    let demo = t.push(users, &w("demo"), dir(T0 - DAY));
    let appdata = t.push(demo, &w("AppData"), dir(T0 - DAY));
    let local = t.push(appdata, &w("Local"), dir(T0 - DAY));
    let capcut = t.push(local, &w("CapCut"), dir(T0 - 3 * DAY));
    let apps = t.push(capcut, &w("Apps"), dir(T0 - 3 * DAY));
    t.push(apps, &w("ProductInfo.xml"), file(368, T0 - 3 * DAY));
    t.push(apps, &w("uninst.exe"), file(2 * MIB, T0 - 3 * DAY));
    // 11 cartelle di versione: CapCut tiene le versioni precedenti a ogni aggiornamento.
    for (k, v) in CAPCUT_VERSIONS.iter().enumerate() {
        let v = t.push(apps, &w(v), dir(T0 - (330 - k as i64 * 30) * DAY));
        for j in 0..6 {
            t.push(v, &w(&format!("modulo{j}.dll")), file(190 * MIB + rng.below(25) * MIB, T0 - (330 - k as i64 * 30) * DAY));
        }
    }
    let ud = t.push(capcut, &w("User Data"), dir(T0 - 2 * DAY));
    let cache = t.push(ud, &w("Cache"), dir(T0 - 2 * DAY));
    for j in 0..40 {
        t.push(cache, &w(&format!("blocco_{j:03}.bin")), file(95 * MIB + rng.below(20) * MIB, T0 - rng.below(30) as i64 * DAY));
    }
    let projects = t.push(ud, &w("Projects"), dir(T0 - 5 * DAY));
    for j in 0..6 {
        t.push(projects, &w(&format!("Progetto {j}.mp4")), file(480 * MIB + rng.below(120) * MIB, T0 - (5 + j) as i64 * DAY));
    }
    let npm = t.push(local, &w("npm-cache"), dir(T0 - 9 * DAY));
    let cacache = t.push(npm, &w("_cacache"), dir(T0 - 9 * DAY));
    for j in 0..120 {
        t.push(cacache, &w(&format!("pacchetto-{j}.tgz")), file(28 * MIB + rng.below(6) * MIB, T0 - 60 * DAY));
    }
    // Roaming e LocalLow, con una cartella senza programma corrispondente (possibile residuo).
    let roaming = t.push(appdata, &w("Roaming"), dir(T0 - DAY));
    let old = t.push(roaming, &w("VecchioEditor"), dir(T0 - 500 * DAY));
    t.push(old, &w("impostazioni.json"), file(40 * 1024, T0 - 500 * DAY));
    let locallow = t.push(appdata, &w("LocalLow"), dir(T0 - 20 * DAY));
    let unity = t.push(locallow, &w("Unity"), dir(T0 - 20 * DAY));
    t.push(unity, &w("cache.data"), file(300 * MIB, T0 - 20 * DAY));
    for (name, gib) in [("Autodesk", 5u64), ("NVIDIA", 5), ("BraveSoftware", 5), ("GiocoArcade", 4)] {
        let d = t.push(local, &w(name), dir(T0 - 50 * DAY));
        for j in 0..8 {
            t.push(d, &w(&format!("dati{j}.dat")), file(gib * GIB / 8, T0 - 50 * DAY));
        }
    }
    // Una cartella cloud non elencata (RECALL_ON_DATA_ACCESS) e un segnaposto.
    let onedrive = t.push(demo, &w("OneDrive"), dir(T0 - 4 * DAY));
    let ph = t.push(onedrive, &w("Relazione.pdf"), NodeData { logical: 48 * MIB, alloc: 0, mtime: T0 - 4 * DAY, attrs: 0x0040_0000, flags: Flags::CLOUD });
    let _ = ph;
    let cl = t.push(onedrive, &w("Archivio"), NodeData { flags: Flags::DIR | Flags::CLOUD | Flags::CLOUD_NOT_LISTED, mtime: T0 - 4 * DAY, ..Default::default() });
    let _ = cl;
    let dl = t.push(demo, &w("Downloads"), dir(T0 - DAY));
    for j in 0..140 {
        let ext = ["exe", "zip", "msi", "iso", "pdf", "mp4"][j % 6];
        t.push(dl, &w(&format!("download-{j}.{ext}")), file(400 * MIB + rng.below(900) * MIB, T0 - rng.below(700) as i64 * DAY));
    }
    t.push(dl, &w("写真アルバム.zip"), file(700 * MIB, T0 - 20 * DAY));
    // Lente Download: un caso per famiglia e per tipo di voce.
    t.push(dl, &w("setup.exe"), file(120 * MIB, T0 - 40 * DAY));
    t.push(dl, &w("setup (1).exe"), file(121 * MIB, T0 - 39 * DAY));
    t.push(dl, &w("SteamSetup.exe"), file(2 * MIB + 300 * 1024, T0 - 400 * DAY));
    t.push(dl, &w("7z2409-x64.msi"), file(1_900 * 1024, T0 - 300 * DAY));
    t.push(dl, &w("Fattura marzo.pdf"), file(312 * 1024, T0 - 90 * DAY));
    t.push(dl, &w("Fattura marzo (1).pdf"), file(312 * 1024, T0 - 89 * DAY));
    t.push(dl, &w("fattura-copia.pdf"), file(312 * 1024, T0 - 10 * DAY));
    t.push(dl, &w("Sito web.zip"), file(180 * MIB, T0 - 30 * DAY));
    let site = t.push(dl, &w("Sito web"), dir(T0 - 30 * DAY));
    for j in 0..12 {
        t.push(site, &w(&format!("pagina{j}.html")), file(15 * MIB, T0 - 30 * DAY));
    }
    t.push(dl, &w("Win11_24H2_Italian_x64.iso"), file(5 * GIB + 700 * MIB, T0 - 200 * DAY));
    t.push(dl, &w("concerto.mp4.crdownload"), file(GIB + 200 * MIB, T0 - 2 * DAY));
    t.push(dl, &w("vacanze.jpg"), file(6 * MIB, T0 - 60 * DAY));
    t.push(dl, &w("podcast.mp3"), file(80 * MIB, T0 - 15 * DAY));
    let docs = t.push(demo, &w("Documents"), dir(T0 - 2 * DAY));
    let proj = t.push(docs, &w("progetto-rust"), dir(T0 - DAY));
    t.push(proj, &w("Cargo.lock"), file(110 * 1024, T0 - DAY));
    let target = t.push(proj, &w("target"), dir(T0 - DAY));
    for j in 0..30 {
        t.push(target, &w(&format!("progetto-{j}.rlib")), file(150 * MIB, T0 - DAY));
    }
    t.push(docs, &w("package-lock.json"), file(24 * 1024, T0 - 30 * DAY));
    let nm = t.push(docs, &w("node_modules"), dir(T0 - 30 * DAY));
    for j in 0..300 {
        t.push(nm, &w(&format!("modulo-{j}.js")), file(9 * MIB, T0 - 30 * DAY));
    }
    // Junction di compatibilità: non seguita.
    t.push(demo, &w("Dati applicazioni"), NodeData { flags: Flags::DIR | Flags::REPARSE | Flags::NOT_FOLLOWED, mtime: T0 - 400 * DAY, attrs: 0x416, ..Default::default() });
    let other = t.push(users, &w("ospite"), NodeData { flags: Flags::DIR | Flags::DENIED, mtime: T0 - 200 * DAY, ..Default::default() });
    problems.push((other, Problem::AccessDenied));

    let pf86 = t.push(root, &w("Program Files (x86)"), dir(T0 - 10 * DAY));
    let steam = t.push(pf86, &w("Steam"), dir(T0 - 10 * DAY));
    t.push(steam, &w("uninstall.exe"), file(MIB, T0 - 200 * DAY));
    let steamapps = t.push(steam, &w("steamapps"), dir(T0 - 10 * DAY));
    let common = t.push(steamapps, &w("common"), dir(T0 - 10 * DAY));
    // Giochi inventati: Steam li riconosce dai manifest, nessuna regola guarda il nome.
    for (name, gib) in [("hollow peaks", 58u64), ("Dune Rally", 44), ("Brinewood", 27), ("Quillfire", 11)] {
        let g = t.push(common, &w(name), dir(T0 - 15 * DAY));
        for j in 0..12 {
            t.push(g, &w(&format!("pak{j:02}_dir.vpk")), file(gib * GIB / 12, T0 - 15 * DAY));
        }
    }
    let pf = t.push(root, &w("Program Files"), dir(T0 - 30 * DAY));
    let zip7 = t.push(pf, &w("7-Zip"), dir(T0 - 300 * DAY));
    t.push(zip7, &w("7z.exe"), file(560 * 1024, T0 - 300 * DAY));
    t.push(zip7, &w("7z.dll"), file(1_800 * 1024, T0 - 300 * DAY));
    let epic = t.push(pf, &w("Epic Games"), dir(T0 - 30 * DAY));
    let ue = t.push(epic, &w("UE_5.5"), dir(T0 - 30 * DAY));
    for j in 0..20 {
        t.push(ue, &w(&format!("Engine{j}.pak")), file(GIB + 400 * MIB, T0 - 90 * DAY));
    }
    let win = t.push(root, &w("Windows"), dir(T0 - 5 * DAY));
    let sxs = t.push(win, &w("WinSxS"), dir(T0 - 5 * DAY));
    for j in 0..200 {
        t.push(sxs, &w(&format!("amd64_componente_{j}.dll")), file(40 * MIB, T0 - 100 * DAY));
    }
    let wa = t.push(pf, &w("WindowsApps"), NodeData { flags: Flags::DIR | Flags::DENIED, mtime: T0 - 3 * DAY, ..Default::default() });
    problems.push((wa, Problem::AccessDenied));
    let svi = t.push(root, &w("System Volume Information"), NodeData { flags: Flags::DIR | Flags::DENIED, mtime: T0 - DAY, attrs: 0x16, ..Default::default() });
    problems.push((svi, Problem::AccessDenied));
    let bin = t.push(root, &w("$Recycle.Bin"), NodeData { flags: Flags::DIR, attrs: 0x16, mtime: T0 - DAY, ..Default::default() });
    let mine = t.push(bin, &w("S-1-5-21-1-2-3-1001"), dir(T0 - DAY));
    for j in 0..3 {
        let r = t.push(mine, &w(&format!("$RZK{j}QX")), dir(T0 - 100 * DAY));
        t.push(r, &w("contenuto.bin"), file(3 * GIB + 200 * MIB, T0 - 100 * DAY));
    }
    let otherbin = t.push(bin, &w("S-1-5-21-9-9-9-1002"), NodeData { flags: Flags::DIR | Flags::DENIED, mtime: T0 - 300 * DAY, ..Default::default() });
    problems.push((otherbin, Problem::AccessDenied));
    t.push(root, &w("hiberfil.sys"), NodeData { logical: 6 * GIB + 410 * MIB, alloc: 6 * GIB + 410 * MIB, mtime: T0 - DAY, attrs: 0x26, flags: Flags::default() });
    t.push(root, &w("pagefile.sys"), NodeData { logical: 4 * GIB, alloc: 4 * GIB, mtime: T0 - DAY, attrs: 0x26, flags: Flags::default() });
    let cmp = t.push(root, &w("compresso.log"), file(80 * MIB, T0 - 2 * DAY));
    t.set_alloc(cmp, 20 * MIB);
    t.add_flags(cmp, Flags::ALLOC_FIXED);
    for d in [users, demo, appdata, local, capcut, apps, win, sxs] {
        t.set_dir_extra(DirExtra { node: d, index_alloc: 8192, error: 0 });
    }
    // Unità da 512 GB: 476 GiB per Windows.
    (volume_c(), result(t, problems, 476 * GIB + 330 * MIB, cancelled))
}

/// Versioni di CapCut in `Apps`, dalla più vecchia a quella in uso.
pub const CAPCUT_VERSIONS: [&str; 11] = [
    "6.4.1.2547", "7.0.0.2833", "7.5.0.3062", "8.1.0.3402", "8.4.1.3561", "8.7.2.3701", "9.0.1.3866", "9.1.2.3897", "9.2.1.3944", "9.3.0.3970", "9.4.0.4015",
];

/// Prove degli estrattori per `sample_c`, con valori inventati nel formato di quelli veri: voci di
/// disinstallazione di CapCut e Steam, manifest di Steam ed Epic, CapCut 9.4.0.4015, alimentazione,
/// Sensore memoria, Cestino.
pub fn sample_c_evidence() -> quado_kb::Evidence {
    use quado_kb::extract::{BinSettings, ConfigOrigin, Epic, EpicItem, ExtractError, Power, Steam, SteamApp, StorageSense, ToolPath, TrackProgs, Uninstall, UninstallEntry, UserAssist};
    use quado_model::record::Hive;
    let mut ev = quado_kb::Evidence::empty("C:\\");
    ev.folders = quado_kb::Evidence::default_folders("C:\\", "demo");
    ev.system_volume = true;
    ev.user_sid = Some("S-1-5-21-1-2-3-1001".into());
    let entry = |hive: Hive, name: &str, version: Option<&str>, icon: Option<&str>| UninstallEntry {
        hive,
        key: name.into(),
        display_name: name.into(),
        version: version.map(str::to_owned),
        publisher: None,
        install_location: None,
        uninstall_string: None,
        display_icon: icon.map(str::to_owned),
        install_date: None,
        estimated_kb: None,
        system_component: false,
    };
    let mut capcut = entry(Hive::User, "CapCut", Some("9.4.0.4015"), Some(r"C:\Users\demo\AppData\Local\CapCut\Apps\uninst.exe"));
    capcut.publisher = Some("Bytedance Pte. Ltd.".into());
    capcut.install_date = Some("20260929".into());
    let mut steam = entry(Hive::Machine32, "Steam", Some("2.10.91.91"), Some(r"C:\Program Files (x86)\Steam\uninstall.exe"));
    steam.publisher = Some("Valve Corporation".into());
    // 7-Zip: voce di HKLM con InstallLocation e la dimensione dichiarata.
    let mut zip7 = entry(Hive::Machine, "7-Zip 24.09 (x64 edition)", Some("24.09.00.0"), None);
    zip7.key = "{23170F69-40C1-2702-2409-000001000000}".into();
    zip7.publisher = Some("Igor Pavlov".into());
    zip7.install_location = Some(r"C:\Program Files\7-Zip\".into());
    zip7.install_date = Some("20250412".into());
    zip7.estimated_kb = Some(5_920);
    let mut redist = entry(Hive::Machine, "Microsoft Visual C++ 2022 X64 Additional Runtime", Some("14.40.33810"), None);
    redist.system_component = true;
    ev.uninstall = Ok(Uninstall {
        entries: vec![
            capcut,
            steam,
            zip7,
            redist,
            entry(Hive::Machine, "NVIDIA Graphics Driver 617.42", Some("617.42"), None),
            entry(Hive::User, "Autodesk Fusion", None, None),
        ],
        skipped: Vec::new(),
    });
    ev.msix = Ok(vec![quado_kb::extract::MsixPackage {
        family_name: "Microsoft.WindowsTerminal_8wekyb3d8bbwe".into(),
        installed_path: Some(r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_1.21.2361.0_x64__8wekyb3d8bbwe".into()),
        framework: false,
    }]);
    let lib = r"C:\Program Files (x86)\Steam".to_owned();
    let app = |appid: u32, name: &str, dir: &str, days: i64| SteamApp {
        appid,
        name: name.into(),
        installdir: dir.into(),
        library: lib.clone(),
        size_on_disk: None,
        last_played: Some((T0 / 10_000_000 - 11_644_473_600) - days * 86_400),
    };
    ev.steam = Ok(Steam {
        root: lib.clone(),
        libraries: vec![lib.clone()],
        apps: vec![
            app(3_999_910, "Hollow Peaks", "hollow peaks", 3),
            app(3_999_920, "Dune Rally 2", "Dune Rally", 12),
            app(3_999_950, "Quillfire", "Quillfire", 400),
            app(3_999_940, "Brinewood", "Brinewood", 30),
        ],
        local_last_played: Vec::new(),
        skipped: Vec::new(),
    });
    ev.epic = Ok(Epic {
        items: vec![EpicItem { display_name: "Unreal Engine".into(), app_name: "UE_5.5".into(), install_location: r"C:\Program Files\Epic Games\UE_5.5".into(), version: None }],
        skipped: Vec::new(),
    });
    ev.capcut = Ok("9.4.0.4015".into());
    ev.capcut_drafts = Ok(r"C:\Users\demo\AppData\Local\CapCut\User Data\Projects\com.lveditor.draft".into());
    ev.track_progs = TrackProgs::Off;
    ev.userassist = Ok(UserAssist::NotTracked(TrackProgs::Off));
    ev.power = Ok(Power { hibernate: true, hiberfile_present: true, fast_startup: Some(false), has_battery: true, modern_standby: false, critical_battery_action: Some(2) });
    ev.storage_sense = Ok(StorageSense { configured: true, enabled: true, frequency_days: Some(30), recycle_days: Some(30), downloads_days: None });
    ev.bin = Ok(BinSettings { max_capacity_mib: Some(24_380), nuke_on_delete: false });
    ev.recycle = Ok(quado_kb::RecycleQuery { bytes: 1_318_890, items: 1 });
    ev.npm_cache = ToolPath { path: r"C:\Users\demo\AppData\Local\npm-cache".into(), origin: ConfigOrigin::Default };
    ev.pnpm_store = ToolPath { path: r"C:\Users\demo\AppData\Local\pnpm\store".into(), origin: ConfigOrigin::Default };
    ev.restore = Err(ExtractError::Io(5));
    ev
}

/// Albero da `n` nodi con profondità e ventaglio variabili (tabella da 3,5 milioni di righe).
pub fn big_tree(n: usize, seed: u64) -> (VolumeInfo, ScanResult) {
    let mut t = Tree::with_root(&w("C:\\"), n);
    let mut rng = Rng::new(seed);
    let mut dirs: Vec<u32> = vec![Tree::ROOT];
    let mut name = String::new();
    while t.len() < n {
        let p = dirs[rng.below(dirs.len() as u64) as usize];
        name.clear();
        use std::fmt::Write as _;
        if rng.below(8) == 0 {
            let _ = write!(name, "cartella {}", t.len());
            let d = t.push(p, &w(&name), dir(T0 - rng.below(2000) as i64 * DAY));
            dirs.push(d);
        } else {
            let _ = write!(name, "file {}.bin", t.len());
            let shift = 10 + rng.below(20);
            let bytes = rng.below(1 << shift);
            t.push(p, &w(&name), file(bytes, T0 - rng.below(2000) as i64 * DAY));
        }
    }
    (volume_c(), result(t, Vec::new(), 2000 * GIB, false))
}

/// Una cartella con `children` figli diretti dentro un volume piccolo.
pub fn wide_folder(children: usize) -> (VolumeInfo, ScanResult, u32) {
    let mut t = Tree::with_root(&w("C:\\"), children + 16);
    let mut rng = Rng::new(7);
    let big = t.push(Tree::ROOT, &w("molti file"), dir(T0));
    let mut name = String::new();
    for k in 0..children {
        use std::fmt::Write as _;
        name.clear();
        let _ = write!(name, "voce {k:07}.dat");
        t.push(big, &w(&name), file(4096 + rng.below(1 << 22), T0 - rng.below(500) as i64 * DAY));
    }
    t.push(Tree::ROOT, &w("altro.bin"), file(GIB, T0));
    let (v, r) = (volume_c(), result(t, Vec::new(), 2000 * GIB, false));
    (v, r, big)
}

/// Mappa con `groups` cartelle da `per` file l'una (20.000 aree a 1920×1080).
pub fn map_tree(groups: usize, per: usize) -> (VolumeInfo, ScanResult) {
    let mut t = Tree::with_root(&w("C:\\"), groups * (per + 1) + 1);
    let mut rng = Rng::new(11);
    for g in 0..groups {
        let d = t.push(Tree::ROOT, &w(&format!("gruppo {g}")), dir(T0));
        for k in 0..per {
            // Dimensioni concentrate: le aree restano tutte sopra i 2 pixel quadrati.
            t.push(d, &w(&format!("file {k}")), file(64 * MIB + rng.below(64 * MIB), T0));
        }
    }
    (volume_c(), result(t, Vec::new(), 4000 * GIB, false))
}

// ---------------------------------------------------------------------------------------------
// Letture finte della lente Download

/// Letture finte della lente Download per test e immagini: nessun file si apre. Dimensioni e
/// date vengono dall'albero dato; il contenuto dal nome del file, sui dati di `sample_c`:
/// - impronte: le tre fatture hanno lo stesso contenuto, gli altri file uno diverso per nome; data di
///   creazione: un giorno prima della data di modifica, così la più vecchia è `Fattura marzo.pdf`;
/// - installatori: `7z2409-x64.msi` è 7-Zip 24.09 con il ProductCode della voce di disinstallazione
///   (stessa versione, da fonte), `SteamSetup.exe` è Steam in un'altra versione, `setup.exe` e
///   `setup (1).exe` un programma non installato, gli altri senza dati di versione;
/// - indice di `Sito web.zip`: le 12 pagine della cartella accanto più un file che non c'è;
/// - Restart Manager: nessun processo, salvo i file in [`Self::in_use`].
///
/// Contano le letture del contenuto (impronte, dati di versione, indici) per nome, le letture dei soli
/// attributi e le interrogazioni di Restart Manager.
pub struct FakeReaders {
    files: std::collections::HashMap<String, (u64, i64)>,
    /// Nomi dei file letti nel contenuto, in ordine di lettura.
    pub content: std::sync::Mutex<Vec<String>>,
    pub identities: std::sync::atomic::AtomicUsize,
    pub usage_queries: std::sync::atomic::AtomicUsize,
    /// File in uso, per nome: i processi che li usano.
    pub in_use: std::sync::Mutex<std::collections::HashMap<String, Vec<String>>>,
    /// Zip con l'indice rovinato, per nome.
    pub broken_zip: std::sync::Mutex<Vec<String>>,
    /// Data di modifica cambiata dopo la scansione, per nome (la chiave della cache cambia).
    pub changed: std::sync::Mutex<std::collections::HashMap<String, i64>>,
    /// Dopo questo numero di impronte le altre aspettano l'annullamento (annullamento a metà).
    pub hash_limit: std::sync::atomic::AtomicUsize,
    hashes: std::sync::atomic::AtomicUsize,
}

impl FakeReaders {
    /// Letture finte per i file di `tree`.
    pub fn for_tree(tree: &Tree) -> Self {
        let mut files = std::collections::HashMap::new();
        for n in 0..tree.len() as u32 {
            if !tree.is_dir(n) {
                files.insert(tree.path_lossy(n).to_lowercase(), (tree.logical(n), tree.mtime(n)));
            }
        }
        Self {
            files,
            content: std::sync::Mutex::new(Vec::new()),
            identities: std::sync::atomic::AtomicUsize::new(0),
            usage_queries: std::sync::atomic::AtomicUsize::new(0),
            in_use: std::sync::Mutex::new(std::collections::HashMap::new()),
            broken_zip: std::sync::Mutex::new(Vec::new()),
            changed: std::sync::Mutex::new(std::collections::HashMap::new()),
            hash_limit: std::sync::atomic::AtomicUsize::new(usize::MAX),
            hashes: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Letture del contenuto fatte finora.
    pub fn content_reads(&self) -> usize {
        self.content.lock().map(|c| c.len()).unwrap_or(0)
    }

    /// Nomi dei file letti nel contenuto.
    pub fn content_names(&self) -> Vec<String> {
        self.content.lock().map(|c| c.clone()).unwrap_or_default()
    }

    fn name(path: &str) -> String {
        path.rsplit('\\').next().unwrap_or(path).to_owned()
    }

    fn read(&self, path: &str) {
        if let Ok(mut c) = self.content.lock() {
            c.push(Self::name(path));
        }
    }

    fn id(&self, path: &str) -> Result<quado_scan_win::content::FileIdentity, quado_scan_win::content::ContentError> {
        let &(size, mtime) = self.files.get(&path.to_lowercase()).ok_or(quado_scan_win::content::ContentError::Io(2))?;
        let modified = self.changed.lock().ok().and_then(|c| c.get(&Self::name(path)).copied()).unwrap_or(mtime);
        let file_id = path.to_lowercase().bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
        Ok(quado_scan_win::content::FileIdentity { size, created: mtime - DAY, modified, accessed: modified, file_id: u128::from(file_id), volume_serial: 1, attributes: 0x20 })
    }
}

impl super::lens_jobs::LensReaders for FakeReaders {
    fn identity(&self, path: &str) -> Result<quado_scan_win::content::FileIdentity, quado_scan_win::content::ContentError> {
        self.identities.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.id(path)
    }

    fn sha256(
        &self,
        path: &str,
        cancel: &std::sync::atomic::AtomicBool,
        progress: &std::sync::atomic::AtomicU64,
    ) -> Result<(quado_scan_win::content::FileIdentity, [u8; 32]), quado_scan_win::content::ContentError> {
        use std::sync::atomic::Ordering::Relaxed;
        let k = self.hashes.fetch_add(1, Relaxed);
        if k >= self.hash_limit.load(Relaxed) {
            // Ferma qui finché non arriva l'annullamento (al più qualche secondo, per non bloccare un test).
            for _ in 0..5000 {
                if cancel.load(Relaxed) {
                    return Err(quado_scan_win::content::ContentError::Cancelled);
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        if cancel.load(Relaxed) {
            return Err(quado_scan_win::content::ContentError::Cancelled);
        }
        let id = self.id(path)?;
        self.read(path);
        progress.fetch_add(id.size, Relaxed);
        let name = Self::name(path).to_lowercase();
        let digest = if name.contains("fattura") {
            [0xFA; 32]
        } else {
            let mut d = [0u8; 32];
            let h = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
            for (k, b) in d.iter_mut().enumerate() {
                *b = (h >> ((k % 8) * 8)) as u8 ^ k as u8;
            }
            d
        };
        Ok((id, digest))
    }

    fn installer(&self, path: &str, kind: quado_kb::installers::InstallerKind) -> Result<quado_kb::installers::InstallerInfo, quado_kb::extract::ExtractError> {
        use quado_kb::installers::InstallerInfo;
        self.id(path).map_err(|_| quado_kb::extract::ExtractError::NotPresent)?;
        self.read(path);
        let info = |name: &str, version: &str, company: &str| InstallerInfo {
            kind: Some(kind),
            product_name: Some(name.into()),
            product_version: Some(version.into()),
            company: Some(company.into()),
            ..Default::default()
        };
        Ok(match Self::name(path).as_str() {
            "7z2409-x64.msi" => InstallerInfo { product_code: Some("{23170F69-40C1-2702-2409-000001000000}".into()), ..info("7-Zip 24.09 (x64 edition)", "24.09.00.0", "Igor Pavlov") },
            "SteamSetup.exe" => info("Steam", "2.10.91.0", "Valve Corporation"),
            "setup.exe" => info("Programma di prova", "1.0", "Editore di prova"),
            "setup (1).exe" => info("Programma di prova", "1.1", "Editore di prova"),
            _ => InstallerInfo { kind: Some(kind), ..Default::default() },
        })
    }

    fn zip_index(&self, path: &str) -> Result<quado_scan_win::zip::ZipIndex, quado_scan_win::zip::ZipError> {
        use quado_model::downloads::ArchiveEntry;
        self.read(path);
        let name = Self::name(path);
        if self.broken_zip.lock().is_ok_and(|b| b.contains(&name)) {
            return Err(quado_scan_win::zip::ZipError::Corrupt);
        }
        let mut entries = Vec::new();
        if name == "Sito web.zip" {
            for j in 0..12 {
                entries.push(ArchiveEntry { path: format!("pagina{j}.html"), size: 15 * MIB, dir: false });
            }
            entries.push(ArchiveEntry { path: "immagini/logo.png".into(), size: 2048, dir: false });
        }
        Ok(quado_scan_win::zip::ZipIndex { entries, zip64: false })
    }

    fn processes(&self, paths: &[String]) -> Result<Vec<String>, u32> {
        self.usage_queries.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let used = self.in_use.lock().map(|u| u.clone()).unwrap_or_default();
        Ok(paths.iter().filter_map(|p| used.get(&Self::name(p))).flatten().cloned().collect())
    }

    fn now(&self) -> i64 {
        T0
    }
}
