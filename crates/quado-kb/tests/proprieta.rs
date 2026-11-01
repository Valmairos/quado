// SPDX-License-Identifier: GPL-3.0-or-later
//! Test di proprietà sul modello delle raccomandazioni (06 §2.2) e controlli del caso CapCut
//! (decisioni §3), su alberi e prove generati a caso e sulle fixture reali.

mod common;

use proptest::prelude::*;
use quado_kb::engine::{self, Classification, interventions};
use quado_kb::evidence::Evidence;
use quado_kb::extract::{UninstallEntry, Uninstall};
use quado_kb::{Kb, classify};
use quado_model::record::{Action, ColorGroup, Criterion, Date, Estimate, Expiry, Provenance, Reliability, State, UnknownReason};
use quado_model::{Flags, NodeData, Tree};

const TODAY: Date = Date { year: 2026, month: 10, day: 13 };

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn kb() -> Kb {
    Kb::embedded().expect("regole")
}

/// Albero casuale con i nomi che le regole cercano: profili, CapCut con versioni, cache, cartelle di
/// progetto con e senza lockfile, Windows, il Cestino con cartelle orfane.
fn random_tree(seed: u64, versions: &[String]) -> Tree {
    let mut rng = seed | 1;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let d = NodeData { flags: Flags::DIR, ..Default::default() };
    let f = |n: u64| NodeData { alloc: n, logical: n, ..Default::default() };
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let users = t.push(Tree::ROOT, &w("Users"), d);
    let u = t.push(users, &w("demo"), d);
    let appdata = t.push(u, &w("AppData"), d);
    let local = t.push(appdata, &w("Local"), d);
    let capcut = t.push(local, &w("CapCut"), d);
    let apps = t.push(capcut, &w("Apps"), d);
    t.push(apps, &w("ProductInfo.xml"), f(400));
    t.push(apps, &w("uninst.exe"), f(1 << 20));
    for v in versions {
        let x = t.push(apps, &w(v), d);
        t.push(x, &w("CapCut.exe"), f(next() % (1 << 30)));
    }
    let ud = t.push(capcut, &w("User Data"), d);
    for name in ["Cache", "Projects"] {
        let x = t.push(ud, &w(name), d);
        t.push(x, &w("dati.bin"), f(next() % (1 << 28)));
    }
    let npm = t.push(local, &w("npm-cache"), d);
    let cc = t.push(npm, &w(if next() % 2 == 0 { "_cacache" } else { "altro" }), d);
    t.push(cc, &w("indice"), f(next() % (1 << 26)));
    for name in ["NVIDIA", "Autodesk", "Sconosciuto", "Steam"] {
        let x = t.push(local, &w(name), d);
        t.push(x, &w("a.dat"), f(next() % (1 << 26)));
    }
    let docs = t.push(u, &w("Documents"), d);
    for k in 0..(next() % 6) {
        let p = t.push(docs, &w(&format!("progetto{k}")), d);
        let which = ["node_modules", "target", ".next", "build", "src"][(next() % 5) as usize];
        let nm = t.push(p, &w(which), d);
        t.push(nm, &w("x.js"), f(next() % (1 << 24)));
        // Una node_modules annidata: fa parte della prima.
        let inner = t.push(nm, &w("node_modules"), d);
        t.push(inner, &w("y.js"), f(next() % (1 << 20)));
        if next() % 2 == 0 {
            t.push(p, &w(["package-lock.json", "Cargo.lock", "yarn.lock"][(next() % 3) as usize]), f(4096));
        }
    }
    let win = t.push(Tree::ROOT, &w("Windows"), d);
    for name in ["WinSxS", "Installer", "System32"] {
        let x = t.push(win, &w(name), d);
        t.push(x, &w("c.dll"), f(next() % (1 << 28)));
    }
    t.push(Tree::ROOT, &w("hiberfil.sys"), f(next() % (1 << 34)));
    t.push(Tree::ROOT, &w("pagefile.sys"), f(next() % (1 << 33)));
    let bin = t.push(Tree::ROOT, &w("$Recycle.Bin"), d);
    let mine = t.push(bin, &w(common::SID), d);
    for k in 0..(next() % 4) {
        let r = t.push(mine, &w(&format!("$RAB{k}")), d);
        t.push(r, &w("f.bin"), f(next() % (1 << 30)));
        if next() % 2 == 0 {
            t.push(mine, &w(&format!("$IAB{k}")), f(128));
        }
    }
    t.aggregate();
    t.seal();
    t
}

