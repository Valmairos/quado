// SPDX-License-Identifier: GPL-3.0-or-later
//! Procedure e comandi, vista Interventi e Dati e privacy (M5, agente E2) provati con `egui_kittest`
//! (06 §5.6, §5.7, §6.2; 03 F-AC-1, F-AC-6, F-VE-1, F-VE-2, F-VE-5).
//!
//! Nessun test usa `Actions::live`: letture, misure e Cestino passano da `FakeProbe`, il lanciatore è
//! finto, la riscansione è finta (nessuna cartella vera si legge), il registro sta in cartelle temporanee.
//! Quado non crea processi: lo controlla il test d'architettura di `quado-actions` (`processi.rs`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use quado_actions::fake::{FakeProbe, NOW};
use quado_actions::journal;
use quado_actions::launch::Launch;
use quado_actions::legacy;
use quado_actions::shell::ShellError;
use quado_model::intervention::{
    BinCount, InterventionKind, ItemOutcome, JournalEntry, JournalItem, Measure, MeasurePoint, SkipReason,
};
use quado_model::record::{Estimate, NotEstimable};
use quado_model::verification::{self, ExplainFacts};
use quado_model::{Flags, NodeData, Tree};
use quado_scan_win::ScanResult;
use quado_ui::app::intervention_jobs::{self, Actions};
use quado_ui::app::procedure_flow::Subject;
use quado_ui::app::{App, Env, Screen, interventions_view, settings, synthetic};
use quado_ui::tokens::ThemeChoice;

const GIB: u64 = 1 << 30;
const MIB: u64 = 1 << 20;
const ROOT: &str = "C:\\";
const BIN: &str = r"C:\$Recycle.Bin\S-1-5-21-1-2-3-1001";
const NPM: &str = r"C:\Users\demo\AppData\Local\npm-cache";
/// Un minuto in `FILETIME`.
const MINUTE: i64 = 600_000_000;

/// Cartella temporanea del test, tolta alla fine.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let p = std::env::temp_dir().join(format!("quado-interventi-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("cartella temporanea");
        Self(p)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Riscansione finta: ogni cartella contiene ora un solo file da 4 KB. Nessuna cartella vera si legge.
fn fake_rescan(path: &str) -> Result<Box<ScanResult>, u32> {
    let mut t = Tree::with_root(&w(path), 0);
    t.push(Tree::ROOT, &w("resto.bin"), NodeData { logical: 4096, alloc: 4096, mtime: NOW, attrs: 0x20, flags: Flags::default() });
    t.aggregate();
    t.seal();
    Ok(Box::new(ScanResult { tree: t, links: Default::default(), problems: Vec::new(), stats: Default::default(), volume: None, balance: None, cancelled: false }))
}

/// Chiamate alla riscansione del test sul file di ibernazione (che non deve riscansionare).
static ROOT_RESCANS: AtomicUsize = AtomicUsize::new(0);

fn counting_rescan(path: &str) -> Result<Box<ScanResult>, u32> {
    ROOT_RESCANS.fetch_add(1, Ordering::SeqCst);
    fake_rescan(path)
}

/// Lanciatore finto del test sull'apertura del Cestino: registra l'URI e quante letture la sonda aveva già
/// ricevuto, poi cambia lo spazio libero. Se la misura di partenza venisse dopo l'apertura, vedrebbe il
/// valore nuovo. Lo usa un solo test (gira nel thread della misura, non in quello del test).
static LAUNCHED: Mutex<Vec<(String, usize)>> = Mutex::new(Vec::new());
static LAUNCH_PROBE: Mutex<Option<FakeProbe>> = Mutex::new(None);

fn recording_launcher(l: &Launch) -> Result<(), ShellError> {
    let reads = match LAUNCH_PROBE.lock().expect("sonda").as_ref() {
        Some(p) => {
            let n = p.reads().len();
            p.set_free(ROOT, 101 * GIB);
            n
        }
        None => 0,
    };
    LAUNCHED.lock().expect("chiamate").push((l.uri(), reads));
    Ok(())
}

