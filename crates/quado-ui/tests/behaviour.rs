// SPDX-License-Identifier: GPL-3.0-or-later
//! Comportamenti dell'interfaccia provati con `egui_kittest`, senza mouse.
//!
//! - Ogni azione dell'Esplora da tastiera (frecce, Invio, Backspace, Alt+frecce, Ctrl+F, `*`).
//! - Banner "Scansione interrotta" su ogni schermata con dati parziali.
//! - Cambio di lingua senza riavvio; file dell'utente con una riga sbagliata.
//! - Con "Effetti di animazione" spenti nessuna animazione.
//! - Barra laterale, dimensione del testo, densità, accento, colori dei dati, vista iniziale e colonne di
//!   Esplora valgono subito.

use std::path::PathBuf;
use std::time::Duration;

use egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use quado_model::Tree;
use quado_ui::app::{App, Env, Screen, synthetic};

fn app(lang: &str) -> App {
    let mut env = Env::fixed();
    env.windows_languages = vec![if lang == "en" { "en-US".into() } else { "it-IT".into() }];
    App::new(env, Vec::new())
}

fn harness(app: App) -> Harness<'static, App> {
    Harness::builder().with_size([1600.0, 1000.0]).build_ui_state(|ui, app: &mut App| app.show(ui), app)
}

fn loaded(cancelled: bool, lang: &str) -> Harness<'static, App> {
    let mut a = app(lang);
    let (v, r) = synthetic::sample_c(cancelled);
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut h = harness(a);
    h.run_steps(4);
    h
}

/// Un tasto premuto e rilasciato nello stesso frame, poi due frame.
fn press(h: &mut Harness<'_, App>, key: Key, modifiers: Modifiers) {
    h.input_mut().events.push(Event::ModifiersChanged(modifiers));
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
    h.step();
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.step();
}

fn key(h: &mut Harness<'_, App>, k: Key) {
    press(h, k, Modifiers::NONE);
}

fn text(h: &mut Harness<'_, App>, s: &str) {
    h.input_mut().events.push(Event::Text(s.to_owned()));
    h.step();
    h.step();
}

fn name(h: &Harness<'_, App>, n: u32) -> String {
    h.state().data.as_ref().expect("dati").tree().name_lossy(n)
}

