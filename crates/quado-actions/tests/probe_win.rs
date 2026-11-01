// SPDX-License-Identifier: GPL-3.0-or-later
//! `WinProbe` sul sistema vero (05 D7, 03 §6.3): attributi senza aprire il contenuto, elenco delle
//! cartelle, Restart Manager, cartella di sincronizzazione di prova, spazio libero. Solo letture delle
//! cose create dal test in una cartella temporanea su C:; il Cestino di C: si legge soltanto.

use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::Path;
use std::sync::Mutex;

use quado_actions::fake;
use quado_actions::plan::Block;
use quado_actions::probe::{Attrs, Entry, ListError, Probe, SystemConfig, VolumeBin, entries_one_by_one};
use quado_actions::restart::{ProcessUse, RmError};
use quado_actions::simulate::{Request, RequestItem, SimProgress, simulate};
use quado_actions::win_probe::WinProbe;
use quado_model::intervention::ReferenceReport;
use quado_model::record::State;
use quado_testkit::cloud::{Placeholder, TestSyncRoot};
use quado_testkit::trees::{self, TestDir};

/// Configurazione vuota: nessuna impostazione del Cestino leggibile, nessun riferimento.
struct NoConfig;

impl SystemConfig for NoConfig {
    fn bin_settings(&self, _: &str) -> Result<(Option<u64>, bool), u32> {
        Err(2)
    }
    fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
        Ok(Some(30))
    }
    fn references(&self, _: &[String]) -> ReferenceReport {
        ReferenceReport::default()
    }
}

fn probe() -> WinProbe {
    WinProbe::new(Box::new(NoConfig))
}

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}

const ATTR_DIRECTORY: u32 = 0x10;
const ATTR_SPARSE: u32 = 0x200;
const ATTR_REPARSE: u32 = 0x400;
const ATTR_COMPRESSED: u32 = 0x800;
const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;

#[test]
fn file_and_directory() {
    let d = TestDir::new("sonda-attributi");
    let f = d.join("normale.bin");
    trees::write_file(&f, 100_000, 3);
    let p = probe();
    let a = p.attributes(&s(&f)).unwrap();
    let m = fs::metadata(&f).unwrap();
    eprintln!("file: {a:?}");
    assert_eq!(a.logical, 100_000);
    assert!(a.alloc >= 100_000 && a.alloc < 100_000 + 65_536, "allocata {}", a.alloc);
    assert_eq!(a.links, 1);
    assert_ne!(a.file_id, 0);
    assert_ne!(a.volume_serial, 0);
    assert_eq!(a.modified, m.last_write_time() as i64);
    assert_eq!(a.attributes, m.file_attributes());
    assert!(!a.directory && !a.link && !a.placeholder);

    let dir = p.attributes(&s(&d.path)).unwrap();
    eprintln!("cartella: {dir:?}");
    assert!(dir.directory && !dir.link && !dir.placeholder);
    assert_ne!(dir.attributes & ATTR_DIRECTORY, 0);
    assert_ne!(dir.file_id, a.file_id);
    assert_eq!(dir.volume_serial, a.volume_serial);
    // Con il prefisso `\\?\` è lo stesso file.
    assert_eq!(p.attributes(&format!(r"\\?\{}", s(&f))).unwrap(), a);
}

#[test]
fn hard_links_share_the_identity() {
    let d = TestDir::new("sonda-hardlink");
    let a = d.join("originale.bin");
    let b = d.join("secondo-nome.bin");
    trees::write_file(&a, 50_000, 1);
    fs::hard_link(&a, &b).unwrap();
    let p = probe();
    let (x, y) = (p.attributes(&s(&a)).unwrap(), p.attributes(&s(&b)).unwrap());
    assert_eq!((x.links, y.links), (2, 2));
    assert_eq!(x.file_id, y.file_id);
    assert_eq!(x.volume_serial, y.volume_serial);
    assert_eq!(x.alloc, y.alloc);
}