fn random_evidence(in_use: Option<String>, reg_version: Option<String>) -> Evidence {
    let mut ev = Evidence::empty("C:\\");
    ev.folders = Evidence::default_folders("C:\\", "demo");
    ev.user_sid = Some(common::SID.into());
    ev.capcut = in_use.ok_or(quado_kb::extract::ExtractError::NotPresent);
    let mut entries = vec![UninstallEntry {
        hive: quado_model::record::Hive::User,
        key: "CapCut".into(),
        display_name: "CapCut".into(),
        version: reg_version,
        publisher: Some("Bytedance Pte. Ltd.".into()),
        install_location: None,
        uninstall_string: Some(r#""C:\Users\demo\AppData\Local\CapCut\Apps\uninst.exe""#.into()),
        display_icon: None,
        install_date: None,
        estimated_kb: None,
        system_component: false,
    }];
    for name in ["NVIDIA Graphics Driver", "Autodesk Fusion"] {
        entries.push(UninstallEntry { hive: quado_model::record::Hive::Machine, key: name.into(), display_name: name.into(), ..entries[0].clone() });
    }
    ev.uninstall = Ok(Uninstall { entries, skipped: Vec::new() });
    ev
}

fn version(a: u8, b: u8, c: u8, d: u16) -> String {
    format!("{a}.{b}.{c}.{d}")
}

/// Proprietà valide per ogni classificazione (06 §2.2).
fn check_invariants(kb: &Kb, tree: &Tree, cls: &Classification) {
    for m in &cls.matches {
        let r = &m.record;
        // Gestibile e Da verificare solo con una prova di gestibilità distinta dalla prova di identità.
        if let Some(p) = r.state.manageability() {
            assert!(!p.facts().is_empty());
            for f in p.facts() {
                assert!(!r.identity.0.contains(f), "{}: un fatto di identità usato come prova di gestibilità", r.origin_id());
            }
        }
        // Campi obbligatori: nessuna chiave vuota, regola con data, identità con almeno un fatto.
        assert!(!r.origin_id().is_empty());
        assert!(r.text_keys().all(|k| !k.trim().is_empty()));
        assert!(!r.identity.0.is_empty(), "{}: prova di identità vuota", r.origin_id());
        // Ogni fatto dedotto ha un criterio; il livello del record è quello del criterio.
        for f in &r.identity.0 {
            if let Provenance::Deduced(c) = f.provenance {
                assert!(matches!(c.level(), Reliability::Low | Reliability::Medium | Reliability::High));
            }
        }
        // Euristiche e affidabilità bassa: mai proposte, mai azioni.
        if r.reliability.level() == Reliability::Low {
            assert!(!r.state.proposes() && r.action == Action::None, "{}: proposta con affidabilità bassa", r.origin_id());
        }
        // Non sappiamo: solo misure e "Apri in Esplora file".
        if let State::Unknown(_) = r.state {
            assert_eq!(r.action, Action::None);
            assert!(r.preferred.is_none() && r.alternatives.is_empty() && r.checks.is_empty());
        }
        // Eccezione a F-AC-4 solo con Da verificare.
        if r.protected_exception {
            assert!(matches!(r.state, State::ToVerify(_)));
        }
        assert!(m.nodes.iter().all(|&n| (n as usize) < tree.len()));
        let _ = kb;
    }
    // Colore solo da fatti da fonte o dedotti alta.
    for n in 0..tree.len() as u32 {
        if let ColorGroup::Manager(_) = cls.color(n) {
            let m = cls.match_of(n).expect("corrispondenza");
            assert!(cls.matches[m].attribution.colors(), "{}: colore senza fonte", tree.path_lossy(n));
        }
    }
    // Card interventi: solo voci da record che propongono un'azione.
    for i in interventions(kb, cls, tree) {
        for &k in &i.matches {
            let r = &cls.matches[k].record;
            assert!(r.state.proposes() && r.action != Action::None, "{}: voce senza proposta", r.origin_id());
            assert!(!matches!(r.state, State::Unknown(_) | State::ManagedBy { procedure: false, .. }));
            assert_ne!(kb.rules[cls.matches[k].rule].id, "cestino-orfane");
        }
    }
    // Bilancio per gestore: somma pari all'allocato letto; ogni file conta per il gruppo del suo
    // proprietario (le ipotesi restano nel grigio).
    let b = cls.by_manager;
    assert_eq!(b.windows + b.programs + b.you + b.unclassified, tree.alloc(Tree::ROOT));
    let mut by = [0u64; 4];
    for n in 0..tree.len() as u32 {
        if tree.is_dir(n) {
            continue;
        }
        let slot = match cls.color(n) {
            ColorGroup::Manager(quado_model::record::Manager::Windows) => 0,
            ColorGroup::Manager(quado_model::record::Manager::Programs) => 1,
            ColorGroup::Manager(quado_model::record::Manager::You) => 2,
            ColorGroup::Unclassified => 3,
        };
        by[slot] += tree.alloc(n);
    }
    assert_eq!(by, [b.windows, b.programs, b.you, b.unclassified]);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 200, ..ProptestConfig::default() })]

    #[test]
    fn records_from_random_inputs_keep_the_rules(
        seed in any::<u64>(),
        versions in proptest::collection::vec((5u8..12, 0u8..10, 0u8..3, 1000u16..5000), 0..8),
        pick in proptest::option::of(0usize..8),
        reg_same in any::<bool>(),
    ) {
        let kb = kb();
        let names: Vec<String> = versions.iter().map(|&(a, b, c, d)| version(a, b, c, d)).collect();
        let in_use = pick.and_then(|k| names.get(k).cloned());
        let reg = if reg_same { in_use.clone() } else { Some("1.0.0.1".into()) };
        let tree = random_tree(seed, &names);
        let ev = random_evidence(in_use.clone(), reg);
        let cls = classify(&kb, &tree, &ev, TODAY);
        check_invariants(&kb, &tree, &cls);
        // Versione in uso fuori intervallo, o fonti discordi: niente Da verificare per CapCut.
        let range = kb.rules[kb.index("capcut-versioni").expect("regola")].versions.clone().expect("intervallo");
        let lo = quado_kb::extract::parse_version(&range.min).expect("min");
        let hi = quado_kb::extract::parse_version(&range.max).expect("max");
        for m in cls.matches.iter().filter(|m| kb.rules[m.rule].id == "capcut-versioni") {
            if let State::ToVerify(_) = m.record.state {
                let v = quado_kb::extract::parse_version(in_use.as_deref().expect("versione in uso")).expect("versione");
                prop_assert!(v >= lo && v <= hi, "Da verificare con la versione fuori intervallo");
                prop_assert!(reg_same, "Da verificare con le due fonti discordi");
            }
        }
    }
}

