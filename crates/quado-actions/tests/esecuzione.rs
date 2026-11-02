// SPDX-License-Identifier: GPL-3.0-or-later
//! Esecuzione di un piano confermato con letture e spostamenti simulati (`fake`): ricontrollo subito prima,
//! un elemento alla volta, arresto al primo elemento sparito senza arrivare nel Cestino. Nessun file vero.

use std::sync::atomic::{AtomicBool, Ordering};

use quado_actions::execute::{ExecProgress, MoveResult, Mover, Riepilogo, esegui};
use quado_actions::fake::{self, FakeMover, FakeProbe};
use quado_actions::plan::{PianoConfermato, conferma};
use quado_actions::restart::RmError;
use quado_actions::simulate::{FixedReason, Request, RequestItem, SimProgress, simulate};
use quado_model::intervention::{BinCount, ItemOutcome, SkipReason};
use quado_model::record::{Estimate, State};

/// Simula `paths` (con eventuali elementi fissi) e conferma la selezione proposta.
fn plan(probe: &FakeProbe, items: &[(&str, Option<FixedReason>)]) -> PianoConfermato {
    let req = Request {
        items: items.iter().map(|(p, f)| RequestItem { path: (*p).to_owned(), fixed: *f }).collect(),
        record: fake::record(State::Manageable(fake::proof())),
        protected: Vec::new(),
    };
    let sim = simulate(req, probe, &SimProgress::default()).expect("simulazione");
    conferma(&sim, &sim.default_choice()).expect("piano")
}

fn plan_of(probe: &FakeProbe, paths: &[&str]) -> PianoConfermato {
    plan(probe, &paths.iter().map(|p| (*p, None)).collect::<Vec<_>>())
}

fn outcomes(r: &Riepilogo) -> Vec<ItemOutcome> {
    r.items.iter().map(|i| i.outcome).collect()
}

fn world() -> FakeProbe {
    FakeProbe::new()
        .volume("R:\\")
        .file("R:\\a.bin", 1_000, 4_096)
        .file("R:\\b.bin", 2_000, 4_096)
        .file("R:\\c.bin", 3_000, 4_096)
        .file("R:\\d\\x.dll", 100, 4_096)
        .file("R:\\d\\y.dat", 200, 4_096)
        .file("R:\\e.bin", 5_000, 8_192)
}

const ALL: [&str; 5] = ["R:\\a.bin", "R:\\b.bin", "R:\\c.bin", "R:\\d", "R:\\e.bin"];

/// La quota di ogni elemento nel piano è quella della stima: un hard link con un nome fuori dalla
/// selezione non c'è, uno con tutti i nomi nella selezione conta una volta, nel primo elemento (prova di
/// fine su R:, 14/10/2026: senza, il confronto per elemento vedeva 16 MiB di "altri processi").
#[test]
fn plan_items_carry_their_share_of_the_estimate() {
    let p = FakeProbe::new()
        .volume("R:\\")
        .file("R:\\a\\uno.bin", 10_000, 12_288)
        .file("R:\\a\\fuori.bin", 50_000, 53_248)
        .hard_link("R:\\altrove\\fuori.bin", "R:\\a\\fuori.bin")
        .file("R:\\a\\dentro.bin", 20_000, 20_480)
        .hard_link("R:\\b\\dentro.bin", "R:\\a\\dentro.bin")
        .file("R:\\b\\due.bin", 1_000, 4_096);
    let piano = plan_of(&p, &["R:\\a", "R:\\b"]);
    let shares: Vec<u64> = piano.items().iter().map(|i| i.alloc()).collect();
    assert_eq!(shares, vec![12_288 + 20_480, 4_096]);
    assert_eq!(piano.estimate().estimate, Estimate::Exact(shares.iter().sum()));
    assert_eq!(piano.estimate().excluded_names, (1, 53_248));
}