#[test]
fn junction_is_a_link_and_not_followed() {
    let d = TestDir::new("sonda-junction");
    let target = d.join("destinazione");
    fs::create_dir_all(&target).unwrap();
    trees::write_file(&target.join("dentro.bin"), 10_000, 2);
    let link = d.join("collegamento");
    assert!(trees::junction(&link, &target), "mklink /J");
    let p = probe();
    let j = p.attributes(&s(&link)).unwrap();
    let t = p.attributes(&s(&target)).unwrap();
    eprintln!("junction: {j:?}");
    assert!(j.link && j.directory);
    assert_ne!(j.attributes & ATTR_REPARSE, 0);
    assert_ne!(j.file_id, t.file_id, "gli attributi sono del collegamento, non della destinazione");
    assert!(!t.link);
    // Collegamento simbolico, se la Modalità sviluppatore lo permette.
    let sym = d.join("simbolico");
    if trees::dir_symlink(&sym, &target) {
        assert!(p.attributes(&s(&sym)).unwrap().link);
    } else {
        eprintln!("collegamento simbolico non creato: manca la Modalità sviluppatore");
    }
    // Il collegamento non si attraversa: nessun figlio suo, mentre la destinazione ha i suoi.
    assert_eq!(p.children(&s(&link)), Ok(Vec::new()));
    assert_eq!(p.children(&s(&target)).unwrap(), ["dentro.bin"]);
    // L'elenco della scansione sul reparse point stesso: cosa risponde Windows (per la documentazione).
    eprintln!("attrs::list sulla junction: {:?}", quado_scan_win::attrs::list(&s(&link)));
}

#[test]
fn compressed_sparse_and_small_files() {
    let d = TestDir::new("sonda-allocata");
    let c = d.join("compresso.txt");
    trees::write_compressible(&c, 2_000_000);
    assert!(trees::ntfs_compress(&c), "compact /c");
    let sp = d.join("sparso.bin");
    trees::sparse_file(&sp, 64 << 20);
    let small = d.join("piccolo.txt");
    fs::write(&small, b"pochi byte").unwrap();
    let p = probe();

    let a = p.attributes(&s(&c)).unwrap();
    eprintln!("compresso: logica {}, allocata {}, attributi {:#x}", a.logical, a.alloc, a.attributes);
    assert_eq!(a.logical, 2_000_000);
    assert!(a.alloc < a.logical);
    assert_ne!(a.attributes & ATTR_COMPRESSED, 0);

    let a = p.attributes(&s(&sp)).unwrap();
    eprintln!("sparso: logica {}, allocata {}, attributi {:#x}", a.logical, a.alloc, a.attributes);
    assert_eq!(a.logical, 64 << 20);
    assert!(a.alloc < 1 << 20, "allocata {}", a.alloc);
    assert_ne!(a.attributes & ATTR_SPARSE, 0);

    let a = p.attributes(&s(&small)).unwrap();
    eprintln!("piccolo (residente nella MFT): logica {}, allocata {}", a.logical, a.alloc);
    assert_eq!(a.logical, 10);
    assert!(a.alloc < 4096, "allocata {}", a.alloc);
    assert!(a.resident, "un file da 10 byte sta nella MFT");
    // Compresso e sparse non sono residenti: la loro allocata è in cluster.
    assert!(!p.attributes(&s(&c)).unwrap().resident);
    assert!(!p.attributes(&s(&sp)).unwrap().resident);
}

#[test]
fn children_and_missing_paths() {
    let d = TestDir::new("sonda-figli");
    for n in ["a.txt", "b.bin", "è-accentato.txt"] {
        fs::write(d.join(n), b"x").unwrap();
    }
    fs::create_dir_all(d.join("sotto").join("profondo")).unwrap();
    let p = probe();
    let mut names = p.children(&s(&d.path)).unwrap();
    names.sort();
    assert_eq!(names, ["a.txt", "b.bin", "sotto", "è-accentato.txt"]);
    assert_eq!(p.children(&s(&d.join("sotto"))).unwrap(), ["profondo"]);
    assert_eq!(p.children(&s(&d.join("sotto").join("profondo"))).unwrap(), Vec::<String>::new());

    let missing_file = p.attributes(&s(&d.join("manca.txt")));
    let missing_dir = p.attributes(&s(&d.join("manca").join("x.txt")));
    eprintln!("inesistenti: {missing_file:?} {missing_dir:?}");
    assert!(matches!(missing_file, Err(2 | 3)));
    assert!(matches!(missing_dir, Err(2 | 3)));
    assert!(matches!(p.children(&s(&d.join("manca"))), Err(2 | 3)));
    // Un file non è una cartella da elencare.
    let not_dir = p.children(&s(&d.join("a.txt")));
    eprintln!("figli di un file: {not_dir:?}");
    assert!(not_dir.is_err());
    // Un carattere nullo non tronca il percorso.
    assert_eq!(p.attributes(&format!("{}\0x", s(&d.path))), Err(123));
}