#[test]
fn capcut_case_of_this_pc() {
    // Controllo del caso guida (decisioni §3, 07 §1.4 criterio 6) sulla fixture reale.
    let kb = kb();
    let tree = common::tree(&["capcut.albero"]);
    let cls = classify(&kb, &tree, &common::evidence(), TODAY);
    let m = cls.matches.iter().find(|m| kb.rules[m.rule].id == "capcut-versioni").expect("regola CapCut");
    assert!(matches!(m.record.state, State::ToVerify(_)));
    assert!(m.record.protected_exception, "eccezione a F-AC-4");
    assert_eq!(m.nodes.len(), 9, "9 cartelle di versioni precedenti");
    let apps = engine::find(&tree, r"C:\Users\demo\AppData\Local\CapCut\Apps").expect("Apps");
    for kept in ["9.3.0.3970", "9.4.0.4015"] {
        let n = engine::child(&tree, apps, kept).expect("versione tenuta");
        assert!(!m.nodes.contains(&n), "{kept} è tenuta");
    }
    let Estimate::Exact(bytes) = m.record.estimate else { panic!("stima esatta") };
    assert_eq!(bytes, m.nodes.iter().map(|&n| tree.alloc(n)).sum::<u64>());
    let all: u64 = tree.children(apps).filter(|&c| quado_kb::extract::parse_version(&tree.name_lossy(c)).is_some_and(|v| v.len() == 4)).map(|c| tree.alloc(c)).sum();
    let kept: u64 = ["9.3.0.3970", "9.4.0.4015"].iter().map(|v| tree.alloc(engine::child(&tree, apps, v).expect("tenuta"))).sum();
    assert_eq!(bytes, all - kept, "la stima esclude le ultime due versioni");
    assert_eq!(m.record.reliability, Criterion::StructureOnly, "che non servano più è dedotto");
    // La versione in uso è da fonte con due fonti concordi; le cartelle hanno il colore dei programmi.
    let sources: Vec<_> = m.record.identity.0.iter().filter(|f| f.key == "fatto-versione-in-uso").map(|f| f.source.clone()).collect();
    assert_eq!(sources.len(), 2);
    assert!(m.record.identity.0.iter().filter(|f| f.key == "fatto-versione-in-uso").all(|f| f.provenance == Provenance::FromSource));
    assert_eq!(m.color(), ColorGroup::Manager(quado_model::record::Manager::Programs));
    // La versione in uso e la precedente stanno sotto la cartella del programma (prefisso più lungo).
    let in_use = engine::child(&tree, apps, "9.4.0.4015").expect("in uso");
    let owner = cls.match_of(in_use).expect("programma");
    assert_eq!(kb.rules[cls.matches[owner].rule].id, "programmi-registrati");
    assert_eq!(cls.matches[owner].attribution, Provenance::Deduced(Criterion::TwoSources), "voce HKCU e ProductInfo.xml concordi");
    // Progetti: tuoi file, mai proposti.
    let projects = engine::find(&tree, r"C:\Users\demo\AppData\Local\CapCut\User Data\Projects").expect("Projects");
    let p = &cls.matches[cls.match_of(projects).expect("progetti")];
    assert!(matches!(p.record.state, State::YouDecide) && p.record.action == Action::None);
    // La card interventi ha CapCut con la stima esatta.
    let card = interventions(&kb, &cls, &tree);
    assert!(card.iter().any(|i| kb.rules[i.rule].id == "capcut-versioni" && i.estimate == Estimate::Exact(bytes)));
}