#[test]
fn everything_unchanged_goes_to_the_bin_in_order() {
    let p = world().set_now(fake::NOW + 5);
    let piano = plan_of(&p, &ALL);
    assert_eq!(piano.items().iter().map(|i| i.path()).collect::<Vec<_>>(), ALL);
    assert_eq!(piano.volume(), "R:\\");
    assert_eq!(piano.origin(), "prova");
    let mover = FakeMover::new(&p);
    let progress = ExecProgress::default();
    let r = esegui(piano, &p, &mover, &progress);
    assert_eq!(outcomes(&r), vec![ItemOutcome::Recycled; 5]);
    assert!(!r.stopped);
    assert_eq!((r.started, r.finished), (fake::NOW + 5, fake::NOW + 5));
    assert_eq!(r.volume, "R:\\");
    assert_eq!(r.origin, "prova");
    assert_eq!(r.estimate.estimate, Estimate::Exact(5 * 4_096 + 8_192));
    assert_eq!(r.items[3].alloc, 8_192);
    assert!(r.items[3].directory);
    assert_eq!(progress.done.load(Ordering::Relaxed), 5);
    assert_eq!(mover.calls(), ALL);
    for path in ALL {
        assert!(!p.exists(path));
    }
    // Il Cestino conta la logica; lo spazio libero cambia solo svuotandolo.
    let bin = || quado_actions::probe::Probe::volume_bin(&p, "R:\\").expect("cestino").content;
    let free = || quado_actions::probe::Probe::free_space(&p, "R:\\").expect("spazio");
    assert_eq!(bin(), Some(BinCount { items: 5, bytes: 1_000 + 2_000 + 3_000 + 300 + 5_000 }));
    assert_eq!(free(), fake::FREE);
    p.empty_bin("R:\\");
    assert_eq!(bin(), Some(BinCount::default()));
    assert_eq!(free(), fake::FREE + 5 * 4_096 + 8_192);
}

#[test]
fn items_changed_after_the_simulation_are_skipped() {
    let p = world();
    let piano = plan_of(&p, &ALL);
    // Dimensione, data, identità (file sostituito), una cartella con un file in più.
    p.set_attrs("R:\\a.bin", |a| a.logical += 1)
        .set_attrs("R:\\b.bin", |a| a.modified += 1)
        .set_attrs("R:\\c.bin", |a| a.file_id += 1)
        .file("R:\\d\\nuovo.txt", 0, 0);
    let mover = FakeMover::new(&p);
    let r = esegui(piano, &p, &mover, &ExecProgress::default());
    let changed = ItemOutcome::Skipped(SkipReason::Changed);
    assert_eq!(outcomes(&r), vec![changed, changed, changed, changed, ItemOutcome::Recycled]);
    assert!(!r.stopped);
    assert_eq!(mover.calls(), ["R:\\e.bin"]);
    assert!(p.exists("R:\\a.bin") && p.exists("R:\\d\\nuovo.txt"));
}

#[test]
fn folders_whose_content_changed_size_or_kind_are_skipped() {
    let p = world().file("R:\\f\\z.bin", 10, 4_096);
    let piano = plan_of(&p, &["R:\\d", "R:\\f", "R:\\e.bin"]);
    // Un file della cartella cambia dimensione; un file diventa una cartella.
    p.set_attrs("R:\\d\\y.dat", |a| a.logical = 1);
    p.remove("R:\\e.bin").dir("R:\\e.bin");
    // Cartella con la stessa identità e la stessa data ma senza un file.
    p.remove("R:\\f\\z.bin");
    let mover = FakeMover::new(&p);
    let r = esegui(piano, &p, &mover, &ExecProgress::default());
    assert_eq!(outcomes(&r), vec![ItemOutcome::Skipped(SkipReason::Changed); 3]);
    assert!(mover.calls().is_empty());
}

#[test]
fn a_folder_with_a_path_that_became_long_is_skipped() {
    // Una sottocartella rinominata con un nome lungo: stessi file, stessa logica, stessa data della
    // cartella; ma la shell eliminerebbe per sempre il file dal percorso lungo.
    let p = world().file("R:\\h\\sotto\\z.bin", 10, 4_096);
    let piano = plan_of(&p, &["R:\\h", "R:\\a.bin"]);
    let long = format!("R:\\h\\{}", "n".repeat(250));
    p.remove("R:\\h\\sotto").file(&format!("{long}\\z.bin"), 10, 4_096);
    let mover = FakeMover::new(&p);
    let r = esegui(piano, &p, &mover, &ExecProgress::default());
    assert_eq!(outcomes(&r), vec![ItemOutcome::Skipped(SkipReason::Changed), ItemOutcome::Recycled]);
    assert_eq!(mover.calls(), ["R:\\a.bin"]);
}

#[test]
fn missing_or_unreadable_items_are_skipped() {
    let p = world();
    let piano = plan_of(&p, &["R:\\a.bin", "R:\\b.bin", "R:\\c.bin"]);
    p.remove("R:\\a.bin").attributes_error("R:\\b.bin", 5);
    let mover = FakeMover::new(&p);
    let r = esegui(piano, &p, &mover, &ExecProgress::default());
    assert_eq!(outcomes(&r), vec![ItemOutcome::Skipped(SkipReason::Missing), ItemOutcome::Skipped(SkipReason::Unreadable(5)), ItemOutcome::Recycled]);
    assert_eq!(mover.calls(), ["R:\\c.bin"]);
}

