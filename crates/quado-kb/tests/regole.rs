// SPDX-License-Identifier: GPL-3.0-or-later
//! Tabella attesa per regola: sulla fixture reale della regola il record prodotto
//! coincide campo per campo con `tests/attese/<id>.toml`: stato, azione, metodo e alternative,
//! controlli, stato della stima, privilegi, gestore, colore e numero di elementi.

mod common;

use std::path::Path;

use quado_kb::engine::{self, Classification};
use quado_kb::{Kb, classify};
use quado_model::record::{CheckKind, ColorGroup, Date, Estimate, Privileges, Value};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct Voce {
    percorso: String,
    stato: String,
    azione: String,
    /// Comando o URI letterale del metodo preferito; "" se il metodo è solo un testo; assente se non c'è.
    #[serde(default)]
    metodo: Option<String>,
    #[serde(default)]
    alternative: Vec<String>,
    #[serde(default)]
    controlli: Vec<String>,
    stima: String,
    privilegi: String,
    gestore: String,
    colore: String,
    elementi: usize,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct Attesa {
    voci: Vec<Voce>,
}

pub const TODAY: Date = Date { year: 2026, month: 10, day: 13 };

fn literal(s: &quado_model::record::Step) -> String {
    s.command.clone().or_else(|| s.uri.clone()).unwrap_or_default()
}

fn describe(cls: &Classification, m: usize) -> Voce {
    let x = &cls.matches[m];
    let r = &x.record;
    Voce {
        percorso: String::new(),
        stato: r.state.code().to_owned(),
        azione: r.action.code().to_owned(),
        metodo: r.preferred.as_ref().map(literal),
        alternative: r.alternatives.iter().map(literal).collect(),
        controlli: r.checks.iter().map(|c| format!("{}:{}", c.key, if c.kind == CheckKind::Automatic { "automatico" } else { "manuale" })).collect(),
        stima: match r.estimate {
            Estimate::Exact(_) => "esatta".into(),
            Estimate::Range { .. } => "intervallo".into(),
            Estimate::NotEstimable(n) => format!("non-stimabile-{}", format!("{n:?}").to_lowercase()),
        },
        privilegi: if r.privileges == Privileges::Admin { "amministratore".into() } else { "nessuno".into() },
        gestore: x.manager.code().to_owned(),
        colore: match x.color() {
            ColorGroup::Manager(m) => m.code().to_owned(),
            ColorGroup::Unclassified => "grigio".into(),
        },
        elementi: x.nodes.len(),
    }
}

#[test]
fn every_rule_matches_its_expected_table() {
    let kb = Kb::embedded().expect("regole");
    let ev = common::evidence();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("attese");
    let print = std::env::var_os("QUADO_STAMPA_ATTESE").is_some();
    let mut problems = Vec::new();
    for (idx, rule) in kb.rules.iter().enumerate() {
        let tree = common::tree(&[&rule.fixture]);
        let cls = classify(&kb, &tree, &ev, TODAY);
        if print {
            println!("=== {}", rule.id);
            for (k, m) in cls.matches.iter().enumerate().filter(|(_, m)| m.rule == idx) {
                let mut v = describe(&cls, k);
                v.percorso = m.nodes.first().map(|&n| tree.path_lossy(n)).unwrap_or_default();
                println!("{v:?}");
            }
            continue;
        }
        let text = match std::fs::read_to_string(dir.join(format!("{}.toml", rule.id))) {
            Ok(t) => t,
            Err(_) => {
                problems.push(format!("{}: tabella attesa mancante", rule.id));
                continue;
            }
        };
        let attesa: Attesa = toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", rule.id));
        assert!(!attesa.voci.is_empty(), "{}: tabella vuota", rule.id);
        for want in attesa.voci {
            let Some(node) = engine::find(&tree, &want.percorso) else {
                problems.push(format!("{}: {} non è nella fixture", rule.id, want.percorso));
                continue;
            };
            let Some(m) = cls.match_of(node) else {
                problems.push(format!("{}: {} senza regola", rule.id, want.percorso));
                continue;
            };
            if cls.matches[m].rule != idx {
                problems.push(format!("{}: {} riconosciuto da {}", rule.id, want.percorso, kb.rules[cls.matches[m].rule].id));
                continue;
            }
            let mut got = describe(&cls, m);
            got.percorso = want.percorso.clone();
            let same = got.stato == want.stato
                && got.azione == want.azione
                && got.metodo == want.metodo
                && got.alternative == want.alternative
                && got.controlli == want.controlli
                && got.stima == want.stima
                && got.privilegi == want.privilegi
                && got.gestore == want.gestore
                && got.colore == want.colore
                && got.elementi == want.elementi;
            if !same {
                problems.push(format!("{}:\n  atteso   {want:?}\n  prodotto {got:?}", rule.id));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn recycle_bin_without_the_windows_count_has_its_own_text() {
    // Senza il conteggio di Windows il fatto non ha valori: usa una chiave senza segnaposto, non quella con
    // `$a` e `$b`, che mostrerebbe i segnaposto grezzi ("Secondo Windows:  in {$b_testo} elementi").
    let kb = Kb::embedded().expect("regole");
    let mut ev = common::evidence();
    ev.recycle = Err(quado_kb::extract::ExtractError::Io(5));
    let (idx, rule) = kb.rules.iter().enumerate().find(|(_, r)| r.id == "cestino").expect("regola del Cestino");
    let tree = common::tree(&[&rule.fixture]);
    let cls = classify(&kb, &tree, &ev, TODAY);
    let m = cls.matches.iter().find(|m| m.rule == idx).expect("Cestino riconosciuto");
    let windows: Vec<_> = m.record.identity.0.iter().filter(|f| f.key.starts_with("fatto-cestino-secondo-windows")).collect();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].key, "fatto-cestino-secondo-windows-non-letto");
    assert!(windows[0].values.is_empty());
    // La chiave esiste nelle due lingue e non ha segnaposto.
    for (tag, src) in quado_kb::TEXTS {
        let line = src.lines().find(|l| l.starts_with("fatto-cestino-secondo-windows-non-letto =")).unwrap_or_else(|| panic!("{tag}: chiave mancante"));
        assert!(!line.contains('{'), "{tag}: {line}");
    }
}

#[test]
fn steam_app_id_is_text_not_a_count() {
    // L'ID di un'app di Steam è un identificativo: arriva come testo, così nessuno lo scrive con il separatore
    // delle migliaia ("ID app 3.999.910").
    let kb = Kb::embedded().expect("regole");
    let ev = common::evidence();
    let (idx, rule) = kb.rules.iter().enumerate().find(|(_, r)| r.id == "steam").expect("regola di Steam");
    let tree = common::tree(&[&rule.fixture]);
    let cls = classify(&kb, &tree, &ev, TODAY);
    let facts: Vec<_> = cls.matches.iter().filter(|m| m.rule == idx).flat_map(|m| m.record.identity.0.iter()).filter(|f| f.key == "fatto-gioco-steam").collect();
    assert!(!facts.is_empty(), "giochi di Steam");
    for f in facts {
        match &f.values[1] {
            Value::Text(id) => assert!(!id.is_empty() && id.chars().all(|c| c.is_ascii_digit()), "{id}"),
            v => panic!("ID come {v:?}"),
        }
    }
}