#[test]
fn unrecognized_folders_are_unknown() {
    let kb = kb();
    let tree = common::tree(&["profilo.albero"]);
    let cls = classify(&kb, &tree, &common::evidence(), TODAY);
    // Una cartella senza regola: nessuna corrispondenza, quindi Non sappiamo con la sola apertura in Esplora.
    let n = engine::find(&tree, r"C:\Users\demo\AppData\Local\Packages").expect("Packages");
    assert!(cls.match_of(n).is_none());
    let r = engine::unknown_record();
    assert_eq!(r.state, State::Unknown(UnknownReason::NoRule));
    assert_eq!(r.action, Action::None);
    // Le node_modules senza lockfile non sono riconosciute (estensioni di VS Code, progetto senza lockfile).
    let dev = common::tree(&["sviluppo.albero"]);
    let cls = classify(&kb, &dev, &common::evidence(), TODAY);
    for p in [r"C:\Users\demo\.vscode\extensions\github.copilot-chat-0.48.1\node_modules", r"C:\Users\demo\Documents\foto-vacanze\node_modules"] {
        let n = engine::find(&dev, p).expect("cartella");
        assert!(cls.match_of(n).is_none_or(|m| kb.rules[cls.matches[m].rule].id != "progetti-sviluppo"), "{p}");
    }
    check_invariants(&kb, &dev, &cls);
}

#[test]
fn every_fixture_keeps_the_invariants() {
    let kb = kb();
    let ev = common::evidence();
    let all = ["capcut.albero", "radice.albero", "sviluppo.albero", "giochi.albero", "browser.albero", "profilo.albero"];
    for f in all {
        let tree = common::tree(&[f]);
        check_invariants(&kb, &tree, &classify(&kb, &tree, &ev, TODAY));
    }
    let tree = common::tree(&all);
    check_invariants(&kb, &tree, &classify(&kb, &tree, &ev, TODAY));
}

#[test]
fn conflicting_rules_are_both_shown_without_proposals() {
    // Una seconda regola che dice "tuoi file" sulla cache di npm: conflitto con "Lo gestisce npm".
    let mut files: Vec<(String, String)> = quado_kb::rules::RULE_FILES.iter().map(|(n, t)| ((*n).to_owned(), (*t).to_owned())).collect();
    let extra = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("regole/capcut-progetti.toml"))
        .expect("regola")
        .replace("id = \"capcut-progetti\"", "id = \"conflitto\"")
        .replace(r#"percorsi = ["CapCut/User Data/Projects", "CapCut/Videos"]"#, r#"percorsi = ["npm-cache"]"#);
    files.push(("conflitto.toml".into(), extra));
    let kb = Kb::from_files(files.iter().map(|(a, b)| (a.as_str(), b.as_str()))).expect("regole");
    let tree = common::tree(&["sviluppo.albero"]);
    let cls = classify(&kb, &tree, &common::evidence(), TODAY);
    let n = engine::find(&tree, r"C:\Users\demo\AppData\Local\npm-cache").expect("cache");
    let m = &cls.matches[cls.match_of(n).expect("corrispondenza")];
    assert_eq!(m.record.state, State::Unknown(UnknownReason::Conflict));
    assert_eq!(m.record.reliability, Criterion::Conflict);
    assert_eq!(m.record.action, Action::None);
    assert_eq!(m.others.len(), 1, "l'altra regola si mostra insieme");
    assert_eq!(m.color(), ColorGroup::Unclassified);
    assert!(!interventions(&kb, &cls, &tree).iter().any(|i| i.matches.iter().any(|&k| cls.matches[k].nodes.contains(&n))));
    check_invariants(&kb, &tree, &cls);
}