#[test]
fn paths_longer_than_260() {
    let d = TestDir::new("sonda-lungo");
    let long = trees::long_path(&d.path, 300);
    let f = long.join("fondo.bin");
    trees::write_file(&f, 12_345, 4);
    let path = s(&f);
    assert!(path.len() > 260);
    let p = probe();
    let a = p.attributes(&path).unwrap();
    assert_eq!(a.logical, 12_345);
    assert_eq!(p.attributes(&format!(r"\\?\{path}")).unwrap(), a);
    assert_eq!(p.children(&s(&long)).unwrap(), ["fondo.bin"]);
    let vb = p.volume_bin(&path).unwrap();
    assert_eq!(vb.root, r"C:\");
    assert!(p.free_space(&path).unwrap() > 0);
    assert_eq!(p.sync_root(&path), None);
}

/// `WinProbe` che registra i file passati a Restart Manager. Con `fast` falso legge come prima delle letture in parallelo:
/// `entries` e `threads` predefiniti (`children`, poi `attributes` di ogni voce, un thread).
struct Reads<'a> {
    inner: &'a WinProbe,
    fast: bool,
    rm: Mutex<Vec<Vec<String>>>,
}

impl<'a> Reads<'a> {
    fn new(inner: &'a WinProbe, fast: bool) -> Self {
        Self { inner, fast, rm: Mutex::new(Vec::new()) }
    }

    /// Campioni passati a Restart Manager, in un ordine fisso (con più thread le chiamate si mescolano).
    fn samples(self) -> Vec<Vec<String>> {
        let mut v = self.rm.into_inner().unwrap();
        v.sort();
        v
    }
}

impl Probe for Reads<'_> {
    fn attributes(&self, path: &str) -> Result<Attrs, u32> {
        self.inner.attributes(path)
    }
    fn children(&self, dir: &str) -> Result<Vec<String>, u32> {
        self.inner.children(dir)
    }
    fn entries(&self, dir: &str, step: &dyn Fn() -> bool) -> Result<Vec<Entry>, ListError> {
        if self.fast { self.inner.entries(dir, step) } else { entries_one_by_one(self, dir, step) }
    }
    fn threads(&self) -> usize {
        if self.fast { self.inner.threads() } else { 1 }
    }
    fn processes_using(&self, paths: &[String]) -> Result<Vec<ProcessUse>, RmError> {
        self.rm.lock().unwrap().push(paths.to_vec());
        self.inner.processes_using(paths)
    }
    fn references(&self, paths: &[String]) -> ReferenceReport {
        self.inner.references(paths)
    }
    fn volume_bin(&self, path: &str) -> Result<VolumeBin, u32> {
        self.inner.volume_bin(path)
    }
    fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
        self.inner.storage_sense_days()
    }
    fn sync_root(&self, path: &str) -> Option<String> {
        self.inner.sync_root(path)
    }
    fn free_space(&self, path: &str) -> Result<u64, u32> {
        self.inner.free_space(path)
    }
    fn now(&self) -> i64 {
        self.inner.now()
    }
}

