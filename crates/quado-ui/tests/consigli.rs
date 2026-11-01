// SPDX-License-Identifier: GPL-3.0-or-later
//! Consigli nell'interfaccia dopo la valutazione del 01/11/2026:
//! - nella card «Interventi possibili» la cifra grande ha l'etichetta «Occupa» e sotto lo stato c'è quanto si
//!   libera e quando (svuotando il Cestino, o dopo la procedura);
//! - per Da verificare la card porta all'ispettore, mai direttamente all'anteprima;
//! - ibernazione e cache dei browser non sono nella card;
//! - nella vista Regole una fonte che è un file del PC si descrive come tale, senza il nome di un utente.

use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use quado_ui::app::{App, Env, Screen, kbtext, synthetic};

fn app(lang: &str) -> Harness<'static, App> {
    let (v, r) = synthetic::sample_c(false);
    let mut env = Env::fixed();
    env.windows_languages = vec![if lang == "en" { "en-US".into() } else { "it-IT".into() }];
    let mut a = App::new(env, Vec::new());
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut h = Harness::builder().with_size([1600.0, 2600.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
    h.run_steps(4);
    h
}

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
fn the_card_says_what_an_item_uses_and_when_space_is_freed() {
    for lang in ["it", "en"] {
        let h = app(lang);
        assert_eq!(h.state().screen, Screen::Overview);
        let a = h.state();
        let l = &a.lingue;
        let texts = drawn(&h);
        assert!(texts.iter().any(|t| t == &l.t("interventi-occupa")), "{lang}: etichetta «Occupa»");
        let freed = l.ta("interventi-libera-cestino", &[("valore", "X".into())]);
        let prefix = freed.split('X').next().expect("prefisso");
        assert!(texts.iter().any(|t| t.starts_with(prefix)), "{lang}: «{prefix}…» nella card");
        assert!(!texts.iter().any(|t| t.starts_with(&l.ta("stima-esatta", &[("valore", "X".into())]).replace('X', ""))), "{lang}: la card non dice più «Stima esatta»");
        // Le versioni di CapCut (Da verificare): «Vedi cosa puoi fare», mai «Sposta nel Cestino…» dalla card.
        let d = a.data.as_ref().expect("dati");
        let m = d.cls.matches.iter().find(|m| d.kb.rules[m.rule].id == "capcut-versioni").expect("CapCut");
        let title = kbtext::rule_line(l, &a.fmt, &d.kb, m);
        let next_for = |label: &str| h.query_all_by_label(label).filter(|n| n.accesskit_node().description().as_deref() == Some(title.as_str())).count();
        assert_eq!(next_for(&l.t("interventi-cosa-puoi-fare")), 1, "{lang}: CapCut porta all'ispettore");
        assert_eq!(next_for(&l.t("azione-prepara-cestino")), 0, "{lang}: niente spostamento diretto per Da verificare");
        // Ibernazione e cache dei browser non sono voci della card.
        for id in ["ibernazione", "cache-chrome", "cache-edge", "cache-brave", "winsxs"] {
            for m in d.cls.matches.iter().filter(|m| d.kb.rules[m.rule].id == id) {
                let line = kbtext::rule_line(l, &a.fmt, &d.kb, m);
                let in_card = h.query_all_by_label(&line).any(|n| n.accesskit_node().description().is_some_and(|x| x.contains(&l.t("interventi-occupa"))));
                assert!(!in_card, "{lang}: {id} nella card");
            }
        }
    }
}

#[test]
fn rules_view_describes_file_sources_without_a_user_name() {
    let mut h = app("it");
    {
        let a = h.state_mut();
        a.rules_view.selected = a.kb.index("capcut-versioni");
        a.rules_view.scroll_to = true;
        a.go(Screen::Rules);
    }
    h.run_steps(6);
    let texts = drawn(&h);
    let l = &h.state().lingue;
    assert!(texts.iter().any(|t| t == &l.t("regole-fonte-file")), "etichetta della fonte sul PC");
    assert!(texts.iter().any(|t| t == r"%LOCALAPPDATA%\CapCut\Apps\ProductInfo.xml"), "percorso dalla cartella nota");
    assert!(!texts.iter().any(|t| t.contains("Users/demo") || t.contains(r"Users\demo\AppData\Local\CapCut\Apps\ProductInfo")), "nessun utente nella fonte");
    // L'indirizzo di un file del PC non si copia: «Copia indirizzo» non c'è per questa fonte.
    assert_eq!(h.query_all_by_label(&l.t("regole-copia-link")).count(), 0);
}
