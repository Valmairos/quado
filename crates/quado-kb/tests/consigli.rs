// SPDX-License-Identifier: GPL-3.0-or-later
//! Comportamenti nuovi della valutazione dei consigli (01/11/2026): regola dei progetti con il nome abbinato
//! al suo lockfile e senza le cartelle dei programmi, regole fuori dalla card, rischi dell'ibernazione solo
//! con la batteria, WinSxS e Windows.old come spiegazione, campi nuovi delle regole.

mod common;

use quado_kb::engine::{Classification, interventions};
use quado_kb::extract::Power;
use quado_kb::rules::{Rule, RuleIssue};
use quado_kb::{Kb, classify};
use quado_model::record::{Action, Date, Value};
use quado_model::{Flags, NodeData, Tree};

const TODAY: Date = Date { year: 2026, month: 11, day: 1 };

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Albero dai percorsi: le cartelle finiscono con `\`, i file hanno 1 MiB.
fn tree(paths: &[&str]) -> Tree {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let mut index: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for p in paths {
        let dir = p.ends_with('\\');
        let p = p.trim_end_matches('\\');
        let mut parent = Tree::ROOT;
        let mut acc = String::new();
        let segs: Vec<&str> = p.split('\\').skip(1).collect();
        for (k, seg) in segs.iter().enumerate() {
            acc.push('\\');
            acc.push_str(seg);
            let last = k + 1 == segs.len();
            if let Some(&n) = index.get(&acc.to_lowercase()) {
                parent = n;
                continue;
            }
            let data = if last && !dir { NodeData { alloc: 1 << 20, logical: 1 << 20, ..Default::default() } } else { NodeData { flags: Flags::DIR, ..Default::default() } };
            let n = t.push(parent, &w(seg), data);
            index.insert(acc.to_lowercase(), n);
            parent = n;
        }
    }
    t.aggregate();
    t.seal();
    t
}

fn rule_of(kb: &Kb, cls: &Classification, tree: &Tree, path: &str) -> Option<String> {
    let n = quado_kb::engine::find(tree, path)?;
    cls.own(n).map(|m| kb.rules[cls.matches[m].rule].id.clone())
}

#[test]
fn projects_need_the_lockfile_of_their_kind_and_stay_out_of_program_folders() {
    let kb = Kb::embedded().expect("regole");
    let t = tree(&[
        r"C:\Users\demo\Documents\rust\Cargo.lock",
        r"C:\Users\demo\Documents\rust\target\x.o",
        r"C:\Users\demo\Documents\web\package-lock.json",
        r"C:\Users\demo\Documents\web\target\x.o",
        r"C:\Users\demo\Documents\web\build\icon.ico",
        r"C:\Users\demo\Documents\web\node_modules\a.js",
        r"C:\Users\demo\Documents\next\yarn.lock",
        r"C:\Users\demo\Documents\next\.next\b.js",
        r"C:\Users\demo\Documents\cargo-web\Cargo.lock",
        r"C:\Users\demo\Documents\cargo-web\node_modules\a.js",
        r"C:\Users\demo\AppData\Local\Programs\editor\resources\app\package-lock.json",
        r"C:\Users\demo\AppData\Local\Programs\editor\resources\app\node_modules\a.js",
        r"C:\Users\demo\.vscode\extensions\estensione\package-lock.json",
        r"C:\Users\demo\.vscode\extensions\estensione\node_modules\a.js",
        r"C:\Program Files\App\resources\package-lock.json",
        r"C:\Program Files\App\resources\node_modules\a.js",
    ]);
    let mut ev = common::evidence();
    ev.npm_cache.path = r"C:\nessuna-cache".into();
    let cls = classify(&kb, &t, &ev, TODAY);
    let id = |p: &str| rule_of(&kb, &cls, &t, p);
    let dev = Some("progetti-sviluppo".to_owned());
    assert_eq!(id(r"C:\Users\demo\Documents\rust\target"), dev, "target con Cargo.lock");
    assert_eq!(id(r"C:\Users\demo\Documents\web\node_modules"), dev, "node_modules con package-lock.json");
    assert_eq!(id(r"C:\Users\demo\Documents\next\.next"), dev, ".next con yarn.lock");
    assert_eq!(id(r"C:\Users\demo\Documents\web\target"), None, "target accanto a un lockfile JavaScript");
    assert_eq!(id(r"C:\Users\demo\Documents\web\build"), None, "build non è più riconosciuta");
    assert_eq!(id(r"C:\Users\demo\Documents\cargo-web\node_modules"), None, "node_modules accanto a Cargo.lock");
    for p in [
        r"C:\Users\demo\AppData\Local\Programs\editor\resources\app\node_modules",
        r"C:\Users\demo\.vscode\extensions\estensione\node_modules",
        r"C:\Program Files\App\resources\node_modules",
    ] {
        assert_ne!(id(p), dev, "{p}: cartella di un programma");
    }
    // La riga dice il comando che di solito ricrea la cartella, come deduzione.
    let cmd = |p: &str| {
        let n = quado_kb::engine::find(&t, p).expect("nodo");
        cls.matches[cls.own(n).expect("regola")].arg("comando").cloned()
    };
    assert_eq!(cmd(r"C:\Users\demo\Documents\rust\target"), Some(Value::Text("cargo build".into())));
    assert_eq!(cmd(r"C:\Users\demo\Documents\web\node_modules"), Some(Value::Text("npm install".into())));
    assert_eq!(cmd(r"C:\Users\demo\Documents\next\.next"), Some(Value::Text("yarn run build".into())));
    for (tag, src) in quado_kb::TEXTS {
        let line = src.lines().find(|l| l.starts_with("regola-progetti-sviluppo-riga =")).expect("riga");
        assert!(line.contains("{ $comando }"), "{tag}: {line}");
    }
    // Il controllo dei riferimenti si fa anche per i progetti.
    let n = quado_kb::engine::find(&t, r"C:\Users\demo\Documents\rust\target").expect("target");
    let r = &cls.matches[cls.own(n).expect("regola")].record;
    assert!(r.checks.iter().any(|c| c.key == "controllo-riferimenti"));
}

#[test]
fn hibernation_winsxs_and_browser_caches_stay_out_of_the_card() {
    let kb = Kb::embedded().expect("regole");
    let off: Vec<&str> = kb.rules.iter().filter(|r| !r.in_card).map(|r| r.id.as_str()).collect();
    assert_eq!(off, vec!["ibernazione", "cache-brave", "cache-chrome", "cache-edge"]);
    let t = common::tree(&["radice.albero", "browser.albero"]);
    let cls = classify(&kb, &t, &common::evidence(), TODAY);
    // Le regole ci sono e propongono ancora qualcosa nell'ispettore...
    let proposing = |id: &str| cls.matches.iter().any(|m| kb.rules[m.rule].id == id && m.record.state.proposes() && m.record.action != Action::None);
    assert!(proposing("ibernazione"));
    assert!(proposing("cache-chrome"));
    // ...ma non nella card.
    let card: Vec<&str> = interventions(&kb, &cls, &t).iter().map(|i| kb.rules[i.rule].id.as_str()).collect();
    for id in ["ibernazione", "cache-chrome", "cache-edge", "cache-brave", "winsxs"] {
        assert!(!card.contains(&id), "{id} nella card: {card:?}");
    }
    assert!(card.contains(&"windows-old"), "Windows.old nella card: {card:?}");
}

#[test]
fn winsxs_is_explanation_only_and_windows_old_is_managed_by_windows() {
    let kb = Kb::embedded().expect("regole");
    let t = common::tree(&["radice.albero"]);
    let cls = classify(&kb, &t, &common::evidence(), TODAY);
    let rec = |p: &str| {
        let n = quado_kb::engine::find(&t, p).expect("nodo");
        cls.matches[cls.own(n).expect("regola")].record.clone()
    };
    let w = rec(r"C:\Windows\WinSxS");
    assert_eq!(w.action, Action::None);
    assert!(w.preferred.is_none() && w.alternatives.is_empty(), "nessun comando DISM");
    assert_eq!(w.state.code(), "lo-gestisce-nessuna-procedura", "resta l'avviso «non toccare»");
    let o = rec(r"C:\Windows.old");
    assert_eq!(o.action, Action::Procedure);
    assert_eq!(o.preferred.as_ref().and_then(|s| s.uri.as_deref()), Some("ms-settings:storagesense"));
    assert_eq!(o.risks, vec!["regola-windows-old-rischio-1".to_owned()]);
}

#[test]
fn hibernation_battery_risks_only_with_a_battery() {
    let kb = Kb::embedded().expect("regole");
    let t = common::tree(&["radice.albero"]);
    let risks = |power: Power| {
        let mut ev = common::evidence();
        ev.power = Ok(power);
        let cls = classify(&kb, &t, &ev, TODAY);
        let n = quado_kb::engine::find(&t, r"C:\hiberfil.sys").expect("hiberfil");
        cls.matches[cls.own(n).expect("regola")].record.risks.clone()
    };
    let base = Power { hibernate: true, hiberfile_present: true, fast_startup: Some(true), has_battery: false, modern_standby: false, critical_battery_action: None };
    assert_eq!(risks(base), vec!["regola-ibernazione-rischio-1".to_owned()], "PC fisso: niente frasi sulla batteria");
    let laptop = Power { has_battery: true, critical_battery_action: Some(2), ..base };
    assert_eq!(risks(laptop), vec!["regola-ibernazione-rischio-1".to_owned(), "regola-ibernazione-rischio-batteria".to_owned()]);
    let modern = Power { modern_standby: true, ..laptop };
    assert_eq!(
        risks(modern),
        vec!["regola-ibernazione-rischio-1".to_owned(), "regola-ibernazione-rischio-batteria".to_owned(), "regola-ibernazione-rischio-standby".to_owned()]
    );
    // Standby moderno senza batteria (un fisso): nessun avviso.
    assert_eq!(risks(Power { modern_standby: true, ..base }), vec!["regola-ibernazione-rischio-1".to_owned()]);
    for (tag, src) in quado_kb::TEXTS {
        for k in ["regola-ibernazione-rischio-batteria", "regola-ibernazione-rischio-standby", "fatto-standby-moderno"] {
            assert!(src.lines().any(|l| l.starts_with(&format!("{k} ="))), "{tag}: {k}");
        }
    }
}

#[test]
fn capcut_and_projects_say_what_to_try_before_emptying_the_bin() {
    let kb = Kb::embedded().expect("regole");
    let rule = |id: &str| kb.rules.iter().find(|r| r.id == id).expect("regola").clone();
    let capcut = rule("capcut-versioni");
    assert!(capcut.try_before_empty && capcut.tick_tried);
    assert!(capcut.text_keys().contains(&"regola-capcut-versioni-prova-fatta".to_owned()));
    let dev = rule("progetti-sviluppo");
    assert!(dev.try_before_empty && !dev.tick_tried);
    assert!(kb.rules.iter().filter(|r| r.try_before_empty).all(|r| r.action == quado_kb::rules::ActionCode::Recycle));
}

#[test]
fn rule_files_reject_bad_new_fields() {
    let good = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("regole/capcut-versioni.toml")).expect("regola");
    assert!(Rule::parse("capcut-versioni.toml", &good).is_ok());
    // La spunta senza il testo della prova.
    let tick_only = good.replacen("prova_prima_di_svuotare = true\n", "", 1);
    assert_eq!(Rule::parse("capcut-versioni.toml", &tick_only).map(|_| ()), Err(RuleIssue::Method));
    let dev = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("regole/progetti-sviluppo.toml")).expect("regola");
    assert!(Rule::parse("progetti-sviluppo.toml", &dev).is_ok());
    // Un codice di cartella sconosciuto in `escludi`, un nome senza lockfile.
    let bad_code = dev.replacen("\"profilo:AppData\"", "\"altrove:AppData\"", 1);
    assert_eq!(Rule::parse("progetti-sviluppo.toml", &bad_code).map(|_| ()), Err(RuleIssue::Place));
    let no_lock = dev.replacen("lockfile = [\"Cargo.lock\"]", "lockfile = []", 1);
    assert_eq!(Rule::parse("progetti-sviluppo.toml", &no_lock).map(|_| ()), Err(RuleIssue::Place));
}
