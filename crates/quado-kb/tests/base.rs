// SPDX-License-Identifier: GPL-3.0-or-later
//! Test sulla base delle regole (06 §2.2, RS10): ogni regola ha almeno una fonte con URL ed estratto,
//! una fixture, una data di verifica, una tabella attesa e i testi in italiano e in inglese.

use std::collections::BTreeSet;
use std::path::Path;

use quado_kb::Kb;

fn keys(ftl: &str) -> BTreeSet<String> {
    ftl.lines()
        .filter(|l| !l.starts_with(['#', ' ', '\t', '.', '[', '*', '}']) && l.contains(" ="))
        .filter_map(|l| l.split_once(" =").map(|(k, _)| k.trim().to_owned()))
        .collect()
}

#[test]
fn every_rule_has_source_fixture_date_table_and_texts() {
    let kb = Kb::embedded().unwrap_or_else(|e| panic!("regola non valida: {e:?} ({})", quado_kb::rules::RULE_FILES[e.file].0));
    assert!(kb.rules.len() >= 20, "regole: {}", kb.rules.len());
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let it = keys(quado_kb::TEXTS[0].1);
    let en = keys(quado_kb::TEXTS[1].1);
    let mut problems = Vec::new();
    for r in &kb.rules {
        if r.sources.is_empty() || r.sources.iter().any(|s| s.url.is_empty() || s.estratto.trim().is_empty()) {
            problems.push(format!("{}: fonte senza URL o estratto", r.id));
        }
        // Un documento del repository c'è e contiene l'estratto.
        for s in r.sources.iter().filter(|s| s.url.starts_with("docs/")) {
            match std::fs::read_to_string(dir.join("..").join("..").join(&s.url)) {
                Ok(text) if text.contains(s.estratto.trim()) => {}
                Ok(_) => problems.push(format!("{}: l'estratto non è in {}", r.id, s.url)),
                Err(e) => problems.push(format!("{}: {} non letto ({e})", r.id, s.url)),
            }
        }
        if !dir.join("fixture").join(&r.fixture).is_file() {
            problems.push(format!("{}: fixture {} mancante", r.id, r.fixture));
        }
        if !dir.join("tests").join("attese").join(format!("{}.toml", r.id)).is_file() {
            problems.push(format!("{}: tabella attesa mancante", r.id));
        }
        for k in r.text_keys() {
            if !it.contains(&k) {
                problems.push(format!("{}: «{k}» manca in regole/it.ftl", r.id));
            }
            if !en.contains(&k) {
                problems.push(format!("{}: «{k}» manca in regole/en.ftl", r.id));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn rule_files_reject_missing_sources_and_bad_exceptions() {
    use quado_kb::rules::{Rule, RuleIssue};
    let good = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("regole/winsxs.toml")).expect("regola");
    assert!(Rule::parse("winsxs.toml", &good).is_ok());
    assert_eq!(Rule::parse("altro.toml", &good).map(|_| ()), Err(RuleIssue::Id));
    // Senza fonti.
    let no_source: String = good.split("[[fonti]]").next().unwrap_or_default().to_owned();
    assert_eq!(Rule::parse("winsxs.toml", &no_source).map(|_| ()), Err(RuleIssue::Source));
    // Fonte con un indirizzo che non è una pagina, un file del PC o un documento del repository.
    let start = good.find("url = \"").expect("url") + "url = \"".len();
    let end = start + good[start..].find('"').expect("fine dell'url");
    let with_url = |u: &str| format!("{}{u}{}", &good[..start], &good[end..]);
    assert!(Rule::parse("winsxs.toml", &with_url("docs/m1/cestino-verifiche.md")).is_ok());
    for bad in ["http://example.com", "C:/Users/demo/a.md", "docs/../segreto.md", r"docs\\m1\\a.md"] {
        assert_eq!(Rule::parse("winsxs.toml", &with_url(bad)).map(|_| ()), Err(RuleIssue::Source), "{bad}");
        assert!(!quado_kb::rules::source_url_ok(&bad.replace(r"\\", r"\")), "{bad}");
    }
    assert!(quado_kb::rules::source_url_ok("docs/m1/cestino-verifiche.md"));
    // Estratto vuoto.
    let empty = good.replacen("estratto = '''Deleting", "estratto = ''' '''\nx = '''Deleting", 1);
    assert!(Rule::parse("winsxs.toml", &empty).is_err());
    // Eccezione a F-AC-4 senza stato Da verificare.
    let exc = good.replacen("protetto = true", "protetto = true\neccezione_protetti = true", 1);
    assert_eq!(Rule::parse("winsxs.toml", &exc).map(|_| ()), Err(RuleIssue::Exception));
    // Data non valida.
    let date = good.replacen("2026-10-13", "2026-13-01", 1);
    assert_eq!(Rule::parse("winsxs.toml", &date).map(|_| ()), Err(RuleIssue::Date));
}