#[test]
fn folders_in_use_at_the_recheck_are_skipped_with_the_same_sample() {
    let p = world().file("R:\\g\\z.bin", 10, 4_096);
    let piano = plan_of(&p, &["R:\\d", "R:\\g", "R:\\a.bin"]);
    let at_simulation = p.rm_calls();
    assert_eq!(at_simulation.len(), 3);
    // Processo aperto dopo la simulazione su un file della cartella; Restart Manager che non risponde
    // sull'altra. Per i file singoli il ricontrollo non ripete Restart Manager: se il file è aperto,
    // fallisce lo spostamento.
    p.in_use("R:\\d\\x.dll", 99, "Processo di prova").rm_error("R:\\g\\z.bin", RmError::List(1223)).in_use("R:\\a.bin", 98, "Altro");
    let mover = FakeMover::new(&p);
    let r = esegui(piano, &p, &mover, &ExecProgress::default());
    assert_eq!(outcomes(&r)[..2], [ItemOutcome::Skipped(SkipReason::InUse), ItemOutcome::Skipped(SkipReason::Unreadable(1223))]);
    let calls = p.rm_calls();
    assert_eq!(calls.len(), 5, "una chiamata per cartella al ricontrollo, nessuna per i file");
    assert_eq!(calls[3], at_simulation[0]);
    assert_eq!(calls[4], at_simulation[1]);
    assert_eq!(mover.calls(), ["R:\\a.bin"]);
}

#[test]
fn a_failed_item_still_in_place_lets_the_others_continue() {
    let p = world();
    let piano = plan_of(&p, &["R:\\a.bin", "R:\\b.bin", "R:\\c.bin"]);
    // File aperti dopo il controllo: Windows non li sposta e restano al loro posto.
    let hr = 0x8007_0020_u32 as i32;
    let mover = FakeMover::new(&p).force("R:\\a.bin", MoveResult::Failed(hr), false).force("R:\\b.bin", MoveResult::Failed(hr), false);
    let r = esegui(piano, &p, &mover, &ExecProgress::default());
    assert_eq!(outcomes(&r), vec![ItemOutcome::Failed(hr), ItemOutcome::Failed(hr), ItemOutcome::Recycled]);
    assert!(!r.stopped);
    assert!(p.exists("R:\\a.bin") && p.exists("R:\\b.bin") && !p.exists("R:\\c.bin"));
}

#[test]
fn an_operation_finished_without_the_bin_always_stops_the_plan() {
    // Windows ha finito senza un elemento nel Cestino ma l'elemento è ancora lì: stato inatteso, il
    // piano si ferma lo stesso.
    let p = world();
    let piano = plan_of(&p, &["R:\\a.bin", "R:\\b.bin", "R:\\c.bin"]);
    let mover = FakeMover::new(&p).force("R:\\b.bin", MoveResult::NotRecycled(1), false);
    let r = esegui(piano, &p, &mover, &ExecProgress::default());
    assert_eq!(outcomes(&r), vec![ItemOutcome::Recycled, ItemOutcome::Failed(1), ItemOutcome::NotReached]);
    assert!(r.stopped);
    assert_eq!(mover.calls(), ["R:\\a.bin", "R:\\b.bin"]);
    assert!(p.exists("R:\\b.bin") && p.exists("R:\\c.bin"));
}

#[test]
fn an_item_gone_without_reaching_the_bin_stops_the_plan() {
    let p = world();
    let piano = plan_of(&p, &["R:\\a.bin", "R:\\b.bin", "R:\\c.bin", "R:\\e.bin"]);
    let mover = FakeMover::new(&p).force("R:\\b.bin", MoveResult::NotRecycled(0), true);
    let progress = ExecProgress::default();
    let r = esegui(piano, &p, &mover, &progress);
    assert_eq!(outcomes(&r), vec![ItemOutcome::Recycled, ItemOutcome::NotInRecycleBin(0), ItemOutcome::NotReached, ItemOutcome::NotReached]);
    assert!(r.stopped);
    assert_eq!(mover.calls(), ["R:\\a.bin", "R:\\b.bin"]);
    assert_eq!(progress.done.load(Ordering::Relaxed), 2);
    assert!(p.exists("R:\\c.bin") && p.exists("R:\\e.bin"));
    // Anche un errore con l'elemento sparito ferma il piano.
    let q = world();
    let piano = plan_of(&q, &["R:\\a.bin", "R:\\b.bin"]);
    let mover = FakeMover::new(&q).force("R:\\a.bin", MoveResult::Failed(-1), true);
    let r = esegui(piano, &q, &mover, &ExecProgress::default());
    assert_eq!(outcomes(&r), vec![ItemOutcome::NotInRecycleBin(-1), ItemOutcome::NotReached]);
}

