// SPDX-License-Identifier: GPL-3.0-or-later
//! Spostamento nel Cestino dall'interfaccia (M5, agente E1), provato con `egui_kittest`: simulazione,
//! dialogo di conferma (04 §6.6 schermata 4), esecuzione, misure, riscansione, risultato (schermata 5),
//! registro.
//!
//! - 06 §5.1 e F-AC-3: Esc, Annulla, la X e il clic fuori non costruiscono il piano e il mover non riceve
//!   chiamate.
//! - F-AC-2: un controllo manuale non spuntato tiene disattivato il pulsante principale.
//! - F-AC-3, 06 §5.6: il risultato separa riusciti, saltati e falliti con il motivo; esito parziale.
//! - F-AC-8: un file in uso blocca l'elemento con il nome del processo.
//! - F-AC-7: capacità superata, testo di 04 §6.8 e pulsante disattivato.
//! - F-AC-6: il registro esiste solo dopo la conferma e contiene stimato e ottenuto.
//! - F-VE-2, F-VE-3: "Svuota il Cestino…" con l'azione finta libera lo spazio e compare il confronto.
//! - F-AC-3: il dialogo contiene i campi del record; 03 §6.1: la versione in uso di CapCut è visibile e
//!   disattivata.
//!
//! Sicurezza: `Actions::fake` su un `FakeProbe` che rispecchia l'albero sintetico di `sample_c` (mover,
//! svuotamento, lanciatore e riscansione simulati). Niente legge o scrive sul disco vero, salvo il
//! registro in una cartella temporanea dei test.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::{Event, Key, Modifiers, PointerButton, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use quado_actions::execute::{MoveResult, Mover};
use quado_actions::fake::{FakeMover, FakeProbe};
use quado_actions::journal;
use quado_actions::launch::Launch;
use quado_actions::probe::{Attrs, Probe, VolumeBin};
use quado_actions::restart::{ProcessUse, RmError};
use quado_actions::shell::ShellError;
use quado_kb::lens::Finding;
use quado_model::intervention::{BinCount, ItemOutcome, MeasurePoint, ReferenceKind, ReferenceReport, SkipReason};
use quado_model::record::Estimate;
use quado_model::{Flags, Tree};
use quado_ui::app::intervention_jobs::{self, ActRequest, Actions, Origin};
use quado_ui::app::lens_jobs::Readers;
use quado_ui::app::{App, Env, Screen, synthetic};
use quado_ui::i18n::PSEUDO;
use quado_ui::tokens::ThemeChoice;

/// Contenuto del Cestino di C: prima dell'intervento: un elemento da 1,25 MB (valore di prova).
const OTHER_BIN: BinCount = BinCount { items: 1, bytes: 1_318_890 };
/// Capacità del Cestino di C: 24.380 MiB (valore di prova).
const CAPACITY: u64 = 24_380 << 20;
/// HRESULT di un file in uso (`ERROR_SHARING_VIOLATION`).
const HR_SHARING: i32 = 0x8007_0020_u32 as i32;

struct Setup {
    h: Harness<'static, App>,
    probe: FakeProbe,
    mover: FakeMover,
}

/// Letture simulate con lo stesso albero di `tree`: cartelle, file con le loro dimensioni, cartelle negate.
fn mirror(tree: &Tree) -> FakeProbe {
    let p = FakeProbe::new().volume("C:\\");
    for n in 1..tree.len() as u32 {
        let fl = tree.flags(n);
        let path = tree.path_lossy(n);
        if tree.is_dir(n) {
            if fl.has(Flags::DENIED) {
                p.denied(&path);
            } else {
                p.dir(&path);
            }
        } else {
            p.file(&path, tree.logical(n), tree.alloc(n));
        }
    }
    p
}

/// Aperture chieste al lanciatore finto.
static LAUNCHED: Mutex<Vec<Launch>> = Mutex::new(Vec::new());

fn record_launch(l: &Launch) -> Result<(), ShellError> {
    LAUNCHED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(l.clone());
    Ok(())
}

/// Il C: sintetico con le azioni simulate e il mover condiviso con il test; `prepare` configura le letture
/// prima del caricamento. Finestra alta: dialogo e risultato si vedono senza scorrere.
fn setup_in(lang: &str, data_dir: Option<PathBuf>, prepare: impl FnOnce(&FakeProbe)) -> Setup {
    setup_full(lang, data_dir, None, prepare)
}

/// Come `setup_in`; con `look` (dimensione e tema) il disegno passa da wgpu, per le immagini.
fn setup_full(lang: &str, data_dir: Option<PathBuf>, look: Option<([f32; 2], ThemeChoice)>, prepare: impl FnOnce(&FakeProbe)) -> Setup {
    let (v, r) = synthetic::sample_c(false);
    let probe = mirror(&r.tree);
    probe.bin("C:\\", |b| {
        b.capacity = Some(CAPACITY);
        b.content = Some(OTHER_BIN);
    });
    prepare(&probe);
    let mover = FakeMover::new(&probe);
    let mut env = Env::fixed();
    env.windows_languages = vec![if lang == "en" { "en-US".into() } else { "it-IT".into() }];
    env.data_dir = data_dir;
    let mut actions = Actions::fake(&probe);
    let m = mover.clone();
    actions.mover = Arc::new(move |_| Box::new(m.clone()) as Box<dyn Mover>);
    actions.launcher = record_launch;
    env.actions = actions;
    env.readers = Readers(Arc::new(synthetic::FakeReaders::for_tree(&r.tree)));
    let mut a = App::new(env, Vec::new());
    if lang == PSEUDO {
        a.set_language(PSEUDO);
    }
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut b = Harness::builder();
    match look {
        None => b = b.with_size([1600.0, 2600.0]),
        Some((size, theme)) => {
            a.settings.theme = theme;
            b = b.with_size(size).wgpu();
        }
    }
    let mut h = b.build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(4);
    Setup { h, probe, mover }
}

fn setup(prepare: impl FnOnce(&FakeProbe)) -> Setup {
    setup_in("it", None, prepare)
}

/// Cartella temporanea vuota per il registro.
fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("quado-test-e1-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn t(h: &Harness<'_, App>, key: &str) -> String {
    h.state().lingue.t(key)
}