/// Lanciatore della vista Programmi: non apre niente.
fn programs_launcher(_: &Launch) -> Result<(), ShellError> {
    Ok(())
}

/// Volume `C:\` finto con 100 GB liberi e 3 elementi (2 GB) nel Cestino secondo Windows.
fn probe() -> FakeProbe {
    FakeProbe::new().volume(ROOT).set_free(ROOT, 100 * GIB).bin(ROOT, |b| b.content = Some(BinCount { items: 3, bytes: 2 * GIB }))
}

fn app(probe: &FakeProbe, data_dir: Option<&Path>) -> App {
    let mut env = Env::fixed();
    env.actions = Actions::fake(probe);
    env.data_dir = data_dir.map(Path::to_path_buf);
    let mut a = App::new(env, Vec::new());
    a.env.actions.rescan = std::sync::Arc::new(fake_rescan);
    let (v, r) = synthetic::sample_c(false);
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    a
}

/// Finestra alta: l'ispettore intero è visibile senza scorrere.
fn harness(a: App) -> Harness<'static, App> {
    let mut h = Harness::builder().with_size([1600.0, 2600.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(4);
    h
}

/// Aspetta misure, riscansioni e letture del Cestino, poi qualche frame.
fn settle(h: &mut Harness<'_, App>) {
    for _ in 0..2000 {
        h.step();
        if !h.state().procedures.busy() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(3);
}

/// Seleziona in Esplora il nodo con questo percorso: l'ispettore mostra il suo record.
fn select(h: &mut Harness<'_, App>, path: &str) {
    {
        let a = h.state_mut();
        a.go(Screen::Explore);
        let d = a.data.as_ref().expect("dati");
        let node = intervention_jobs::find_node(d.tree(), path).unwrap_or_else(|| panic!("{path}"));
        let parent = d.tree().parent(node);
        a.explore.open(d, parent, Some(node));
    }
    h.run_steps(4);
    // Le misure dell'ispettore arrivano in background e allungano il pannello.
    h.state_mut().wait_background();
    settle(h);
}

fn t(h: &Harness<'_, App>, key: &str) -> String {
    h.state().lingue.t(key)
}

fn click(h: &mut Harness<'_, App>, label: &str) {
    h.query_all_by_label(label).next().unwrap_or_else(|| panic!("manca «{label}»")).click();
    h.step();
    h.run_steps(2);
}

/// Clic sul pulsante con il testo di una chiave.
fn click_t(h: &mut Harness<'_, App>, key: &str) {
    let label = t(h, key);
    click(h, &label);
}

fn press(h: &mut Harness<'_, App>, key: Key, modifiers: Modifiers) {
    h.input_mut().events.push(Event::ModifiersChanged(modifiers));
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
    h.step();
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.step();
}

/// "Variazione dello spazio libero: +3,00 GB" come la scrive l'interfaccia.
fn change_text(h: &Harness<'_, App>, bytes: i64) -> String {
    let a = h.state();
    let v = interventions_view::signed(&a.lingue, &a.fmt, bytes);
    format!("Variazione dello spazio libero: {v}.")
}

fn has_text(h: &Harness<'_, App>, s: &str) -> bool {
    h.query_all_by_label_contains(s).next().is_some()
}

/// Un nodo con esattamente questo testo (anche più d'uno).
fn has_label(h: &Harness<'_, App>, s: &str) -> bool {
    h.query_all_by_label(s).next().is_some()
}

#[test]
fn procedure_measures_before_opening_and_done_writes_the_journal() {
    // F-AC-1, F-VE-1, F-VE-2: "Apri il Cestino" prende la misura di partenza e poi chiama il lanciatore;
    // "Ho eseguito la procedura" misura, riscansiona, confronta e scrive il registro.
    let dir = TempDir::new("apri");
    let p = probe();
    *LAUNCH_PROBE.lock().expect("sonda") = Some(p.clone());
    let mut a = app(&p, Some(dir.path()));
    a.env.actions.launcher = recording_launcher;
    let mut h = harness(a);
    select(&mut h, BIN);
    // Pulsanti del record del Cestino: la pagina che si apre, lo svuotamento, "Ho eseguito".
    assert!(h.query_by_label(&t(&h, "procedura-apri-cestino")).is_some());
    assert!(h.query_by_label(&t(&h, "procedura-svuota-cestino")).is_some());
    assert!(has_text(&h, &t(&h, "procedura-senza-partenza")), "senza partenza lo dice");
    let reads_before = p.reads().len();
    click_t(&mut h, "procedura-apri-cestino");
    settle(&mut h);
    let launched = LAUNCHED.lock().expect("chiamate").clone();
    assert_eq!(launched.len(), 1, "{launched:?}");
    assert_eq!(launched[0].0, Launch::RecycleBin.uri());
    assert!(launched[0].1 >= reads_before + 2, "spazio libero e Cestino letti prima di aprire: {launched:?}");
    // Il lanciatore finto cambia lo spazio libero: la misura di partenza ha il valore di prima.
    let subject = Subject::Rule("cestino".into());
    let start = h.state().procedures.start_measure(&subject).expect("procedura aperta").expect("misura di partenza");
    assert_eq!((start.point, start.free, start.bin), (MeasurePoint::Start, 100 * GIB, Some(BinCount { items: 3, bytes: 2 * GIB })));
    assert!(has_text(&h, "Pagina aperta alle"));

    // L'utente svuota il Cestino dalla pagina di Windows.
    p.set_free(ROOT, 103 * GIB).set_now(NOW + MINUTE);
    click_t(&mut h, "azione-ho-eseguito");
    settle(&mut h);
    let report = h.state().procedures.report(&subject).expect("esito");
    assert_eq!(report.obtained(), Some(3 * GIB as i64));
    assert_eq!(report.still_present, Some(4096), "riscansione finta: un file da 4 KB");
    assert_eq!(report.journal, Some(Ok(())));
    assert!(has_text(&h, &change_text(&h, 3 * GIB as i64)), "variazione vicino ai pulsanti");
    assert!(has_text(&h, "altri programmi hanno scritto o eliminato"));

    let j = journal::read(dir.path()).expect("registro");
    assert_eq!((j.entries.len(), j.unreadable_lines), (1, 0));
    let e = &j.entries[0];
    assert_eq!(e.kind, InterventionKind::Procedure("cestino-apri".into()));
    assert_eq!((e.origin.as_str(), e.method.as_str(), e.volume.as_str()), ("cestino", "shell:RecycleBinFolder", ROOT));
    assert_eq!(e.estimate, Estimate::NotEstimable(NotEstimable::ExternalProcedure));
    assert_eq!(e.id, start.at);
    let points: Vec<(MeasurePoint, u64)> = e.measures.iter().map(|m| (m.point, m.free)).collect();
    assert_eq!(points, vec![(MeasurePoint::Start, 100 * GIB), (MeasurePoint::AfterProcedure, 103 * GIB)]);
    assert_eq!(e.measures[1].still_present, Some(4096));
    let c = e.comparison.as_ref().expect("confronto");
    assert_eq!((c.obtained, c.difference), (3 * GIB as i64, None), "stima non stimabile: solo l'ottenuto");
    // Il modello è aggiornato con la riscansione (F-VE-4).
    let d = h.state().data.as_ref().expect("dati");
    let bin = intervention_jobs::find_node(d.tree(), BIN).expect("Cestino");
    assert_eq!(d.tree().alloc(bin), 4096);
}

#[test]
fn copying_a_command_takes_the_start_measure_and_runs_nothing() {
    // F-AC-1: il comando si copia negli appunti, non si esegue; la copia prende la misura di partenza.
    let dir = TempDir::new("comando");
    let p = probe();
    let mut h = harness(app(&p, Some(dir.path())));
    select(&mut h, NPM);
    assert!(has_text(&h, &t(&h, "procedura-comando-nota")));
    let copy = t(&h, "record-copia");
    h.query_all_by_label(&copy).next().expect("Copia").click();
    h.step();
    let copied: Vec<String> = h
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(copied, vec!["npm cache verify".to_owned()]);
    settle(&mut h);
    let subject = Subject::Rule("npm-cache".into());
    let start = h.state().procedures.start_measure(&subject).expect("comando copiato").expect("misura");
    assert_eq!(start.free, 100 * GIB);
    assert!(has_text(&h, "Comando copiato alle"));

    p.set_free(ROOT, 100 * GIB + 500 * MIB);
    click_t(&mut h, "azione-ho-eseguito");
    settle(&mut h);
    assert_eq!(h.state().procedures.report(&subject).and_then(|r| r.obtained()), Some(500 * MIB as i64));
    let j = journal::read(dir.path()).expect("registro");
    assert_eq!(j.entries.len(), 1);
    let e = &j.entries[0];
    assert_eq!(e.kind, InterventionKind::Command("npm-cache".into()));
    assert_eq!(e.method, "npm cache verify");
    assert_eq!(e.measures.len(), 2);
    assert!(e.items.is_empty(), "Quado non tocca elementi");
}

#[test]
fn done_without_start_measure_records_only_the_measure() {
    // Senza misura di partenza la variazione non si calcola: si registra la sola misura e lo si dice. Il
    // file di ibernazione sta nella radice: niente riscansione a metà, serve «Scansiona di nuovo».
    let dir = TempDir::new("senza-partenza");
    let p = probe();
    let mut a = app(&p, Some(dir.path()));
    a.env.actions.rescan = std::sync::Arc::new(counting_rescan);
    let mut h = harness(a);
    select(&mut h, r"C:\hiberfil.sys");
    click_t(&mut h, "azione-ho-eseguito");
    settle(&mut h);
    let report = h.state().procedures.report(&Subject::Rule("ibernazione".into())).expect("esito");
    assert_eq!((report.obtained(), report.full_scan, report.still_present), (None, true, None));
    assert_eq!(ROOT_RESCANS.load(Ordering::SeqCst), 0, "nessuna riscansione a metà");
    assert!(has_text(&h, "Senza la misura di partenza la variazione non si calcola"));
    assert!(has_text(&h, &t(&h, "procedura-serve-riesamina")));
    let j = journal::read(dir.path()).expect("registro");
    assert_eq!(j.entries.len(), 1);
    let e = &j.entries[0];
    assert_eq!(e.kind, InterventionKind::Command("ibernazione".into()));
    assert_eq!((e.measures.len(), e.measures[0].point, e.comparison.is_none()), (1, MeasurePoint::AfterProcedure, true));
}

#[test]
fn empty_bin_says_the_windows_count_first_then_measures() {
    // F-AC-1, 04 §6.7: "Svuota il Cestino…" preceduto dal numero e dai GB secondo Windows; il dialogo di
    // Windows (finto) svuota, poi misura dopo lo svuotamento e registro.
    let dir = TempDir::new("svuota");
    let p = probe();
    let mut h = harness(app(&p, Some(dir.path())));
    select(&mut h, BIN);
    let count = {
        let a = h.state();
        a.lingue.ta("procedura-cestino-conteggio", &[("n", 3u64.into()), ("testo", a.fmt.count(3).into()), ("dimensione", a.fmt.size(2 * GIB).into())])
    };
    assert!(count.contains("3 elementi") && count.contains("2,00 GB"), "{count}");
    let text = h.query_by_label(&count).expect("conteggio di Windows").rect();
    let button = h.get_by_label(&t(&h, "procedura-svuota-cestino")).rect();
    assert!(text.min.y < button.min.y, "il conteggio viene prima del pulsante");
    click_t(&mut h, "procedura-svuota-cestino");
    settle(&mut h);
    let report = h.state().procedures.report(&Subject::Rule("cestino".into())).expect("esito");
    assert_eq!(report.obtained(), Some(2 * GIB as i64));
    let j = journal::read(dir.path()).expect("registro");
    assert_eq!(j.entries.len(), 1);
    let e = &j.entries[0];
    assert_eq!(e.kind, InterventionKind::Procedure("cestino-svuota".into()));
    let m: Vec<(MeasurePoint, u64, Option<BinCount>)> = e.measures.iter().map(|m| (m.point, m.free, m.bin)).collect();
    assert_eq!(
        m,
        vec![(MeasurePoint::Start, 100 * GIB, Some(BinCount { items: 3, bytes: 2 * GIB })), (MeasurePoint::AfterEmpty, 102 * GIB, Some(BinCount::default()))]
    );
}

#[test]
fn uninstall_dialog_measures_before_opening_and_after_done() {
    // F-AP-5, F-VE-1, F-VE-2: misura di partenza al pulsante che apre Impostazioni, poi "Ho eseguito la
    // procedura" nel dialogo, con l'esito nel dialogo e il registro.
    let dir = TempDir::new("disinstalla");
    let p = probe();
    let mut a = app(&p, Some(dir.path()));
    a.programs.launcher = programs_launcher;
    let mut h = harness(a);
    press(&mut h, Key::Num6, Modifiers::COMMAND);
    let name = "7-Zip 24.09 (x64 edition)";
    let i = h.state().data.as_ref().expect("dati").programs.items.iter().position(|x| x.name == name).expect("7-Zip");
    h.state_mut().programs.selected = Some(i);
    h.run_steps(3);
    click_t(&mut h, "programmi-disinstalla");
    click_t(&mut h, "programmi-dialogo-apri-impostazioni");
    let subject = Subject::Program(name.into());
    let start = h.state().procedures.start_measure(&subject).expect("procedura aperta").expect("misura");
    assert_eq!(start.free, 100 * GIB);
    p.set_free(ROOT, 100 * GIB + 5 * MIB);
    click_t(&mut h, "azione-ho-eseguito");
    settle(&mut h);
    h.run_steps(3);
    let dlg = h.state().programs.dialog.clone().expect("il dialogo resta aperto dopo la riscansione");
    assert_eq!(dlg.program, i);
    assert!(has_text(&h, &change_text(&h, 5 * MIB as i64)), "esito nel dialogo");
    let j = journal::read(dir.path()).expect("registro");
    assert_eq!(j.entries.len(), 1);
    let e = &j.entries[0];
    assert_eq!(e.kind, InterventionKind::Procedure("disinstallazione".into()));
    assert_eq!((e.origin.as_str(), e.method.as_str()), ("programma", "ms-settings:appsfeatures"));
    assert_eq!(e.measures.iter().map(|m| m.free).collect::<Vec<_>>(), vec![100 * GIB, 100 * GIB + 5 * MIB]);
}

fn measure(point: MeasurePoint, at: i64, free: u64, bin: Option<BinCount>, still: Option<u64>) -> Measure {
    Measure { point, at, free, bin, still_present: still }
}

/// Registro di prova con quattro interventi diversi.
fn fixture() -> Vec<JournalEntry> {
    let t1 = NOW + 10 * MINUTE;
    let recycle_measures = vec![
        measure(MeasurePoint::Start, t1, 100 * GIB, Some(BinCount { items: 1, bytes: GIB }), None),
        measure(MeasurePoint::AfterEmpty, t1 + MINUTE, 109 * GIB, Some(BinCount::default()), Some(0)),
    ];
    let facts = ExplainFacts { skipped_or_failed: (1, 2 * GIB), recycled_bytes: 8 * GIB, ..ExplainFacts::default() };
    let recycle = JournalEntry {
        id: t1,
        volume: ROOT.into(),
        kind: InterventionKind::Recycle,
        origin: "capcut-versioni".into(),
        method: "cestino".into(),
        estimate: Estimate::Exact(10 * GIB),
        items: vec![
            JournalItem { path: r"C:\a".into(), bytes: 4 * GIB, outcome: ItemOutcome::Recycled },
            JournalItem { path: r"C:\b".into(), bytes: 4 * GIB, outcome: ItemOutcome::Recycled },
            JournalItem { path: r"C:\c".into(), bytes: 2 * GIB, outcome: ItemOutcome::Skipped(SkipReason::InUse) },
        ],
        comparison: verification::compare(&Estimate::Exact(10 * GIB), &recycle_measures, &facts),
        measures: recycle_measures,
    };
    let t2 = NOW + 20 * MINUTE;
    let empty_measures = vec![measure(MeasurePoint::Start, t2, 50 * GIB, None, None), measure(MeasurePoint::AfterEmpty, t2 + MINUTE, 52 * GIB, None, Some(4096))];
    let empty = JournalEntry {
        id: t2,
        volume: ROOT.into(),
        kind: InterventionKind::Procedure("cestino-svuota".into()),
        origin: "cestino".into(),
        method: "cestino-svuota".into(),
        estimate: Estimate::NotEstimable(NotEstimable::ExternalProcedure),
        items: Vec::new(),
        comparison: verification::compare(&Estimate::NotEstimable(NotEstimable::ExternalProcedure), &empty_measures, &ExplainFacts::default()),
        measures: empty_measures,
    };
    let t3 = NOW + 30 * MINUTE;
    let command = JournalEntry {
        id: t3,
        volume: ROOT.into(),
        kind: InterventionKind::Command("npm-cache".into()),
        origin: "npm-cache".into(),
        method: "npm cache verify".into(),
        estimate: Estimate::NotEstimable(NotEstimable::ExternalProcedure),
        items: Vec::new(),
        measures: vec![measure(MeasurePoint::AfterProcedure, t3, 70 * GIB, None, None)],
        comparison: None,
    };
    let t4 = NOW + 40 * MINUTE;
    let uninstall_measures = vec![measure(MeasurePoint::Start, t4, 60 * GIB, None, None), measure(MeasurePoint::AfterProcedure, t4 + MINUTE, 60 * GIB + 490 * MIB, None, Some(0))];
    let uninstall = JournalEntry {
        id: t4,
        volume: ROOT.into(),
        kind: InterventionKind::Procedure("disinstallazione".into()),
        origin: "programma".into(),
        method: "ms-settings:appsfeatures".into(),
        estimate: Estimate::Exact(500 * MIB),
        items: Vec::new(),
        comparison: verification::compare(&Estimate::Exact(500 * MIB), &uninstall_measures, &ExplainFacts::default()),
        measures: uninstall_measures,
    };
    vec![recycle, empty, command, uninstall]
}

#[test]
fn history_shows_one_entry_per_journal_line_with_the_same_values() {
    // F-VE-5, 06 §6.2: con un registro di N interventi la vista mostra N voci con gli stessi valori del
    // file; le righe illeggibili si dichiarano; la barra laterale mostra il numero.
    let dir = TempDir::new("storico");
    let entries = fixture();
    for e in &entries {
        journal::append(dir.path(), e).expect("registro di prova");
    }
    // Una riga scritta a metà.
    let mut f = std::fs::OpenOptions::new().append(true).open(journal::path(dir.path())).expect("registro");
    std::io::Write::write_all(&mut f, b"{\"versione\":1,\"id\":").expect("riga troncata");
    drop(f);
    let mut h = harness(app(&probe(), Some(dir.path())));
    assert!(h.query_by_label(&format!("{} 4", t(&h, "nav-interventi"))).is_some(), "il numero nella barra laterale");
    press(&mut h, Key::Num7, Modifiers::COMMAND);
    assert_eq!(h.state().screen, Screen::Interventions);
    h.run_steps(2);

    let n = entries.len();
    // Il testo di un'etichetta sta nel valore del nodo di accessibilità, quello di un pulsante nel nome.
    let titles = h
        .query_all_by_label_contains(" alle ")
        .filter(|x| {
            let a = x.accesskit_node();
            a.value().or_else(|| a.label()).is_some_and(|s| s.contains(" · "))
        })
        .count();
    assert_eq!(titles, n, "una voce per intervento");
    for e in &entries {
        let lines = {
            let a = h.state();
            interventions_view::entry_lines(&a.lingue, &a.fmt, e)
        };
        for (s, _) in &lines {
            assert!(has_label(&h, s), "manca «{s}»");
        }
    }
    // Valori scritti a mano dal file di prova.
    for s in [
        "Spostamento nel Cestino · Versioni precedenti di CapCut",
        "Elementi: 3 (2 nel Cestino, 1 saltato)",
        "Metodo: cestino · unità C:\\",
        "Stima esatta: 10,0 GB",
        "Stimato: 10,0 GB. Variazione dello spazio libero: +9,00 GB.",
        "Differenza: -1,00 GB (10,00%), oltre il 5%.",
        "Un elemento non spostato: 2,00 GB della stima restano al loro posto.",
        "Procedura: Svuota il Cestino · Cestino",
        "Variazione dello spazio libero: +2,00 GB.",
        "Comando ufficiale, copiato e non eseguito da Quado · Cache di npm",
        "Metodo: npm cache verify · unità C:\\",
        "Senza la misura di partenza la variazione non si calcola.",
        "Procedura: disinstallazione del programma · Programmi installati",
        "Differenza: -10,0 MB (2,00%), entro il 5%.",
        "Una riga del registro non si legge",
    ] {
        assert!(has_text(&h, s), "manca «{s}»");
    }
}

#[test]
fn privacy_page_lists_the_files_and_deletes_the_journal() {
    // F-AC-6, 05 D8: Dati e privacy elenca i file con la dimensione; la cancellazione del registro chiede
    // conferma, poi il file non c'è più e la vista Interventi è vuota. Le impostazioni restano.
    let dir = TempDir::new("privacy");
    std::fs::write(dir.path().join("impostazioni.txt"), "tema = scuro\n").expect("impostazioni");
    journal::append(dir.path(), &fixture()[1]).expect("registro");
    let size = journal::size(dir.path()).expect("dimensione");
    let mut h = harness(app(&probe(), Some(dir.path())));
    assert!(h.query_by_label(&format!("{} 1", t(&h, "nav-interventi"))).is_some());
    h.state_mut().go(Screen::Settings);
    h.run_steps(3);
    let row = {
        let a = h.state();
        a.lingue.ta("privacy-file-registro", &[("dimensione", a.fmt.size(size).into())])
    };
    assert!(h.query_by_label(&row).is_some(), "manca «{row}»");
    assert!(has_text(&h, "Impostazioni: 13 byte"));
    click_t(&mut h, "privacy-cancella-registro");
    assert!(has_text(&h, "Lo storico della vista Interventi si svuota e non si può recuperare"));
    assert!(journal::path(dir.path()).exists(), "prima della conferma il registro c'è ancora");
    click_t(&mut h, "privacy-cancella-conferma");
    assert!(!journal::path(dir.path()).exists(), "il file non esiste più");
    assert!(dir.path().join("impostazioni.txt").exists(), "le impostazioni restano");
    h.run_steps(3);
    assert!(h.query_by_label(&row).is_none());
    assert!(has_label(&h, &t(&h, "nav-interventi")), "nessun numero nella barra laterale");
    h.state_mut().go(Screen::Interventions);
    h.run_steps(3);
    assert!(has_text(&h, &t(&h, "storico-vuoto")));
}

/// Una cartella di Rilievo in `appdata` (una cartella temporanea, mai `%APPDATA%`): impostazioni, registro e
/// una lingua aggiunta. Restituisce le cartelle di Quado e di Rilievo.
fn rilievo_folder(appdata: &Path) -> (PathBuf, PathBuf) {
    let (new, old) = legacy::dirs(appdata);
    std::fs::create_dir_all(old.join("lingue")).expect("rilievo");
    std::fs::write(old.join("impostazioni.txt"), "lingua = en\ntema = scuro\n").expect("impostazioni");
    journal::append(&old, &fixture()[1]).expect("registro");
    std::fs::write(old.join("lingue").join("de.ftl"), "nav-panoramica = Übersicht\n").expect("lingua");
    (new, old)
}

#[test]
fn the_first_start_uses_the_settings_and_the_journal_copied_from_rilievo() {
    // Cambio di nome: `Env::from_system` copia i dati di Rilievo prima di `App::new`, che poi legge le
    // impostazioni copiate (lingua, tema) e il registro; la barra di stato dice da dove vengono.
    let dir = TempDir::new("migrazione");
    let (new, old) = rilievo_folder(dir.path());
    let migration = settings::migrate(&old, &new);
    assert_eq!(migration, legacy::Migration::Copied(vec!["impostazioni.txt".into(), "interventi.jsonl".into(), "lingue".into()]));
    let mut env = Env::fixed();
    env.actions = Actions::fake(&probe());
    env.data_dir = Some(new.clone());
    env.old_data_dir = Some(old.clone());
    env.migration = migration;
    let a = App::new(env, Vec::new());
    assert_eq!(a.settings.theme, ThemeChoice::Dark);
    assert_eq!(a.settings.language.as_deref(), Some("en"));
    assert!(a.lingue.available.iter().any(|l| l.tag == "de" && l.user_file), "la lingua aggiunta in Rilievo");
    let expected = a.lingue.ta("avviso-rilievo-copiati", &[("cartella", new.display().to_string().into())]);
    assert_eq!(a.notice.current().as_deref(), Some(expected.as_str()));
    assert_eq!(journal::read(&new).expect("registro").entries.len(), 1);
    // Copia non riuscita: l'avviso con il codice, impostazioni predefinite.
    let mut env = Env::fixed();
    env.migration = legacy::Migration::Failed(32);
    let a = App::new(env, Vec::new());
    assert!(a.notice.current().is_some_and(|n| n.contains("(Codice 32)")), "{:?}", a.notice.current());
}

#[test]
fn privacy_page_shows_the_rilievo_folder_and_deletes_it() {
    // Dati e privacy mostra la cartella di Rilievo con la dimensione; la cancellazione chiede conferma,
    // poi la cartella non c'è più, la riga sparisce e la cartella di Quado resta com'è.
    let dir = TempDir::new("privacy-rilievo");
    let (new, old) = rilievo_folder(dir.path());
    std::fs::create_dir_all(&new).expect("quado");
    std::fs::write(new.join("impostazioni.txt"), "tema = scuro\n").expect("impostazioni");
    let size: u64 = ["impostazioni.txt", "interventi.jsonl", r"lingue\de.ftl"].iter().map(|f| std::fs::metadata(old.join(f)).expect("file").len()).sum();
    let mut a = app(&probe(), Some(&new));
    a.env.old_data_dir = Some(old.clone());
    let mut h = harness(a);
    h.state_mut().go(Screen::Settings);
    h.run_steps(3);
    assert!(has_text(&h, &t(&h, "privacy-rilievo-titolo")));
    let note = {
        let a = h.state();
        a.lingue.ta("privacy-rilievo-nota", &[("dimensione", a.fmt.size(size).into()), ("cartella", old.display().to_string().into())])
    };
    assert!(has_text(&h, &note), "manca «{note}»");
    click_t(&mut h, "privacy-rilievo-cancella");
    assert!(has_text(&h, "Cancellare la cartella di Rilievo"));
    assert!(old.exists(), "prima della conferma la cartella c'è ancora");
    click_t(&mut h, "privacy-annulla");
    assert!(old.exists(), "Annulla non cancella");
    click_t(&mut h, "privacy-rilievo-cancella");
    click_t(&mut h, "privacy-rilievo-cancella-conferma");
    assert!(!old.exists(), "la cartella di Rilievo non c'è più");
    assert!(new.join("impostazioni.txt").exists(), "i dati di Quado restano");
    h.run_steps(3);
    assert!(!has_text(&h, &t(&h, "privacy-rilievo-titolo")), "la riga sparisce");
    assert_eq!(h.state().notice.current(), Some(t(&h, "privacy-rilievo-cancellata")));
}
