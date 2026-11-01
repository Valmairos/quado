// SPDX-License-Identifier: GPL-3.0-or-later
//! Testi dell'interfaccia (06 §7, 04 §6.8, 07 §1.7 "rifinitura"): i controlli del vocabolario e delle
//! lingue che mancavano. Quelli che esistono già non si ripetono qui:
//! - stesse chiavi e stessi segnaposto in `it.ftl` ed `en.ftl`, ogni chiave italiana con il commento di
//!   contesto, nessuna riga con errori: `quado_ui::i18n::tests::embedded_languages_are_complete_and_aligned`;
//! - le parole vietate principali di 04 §6.8, i punti esclamativi, i trattini lunghi e medi nei valori dei
//!   cataloghi dell'interfaccia e delle regole: `i18n::tests::forbidden_words_are_absent`;
//! - ogni chiave scritta nel codice esiste: `i18n::tests::every_key_used_in_code_exists`;
//! - pseudo-lingua su ogni schermata, nessun testo nel codice né tagliato:
//!   `tests/screens.rs::pseudo_language_covers_every_text`;
//! - cambio di lingua senza riavvio e riga sbagliata di un file dell'utente ignorata, con l'errore in
//!   Impostazioni: `tests/behaviour.rs::language_changes_without_restart_and_bad_user_lines_are_reported`
//!   e `i18n::tests::fallback_and_user_overrides`;
//! - nessun testo nei tipi pubblici di errore e di stato di `quado-model`, `quado-scan-win` (helper
//!   compreso) e `quado-actions`: `quado-model/tests/architettura.rs::core_types_carry_codes_not_sentences`.
//!
//! Qui:
//! - il resto del vocabolario di 04 §6.8: le forme flesse delle parole vietate, "non usato da N giorni",
//!   gli equivalenti inglesi, "residuo" solo come possibilità;
//! - i cinque stati con le stesse parole in card, ispettore, conferma e risultato, da una sola funzione;
//! - nessun testo nei tipi di errore e di esito della base delle regole (`quado-kb`);
//! - gli stati vuoti e i messaggi aggiunti in M6, in pseudo-lingua: dal catalogo e interi;
//! - l'avviso "Fine scansione" di 04 §6.8;
//! - la barra di stato e la riga della scansione dicono com'è avviato Quado (`Env::elevated`), non chi è
//!   l'utente.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use quado_actions::execute::Mover;
use quado_actions::fake::{FakeMover, FakeProbe};
use quado_model::intervention::BinCount;
use quado_model::{Flags, Links, Tree};
use quado_scan_win::{ScanResult, Stats};
use quado_ui::app::intervention_jobs::{self, ActRequest, Actions, Origin};
use quado_ui::app::lens_jobs::Readers;
use quado_ui::app::{App, Env, Screen, kbtext, synthetic, volumes_view};
use quado_ui::i18n::{PSEUDO, embedded};

// ------------------------------------------------------------------------------------------------
// Catalogo

/// Messaggi di un file Fluent: (chiave, valore su una riga), senza i commenti.
fn messages(src: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in src.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if line.starts_with(|c: char| c.is_ascii_lowercase())
            && let Some((k, v)) = line.split_once(" = ")
        {
            out.push((k.trim().to_owned(), v.to_owned()));
        } else if let Some(last) = out.last_mut() {
            last.1.push(' ');
            last.1.push_str(line.trim());
        }
    }
    out
}

/// 04 §6.8: "Parole da non usare mai". Radici, così valgono anche le forme flesse ("inutilizzati",
/// "ottimizzare"). Le altre parole dell'elenco sono già in `i18n::tests::forbidden_words_are_absent`.
const FORBIDDEN_IT: [&str; 12] = [
    "inutilizzat",
    "mai usat",
    "non usat",
    "non più in uso",
    "sicuro da",
    "sicura da",
    "sicuri da",
    "sicure da",
    "ottimizz",
    "sprecat",
    "spazzatur",
    "pulizia intelligente",
];

/// Gli stessi divieti in inglese (RS29: la traduzione conserva il senso di stati e avvisi).
const FORBIDDEN_EN: [&str; 12] = [
    "unused",
    "never used",
    "not used",
    "no longer in use",
    "no longer used",
    "safe to",
    "restore everything",
    "wasted",
    "junk",
    "optimiz",
    "smart clean",
    "intelligent clean",
];