#[test]
fn windows_deleting_for_good_stops_the_plan() {
    // `NukeOnDelete` acceso dopo la simulazione: Windows elimina il primo elemento senza Cestino (come
    // misurato su un VHDX di prova, `docs/m1/cestino-verifiche.md`) e il piano si ferma lì.
    let p = world();
    let piano = plan_of(&p, &["R:\\a.bin", "R:\\b.bin"]);
    p.bin("R:\\", |b| b.nuke_on_delete = true);
    let mover = FakeMover::new(&p);
    let r = esegui(piano, &p, &mover, &ExecProgress::default());
    assert_eq!(outcomes(&r), vec![ItemOutcome::NotInRecycleBin(0), ItemOutcome::NotReached]);
    assert!(r.stopped);
    assert!(p.exists("R:\\b.bin"));
}

/// Mover che dopo la prima chiamata chiede l'annullamento, come l'utente durante l'esecuzione.
struct CancelAfterFirst<'a> {
    inner: FakeMover,
    cancel: &'a AtomicBool,
}

impl Mover for CancelAfterFirst<'_> {
    fn recycle(&self, path: &str) -> MoveResult {
        self.cancel.store(true, Ordering::Relaxed);
        self.inner.recycle(path)
    }
}

#[test]
fn cancelling_leaves_the_rest_not_reached() {
    let p = world();
    let piano = plan_of(&p, &["R:\\a.bin", "R:\\b.bin", "R:\\c.bin"]);
    let progress = ExecProgress::default();
    progress.cancel.store(true, Ordering::Relaxed);
    let mover = FakeMover::new(&p);
    let r = esegui(piano, &p, &mover, &progress);
    assert_eq!(outcomes(&r), vec![ItemOutcome::NotReached; 3]);
    assert!(r.stopped);
    assert!(mover.calls().is_empty());
    assert_eq!(progress.done.load(Ordering::Relaxed), 0);
    // Annullamento durante il primo elemento: il primo si conclude, gli altri no.
    let piano = plan_of(&p, &["R:\\a.bin", "R:\\b.bin", "R:\\c.bin"]);
    let progress = ExecProgress::default();
    let mover = CancelAfterFirst { inner: FakeMover::new(&p), cancel: &progress.cancel };
    let r = esegui(piano, &p, &mover, &progress);
    assert_eq!(outcomes(&r), vec![ItemOutcome::Recycled, ItemOutcome::NotReached, ItemOutcome::NotReached]);
    assert!(r.stopped);
    assert_eq!(mover.inner.calls(), ["R:\\a.bin"]);
}

#[test]
fn unreadable_after_a_failure_stops_for_prudence() {
    /// Windows non sposta e subito dopo l'elemento non si legge più: non si sa se c'è ancora.
    struct FailThenHide(FakeProbe);
    impl Mover for FailThenHide {
        fn recycle(&self, path: &str) -> MoveResult {
            self.0.attributes_error(path, 5);
            MoveResult::Failed(-2)
        }
    }
    let p = world();
    let piano = plan_of(&p, &["R:\\a.bin", "R:\\b.bin"]);
    let r = esegui(piano, &p, &FailThenHide(p.clone()), &ExecProgress::default());
    assert_eq!(outcomes(&r), vec![ItemOutcome::Failed(-2), ItemOutcome::NotReached]);
    assert!(r.stopped);
}

#[test]
fn the_mover_receives_only_the_paths_of_the_plan() {
    let p = world()
        .file("R:\\bloccato\\app.exe", 1, 4_096)
        .in_use("R:\\bloccato\\app.exe", 1, "app")
        .file("R:\\fisso\\v.bin", 1, 4_096)
        .file("R:\\d\\sotto\\z.bin", 1, 4_096);
    let items = [
        ("R:\\a.bin", None),
        ("R:\\bloccato", None),
        ("R:\\fisso", Some(FixedReason::KeptCopy)),
        ("R:\\d", None),
        ("R:\\manca.bin", None),
        ("R:\\e.bin", None),
    ];
    let piano = plan(&p, &items);
    assert_eq!(piano.items().iter().map(|i| i.path()).collect::<Vec<_>>(), ["R:\\a.bin", "R:\\d", "R:\\e.bin"]);
    let mover = FakeMover::new(&p);
    let r = esegui(piano, &p, &mover, &ExecProgress::default());
    assert_eq!(outcomes(&r), vec![ItemOutcome::Recycled; 3]);
    assert_eq!(mover.calls(), ["R:\\a.bin", "R:\\d", "R:\\e.bin"]);
    // Il contenuto delle cartelle si sposta con la cartella, senza chiamate per i singoli file.
    assert!(!p.exists("R:\\d\\sotto\\z.bin"));
    assert!(p.exists("R:\\bloccato\\app.exe") && p.exists("R:\\fisso\\v.bin") && p.exists("R:\\b.bin"));
}
