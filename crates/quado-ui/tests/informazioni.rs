// SPDX-License-Identifier: GPL-3.0-or-later
//! Avvisi legali in Impostazioni › Informazioni (GPLv3 §0 e §5(d), "Appropriate Legal Notices"): copyright,
//! licenza e come leggerla (testo incorporato), nessuna garanzia, indirizzo del codice sorgente con «Copia».

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use quado_ui::app::{App, Env, Screen};
use quado_ui::brand;

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
fn the_embedded_license_is_the_gpl_v3() {
    assert!(brand::LICENSE_TEXT.trim_start().starts_with("GNU GENERAL PUBLIC LICENSE"));
    assert!(brand::LICENSE_TEXT.contains("Version 3, 29 June 2007"));
    assert!(brand::LICENSE_TEXT.contains("15. Disclaimer of Warranty."));
    assert_eq!(brand::COPYRIGHT, "Copyright (C) 2026 Valmairos");
    assert_eq!(brand::SOURCE_URL, "https://github.com/Valmairos/quado");
}

#[test]
fn about_shows_copyright_license_warranty_and_source() {
    for lang in ["it", "en"] {
        let mut env = Env::fixed();
        env.windows_languages = vec![if lang == "en" { "en-US".into() } else { "it-IT".into() }];
        let mut a = App::new(env, Vec::new());
        a.go(Screen::Settings);
        // Finestra alta: la sezione Informazioni, l'ultima, si vede senza scorrere.
        let mut h = Harness::builder().with_size([1600.0, 6000.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a);
        h.run_steps(4);
        let l = &h.state().lingue;
        let (licenza, garanzia, testo, copia) = (l.t("info-licenza"), l.t("info-garanzia"), l.t("info-testo-licenza"), l.t("regole-copia-link"));
        let texts = drawn(&h);
        for want in [brand::COPYRIGHT, licenza.as_str(), garanzia.as_str(), brand::SOURCE_URL] {
            assert!(texts.iter().any(|t| t == want), "{lang}: manca «{want}»");
        }
        assert!(licenza.contains("GNU General Public License"), "{lang}: {licenza}");
        assert!(garanzia.contains("15") && garanzia.contains("16"), "{lang}: {garanzia}");
        // Il testo della licenza si apre dall'app, senza rete.
        assert!(!texts.iter().any(|t| t.contains("GNU GENERAL PUBLIC LICENSE")), "{lang}: chiuso all'inizio");
        h.get_by_label(&testo).click();
        h.run_steps(3);
        assert!(drawn(&h).iter().any(|t| t.contains("GNU GENERAL PUBLIC LICENSE")), "{lang}: testo della licenza");
        // «Copia indirizzo» accanto all'indirizzo del codice sorgente.
        assert!(h.query_all_by_label(&copia).next().is_some(), "{lang}: «{copia}»");
    }
}