/// Almeno un elemento con questa etichetta.
fn has(h: &Harness<'_, App>, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

/// Almeno un elemento la cui etichetta contiene questo testo.
fn has_part(h: &Harness<'_, App>, part: &str) -> bool {
    h.query_all_by_label_contains(part).next().is_some()
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
    assert!(done(h.state()), "il lavoro in background non è finito");
    h.run_steps(3);
}

/// Un tasto premuto e rilasciato, poi due frame.
fn key(h: &mut Harness<'_, App>, k: Key) {
    h.input_mut().events.push(Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    h.input_mut().events.push(Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE });
    h.step();
    h.step();
}

/// Clic sull'unico elemento con questa etichetta, poi i frame per applicarlo.
fn click(h: &mut Harness<'_, App>, label: &str) {
    let n = h.query_all_by_label(label).count();
    assert_eq!(n, 1, "«{label}»: {n} elementi");
    h.get_by_label(label).click();
    h.run_steps(3);
}

fn click_t(h: &mut Harness<'_, App>, key: &str) {
    let label = t(h, key);
    click(h, &label);
}

/// Clic del mouse in un punto della finestra.
fn click_at(h: &mut Harness<'_, App>, x: f32, y: f32) {
    let pos = pos2(x, y);
    h.input_mut().events.push(Event::PointerMoved(pos));
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}

/// Casella con questo nome: (disattivata, spuntata).
fn checkbox(h: &Harness<'_, App>, name: &str) -> (bool, bool) {
    let n = h.query_all_by(|n| n.label().is_some_and(|l| l == name) && n.toggled().is_some()).next().unwrap_or_else(|| panic!("casella «{name}»"));
    let node = n.accesskit_node();
    (node.is_disabled(), format!("{:?}", node.toggled()).contains("True"))
}

fn primary_disabled(h: &Harness<'_, App>) -> bool {
    h.get_by_label(&t(h, "spostamento-sposta")).accesskit_node().is_disabled()
}

/// Indice della corrispondenza delle versioni precedenti di CapCut.
fn capcut(h: &Harness<'_, App>) -> usize {
    let d = h.state().data.as_ref().expect("dati");
    d.cls.matches.iter().position(|m| d.kb.rules[m.rule].id == "capcut-versioni").expect("CapCut")
}

const APPS: &str = r"C:\Users\demo\AppData\Local\CapCut\Apps";

fn version(v: &str) -> String {
    format!(r"{APPS}\{v}")
}

/// Avvia la simulazione e aspetta il dialogo di conferma.
fn open(s: &mut Setup, origin: Origin) {
    intervention_jobs::dispatch(s.h.state_mut(), ActRequest::PrepareRecycle, origin);
    assert!(s.h.state().intervention.dialog_open());
    wait(&mut s.h, |a| !a.intervention.simulating());
    assert!(s.h.state().intervention.simulation().is_some(), "dialogo di conferma aperto");
}

/// Anteprima delle versioni di CapCut con tutti gli elementi scelti: per Da verificare le caselle partono vuote
/// e «Seleziona tutti» li sceglie.
fn open_capcut(s: &mut Setup) {
    let m = capcut(&s.h);
    open(s, Origin::Rule(m));
    click_t(&mut s.h, "spostamento-seleziona-tutti");
}

/// «Svuota il Cestino…» nel risultato; per le versioni di CapCut prima la spunta «Ho aperto CapCut e funziona».
fn empty_bin(s: &mut Setup) {
    let tried = t(&s.h, "regola-capcut-versioni-prova-fatta");
    if s.h.query_all_by_label(&tried).next().is_some() {
        click(&mut s.h, &tried);
    }
    click_t(&mut s.h, "spostamento-svuota");
}

/// "Sposta nel Cestino", poi l'attesa di esecuzione, riscansione e registro.
fn confirm(s: &mut Setup) {
    assert!(!primary_disabled(&s.h), "pulsante principale attivo");
    click_t(&mut s.h, "spostamento-sposta");
    assert_eq!(s.h.state().screen, Screen::Interventions);
    wait(&mut s.h, |a| a.intervention.summary().is_some() && !a.intervention.busy());
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

#[test]
fn esc_cancel_close_and_outside_click_never_build_a_plan() {
    let dir = temp_dir("annulla");
    let mut s = setup_in("it", Some(dir.clone()), |_| {});
    let sub = t(&s.h, "spostamento-sottotitolo");
    let closes: [&dyn Fn(&mut Setup); 4] = [
        &|s: &mut Setup| key(&mut s.h, Key::Escape),
        &|s: &mut Setup| click_t(&mut s.h, "spostamento-annulla"),
        &|s: &mut Setup| click_t(&mut s.h, "spostamento-chiudi"),
        &|s: &mut Setup| click_at(&mut s.h, 6.0, 6.0),
    ];
    for close in closes {
        open_capcut(&mut s);
        assert!(has(&s.h, &sub), "«{sub}» nel dialogo");
        close(&mut s);
        assert!(!s.h.state().intervention.dialog_open(), "il dialogo si chiude");
        assert!(!has(&s.h, &sub));
    }
    // Nessun piano, nessuna chiamata al mover, niente spostato, nessun registro.
    assert_eq!(s.h.state().intervention.confirmations(), 0);
    assert!(s.mover.calls().is_empty(), "il mover non riceve chiamate: {:?}", s.mover.calls());
    assert!(s.h.state().intervention.summary().is_none());
    for v in &synthetic::CAPCUT_VERSIONS {
        assert!(s.probe.exists(&version(v)), "{v} è ancora al suo posto");
    }
    assert!(!journal::path(&dir).exists(), "nessun registro senza conferma");
    assert_eq!(s.h.state().screen, Screen::Overview);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Letture simulate che si fermano su ogni attributo finché il test non apre il cancello: la simulazione
/// resta in corso quanto serve.
struct Gated {
    inner: FakeProbe,
    open: Arc<AtomicBool>,
}

impl Gated {
    fn hold(&self) {
        for _ in 0..5000 {
            if self.open.load(Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

impl Probe for Gated {
    fn attributes(&self, path: &str) -> Result<Attrs, u32> {
        self.hold();
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
fn cancel_during_the_simulation_stops_it_without_a_confirmation() {
    // 03 §6.3: la simulazione mostra l'avanzamento e si può annullare; poi niente dialogo e niente piano.
    let mut s = setup(|_| {});
    let open = Arc::new(AtomicBool::new(false));
    s.h.state_mut().env.actions.probe = Arc::new(Gated { inner: s.probe.clone(), open: Arc::clone(&open) });
    let m = capcut(&s.h);
    intervention_jobs::dispatch(s.h.state_mut(), ActRequest::PrepareRecycle, Origin::Rule(m));
    s.h.run_steps(4);
    assert!(s.h.state().intervention.simulating());
    assert!(has(&s.h, &t(&s.h, "spostamento-simulazione-testo")), "il dialogo dice cosa legge");
    assert!(has(&s.h, "0 file e cartelle letti"), "avanzamento");
    click_t(&mut s.h, "spostamento-annulla");
    assert!(!s.h.state().intervention.dialog_open(), "Annulla chiude subito");
    open.store(true, Relaxed);
    // Il thread vede l'annullamento e finisce; il suo esito non apre niente.
    s.h.run_steps(10);
    std::thread::sleep(Duration::from_millis(50));
    s.h.run_steps(5);
    assert!(!s.h.state().intervention.dialog_open());
    assert_eq!(s.h.state().intervention.confirmations(), 0);
    assert!(s.mover.calls().is_empty());
}

#[test]
fn capcut_versions_in_use_and_kept_are_visible_and_disabled() {
    // 03 §6.1, 04 §6.6: la versione in uso e la precedente compaiono, disattivate, con "esclusa" e "tenuta".
    // Sensore memoria svuota il Cestino dopo 30 giorni; le attività pianificate non si leggono per intero.
    let mut s = setup(|p| {
        p.storage_sense(Ok(Some(30)));
        p.partial(ReferenceKind::ScheduledTask, 5);
    });
    open_capcut(&mut s);
    let sim = s.h.state().intervention.simulation().expect("simulazione");
    assert_eq!(sim.items().len(), 11);
    assert_eq!(sim.items().iter().filter(|it| it.fixed.is_some()).count(), 2);
    assert_eq!(checkbox(&s.h, "9.4.0.4015"), (true, false), "versione in uso: visibile e disattivata");
    assert_eq!(checkbox(&s.h, "9.3.0.3970"), (true, false), "versione precedente: visibile e disattivata");
    assert_eq!(checkbox(&s.h, "8.1.0.3402"), (false, true), "una versione vecchia è scelta con «Seleziona tutti»");
    assert!(has(&s.h, &t(&s.h, "spostamento-fisso-versione-in-uso")));
    assert!(has(&s.h, &t(&s.h, "spostamento-fisso-versione-tenuta")));
    // Ipotesi e rischi della regola nel corpo e la reversibilità (04 §6.8); i tre numeri, con "liberato
    // subito" a zero.
    for k in ["spostamento-ipotesi-rischi", "regola-capcut-versioni-ipotesi-1", "regola-capcut-versioni-rischio-1", "regola-capcut-versioni-rischio-2"] {
        assert!(has(&s.h, &t(&s.h, k)), "{k}");
    }
    assert!(has(&s.h, &t(&s.h, "spostamento-reversibilita")));
    for k in ["spostamento-numero-da-spostare", "spostamento-numero-subito", "spostamento-numero-dopo", "spostamento-stima-esatta"] {
        assert!(has(&s.h, &t(&s.h, k)), "{k}");
    }
    assert!(has(&s.h, "0 byte"), "liberato subito: 0");
    // Restart Manager con l'ora, riferimenti, capacità, Sensore memoria (spento nelle letture finte).
    let a = s.h.state();
    let ora = a.fmt.time(sim.checked_at());
    assert!(has(&s.h, &a.lingue.ta("spostamento-rm-ok", &[("ora", ora.into())])));
    assert!(has(&s.h, &t(&s.h, "spostamento-rif-ok")));
    assert!(has_part(&s.h, "Il Cestino di C: può contenere questa selezione: arriverebbe a"));
    assert!(has(&s.h, "Sensore memoria elimina in automatico gli elementi che restano nel Cestino per più di 30 giorni."));
    assert!(has(&s.h, "Controllo parziale. Fonti non lette per intero: attività pianificata."));
    // Punti di ripristino non letti nelle prove di prova: la nota con lo stato non noto.
    assert!(has(&s.h, &t(&s.h, "spostamento-punti-ripristino-ignoti")));
    // Il pulsante principale è attivo: CapCut non ha controlli manuali e ogni controllo passa.
    assert!(!primary_disabled(&s.h));
}

#[test]
fn the_dialog_contains_the_record_fields() {
    // F-AC-3: stato, prove con la provenienza, regola con fonte e data, i campi del record.
    let mut s = setup(|_| {});
    open_capcut(&mut s);
    // La scheda completa è una riga chiusa (variante A): si apre come farebbe l'utente.
    click_t(&mut s.h, "spostamento-scheda");
    let a = s.h.state();
    let (l, f) = (&a.lingue, &a.fmt);
    let d = a.data.as_ref().expect("dati");
    let m = &d.cls.matches[capcut(&s.h)];
    let r = &m.record;
    let mut want = vec![
        l.t("stato-da-verificare"),
        format!("{}: {}", l.t("record-azione"), quado_ui::app::kbtext::action_label(l, &r.action)),
        format!("{}: {}", l.t("record-privilegi"), quado_ui::app::kbtext::privileges_label(l, r.privileges)),
        format!("{}: {}", l.t("record-reversibilita"), l.t(&r.reversibility)),
        format!("{}: {}", l.t("record-errore"), l.t(&r.on_error)),
        format!("{}: {}", l.t("record-verifica"), l.t(&r.after)),
        l.t("ispettore-prova-gestibilita"),
        quado_ui::app::kbtext::reliability_label(l, r.reliability),
    ];
    for fa in r.identity.0.iter().chain(r.state.manageability().map(|p| p.facts()).unwrap_or_default()) {
        want.push(quado_ui::app::kbtext::fact_with_source(l, f, fa));
    }
    want.extend(r.hypotheses.iter().map(|h| l.t(h)));
    if let Some(step) = &r.preferred {
        want.push(l.t(&step.key));
    }
    // Controlli della regola con l'esito della simulazione.
    for c in &r.checks {
        want.push(format!("{} · {}", l.t(&c.key), quado_ui::app::kbtext::check_outcome(l, quado_model::record::CheckOutcome::Passed)));
    }
    let rule = &d.kb.rules[m.rule];
    want.push(l.ta("regole-verificata", &[("data", f.day(rule.verified).into())]) + " · " + &l.ta("regole-versioni", &[("programma", "capcut".into()), ("min", "5.3.0.1964".into()), ("max", "9.4.0.4015".into())]));
    for src in &rule.sources {
        let (label, shown, _) = quado_ui::app::kbtext::source_place(l, &src.url);
        want.extend(label);
        assert!(!shown.contains("demo"), "la fonte non mostra il nome di un utente: {shown}");
        want.push(shown);
        want.push(format!("«{}»", src.estratto));
    }
    want.push(l.ta("spostamento-scheda-regola", &[("nome", l.t(&rule.key("nome")).into()), ("id", rule.id.clone().into())]));
    // La stima del record compare da sola: il testo comincia già con "Stima" e nessuna etichetta lo
    // precede (niente «Stima: Stima esatta: …»).
    let sim = a.intervention.simulation().expect("dialogo di conferma");
    let estimate = quado_ui::app::kbtext::estimate_text(l, f, &sim.estimate(&sim.selectable()).estimate);
    want.push(estimate.clone());
    for w in &want {
        assert!(has(&s.h, w), "manca nel dialogo: «{w}»");
    }
    assert!(!has_part(&s.h, &format!(": {estimate}")), "la stima ha un'etichetta davanti");
}

#[test]
fn a_file_in_use_blocks_its_item_with_the_process_name() {
    // F-AC-8: un file aperto da un processo blocca l'elemento, con il nome del processo; gli altri restano.
    // F-AC-9: un collegamento che punta a un'altra versione la blocca e compare nella conferma.
    let busy = version("8.1.0.3402");
    let linked = version("8.7.2.3701");
    let lnk = r"C:\Users\demo\Desktop\CapCut vecchio.lnk";
    let mut s = setup(|p| {
        p.in_use(&format!(r"{busy}\modulo0.dll"), 4242, "CapCut.exe");
        p.reference(ReferenceKind::Shortcut, lnk, &format!(r"{linked}\CapCut.exe"));
    });
    open_capcut(&mut s);
    let a = s.h.state();
    let proc = a.lingue.ta("spostamento-processo", &[("nome", "CapCut.exe".into()), ("pid", "4242".into())]);
    let text = a.lingue.ta("spostamento-blocco-in-uso", &[("processi", proc.into())]);
    assert!(has(&s.h, &text), "«{text}»");
    assert_eq!(checkbox(&s.h, "8.1.0.3402"), (true, false), "elemento bloccato: disattivato");
    assert!(has_part(&s.h, "Un elemento è in uso secondo Windows"));
    assert!(has(&s.h, &format!("Puntano a questo elemento: {lnk} (collegamento)")));
    assert_eq!(checkbox(&s.h, "8.7.2.3701"), (true, false));
    assert!(has_part(&s.h, "Collegamenti, voci di registro o attività pianificate puntano a un elemento"));
    // Gli altri si spostano; il mover non riceve mai gli elementi bloccati.
    confirm(&mut s);
    assert!(!s.mover.calls().iter().any(|c| c.eq_ignore_ascii_case(&busy) || c.eq_ignore_ascii_case(&linked)));
    assert!(s.probe.exists(&busy) && s.probe.exists(&linked));
    let r = s.h.state().intervention.summary().expect("riepilogo");
    assert_eq!(r.items.len(), 7);
    assert!(r.items.iter().all(|i| i.outcome == ItemOutcome::Recycled));
}

#[test]
fn exceeded_capacity_blocks_with_the_text_and_disables_the_primary() {
    // F-AC-7, 04 §6.8: Cestino da 6 GiB, selezione di circa 11 GB.
    let mut s = setup(|p| {
        p.bin("C:\\", |b| b.capacity = Some(6 << 30));
    });
    open_capcut(&mut s);
    let a = s.h.state();
    let sim = a.intervention.simulation().expect("simulazione");
    let quado_actions::plan::CapacityCheck::Exceeded { capacity, total } = sim.capacity(&sim.selectable()) else {
        panic!("capacità superata")
    };
    let text = a.lingue.ta("spostamento-capacita-superata", &[("volume", "C:".into()), ("capacita", a.fmt.size(capacity).into()), ("totale", a.fmt.size(total).into())]);
    assert!(text.starts_with("Spostamento bloccato: il Cestino di C: può contenere 6,00 GB"), "{text}");
    assert!(has(&s.h, &text), "«{text}»");
    assert!(has(&s.h, &t(&s.h, "spostamento-manca-capacita")));
    assert!(primary_disabled(&s.h));
    // Il clic sul pulsante disattivato non costruisce niente.
    s.h.get_by_label(&t(&s.h, "spostamento-sposta")).click();
    s.h.run_steps(3);
    assert_eq!(s.h.state().intervention.confirmations(), 0);
    assert!(s.h.state().intervention.dialog_open());
    assert!(s.mover.calls().is_empty());
}

#[test]
fn confirmation_runs_and_the_result_separates_moved_skipped_and_failed() {
    // F-AC-3, 06 §5.6: un elemento cambiato dopo l'anteprima si salta, uno che Windows non sposta fallisce,
    // gli altri vanno nel Cestino; il risultato lo dice con il motivo e dice come ripristinare.
    let mut s = setup(|_| {});
    open_capcut(&mut s);
    let (changed, failed) = (version("6.4.1.2547"), version("7.0.0.2833"));
    s.probe.set_attrs(&changed, |a| a.modified += 1);
    s.mover.force(&failed, MoveResult::Failed(HR_SHARING), false);
    confirm(&mut s);
    assert_eq!(s.h.state().intervention.confirmations(), 1);
    let r = s.h.state().intervention.summary().expect("riepilogo").clone();
    assert_eq!(r.items.len(), 9);
    let outcome = |p: &str| r.items.iter().find(|i| i.path.eq_ignore_ascii_case(p)).map(|i| i.outcome);
    assert_eq!(outcome(&changed), Some(ItemOutcome::Skipped(SkipReason::Changed)));
    assert_eq!(outcome(&failed), Some(ItemOutcome::Failed(HR_SHARING)));
    assert_eq!(r.items.iter().filter(|i| i.outcome == ItemOutcome::Recycled).count(), 7);
    // Al mover solo gli elementi confermati e ricontrollati, mai le versioni tenute.
    let calls = s.mover.calls();
    assert_eq!(calls.len(), 8);
    for keep in ["9.4.0.4015", "9.3.0.3970"] {
        assert!(s.probe.exists(&version(keep)));
        assert!(!calls.iter().any(|c| c.ends_with(keep)));
    }
    assert!(s.probe.exists(&changed) && s.probe.exists(&failed));
    assert!(!s.probe.exists(&version("8.1.0.3402")));
    // Il risultato (04 §6.6 schermata 5).
    let a = s.h.state();
    let l = &a.lingue;
    let want = [
        l.ta("spostamento-spostati", &[("n", 7u64.into()), ("n_testo", "7".into()), ("totale", "9".into())]),
        l.ta("spostamento-esito-riga", &[("nome", "6.4.1.2547".into()), ("esito", l.t("spostamento-esito-cambiato").into())]),
        l.ta("spostamento-esito-riga", &[("nome", "7.0.0.2833".into()), ("esito", l.ta("spostamento-esito-fallito", &[("codice", "0x80070020".into())]).into())]),
        l.ta("spostamento-parziale", &[("n", 2u64.into()), ("n_testo", "2".into()), ("m", "7".into())]),
        l.t("spostamento-ricontrollati"),
    ];
    for w in &want {
        assert!(has(&s.h, w), "manca nel risultato: «{w}»");
    }
    assert!(want[3].contains("Gli elementi spostati (7) sono nel Cestino"), "{}", want[3]);
    // Il modello segue: le versioni spostate non ci sono più, quelle rimaste sì (F-VE-4).
    let d = a.data.as_ref().expect("dati");
    assert_eq!(intervention_jobs::find_node(d.tree(), &version("8.1.0.3402")), None);
    assert!(intervention_jobs::find_node(d.tree(), &changed).is_some());
    let m = a.intervention.measures();
    assert_eq!(m.iter().map(|m| m.point).collect::<Vec<_>>(), [MeasurePoint::Start, MeasurePoint::AfterAction]);
    assert_eq!(m[1].still_present, Some(0), "la riscansione dice che gli elementi spostati non occupano più spazio");
}

#[test]
fn the_journal_exists_only_after_confirmation_with_estimate_and_obtained() {
    // F-AC-6: niente registro prima della conferma; dopo, una riga con stimato e ottenuto; dopo lo
    // svuotamento una nuova riga con lo stesso id, che vale.
    let dir = temp_dir("registro");
    let mut s = setup_in("it", Some(dir.clone()), |_| {});
    open_capcut(&mut s);
    assert!(!journal::path(&dir).exists(), "simulazione e dialogo non scrivono");
    confirm(&mut s);
    assert_eq!(s.h.state().intervention.journal(), Some(quado_ui::app::intervention_view::JournalState::Written));
    assert!(has(&s.h, &t(&s.h, "spostamento-registro-scritto")));
    let j = journal::read(&dir).expect("registro");
    assert_eq!((j.entries.len(), j.unreadable_lines), (1, 0));
    let e = &j.entries[0];
    let est = s.h.state().intervention.summary().expect("riepilogo").estimate.estimate;
    assert!(matches!(est, Estimate::Exact(b) if b > 9 << 30), "{est:?}");
    assert_eq!(e.estimate, est);
    assert_eq!(e.items.len(), 9);
    assert_eq!(e.measures.len(), 2);
    assert_eq!(e.id, e.measures[0].at, "id: ora della misura di partenza");
    let c = e.comparison.as_ref().expect("confronto");
    assert_eq!((c.estimated, c.obtained), (est, 0), "subito dopo lo spostamento il volume non cambia");
    // Dopo lo svuotamento.
    empty_bin(&mut s);
    wait(&mut s.h, |a| a.intervention.comparison(MeasurePoint::AfterEmpty).is_some() && !a.intervention.busy());
    let j = journal::read(&dir).expect("registro");
    assert_eq!(j.entries.len(), 1, "stesso id: vale l'ultima riga");
    let e2 = &j.entries[0];
    assert_eq!((e2.id, e2.measures.len()), (e.id, 3));
    let c = e2.comparison.as_ref().expect("confronto");
    assert!(c.obtained > 0, "l'ottenuto c'è: {}", c.obtained);
    let lines = std::fs::read_to_string(journal::path(&dir)).expect("file").lines().count();
    assert_eq!(lines, 2, "una riga per misura");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn emptying_the_bin_frees_the_space_and_the_comparison_appears() {
    // F-AC-1, F-VE-2, F-VE-3: prima il numero e i GB degli altri elementi secondo Windows, poi il dialogo
    // di Windows (qui finto), la misura, la riscansione e i due numeri separati.
    let mut s = setup(|_| {});
    open_capcut(&mut s);
    confirm(&mut s);
    let (est, free0) = {
        let a = s.h.state();
        (a.intervention.summary().expect("riepilogo").estimate.estimate, a.intervention.measures()[0].free)
    };
    let Estimate::Exact(est) = est else { panic!("stima esatta") };
    let a = s.h.state();
    let other = a.lingue.ta("spostamento-svuota-testo", &[("n", 1u64.into()), ("n_testo", "1".into()), ("byte", a.fmt.size(OTHER_BIN.bytes).into())]);
    assert_eq!(other, "Il dialogo di Windows elimina anche un altro elemento per 1,25 MB, secondo il conteggio di Windows.");
    assert!(has(&s.h, &other));
    // Subito dopo lo spostamento: confronto con la voce "ancora nel Cestino".
    let c = a.intervention.comparison(MeasurePoint::AfterAction).expect("confronto dopo l'azione").clone();
    assert!(c.explanations.iter().any(|e| matches!(e, quado_model::intervention::Explanation::StillInBin { bytes } if *bytes == est)), "{c:?}");
    assert_eq!(c.unexplained, None);
    let still = a.lingue.ta("spostamento-voce-nel-cestino", &[("byte", a.fmt.size(est).into())]);
    assert!(has(&s.h, &still), "«{still}»");
    // "Apri il Cestino": il lanciatore tipizzato, con il testo degli elementi spostati.
    assert!(has_part(&s.h, "Apri il Cestino ed elimina solo i 9 elementi spostati da Quado il"));
    click_t(&mut s.h, "spostamento-apri");
    assert_eq!(*LAUNCHED.lock().unwrap_or_else(std::sync::PoisonError::into_inner), vec![Launch::RecycleBin]);
    // "Svuota il Cestino…".
    empty_bin(&mut s);
    wait(&mut s.h, |a| a.intervention.comparison(MeasurePoint::AfterEmpty).is_some() && !a.intervention.busy());
    let a = s.h.state();
    let c = a.intervention.comparison(MeasurePoint::AfterEmpty).expect("confronto").clone();
    let free = a.intervention.measures().last().expect("misura").free;
    assert_eq!(free - free0, est + OTHER_BIN.bytes, "il Cestino finto libera la selezione e l'altro elemento");
    assert_eq!(c.obtained, i64::try_from(est + OTHER_BIN.bytes).expect("byte"));
    assert_eq!(c.difference, Some(i64::try_from(OTHER_BIN.bytes).expect("byte")));
    for k in ["spostamento-confronto-dopo-svuotamento", "spostamento-variazione-nota"] {
        assert!(has(&s.h, &t(&s.h, k)), "{k}");
    }
    // Stimato e variazione: due numeri separati, mai sommati.
    assert!(s.h.query_all_by_label(&t(&s.h, "spostamento-stimato")).count() >= 2);
    assert!(has(&s.h, &a.fmt.size(est)), "stimato");
    let obtained = a.lingue.ta("spostamento-piu", &[("valore", a.fmt.size(est + OTHER_BIN.bytes).into())]);
    assert!(has(&s.h, &obtained), "variazione dello spazio libero «{obtained}»");
    assert!(has_part(&s.h, "Differenza: +1,25 MB"));
    assert_eq!(a.intervention.measures().last().map(|m| m.still_present), Some(Some(0)));
    // Una volta svuotato, il pulsante non riparte.
    assert!(s.h.get_by_label(&t(&s.h, "spostamento-svuota")).accesskit_node().is_disabled());
}

#[test]
fn identical_copies_need_the_sync_check_ticked_and_the_kept_copy_stays() {
    // F-AC-2, R10: in una cartella sincronizzata ogni copia scelta ha il controllo manuale; senza la spunta
    // il pulsante principale è disattivato. F-DL-4: la copia da tenere è visibile, disattivata, "resta".
    let mut s = setup(|p| {
        p.synced(r"C:\Users\demo\Downloads", "OneDrive");
    });
    s.h.state_mut().go(Screen::Downloads);
    let lens_idle = |a: &App| !a.downloads.pending() && !a.downloads.busy();
    wait(&mut s.h, lens_idle);
    for k in ["download-comando-confronta", "download-conferma-confronta"] {
        let label = t(&s.h, k);
        s.h.query_all_by_label(&label).next().unwrap_or_else(|| panic!("manca «{label}»")).click();
        s.h.run_steps(3);
    }
    wait(&mut s.h, lens_idle);
    let k = {
        let v = s.h.state().downloads.view.as_ref().expect("lente");
        v.findings.iter().position(|f| matches!(f.finding, Finding::IdenticalCopies { .. })).expect("copie identiche")
    };
    // Dietro il dialogo la Panoramica: le caselle della lente non si confondono con quelle del dialogo.
    s.h.state_mut().go(Screen::Overview);
    s.h.run_steps(2);
    open(&mut s, Origin::Lens(vec![k]));
    assert_eq!(checkbox(&s.h, "Fattura marzo.pdf"), (true, false), "la copia da tenere è disattivata");
    assert!(has(&s.h, &t(&s.h, "spostamento-fisso-copia")));
    assert_eq!(checkbox(&s.h, "Fattura marzo (1).pdf"), (false, true));
    assert_eq!(checkbox(&s.h, "fattura-copia.pdf"), (false, true));
    assert!(primary_disabled(&s.h), "controlli manuali non spuntati");
    assert!(has(&s.h, &t(&s.h, "spostamento-manca-spunta")));
    let sync = s.h.state().lingue.ta("spostamento-sincronizzato", &[("provider", "OneDrive".into())]);
    assert!(sync.contains("lo elimina anche nel cloud e sugli altri dispositivi"), "{sync}");
    let label = |name: &str| s.h.state().lingue.ta("spostamento-manuale-elemento", &[("nome", name.into()), ("testo", sync.clone().into())]);
    let (first, second) = (label("Fattura marzo (1).pdf"), label("fattura-copia.pdf"));
    click(&mut s.h, &first);
    assert!(primary_disabled(&s.h), "ne manca uno");
    click(&mut s.h, &second);
    assert!(!primary_disabled(&s.h));
    confirm(&mut s);
    let downloads = r"C:\Users\demo\Downloads";
    assert!(s.probe.exists(&format!(r"{downloads}\Fattura marzo.pdf")), "la copia da tenere resta");
    assert!(!s.probe.exists(&format!(r"{downloads}\Fattura marzo (1).pdf")));
    assert!(!s.probe.exists(&format!(r"{downloads}\fattura-copia.pdf")));
    assert_eq!(s.mover.calls().len(), 2);
    let r = s.h.state().intervention.summary().expect("riepilogo");
    assert!(r.items.iter().all(|i| i.outcome == ItemOutcome::Recycled));
    assert!(has(&s.h, &t(&s.h, "spostamento-nessuno-saltato")));
}

#[test]
fn copies_in_a_syncthing_folder_say_they_go_from_the_other_devices_too() {
    // R10: una cartella sincronizzata che Windows non conosce (marcatore di Syncthing): la spunta dice che
    // l'eliminazione arriva agli altri dispositivi, senza la riga del cestino nel cloud.
    let mut s = setup(|p| {
        p.synced_by(r"C:\Users\demo\Downloads", "Syncthing", quado_actions::sync::SyncKind::Devices);
    });
    s.h.state_mut().go(Screen::Downloads);
    let lens_idle = |a: &App| !a.downloads.pending() && !a.downloads.busy();
    wait(&mut s.h, lens_idle);
    for k in ["download-comando-confronta", "download-conferma-confronta"] {
        let label = t(&s.h, k);
        s.h.query_all_by_label(&label).next().unwrap_or_else(|| panic!("manca «{label}»")).click();
        s.h.run_steps(3);
    }
    wait(&mut s.h, lens_idle);
    let k = {
        let v = s.h.state().downloads.view.as_ref().expect("lente");
        v.findings.iter().position(|f| matches!(f.finding, Finding::IdenticalCopies { .. })).expect("copie identiche")
    };
    s.h.state_mut().go(Screen::Overview);
    s.h.run_steps(2);
    open(&mut s, Origin::Lens(vec![k]));
    let a = s.h.state();
    let sync = a.lingue.ta("spostamento-sincronizzato-dispositivi", &[("provider", "Syncthing".into())]);
    assert!(sync.contains("altri dispositivi"), "{sync}");
    let label = a.lingue.ta("spostamento-manuale-elemento", &[("nome", "fattura-copia.pdf".into()), ("testo", sync.clone().into())]);
    assert!(has(&s.h, &label), "«{label}»");
    assert!(primary_disabled(&s.h), "spunta mancante");
    assert!(!has(&s.h, &t(&s.h, "spostamento-sinc-ok")), "c'è una cartella sincronizzata");
    assert!(!has_part(&s.h, "Syncthing sul web"), "niente cestino nel cloud per Syncthing");
}

#[test]
fn without_synced_folders_the_dialog_says_what_quado_checked() {
    let mut s = setup(|_| {});
    open_capcut(&mut s);
    assert!(has(&s.h, &t(&s.h, "spostamento-sinc-ok")));
}

#[test]
fn dialog_and_result_use_only_language_files_and_no_forbidden_words() {
    // F-GE-1, 04 §6.8: con la pseudo-lingua ogni testo del dialogo e del risultato viene dai file di lingua;
    // in italiano e in inglese nessuna parola vietata, niente punti esclamativi né trattini lunghi.
    const FORBIDDEN: [&str; 14] = [
        "inutilizzat",
        "mai usato",
        "non usato da",
        "non più in uso",
        "sicuro da eliminare",
        "puoi ripristinare tutto",
        "spazzatura",
        "ottimizza",
        "unused",
        "never used",
        "not used",
        "safe to delete",
        "junk",
        "optimiz",
    ];
    for lang in ["it", "en", PSEUDO] {
        let mut s = setup_in(lang, None, |p| {
            p.in_use(&format!(r"{}\modulo0.dll", version("8.4.1.3561")), 77, "CapCut.exe");
        });
        // Dietro il dialogo l'elenco delle regole: testi tradotti e dati delle regole.
        s.h.state_mut().go(Screen::Rules);
        open_capcut(&mut s);
        let mut texts = drawn(&s.h);
        confirm(&mut s);
        empty_bin(&mut s);
        wait(&mut s.h, |a| a.intervention.comparison(MeasurePoint::AfterEmpty).is_some() && !a.intervention.busy());
        texts.extend(drawn(&s.h));
        assert!(texts.len() > 60, "{lang}: testi raccolti {}", texts.len());
        for x in &texts {
            let low = x.to_lowercase();
            for bad in FORBIDDEN {
                assert!(!low.contains(bad), "{lang}: «{bad}» in {x:?}");
            }
            assert!(!x.contains('!') && !x.contains('\u{2014}') && !x.contains('\u{2013}'), "{lang}: {x:?}");
        }
        if lang == PSEUDO {
            let data = data_strings(s.h.state());
            let plain: Vec<&String> = texts.iter().filter(|x| !is_pseudo_or_data(x, &data)).collect();
            assert!(plain.is_empty(), "testi non tradotti: {plain:?}");
        }
    }
}

#[test]
#[ignore = "immagini del dialogo e del risultato: cargo test -p quado-ui --test intervento -- --ignored"]
fn images() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/m5-immagini");
    std::fs::create_dir_all(&out).expect("cartella");
    for lang in ["it", "en"] {
        for (theme, tname) in [(ThemeChoice::Light, "chiaro"), (ThemeChoice::Dark, "scuro")] {
            let mut s = setup_full(lang, None, Some(([1600.0, 1250.0], theme)), |p| {
                p.in_use(&format!(r"{}\modulo0.dll", version("8.4.1.3561")), 77, "CapCut.exe");
            });
            open_capcut(&mut s);
            let shot = |s: &mut Setup, name: &str| {
                s.h.run_steps(3);
                let img = s.h.render().expect("disegno");
                img.save(out.join(format!("{name}-{lang}-{tname}.png"))).expect("png");
            };
            shot(&mut s, "conferma");
            s.mover.force(&version("7.0.0.2833"), MoveResult::Failed(HR_SHARING), false);
            confirm(&mut s);
            shot(&mut s, "risultato");
            empty_bin(&mut s);
            wait(&mut s.h, |a| a.intervention.comparison(MeasurePoint::AfterEmpty).is_some() && !a.intervention.busy());
            shot(&mut s, "verifica");
        }
    }
}

/// Nomi, percorsi, regole e fonti: dati, non testi da tradurre.
fn data_strings(a: &App) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Some(d) = &a.data {
        let t = d.tree();
        for i in 0..t.len() as u32 {
            set.insert(t.name_lossy(i));
            set.insert(t.path_lossy(i));
        }
        set.insert(d.volume.letter());
        set.insert(d.volume.file_system.clone());
    }
    for v in synthetic::CAPCUT_VERSIONS {
        set.insert(v.to_owned());
    }
    for r in &a.kb.rules {
        set.insert(r.id.clone());
        for src in &r.sources {
            set.insert(src.url.clone());
            set.insert(src.titolo.clone());
            set.insert(format!("«{}»", src.estratto));
        }
    }
    set.insert("Quado".into());
    set
}

fn is_pseudo_or_data(s: &str, data: &HashSet<String>) -> bool {
    let t = s.trim();
    t.is_empty()
        || (t.contains('[') && t.contains(']'))
        || data.contains(t)
        || t.chars().all(|c| c.is_ascii_digit() || " .,:%/±-…+".contains(c))
}

/// Titolo del dialogo di CapCut con `n` elementi scelti.
fn capcut_title(h: &Harness<'_, App>, n: u64) -> String {
    let a = h.state();
    let rule = &a.kb.rules[a.kb.index("capcut-versioni").expect("regola")];
    a.lingue.ta("spostamento-titolo", &[("n", n.into()), ("n_testo", a.fmt.count(n).into()), ("nome", a.lingue.t(&rule.key("nome")).into())])
}

#[test]
fn title_and_numbers_follow_the_boxes_and_the_focus_stays_in_the_dialog() {
    // Variante A: togliendo una casella cambiano titolo, "Da spostare" e il conteggio accanto all'elenco;
    // Tab non porta il focus fuori dal dialogo; Esc chiude senza costruire il piano.
    let mut s = setup(|_| {});
    open_capcut(&mut s);
    let (total, first, size_of_first) = {
        let sim = s.h.state().intervention.simulation().expect("simulazione");
        let free: Vec<_> = sim.items().iter().filter(|it| it.fixed.is_none()).collect();
        let first = free[0].path.rsplit('\\').next().expect("nome").to_owned();
        (free.iter().map(|it| it.alloc).sum::<u64>(), first, free[0].alloc)
    };
    assert!(has(&s.h, &capcut_title(&s.h, 9)), "titolo con 9 elementi");
    let size = |h: &Harness<'_, App>, b: u64| h.state().fmt.size(b);
    assert!(has(&s.h, &size(&s.h, total)), "da spostare: tutte le versioni");
    s.h.query_all_by(|n| n.label().is_some_and(|l| l == first) && n.toggled().is_some()).next().expect("casella").click();
    s.h.run_steps(3);
    assert_eq!(checkbox(&s.h, &first), (false, false), "la casella si toglie");
    assert!(has(&s.h, &capcut_title(&s.h, 8)), "il titolo segue la scelta");
    assert!(has(&s.h, &size(&s.h, total - size_of_first)), "da spostare segue la scelta");
    let chosen = s.h.state().lingue.ta("spostamento-selezionati", &[("n", 8u64.into()), ("testo", "8".into()), ("totale", "9".into())]);
    assert!(has(&s.h, &chosen), "«{chosen}»");
    // Il focus da tastiera resta sui controlli del dialogo, che sta in primo piano.
    let mut focused = 0;
    for _ in 0..40 {
        key(&mut s.h, Key::Tab);
        if let Some(id) = s.h.ctx.memory(|m| m.focused()) {
            let r = s.h.ctx.read_response(id).expect("controllo con il focus");
            assert_eq!(r.layer_id.order, egui::Order::Foreground, "il focus è uscito dal dialogo");
            focused += 1;
        }
    }
    assert!(focused > 30, "Tab porta il focus sui controlli del dialogo");
    key(&mut s.h, Key::Escape);
    assert!(!s.h.state().intervention.dialog_open(), "Esc chiude");
    assert_eq!(s.h.state().intervention.confirmations(), 0);
    assert!(s.mover.calls().is_empty());
}

#[test]
fn to_verify_starts_with_empty_boxes_and_shows_assumptions_before_the_button() {
    // Valutazione dei consigli (01/11/2026): per Da verificare nessun elemento è scelto finché non lo scegli tu,
    let mut s = setup(|_| {});
    let m = capcut(&s.h);
    open(&mut s, Origin::Rule(m));
    assert_eq!(checkbox(&s.h, "8.1.0.3402"), (false, false), "casella vuota");
    assert!(primary_disabled(&s.h), "niente da spostare");
    assert!(has(&s.h, &t(&s.h, "spostamento-manca-scelta")));
    for k in ["regola-capcut-versioni-ipotesi-1", "regola-capcut-versioni-rischio-1", "regola-capcut-versioni-rischio-2"] {
        assert!(has(&s.h, &t(&s.h, k)), "{k} visibile senza aprire la scheda");
    }
    click_t(&mut s.h, "spostamento-seleziona-tutti");
    assert_eq!(checkbox(&s.h, "8.1.0.3402"), (false, true));
    assert!(!primary_disabled(&s.h));
    assert!(s.h.get_by_label(&t(&s.h, "spostamento-seleziona-tutti")).accesskit_node().is_disabled(), "tutto già scelto");
}

#[test]
fn emptying_after_capcut_waits_for_the_tick_and_repeats_what_to_try() {
    // Prima di «Svuota il Cestino…» il risultato ripete cosa provare; per CapCut il pulsante resta disattivato
    // finché non spunti «Ho aperto CapCut e funziona». Il risultato dice anche come ripristinare.
    let mut s = setup(|_| {});
    open_capcut(&mut s);
    confirm(&mut s);
    assert!(has(&s.h, &t(&s.h, "regola-capcut-versioni-prova")));
    assert!(has(&s.h, &t(&s.h, "spostamento-ripristina")));
    let empty = t(&s.h, "spostamento-svuota");
    assert!(s.h.get_by_label(&empty).accesskit_node().is_disabled(), "disattivato senza la spunta");
    s.h.get_by_label(&empty).click();
    s.h.run_steps(3);
    assert!(s.h.state().intervention.measures().iter().all(|m| m.point != MeasurePoint::AfterEmpty), "niente svuotato");
    click_t(&mut s.h, "regola-capcut-versioni-prova-fatta");
    assert!(!s.h.get_by_label(&empty).accesskit_node().is_disabled(), "attivo dopo la spunta");
    click(&mut s.h, &empty);
    wait(&mut s.h, |a| a.intervention.comparison(MeasurePoint::AfterEmpty).is_some() && !a.intervention.busy());
}
