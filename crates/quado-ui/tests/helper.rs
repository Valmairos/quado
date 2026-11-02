// SPDX-License-Identifier: GPL-3.0-or-later
//! "Ripeti come amministratore", provato con `egui_kittest` e l'helper finto (`Elevator::fake`): niente
//! prompt UAC, niente processi, niente letture del disco.
//!
//! - Il pulsante compare solo dopo la scansione di un volume con cartelle negate; la richiesta contiene solo
//!   le cartelle negate più alte, con percorsi convertibili senza perdita.
//! - Un giro completo (`Folder`, `Totals`, `Bin` orfano e non, `Shadow`, `Done`) aggiorna albero, card e
//!   bilancio: il non attribuito scende esattamente dei byte letti.
//! - No alla richiesta di Windows, annullamento, errore della pipe: niente cambia. Una cartella con
//!   `Failed` resta negata.
//! - Risposte arrivate dopo una modifica del modello: i nodi si ritrovano dal percorso.
//! - Dopo `Totals` nessun nome di file o di sottocartella del profilo altrui nell'albero AccessKit.
//! - Nessuna parola vietata e nessuna frase causale; con la pseudo-lingua ogni testo è tradotto.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use quado_model::{Balance, DirExtra, Flags, NodeData, Tree};
use quado_scan_win::helper::pipe::PipeError;
use quado_scan_win::helper::proto::{Reply, Request, ShadowStorage, Totals};
use quado_scan_win::{Problem, ScanResult};
use quado_ui::app::data::{self, Loaded, VolumeInfo};
use quado_ui::app::helper_jobs::{self, Elevator, FakeStep, HelperEvent, HelperFailure, HelperRequest, Outcome, Phase};
use quado_ui::app::intervention_jobs::find_node;
use quado_ui::app::{App, Env, Screen, synthetic};
use quado_ui::i18n::{self, Arg, PSEUDO};
use quado_ui::tokens::ThemeChoice;

const WINDOWS_APPS: &str = r"C:\Program Files\WindowsApps";
const SVI: &str = r"C:\System Volume Information";
const GUEST: &str = r"C:\Users\ospite";
/// Cestino di un altro account in `sample_c`: nel giro completo il suo SID non esiste più.
const ORPHAN_SID: &str = "S-1-5-21-9-9-9-1002";
/// Cestino di un altro account aggiunto dal test: account esistente.
const OTHER_SID: &str = "S-1-5-21-9-9-9-1003";
/// Nomi che l'helper vedrebbe nel profilo altrui: non devono comparire da nessuna parte.
const GUEST_SECRETS: [&str; 3] = ["Diario.docx", "Foto private", "Desktop"];

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn denied() -> NodeData {
    NodeData { flags: Flags::DIR | Flags::DENIED, ..Default::default() }
}

fn bin_path(sid: &str) -> String {
    format!(r"C:\$Recycle.Bin\{sid}")
}

/// Il C: sintetico con un secondo Cestino altrui negato. Con `extra`: una cartella negata dentro una
/// negata, una con un nome non convertibile in UTF-8 e una non leggibile per un altro motivo.
fn fixture(extra: bool) -> (VolumeInfo, ScanResult) {
    let (v, mut r) = synthetic::sample_c(false);
    let bin = find_node(&r.tree, r"C:\$Recycle.Bin").expect("Cestino");
    let second = r.tree.push(bin, &w(OTHER_SID), denied());
    r.problems.push((second, Problem::AccessDenied));
    if extra {
        let wa = find_node(&r.tree, WINDOWS_APPS).expect("WindowsApps");
        let inner = r.tree.push(wa, &w("Interna"), denied());
        r.problems.push((inner, Problem::AccessDenied));
        let odd: Vec<u16> = w("Strana").into_iter().chain([0xD800]).collect();
        let odd = r.tree.push(Tree::ROOT, &odd, denied());
        r.problems.push((odd, Problem::AccessDenied));
        let busy = r.tree.push(Tree::ROOT, &w("Occupata"), denied());
        r.problems.push((busy, Problem::InUse));
    }
    r.tree.aggregate();
    r.tree.seal();
    (v, r)
}

struct Setup {
    h: Harness<'static, App>,
    requests: Arc<Mutex<Vec<Request>>>,
}

fn setup(lang: &str, extra: bool, look: Option<ThemeChoice>) -> Setup {
    let mut env = Env::fixed();
    env.windows_languages = vec![if lang == "en" { "en-US".into() } else { "it-IT".into() }];
    let mut a = App::new(env, Vec::new());
    if lang == PSEUDO {
        a.set_language(PSEUDO);
    }
    let (v, r) = fixture(extra);
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut b = Harness::builder().with_size([1600.0, 2600.0]);
    if let Some(theme) = look {
        a.settings.theme = theme;
        b = b.with_size([1600.0, 1500.0]).wgpu();
    }
    let mut h = b.build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(4);
    // "Perché i conti non tornano del tutto" è una riga chiusa (variante A): la si apre come farebbe l'utente.
    let why = h.state().lingue.t("conti-titolo");
    h.get_by_label(&why).click();
    h.run_steps(3);
    Setup { h, requests: Arc::new(Mutex::new(Vec::new())) }
}

/// La richiesta che l'interfaccia costruirebbe adesso.
fn request(s: &Setup) -> HelperRequest {
    helper_jobs::build_request(s.h.state().data.as_ref().expect("dati")).expect("cartelle negate")
}