fn wait_jobs(h: &mut Harness<'_, App>) {
    for _ in 0..300 {
        h.step();
        let a = h.state();
        if !a.search.pending() && !a.big.pending() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(2);
}

#[test]
fn explore_is_fully_usable_from_the_keyboard() {
    let mut h = loaded(false, "it");
    // Ctrl+2: Esplora, con la tabella pronta per i tasti.
    press(&mut h, Key::Num2, Modifiers::COMMAND);
    assert_eq!(h.state().screen, Screen::Explore);
    // Prima riga: la cartella più grande della radice.
    key(&mut h, Key::ArrowDown);
    let first = h.state().explore.selected.expect("selezione");
    assert_eq!(name(&h, first), "Users");
    // Freccia giù e su.
    key(&mut h, Key::ArrowDown);
    let second = h.state().explore.selected.expect("selezione");
    assert_ne!(second, first);
    key(&mut h, Key::ArrowUp);
    assert_eq!(h.state().explore.selected, Some(first));
    // Freccia destra: espande; ancora destra: entra nel primo figlio.
    key(&mut h, Key::ArrowRight);
    assert!(h.state().explore.outline.is_expanded(first));
    key(&mut h, Key::ArrowRight);
    let child = h.state().explore.selected.expect("figlio");
    assert_eq!(name(&h, child), "demo");
    // Freccia sinistra: torna al genitore; ancora sinistra: lo chiude.
    key(&mut h, Key::ArrowLeft);
    assert_eq!(h.state().explore.selected, Some(first));
    key(&mut h, Key::ArrowLeft);
    assert!(!h.state().explore.outline.is_expanded(first));
    // Invio: apre la cartella; Backspace: torna su; Alt+sinistra e Alt+destra: indietro e avanti.
    key(&mut h, Key::Enter);
    assert_eq!(h.state().explore.outline.root, first);
    key(&mut h, Key::Backspace);
    assert_eq!(h.state().explore.outline.root, Tree::ROOT);
    assert_eq!(h.state().explore.selected, Some(first), "dopo Backspace resta selezionata la cartella da cui si esce");
    press(&mut h, Key::ArrowLeft, Modifiers::ALT);
    assert_eq!(h.state().explore.outline.root, first);
    press(&mut h, Key::ArrowRight, Modifiers::ALT);
    assert_eq!(h.state().explore.outline.root, Tree::ROOT);
    // Fine e Inizio, Pagina giù.
    key(&mut h, Key::End);
    let rows = h.state().explore.outline.len();
    assert_eq!(h.state().explore.outline.row_of(h.state().explore.selected.expect("sel")), Some(rows - 1));
    key(&mut h, Key::Home);
    assert_eq!(h.state().explore.selected, Some(first));
    // `*`: espande tutto.
    text(&mut h, "*");
    let t_len = h.state().data.as_ref().expect("dati").tree().len();
    assert!(h.state().explore.outline.len() > t_len / 2, "espansione completa");

    // Ctrl+F e un testo: risultati della ricerca; Invio porta il focus ai risultati, Invio mostra l'elemento.
    press(&mut h, Key::F, Modifiers::COMMAND);
    text(&mut h, "Projects");
    wait_jobs(&mut h);
    let hits = h.state().search.outcome.as_ref().map_or(0, |o| o.hits.len());
    assert_eq!(hits, 1);
    key(&mut h, Key::Enter);
    wait_jobs(&mut h);
    key(&mut h, Key::ArrowDown);
    assert_eq!(h.state().search.selected, Some(0));
    key(&mut h, Key::Enter);
    assert!(!h.state().search.active(), "la ricerca si chiude");
    let sel = h.state().explore.selected.expect("elemento trovato");
    assert_eq!(name(&h, sel), "Projects");
    assert_eq!(name(&h, h.state().explore.outline.root), "User Data");

    // Tab porta il focus fuori dalla tabella; si torna con Maiusc+Tab. Nessun tasto resta senza effetto
    // visibile: l'anello del focus è disegnato (controllato negli screenshot).
    key(&mut h, Key::Tab);
    press(&mut h, Key::Tab, Modifiers::SHIFT);
}

#[test]
fn big_files_and_overview_lead_to_explore_from_the_keyboard() {
    let mut h = loaded(false, "it");
    press(&mut h, Key::Num3, Modifiers::COMMAND);
    assert_eq!(h.state().screen, Screen::BigFiles);
    wait_jobs(&mut h);
    key(&mut h, Key::ArrowDown);
    assert_eq!(h.state().big.selected, Some(0));
    let top = h.state().big.outcome.as_ref().expect("elenco").files[0];
    assert_eq!(name(&h, top), "hiberfil.sys");
    key(&mut h, Key::Enter);
    assert_eq!(h.state().screen, Screen::Explore);
    assert_eq!(h.state().explore.selected, Some(top));
    assert_eq!(h.state().explore.outline.root, Tree::ROOT);

    // Panoramica: Tab fino alla prima riga di "Dove finisce lo spazio", Invio apre l'Esplora lì.
    press(&mut h, Key::Num1, Modifiers::COMMAND);
    assert_eq!(h.state().screen, Screen::Overview);
    let first_top = h.state().data.as_ref().expect("dati").top[0];
    let label = name(&h, first_top);
    let mut reached = false;
    for _ in 0..40 {
        key(&mut h, Key::Tab);
        if h.query_by(|n| n.is_focused() && n.label().as_deref() == Some(label.as_str())).is_some() {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab non arriva alla prima riga di «Dove finisce lo spazio»");
    key(&mut h, Key::Enter);
    assert_eq!(h.state().screen, Screen::Explore);
    assert_eq!(h.state().explore.selected, Some(first_top));
}

#[test]
fn interrupted_scan_banner_on_every_data_screen() {
    let mut h = loaded(true, "it");
    for s in [Screen::Overview, Screen::Explore, Screen::BigFiles] {
        h.state_mut().go(s);
        h.run_steps(3);
        assert!(h.query_by_label_contains("Scansione interrotta").is_some(), "{s:?}: manca il banner");
    }
    let mut ok = loaded(false, "it");
    ok.run_steps(2);
    assert!(ok.query_by_label_contains("Scansione interrotta").is_none());
}

#[test]
fn language_changes_without_restart_and_bad_user_lines_are_reported() {
    let dir = std::env::temp_dir().join(format!("quado-ui-lingue-{}", std::process::id()));
    let lingue = dir.join("lingue");
    std::fs::create_dir_all(&lingue).expect("cartella");
    std::fs::write(lingue.join("it.ftl"), "nav-esplora = Esplora tutto\nquesta riga è sbagliata\n").expect("file");
    let mut env = Env::fixed();
    env.data_dir = Some(dir.clone());
    let mut a = App::new(env, Vec::new());
    let (v, r) = synthetic::sample_c(false);
    a.load_result_with(v, r, Duration::from_secs(1), synthetic::sample_c_evidence());
    let mut h = harness(a);
    h.run_steps(3);
    assert!(h.query_by_label("Esplora tutto").is_some(), "la correzione dell'utente vale");
    h.state_mut().go(Screen::Settings);
    h.run_steps(3);
    assert!(h.query_by_label_contains("it.ftl:2").is_some(), "l'errore compare in Impostazioni");
    // Cambio lingua: subito, senza riavvio.
    h.state_mut().set_language("en");
    h.run_steps(3);
    assert!(h.query_all_by_label("Settings").count() >= 2, "titolo e barra laterale in inglese");
    // "Explore" compare nella barra laterale, nell'indice di Impostazioni e come titolo della sua sezione.
    assert!(h.query_all_by_label("Explore").count() >= 2);
    std::fs::remove_dir_all(&dir).ok();
    let _ = PathBuf::new();
}

#[test]
fn no_animations_when_windows_animations_are_off() {
    let mut env = Env::fixed();
    env.animations = false;
    let mut h = harness(App::new(env, Vec::new()));
    h.run_steps(3);
    assert_eq!(h.ctx.global_style().animation_time, 0.0);
    let mut env = Env::fixed();
    env.animations = true;
    let mut h = harness(App::new(env, Vec::new()));
    h.run_steps(3);
    assert!(h.ctx.global_style().animation_time > 0.0);
}

#[test]
fn table_and_map_select_the_same_item() {
    let mut h = loaded(false, "it");
    h.state_mut().go(Screen::Explore);
    h.run_steps(3);
    // Riga → mappa: la riga selezionata da tastiera ha la sua area evidenziata.
    key(&mut h, Key::ArrowDown);
    let sel = h.state().explore.selected.expect("selezione");
    {
        let a = h.state();
        let t = a.data.as_ref().expect("dati").tree();
        let k = quado_ui::app::explore::selected_tile(&a.explore.map.layout, t, sel).expect("area");
        assert_eq!(a.explore.map.layout.tiles[k].node, sel);
    }
    // Mappa → riga: un clic su un'area di secondo livello seleziona quel nodo e lo rivela nella tabella.
    let (pos, node) = {
        let a = h.state();
        let tile = a.explore.map.layout.tiles.iter().find(|t| t.depth >= 2 && t.leaf && t.rect.width() > 20.0 && t.rect.height() > 20.0).expect("area di secondo livello");
        (tile.rect.center(), tile.node)
    };
    h.input_mut().events.push(Event::PointerMoved(pos));
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.input_mut().events.push(Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.step();
    h.step();
    assert_eq!(h.state().explore.selected, Some(node));
    assert!(h.state().explore.outline.row_of(node).is_some(), "il nodo cliccato è una riga della tabella");
}

#[test]
fn si_units_change_every_size_shown() {
    let mut h = loaded(false, "it");
    h.run_steps(2);
    let texts = |h: &Harness<'_, App>| -> Vec<String> {
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
    };
    let before = texts(&h).join("\n");
    assert!(before.contains("1024³"));
    h.state_mut().settings.units = quado_ui::format::Units::Si;
    h.state_mut().fmt.units = quado_ui::format::Units::Si;
    h.run_steps(2);
    let after = texts(&h).join("\n");
    assert!(after.contains("10⁹"), "la nota delle unità cambia");
    assert_ne!(before, after);
    // hiberfil.sys, 6 GiB e 410 MiB: 6,40 GB come Esplora, 6,87 GB in SI.
    h.state_mut().go(Screen::BigFiles);
    wait_jobs(&mut h);
    let all = texts(&h).join("\n");
    let kb: Vec<&str> = all.lines().filter(|l| l.contains(" KB")).collect();
    assert!(all.contains("6,87 GB") && !all.contains("6,40 GB") && kb.is_empty(), "dimensioni SI in File grandi; con KB: {kb:?}; GB: {:?}", all.lines().filter(|l| l.contains("GB")).take(8).collect::<Vec<_>>());
}

// ---------------------------------------------------------------------------------------------
// Classificazione nell'interfaccia.

fn node_named(h: &Harness<'_, App>, n: &str) -> u32 {
    let t = h.state().data.as_ref().expect("dati").tree();
    (0..t.len() as u32).find(|&i| t.name_lossy(i) == n).unwrap_or_else(|| panic!("{n}"))
}

#[test]
fn system_view_and_rules_from_the_keyboard() {
    // Ctrl+4 apre Windows e sistema; Tab e Invio su una voce la mostrano in Esplora.
    let mut h = loaded(false, "it");
    press(&mut h, Key::Num4, Modifiers::COMMAND);
    assert_eq!(h.state().screen, Screen::System);
    let label = h.state().lingue.t("sistema-voce-ibernazione");
    for _ in 0..40 {
        if h.query_by(|n| n.is_focused() && n.label().is_some_and(|l| l == label)).is_some() {
            break;
        }
        key(&mut h, Key::Tab);
    }
    key(&mut h, Key::Enter);
    h.run_steps(2);
    assert_eq!(h.state().screen, Screen::Explore);
    let hib = node_named(&h, "hiberfil.sys");
    assert_eq!(h.state().explore.selected, Some(hib));
}

#[test]
fn inspector_record_leads_to_its_rule() {
    // Una versione precedente di CapCut: record Da verificare con la regola e "Apri la regola".
    // Finestra alta: l'ispettore intero è visibile senza scorrere.
    let mut a = app("it");
    let (v, r) = synthetic::sample_c(false);
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut h = Harness::builder().with_size([1600.0, 2600.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(4);
    let old = node_named(&h, "8.1.0.3402");
    {
        let a = h.state_mut();
        a.go(Screen::Explore);
        let d = a.data.as_ref().expect("dati");
        let apps = d.tree().parent(old);
        a.explore.open(d, apps, Some(old));
    }
    h.run_steps(4);
    // Le misure dell'ispettore arrivano in background e allungano il pannello: si aspettano prima di
    // cercare il pulsante, se no sotto carico il clic cade dove il pulsante era un frame prima.
    h.state_mut().wait_background();
    h.run_steps(4);
    let state = h.state().lingue.t("stato-da-verificare");
    assert!(h.query_by_label_contains(&state).is_some(), "lo stato Da verificare compare nell'ispettore");
    let see = h.state().lingue.t("ispettore-vedi-regola");
    h.get_by_label(&see).click();
    h.run_steps(3);
    assert_eq!(h.state().screen, Screen::Rules);
    let idx = h.state().kb.index("capcut-versioni");
    assert_eq!(h.state().rules_view.selected, idx);
}

#[test]
fn interventions_card_opens_the_item() {
    // La voce di CapCut nella card porta all'elemento in Esplora.
    let mut h = loaded(false, "it");
    h.run_steps(2);
    let line = {
        let a = h.state();
        let d = a.data.as_ref().expect("dati");
        let m = d.cls.matches.iter().find(|m| d.kb.rules[m.rule].id == "capcut-versioni").expect("CapCut");
        quado_ui::app::kbtext::rule_line(&a.lingue, &a.fmt, &d.kb, m)
    };
    for _ in 0..80 {
        if h.query_by(|n| n.is_focused() && n.label().is_some_and(|l| l == line)).is_some() {
            break;
        }
        key(&mut h, Key::Tab);
    }
    key(&mut h, Key::Enter);
    h.run_steps(2);
    assert_eq!(h.state().screen, Screen::Explore);
    let sel = h.state().explore.selected.expect("selezione");
    let d = h.state().data.as_ref().expect("dati");
    assert_eq!(d.kb.rules[d.cls.matches[d.cls.own(sel).expect("elemento")].rule].id, "capcut-versioni");
}

#[test]
fn manager_filter_matches_the_classification() {
    // Il filtro per gestore restituisce lo stesso insieme di una query di riferimento.
    let mut h = loaded(false, "it");
    {
        let a = h.state_mut();
        a.search.filters.group = Some(quado_ui::tokens::Gestore::Mine);
        a.search.filter.group = Some(quado_ui::tokens::Gestore::Mine as u8);
        a.search.set_query("a");
        a.go(Screen::Explore);
    }
    wait_jobs(&mut h);
    let a = h.state();
    let d = a.data.as_ref().expect("dati");
    let t = d.tree();
    let hits: std::collections::BTreeSet<u32> = a.search.outcome.as_ref().expect("risultati").hits.iter().copied().collect();
    let want: std::collections::BTreeSet<u32> = (1..t.len() as u32)
        .filter(|&i| t.name_lossy(i).to_lowercase().contains('a'))
        .filter(|&i| !t.flags(i).has(quado_model::Flags::SECONDARY_LINK))
        .filter(|&i| quado_ui::app::data::group_code(&d.cls, i) == quado_ui::tokens::Gestore::Mine as u8)
        .collect();
    assert!(!want.is_empty());
    assert_eq!(hits, want);
}

// ---------------------------------------------------------------------------------------------
// Schermata Programmi. Il lanciatore e "Apri in Esplora file" sono finti: registrano le chiamate nel thread
// del test, senza aprire Impostazioni, Steam o Esplora file.

mod programmi {
    use std::cell::RefCell;
    use std::time::Duration;

    use egui::accesskit::Role;
    use egui::{Key, Modifiers};
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT, Queryable};
    use quado_actions::launch::{Launch, Pfn, SteamAppId};
    use quado_actions::shell::ShellError;
    use quado_kb::extract::TrackProgs;
    use quado_kb::programs::{Location, ProgramSource, Space, Usage};
    use quado_ui::app::programs_view::{Outcome, Section};
    use quado_ui::app::{App, Screen, kbtext, synthetic};

    use super::{app, key, press};

    thread_local! {
        static LAUNCHED: RefCell<Vec<Launch>> = const { RefCell::new(Vec::new()) };
        static REVEALED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }

    fn fake_launcher(l: &Launch) -> Result<(), ShellError> {
        LAUNCHED.with(|v| v.borrow_mut().push(l.clone()));
        Ok(())
    }

    fn fake_reveal(path: &str) -> Result<(), ShellError> {
        REVEALED.with(|v| v.borrow_mut().push(path.to_owned()));
        Ok(())
    }

    fn launched() -> Vec<Launch> {
        LAUNCHED.with(|v| v.borrow().clone())
    }

    fn revealed() -> Vec<String> {
        REVEALED.with(|v| v.borrow().clone())
    }

    /// Dati di prova con le prove degli estrattori, lanciatore finto, schermata aperta con Ctrl+6.
    fn programs(lang: &str, height: f32) -> Harness<'static, App> {
        let mut a = app(lang);
        let (v, r) = synthetic::sample_c(false);
        a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
        a.programs.launcher = fake_launcher;
        a.programs.reveal = fake_reveal;
        let mut h = Harness::builder().with_size([1600.0, height]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
        h.run_steps(4);
        press(&mut h, Key::Num6, Modifiers::COMMAND);
        assert_eq!(h.state().screen, Screen::Programs);
        h
    }

    fn index(h: &Harness<'_, App>, name: &str) -> usize {
        h.state().data.as_ref().expect("dati").programs.items.iter().position(|x| x.name == name).unwrap_or_else(|| panic!("{name}"))
    }

    fn select(h: &mut Harness<'_, App>, name: &str) {
        let i = index(h, name);
        h.state_mut().programs.selected = Some(i);
        h.run_steps(3);
    }

    fn t(h: &Harness<'_, App>, k: &str) -> String {
        h.state().lingue.t(k)
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

    /// Testo di un fatto del record di un programma, come lo scrive il dettaglio.
    fn fact_text(h: &Harness<'_, App>, name: &str, key: &str) -> String {
        let a = h.state();
        let x = &a.data.as_ref().expect("dati").programs.items[index(h, name)];
        let fa = x.record.identity.0.iter().find(|f| f.key == key).unwrap_or_else(|| panic!("{name}: {key}"));
        kbtext::fact_with_source(&a.lingue, &a.fmt, fa)
    }

    /// Testo di un fatto senza la fonte (avviso dei tuoi file).
    fn plain_fact(h: &Harness<'_, App>, name: &str, key: &str) -> String {
        let a = h.state();
        let x = &a.data.as_ref().expect("dati").programs.items[index(h, name)];
        let fa = x.record.identity.0.iter().find(|f| f.key == key).unwrap_or_else(|| panic!("{name}: {key}"));
        kbtext::fact_text(&a.lingue, &a.fmt, fa)
    }

    /// Un nodo con questo testo nel pannello del dettaglio.
    fn in_detail(h: &Harness<'_, App>, label: &str) -> bool {
        let detail = h.state().programs.detail;
        h.query_all_by_label(label).any(|n| detail.contains_rect(n.rect()))
    }

    fn click(h: &mut Harness<'_, App>, label: &str) {
        h.get_by_label(label).click();
        h.step();
        h.run_steps(2);
    }

    /// Clic sull'elemento con il testo di una chiave.
    fn click_t(h: &mut Harness<'_, App>, key: &str) {
        let label = t(h, key);
        click(h, &label);
    }

    #[test]
    fn list_from_the_sample_evidence() {
        let mut h = programs("it", 1000.0);
        let names = |h: &Harness<'_, App>| -> Vec<String> {
            let a = h.state();
            let p = &a.data.as_ref().expect("dati").programs;
            a.programs.rows.iter().map(|&i| p.items[i].name.clone()).collect()
        };
        let mut got = names(&h);
        got.sort();
        let mut want: Vec<String> = [
            "7-Zip 24.09 (x64 edition)",
            "Autodesk Fusion",
            "CapCut",
            "Brinewood",
            "Dune Rally 2",
            "Quillfire",
            "Hollow Peaks",
            "Microsoft.WindowsTerminal_8wekyb3d8bbwe",
            "NVIDIA Graphics Driver 617.42",
            "Steam",
            "Unreal Engine",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        want.sort();
        assert_eq!(got, want, "programmi dai dati di prova, senza il componente di sistema");
        // Ordine per spazio misurato decrescente, poi i non misurati e quelli senza spazio.
        {
            let a = h.state();
            let p = &a.data.as_ref().expect("dati").programs;
            let key = |i: usize| match p.items[i].space {
                Space::Measured(b) => (0u8, u64::MAX - b),
                Space::NotMeasured => (1, 0),
                Space::NotAttributed => (2, 0),
            };
            assert!(a.programs.rows.windows(2).all(|w| key(w[0]) <= key(w[1])), "ordine per spazio misurato");
            assert_eq!(p.items[a.programs.rows[0]].name, "Hollow Peaks");
        }
        // Componente di sistema: nascosto per default, visibile con l'interruttore.
        let redist = "Microsoft Visual C++ 2022 X64 Additional Runtime";
        assert!(!names(&h).iter().any(|n| n == redist));
        let chip = h.state().lingue.ta("programmi-mostra-componenti", &[("testo", "1".into())]);
        click(&mut h, &chip);
        assert!(h.state().programs.show_hidden);
        assert!(names(&h).iter().any(|n| n == redist), "visibile con l'interruttore");
        click(&mut h, &chip);
        assert!(!names(&h).iter().any(|n| n == redist), "di nuovo nascosto");

        // NVIDIA senza percorso: "posizione non dichiarata" e nessuna cifra.
        h.state_mut().programs.query = "nvidia".into();
        h.run_steps(2);
        assert_eq!(names(&h), vec!["NVIDIA Graphics Driver 617.42".to_owned()]);
        let nv = index(&h, "NVIDIA Graphics Driver 617.42");
        {
            let x = &h.state().data.as_ref().expect("dati").programs.items[nv];
            assert_eq!((x.location.clone(), x.space), (Location::NotDeclared, Space::NotAttributed));
        }
        assert!(drawn(&h).contains(&t(&h, "programmi-cella-non-dichiarata")), "cella dello spazio");
        select(&mut h, "NVIDIA Graphics Driver 617.42");
        assert!(in_detail(&h, &t(&h, "programmi-titolo-non-dichiarata")));
        assert!(h.query_by_label(&fact_text(&h, "NVIDIA Graphics Driver 617.42", "fatto-programma-nessuno-spazio")).is_some());
        assert!(h.query_by_label_contains("Dimensioni su disco:").is_none(), "nessuno spazio misurato");

        // 7-Zip: misurato e dichiarato separati, il dichiarato con la sua fonte.
        h.state_mut().programs.query = "7-zip".into();
        h.run_steps(2);
        let (measured, declared) = {
            let a = h.state();
            let x = &a.data.as_ref().expect("dati").programs.items[index(&h, "7-Zip 24.09 (x64 edition)")];
            let d = x.declared.as_ref().expect("dichiarato");
            assert_ne!(d.bytes, x.space.bytes());
            let cell = a.lingue.ta("programmi-cella-dichiarata", &[("dimensione", a.fmt.size(d.bytes).into()), ("fonte", a.lingue.t("programmi-dichiarata-registro").into())]);
            (a.fmt.size(x.space.bytes()), cell)
        };
        let texts = drawn(&h);
        assert!(texts.contains(&measured) && texts.contains(&declared), "{measured} e {declared} nella riga");
        assert_ne!(measured, declared);
        select(&mut h, "7-Zip 24.09 (x64 edition)");
        let declared_fact = fact_text(&h, "7-Zip 24.09 (x64 edition)", "fatto-dimensione-dichiarata");
        assert!(declared_fact.contains(&t(&h, "origine-disinstallazione-hklm")), "{declared_fact}");
        assert!(h.query_by_label(&declared_fact).is_some());
        assert!(h.query_by_label(&fact_text(&h, "7-Zip 24.09 (x64 edition)", "fatto-programma-su-disco")).is_some());

        // Giochi di Steam: l'ultima sessione registrata da Steam.
        h.state_mut().programs.query = String::new();
        h.run_steps(2);
        let texts = drawn(&h);
        {
            let a = h.state();
            let p = &a.data.as_ref().expect("dati").programs;
            let mut games = 0;
            for x in p.items.iter().filter(|x| matches!(x.source, ProgramSource::Steam { .. })) {
                let Usage::Steam { manifest: Some(t), .. } = x.usage else { panic!("{}: LastPlayed", x.name) };
                let cell = a.lingue.ta("programmi-uso-steam", &[("data", a.fmt.date(t).into())]);
                assert!(texts.contains(&cell), "{}: {cell}", x.name);
                games += 1;
            }
            assert_eq!(games, 4);
        }
        select(&mut h, "Hollow Peaks");
        assert!(h.query_by_label(&fact_text(&h, "Hollow Peaks", "fatto-ultima-sessione-steam")).is_some());
    }

    #[test]
    fn tracking_off_shows_no_userassist_value() {
        // Con Start_TrackProgs = 0 il testo onesto e nessun valore UserAssist. Finestra alta:
        // la sezione dell'uso del dettaglio è visibile senza scorrere.
        let mut h = programs("it", 2400.0);
        assert_eq!(h.state().data.as_ref().expect("dati").programs.tracking, TrackProgs::Off);
        let texts = drawn(&h);
        assert!(texts.iter().any(|s| s == "Tracciamento degli avvii di Windows disattivato: nessun dato d'uso"), "testo del tracciamento disattivato");
        assert!(texts.iter().all(|s| !s.contains("(UserAssist)") && !s.contains("Ultimo avvio da Esplora file")));
        select(&mut h, "CapCut");
        assert!(in_detail(&h, &fact_text(&h, "CapCut", "fatto-uso-tracciamento-disattivato")));
        assert!(h.query_by_label_contains("Ultimo avvio da Esplora file").is_none());
        let a = h.state();
        assert!(a.data.as_ref().expect("dati").programs.items.iter().all(|x| !matches!(x.usage, Usage::UserAssist(_))));
    }

    #[test]
    fn capcut_uninstall_shows_your_files_first_then_opens_settings() {
        // Prima i tuoi file con la conferma esplicita, poi il nome negli appunti e Impostazioni ▸ App.
        let mut h = programs("it", 1000.0);
        select(&mut h, "CapCut");
        click_t(&mut h, "programmi-disinstalla");
        assert!(h.state().programs.dialog.is_some(), "dialogo aperto");
        // Il titolo chiede quello che fa il pulsante principale: Quado apre Impostazioni, non disinstalla.
        let title = |h: &Harness<'_, App>, key: &str| h.state().lingue.ta(key, &[("programma", "CapCut".into())]);
        assert!(h.query_by_label(&title(&h, "programmi-dialogo-titolo")).is_some(), "titolo prima dell'apertura");
        let warning = plain_fact(&h, "CapCut", "fatto-programma-tuoi-file");
        assert!(warning.contains("Copiali altrove"), "{warning}");
        let method = h.state().lingue.ta("programmi-dialogo-impostazioni", &[("programma", "CapCut".into())]);
        // L'avviso compare nel dettaglio e nel dialogo: quello del dialogo sta fuori dal pannello.
        let detail = h.state().programs.detail;
        let in_dialog = |l: &str| h.query_all_by_label(l).find(|n| !detail.contains_rect(n.rect())).map(|n| n.rect().min.y);
        let (w, m) = (in_dialog(&warning).expect("avviso nel dialogo"), in_dialog(&method).expect("metodo nel dialogo"));
        assert!(w < m, "l'avviso dei tuoi file viene prima del metodo");
        // Senza la conferma il pulsante è disattivato e non apre niente.
        let open = t(&h, "programmi-dialogo-apri-impostazioni");
        assert!(h.get_by_label(&open).accesskit_node().is_disabled());
        click(&mut h, &open);
        assert!(launched().is_empty());
        assert_eq!(h.state().programs.dialog.as_ref().map(|d| d.outcome), Some(None));
        // Conferma, poi apertura: nome negli appunti, `Launch::AppsFeatures`, avviso «Si apre Impostazioni › App».
        click_t(&mut h, "funzione-programma-controllo-tuoi-file");
        assert!(h.state().programs.dialog.as_ref().is_some_and(|d| d.copied));
        h.get_by_label(&open).click();
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
        assert_eq!(copied, vec!["CapCut".to_owned()], "il nome negli appunti, mai UninstallString");
        assert_eq!(launched(), vec![Launch::AppsFeatures]);
        h.run_steps(2);
        assert_eq!(h.state().programs.dialog.as_ref().and_then(|d| d.outcome), Some(Outcome::Opened));
        // Nel dialogo e nella barra di stato.
        assert_eq!(h.query_all_by_label("Si apre Impostazioni › App. Il nome «CapCut» è negli appunti: incollalo nella casella di ricerca.").count(), 2);
        // Dopo l'apertura il titolo nomina il compito: i pulsanti sono «Ho eseguito la procedura» e «Chiudi».
        assert!(h.query_by_label(&title(&h, "programmi-dialogo-titolo-guida")).is_some(), "titolo dopo l'apertura");
        assert!(h.query_by_label(&title(&h, "programmi-dialogo-titolo")).is_none());
        click_t(&mut h, "programmi-dialogo-chiudi");
        assert!(h.state().programs.dialog.is_none());
        assert_eq!(launched().len(), 1, "chiudere non apre altro");
    }

    #[test]
    fn cancel_and_escape_close_the_dialog_without_launching() {
        let mut h = programs("it", 1000.0);
        select(&mut h, "7-Zip 24.09 (x64 edition)");
        let uninstall = t(&h, "programmi-disinstalla");
        click(&mut h, &uninstall);
        assert!(h.state().programs.dialog.is_some());
        // Senza tuoi file il pulsante è attivo da subito; Annulla chiude senza fare niente.
        assert!(!h.get_by_label(&t(&h, "programmi-dialogo-apri-impostazioni")).accesskit_node().is_disabled());
        click_t(&mut h, "programmi-dialogo-annulla");
        assert!(h.state().programs.dialog.is_none());
        click(&mut h, &uninstall);
        assert!(h.state().programs.dialog.is_some());
        key(&mut h, Key::Escape);
        assert!(h.state().programs.dialog.is_none(), "Esc chiude il dialogo");
        assert!(launched().is_empty(), "né Annulla né Esc chiamano il lanciatore");
        let copies = h.output().platform_output.commands.iter().filter(|c| matches!(c, egui::OutputCommand::CopyText(_))).count();
        assert_eq!(copies, 0);
        assert_eq!(h.state().screen, Screen::Programs);
    }

    #[test]
    fn steam_msix_and_epic_use_their_official_interface() {
        let mut h = programs("it", 1000.0);
        let uninstall = t(&h, "programmi-disinstalla");
        // Steam: `steam://uninstall/<appid>` con l'appid validato.
        select(&mut h, "Hollow Peaks");
        click(&mut h, &uninstall);
        let steam_title = h.state().lingue.ta("programmi-dialogo-titolo-steam", &[("programma", "Hollow Peaks".into())]);
        assert!(h.query_by_label(&steam_title).is_some(), "titolo con lo stesso verbo di «Apri Steam»");
        click_t(&mut h, "programmi-dialogo-apri-steam");
        assert_eq!(launched(), vec![Launch::SteamUninstall(SteamAppId::new(3_999_910).expect("appid"))]);
        click_t(&mut h, "programmi-dialogo-chiudi");
        // MSIX: la pagina dell'app in Impostazioni con il PFN validato.
        select(&mut h, "Microsoft.WindowsTerminal_8wekyb3d8bbwe");
        click(&mut h, &uninstall);
        click_t(&mut h, "programmi-dialogo-apri-impostazioni");
        assert_eq!(launched()[1], Launch::AppsFeaturesApp(Pfn::new("Microsoft.WindowsTerminal_8wekyb3d8bbwe").expect("PFN")));
        let copies = h.output().platform_output.commands.iter().filter(|c| matches!(c, egui::OutputCommand::CopyText(_))).count();
        assert_eq!(copies, 0, "per MSIX nessun nome negli appunti");
        click_t(&mut h, "programmi-dialogo-chiudi");
        // Epic: solo il rinvio alla libreria del launcher, nessun URI e nessuna chiamata.
        select(&mut h, "Unreal Engine");
        click(&mut h, &uninstall);
        assert!(h.query_by_label_contains("libreria del launcher Epic").is_some());
        // Quado non apre niente: il titolo nomina il compito invece di chiedere un'azione.
        let epic_title = h.state().lingue.ta("programmi-dialogo-titolo-guida", &[("programma", "Unreal Engine".into())]);
        assert!(h.query_by_label(&epic_title).is_some(), "titolo per Epic");
        assert!(h.query_by_label(&t(&h, "programmi-dialogo-apri-impostazioni")).is_none());
        assert!(h.query_by_label(&t(&h, "programmi-dialogo-apri-steam")).is_none());
        click_t(&mut h, "programmi-dialogo-chiudi");
        assert!(h.state().programs.dialog.is_none());
        assert_eq!(launched().len(), 2, "Epic non chiama il lanciatore");
    }

    #[test]
    fn possible_residues_have_only_the_allowed_actions() {
        // VecchioEditor (Roaming) e Unity (LocalLow) con data e testo, Non sappiamo, solo
        // "Apri in Esplora file" e "Mostra in Esplora".
        let mut h = programs("it", 1000.0);
        let n = h.state().data.as_ref().expect("dati").programs.residues.len();
        let chip = h.state().lingue.ta("programmi-sezione-residui", &[("testo", n.to_string().into())]);
        click(&mut h, &chip);
        assert_eq!(h.state().programs.section, Section::Residues);
        let texts = drawn(&h);
        let (rows, path, node) = {
            let a = h.state();
            let d = a.data.as_ref().expect("dati");
            let t = d.tree();
            let mut rows = Vec::new();
            for (k, r) in d.programs.residues.iter().enumerate() {
                let name = t.name_lossy(r.node);
                let f = &r.record.identity.0[0];
                assert_eq!(f.key, "fatto-residuo-possibile", "{name}: con la data");
                let text = kbtext::fact_text(&a.lingue, &a.fmt, f);
                assert!(text.contains(&a.fmt.date(r.last_modified.expect("data"))), "{text}");
                assert!(texts.contains(&text), "{name}: testo nella riga");
                assert_eq!(r.record.action, quado_model::record::Action::None);
                rows.push((name, format!("{:?}", r.base), k));
            }
            let v = d.programs.residues.iter().find(|r| t.name_lossy(r.node) == "VecchioEditor").expect("VecchioEditor");
            (rows, t.path_lossy(v.node), v.node)
        };
        assert!(rows.iter().any(|(n, b, _)| n == "VecchioEditor" && b == "Roaming"));
        assert!(rows.iter().any(|(n, b, _)| n == "Unity" && b == "LocalLow"));
        let k = rows.iter().find(|r| r.0 == "VecchioEditor").map(|r| r.2).expect("riga");
        h.state_mut().programs.selected_residue = Some(k);
        h.run_steps(3);
        assert!(h.query_by_label(&t(&h, "stato-non-sappiamo")).is_some(), "stato con la parola");
        // Pulsanti nel dettaglio: solo i due ammessi.
        let detail = h.state().programs.detail;
        let mut buttons: Vec<String> = h.query_all_by_role(Role::Button).filter(|b| detail.contains_rect(b.rect())).filter_map(|b| b.accesskit_node().label()).collect();
        buttons.sort();
        let mut allowed = vec![t(&h, "azione-apri-in-esplora"), t(&h, "programmi-mostra-in-esplora")];
        allowed.sort();
        assert_eq!(buttons, allowed);
        click_t(&mut h, "azione-apri-in-esplora");
        assert_eq!(revealed(), vec![path]);
        click_t(&mut h, "programmi-mostra-in-esplora");
        assert_eq!(h.state().screen, Screen::Explore);
        assert_eq!(h.state().explore.selected, Some(node));
    }

    #[test]
    fn no_forbidden_words_on_the_screen() {
        // Nessun testo disegnato usa le parole vietate (docs/testi.md §3), in italiano e in inglese.
        const FORBIDDEN: [&str; 16] = [
            "inutilizzat",
            "mai usato",
            "non usato da",
            "non più in uso",
            "sicuro da disinstallare",
            "sicuro da eliminare",
            "spazzatura",
            "ottimizza",
            "unused",
            "never used",
            "not used",
            "no longer used",
            "safe to uninstall",
            "safe to delete",
            "junk",
            "optimiz",
        ];
        for lang in ["it", "en"] {
            let mut h = programs(lang, 2400.0);
            let mut all: Vec<String> = Vec::new();
            for name in ["CapCut", "7-Zip 24.09 (x64 edition)", "Hollow Peaks", "Unreal Engine", "Microsoft.WindowsTerminal_8wekyb3d8bbwe", "NVIDIA Graphics Driver 617.42"] {
                select(&mut h, name);
                all.extend(drawn(&h));
                // Il dialogo, prima e dopo l'apertura.
                let i = index(&h, name);
                h.state_mut().programs.dialog = Some(quado_ui::app::programs_view::UninstallDialog { program: i, copied: false, outcome: None });
                h.run_steps(2);
                all.extend(drawn(&h));
                h.state_mut().programs.dialog = Some(quado_ui::app::programs_view::UninstallDialog { program: i, copied: true, outcome: Some(Outcome::Opened) });
                h.run_steps(2);
                all.extend(drawn(&h));
                h.state_mut().programs.dialog = None;
            }
            h.state_mut().programs.show_hidden = true;
            h.state_mut().programs.section = Section::Residues;
            h.state_mut().programs.selected_residue = Some(0);
            h.run_steps(3);
            all.extend(drawn(&h));
            assert!(all.len() > 200, "{lang}: testi raccolti {}", all.len());
            for s in &all {
                let low = s.to_lowercase();
                for bad in FORBIDDEN {
                    assert!(!low.contains(bad), "{lang}: «{bad}» in {s:?}");
                }
                assert!(!s.contains('!') && !s.contains('\u{2014}') && !s.contains('\u{2013}'), "{lang}: {s:?}");
            }
        }
    }

    #[test]
    fn table_from_the_keyboard() {
        let mut h = programs("it", 1000.0);
        let rows = h.state().programs.rows.clone();
        assert!(rows.len() > 3);
        key(&mut h, Key::ArrowDown);
        assert_eq!(h.state().programs.selected, Some(rows[0]), "la prima freccia seleziona la prima riga");
        key(&mut h, Key::ArrowDown);
        assert_eq!(h.state().programs.selected, Some(rows[1]));
        key(&mut h, Key::ArrowUp);
        assert_eq!(h.state().programs.selected, Some(rows[0]));
        key(&mut h, Key::End);
        assert_eq!(h.state().programs.selected, rows.last().copied());
        key(&mut h, Key::Home);
        assert_eq!(h.state().programs.selected, Some(rows[0]));
        // Il dettaglio segue la selezione.
        let name = h.state().data.as_ref().expect("dati").programs.items[rows[0]].name.clone();
        assert!(in_detail(&h, &name), "titolo del dettaglio");
        // Invio: la cartella del programma nell'Esplora.
        let folder = h.state().data.as_ref().expect("dati").programs.items[rows[0]].location.node().expect("cartella");
        key(&mut h, Key::Enter);
        assert_eq!(h.state().screen, Screen::Explore);
        assert_eq!(h.state().explore.selected, Some(folder));
        // Di nuovo Ctrl+6, sezione dei residui: la tabella ha il focus e le frecce selezionano.
        press(&mut h, Key::Num6, Modifiers::COMMAND);
        assert_eq!(h.state().screen, Screen::Programs);
        h.state_mut().programs.section = Section::Residues;
        h.state_mut().programs.focus_pending = true;
        h.run_steps(2);
        key(&mut h, Key::ArrowDown);
        assert_eq!(h.state().programs.selected_residue, Some(0));
        key(&mut h, Key::ArrowDown);
        assert_eq!(h.state().programs.selected_residue, Some(1));
    }

    #[test]
    fn empty_states_say_why() {
        // Scansione senza prove degli estrattori: nessun programma e il perché; nessun residuo.
        let mut a = app("it");
        let (v, r) = synthetic::sample_c(false);
        a.load_result(v, r, Duration::from_secs(1));
        let mut h = Harness::builder().with_size([1600.0, 1000.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
        h.run_steps(4);
        press(&mut h, Key::Num6, Modifiers::COMMAND);
        assert!(h.query_by_label(&t(&h, "programmi-vuoto-senza-fonti")).is_some());
        h.state_mut().programs.section = Section::Residues;
        h.run_steps(2);
        assert!(h.query_by_label(&t(&h, "programmi-residui-nessuno")).is_some());
        // Con le prove, una ricerca senza risultati.
        let mut h = programs("it", 1000.0);
        h.state_mut().programs.query = "nessun programma ha questo nome".into();
        h.run_steps(2);
        assert!(h.state().programs.rows.is_empty());
        assert!(h.query_by_label(&t(&h, "programmi-vuoto-ricerca")).is_some());
        assert!(h.query_by_label(&t(&h, "programmi-nessuna-selezione")).is_some());
    }
}

// ---------------------------------------------------------------------------------------------
// Lente Download con letture finte (nessun file letto, `synthetic::FakeReaders`).

use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;

use quado_kb::lens::{Finding, InstallerStatus, UseStatus};
use quado_model::downloads::{GroupComparison, LensFamily};
use quado_model::record::{Provenance, State};
use quado_ui::app::downloads_view::ReadKind;
use quado_ui::app::lens_jobs::Readers;

/// Sample di C: con le letture finte; `prepare` le configura prima del caricamento. Finestra alta: ispettore
/// e card interventi sono visibili senza scorrere, così i clic arrivano.
fn lens_app(prepare: impl FnOnce(&synthetic::FakeReaders)) -> (Harness<'static, App>, Arc<synthetic::FakeReaders>) {
    let mut a = app("it");
    let (v, r) = synthetic::sample_c(false);
    let fake = Arc::new(synthetic::FakeReaders::for_tree(&r.tree));
    prepare(&fake);
    a.env.readers = Readers(fake.clone());
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut h = Harness::builder().with_size([1600.0, 2600.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(4);
    wait_lens(&mut h);
    (h, fake)
}

/// Frame finché la lente non ha più lavori in corso.
fn wait_lens(h: &mut Harness<'_, App>) {
    for _ in 0..600 {
        h.step();
        if !h.state().downloads.pending() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(3);
}

/// Clic sul primo elemento con questa etichetta, poi i frame per applicarlo.
fn click(h: &mut Harness<'_, App>, label: &str) {
    h.query_all_by_label(label).next().unwrap_or_else(|| panic!("manca «{label}»")).click();
    h.run_steps(2);
}

/// Clic sull'elemento con il testo di questa chiave.
fn click_t(h: &mut Harness<'_, App>, key: &str) {
    let label = h.state().lingue.t(key);
    click(h, &label);
}

fn open_lens(h: &mut Harness<'_, App>) {
    h.state_mut().go(Screen::Downloads);
    wait_lens(h);
}

fn t(h: &Harness<'_, App>, key: &str) -> String {
    h.state().lingue.t(key)
}

/// Voci della lente con il loro tipo e i file.
fn findings(h: &Harness<'_, App>) -> Vec<(Finding, Vec<u32>, &'static str)> {
    let v = h.state().downloads.view.as_ref().expect("lente");
    v.findings.iter().map(|f| (f.finding, f.nodes.clone(), f.record.state.code())).collect()
}

/// Riga dell'elenco con questo tipo di voce.
fn row_with(h: &Harness<'_, App>, pred: impl Fn(&Finding) -> bool) -> Option<usize> {
    h.state().downloads.view.as_ref().and_then(|v| v.rows.iter().position(|r| pred(&r.finding)))
}

#[test]
fn download_lens_opens_without_reading_content() {
    // Apertura, selezione con le frecce e ispettore senza nessuna lettura di contenuto.
    let (mut h, fake) = lens_app(|_| {});
    press(&mut h, Key::Num5, Modifiers::COMMAND);
    assert_eq!(h.state().screen, Screen::Downloads);
    wait_lens(&mut h);
    assert!(h.query_by_label(&t(&h, "download-elenco-accessibile")).is_some());
    // Le voci si scelgono con le frecce, come nelle altre tabelle.
    key(&mut h, Key::ArrowDown);
    assert_eq!(h.state().downloads.selected, Some(0));
    key(&mut h, Key::ArrowDown);
    assert_eq!(h.state().downloads.selected, Some(1));
    key(&mut h, Key::ArrowUp);
    assert_eq!(h.state().downloads.selected, Some(0));
    key(&mut h, Key::End);
    let rows = h.state().downloads.view.as_ref().map_or(0, |v| v.rows.len());
    assert_eq!(h.state().downloads.selected, Some(rows - 1));
    // Ispettore di Esplora sulla cartella Download: c'è il comando della lente.
    let dl = node_named(&h, "Downloads");
    {
        let a = h.state_mut();
        a.go(Screen::Explore);
        let d = a.data.as_ref().expect("dati");
        let demo = d.tree().parent(dl);
        a.explore.open(d, demo, Some(dl));
    }
    h.run_steps(4);
    assert!(h.query_by_label(&t(&h, "download-apri-lente")).is_some());
    assert_eq!(fake.content_reads(), 0, "nessuna lettura di contenuto: {:?}", fake.content_names());
    assert_eq!(fake.identities.load(Relaxed), 0, "nemmeno gli attributi prima di una richiesta");
    // Restart Manager parte da solo sui download incompleti: non legge il contenuto.
    assert!(fake.usage_queries.load(Relaxed) > 0);
}

#[test]
fn download_families_and_the_copy_suffix_filter() {
    let (mut h, fake) = lens_app(|_| {});
    open_lens(&mut h);
    // Famiglie secondo l'estensione sulla cartella dei dati di prova (contate a mano su `sample_c`).
    {
        let v = h.state().downloads.view.as_ref().expect("lente");
        let files = |f: LensFamily| v.lens.total(f).files;
        let want = [
            (LensFamily::Installers, 51),
            (LensFamily::Archives, 26),
            (LensFamily::DiskImages, 24),
            (LensFamily::Media, 24),
            (LensFamily::DocsImages, 27),
            (LensFamily::Incomplete, 1),
            (LensFamily::Other, 12),
        ];
        for (f, n) in want {
            assert_eq!(files(f), n, "{f:?}");
        }
        assert_eq!(v.lens.files, 165);
        let sum: u64 = LensFamily::ALL.iter().map(|&f| v.lens.total(f).alloc).sum();
        assert_eq!(sum, v.lens.alloc, "la somma delle famiglie torna con il totale");
    }
    // Sei riquadri, compreso quello dei download incompleti perché ce n'è uno.
    for f in LensFamily::SHOWN {
        let name = t(&h, &format!("lente-famiglia-{}", f.code()));
        assert!(h.query_by_label_contains(&format!("{name}: ")).is_some(), "riquadro {name}");
    }
    // Filtro "(N)": trova `setup (1).exe`, non cambia voci né stati.
    let before = findings(&h);
    let chip = h.state().lingue.ta("download-filtro-suffisso", &[("n", 2u64.into()), ("testo", "2".into())]);
    click(&mut h, &chip);
    wait_lens(&mut h);
    assert_eq!(h.state().downloads.panel, quado_ui::app::downloads_view::Panel::Suffix);
    let action = t(&h, "azione-apri-in-esplora");
    assert!(h.query_by_label(&format!("{action}: setup (1).exe")).is_some());
    assert!(h.query_by_label(&format!("{action}: Fattura marzo (1).pdf")).is_some());
    assert!(h.query_by_label(&format!("{action}: setup.exe")).is_none(), "solo i file con il suffisso");
    assert_eq!(findings(&h), before, "il filtro non cambia voci o stati");
    // Un'altra cartella dell'utente, dall'ispettore di Esplora.
    let site = node_named(&h, "Sito web");
    {
        let a = h.state_mut();
        a.go(Screen::Explore);
        let d = a.data.as_ref().expect("dati");
        let parent = d.tree().parent(site);
        a.explore.open(d, parent, Some(site));
    }
    h.run_steps(4);
    click_t(&mut h, "download-apri-lente");
    assert_eq!(h.state().screen, Screen::Downloads);
    wait_lens(&mut h);
    let v = h.state().downloads.view.as_ref().expect("lente");
    assert_eq!(v.root, site);
    assert_eq!((v.lens.total(LensFamily::Other).files, v.lens.files), (12, 12));
    assert_eq!(fake.content_reads(), 0);
}

#[test]
fn compare_the_content_after_saying_what_it_reads() {
    // Prima il testo con file, byte e ultimo accesso; dopo la conferma le copie sono Gestibile e
    // resta la più vecchia; la scelta di un'altra cambia la copia tenuta; i file di dimensione unica non si
    // leggono.
    let (mut h, fake) = lens_app(|_| {});
    open_lens(&mut h);
    let fatture: Vec<u32> = ["Fattura marzo.pdf", "Fattura marzo (1).pdf", "fattura-copia.pdf"].iter().map(|n| node_named(&h, n)).collect();
    let same = findings(&h).into_iter().find(|f| f.1.contains(&fatture[0])).expect("fatture");
    assert_eq!((same.0, same.2), (Finding::SameSize, "decidi-tu"));
    click_t(&mut h, "download-comando-confronta");
    let (files, bytes) = {
        let p = h.state().downloads.prompt.as_ref().expect("richiesta di conferma");
        assert_eq!(p.kind, ReadKind::Hash);
        (p.files, p.bytes)
    };
    let text = {
        let a = h.state();
        let f = &a.fmt;
        a.lingue.ta("download-confronta-domanda", &[("n", files.into()), ("testo", f.count(files as u64).into()), ("byte", f.size(bytes).into())])
    };
    assert!(text.contains("Aggiorna la data di ultimo accesso di questi file"));
    assert!(h.query_by_label(&text).is_some(), "il testo della lettura compare prima: {text}");
    assert_eq!(fake.content_reads(), 0, "niente letto prima della conferma");
    click_t(&mut h, "download-conferma-confronta");
    wait_lens(&mut h);
    // Solo i file dei gruppi della stessa dimensione, tutti.
    let read = fake.content_names();
    assert_eq!(read.len(), files);
    for unique in ["setup.exe", "setup (1).exe", "Win11_24H2_Italian_x64.iso", "SteamSetup.exe", "vacanze.jpg"] {
        assert!(!read.iter().any(|n| n == unique), "{unique} ha una dimensione unica: non si legge");
    }
    {
        let v = h.state().downloads.view.as_ref().expect("lente");
        assert!(v.compared.iter().all(|c| !matches!(c, GroupComparison::NotRead { .. })));
    }
    let copies = findings(&h).into_iter().find(|f| f.1.contains(&fatture[0])).expect("copie");
    assert_eq!(copies.0, Finding::IdenticalCopies { keep: fatture[0] }, "resta la più vecchia per data di creazione");
    assert_eq!(copies.2, "gestibile");
    assert_eq!(copies.1.len(), 3);
    // La scelta di un'altra copia.
    click_t(&mut h, "download-voce-copie");
    assert!(h.state().downloads.selected.is_some());
    let keep = h.state().lingue.ta("download-tieni", &[("nome", "fattura-copia.pdf".into())]);
    click(&mut h, &keep);
    wait_lens(&mut h);
    let copies = findings(&h).into_iter().find(|f| f.1.contains(&fatture[0])).expect("copie");
    assert_eq!(copies.0, Finding::IdenticalCopies { keep: fatture[2] });
    assert_eq!(fake.content_reads(), files, "scegliere la copia non rilegge niente");
}

#[test]
fn an_interrupted_comparison_shows_no_group_as_complete() {
    // Annullamento a metà; un gruppo letto in parte resta "contenuto non confrontato".
    let (mut h, fake) = lens_app(|f| f.hash_limit.store(5, Relaxed));
    open_lens(&mut h);
    click_t(&mut h, "download-comando-confronta");
    click_t(&mut h, "download-conferma-confronta");
    for _ in 0..400 {
        h.step();
        if fake.content_reads() >= 5 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(2);
    assert!(h.state().downloads.progress().is_some(), "confronto in corso");
    click_t(&mut h, "download-annulla-lettura");
    wait_lens(&mut h);
    let o = h.state().downloads.outcome.expect("esito");
    assert!(o.interrupted && o.read < o.total, "{o:?}");
    assert!(h.query_by_label_contains("Confronto interrotto").is_some());
    let a = h.state();
    let st = &a.downloads;
    let v = st.view.as_ref().expect("lente");
    let hashed: std::collections::HashSet<u32> = st.readings.hashed.iter().map(|x| x.node).collect();
    for (g, c) in v.candidates.iter().zip(&v.compared) {
        let read = g.nodes.iter().filter(|n| hashed.contains(n)).count();
        match c {
            GroupComparison::Compared { .. } => assert_eq!(read, g.nodes.len(), "un gruppo confrontato è stato letto per intero"),
            GroupComparison::NotRead { read: r } => assert_eq!(*r, read),
            GroupComparison::Changed { .. } => panic!("nessun file è cambiato"),
        }
    }
    // Le voci di copie e di contenuto diverso vengono solo da gruppi letti per intero.
    for f in &v.findings {
        if matches!(f.finding, Finding::IdenticalCopies { .. } | Finding::DifferentContent) {
            assert!(f.nodes.iter().all(|n| hashed.contains(n)));
        }
    }
    assert!(!fake.content_names().iter().any(|n| n.contains("attura")), "le fatture, ultimo gruppo, non sono state lette");
    assert!(v.findings.iter().all(|f| !matches!(f.finding, Finding::IdenticalCopies { .. })));
}

#[test]
fn installers_three_outcomes_then_the_cache() {
    // Tre esiti con letture finte; una seconda richiesta non rilegge; un file cambiato si rilegge,
    // solo lui.
    let (mut h, fake) = lens_app(|_| {});
    open_lens(&mut h);
    click_t(&mut h, "download-comando-installatori");
    wait_lens(&mut h);
    let files = h.state().downloads.prompt.as_ref().map(|p| (p.kind, p.files)).expect("richiesta di conferma");
    assert_eq!(files, (ReadKind::Installers, 51));
    assert_eq!(fake.content_reads(), 0, "prima della conferma solo gli attributi");
    assert_eq!(fake.identities.load(Relaxed), 51);
    assert!(h.query_by_label_contains("Aggiorna la data di ultimo accesso di questi file").is_some());
    click_t(&mut h, "download-conferma-leggi");
    wait_lens(&mut h);
    assert_eq!(fake.content_reads(), 51);
    let by_name = |h: &Harness<'_, App>, name: &str| {
        let n = node_named(h, name);
        let v = h.state().downloads.view.as_ref().expect("lente");
        let f = v.findings.iter().find(|f| f.nodes == [n]).expect("voce");
        (f.finding, f.record.identity.0[0].key.clone(), f.record.identity.0[0].provenance)
    };
    assert_eq!(by_name(&h, "7z2409-x64.msi"), (Finding::Installer(InstallerStatus::SameVersion), "fatto-installatore-stesso-codice".into(), Provenance::FromSource));
    let steam = by_name(&h, "SteamSetup.exe");
    assert_eq!((steam.0, steam.1.as_str()), (Finding::Installer(InstallerStatus::OtherVersion), "fatto-installatore-nome-versione-diversa"));
    assert!(matches!(steam.2, Provenance::Deduced(_)));
    assert_eq!(by_name(&h, "setup.exe").0, Finding::Installer(InstallerStatus::NotInstalled));
    // Seconda richiesta: nessuna lettura, nessuna conferma.
    click_t(&mut h, "download-comando-installatori");
    wait_lens(&mut h);
    assert!(h.state().downloads.prompt.is_none());
    assert!(h.state().downloads.outcome.is_some_and(|o| o.cached));
    assert!(h.query_by_label(&t(&h, "download-esito-installatori-cache")).is_some());
    assert_eq!(fake.content_reads(), 51);
    // Un file cambiato dopo la scansione: si rilegge lui solo.
    fake.changed.lock().expect("finto").insert("setup.exe".into(), synthetic::T0 + 864_000_000_000);
    click_t(&mut h, "download-comando-installatori");
    wait_lens(&mut h);
    assert_eq!(h.state().downloads.prompt.as_ref().map(|p| p.files), Some(1));
    click_t(&mut h, "download-conferma-leggi");
    wait_lens(&mut h);
    assert_eq!(fake.content_reads(), 52);
    assert_eq!(fake.content_names().last().map(String::as_str), Some("setup.exe"));
    assert_eq!(by_name(&h, "setup.exe").0, Finding::Installer(InstallerStatus::NotInstalled));
}

#[test]
fn archive_index_against_the_folder_and_a_broken_zip() {
    // "N voci su M presenti"; uno zip non leggibile non ferma la lente.
    let (mut h, fake) = lens_app(|_| {});
    open_lens(&mut h);
    click_t(&mut h, "download-comando-archivi");
    assert_eq!(h.state().downloads.prompt.as_ref().map(|p| (p.kind, p.files)), Some((ReadKind::Zip, 1)));
    click_t(&mut h, "download-conferma-leggi");
    wait_lens(&mut h);
    assert_eq!(fake.content_names(), ["Sito web.zip"]);
    let row = row_with(&h, |f| matches!(f, Finding::ArchiveWithFolder { .. })).expect("archivio");
    h.state_mut().downloads.select(row);
    h.run_steps(3);
    assert!(h.query_by_label_contains("12 file su 13 dell'archivio sono nella cartella").is_some());
    let voci = findings(&h).len();
    fake.broken_zip.lock().expect("finto").push("Sito web.zip".into());
    click_t(&mut h, "download-comando-archivi");
    click_t(&mut h, "download-conferma-leggi");
    wait_lens(&mut h);
    assert!(h.query_by_label_contains("Impossibile leggere l'indice dell'archivio: formato inatteso").is_some());
    assert_eq!(findings(&h).len(), voci, "le altre voci restano");
    assert_eq!(h.state().downloads.outcome.map(|o| o.failed), Some(1));
}

#[test]
fn incomplete_downloads_follow_restart_manager_in_the_lens() {
    // Libero → Gestibile; in uso → bloccato, con il nome del processo.
    let (mut h, _) = lens_app(|_| {});
    open_lens(&mut h);
    let row = row_with(&h, |f| *f == Finding::Incomplete(UseStatus::Free)).expect("download libero");
    let v = h.state().downloads.view.as_ref().expect("lente");
    assert!(matches!(v.findings[v.rows[row].voci[0]].record.state, State::Manageable(_)));
    let (mut h, _) = lens_app(|f| {
        f.in_use.lock().expect("finto").insert("concerto.mp4.crdownload".into(), vec!["chrome.exe".into()]);
    });
    open_lens(&mut h);
    assert!(row_with(&h, |f| *f == Finding::Incomplete(UseStatus::Free)).is_none());
    let row = row_with(&h, |f| *f == Finding::Incomplete(UseStatus::InUse)).expect("download in uso");
    h.state_mut().downloads.select(row);
    h.run_steps(3);
    assert!(h.query_by_label_contains("Lo usa chrome.exe").is_some());
    let v = h.state().downloads.view.as_ref().expect("lente");
    let r = &v.findings[v.rows[row].voci[0]].record;
    assert_eq!(r.state, State::YouDecide);
    assert_eq!(r.action, quado_model::record::Action::None);
    // "Controlla di nuovo l'uso": il processo ha chiuso il file.
    let (mut h, fake) = lens_app(|f| {
        f.in_use.lock().expect("finto").insert("concerto.mp4.crdownload".into(), vec!["chrome.exe".into()]);
    });
    open_lens(&mut h);
    let row = row_with(&h, |f| *f == Finding::Incomplete(UseStatus::InUse)).expect("download in uso");
    h.state_mut().downloads.select(row);
    h.run_steps(2);
    fake.in_use.lock().expect("finto").clear();
    click_t(&mut h, "download-comando-uso");
    wait_lens(&mut h);
    assert!(row_with(&h, |f| *f == Finding::Incomplete(UseStatus::Free)).is_some());
}

#[test]
fn interventions_card_gets_copies_only_after_the_comparison() {
    // Le copie entrano nella card solo dopo il confronto; un clic apre la voce.
    let (mut h, _) = lens_app(|_| {});
    h.state_mut().go(Screen::Overview);
    h.run_steps(3);
    let copies = t(&h, "funzione-copie-identiche-nome");
    assert!(h.query_by_label(&copies).is_none(), "nessuna copia prima del confronto");
    assert!(h.query_by_label(&t(&h, "funzione-download-incompleto-nome")).is_some(), "il download incompleto non in uso c'è");
    {
        let a = h.state_mut();
        let readers = a.env.readers.clone();
        let d = a.data.as_ref().expect("dati");
        a.downloads.request(ReadKind::Hash, d, &readers);
        a.downloads.confirm(d, &readers);
        a.downloads.settle(Some(d), &readers);
    }
    h.run_steps(3);
    assert!(h.query_by_label(&t(&h, "interventi-sottotitolo-lente")).is_some());
    click(&mut h, &copies);
    assert_eq!(h.state().screen, Screen::Downloads);
    let a = h.state();
    let v = a.downloads.view.as_ref().expect("lente");
    let sel = a.downloads.selected.expect("voce selezionata");
    assert!(matches!(v.rows[sel].finding, Finding::IdenticalCopies { .. }));
}

#[test]
fn appearance_and_explore_settings_apply_at_once() {
    // Barra laterale comprimibile, dimensione del testo come zoom, densità, accento, colori dei dati e vista
    // iniziale di Esplora valgono subito, dalla barra superiore e da Impostazioni.
    use quado_ui::app::settings::ExploreView;
    use quado_ui::tokens::{self, Accent, DataPalette, Density, Palette};
    let mut h = loaded(false, "it");
    // La barra laterale si comprime dal pulsante della barra superiore e resta compressa.
    h.get_by_label("Comprimi la barra laterale").click();
    h.run_steps(2);
    assert!(h.state().settings.sidebar_collapsed);
    assert!(h.query_by_label("Espandi la barra laterale").is_some());
    // Compressa, la barra mostra solo le icone: il nome della vista resta per AccessKit e nel suggerimento.
    assert!(h.query_all_by_label("Panoramica").count() >= 2, "voce della barra laterale e titolo");
    // Ctrl+F con la barra compressa la apre per la ricerca, con il focus nella casella.
    press(&mut h, Key::F, Modifiers::COMMAND);
    h.run_steps(2);
    assert!(h.state().sidebar_peek && h.state().search.box_focused, "casella di ricerca aperta e con il focus");
    assert!(h.state().settings.sidebar_collapsed, "la scelta salvata non cambia");
    key(&mut h, Key::Escape);
    // Ctrl+più e Ctrl+meno cambiano la dimensione del testo a passi di 10, nei limiti; lo zoom segue.
    press(&mut h, Key::Plus, Modifiers::COMMAND);
    assert_eq!(h.state().settings.text_scale, 110);
    h.run_steps(2);
    assert!((h.ctx.zoom_factor() - 1.1).abs() < 1e-3, "zoom {}", h.ctx.zoom_factor());
    press(&mut h, Key::Minus, Modifiers::COMMAND);
    press(&mut h, Key::Minus, Modifiers::COMMAND);
    press(&mut h, Key::Minus, Modifiers::COMMAND);
    assert_eq!(h.state().settings.text_scale, 90);
    press(&mut h, Key::Num0, Modifiers::COMMAND);
    assert_eq!(h.state().settings.text_scale, 100);
    // Impostazioni: densità compatta, accento rosa, colori per le discromatopsie, Esplora con il solo elenco.
    h.state_mut().go(Screen::Settings);
    h.run_steps(3);
    for label in ["Compatta", "Rosa", "Per le discromatopsie", "Solo elenco"] {
        h.get_by_label(label).click();
        h.run_steps(3);
    }
    let s = h.state().settings.clone();
    assert_eq!(s.density, Density::Compact);
    assert_eq!(s.accent, Accent::Rose);
    assert_eq!(s.data_palette, DataPalette::ColorBlind);
    assert_eq!(s.explore_view, ExploreView::ListOnly);
    assert_eq!(tokens::metrics(&h.ctx).row, 32.0, "righe della densità compatta");
    let p = h.state().palette(&h.ctx);
    assert_eq!(p, Palette::new(p.theme, Accent::Rose, DataPalette::ColorBlind));
    assert!(h.state().explore.list_only, "Esplora passa subito al solo elenco");
    // Colonne della tabella: la casella toglie la colonna e la scelta resta.
    h.get_by_label("Quota della cartella").click();
    h.run_steps(3);
    assert!(!h.state().settings.columns.share);
}

// ---------------------------------------------------------------------------------------------
// Palette dei comandi, filtri in linea di Esplora, Panoramica.

/// Frame finché la ricerca dei nomi della palette non ha finito.
fn wait_palette(h: &mut Harness<'_, App>) {
    for _ in 0..300 {
        h.step();
        if !h.state().palette.pending() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(2);
}

#[test]
fn command_palette_reaches_views_folders_and_commands_from_the_keyboard() {
    let mut h = loaded(false, "it");
    // Ctrl+K apre la palette con il focus nella casella; Invio apre la voce scelta.
    press(&mut h, Key::K, Modifiers::COMMAND);
    assert!(h.state().palette.open, "Ctrl+K apre la palette");
    text(&mut h, "impost");
    // Ogni voce ha un nome accessibile e dice se è la scelta.
    let settings = h.state().lingue.t("nav-impostazioni");
    let chosen = h.query_all_by(|n| n.label().is_some_and(|l| l == settings) && format!("{:?}", n.toggled()).contains("True")).count();
    assert_eq!(chosen, 1, "«{settings}» è la voce scelta, con il suo nome");
    key(&mut h, Key::Enter);
    assert!(!h.state().palette.open, "Invio chiude la palette");
    assert_eq!(h.state().screen, Screen::Settings);

    // Una cartella del modello, trovata con la ricerca per nome: Invio la mostra in Esplora.
    press(&mut h, Key::K, Modifiers::COMMAND);
    text(&mut h, "Projects");
    wait_palette(&mut h);
    key(&mut h, Key::Enter);
    assert_eq!(h.state().screen, Screen::Explore);
    let sel = h.state().explore.selected.expect("elemento trovato");
    assert_eq!(name(&h, sel), "Projects");

    // Le frecce muovono la scelta; Esc chiude senza fare niente.
    press(&mut h, Key::K, Modifiers::COMMAND);
    key(&mut h, Key::ArrowDown);
    key(&mut h, Key::ArrowDown);
    assert_eq!(h.state().palette.selected, 2);
    key(&mut h, Key::ArrowUp);
    assert_eq!(h.state().palette.selected, 1);
    // Tab non porta il focus fuori dalla palette.
    key(&mut h, Key::Tab);
    assert!(h.state().palette.open);
    key(&mut h, Key::Escape);
    assert!(!h.state().palette.open, "Esc chiude");
    assert_eq!(h.state().screen, Screen::Explore, "Esc non apre niente");
    assert_eq!(h.state().explore.selected, Some(sel));

    // Un comando: «Solo elenco» porta Esplora al solo elenco.
    press(&mut h, Key::K, Modifiers::COMMAND);
    let only = h.state().lingue.t("esplora-solo-elenco");
    text(&mut h, &only);
    wait_palette(&mut h);
    key(&mut h, Key::Enter);
    assert!(h.state().explore.list_only);
    // Con la palette aperta le scorciatoie sotto non valgono.
    press(&mut h, Key::K, Modifiers::COMMAND);
    press(&mut h, Key::Num1, Modifiers::COMMAND);
    assert_eq!(h.state().screen, Screen::Explore, "Ctrl+1 resta alla palette");
    key(&mut h, Key::Escape);
}

#[test]
fn inline_filters_list_the_items_of_the_open_folder() {
    // I filtri in linea senza un nome cercato valgono nella cartella aperta, a ogni profondità.
    use quado_ui::tokens::Gestore;
    let mut h = loaded(false, "it");
    let capcut = node_named(&h, "CapCut");
    {
        let a = h.state_mut();
        a.go(Screen::Explore);
        let d = a.data.as_ref().expect("dati");
        a.explore.open(d, capcut, None);
    }
    h.run_steps(3);
    // «Gestito da Tutti» apre il suo menu; «Tuoi file» filtra.
    let (chip, mine, clear) = {
        let l = &h.state().lingue;
        (format!("{} {}", l.t("filtro-gestore"), l.t("filtro-gestore-tutti")), l.t(Gestore::Mine.key()), l.t("filtro-azzera"))
    };
    h.get_by_label(&chip).click();
    h.run_steps(2);
    h.query_all_by(|n| n.label().is_some_and(|x| x == mine) && n.toggled().is_some()).next().expect("voce del menu").click();
    h.run_steps(2);
    wait_jobs(&mut h);
    let a = h.state();
    assert!(a.search.filtering(), "solo filtri, nessun nome");
    assert_eq!(a.explore.outline.root, capcut, "la cartella aperta resta");
    let d = a.data.as_ref().expect("dati");
    let t = d.tree();
    let hits: std::collections::BTreeSet<u32> = a.search.outcome.as_ref().expect("risultati").hits.iter().copied().collect();
    let want: std::collections::BTreeSet<u32> = (1..t.len() as u32)
        .filter(|&i| quado_model::view::is_ancestor(t, capcut, i))
        .filter(|&i| !t.flags(i).has(quado_model::Flags::SECONDARY_LINK))
        .filter(|&i| quado_ui::app::data::group_code(&d.cls, i) == Gestore::Mine as u8)
        .collect();
    assert!(!want.is_empty());
    assert_eq!(hits, want);
    // «Togli i filtri» torna alla mappa e all'albero.
    h.get_by_label(&clear).click();
    h.run_steps(3);
    assert!(!h.state().search.showing());
}

#[test]
fn overview_opens_with_the_answer_and_the_why_row_opens() {
    // Panoramica: le cifre dell'usato e del libero per prime; la riga dei conti chiusa con il non attribuito
    // in vista, aperta con "Osservato"; ogni voce degli interventi ha il suo pulsante successivo. Finestra
    // alta: la riga dei conti si vede senza scorrere.
    let mut a = app("it");
    let (v, r) = synthetic::sample_c(false);
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut h = Harness::builder().with_size([1600.0, 2600.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(4);
    let (used, observed, why, next, see) = {
        let a = h.state();
        let b = a.data.as_ref().expect("dati").balance.expect("bilancio");
        let l = &a.lingue;
        (a.fmt.size(b.used), l.t("conti-osservato"), l.t("conti-titolo"), [l.t("azione-prepara-cestino"), l.t("interventi-cosa-puoi-fare")], l.t("interventi-cosa-puoi-fare"))
    };
    assert!(h.query_by_label(&used).is_some(), "cifra dello spazio usato");
    assert!(h.query_by_label(&observed).is_none(), "la riga dei conti è chiusa");
    let buttons = next.iter().map(|l| h.query_all_by_label(l).count()).sum::<usize>();
    assert!(buttons >= 3, "pulsanti successivi nella card: {buttons}");
    h.get_by_label(&why).click();
    h.run_steps(3);
    assert!(h.query_by_label(&observed).is_some(), "aperta: Osservato");
    // «Vedi cosa puoi fare» apre l'elemento in Esplora, dove l'ispettore dice cosa puoi fare.
    h.query_all_by_label(&see).next().expect("pulsante").click();
    h.run_steps(3);
    assert_eq!(h.state().screen, Screen::Explore);
    assert!(h.state().explore.selected.is_some());
}

#[test]
fn a_name_in_a_rare_script_loads_only_the_fonts_it_needs() {
    // Con i font di Windows accesi, come nell'app: un nome in thai fa caricare in un altro thread il font che
    // lo copre (Leelawadee UI) e il C: di prova uno per il suo nome giapponese, non i sette di prima (niente
    // coreano né scritture indiane); poi il nome si disegna con i suoi glifi.
    let dir = std::env::var_os("WINDIR").map(|w| PathBuf::from(w).join("Fonts"));
    if !dir.is_some_and(|d| d.join("LeelawUI.ttf").is_file()) {
        return;
    }
    let mut env = Env::fixed();
    env.system_fonts = true;
    let mut a = App::new(env, Vec::new());
    let (v, mut r) = synthetic::sample_c(false);
    let name: Vec<u16> = "ภาษาไทย.txt".encode_utf16().collect();
    r.tree.push(Tree::ROOT, &name, quado_model::NodeData { logical: 10, alloc: 4096, ..Default::default() });
    r.tree.aggregate();
    r.tree.seal();
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut h = harness(a);
    let thai = |h: &Harness<'_, App>| h.ctx.fonts_mut(|f| f.has_glyph(&egui::FontId::proportional(13.0), 'ภ'));
    h.run_steps(2);
    let started = std::time::Instant::now();
    while !thai(&h) && started.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(20));
        h.step();
    }
    assert!(thai(&h), "il nome in thai ha i suoi glifi");
    let files: Vec<String> = h.ctx.fonts(|f| f.definitions().font_data.keys().filter(|k| k.starts_with("sys:")).cloned().collect());
    assert!(files.contains(&"sys:LeelawUI.ttf".to_owned()), "{files:?}");
    assert!(!files.iter().any(|f| f == "sys:malgun.ttf" || f == "sys:Nirmala.ttf"), "{files:?}");
    assert!(files.len() <= 3, "{files:?}");
    let jp = |h: &Harness<'_, App>| h.ctx.fonts_mut(|f| "写真アルバム".chars().all(|c| f.has_glyph(&egui::FontId::monospace(13.0), c)));
    assert!(jp(&h), "anche il nome giapponese");
}

#[test]
fn a_scan_result_waits_for_its_fonts_before_the_overview() {
    // Fine di una scansione con un nome in thai: il risultato resta in attesa finché i font di ripiego sono
    // scelti (in un altro thread) e attivi, poi la Panoramica si disegna già con i glifi giusti. Così egui
    // dispone il testo della prima Panoramica una volta sola, non prima con i font di base e poi di nuovo.
    let dir = std::env::var_os("WINDIR").map(|w| PathBuf::from(w).join("Fonts"));
    if !dir.is_some_and(|d| d.join("LeelawUI.ttf").is_file()) {
        return;
    }
    let mut env = Env::fixed();
    env.system_fonts = true;
    let mut a = App::new(env, Vec::new());
    let (v, mut r) = synthetic::sample_c(false);
    let name: Vec<u16> = "ภาษาไทย.txt".encode_utf16().collect();
    r.tree.push(Tree::ROOT, &name, quado_model::NodeData { logical: 10, alloc: 4096, ..Default::default() });
    r.tree.aggregate();
    r.tree.seal();
    let loaded = quado_ui::app::data::Loaded::with_rules(v.clone(), r, Duration::from_secs(14), None, std::sync::Arc::clone(&a.kb), synthetic::sample_c_evidence());
    a.job = Some(quado_ui::app::data::ScanJob::done(v, loaded));
    a.screen = Screen::Scanning;
    // Un frame per volta, senza `Harness` (che all'inizio gira fino a quando l'interfaccia è ferma).
    let ctx = egui::Context::default();
    let input = || egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0))), ..Default::default() };
    let thai = |ctx: &egui::Context| ctx.fonts_mut(|f| f.has_glyph(&egui::FontId::proportional(13.0), 'ภ'));
    let started = std::time::Instant::now();
    let mut frames_waiting = 0;
    loop {
        ctx.run_ui(input(), |ui| a.show(ui)).textures_delta.clear();
        if a.data.is_some() || started.elapsed() > Duration::from_secs(20) {
            break;
        }
        assert_eq!(a.screen, Screen::Scanning, "in attesa resta la schermata della scansione");
        assert!(a.job.is_some());
        frames_waiting += 1;
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(a.data.is_some(), "il risultato è arrivato");
    // Il primo frame installa i font di Inter, poi uno con il risultato in attesa e la scelta in un altro
    // thread, uno che installa i ripieghi, e il risultato arriva quando valgono.
    assert!(frames_waiting >= 3, "frame di attesa: {frames_waiting}");
    assert!(thai(&ctx), "la Panoramica arriva con i font del nome in thai già attivi");
    assert_eq!(a.screen, Screen::Overview);
    assert!(a.job.is_none());
}
