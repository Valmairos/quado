// SPDX-License-Identifier: GPL-3.0-or-later
//! Simulazione con letture simulate (`fake`): stima (allocate, residenti nella MFT, file con più nomi,
//! cartelle negate), campione di Restart Manager, cartelle di sincronizzazione, Sensore memoria,
//! annullamento, elementi fissi. Nessun file vero.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use quado_actions::fake::{self, FakeProbe};
use quado_actions::plan::{Block, ManualCheck, Rifiuto, Simulazione, check, conferma};
use quado_actions::probe::{Attrs, Probe, VolumeBin};
use quado_actions::restart::{ProcessUse, RmError};
use quado_actions::simulate::{Cancelled, FixedReason, Request, RequestItem, SimProgress, simulate};
use quado_actions::sync::SyncKind;
use quado_model::intervention::{ReferenceKind, ReferenceReport};
use quado_model::record::{CheckOutcome, Estimate, Record, State};

fn record() -> Record {
    fake::record(State::Manageable(fake::proof()))
}

fn request(paths: &[&str], record: Record) -> Request {
    Request { items: paths.iter().map(|p| RequestItem { path: (*p).to_owned(), fixed: None }).collect(), record, protected: Vec::new() }
}

fn run(probe: &FakeProbe, paths: &[&str]) -> Simulazione {
    simulate(request(paths, record()), probe, &SimProgress::default()).expect("non annullata")
}