#[test]
fn rules_to_recheck_after_6_11_12_months() {
    // 07 §5.1, con date simulate: banner oltre 6 mesi, annuncio oltre 11, stato dichiarato oltre 12.
    let kb = kb();
    let tree = common::tree(&["sviluppo.albero"]);
    let ev = common::evidence();
    let at = |d: Date| classify(&kb, &tree, &ev, d);
    let fresh = at(TODAY);
    assert!(fresh.recheck.is_empty());
    let six = at(Date::new(2027, 4, 14));
    // Le regole verificate il 13/10/2026 (tutte tranne windows-old, aggiunta il 01/11/2026).
    let old = kb.rules.iter().filter(|r| r.verified == Date::new(2026, 10, 13)).count();
    assert!(old + 1 >= kb.rules.len(), "regole verificate dopo il 13/10/2026: {}", kb.rules.len() - old);
    assert_eq!(six.recheck.len(), old, "banner: le regole verificate il 13/10/2026");
    assert!(six.matches.iter().all(|m| m.record.expiry == Expiry::Recheck));
    let eleven = at(Date::new(2027, 9, 14));
    assert!(eleven.matches.iter().all(|m| m.record.expiry == Expiry::Announce));
    assert!(eleven.matches.iter().any(|m| m.record.state.proposes()), "fino a 12 mesi le proposte restano");
    let twelve = at(Date::new(2027, 10, 14));
    for m in &twelve.matches {
        assert_eq!(m.record.expiry, Expiry::Expired);
        assert_eq!(m.record.state, State::Unknown(UnknownReason::RuleExpired), "{}: il cambio di stato ha il suo motivo", m.record.origin_id());
        assert_eq!(m.record.action, Action::None);
        assert_eq!(m.color(), ColorGroup::Unclassified, "colore del gestore come ipotesi");
        assert!(m.record.identity.0.iter().all(|f| !matches!(f.provenance, Provenance::Deduced(c) if c != Criterion::RuleExpired)));
    }
    assert!(interventions(&kb, &twelve, &tree).is_empty());
    check_invariants(&kb, &tree, &twelve);
}

#[test]
fn package_manager_caches_are_not_your_projects() {
    // npx installa pacchetti in `npm-cache\_npx\<hash>` con node_modules e package-lock.json: è cache.
    let kb = kb();
    let d = NodeData { flags: Flags::DIR, ..Default::default() };
    let f = |n: u64| NodeData { alloc: n, logical: n, ..Default::default() };
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let mut at = Tree::ROOT;
    for name in ["Users", "demo", "AppData", "Local", "npm-cache"] {
        at = t.push(at, &w(name), d);
    }
    let cc = t.push(at, &w("_cacache"), d);
    t.push(cc, &w("indice"), f(1 << 20));
    let npx = t.push(at, &w("_npx"), d);
    let h = t.push(npx, &w("a1b2c3"), d);
    t.push(h, &w("package-lock.json"), f(4096));
    let nm = t.push(h, &w("node_modules"), d);
    t.push(nm, &w("x.js"), f(1 << 22));
    t.aggregate();
    t.seal();
    let mut ev = Evidence::empty("C:\\");
    ev.folders = Evidence::default_folders("C:\\", "demo");
    ev.npm_cache = quado_kb::extract::ToolPath { path: r"C:\Users\demo\AppData\Local\npm-cache".into(), origin: quado_kb::extract::ConfigOrigin::Default };
    let cls = classify(&kb, &t, &ev, TODAY);
    let m = cls.match_of(nm).expect("cache");
    assert_eq!(kb.rules[cls.matches[m].rule].id, "npm-cache");
}