#[test]
fn vocabolario_di_04_completo_nei_due_cataloghi() {
    let mut problems = Vec::new();
    for (tag, forbidden, residue, maybe) in [
        ("it", &FORBIDDEN_IT, ["residu", "residu"], ["possibil", "potrebbe"]),
        ("en", &FORBIDDEN_EN, ["leftover", "residue"], ["possible", "might"]),
    ] {
        let src = embedded(tag).expect("lingua incorporata");
        for (k, v) in messages(src) {
            let low = v.to_lowercase();
            for w in forbidden {
                if low.contains(w) {
                    problems.push(format!("{tag}: {k} contiene «{w}»"));
                }
            }
            // "Residuo" si scrive solo come possibilità (04 §6.8, F-AP-6).
            if residue.iter().any(|r| low.contains(r)) && !maybe.iter().any(|m| low.contains(m)) {
                problems.push(format!("{tag}: {k} usa «residuo» senza dirlo possibile: {v}"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Le chiavi dei cinque stati (03 §6.1, 04 §6.1). `stato-lo-gestisce` ha la variante senza procedura.
const STATE_KEYS: [&str; 6] = ["stato-gestibile", "stato-da-verificare", "stato-decidi-tu", "stato-lo-gestisce", "stato-lo-gestisce-nessuna-procedura", "stato-non-sappiamo"];

#[test]
fn i_cinque_stati_vengono_da_una_sola_funzione() {
    // Nel codice delle viste le chiavi degli stati si leggono solo in `kbtext::state_label` (card,
    // ispettore, conferma, Download, Programmi, Windows e sistema) e nella legenda della lente Download.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("app");
    let mut found = Vec::new();
    for e in std::fs::read_dir(&dir).expect("src/app") {
        let p = e.expect("voce").path();
        let src = std::fs::read_to_string(&p).expect("sorgente");
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        for k in STATE_KEYS {
            if src.contains(&format!("\"{k}\"")) && !matches!(name.as_str(), "kbtext.rs" | "downloads_view.rs") {
                found.push(format!("{name}: {k}"));
            }
        }
    }
    assert!(found.is_empty(), "stati scritti fuori da kbtext::state_label: {found:?}");
    // Nei cataloghi nessun'altra chiave ripete la parola di uno stato: una seconda copia potrebbe cambiare
    // da sola e dare due nomi allo stesso stato.
    for tag in ["it", "en"] {
        let msgs = messages(embedded(tag).expect("lingua"));
        let words: Vec<String> = msgs.iter().filter(|(k, _)| STATE_KEYS.contains(&k.as_str()) && !k.starts_with("stato-lo-gestisce")).map(|(_, v)| v.trim().to_lowercase()).collect();
        assert_eq!(words.len(), 4, "{tag}: stati senza segnaposto");
        for (k, v) in &msgs {
            if !STATE_KEYS.contains(&k.as_str()) && words.contains(&v.trim().to_lowercase()) {
                panic!("{tag}: {k} ripete la parola di uno stato: «{v}»");
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Interfaccia

/// Testi disegnati nell'ultimo frame, con il segno di troncamento.
fn drawn(h: &Harness<'_, App>) -> Vec<(String, bool)> {
    fn walk(s: &egui::epaint::Shape, out: &mut Vec<(String, bool)>) {
        match s {
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            egui::epaint::Shape::Text(t) => out.push((t.galley.text().to_owned(), t.galley.elided)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut out);
    }
    out
}

fn has_text(h: &Harness<'_, App>, s: &str) -> bool {
    drawn(h).iter().any(|(t, _)| t == s)
}

/// Capacità e contenuto del Cestino di C: (valori di prova, nel formato letto da Windows).
const CAPACITY: u64 = 24_380 << 20;
const OTHER_BIN: BinCount = BinCount { items: 1, bytes: 1_318_890 };

/// Letture simulate con lo stesso albero del C: sintetico (come in `tests/intervento.rs`).
fn mirror(tree: &Tree) -> FakeProbe {
    let p = FakeProbe::new().volume("C:\\");
    for n in 1..tree.len() as u32 {
        let path = tree.path_lossy(n);
        if tree.is_dir(n) {
            if tree.flags(n).has(Flags::DENIED) {
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

/// Il C: sintetico con le azioni simulate: niente legge o scrive sul disco vero.
fn capcut_app(lang: &str) -> Harness<'static, App> {
    let (v, r) = synthetic::sample_c(false);
    let probe = mirror(&r.tree);
    probe.bin("C:\\", |b| {
        b.capacity = Some(CAPACITY);
        b.content = Some(OTHER_BIN);
    });
    let mover = FakeMover::new(&probe);
    let mut env = Env::fixed();
    env.windows_languages = vec![if lang == "en" { "en-US".into() } else { "it-IT".into() }];
    let mut actions = Actions::fake(&probe);
    actions.mover = Arc::new(move |_| Box::new(mover.clone()) as Box<dyn Mover>);
    env.actions = actions;
    env.readers = Readers(Arc::new(synthetic::FakeReaders::for_tree(&r.tree)));
    let mut a = App::new(env, Vec::new());
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut h = Harness::builder().with_size([1600.0, 2600.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(4);
    h
}

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

/// Ogni etichetta di stato disegnata ha la forma esatta del catalogo (niente "DA VERIFICARE" o "da
/// verificare" come etichetta). Le frasi che usano le stesse parole nel loro senso comune ("Tuoi file:
/// decidi tu cosa tenere.") non sono etichette di stato.
fn same_state_words(h: &Harness<'_, App>, words: &[String], screen: &str) {
    for (t, _) in drawn(h) {
        for w in words {
            if t.trim().eq_ignore_ascii_case(w) || t.trim().to_lowercase() == w.to_lowercase() {
                assert_eq!(t.trim(), w.as_str(), "{screen}: etichetta di stato in un'altra forma");
            }
        }
    }
}

#[test]
fn i_cinque_stati_hanno_le_stesse_parole_in_card_ispettore_conferma_e_risultato() {
    for lang in ["it", "en"] {
        let mut h = capcut_app(lang);
        let words: Vec<String> = ["stato-gestibile", "stato-da-verificare", "stato-decidi-tu", "stato-non-sappiamo"].iter().map(|k| h.state().lingue.t(k)).collect();
        let to_verify = h.state().lingue.t("stato-da-verificare");
        let m = {
            let d = h.state().data.as_ref().expect("dati");
            d.cls.matches.iter().position(|m| d.kb.rules[m.rule].id == "capcut-versioni").expect("CapCut")
        };
        // La parola viene da `kbtext::state_label`, la stessa per ogni vista.
        {
            let a = h.state();
            let d = a.data.as_ref().expect("dati");
            assert_eq!(kbtext::state_label(&a.lingue, &d.cls.matches[m].record.state), to_verify);
        }
        // Card "Interventi possibili" della Panoramica.
        assert_eq!(h.state().screen, Screen::Overview);
        assert!(has_text(&h, &to_verify), "{lang}: stato nella card");
        same_state_words(&h, &words, "card");
        // Ispettore: una versione precedente di CapCut nell'Esplora.
        {
            let a = h.state_mut();
            a.go(Screen::Explore);
            let d = a.data.as_ref().expect("dati");
            let t = d.tree();
            let old = (0..t.len() as u32).find(|&i| t.name_lossy(i) == "8.1.0.3402").expect("versione");
            a.explore.open(d, t.parent(old), Some(old));
        }
        h.run_steps(4);
        h.state_mut().wait_background();
        h.run_steps(4);
        assert!(has_text(&h, &to_verify), "{lang}: stato nell'ispettore");
        same_state_words(&h, &words, "ispettore");
        // Conferma: il dialogo dopo la simulazione.
        intervention_jobs::dispatch(h.state_mut(), ActRequest::PrepareRecycle, Origin::Rule(m));
        wait(&mut h, |a| !a.intervention.simulating());
        assert!(h.state().intervention.simulation().is_some(), "{lang}: dialogo di conferma");
        assert!(has_text(&h, &to_verify), "{lang}: stato nella conferma");
        same_state_words(&h, &words, "conferma");
        // Risultato: dopo lo spostamento simulato. Il risultato nomina la regola, non ripete lo stato; se una
        // parola di stato compare, è nella forma del catalogo.
        // Da verificare: le caselle partono vuote, «Seleziona tutti» sceglie le versioni da spostare.
        let (all, label) = (h.state().lingue.t("spostamento-seleziona-tutti"), h.state().lingue.t("spostamento-sposta"));
        {
            use egui_kittest::kittest::Queryable as _;
            h.get_by_label(&all).click();
        }
        h.run_steps(3);
        {
            use egui_kittest::kittest::Queryable as _;
            h.get_by_label(&label).click();
        }
        h.run_steps(3);
        wait(&mut h, |a| a.intervention.summary().is_some() && !a.intervention.busy());
        assert_eq!(h.state().screen, Screen::Interventions);
        same_state_words(&h, &words, "risultato");
    }
}

/// Un volume vuoto: nessun file, nessuna cartella (stati vuoti di Esplora e File grandi).
fn empty_volume() -> ScanResult {
    let mut tree = Tree::with_root(&"C:\\".encode_utf16().collect::<Vec<u16>>(), 0);
    tree.aggregate();
    tree.seal();
    ScanResult { tree, links: Links::default(), problems: Vec::new(), stats: Stats::default(), volume: None, balance: None, cancelled: false }
}

/// App in pseudo-lingua, senza volumi.
fn pseudo_app() -> App {
    let mut env = Env::fixed();
    env.windows_languages = vec!["it-IT".into()];
    let mut a = App::new(env, Vec::new());
    a.set_language(PSEUDO);
    a
}

/// Il testo della chiave, pseudo-localizzato, è disegnato intero.
fn shown_whole(h: &Harness<'_, App>, key: &str) {
    let want = h.state().lingue.t(key);
    assert!(want.starts_with('[') && want.contains('~'), "{key}: non è pseudo-localizzato: {want}");
    let all = drawn(h);
    let hit = all.iter().find(|(t, _)| *t == want);
    assert!(hit.is_some(), "{key}: «{want}» non è disegnato; testi: {:?}", all.iter().map(|(t, _)| t).collect::<Vec<_>>());
    assert!(hit.is_some_and(|(_, elided)| !elided), "{key}: testo tagliato");
}

#[test]
fn gli_stati_vuoti_di_m6_vengono_dal_catalogo_e_non_si_tagliano() {
    // Volumi: nessun volume con una lettera di unità.
    let mut h = Harness::builder().with_size([1280.0, 800.0]).build_ui_state(|ui, app: &mut App| app.show(ui), pseudo_app());
    h.run_steps(4);
    assert_eq!(h.state().screen, Screen::Volumes);
    shown_whole(&h, "volumi-nessuno");

    // Volume vuoto: Esplora senza righe, File grandi senza file e senza filtri.
    let mut a = pseudo_app();
    a.load_result(synthetic::volume_c(), empty_volume(), Duration::from_secs(1));
    let mut h = Harness::builder().with_size([1280.0, 800.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(4);
    h.state_mut().go(Screen::Explore);
    h.run_steps(4);
    shown_whole(&h, "esplora-vuoto");
    h.state_mut().go(Screen::BigFiles);
    wait(&mut h, |a| !a.big.pending());
    shown_whole(&h, "file-grandi-vuoto");
    // Una dimensione minima che non è un numero: lo si dice sotto i filtri.
    h.state_mut().big.filters.min_text = "abc".into();
    h.run_steps(3);
    shown_whole(&h, "filtro-dimensione-non-valida");

    // Cartella non letta (accesso negato) aperta nell'Esplora.
    let mut a = pseudo_app();
    let (v, r) = synthetic::sample_c(false);
    a.load_result(v, r, Duration::from_secs(1));
    let mut h = Harness::builder().with_size([1280.0, 800.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(4);
    {
        let a = h.state_mut();
        a.go(Screen::Explore);
        let d = a.data.as_ref().expect("dati");
        let t = d.tree();
        let denied = (0..t.len() as u32).find(|&i| t.is_dir(i) && t.flags(i).has(Flags::DENIED)).expect("cartella negata");
        a.explore.open(d, denied, None);
    }
    h.run_steps(4);
    shown_whole(&h, "esplora-vuoto-non-letta");

    // Ricerca senza risultati, senza filtri.
    h.input_mut().events.push(Event::Key { key: Key::F, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND });
    h.input_mut().events.push(Event::Key { key: Key::F, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::COMMAND });
    h.run_steps(2);
    h.input_mut().events.push(Event::Text("nessun file ha questo nome".into()));
    h.run_steps(2);
    for _ in 0..300 {
        h.step();
        if !h.state().search.pending() && h.state().search.outcome.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(2);
    shown_whole(&h, "ricerca-nessun-risultato");

    // Regole: la base incorporata non si è caricata.
    let mut a = pseudo_app();
    a.kb = Arc::new(quado_kb::Kb { rules: Vec::new() });
    let mut h = Harness::builder().with_size([1280.0, 800.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(3);
    h.state_mut().go(Screen::Rules);
    h.run_steps(3);
    shown_whole(&h, "regole-vuoto");
}

#[test]
fn fine_scansione_dice_durata_e_cartelle_negate() {
    // 04 §6.8 "Fine scansione": durata, numero di cartelle con l'accesso negato da Windows, il loro
    // contenuto nel non attribuito.
    let mut env = Env::fixed();
    env.windows_languages = vec!["it-IT".into()];
    let mut a = App::new(env, Vec::new());
    let (v, r) = synthetic::sample_c(false);
    a.load_result(v, r, Duration::from_secs(14));
    let d = a.data.as_ref().expect("dati");
    let notice = volumes_view::completion_notice(&a.lingue, &a.fmt, d).expect("avviso");
    assert_eq!(notice, "Scansione completata in 14,0 s. Windows ha negato l'accesso a 4 cartelle: il loro contenuto resta nel non attribuito.");
    // Una scansione annullata non è "completata": vale il banner "Scansione interrotta".
    let (v, r) = synthetic::sample_c(true);
    a.load_result(v, r, Duration::from_secs(3));
    assert!(volumes_view::completion_notice(&a.lingue, &a.fmt, a.data.as_ref().expect("dati")).is_none());
}

#[test]
fn la_barra_di_stato_dice_come_e_avviato_quado() {
    // Con UAC anche un amministratore avvia i programmi senza elevazione: la voce a destra della barra di
    // stato parla di Quado (docs/testi.md §6.1) e cambia se Quado è avviato come amministratore. Nella
    // finestra più stretta (960 px, `with_min_inner_size` di main.rs) resta intera.
    for lang in ["it", "en"] {
        for elevated in [false, true] {
            let mut env = Env::fixed();
            env.elevated = elevated;
            env.windows_languages = vec![if lang == "en" { "en-US".into() } else { "it-IT".into() }];
            let mut a = App::new(env, Vec::new());
            let (v, r) = synthetic::sample_c(false);
            a.load_result(v, r, Duration::from_secs(14));
            let mut h = Harness::builder().with_size([960.0, 600.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
            h.run_steps(4);
            assert_eq!(h.state().screen, Screen::Overview);
            let (a, all) = (h.state(), drawn(&h));
            let (l, f) = (&a.lingue, &a.fmt);
            let (yes, no) = if elevated { ("stato-amministratore", "stato-senza-privilegi") } else { ("stato-senza-privilegi", "stato-amministratore") };
            let (yes, no) = (l.t(yes), l.t(no));
            assert!(all.iter().any(|(t, elided)| *t == yes && !elided), "{lang}, elevato {elevated}: «{yes}» non è disegnato intero");
            assert!(!all.iter().any(|(t, _)| *t == no), "{lang}, elevato {elevated}: «{no}» non deve comparire");
            // La riga della scansione nella testata della Panoramica dice lo stesso.
            let d = a.data.as_ref().expect("dati");
            let key = if elevated { "panoramica-scansione-amministratore" } else { "panoramica-scansione" };
            let scan = l.ta(key, &[("data", f.date(d.finished).into()), ("ora", f.time(d.finished).into()), ("durata", f.seconds(d.duration.as_secs_f64()).into())]);
            assert!(all.iter().any(|(t, _)| t.ends_with(&scan)), "{lang}, elevato {elevated}: manca «{scan}»");
        }
    }
}

#[test]
fn fa_parte_di_e_la_riga_della_regola_stanno_su_due_righe() {
    // La riga della regola ha spesso già i suoi due punti ("Cache di CapCut: la svuoti…"): accodata a "Fa
    // parte di X:" ne darebbe due di fila. Nell'ispettore sono due righe separate.
    let mut h = capcut_app("it");
    let (part_of, line) = {
        let a = h.state_mut();
        a.go(Screen::Explore);
        let d = a.data.as_ref().expect("dati");
        let t = d.tree();
        let version = (0..t.len() as u32).find(|&i| t.name_lossy(i) == "8.1.0.3402").expect("versione");
        let inner = t.children(version).next().expect("file nella versione");
        let m = d.cls.match_of(inner).expect("riconosciuto");
        assert_ne!(d.cls.own(inner), Some(m), "l'elemento sta dentro quello riconosciuto");
        let part_of = a.lingue.ta("ispettore-fa-parte-di", &[("nome", t.name_lossy(version).into())]);
        let line = kbtext::rule_line(&a.lingue, &a.fmt, &d.kb, &d.cls.matches[m]);
        a.explore.open(d, version, Some(inner));
        (part_of, line)
    };
    h.run_steps(4);
    h.state_mut().wait_background();
    h.run_steps(4);
    assert!(has_text(&h, &part_of), "«{part_of}»");
    assert!(has_text(&h, &line), "«{line}»");
    assert!(drawn(&h).iter().all(|(t, _)| !t.starts_with(&format!("{part_of}:"))), "riga della regola accodata");
}

#[test]
fn l_id_di_un_gioco_di_steam_si_scrive_senza_separatore() {
    // È un identificativo: "ID app 3999950", come in Steam, mai "ID app 3.999.950".
    let (_, r) = synthetic::sample_c(false);
    let kb = quado_kb::Kb::embedded().expect("regole");
    let ev = synthetic::sample_c_evidence();
    let cls = quado_kb::classify(&kb, &r.tree, &ev, quado_model::record::Date { year: 2026, month: 10, day: 13 });
    let game = quado_model::record::Value::Text("Quillfire".into());
    let fact = cls.matches.iter().flat_map(|m| m.record.identity.0.iter()).find(|f| f.key == "fatto-gioco-steam" && f.values.first() == Some(&game)).expect("Quillfire");
    let f = quado_ui::format::Formatter::default();
    for lang in ["it", "en"] {
        let l = quado_ui::i18n::Lingue::load(lang, None);
        let s = kbtext::fact_text(&l, &f, fact);
        assert!(s.ends_with(" 3999950)"), "{lang}: {s}");
    }
}

// ------------------------------------------------------------------------------------------------
// Core

/// Corpo di una definizione `pub struct`/`pub enum` che comincia a `start`, fino alla graffa chiusa.
fn body(src: &str, start: usize) -> &str {
    let open = src[start..].find(['{', ';', '(']).map_or(src.len(), |k| start + k);
    if src[open..].starts_with(';') {
        return &src[start..open];
    }
    let (o, c) = if src[open..].starts_with('(') { ('(', ')') } else { ('{', '}') };
    let mut depth = 0;
    for (k, ch) in src[open..].char_indices() {
        if ch == o {
            depth += 1;
        } else if ch == c {
            depth -= 1;
            if depth == 0 {
                return &src[start..open + k + 1];
            }
        }
    }
    &src[start..]
}

#[test]
fn la_base_delle_regole_restituisce_codici_non_frasi() {
    // 05 D13, 06 §7: come `core_types_carry_codes_not_sentences` per model, scan-win e actions. I tipi di
    // errore, problema ed esito di `quado-kb` portano codici e valori; un percorso è un dato.
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("quado-kb").join("src");
    let mut stack = vec![src_dir];
    let mut files = Vec::new();
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).expect("cartella dei sorgenti").flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    assert!(files.len() > 5, "sorgenti trovati: {}", files.len());
    let mut problems = Vec::new();
    for f in files {
        let src = std::fs::read_to_string(&f).expect("sorgente");
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        for bad in ["impl std::fmt::Display", "impl fmt::Display", "impl Display", "impl std::error::Error", "impl Error for"] {
            if src.contains(bad) {
                problems.push(format!("{name}: {bad}"));
            }
        }
        for kw in ["pub enum ", "pub struct "] {
            let mut from = 0;
            while let Some(k) = src[from..].find(kw) {
                let start = from + k;
                let ident: String = src[start + kw.len()..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                from = start + kw.len();
                let status = ["Error", "Issue", "Problem", "Outcome"].iter().any(|s| ident.ends_with(s));
                if !status {
                    continue;
                }
                let b = body(&src, start);
                let texts = b.lines().filter(|l| l.contains("String") || l.contains("&str")).filter(|l| !l.contains("path")).count();
                if texts > 0 {
                    problems.push(format!("{name}: {ident} contiene testo"));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