fn exact(sim: &Simulazione, selected: &[bool]) -> u64 {
    match sim.estimate(selected).estimate {
        Estimate::Exact(n) => n,
        e => panic!("stima non esatta: {e:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Stima

#[test]
fn ordinary_files_give_an_exact_estimate_of_allocated_bytes() {
    let p = FakeProbe::new()
        .volume("R:\\")
        .file("R:\\a\\uno.bin", 10_000, 12_288)
        .file("R:\\a\\sotto\\due.bin", 5_000, 8_192)
        // Compresso: conta l'allocata, non la logica.
        .file("R:\\a\\compresso.log", 1_000_000, 65_536)
        .file("R:\\b.bin", 100, 4_096);
    let sim = run(&p, &["R:\\a", "R:\\b.bin"]);
    let a = &sim.items()[0];
    assert!(a.directory);
    assert_eq!((a.files, a.dirs, a.denied_dirs), (3, 1, 0));
    assert_eq!(a.logical, 1_015_000);
    assert_eq!(a.alloc, 12_288 + 8_192 + 65_536);
    let b = &sim.items()[1];
    assert!(!b.directory);
    assert_eq!((b.files, b.alloc, b.logical), (1, 4_096, 100));
    let all = [true, true];
    let e = sim.estimate(&all);
    assert_eq!(e.estimate, Estimate::Exact(12_288 + 8_192 + 65_536 + 4_096));
    assert_eq!(e.excluded_names, (0, 0));
    assert_eq!(e.denied_dirs, 0);
    assert_eq!(e.logical, 1_015_100);
    assert_eq!(sim.default_selection(), vec![true, true]);
}

#[test]
fn a_file_whose_names_are_all_selected_counts_once() {
    // Due nomi nello stesso elemento e un terzo in un altro elemento della selezione.
    let p = FakeProbe::new()
        .volume("R:\\")
        .file("R:\\a\\x.bin", 50_000, 53_248)
        .hard_link("R:\\a\\y.bin", "R:\\a\\x.bin")
        .hard_link("R:\\b\\z.bin", "R:\\a\\x.bin")
        .file("R:\\b\\w.bin", 10, 4_096);
    assert_eq!(p.attributes("R:\\a\\x.bin").expect("x").links, 3);
    let sim = run(&p, &["R:\\a", "R:\\b"]);
    // Nell'elemento il file conta una volta, anche con due nomi.
    assert_eq!(sim.items()[0].alloc, 53_248);
    assert_eq!(sim.items()[1].alloc, 53_248 + 4_096);
    assert_eq!(exact(&sim, &[true, true]), 53_248 + 4_096);
    assert_eq!(sim.estimate(&[true, true]).excluded_names, (0, 0));
}

#[test]
fn a_file_with_a_name_outside_the_selection_is_excluded_and_cited() {
    let p = FakeProbe::new()
        .volume("R:\\")
        .file("R:\\a\\x.bin", 50_000, 53_248)
        .hard_link("R:\\fuori\\x.bin", "R:\\a\\x.bin")
        .file("R:\\a\\y.bin", 10, 4_096);
    let sim = run(&p, &["R:\\a"]);
    let e = sim.estimate(&[true]);
    assert_eq!(e.estimate, Estimate::Exact(4_096));
    assert_eq!(e.excluded_names, (1, 53_248));
}

#[test]
fn removing_an_item_from_the_selection_recomputes_the_estimate() {
    let p = FakeProbe::new()
        .volume("R:\\")
        .file("R:\\a\\x.bin", 50_000, 53_248)
        .hard_link("R:\\b\\x.bin", "R:\\a\\x.bin")
        .file("R:\\a\\y.bin", 10, 4_096)
        .file("R:\\b\\z.bin", 10, 8_192)
        .file("R:\\c.bin", 10, 16_384);
    let sim = run(&p, &["R:\\a", "R:\\b", "R:\\c.bin"]);
    assert_eq!(exact(&sim, &[true, true, true]), 53_248 + 4_096 + 8_192 + 16_384);
    // Senza `b` il file con due nomi ha un nome fuori: escluso e citato.
    let e = sim.estimate(&[true, false, true]);
    assert_eq!(e.estimate, Estimate::Exact(4_096 + 16_384));
    assert_eq!(e.excluded_names, (1, 53_248));
    assert_eq!(e.logical, 50_000 + 10 + 10);
    assert_eq!(exact(&sim, &[false, false, true]), 16_384);
    // Selezione vuota: esatta, zero.
    let none = sim.estimate(&[false, false, false]);
    assert_eq!(none.estimate, Estimate::Exact(0));
    assert_eq!((none.logical, none.excluded_names, none.denied_dirs), (0, (0, 0), 0));
}

#[test]
fn a_denied_folder_turns_the_estimate_into_a_range() {
    let p = FakeProbe::new()
        .volume("R:\\")
        .file("R:\\a\\x.bin", 10, 4_096)
        // Un file con un altro nome fuori: potrebbe averlo nella cartella negata.
        .file("R:\\a\\h.bin", 50_000, 53_248)
        .hard_link("R:\\altrove\\h.bin", "R:\\a\\h.bin")
        .file("R:\\a\\negata\\segreto.bin", 1 << 20, 1 << 20)
        .denied("R:\\a\\negata")
        .file("R:\\b\\y.bin", 10, 8_192)
        .denied("R:\\b\\vuota");
    let sim = run(&p, &["R:\\a", "R:\\b"]);
    assert_eq!(sim.items()[0].denied_dirs, 1);
    // Il contenuto della cartella negata non si legge e non entra nella stima.
    assert!(!p.reads().iter().any(|r| r.ends_with("segreto.bin")));
    let e = sim.estimate(&[true, false]);
    assert_eq!(e.estimate, Estimate::Range { min: 4_096, max: 4_096 + 53_248 });
    assert_eq!(e.denied_dirs, 1);
    assert_eq!(e.excluded_names, (0, 0));
    // Solo una cartella negata, senza file con altri nomi: intervallo con estremi uguali.
    assert_eq!(sim.estimate(&[false, true]).estimate, Estimate::Range { min: 8_192, max: 8_192 });
    assert_eq!(sim.estimate(&[true, true]).denied_dirs, 2);
    // Una cartella di cui non si leggono nemmeno gli attributi conta come negata.
    let q = FakeProbe::new().volume("R:\\").file("R:\\c\\x.bin", 10, 4_096).dir("R:\\c\\strana").attributes_error("R:\\c\\strana", 5);
    let sim = run(&q, &["R:\\c"]);
    assert_eq!(sim.items()[0].denied_dirs, 1);
    assert_eq!(sim.estimate(&[true]).estimate, Estimate::Range { min: 4_096, max: 4_096 });
}

#[test]
fn placeholders_count_local_bytes_and_resident_files_count_zero() {
    let p = FakeProbe::new()
        .volume("R:\\")
        // Segnaposto da 1 GiB con 64 KiB locali.
        .placeholder("R:\\cloud\\video.mp4", 1 << 30, 65_536)
        // Segnaposto senza byte locali.
        .placeholder("R:\\cloud\\foto.jpg", 4 << 20, 0)
        // Residente nella MFT: logica 500, Windows riporta 504 byte allocati.
        .resident("R:\\cloud\\nota.txt", 500);
    let sim = run(&p, &["R:\\cloud"]);
    let e = sim.estimate(&[true]);
    assert_eq!(e.estimate, Estimate::Exact(65_536));
    // La capacità del Cestino usa la logica.
    assert_eq!(e.logical, (1 << 30) + (4 << 20) + 500);
}

#[test]
fn resident_files_count_zero_whatever_windows_reports() {
    // La MFT non si restringe. Windows riporta 16 byte allocati per un file da 10.
    let p = FakeProbe::new().volume("R:\\").resident("R:\\a\\piccolo.txt", 10).file("R:\\a\\grande.bin", 10_000, 12_288).resident("R:\\b.ini", 200);
    assert_eq!(p.attributes("R:\\a\\piccolo.txt").expect("piccolo").alloc, 16);
    let sim = run(&p, &["R:\\a", "R:\\b.ini"]);
    assert_eq!(sim.items()[0].alloc, 12_288);
    assert_eq!(sim.items()[1].alloc, 0);
    assert_eq!(sim.items()[1].logical, 200);
    assert_eq!(exact(&sim, &[true, true]), 12_288);
    assert_eq!(exact(&sim, &[false, true]), 0);
    // Anche con più nomi: il file conta 0 una volta.
    let q = FakeProbe::new().volume("R:\\").resident("R:\\c\\x.txt", 10).hard_link("R:\\c\\y.txt", "R:\\c\\x.txt");
    assert_eq!(exact(&run(&q, &["R:\\c"]), &[true]), 0);
}

#[test]
fn files_without_identity_count_by_themselves() {
    // exFAT: niente identità, anche con un numero di nomi strano.
    let a = Attrs { logical: 10, alloc: 32_768, links: 2, file_id: 0, volume_serial: 7, ..Attrs::default() };
    let p = FakeProbe::new().volume("R:\\").entry("R:\\a\\x.bin", a).entry("R:\\a\\y.bin", a);
    let sim = run(&p, &["R:\\a"]);
    assert_eq!(exact(&sim, &[true]), 65_536);
}

#[test]
fn links_count_by_themselves_and_are_not_crossed() {
    let p = FakeProbe::new()
        .volume("R:\\")
        .file("R:\\a\\x.bin", 10, 4_096)
        .link("R:\\a\\giunzione", true)
        .file("R:\\a\\giunzione\\bersaglio.bin", 1 << 30, 1 << 30)
        .link("R:\\a\\simbolico.txt", false);
    let sim = run(&p, &["R:\\a"]);
    let a = &sim.items()[0];
    assert_eq!((a.files, a.dirs), (2, 1));
    assert_eq!(exact(&sim, &[true]), 4_096);
    assert!(!p.reads().iter().any(|r| r.contains("bersaglio")), "il collegamento è stato attraversato");
    // Il collegamento non entra nel campione di Restart Manager.
    assert_eq!(p.rm_calls(), vec![vec!["R:\\a\\x.bin".to_owned()]]);
}

// ---------------------------------------------------------------------------------------------
// Restart Manager

#[test]
fn large_folders_register_every_executable_and_the_1000_most_recent_files() {
    let mut p = FakeProbe::new().volume("R:\\");
    let mut want = Vec::new();
    for k in 0..1_500i64 {
        let path = format!("R:\\grande\\dati{}\\f{k}.dat", k % 7);
        p = p.file(&path, 10, 4_096).set_attrs(&path, |a| a.modified = fake::NOW - 1_000_000 + k);
        if k >= 500 {
            want.push(path);
        }
    }
    for k in 0..30 {
        let path = format!("R:\\grande\\bin\\prog{k}.exe");
        // Eseguibili vecchi: entrano lo stesso.
        p = p.file(&path, 10, 4_096).set_attrs(&path, |a| a.modified = 1);
        want.push(path);
    }
    for k in 0..20 {
        let path = format!("R:\\grande\\bin\\lib{k}.DLL");
        p = p.file(&path, 10, 4_096).set_attrs(&path, |a| a.modified = 1);
        want.push(path);
    }
    let sim = run(&p, &["R:\\grande"]);
    assert!(sim.items()[0].blocks.is_empty());
    let calls = p.rm_calls();
    assert_eq!(calls.len(), 1, "una chiamata per elemento");
    let mut got = calls[0].clone();
    assert_eq!(got.len(), 1_050);
    got.sort();
    want.sort();
    assert_eq!(got, want);
}

#[test]
fn small_folders_register_every_file_and_single_files_themselves() {
    let p = FakeProbe::new()
        .volume("R:\\")
        .file("R:\\piccola\\a.txt", 1, 4_096)
        .file("R:\\piccola\\b\\c.txt", 1, 4_096)
        .file("R:\\solo.crdownload", 1, 4_096)
        .dir("R:\\vuota");
    run(&p, &["R:\\piccola", "R:\\solo.crdownload", "R:\\vuota"]);
    let mut calls = p.rm_calls();
    calls[0].sort();
    assert_eq!(calls, vec![vec!["R:\\piccola\\a.txt".to_owned(), "R:\\piccola\\b\\c.txt".to_owned()], vec!["R:\\solo.crdownload".to_owned()], vec![]]);
}

#[test]
fn processes_and_restart_manager_errors_block_the_item() {
    let p = FakeProbe::new()
        .volume("R:\\")
        .file("R:\\a\\app.exe", 1, 4_096)
        .in_use("R:\\a\\app.exe", 4242, "App di prova")
        .file("R:\\b\\x.bin", 1, 4_096)
        .rm_error("R:\\b\\x.bin", RmError::Register(29))
        .file("R:\\c\\y.bin", 1, 4_096);
    let sim = run(&p, &["R:\\a", "R:\\b", "R:\\c"]);
    let use_ = ProcessUse { pid: 4242, app_name: "App di prova".into(), service: None };
    assert_eq!(sim.items()[0].blocks, vec![Block::InUse(vec![use_])]);
    assert_eq!(sim.items()[1].blocks, vec![Block::RestartManager(RmError::Register(29))]);
    assert!(sim.items()[2].blocks.is_empty());
    assert_eq!(sim.default_selection(), vec![false, false, true]);
}

// ---------------------------------------------------------------------------------------------
// Controlli che non dipendono dal contenuto

#[test]
fn folders_synced_without_windows_get_the_check_with_their_kind() {
    // Cartelle che Windows non conosce (Dropbox dalla configurazione, Syncthing dal marcatore): stessa spunta,
    // con il tipo che decide il testo. La cartella registrata in Windows vince.
    let p = FakeProbe::new()
        .volume("R:\\")
        .synced_by(r"R:\Dropbox", "Dropbox", SyncKind::Cloud)
        .synced_by(r"R:\Sync", "Syncthing", SyncKind::Devices)
        .synced_by(r"R:\Doppia", "Syncthing", SyncKind::Devices)
        .synced(r"R:\Doppia", "OneDrive")
        .file(r"R:\Dropbox\a.bin", 1, 4_096)
        .file(r"R:\Sync\b.bin", 1, 4_096)
        .file(r"R:\Doppia\c.bin", 1, 4_096)
        .file(r"R:\locale.bin", 1, 4_096);
    let sim = run(&p, &[r"R:\Dropbox\a.bin", r"R:\Sync\b.bin", r"R:\Doppia\c.bin", r"R:\locale.bin"]);
    let manual: Vec<_> = sim.items().iter().map(|it| it.manual.clone()).collect();
    assert_eq!(manual[0], vec![ManualCheck::Synced { provider: "Dropbox".into(), kind: SyncKind::Cloud }]);
    assert_eq!(manual[1], vec![ManualCheck::Synced { provider: "Syncthing".into(), kind: SyncKind::Devices }]);
    assert_eq!(manual[2], vec![ManualCheck::Synced { provider: "OneDrive".into(), kind: SyncKind::Registered }]);
    assert!(manual[3].is_empty());
    // Senza le spunte il piano non si costruisce.
    let scelta = sim.default_choice();
    assert_eq!(check(&sim, &scelta), vec![Rifiuto::ItemNotTicked(0, 0), Rifiuto::ItemNotTicked(1, 0), Rifiuto::ItemNotTicked(2, 0)]);
}

#[test]
fn synced_items_get_a_manual_check_and_need_the_tick() {
    let p = FakeProbe::new()
        .volume("R:\\")
        .synced("R:\\OneDrive", "OneDrive")
        .placeholder("R:\\OneDrive\\Documenti\\tesi.docx", 1 << 20, 0)
        .file("R:\\locale.bin", 1, 4_096);
    let sim = run(&p, &["R:\\onedrive\\documenti\\tesi.docx", "R:\\locale.bin"]);
    assert_eq!(sim.items()[0].manual, vec![ManualCheck::Synced { provider: "OneDrive".into(), kind: SyncKind::Registered }]);
    assert!(sim.items()[1].manual.is_empty());
    // Il controllo manuale non toglie l'elemento dalla selezione proposta: si spunta.
    assert_eq!(sim.default_selection(), vec![true, true]);
    let mut scelta = sim.default_choice();
    assert_eq!(check(&sim, &scelta), vec![Rifiuto::ItemNotTicked(0, 0)]);
    scelta.ticked_items[0][0] = true;
    assert!(check(&sim, &scelta).is_empty());
    assert!(conferma(&sim, &scelta).is_ok());
}

#[test]
fn storage_sense_days_are_reported() {
    let p = FakeProbe::new().volume("R:\\").file("R:\\a.bin", 1, 4_096).storage_sense(Ok(Some(30)));
    assert_eq!(run(&p, &["R:\\a.bin"]).storage_sense_days(), Some(30));
    p.storage_sense(Ok(None));
    assert_eq!(run(&p, &["R:\\a.bin"]).storage_sense_days(), None);
    // Una lettura non riuscita vale come "non lo svuota": la conferma non annuncia giorni inventati.
    p.storage_sense(Err(5));
    assert_eq!(run(&p, &["R:\\a.bin"]).storage_sense_days(), None);
}

#[test]
fn volume_bin_and_time_are_those_read_at_simulation() {
    let p = FakeProbe::new().volume("R:\\").file("R:\\a.bin", 1, 4_096).set_now(fake::NOW + 77);
    let sim = run(&p, &["R:\\a.bin"]);
    assert_eq!(sim.checked_at(), fake::NOW + 77);
    let want = VolumeBin { root: "R:\\".into(), has_bin: true, nuke_on_delete: false, capacity: Some(fake::CAPACITY), content: Some(Default::default()) };
    assert_eq!(sim.volume(), &want);
    assert_eq!(sim.references(), &ReferenceReport::default());
    assert!(p.reference_calls().is_empty(), "riferimenti non chiesti dalla regola");
}

#[test]
fn rule_checks_take_the_outcome_of_the_simulation() {
    let mut r = record();
    r.checks = vec![
        fake::automatic("controllo-restart-manager"),
        fake::automatic("controllo-riferimenti"),
        fake::automatic("controllo-capacita-cestino"),
        fake::automatic("controllo-lockfile"),
        fake::manual("regola-prova-controllo-a-mano"),
    ];
    let p = FakeProbe::new()
        .volume("R:\\")
        .file("R:\\a\\x.exe", 1, 4_096)
        .in_use("R:\\a\\x.exe", 7, "x")
        .file("R:\\b\\y.bin", 1, 4_096)
        .reference(ReferenceKind::RunUser, "HKCU\\...\\Run\\y", "R:\\b\\y.bin");
    let sim = simulate(request(&["R:\\a", "R:\\b", "R:\\c.bin"], r.clone()), &p, &SimProgress::default()).expect("simulazione");
    assert_eq!(sim.rule_manual(), ["regola-prova-controllo-a-mano"]);
    let outcomes: Vec<CheckOutcome> = sim.rule_checks().iter().map(|c| c.outcome).collect();
    assert_eq!(outcomes, [CheckOutcome::Failed, CheckOutcome::Failed, CheckOutcome::Passed, CheckOutcome::AtSimulation, CheckOutcome::ToTick]);
    assert_eq!(sim.items()[2].blocks, vec![Block::Missing]);
    // Senza processi né riferimenti, e con un Cestino pieno: solo la capacità non passa.
    let q = FakeProbe::new().volume("R:\\").file("R:\\a\\x.exe", 1, 4_096).bin("R:\\", |b| b.content = None);
    let sim = simulate(request(&["R:\\a"], r), &q, &SimProgress::default()).expect("simulazione");
    let outcomes: Vec<CheckOutcome> = sim.rule_checks().iter().take(3).map(|c| c.outcome).collect();
    assert_eq!(outcomes, [CheckOutcome::Passed, CheckOutcome::Passed, CheckOutcome::Failed]);
}

// ---------------------------------------------------------------------------------------------
// Avanzamento, annullamento, elementi fissi

#[test]
fn progress_counts_every_entry_read() {
    let p = FakeProbe::new().volume("R:\\").file("R:\\a\\x.bin", 1, 1).file("R:\\a\\b\\y.bin", 1, 1).file("R:\\z.bin", 1, 1);
    let progress = SimProgress::default();
    simulate(request(&["R:\\a", "R:\\z.bin"], record()), &p, &progress).expect("simulazione");
    // a, x.bin, b, y.bin, z.bin.
    assert_eq!(progress.done.load(Ordering::Relaxed), 5);
}

/// Letture simulate che chiedono l'annullamento dopo `limit` attributi letti, come farebbe l'utente a
/// metà simulazione.
struct CancelAfter<'a> {
    inner: FakeProbe,
    reads: AtomicU64,
    limit: u64,
    cancel: &'a AtomicBool,
}

impl Probe for CancelAfter<'_> {
    fn attributes(&self, path: &str) -> Result<Attrs, u32> {
        if self.reads.fetch_add(1, Ordering::Relaxed) + 1 >= self.limit {
            self.cancel.store(true, Ordering::Relaxed);
        }
        self.inner.attributes(path)
    }
    fn children(&self, dir: &str) -> Result<Vec<String>, u32> {
        self.inner.children(dir)
    }
    fn processes_using(&self, paths: &[String]) -> Result<Vec<ProcessUse>, RmError> {
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

#[test]
fn cancelling_stops_the_simulation() {
    let mut p = FakeProbe::new().volume("R:\\");
    for k in 0..200 {
        p = p.file(&format!("R:\\a\\f{k}.bin"), 1, 4_096);
    }
    // Annullata prima di cominciare: nessuna lettura.
    let progress = SimProgress::default();
    progress.cancel.store(true, Ordering::Relaxed);
    assert_eq!(simulate(request(&["R:\\a"], record()), &p, &progress).err(), Some(Cancelled));
    assert!(p.reads().is_empty());
    // Annullata a metà della visita: si ferma subito, senza Restart Manager.
    let progress = SimProgress::default();
    let probe = CancelAfter { inner: p.clone(), reads: AtomicU64::new(0), limit: 50, cancel: &progress.cancel };
    assert_eq!(simulate(request(&["R:\\a"], record()), &probe, &progress).err(), Some(Cancelled));
    assert_eq!(probe.reads.load(Ordering::Relaxed), 50);
    assert!(p.rm_calls().is_empty());
}

#[test]
fn fixed_items_are_shown_but_never_read() {
    let p = FakeProbe::new()
        .volume("R:\\")
        .file("R:\\CapCut\\Apps\\9.1.0\\CapCut.exe", 10, 4_096)
        .file("R:\\CapCut\\Apps\\9.3.0\\CapCut.exe", 20, 8_192)
        .file("R:\\CapCut\\Apps\\9.4.0\\CapCut.exe", 30, 16_384);
    let mut r = record();
    r.checks = vec![fake::automatic("controllo-riferimenti")];
    let req = Request {
        items: vec![
            RequestItem { path: "R:\\CapCut\\Apps\\9.1.0".into(), fixed: None },
            RequestItem { path: "R:\\CapCut\\Apps\\9.3.0".into(), fixed: Some(FixedReason::KeptVersion) },
            RequestItem { path: "R:\\CapCut\\Apps\\9.4.0".into(), fixed: Some(FixedReason::InUseVersion) },
        ],
        record: r,
        protected: Vec::new(),
    };
    let sim = simulate(req, &p, &SimProgress::default()).expect("simulazione");
    for it in &sim.items()[1..] {
        assert!(it.fixed.is_some());
        assert!(it.blocks.is_empty() && it.manual.is_empty());
        assert_eq!((it.files, it.dirs, it.logical, it.alloc, it.denied_dirs, it.directory), (0, 0, 0, 0, 0, false));
    }
    let touched = |s: &String| s.contains("9.3.0") || s.contains("9.4.0");
    assert!(!p.reads().iter().any(touched));
    assert!(!p.rm_calls().iter().flatten().any(touched));
    assert!(!p.reference_calls().iter().flatten().any(touched));
    assert_eq!(p.reference_calls(), vec![vec!["R:\\CapCut\\Apps\\9.1.0".to_owned()]]);
    assert_eq!(sim.default_selection(), vec![true, false, false]);
    // Nemmeno scelti a forza entrano nella stima o nella capacità; la conferma li rifiuta.
    assert_eq!(exact(&sim, &[true, true, true]), 4_096);
    assert_eq!(sim.estimate(&[true, true, true]).logical, 10);
    let mut scelta = sim.default_choice();
    scelta.selected = vec![true, true, false];
    assert_eq!(check(&sim, &scelta), vec![Rifiuto::Fixed(1)]);
}

// ---------------------------------------------------------------------------------------------
// Più thread (`Probe::threads`, come `WinProbe`)

/// Le letture simulate con `threads` thread: cartelle lette in parallelo e Restart Manager su più
/// elementi insieme, come fa la simulazione con `WinProbe`. Con `cancel_on_rm` ogni chiamata di Restart
/// Manager chiede l'annullamento.
struct Parallel<'a> {
    inner: &'a FakeProbe,
    threads: usize,
    cancel_on_rm: Option<&'a AtomicBool>,
}

impl Probe for Parallel<'_> {
    fn attributes(&self, path: &str) -> Result<Attrs, u32> {
        self.inner.attributes(path)
    }
    fn children(&self, dir: &str) -> Result<Vec<String>, u32> {
        self.inner.children(dir)
    }
    fn threads(&self) -> usize {
        self.threads
    }
    fn processes_using(&self, paths: &[String]) -> Result<Vec<ProcessUse>, RmError> {
        if let Some(c) = self.cancel_on_rm {
            c.store(true, Ordering::Relaxed);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
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

/// `CancelAfter` con 8 thread.
struct ParallelCancel<'a>(CancelAfter<'a>);

impl Probe for ParallelCancel<'_> {
    fn attributes(&self, path: &str) -> Result<Attrs, u32> {
        self.0.attributes(path)
    }
    fn children(&self, dir: &str) -> Result<Vec<String>, u32> {
        self.0.children(dir)
    }
    fn threads(&self) -> usize {
        8
    }
    fn processes_using(&self, paths: &[String]) -> Result<Vec<ProcessUse>, RmError> {
        self.0.processes_using(paths)
    }
    fn references(&self, paths: &[String]) -> ReferenceReport {
        self.0.references(paths)
    }
    fn volume_bin(&self, path: &str) -> Result<VolumeBin, u32> {
        self.0.volume_bin(path)
    }
    fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
        self.0.storage_sense_days()
    }
    fn sync_root(&self, path: &str) -> Option<String> {
        self.0.sync_root(path)
    }
    fn free_space(&self, path: &str) -> Result<u64, u32> {
        self.0.free_space(path)
    }
    fn now(&self) -> i64 {
        self.0.now()
    }
}

/// Albero con molte cartelle e i casi della stima: hard link dentro e fuori, cartella negata, voce non
/// leggibile, residente, segnaposto, collegamento, percorso lungo, e 1.300 file con la stessa data (il
/// campione di Restart Manager ne prende 1.000: quali, lo decide l'ordine della visita).
fn wide_tree() -> (FakeProbe, Vec<String>) {
    let mut p = FakeProbe::new().volume("R:\\");
    for k in 0..1_300u64 {
        p = p.file(&format!("R:\\a\\d{}\\e{}\\f{k}.dat", k % 13, k % 5), 10 + k, 4_096);
    }
    for k in 0..40 {
        p = p.file(&format!("R:\\a\\bin{}\\p{k}.exe", k % 4), 10, 8_192);
    }
    let long = format!("R:\\b\\{}\\x.txt", "n".repeat(260));
    p = p
        .file("R:\\b\\uno.bin", 100, 4_096)
        .hard_link("R:\\b\\sotto\\due.bin", "R:\\b\\uno.bin")
        .hard_link("R:\\fuori\\tre.bin", "R:\\b\\uno.bin")
        .file("R:\\b\\dentro.bin", 100, 8_192)
        .hard_link("R:\\a\\d1\\dentro-bis.bin", "R:\\b\\dentro.bin")
        .resident("R:\\b\\piccolo.txt", 10)
        .placeholder("R:\\b\\cloud.bin", 1 << 20, 0)
        .link("R:\\b\\giunzione", true)
        .file("R:\\b\\giunzione\\bersaglio.bin", 1 << 30, 1 << 30)
        .file("R:\\b\\negata\\segreto.bin", 1, 4_096)
        .denied("R:\\b\\negata")
        .file("R:\\b\\illeggibile.bin", 1, 4_096)
        .attributes_error("R:\\b\\illeggibile.bin", 5)
        .file(&long, 1, 4_096)
        .file("R:\\c.bin", 5, 4_096);
    (p, vec!["R:\\a".into(), "R:\\b".into(), "R:\\manca".into(), "R:\\c.bin".into(), "R:\\a\\d1".into()])
}

#[test]
fn threads_give_the_same_simulation_and_sample() {
    let (p, items) = wide_tree();
    let paths: Vec<&str> = items.iter().map(String::as_str).collect();
    let mut seen = Vec::new();
    for threads in [1, 2, 8] {
        let before = p.rm_calls().len();
        let progress = SimProgress::default();
        let probe = Parallel { inner: &p, threads, cancel_on_rm: None };
        let sim = simulate(request(&paths, record()), &probe, &progress).expect("non annullata");
        let mut calls = p.rm_calls()[before..].to_vec();
        calls.sort();
        let all: Vec<bool> = sim.items().iter().map(|_| true).collect();
        seen.push((sim.items().to_vec(), sim.estimate(&all), calls, progress.done.load(Ordering::Relaxed)));
    }
    assert_eq!(seen[0], seen[1], "2 thread");
    assert_eq!(seen[0], seen[2], "8 thread");
    let (items, _, calls, done) = &seen[2];
    assert_eq!(items[0].files, 1_341);
    assert_eq!(items[1].denied_dirs, 2);
    assert!(items[1].blocks.contains(&Block::ContainsLongPath));
    assert_eq!(items[2].blocks, [Block::Missing]);
    // Una chiamata per elemento letto; quella di `R:\a`: 40 eseguibili più 1.000 altri file.
    assert_eq!(calls.len(), 4);
    assert!(calls.iter().any(|c| c.len() == 1_040));
    assert!(*done > 1_400);
}

#[test]
fn cancelling_with_threads_stops_the_simulation() {
    let (p, items) = wide_tree();
    let paths: Vec<&str> = items.iter().map(String::as_str).collect();
    // A metà della lettura delle cartelle: nessuna chiamata a Restart Manager.
    let progress = SimProgress::default();
    let probe = ParallelCancel(CancelAfter { inner: p.clone(), reads: AtomicU64::new(0), limit: 300, cancel: &progress.cancel });
    let before = p.rm_calls().len();
    assert_eq!(simulate(request(&paths, record()), &probe, &progress).err(), Some(Cancelled));
    assert_eq!(p.rm_calls().len(), before);
    assert!(probe.0.reads.load(Ordering::Relaxed) < 1_000);
    // Durante Restart Manager, con più elementi che thread: gli elementi non ancora chiesti restano senza
    // risposta e la simulazione è annullata.
    let mut many = FakeProbe::new().volume("R:\\");
    let mut names = Vec::new();
    for k in 0..24 {
        many = many.file(&format!("R:\\v{k}\\a.exe"), 1, 4_096);
        names.push(format!("R:\\v{k}"));
    }
    let progress = SimProgress::default();
    let probe = Parallel { inner: &many, threads: 4, cancel_on_rm: Some(&progress.cancel) };
    let paths: Vec<&str> = names.iter().map(String::as_str).collect();
    assert_eq!(simulate(request(&paths, record()), &probe, &progress).err(), Some(Cancelled));
    assert!(many.rm_calls().len() < 24, "{} chiamate", many.rm_calls().len());
}