/// Indice di una cartella nella richiesta.
fn index(r: &HelperRequest, path: &str) -> u32 {
    r.request.folders.iter().position(|f| f.eq_ignore_ascii_case(path)).unwrap_or_else(|| panic!("{path} nella richiesta")) as u32
}

/// L'helper finto con questi passi.
fn arm(s: &mut Setup, steps: Vec<FakeStep>) {
    let fake = Elevator::fake(steps, Arc::clone(&s.requests));
    s.h.state_mut().env.elevator = fake;
}

fn t(h: &Harness<'_, App>, key: &str) -> String {
    h.state().lingue.t(key)
}

fn ta(h: &Harness<'_, App>, key: &str, args: &[(&str, Arg<'_>)]) -> String {
    h.state().lingue.ta(key, args)
}

fn size(h: &Harness<'_, App>, bytes: u64) -> String {
    h.state().fmt.size(bytes)
}

fn count(h: &Harness<'_, App>, n: u64) -> String {
    h.state().fmt.count(n)
}

fn buttons(h: &Harness<'_, App>, label: &str) -> usize {
    h.query_all_by_label(label).count()
}

fn click(h: &mut Harness<'_, App>, label: &str) {
    let n = buttons(h, label);
    assert_eq!(n, 1, "«{label}»: {n} elementi");
    h.get_by_label(label).click();
    h.run_steps(2);
}