/// Albero di prova con i casi della stima (03 §6.3): file normali e vuoti, residente nella MFT, compresso
/// NTFS, sparse, WOF, hard link (due nomi dentro e uno fuori), junction, collegamento simbolico se la
/// Modalità sviluppatore lo permette, sottocartella negata, percorso oltre i 260 caratteri, nomi non ASCII,
/// flusso alternativo. Restituisce la cartella da simulare e quella con il nome fuori.
fn estimate_tree(d: &mut TestDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = d.join("elemento");
    let outside = d.join("fuori");
    fs::create_dir_all(root.join("sotto").join("profondo")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    trees::write_file(&root.join("normale.bin"), 100_000, 1);
    fs::write(root.join("vuoto.txt"), b"").unwrap();
    fs::write(root.join("piccolo.txt"), b"pochi byte").unwrap();
    trees::write_compressible(&root.join("compresso.txt"), 2_000_000);
    assert!(trees::ntfs_compress(&root.join("compresso.txt")), "compact /c");
    trees::sparse_file(&root.join("sotto").join("sparso.bin"), 64 << 20);
    let wof = root.join("sotto").join("wof.exe");
    trees::write_compressible(&wof, 1_500_000);
    if !trees::wof_compress(&wof) {
        eprintln!("compressione WOF non riuscita: il caso WOF non è provato");
    }
    let linked = root.join("sotto").join("con-nomi.bin");
    trees::write_file(&linked, 50_000, 2);
    fs::hard_link(&linked, root.join("sotto").join("profondo").join("secondo-nome.bin")).unwrap();
    fs::hard_link(&linked, outside.join("terzo-nome.bin")).unwrap();
    let inside = root.join("dentro-due.bin");
    trees::write_file(&inside, 30_000, 3);
    fs::hard_link(&inside, root.join("sotto").join("dentro-due-bis.bin")).unwrap();
    let target = outside.join("destinazione");
    fs::create_dir_all(&target).unwrap();
    trees::write_file(&target.join("non-attraversato.bin"), 10_000, 4);
    assert!(trees::junction(&root.join("collegamento"), &target), "mklink /J");
    if !trees::dir_symlink(&root.join("simbolico"), &target) {
        eprintln!("collegamento simbolico non creato: manca la Modalità sviluppatore");
    }
    let denied = root.join("negata");
    trees::write_file(&denied.join("segreto.bin"), 20_000, 5);
    assert!(d.deny_listing(&denied), "icacls /deny");
    let long = trees::long_path(&root.join("lungo"), 300);
    trees::write_file(&long.join("fondo.bin"), 12_345, 6);
    trees::write_file(&root.join("è-accentato ñ 漢字.bin"), 777, 7);
    trees::alternate_stream(&root.join("normale.bin"), "nascosto", 5_000);
    (root, outside)
}

/// Gli attributi delle voci letti da `entries` (un'apertura per l'elenco della cartella) sono quelli
/// delle letture una per una di prima, cartella per cartella, con gli stessi errori.
#[test]
fn entries_match_children_and_attributes() {
    let mut d = TestDir::new("sonda-voci");
    let (root, _) = estimate_tree(&mut d);
    let p = probe();
    let go = || true;
    let mut stack = vec![s(&root)];
    let (mut dirs, mut entries) = (0, 0);
    while let Some(dir) = stack.pop() {
        let new = p.entries(&dir, &go);
        let old = entries_one_by_one(&p, &dir, &go);
        assert_eq!(new, old, "{dir}");
        dirs += 1;
        if let Ok(list) = new {
            entries += list.len();
            stack.extend(list.iter().filter(|(_, a)| matches!(a, Ok(a) if a.directory && !a.link)).map(|(n, _)| format!(r"{dir}\{n}")));
        }
    }
    eprintln!("{dirs} cartelle, {entries} voci");
    assert!(dirs >= 8 && entries >= 20);
    // La cartella negata, un collegamento, un file e un percorso che manca: stessi esiti.
    for (path, want) in [
        (root.join("negata"), Err(ListError::Code(5))),
        (root.join("collegamento"), Ok(Vec::new())),
        (root.join("normale.bin"), Err(ListError::Code(267))),
    ] {
        assert_eq!(p.entries(&s(&path), &go), want, "{}", path.display());
        assert_eq!(entries_one_by_one(&p, &s(&path), &go), want, "{}", path.display());
    }
    assert!(matches!(p.entries(&s(&root.join("manca")), &go), Err(ListError::Code(2 | 3))));
    assert_eq!(p.entries(&format!("{}\0x", s(&root)), &go), Err(ListError::Code(123)));
    // Ci si ferma prima della prima voce.
    assert_eq!(p.entries(&s(&root), &|| false), Err(ListError::Stopped));
}

/// La simulazione con le letture in parallelo (un'apertura per cartella, 8 thread per cartelle e uso dei file)
/// dà la stessa simulazione delle letture di prima: elementi, stima, nomi esclusi, cartelle negate,
/// blocchi e campione di Restart Manager.
#[test]
fn simulation_is_the_same_with_the_new_reads() {
    let mut d = TestDir::new("sonda-simulazione");
    let (root, _) = estimate_tree(&mut d);
    for k in 0..12 {
        trees::write_file(&root.join(format!("cartella-{k}")).join(format!("f{k}.dll")), 1_000 + k, 8);
    }
    let p = probe();
    assert_eq!(p.threads(), 8);
    let items: Vec<String> =
        [root.join("sotto"), root.join("normale.bin"), root.join("negata"), root.clone(), root.join("manca")].iter().map(|x| s(x)).collect();
    let request = || Request {
        items: items.iter().map(|path| RequestItem { path: path.clone(), fixed: None }).collect(),
        record: fake::record(State::Manageable(fake::proof())),
        protected: Vec::new(),
    };
    let before = Reads::new(&p, false);
    let old = simulate(request(), &before, &SimProgress::default()).expect("simulazione di prima");
    let after = Reads::new(&p, true);
    let progress = SimProgress::default();
    let new = simulate(request(), &after, &progress).expect("simulazione nuova");
    assert_eq!(new.items(), old.items());
    let all: Vec<bool> = new.items().iter().map(|_| true).collect();
    assert_eq!(new.estimate(&all), old.estimate(&all));
    assert_eq!(new.default_selection(), old.default_selection());
    eprintln!("stima: {:?}", new.estimate(&all));
    let whole = &new.items()[3];
    assert!(whole.denied_dirs >= 1 && whole.files >= 12, "{whole:?}");
    assert!(whole.blocks.contains(&Block::ContainsLongPath));
    assert_eq!(new.items()[4].blocks.first(), Some(&Block::Missing));
    // Nessun elemento è "in uso" per gli handle della simulazione stessa (elementi uno dentro l'altro).
    let me = std::process::id();
    assert!(!new.items().iter().flat_map(|it| &it.blocks).any(|b| matches!(b, Block::InUse(u) if u.iter().any(|x| x.pid == me))), "{:?}", new.items());
    assert!(progress.done.load(std::sync::atomic::Ordering::Relaxed) > 20);
    // Restart Manager: una chiamata per elemento letto, con lo stesso campione.
    let samples = after.samples();
    assert!(samples.len() >= 3, "{samples:?}");
    assert_eq!(samples, before.samples());
}

#[test]
fn processes_using_finds_this_process() {
    let d = TestDir::new("sonda-processi");
    let f = d.join("tenuto.bin");
    trees::write_file(&f, 1000, 5);
    let held = trees::hold_exclusive(&f);
    let users = probe().processes_using(&[s(&f)]).unwrap();
    eprintln!("processi: {users:?}");
    assert!(users.iter().any(|u| u.pid == std::process::id()));
    drop(held);
}

#[test]
fn config_passes_through() {
    let p = probe();
    assert_eq!(p.storage_sense_days(), Ok(Some(30)));
    assert_eq!(p.references(&["C:\\x".to_owned()]), ReferenceReport::default());
    // Ora: FILETIME vicino a quello del sistema (100 ns dal 1601).
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
    let expected = unix.as_nanos() as i64 / 100 + 116_444_736_000_000_000;
    assert!((p.now() - expected).abs() < 10_000_000 * 60, "{} contro {expected}", p.now());
}

#[test]
fn volume_bin_of_c_is_only_read() {
    // Lettura del Cestino di C: (SHQueryRecycleBinW): nessuna modifica. Le impostazioni non leggibili
    // lasciano la capacità sconosciuta, senza far fallire il resto.
    let d = TestDir::new("sonda-cestino");
    let vb = probe().volume_bin(&s(&d.path)).unwrap();
    eprintln!("Cestino di C: {vb:?}");
    assert_eq!(vb.root, r"C:\");
    assert!(vb.has_bin);
    assert_eq!(vb.capacity, None);
    assert!(!vb.nuke_on_delete);
    assert!(vb.content.is_some());
}

/// Byte liberi del volume con `GetDiskFreeSpaceExW`, chiamata qui senza passare da Quado.
fn total_free(root: &str) -> u64 {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let w: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
    let (mut caller, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: stringa terminata da zero; tre variabili locali.
    let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut caller, &mut total, &mut free) };
    assert_ne!(ok, 0);
    free
}

#[test]
fn free_space_is_the_volume_free_space() {
    let d = TestDir::new("sonda-spazio");
    let f = d.join("x.bin");
    trees::write_file(&f, 10, 1);
    let p = probe();
    // C: cambia di continuo: vale una lettura presa tra due letture uguali.
    for attempt in 0..20 {
        let before = total_free(r"C:\");
        let mine = p.free_space(&s(&f)).unwrap();
        let after = total_free(r"C:\");
        if before == after {
            assert_eq!(mine, before, "tentativo {attempt}");
            return;
        }
    }
    panic!("lo spazio libero di C: cambia troppo in fretta per il confronto");
}

#[test]
fn cloud_placeholders_without_requests() {
    let root = match TestSyncRoot::new("sonda") {
        Ok(r) => r,
        Err(e) => panic!("provider di prova non registrato: {e:?}"),
    };
    root.create(
        "",
        &[
            Placeholder { name: "nel-cloud.bin", size: 5 << 20, dir: false, populated: false },
            Placeholder { name: "cartella-non-popolata", size: 0, dir: true, populated: false },
            Placeholder { name: "cartella-popolata", size: 0, dir: true, populated: true },
        ],
    )
    .unwrap();
    root.create("cartella-popolata", &[Placeholder { name: "dentro.bin", size: 3 << 20, dir: false, populated: false }]).unwrap();
    fs::write(root.path.join("locale.txt"), vec![b'x'; 10_000]).unwrap();
    let p = probe();
    let before = TestSyncRoot::requests();

    let file = s(&root.path.join("nel-cloud.bin"));
    assert_eq!(p.sync_root(&file).as_deref(), Some("QuadoProvaCloud"));
    assert_eq!(p.sync_root(&s(&root.path)).as_deref(), Some("QuadoProvaCloud"));
    assert_eq!(p.sync_root(&s(&root.path.join("locale.txt"))).as_deref(), Some("QuadoProvaCloud"));
    assert_eq!(p.sync_root(&format!(r"\\?\{file}")).as_deref(), Some("QuadoProvaCloud"));
    let outside = TestDir::new("sonda-fuori");
    assert_eq!(p.sync_root(&s(&outside.path)), None);
    assert_eq!(p.sync_root(&s(&outside.join("manca.txt"))), None);

    let a = p.attributes(&file).unwrap();
    eprintln!("segnaposto: {a:?}");
    assert!(a.placeholder && !a.link && !a.directory);
    assert_eq!((a.logical, a.alloc), (5 << 20, 0));
    let inside = p.attributes(&s(&root.path.join("cartella-popolata").join("dentro.bin"))).unwrap();
    assert!(inside.placeholder);
    assert_eq!((inside.logical, inside.alloc), (3 << 20, 0));
    let local = p.attributes(&s(&root.path.join("locale.txt"))).unwrap();
    eprintln!("file locale nella sync root: {local:?}");
    assert_eq!(local.logical, 10_000);
    let np = p.attributes(&s(&root.path.join("cartella-non-popolata"))).unwrap();
    eprintln!("cartella non popolata: {np:?}");
    assert!(np.directory && np.placeholder);
    assert_ne!(np.attributes & ATTR_RECALL_ON_DATA_ACCESS, 0);
    eprintln!("radice della sync root: {:?}", p.attributes(&s(&root.path)));

    // Elenchi con l'handle della scansione: i soli figli presenti sul disco, senza chiedere niente al
    // provider, anche per le cartelle non popolate (la radice compresa).
    let mut names = p.children(&s(&root.path)).unwrap();
    names.sort();
    assert_eq!(names, ["cartella-non-popolata", "cartella-popolata", "locale.txt", "nel-cloud.bin"]);
    assert_eq!(p.children(&s(&root.path.join("cartella-popolata"))).unwrap(), ["dentro.bin"]);
    assert_eq!(p.children(&s(&root.path.join("cartella-non-popolata"))), Ok(Vec::new()));
    // Lo stesso con `entries` della simulazione (un'apertura per l'elenco, poi gli attributi delle voci).
    let go = || true;
    for dir in [root.path.clone(), root.path.join("cartella-popolata"), root.path.join("cartella-non-popolata")] {
        assert_eq!(p.entries(&s(&dir), &go), entries_one_by_one(&p, &s(&dir), &go), "{}", dir.display());
    }
    // Il Cestino del volume e lo spazio libero si leggono senza aprire i segnaposto.
    p.volume_bin(&file).unwrap();
    p.free_space(&file).unwrap();

    assert_eq!(TestSyncRoot::requests(), before, "il provider ha ricevuto richieste di dati o di elenco");
}