/// Frame finché `done` non è vero (al più qualche secondo).
fn wait(h: &mut Harness<'_, App>, done: impl Fn(&App) -> bool) {
    for _ in 0..3000 {
        h.step();
        if done(h.state()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(done(h.state()), "il lavoro dell'helper non è arrivato dove atteso");
    h.run_steps(3);
}

/// "Ripeti come amministratore", poi l'attesa della fine del lavoro.
fn run(s: &mut Setup) {
    let label = t(&s.h, "helper-ripeti");
    click(&mut s.h, &label);
    wait(&mut s.h, |a| !a.helper.running());
}

/// Testi disegnati nell'ultimo frame.
fn drawn(h: &Harness<'_, App>) -> Vec<String> {
    fn walk(s: &egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut out);
    }
    out
}

fn shows(h: &Harness<'_, App>, text: &str) -> bool {
    drawn(h).iter().any(|x| x == text)
}

/// Etichette e valori dell'albero AccessKit.
fn accesskit_texts(h: &Harness<'_, App>) -> Vec<String> {
    h.query_all_by(|_| true)
        .flat_map(|n| {
            let a = n.accesskit_node();
            [a.label(), a.value()]
        })
        .flatten()
        .collect()
}

/// Lo stato del modello che "niente è cambiato" deve conservare.
fn snapshot(a: &App) -> (u64, Option<Balance>, u64, usize, usize) {
    let d = a.data.as_ref().expect("dati");
    (d.generation, d.balance, d.tree().alloc(Tree::ROOT), d.tree().len(), d.observed.unreadable.len())
}

// ---------------------------------------------------------------------------------------------
// Risposte dell'helper finto

/// Sottoalbero elencato dall'helper: una cartella `dati` con i file dati e il suo indice.
fn sub_tree(path: &str, files: &[(&str, u64)], index_alloc: u64, busy: bool) -> (Tree, Vec<(u32, Problem)>) {
    let mut t = Tree::with_root(&w(path), 0);
    let d = t.push(Tree::ROOT, &w("dati"), NodeData { flags: Flags::DIR, ..Default::default() });
    for (name, alloc) in files {
        t.push(d, &w(name), NodeData { logical: alloc - 100, alloc: *alloc, attrs: 0x20, ..Default::default() });
    }
    t.set_dir_extra(DirExtra { node: d, index_alloc, error: 0 });
    let mut problems = Vec::new();
    if busy {
        let b = t.push(Tree::ROOT, &w("in uso"), NodeData { flags: Flags::DIR | Flags::ERROR, ..Default::default() });
        problems.push((b, Problem::InUse));
    }
    t.aggregate();
    t.seal();
    (t, problems)
}

fn folder(index: u32, (tree, problems): (Tree, Vec<(u32, Problem)>)) -> FakeStep {
    FakeStep::Send(HelperEvent::Reply(Reply::Folder { index, tree: Box::new(tree), problems }))
}

fn reply(r: Reply) -> FakeStep {
    FakeStep::Send(HelperEvent::Reply(r))
}

const GIB: u64 = 1 << 30;
const MIB: u64 = 1 << 20;

/// Byte delle risposte del giro completo.
struct Round {
    wa: (Tree, Vec<(u32, Problem)>),
    svi: (Tree, Vec<(u32, Problem)>),
    guest: Totals,
    orphan: Totals,
    other: Totals,
    shadow: ShadowStorage,
}

fn round() -> Round {
    Round {
        wa: sub_tree(WINDOWS_APPS, &[("Microsoft.WindowsTerminal.appx", 120 * MIB), ("resources.pri", 3 * MIB)], 8192, true),
        svi: sub_tree(SVI, &[("{3808876b-c176-4e48-b7ae-04046e6cc752}", 4 * GIB), ("tracking.log", 20 * 1024)], 4096, false),
        guest: Totals { logical: 7 * GIB, alloc: 7 * GIB + 40 * MIB, files: 1234, dirs: 87, unreadable: 0 },
        orphan: Totals { logical: 3 * GIB, alloc: 3 * GIB + 4096, files: 12, dirs: 4, unreadable: 0 },
        other: Totals { logical: 500 * MIB, alloc: 500 * MIB + 8192, files: 3, dirs: 1, unreadable: 0 },
        shadow: ShadowStorage { used: 3 * GIB, allocated: 4 * GIB, max: 46 * GIB },
    }
}

/// Il giro completo: `Started`, una risposta per cartella, `Shadow`, `Done`.
fn full_round(r: &HelperRequest, x: &Round) -> Vec<FakeStep> {
    vec![
        FakeStep::Send(HelperEvent::Connected),
        reply(Reply::Started { backup_privilege: true }),
        folder(index(r, WINDOWS_APPS), x.wa.clone()),
        reply(Reply::Totals { index: index(r, GUEST), totals: x.guest }),
        folder(index(r, SVI), x.svi.clone()),
        reply(Reply::Bin { index: index(r, &bin_path(ORPHAN_SID)), sid: ORPHAN_SID.into(), totals: x.orphan, orphan: true }),
        reply(Reply::Bin { index: index(r, &bin_path(OTHER_SID)), sid: OTHER_SID.into(), totals: x.other, orphan: false }),
        reply(Reply::Shadow(Ok(Some(x.shadow)))),
        reply(Reply::Done { code: 0 }),
    ]
}

// ---------------------------------------------------------------------------------------------
// Test

#[test]
fn button_only_with_denied_folders() {
    let s = setup("it", false, None);
    let label = t(&s.h, "helper-ripeti");
    assert_eq!(buttons(&s.h, &label), 1, "cartelle negate: il pulsante c'è");
    let n = request(&s).nodes.len();
    assert!(shows(&s.h, &ta(&s.h, "helper-spiegazione", &[("n", n.into()), ("testo", count(&s.h, n as u64).into())])), "la spiegazione accanto al pulsante");

    // Senza cartelle negate: niente pulsante e niente spiegazione.
    let mut env = Env::fixed();
    env.windows_languages = vec!["it-IT".into()];
    let mut a = App::new(env, Vec::new());
    let (v, mut r) = synthetic::sample_c(false);
    r.problems.retain(|&(_, p)| p != Problem::AccessDenied);
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut h = Harness::builder().with_size([1600.0, 2600.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(4);
    let why = h.state().lingue.t("conti-titolo");
    h.get_by_label(&why).click();
    h.run_steps(3);
    assert!(shows(&h, &t(&h, "conti-osservato")), "la riga dei conti è aperta");
    assert_eq!(buttons(&h, &label), 0, "nessuna cartella negata");
    assert!(helper_jobs::build_request(h.state().data.as_ref().expect("dati")).is_none());

    // Scansione interrotta: non è la scansione di un volume intero.
    let (v, r) = synthetic::sample_c(true);
    h.state_mut().load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    h.run_steps(4);
    assert_eq!(buttons(&h, &label), 0, "scansione interrotta");

    // Radice che non è un volume con lettera: niente richiesta.
    let (mut v, r) = synthetic::sample_c(false);
    v.root = r"C:\Users\".into();
    let l = Loaded::new(v, r, Duration::from_secs(1), Some("S-1-5-21-1-2-3-1001"));
    assert!(helper_jobs::build_request(&l).is_none(), "radice che non è un volume");
}

#[test]
fn request_has_only_the_highest_denied_folders() {
    let mut s = setup("it", true, None);
    let r = request(&s);
    let mut folders = r.request.folders.clone();
    folders.sort();
    let mut expected = vec![WINDOWS_APPS.to_owned(), SVI.to_owned(), GUEST.to_owned(), bin_path(ORPHAN_SID), bin_path(OTHER_SID)];
    expected.sort();
    assert_eq!(folders, expected, "solo le cartelle negate più alte, senza nomi non convertibili e senza altri problemi");
    assert_eq!(r.request.volume, r"C:\");
    // I nodi corrispondono ai percorsi.
    let d = s.h.state().data.as_ref().expect("dati");
    for (k, f) in r.request.folders.iter().enumerate() {
        assert_eq!(d.tree().path_lossy(r.nodes[k]), *f);
    }
    // Nessuna dentro un'altra.
    for a in &r.request.folders {
        for b in &r.request.folders {
            assert!(a == b || !b.to_lowercase().starts_with(&format!("{}\\", a.to_lowercase())), "{b} dentro {a}");
        }
    }
    // La richiesta arriva all'helper così com'è.
    arm(&mut s, vec![FakeStep::Send(HelperEvent::Failed(HelperFailure::Declined))]);
    run(&mut s);
    let sent = s.requests.lock().expect("richieste").clone();
    assert_eq!(sent, vec![r.request.clone()]);
    assert_eq!(s.h.state().helper.started(), 1);
}

#[test]
fn full_round_updates_tree_card_and_balance() {
    let mut s = setup("it", false, None);
    let r = request(&s);
    let x = round();
    arm(&mut s, full_round(&r, &x));
    let before = s.h.state().data.as_ref().expect("dati").balance.expect("bilancio");
    // Una ricerca in background tiene il modello: l'applicazione la aspetta.
    s.h.state_mut().search.set_query("download");
    run(&mut s);
    let a = s.h.state();
    let Some(Outcome::Finished(sum)) = a.helper.outcome() else { panic!("esito: {:?}", a.helper.outcome()) };
    assert_eq!((sum.asked, sum.applied, sum.ignored, sum.code), (5, 5, 0, 0));
    assert!(sum.failed.is_empty());
    let d = a.data.as_ref().expect("dati");
    let tree = d.tree();

    // Albero: sottoalberi innestati, profilo e Cestini misurati senza figli.
    let wa = find_node(tree, WINDOWS_APPS).expect("WindowsApps");
    assert_eq!(tree.alloc(wa), x.wa.0.alloc(Tree::ROOT));
    assert!(find_node(tree, &format!(r"{WINDOWS_APPS}\dati\Microsoft.WindowsTerminal.appx")).is_some());
    let svi = find_node(tree, SVI).expect("SVI");
    assert_eq!(tree.alloc(svi), x.svi.0.alloc(Tree::ROOT));
    let guest = find_node(tree, GUEST).expect("profilo");
    assert!(tree.flags(guest).has(Flags::MEASURED));
    assert_eq!((tree.alloc(guest), u64::from(tree.files(guest)), tree.children(guest).count()), (x.guest.alloc, x.guest.files, 0));
    for (sid, tot) in [(ORPHAN_SID, x.orphan), (OTHER_SID, x.other)] {
        let b = find_node(tree, &bin_path(sid)).expect("Cestino");
        assert!(tree.flags(b).has(Flags::MEASURED));
        assert_eq!(tree.alloc(b), tot.alloc);
    }
    // Il problema del sottoalbero è sul nuovo nodo; nessuna cartella negata resta.
    let busy = find_node(tree, &format!(r"{WINDOWS_APPS}\in uso")).expect("cartella in uso");
    assert_eq!(d.problem(busy), Some(Problem::InUse));
    assert!(d.observed.unreadable.iter().all(|&(_, p)| p != Problem::AccessDenied), "{:?}", d.observed.unreadable);
    // Innesti e cartelle misurate: figli ordinati come da una costruzione completa.
    assert_eq!(*d.ord(), quado_model::Ordered::build(tree));

    // Bilancio: stesso usato; il non attribuito scende esattamente dei byte letti (allocate dei file e
    // degli indici delle cartelle elencate). Le copie shadow non entrano.
    let after = d.balance.expect("bilancio");
    let read = x.wa.0.alloc(Tree::ROOT) + x.svi.0.alloc(Tree::ROOT) + x.guest.alloc + x.orphan.alloc + x.other.alloc;
    let indexes = 8192 + 4096;
    assert_eq!(after.used, before.used);
    assert_eq!(after.allocated - before.allocated, read);
    assert_eq!(after.metadata.dir_indexes - before.metadata.dir_indexes, indexes);
    assert_eq!(before.unattributed - after.unattributed, (read + indexes) as i64);
    assert_eq!(after.metadata.reserved, before.metadata.reserved);
    assert_eq!(after.allocated, tree.alloc(Tree::ROOT));

    // Card: valori misurati in "Osservato", "Possibile" senza le voci misurate, riepilogo, niente pulsante.
    let h = &s.h;
    let texts = drawn(h);
    let has = |x: &str| texts.iter().any(|y| y == x);
    let folders = x.wa.0.alloc(Tree::ROOT) + x.svi.0.alloc(Tree::ROOT);
    assert!(has(&ta(h, "conti-lette-amministratore", &[("n", 2u64.into()), ("testo", count(h, 2).into()), ("dimensione", size(h, folders).into())])), "cartelle lette");
    assert!(has(&ta(h, "conti-cestino-orfano", &[("sid", ORPHAN_SID.into()), ("dimensione", size(h, x.orphan.alloc).into())])), "Cestino orfano");
    assert!(has(&ta(h, "conti-cestino-altro", &[("sid", OTHER_SID.into()), ("dimensione", size(h, x.other.alloc).into())])), "Cestino di un altro account");
    let file = ta(h, "conteggio-file", &[("n", x.guest.files.into()), ("testo", count(h, x.guest.files).into())]);
    assert!(has(&ta(h, "conti-profili-misurati", &[("n", 1u64.into()), ("testo", count(h, 1).into()), ("dimensione", size(h, x.guest.alloc).into()), ("file", file.into())])), "profilo");
    let sh = &x.shadow;
    assert!(has(&ta(h, "conti-copie-shadow-misurate", &[("usato", size(h, sh.used).into()), ("allocato", size(h, sh.allocated).into()), ("massimo", size(h, sh.max).into())])), "copie shadow");
    assert!(!has(&t(h, "conti-copie-shadow")) && !has(&t(h, "conti-archiviazione-riservata")), "\"Possibile\" perde le voci misurate");
    assert!(has(&t(h, "conti-altri-metafile")));
    assert!(!has(&ta(h, "conti-cestini-altri", &[("n", 2u64.into()), ("testo", count(h, 2).into())])) && !has(&ta(h, "conti-profili-altri", &[("n", 1u64.into()), ("testo", count(h, 1).into())])));
    assert!(has(&ta(h, "helper-fine", &[("n", 5u64.into()), ("lette", count(h, 5).into()), ("testo", count(h, 5).into())])), "riepilogo");
    assert_eq!(buttons(h, &t(h, "helper-ripeti")), 0, "nessuna cartella negata da chiedere");
    // Hard link: nota, non riga da sottrarre.
    let links = &d.links;
    assert!(has(&ta(h, "conti-hard-link", &[("n", links.extra_names.into()), ("testo", count(h, links.extra_names).into()), ("byte", size(h, links.extra_alloc).into())])));
}

#[test]
fn reserved_storage_has_its_own_row() {
    let s = setup("it", false, None);
    let h = &s.h;
    let b = h.state().data.as_ref().expect("dati").balance.expect("bilancio");
    let reserved = b.metadata.reserved.expect("archiviazione riservata letta");
    let texts = drawn(h);
    let row = t(h, "conti-riservato");
    assert!(texts.contains(&row), "riga dello spazio riservato");
    assert!(texts.contains(&size(h, reserved)), "valore dello spazio riservato");
    assert!(texts.contains(&size(h, b.metadata.total() - reserved)), "metadati di NTFS senza lo spazio riservato");
    assert!(!texts.contains(&t(h, "conti-archiviazione-riservata")), "misurata: non più tra le voci possibili");
    assert!(texts.contains(&t(h, "conti-copie-shadow")), "le copie shadow restano possibili");

    // Senza la lettura dello spazio riservato la voce resta tra le possibili e la riga non c'è.
    let mut env = Env::fixed();
    env.windows_languages = vec!["it-IT".into()];
    let mut a = App::new(env, Vec::new());
    let (v, mut r) = synthetic::sample_c(false);
    if let Some(b) = r.balance.as_mut() {
        b.metadata.reserved = None;
    }
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut h2 = Harness::builder().with_size([1600.0, 2600.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h2.run_steps(4);
    let why = h2.state().lingue.t("conti-titolo");
    h2.get_by_label(&why).click();
    h2.run_steps(3);
    let texts = drawn(&h2);
    assert!(!texts.contains(&row));
    assert!(texts.contains(&t(&h2, "conti-archiviazione-riservata")));
}

#[test]
fn declined_leaves_everything_as_it_was() {
    let mut s = setup("it", false, None);
    arm(&mut s, vec![FakeStep::Send(HelperEvent::Failed(HelperFailure::Declined))]);
    let before = snapshot(s.h.state());
    run(&mut s);
    assert_eq!(s.h.state().helper.outcome(), Some(&Outcome::Declined));
    assert_eq!(snapshot(s.h.state()), before, "niente è cambiato");
    assert!(shows(&s.h, &t(&s.h, "helper-rifiutato")));
    assert_eq!(buttons(&s.h, &t(&s.h, "helper-ripeti")), 1, "si può ripetere");
}

#[test]
fn waiting_reading_and_cancel() {
    // Prompt di Windows aperto: "in attesa", poi Annulla.
    let mut s = setup("it", false, None);
    arm(&mut s, vec![FakeStep::UntilCancel]);
    let before = snapshot(s.h.state());
    let label = t(&s.h, "helper-ripeti");
    click(&mut s.h, &label);
    wait(&mut s.h, |a| a.helper.phase() == Some(Phase::Consent));
    assert!(shows(&s.h, &t(&s.h, "helper-attesa")));
    assert_eq!(buttons(&s.h, &label), 0, "durante il lavoro niente secondo avvio");
    let cancel = t(&s.h, "helper-annulla");
    click(&mut s.h, &cancel);
    assert_eq!(s.h.state().helper.outcome(), Some(&Outcome::Cancelled));
    assert!(shows(&s.h, &t(&s.h, "helper-annullato")));
    assert_eq!(snapshot(s.h.state()), before);

    // Lettura a metà: una cartella su cinque, poi Annulla: la risposta arrivata si scarta.
    let r = request(&s);
    let x = round();
    arm(&mut s, vec![FakeStep::Send(HelperEvent::Connected), reply(Reply::Started { backup_privilege: true }), folder(index(&r, WINDOWS_APPS), x.wa), FakeStep::UntilCancel]);
    click(&mut s.h, &label);
    wait(&mut s.h, |a| a.helper.phase() == Some(Phase::Reading { done: 1, total: 5 }));
    let reading = ta(&s.h, "helper-lettura", &[("n", 5u64.into()), ("fatte", count(&s.h, 1).into()), ("testo", count(&s.h, 5).into())]);
    assert!(shows(&s.h, &reading), "{reading}");
    click(&mut s.h, &cancel);
    assert_eq!(s.h.state().helper.outcome(), Some(&Outcome::Cancelled));
    s.h.run_steps(4);
    assert_eq!(snapshot(s.h.state()), before, "la cartella letta prima dell'annullamento non entra nel modello");
    let d = s.h.state().data.as_ref().expect("dati");
    assert_eq!(d.tree().children(find_node(d.tree(), WINDOWS_APPS).expect("WindowsApps")).count(), 0);
}

#[test]
fn pipe_errors_change_nothing() {
    for (failure, motivo) in [
        (HelperFailure::Pipe(PipeError::Closed), "helper-motivo-chiuso"),
        (HelperFailure::Pipe(PipeError::WrongPeer(4242)), "helper-motivo-processo"),
        (HelperFailure::Launch(2), "helper-motivo-codice"),
        (HelperFailure::Exited(6), "helper-motivo-uscita"),
        (HelperFailure::Pipe(PipeError::Timeout), "helper-motivo-tempo"),
    ] {
        let mut s = setup("it", false, None);
        let r = request(&s);
        let x = round();
        arm(&mut s, vec![FakeStep::Send(HelperEvent::Connected), folder(index(&r, WINDOWS_APPS), x.wa), FakeStep::Send(HelperEvent::Failed(failure))]);
        let before = snapshot(s.h.state());
        run(&mut s);
        assert_eq!(s.h.state().helper.outcome(), Some(&Outcome::Failed(failure)));
        assert_eq!(snapshot(s.h.state()), before, "{failure:?}: niente è cambiato");
        let why = match failure {
            HelperFailure::Launch(c) | HelperFailure::Exited(c) => ta(&s.h, motivo, &[("codice", c.to_string().into())]),
            HelperFailure::Pipe(PipeError::Timeout) => ta(&s.h, motivo, &[("secondi", count(&s.h, 60).into())]),
            _ => t(&s.h, motivo),
        };
        assert!(shows(&s.h, &ta(&s.h, "helper-errore", &[("motivo", why.into())])), "{failure:?}");
    }
}

#[test]
fn an_executable_in_downloads_says_where_to_move_it() {
    // Da Download o da una cartella temporanea l'helper non parte. Il controllo prima della richiesta
    // di Windows (`Location`) e l'uscita dell'helper con il codice 3 danno lo stesso testo: perché e cosa
    // fare, senza il codice e senza «puoi riprovare».
    for lang in ["it", "en"] {
        for failure in [HelperFailure::Location, HelperFailure::Exited(3)] {
            let mut s = setup(lang, false, None);
            arm(&mut s, vec![FakeStep::Send(HelperEvent::Failed(failure))]);
            let before = snapshot(s.h.state());
            run(&mut s);
            assert_eq!(s.h.state().helper.outcome(), Some(&Outcome::Failed(failure)));
            assert_eq!(snapshot(s.h.state()), before, "{lang} {failure:?}: niente è cambiato");
            let text = t(&s.h, "helper-posizione");
            assert!(shows(&s.h, &text), "{lang} {failure:?}: {text}");
            assert!(text.contains("quado.exe") && text.contains(r"%LOCALAPPDATA%\Programs\Quado"), "{lang}: {text}");
            let exited = ta(&s.h, "helper-motivo-uscita", &[("codice", "3".into())]);
            assert!(!shows(&s.h, &ta(&s.h, "helper-errore", &[("motivo", exited.into())])), "{lang} {failure:?}: niente «Codice 3»");
        }
    }
}

#[test]
fn failed_folder_stays_denied() {
    let mut s = setup("it", false, None);
    let r = request(&s);
    let x = round();
    let steps = vec![
        FakeStep::Send(HelperEvent::Connected),
        reply(Reply::Started { backup_privilege: true }),
        folder(index(&r, WINDOWS_APPS), x.wa),
        reply(Reply::Failed { index: index(&r, SVI), code: 5 }),
        reply(Reply::Totals { index: index(&r, GUEST), totals: x.guest }),
        reply(Reply::Bin { index: index(&r, &bin_path(ORPHAN_SID)), sid: ORPHAN_SID.into(), totals: x.orphan, orphan: true }),
        reply(Reply::Bin { index: index(&r, &bin_path(OTHER_SID)), sid: OTHER_SID.into(), totals: x.other, orphan: false }),
        reply(Reply::Shadow(Err(0x8004_1003))),
        reply(Reply::Done { code: 0 }),
    ];
    arm(&mut s, steps);
    run(&mut s);
    let a = s.h.state();
    let Some(Outcome::Finished(sum)) = a.helper.outcome() else { panic!("esito") };
    assert_eq!(sum.failed, vec![(SVI.to_owned(), 5)]);
    assert_eq!(sum.applied, 4);
    let d = a.data.as_ref().expect("dati");
    let svi = find_node(d.tree(), SVI).expect("SVI");
    let denied: Vec<_> = d.observed.unreadable.iter().copied().filter(|&(_, p)| p == Problem::AccessDenied).collect();
    assert_eq!(denied, vec![(svi, Problem::AccessDenied)], "la cartella resta negata");
    assert_eq!(d.tree().children(svi).count(), 0);
    // Si può ripetere, solo per la cartella rimasta negata.
    let again = helper_jobs::build_request(d).expect("richiesta");
    assert_eq!(again.request.folders, vec![SVI.to_owned()]);
    let h = &s.h;
    assert_eq!(buttons(h, &t(h, "helper-ripeti")), 1);
    assert!(shows(h, &ta(h, "helper-spiegazione", &[("n", 1u64.into()), ("testo", count(h, 1).into())])));
    assert!(shows(h, &ta(h, "helper-fine-non-lette", &[("n", 1u64.into()), ("testo", count(h, 1).into()), ("codici", "5".into())])));
    // Copie shadow non lette: restano possibili, con il codice.
    assert!(shows(h, &ta(h, "conti-copie-shadow-errore", &[("codice", "0x80041003".into())])));
}

#[test]
fn replies_after_a_model_change_follow_the_paths() {
    let (v, r) = fixture(false);
    let mut l = Loaded::new(v, r, Duration::from_secs(1), Some("S-1-5-21-1-2-3-1001"));
    let mut sent = helper_jobs::build_request(&l).expect("richiesta");
    let x = round();
    // Il modello è cambiato dalla richiesta: i nodi della richiesta non valgono più.
    sent.generation += 1;
    sent.nodes = vec![Tree::ROOT; sent.nodes.len()];
    // Una cartella non c'è più nel modello di adesso.
    sent.request.folders.push(r"C:\Non esiste più".into());
    sent.nodes.push(Tree::ROOT);
    let gone = (sent.request.folders.len() - 1) as u32;
    let replies = vec![
        Reply::Started { backup_privilege: true },
        Reply::Folder { index: index(&sent, WINDOWS_APPS), tree: Box::new(x.wa.0.clone()), problems: Vec::new() },
        // Risposta ripetuta per la stessa cartella: ignorata.
        Reply::Folder { index: index(&sent, WINDOWS_APPS), tree: Box::new(x.wa.0.clone()), problems: Vec::new() },
        Reply::Totals { index: index(&sent, GUEST), totals: x.guest },
        Reply::Totals { index: gone, totals: x.guest },
        Reply::Done { code: 0 },
    ];
    let alloc0 = l.tree().alloc(Tree::ROOT);
    let generation = l.generation;
    let sum = data::apply_helper(&mut l, &sent, replies, Some("S-1-5-21-1-2-3-1001"));
    assert_eq!((sum.applied, sum.ignored), (2, 2));
    assert_eq!(l.tree().alloc(Tree::ROOT), alloc0 + x.wa.0.alloc(Tree::ROOT) + x.guest.alloc);
    assert!(l.generation > generation);
    assert!(l.tree().flags(find_node(l.tree(), GUEST).expect("profilo")).has(Flags::MEASURED));
    // Figli ordinati aggiornati solo per le cartelle toccate: gli stessi di una costruzione completa.
    assert_eq!(*l.ord(), quado_model::Ordered::build(l.tree()));

    // Un altro volume: niente si applica.
    let mut other = sent.clone();
    other.request.volume = r"D:\".into();
    let replies = vec![Reply::Totals { index: index(&sent, &bin_path(OTHER_SID)), totals: x.other }, Reply::Done { code: 0 }];
    let alloc1 = l.tree().alloc(Tree::ROOT);
    let sum = data::apply_helper(&mut l, &other, replies, None);
    assert_eq!((sum.applied, sum.ignored), (0, 1));
    assert_eq!(l.tree().alloc(Tree::ROOT), alloc1);
}

#[test]
fn no_names_from_another_profile() {
    let mut s = setup("it", false, None);
    let r = request(&s);
    let x = round();
    arm(&mut s, full_round(&r, &x));
    run(&mut s);
    // Esplora sul profilo altrui: nessun figlio, nessun nome sotto il profilo.
    {
        let a = s.h.state_mut();
        let d = a.data.as_ref().expect("dati");
        let guest = find_node(d.tree(), GUEST).expect("profilo");
        assert_eq!(d.tree().children(guest).count(), 0);
        let users = d.tree().parent(guest);
        a.explore.open(d, users, Some(guest));
        a.go(Screen::Explore);
    }
    s.h.run_steps(6);
    let texts = accesskit_texts(&s.h);
    assert!(texts.len() > 20, "albero AccessKit: {}", texts.len());
    let guest_prefix = format!("{}\\", GUEST.to_lowercase());
    for x in &texts {
        let low = x.to_lowercase();
        assert!(!low.contains(&guest_prefix), "percorso sotto il profilo altrui: {x:?}");
        for secret in GUEST_SECRETS {
            assert!(!x.contains(secret), "nome del profilo altrui: {x:?}");
        }
    }
    {
        let a = s.h.state_mut();
        let d = a.data.as_ref().expect("dati");
        let guest = find_node(d.tree(), GUEST).expect("profilo");
        a.explore.open(d, guest, None);
    }
    s.h.run_steps(6);
    for x in accesskit_texts(&s.h).iter().chain(drawn(&s.h).iter()) {
        assert!(!x.to_lowercase().contains(&guest_prefix), "percorso sotto il profilo altrui: {x:?}");
    }
}

/// Testi della card in ogni stato: iniziale, in attesa, in lettura, annullato, rifiutato, errori, giro
/// completo, cartella fallita.
fn every_state(lang: &str) -> (Vec<String>, HashSet<String>) {
    let mut texts = Vec::new();
    let mut data = HashSet::new();
    let mut s = setup(lang, false, None);
    texts.extend(drawn(&s.h));
    let label = t(&s.h, "helper-ripeti");
    let cancel = t(&s.h, "helper-annulla");
    let r = request(&s);
    let x = round();
    arm(&mut s, vec![FakeStep::Send(HelperEvent::Connected), folder(index(&r, WINDOWS_APPS), x.wa.clone()), FakeStep::UntilCancel]);
    click(&mut s.h, &label);
    wait(&mut s.h, |a| matches!(a.helper.phase(), Some(Phase::Reading { done: 1, .. })));
    texts.extend(drawn(&s.h));
    click(&mut s.h, &cancel);
    texts.extend(drawn(&s.h));
    arm(&mut s, vec![FakeStep::UntilCancel]);
    click(&mut s.h, &label);
    wait(&mut s.h, |a| a.helper.phase() == Some(Phase::Consent));
    texts.extend(drawn(&s.h));
    click(&mut s.h, &cancel);
    for f in [HelperFailure::Declined, HelperFailure::Pipe(PipeError::Timeout), HelperFailure::Pipe(PipeError::Proto(quado_scan_win::helper::proto::ProtoError::Malformed)), HelperFailure::Exited(4), HelperFailure::Location] {
        arm(&mut s, vec![FakeStep::Send(HelperEvent::Failed(f))]);
        run(&mut s);
        texts.extend(drawn(&s.h));
    }
    let steps = vec![
        FakeStep::Send(HelperEvent::Connected),
        reply(Reply::Started { backup_privilege: false }),
        folder(index(&r, WINDOWS_APPS), x.wa.clone()),
        reply(Reply::Failed { index: index(&r, SVI), code: 5 }),
        reply(Reply::Totals { index: index(&r, GUEST), totals: x.guest }),
        reply(Reply::Bin { index: index(&r, &bin_path(ORPHAN_SID)), sid: ORPHAN_SID.into(), totals: x.orphan, orphan: true }),
        reply(Reply::Shadow(Ok(Some(ShadowStorage { max: u64::MAX, ..x.shadow })))),
        reply(Reply::Done { code: 0x8004_1003 }),
    ];
    arm(&mut s, steps);
    run(&mut s);
    texts.extend(drawn(&s.h));
    // Secondo giro sulla cartella rimasta: il Cestino ancora negato e SVI.
    let r2 = request(&s);
    arm(&mut s, vec![
        reply(Reply::Started { backup_privilege: true }),
        folder(index(&r2, SVI), x.svi.clone()),
        reply(Reply::Bin { index: index(&r2, &bin_path(OTHER_SID)), sid: OTHER_SID.into(), totals: x.other, orphan: false }),
        reply(Reply::Shadow(Ok(None))),
        reply(Reply::Done { code: 0 }),
    ]);
    run(&mut s);
    texts.extend(drawn(&s.h));
    let a = s.h.state();
    if let Some(d) = &a.data {
        let t = d.tree();
        for i in 0..t.len() as u32 {
            data.insert(t.name_lossy(i));
            data.insert(t.path_lossy(i));
        }
        data.insert(d.volume.letter());
        data.insert(d.volume.file_system.clone());
    }
    for r in &a.kb.rules {
        data.insert(r.id.clone());
        for src in &r.sources {
            data.insert(src.url.clone());
            data.insert(src.titolo.clone());
        }
    }
    data.insert("Quado".into());
    data.insert("download".into());
    (texts, data)
}

#[test]
fn card_texts_have_no_forbidden_words_and_are_translated() {
    const FORBIDDEN: [&str; 18] = [
        "sicuro da eliminare",
        "sicuro da disinstallare",
        "inutilizzato",
        "mai usato",
        "non usato da",
        "non più in uso",
        "puoi ripristinare tutto",
        "spazio sprecato",
        "spazzatura",
        "ottimizza",
        "pulizia intelligente",
        "unused",
        "never used",
        "safe to delete",
        "safe to uninstall",
        "wasted space",
        "junk",
        "optimiz",
    ];
    for lang in ["it", "en", PSEUDO] {
        let (texts, data) = every_state(lang);
        assert!(texts.len() > 100, "{lang}: testi raccolti {}", texts.len());
        for x in &texts {
            let low = x.to_lowercase();
            for bad in FORBIDDEN {
                assert!(!low.contains(bad), "{lang}: «{bad}» in {x:?}");
            }
            assert!(!x.contains('!') && !x.contains('\u{2014}') && !x.contains('\u{2013}'), "{lang}: {x:?}");
        }
        if lang == PSEUDO {
            let plain: Vec<&String> = texts
                .iter()
                .filter(|x| {
                    let t = x.trim();
                    !(t.is_empty() || (t.contains('[') && t.contains(']')) || data.contains(t) || t.chars().all(|c| c.is_ascii_digit() || " .,:%/±-…+".contains(c)))
                })
                .collect();
            assert!(plain.is_empty(), "testi non tradotti: {plain:?}");
        }
    }
}

/// Nessuna frase causale nei testi della card dei conti e di "Ripeti come amministratore"
/// (tutte le varianti, nelle due lingue), salvo il titolo della card.
#[test]
fn card_catalog_has_no_causal_sentence() {
    let causal = ["perché", "a causa", "dovut", "causat", "per via", "because", "due to", "caused", "since "];
    for tag in ["it", "en"] {
        let src = i18n::embedded(tag).expect("lingua");
        let mut current = String::new();
        let mut checked = 0;
        for line in src.lines() {
            if line.trim_start().starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if !line.starts_with(' ') && !line.starts_with('}') {
                current = line.split('=').next().unwrap_or("").trim().to_owned();
            }
            if !(current.starts_with("conti-") || current.starts_with("helper-")) || current == "conti-titolo" {
                continue;
            }
            checked += 1;
            let v = line.to_lowercase();
            for c in causal {
                assert!(!v.contains(c), "{tag}: «{line}» contiene «{c}»");
            }
        }
        assert!(checked > 40, "{tag}: righe controllate {checked}");
    }
}

#[test]
#[ignore = "immagini della card: cargo test -p quado-ui --test helper -- --ignored"]
fn images() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/m6-immagini");
    std::fs::create_dir_all(&out).expect("cartella");
    for lang in ["it", "en"] {
        for (theme, tname) in [(ThemeChoice::Light, "chiaro"), (ThemeChoice::Dark, "scuro")] {
            let mut s = setup(lang, false, Some(theme));
            let shot = |s: &mut Setup, name: &str| {
                s.h.run_steps(3);
                let img = s.h.render().expect("disegno");
                img.save(out.join(format!("{name}-{lang}-{tname}.png"))).expect("png");
            };
            shot(&mut s, "conti-prima");
            let r = request(&s);
            let x = round();
            let mut steps = full_round(&r, &x);
            // Il profilo resta negato nell'immagine del giro con un errore: una cartella non aperta.
            steps[3] = reply(Reply::Failed { index: index(&r, GUEST), code: 5 });
            arm(&mut s, steps);
            run(&mut s);
            shot(&mut s, "conti-dopo-errore");
            let r = request(&s);
            arm(&mut s, vec![reply(Reply::Started { backup_privilege: true }), reply(Reply::Totals { index: index(&r, GUEST), totals: x.guest }), reply(Reply::Shadow(Ok(Some(x.shadow)))), reply(Reply::Done { code: 0 })]);
            run(&mut s);
            shot(&mut s, "conti-dopo");
        }
    }
}
