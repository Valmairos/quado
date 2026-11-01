// SPDX-License-Identifier: GPL-3.0-or-later
//! Record della lente Download (06 §3.1-3.4, 03 §6.1): voci di "Cosa riconosce Quado in questa cartella" su
//! alberi costruiti in memoria, con le letture su richiesta simulate. Proprietà: nessuna voce Gestibile
//! senza una prova misurata che non sia dimensione, data o nome; nessuna cifra recuperabile prima del
//! confronto; il suffisso " (N)" non cambia niente; ogni chiave di testo esiste in italiano e in inglese.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use proptest::prelude::*;
use quado_kb::extract::{ExtractError, Field, FormatIssue, Uninstall, UninstallEntry};
use quado_kb::installers::{self, InstallerInfo, InstallerKind, MatchOutcome};
use quado_kb::lens::{self, Finding, InstallerMatch, InstallerRead, InstallerStatus, LensFinding, LensInput, UseCheck, UseStatus};
use quado_model::downloads::{self, ArchiveCheck, ArchiveEntry, FileUse, Hashed, LensFamily};
use quado_model::record::{
    Action, CheckKind, CheckOutcome, Criterion, Estimate, Expiry, Fact, Function, Hive, NotEstimable, Origin, Provenance, Record, Reliability, Source,
    State, Value,
};
use quado_model::tree::w;
use quado_model::{Flags, NodeData, Tree};

// ---------------------------------------------------------------------------------------------
// Alberi

struct Folder {
    tree: Tree,
    root: u32,
    nodes: HashMap<String, u32>,
}

impl Folder {
    fn n(&self, path: &str) -> u32 {
        *self.nodes.get(path).unwrap_or_else(|| panic!("{path} non è nell'albero"))
    }
}

/// `C:\Users\demo\<cartella>` con i file dati: `sotto/nome` crea le sottocartelle, un nome che finisce
/// con `/` è una cartella. Allocata: la logica arrotondata a 4 KiB.
fn folder(name: &str, files: &[(&str, u64, Flags)]) -> Folder {
    let d = NodeData { flags: Flags::DIR, ..Default::default() };
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let users = t.push(Tree::ROOT, &w("Users"), d);
    let u = t.push(users, &w("demo"), d);
    let root = t.push(u, &w(name), d);
    let mut nodes = HashMap::new();
    for &(path, size, flags) in files {
        let (parent, leaf) = match path.trim_end_matches('/').rsplit_once('/') {
            Some((p, l)) => (nodes[p], l),
            None => (root, path.trim_end_matches('/')),
        };
        let data = if path.ends_with('/') {
            NodeData { flags: Flags::DIR | flags, ..Default::default() }
        } else {
            NodeData { logical: size, alloc: size.div_ceil(4096) * 4096, flags, ..Default::default() }
        };
        let n = t.push(parent, &w(leaf), data);
        nodes.insert(path.trim_end_matches('/').to_owned(), n);
    }
    t.aggregate();
    t.seal();
    Folder { tree: t, root, nodes }
}

const NO: Flags = Flags(0);
const GB: u64 = 1 << 30;

/// Download con ogni famiglia: copie di nome diverso, `setup (1).exe` di dimensione diversa, installatori,
/// uno zip con la cartella estratta, immagini disco, download incompleti, un segnaposto cloud.
fn downloads_folder() -> Folder {
    folder(
        "Downloads",
        &[
            ("fattura.pdf", 70_000, NO),
            ("fattura scaricata di nuovo.pdf", 70_000, NO),
            ("altra fattura.pdf", 70_000, NO),
            ("setup.exe", 5_000_000, NO),
            ("setup (1).exe", 5_000_123, NO),
            ("pacchetto.msi", 2_000_000, NO),
            ("strumento.exe", 3_000_000, NO),
            ("nel cloud.exe", 9_000_000, Flags::CLOUD),
            ("Progetto.zip", 40_000, NO),
            ("progetto/", 0, NO),
            ("progetto/a.txt", 10, NO),
            ("progetto/src/", 0, NO),
            ("progetto/src/main.rs", 200, NO),
            ("ubuntu.iso", 6 * GB, NO),
            ("vecchio.vhdx", 2 * GB, Flags::CLOUD),
            ("film.mp4.crdownload", 700_000_000, NO),
            ("Non confermato 1.crdownload", 1_000, NO),
            ("serie.mkv.part", 300_000_000, NO),
            ("foto.jpg", 4_000, NO),
        ],
    )
}

fn h(node: u32, size: u64, digest: u8, created: i64) -> Hashed {
    Hashed { node, size, digest: [digest; 32], created }
}

fn run(f: &Folder, input: &LensInput) -> Vec<LensFinding> {
    let lens = downloads::build(&f.tree, f.root);
    let out = lens::findings(&f.tree, &lens, input);
    check_invariants(&out);
    out
}

fn of(out: &[LensFinding], pred: impl Fn(&Finding) -> bool) -> Vec<&LensFinding> {
    out.iter().filter(|v| pred(&v.finding)).collect()
}

fn one(out: &[LensFinding], pred: impl Fn(&Finding) -> bool) -> &LensFinding {
    let v = of(out, pred);
    assert_eq!(v.len(), 1, "{:?}", out.iter().map(|x| x.finding).collect::<Vec<_>>());
    v[0]
}

fn keys(r: &Record) -> Vec<&str> {
    r.identity.0.iter().map(|f| f.key.as_str()).collect()
}

fn proof(r: &Record) -> Vec<&Fact> {
    r.state.manageability().map(|p| p.facts().iter().collect()).unwrap_or_default()
}

/// Voci che la card "Interventi possibili" riceve, con lo stesso filtro di `engine::interventions`.
fn card(out: &[LensFinding]) -> Vec<&LensFinding> {
    out.iter().filter(|v| v.record.state.proposes() && !matches!(v.record.action, Action::None | Action::Uninstall)).collect()
}

// ---------------------------------------------------------------------------------------------
// Invarianti (03 §6.1, 06 §2.2)

fn check_invariants(out: &[LensFinding]) {
    for v in out {
        let r = &v.record;
        let id = r.origin_id();
        assert!(matches!(r.origin, Origin::Function(_)), "{id}: origine");
        assert_eq!(r.expiry, Expiry::Fresh);
        assert!(!r.protected_exception);
        assert!(!r.identity.0.is_empty(), "{id}: prova di identità vuota");
        assert!(r.text_keys().all(|k| !k.trim().is_empty()), "{id}: chiave vuota");
        assert!(!v.nodes.is_empty(), "{id}: voce senza file");
        // Gestibile e Da verificare solo con una prova misurata su richiesta: mai dimensione, data o nome
        // dalla scansione; i fatti della prova non sono fatti di identità.
        if let Some(p) = r.state.manageability() {
            assert!(!p.facts().is_empty());
            for f in p.facts() {
                assert!(matches!(f.source, Source::ContentHash | Source::RestartManager), "{id}: prova da {:?}", f.source);
                assert_eq!(f.provenance, Provenance::Measured, "{id}: prova non misurata");
                assert!(!r.identity.0.contains(f), "{id}: un fatto di identità usato come prova di gestibilità");
            }
        }
        // Azione solo con Gestibile; senza azione niente stima, niente metodo, testi "nessuna azione".
        match r.action {
            Action::Recycle => {
                assert!(matches!(r.state, State::Manageable(_)), "{id}: azione senza prova");
                assert!(r.preferred.is_some() && !r.risks.is_empty());
                assert!(matches!(r.estimate, Estimate::Exact(_) | Estimate::Range { .. }));
                assert_eq!(r.reversibility, "reversibilita-cestino");
                assert!(r.checks.iter().any(|c| c.key == "controllo-restart-manager" && c.kind == CheckKind::Automatic));
            }
            Action::None => {
                assert!(!matches!(r.state, State::Manageable(_) | State::ToVerify(_)), "{id}: Gestibile senza azione");
                assert_eq!(r.estimate, Estimate::NotEstimable(NotEstimable::NoAction), "{id}: cifra senza azione");
                assert!(r.preferred.is_none() && r.alternatives.is_empty());
                assert_eq!(r.reversibility, "reversibilita-nessuna-azione");
            }
            _ => panic!("{id}: azione inattesa {:?}", r.action),
        }
        // Affidabilità bassa: mai un'azione.
        if r.reliability.level() == Reliability::Low {
            assert_eq!(r.action, Action::None, "{id}: azione con affidabilità bassa");
        }
        // Nessun fatto porta il nome di un file: il suffisso non può entrare nei record. I testi sono codici,
        // salvo i dati dichiarati dagli installatori e dalle voci di disinstallazione e i nomi dei processi.
        for f in r.identity.0.iter().chain(proof(r)) {
            let data = f.key.starts_with("fatto-installatore-") || f.key == "fatto-lente-processo";
            for val in &f.values {
                if let Value::Text(s) = val {
                    assert!(data || s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "{id}: {s}");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Copie (F-DL-3, F-DL-4)

#[test]
fn before_any_reading_nothing_is_manageable_and_nothing_has_a_figure() {
    let f = downloads_folder();
    let out = run(&f, &LensInput::default());
    assert!(out.iter().all(|v| !matches!(v.record.state, State::Manageable(_) | State::ToVerify(_))));
    assert!(card(&out).is_empty(), "nessuna voce nella card prima delle letture");
    // Tre file della stessa dimensione con nomi diversi: Decidi tu, contenuto non confrontato.
    let same = one(&out, |x| *x == Finding::SameSize);
    assert_eq!(same.nodes, vec![f.n("fattura.pdf"), f.n("fattura scaricata di nuovo.pdf"), f.n("altra fattura.pdf")]);
    assert_eq!(same.record.state, State::YouDecide);
    assert_eq!(keys(&same.record), ["fatto-lente-stessa-dimensione", "fatto-lente-contenuto-non-confrontato"]);
    assert_eq!(same.record.identity.0[1].provenance, Provenance::Unavailable);
    assert_eq!(same.record.origin, Origin::Function(Function::SameSize));
    assert_eq!(same.bytes, 3 * 73_728);
    // `setup (1).exe` ha un'altra dimensione: nessun gruppo con `setup.exe`.
    let grouped = |v: &&LensFinding| matches!(v.finding, Finding::SameSize | Finding::IdenticalCopies { .. } | Finding::DifferentContent);
    assert!(out.iter().filter(grouped).all(|v| !v.nodes.contains(&f.n("setup.exe")) && !v.nodes.contains(&f.n("setup (1).exe"))));
    // Installatori non letti in una voce, quello nel cloud in un'altra.
    let not_read = one(&out, |x| *x == Finding::Installer(InstallerStatus::NotRead));
    assert_eq!(not_read.nodes.len(), 4);
    assert_eq!(keys(&not_read.record), ["fatto-lente-famiglia", "fatto-lente-installatori-non-letti"]);
    let cloud = one(&out, |x| *x == Finding::Installer(InstallerStatus::CloudOnly));
    assert_eq!(cloud.nodes, vec![f.n("nel cloud.exe")]);
    assert_eq!(keys(&cloud.record), ["fatto-lente-famiglia", "fatto-lente-solo-cloud"]);
    // Zip con la cartella accanto: indice non letto.
    let zip = one(&out, |x| matches!(x, Finding::ArchiveWithFolder { .. }));
    assert_eq!((zip.finding, zip.nodes.clone()), (Finding::ArchiveWithFolder { folder: f.n("progetto") }, vec![f.n("Progetto.zip")]));
    assert_eq!(keys(&zip.record), ["fatto-lente-cartella-accanto", "fatto-lente-indice-non-letto"]);
    // Immagini disco in una voce, con il segnaposto.
    let iso = one(&out, |x| *x == Finding::DiskImages);
    assert_eq!(keys(&iso.record), ["fatto-lente-famiglia", "fatto-lente-segnaposto"]);
    assert_eq!(iso.record.identity.0[0].values, vec![Value::Text("immagini-disco".into()), Value::Count(2)]);
    // Download incompleti non controllati: Decidi tu senza proposta.
    let inc = one(&out, |x| *x == Finding::Incomplete(UseStatus::NotChecked));
    assert_eq!(inc.nodes.len(), 3);
    assert_eq!(inc.record.state, State::YouDecide);
    assert!(inc.record.checks.is_empty());
}

#[test]
fn identical_copies_are_manageable_only_after_the_comparison() {
    let f = downloads_folder();
    let (a, b, c) = (f.n("fattura.pdf"), f.n("fattura scaricata di nuovo.pdf"), f.n("altra fattura.pdf"));
    // a e c identici, b diverso; c è il più vecchio.
    let input = LensInput { hashed: vec![h(a, 70_000, 1, 300), h(b, 70_000, 2, 100), h(c, 70_000, 1, 200)], ..Default::default() };
    let out = run(&f, &input);
    assert!(of(&out, |x| *x == Finding::SameSize).is_empty());
    let copies = one(&out, |x| matches!(x, Finding::IdenticalCopies { .. }));
    assert_eq!(copies.finding, Finding::IdenticalCopies { keep: c });
    assert_eq!(copies.nodes, vec![c, a], "per data di creazione");
    let r = &copies.record;
    assert!(matches!(r.state, State::Manageable(_)));
    assert_eq!(r.action, Action::Recycle);
    assert_eq!(r.origin, Origin::Function(Function::IdenticalCopies));
    let p: Vec<&str> = proof(r).iter().map(|f| f.key.as_str()).collect();
    assert_eq!(p, ["fatto-lente-contenuto-identico", "fatto-lente-copia-tenuta"]);
    assert_eq!(proof(r)[0].values, vec![Value::Count(2), Value::Bytes(70_000)]);
    assert_eq!(proof(r)[0].source, Source::ContentHash);
    assert_eq!(proof(r)[1].values, vec![Value::Time(200)]);
    // Stima esatta: l'allocata della copia da spostare.
    assert_eq!(r.estimate, Estimate::Exact(73_728));
    assert_eq!(r.preferred.as_ref().map(|s| s.key.as_str()), Some("funzione-copie-identiche-metodo"));
    assert_eq!(r.reversibility, "reversibilita-cestino");
    assert_eq!(r.on_error, "errore-per-elemento");
    assert_eq!(r.after, "verifica-riscansione");
    assert!(r.checks.iter().all(|c| c.outcome == CheckOutcome::AtSimulation));
    // b: stessa dimensione, contenuto diverso. Decidi tu, nessuna cifra.
    let diff = one(&out, |x| *x == Finding::DifferentContent);
    assert_eq!(diff.nodes, vec![b]);
    assert_eq!(keys(&diff.record), ["fatto-lente-stessa-dimensione", "fatto-lente-contenuto-diverso"]);
    assert_eq!(diff.record.state, State::YouDecide);
    // La card riceve solo le copie, con la stima.
    let c_ = card(&out);
    assert_eq!(c_.len(), 1);
    assert_eq!(c_[0].record.estimate, Estimate::Exact(73_728));
    // L'utente sceglie di tenere a: c si sposta.
    let mine = run(&f, &LensInput { keep: vec![a], ..input.clone() });
    let copies = one(&mine, |x| matches!(x, Finding::IdenticalCopies { .. }));
    assert_eq!(copies.finding, Finding::IdenticalCopies { keep: a });
    assert_eq!(proof(&copies.record)[1].values, vec![Value::Time(300)]);
}

#[test]
fn copies_with_other_names_get_a_range() {
    // Un file con più nomi torna libero solo se tutti i nomi lasciano il volume (03 §6.3).
    let f = folder(
        "Downloads",
        &[("a.bin", 10_000, NO), ("b.bin", 10_000, Flags::MULTI_LINK), ("c.bin", 10_000, NO), ("d.bin", 10_000, Flags::MULTI_LINK | Flags::SECONDARY_LINK)],
    );
    let nodes: Vec<u32> = ["a.bin", "b.bin", "c.bin", "d.bin"].iter().map(|n| f.n(n)).collect();
    let hashed: Vec<Hashed> = nodes.iter().enumerate().map(|(k, &n)| h(n, 10_000, 9, k as i64)).collect();
    let out = run(&f, &LensInput { hashed, ..Default::default() });
    let copies = one(&out, |x| matches!(x, Finding::IdenticalCopies { .. }));
    assert_eq!(copies.finding, Finding::IdenticalCopies { keep: nodes[0] });
    // d.bin è il nome secondario di b.bin: non è una copia e non entra nel gruppo. Si spostano b e c: c
    // libera i suoi byte, b solo se anche il suo altro nome lascia il volume.
    assert!(!copies.nodes.contains(&nodes[3]));
    assert_eq!(copies.record.estimate, Estimate::Range { min: 12_288, max: 2 * 12_288 });
    // Ogni file contato una volta: il nome secondario non entra nella cifra.
    assert_eq!(copies.bytes, 3 * 12_288);
}

#[test]
fn a_cancelled_or_stale_comparison_stays_not_compared() {
    let f = downloads_folder();
    let (a, b, c) = (f.n("fattura.pdf"), f.n("fattura scaricata di nuovo.pdf"), f.n("altra fattura.pdf"));
    // Annullato dopo due file su tre: nessuna copia presentata.
    let out = run(&f, &LensInput { hashed: vec![h(a, 70_000, 1, 1), h(b, 70_000, 1, 2)], ..Default::default() });
    assert!(of(&out, |x| matches!(x, Finding::IdenticalCopies { .. })).is_empty());
    let same = one(&out, |x| *x == Finding::SameSize);
    assert_eq!(keys(&same.record), ["fatto-lente-stessa-dimensione", "fatto-lente-letti-in-parte"]);
    assert_eq!(same.record.identity.0[1].values, vec![Value::Count(2), Value::Count(3)]);
    // Un file cambiato dopo la scansione: il confronto non vale.
    let out = run(&f, &LensInput { hashed: vec![h(a, 70_000, 1, 1), h(b, 70_000, 1, 2), h(c, 71_000, 1, 3)], ..Default::default() });
    assert!(of(&out, |x| matches!(x, Finding::IdenticalCopies { .. })).is_empty());
    let same = one(&out, |x| *x == Finding::SameSize);
    assert_eq!(keys(&same.record), ["fatto-lente-stessa-dimensione", "fatto-lente-dimensione-cambiata"]);
    assert!(card(&out).is_empty());
}

#[test]
fn the_copy_suffix_does_not_change_states() {
    // `setup (1).exe` di dimensione diversa da `setup.exe`: il filtro lo trova, lo stato non cambia.
    let with = folder("Downloads", &[("setup.exe", 5_000, NO), ("setup (1).exe", 5_001, NO), ("x.bin", 77, NO), ("x (2).bin", 77, NO)]);
    let without = folder("Downloads", &[("setup.exe", 5_000, NO), ("setup1.exe", 5_001, NO), ("x.bin", 77, NO), ("x2.bin", 77, NO)]);
    let lens = downloads::build(&with.tree, with.root);
    let filtered: Vec<u32> = lens.items.iter().filter(|i| i.copy_suffix.is_some()).map(|i| i.node).collect();
    assert_eq!(filtered, vec![with.n("setup (1).exe"), with.n("x (2).bin")]);
    let hashed = vec![h(with.n("x.bin"), 77, 3, 1), h(with.n("x (2).bin"), 77, 3, 2)];
    for input in [LensInput::default(), LensInput { hashed, ..Default::default() }] {
        assert_eq!(run(&with, &input), run(&without, &input));
    }
}

// ---------------------------------------------------------------------------------------------
// Installatori (F-DL-2)

fn info(kind: InstallerKind, name: &str, version: &str) -> InstallerInfo {
    InstallerInfo { kind: Some(kind), product_name: Some(name.into()), product_version: Some(version.into()), company: Some("Editore".into()), ..Default::default() }
}

const CODE: &str = "{0A1B2C3D-0000-1111-2222-333344445555}";

fn entry(hive: Hive, key: &str, name: &str, version: &str) -> UninstallEntry {
    UninstallEntry {
        hive,
        key: key.into(),
        display_name: name.into(),
        version: Some(version.into()),
        publisher: None,
        install_location: None,
        uninstall_string: None,
        display_icon: None,
        install_date: None,
        estimated_kb: None,
        system_component: false,
    }
}

/// Voci di disinstallazione di prova: un MSI registrato con il suo ProductCode e un programma per nome.
fn programs() -> Uninstall {
    Uninstall { entries: vec![entry(Hive::Machine, CODE, "Pacchetto", "2.0"), entry(Hive::User, "Strumento", "Strumento", "1.5")], skipped: Vec::new() }
}

/// Lettura riuscita con la corrispondenza calcolata come la calcola l'interfaccia.
fn read_with_match(i: InstallerInfo) -> InstallerRead {
    let matched = InstallerMatch::compute(&i, &programs());
    InstallerRead { info: Ok(i), matched: Some(matched) }
}

#[test]
fn installers_with_the_three_match_outcomes() {
    let f = downloads_folder();
    let mut input = LensInput::default();
    // MSI con lo stesso ProductCode: da fonte.
    input.installers.insert(f.n("pacchetto.msi"), read_with_match(InstallerInfo { product_code: Some(CODE.into()), ..info(InstallerKind::Msi, "Pacchetto", "2.0") }));
    // Exe con nome e versione uguali: dedotto, media.
    input.installers.insert(f.n("setup.exe"), read_with_match(info(InstallerKind::Exe, "Strumento", "1.5")));
    // Versione diversa: dedotto, bassa (coincide solo il nome).
    input.installers.insert(f.n("setup (1).exe"), read_with_match(info(InstallerKind::Exe, "Strumento", "1.4")));
    // Non installato.
    input.installers.insert(f.n("strumento.exe"), read_with_match(info(InstallerKind::Exe, "Altro", "3.0")));
    let out = run(&f, &input);
    let by = |s: InstallerStatus| of(&out, move |x| *x == Finding::Installer(s));
    let same = by(InstallerStatus::SameVersion);
    assert_eq!(same.len(), 2);
    // Prima la corrispondenza (il riassunto della voce), poi i dati letti, in fondo il tipo.
    let msi = same.iter().find(|v| v.nodes == [f.n("pacchetto.msi")]).expect("msi");
    assert_eq!(
        keys(&msi.record),
        [installers::FACT_SAME_CODE, installers::FACT_PRODUCT, installers::FACT_COMPANY, installers::FACT_PRODUCT_CODE, "fatto-lente-famiglia"]
    );
    let code = &msi.record.identity.0[0];
    assert_eq!((code.provenance, &code.source), (Provenance::FromSource, &Source::Uninstall(Hive::Machine)));
    assert_eq!(msi.record.identity.0[1].source, Source::MsiProperties);
    let exe = same.iter().find(|v| v.nodes == [f.n("setup.exe")]).expect("exe");
    assert_eq!(exe.record.identity.0[0].key, installers::FACT_SAME_NAME);
    assert_eq!(exe.record.identity.0[0].provenance, Provenance::Deduced(Criterion::OneSource));
    assert_eq!(exe.record.identity.0[1].source, Source::VersionResource);
    assert_eq!(exe.record.reliability, Criterion::OneSource, "dedotto, affidabilità media");
    // Versione diversa con lo stesso nome: dedotto, affidabilità bassa (`installers::match_fact`).
    let other = &by(InstallerStatus::OtherVersion)[0];
    let fact = &other.record.identity.0[0];
    assert_eq!((fact.key.as_str(), fact.provenance), (installers::FACT_NAME_OTHER_VERSION, Provenance::Deduced(Criterion::NameOnly)));
    assert_eq!(fact.values, vec![Value::Text("Strumento".into()), Value::Text("1.5".into()), Value::Text("1.4".into())]);
    assert_eq!(other.record.reliability, Criterion::NameOnly);
    let none = &by(InstallerStatus::NotInstalled)[0];
    assert_eq!(none.record.identity.0[0].key, installers::FACT_NOT_INSTALLED);
    // Tutti Decidi tu, senza azione e fuori dalla card.
    for v in of(&out, |x| matches!(x, Finding::Installer(_))) {
        assert_eq!((v.record.state.clone(), v.record.action.clone()), (State::YouDecide, Action::None));
        assert_eq!(v.record.origin, Origin::Function(Function::Installer));
    }
    assert!(card(&out).is_empty());
    // Il segnaposto resta "solo nel cloud" anche con un esito in ingresso.
    assert_eq!(by(InstallerStatus::CloudOnly)[0].nodes, vec![f.n("nel cloud.exe")]);
    assert!(by(InstallerStatus::NotRead).is_empty());
    // Nessuna chiave doppia: i fatti degli installatori sono quelli di `installers`.
    for v in &out {
        assert!(v.record.identity.0.iter().all(|x| !x.key.starts_with("fatto-lente-installatore-")), "{:?}", keys(&v.record));
    }
}

/// Senza nome né codice di prodotto la corrispondenza non si può fare: la voce resta senza esito.
#[test]
fn installers_without_data_are_not_matched() {
    let f = downloads_folder();
    let mut input = LensInput::default();
    input.installers.insert(f.n("setup.exe"), read_with_match(InstallerInfo { kind: Some(InstallerKind::Exe), ..Default::default() }));
    let out = run(&f, &input);
    let v = one(&out, |x| *x == Finding::Installer(InstallerStatus::NotMatched));
    assert_eq!(keys(&v.record), [installers::FACT_NOT_COMPARABLE, installers::FACT_NO_DATA, "fatto-lente-famiglia"]);
    assert_eq!(v.record.identity.0[0].provenance, Provenance::Unavailable);
}

#[test]
fn installer_readings_that_fail_say_why() {
    let f = downloads_folder();
    let cases = [
        (ExtractError::Format(FormatIssue::Truncated), vec![Value::Text("troncato".into())]),
        (ExtractError::Format(FormatIssue::MissingField(Field::Version)), vec![Value::Text("campo-mancante".into()), Value::Text("full_appver".into())]),
        (ExtractError::Io(5), vec![Value::Text("accesso-negato".into())]),
        (ExtractError::Io(1392), vec![Value::Text("errore".into()), Value::Text("1392".into())]),
        (ExtractError::NotPresent, vec![Value::Text("non-presente".into())]),
    ];
    for (e, values) in cases {
        let mut input = LensInput::default();
        input.installers.insert(f.n("pacchetto.msi"), InstallerRead { info: Err(e), matched: None });
        let out = run(&f, &input);
        let v = one(&out, |x| *x == Finding::Installer(InstallerStatus::Failed));
        let fact = &v.record.identity.0[0];
        assert_eq!((fact.key.as_str(), fact.provenance, &fact.source), ("fatto-lente-estrattore-non-riuscito", Provenance::Unavailable, &Source::MsiProperties));
        assert_eq!(fact.values, values);
    }
    // Letto, ma senza nome né versione, e senza corrispondenza calcolata: il tipo viene dall'estensione.
    let mut input = LensInput::default();
    input.installers.insert(f.n("pacchetto.msi"), InstallerRead { info: Ok(InstallerInfo::default()), matched: None });
    let out = run(&f, &input);
    let v = one(&out, |x| *x == Finding::Installer(InstallerStatus::NotMatched));
    assert_eq!(keys(&v.record), [installers::FACT_NO_DATA, "fatto-lente-famiglia"]);
    let no_data = &v.record.identity.0[0];
    assert_eq!((&no_data.source, &no_data.values), (&Source::MsiProperties, &vec![Value::Text("msi".into())]));
    // Un esito "solo nel cloud" dal lettore porta il file nella voce del cloud.
    let mut input = LensInput::default();
    input.installers.insert(f.n("setup.exe"), InstallerRead { info: Err(ExtractError::CloudPlaceholder), matched: None });
    let out = run(&f, &input);
    assert_eq!(one(&out, |x| *x == Finding::Installer(InstallerStatus::CloudOnly)).nodes.len(), 2);
}

// ---------------------------------------------------------------------------------------------
// Archivi (F-DL-5) e immagini disco

#[test]
fn archive_with_folder_carries_the_measured_proof() {
    let f = downloads_folder();
    let zip = f.n("Progetto.zip");
    let entry = |path: &str, size| ArchiveEntry { path: path.into(), size, dir: false };
    let entries = [entry("a.txt", 10), entry("src/main.rs", 201), entry("src/lib.rs", 5)];
    let checked = downloads::compare_archive(&f.tree, f.n("progetto"), &entries);
    assert_eq!(checked, ArchiveCheck { total: 3, present: 1, different_size: 1, missing: 1, unreadable: 0 });
    let mut input = LensInput::default();
    input.archives.insert(zip, Ok(checked));
    let out = run(&f, &input);
    let v = one(&out, |x| matches!(x, Finding::ArchiveWithFolder { .. }));
    assert_eq!(
        keys(&v.record),
        ["fatto-lente-cartella-accanto", "fatto-lente-voci-presenti", "fatto-lente-voci-dimensione-diversa", "fatto-lente-voci-mancanti"]
    );
    let n_of_m = &v.record.identity.0[1];
    assert_eq!((n_of_m.provenance, &n_of_m.source, &n_of_m.values), (Provenance::Measured, &Source::ZipDirectory, &vec![Value::Count(1), Value::Count(3)]));
    assert_eq!((v.record.state.clone(), v.record.action.clone()), (State::YouDecide, Action::None));
    assert_eq!(v.bytes, 40_960);
    // Zip corrotto: "non leggibile", la lente continua.
    input.archives.insert(zip, Err(ExtractError::Format(FormatIssue::Unexpected)));
    let out = run(&f, &input);
    let v = one(&out, |x| matches!(x, Finding::ArchiveWithFolder { .. }));
    assert_eq!(keys(&v.record), ["fatto-lente-cartella-accanto", "fatto-lente-archivio-non-leggibile"]);
    assert_eq!(v.record.identity.0[1].values, vec![Value::Text("inatteso".into())]);
    assert!(out.len() > 5, "le altre voci restano");
    // Voci in cartelle non leggibili.
    input.archives.insert(zip, Ok(ArchiveCheck { total: 4, present: 2, different_size: 0, missing: 0, unreadable: 2 }));
    let out = run(&f, &input);
    let v = one(&out, |x| matches!(x, Finding::ArchiveWithFolder { .. }));
    assert_eq!(v.record.identity.0.last().map(|f| (f.key.as_str(), f.provenance)), Some(("fatto-lente-voci-non-lette", Provenance::Unavailable)));
}

// ---------------------------------------------------------------------------------------------
// Download incompleti (F-DL-6, 06 §3.4)

#[test]
fn incomplete_downloads_follow_restart_manager() {
    let f = downloads_folder();
    let (film, unconfirmed, serie) = (f.n("film.mp4.crdownload"), f.n("Non confermato 1.crdownload"), f.n("serie.mkv.part"));
    let mut input = LensInput::default();
    input.usage.insert(film, UseCheck { usage: FileUse::Free, at: 2_000 });
    input.usage.insert(unconfirmed, UseCheck { usage: FileUse::Free, at: 1_000 });
    input.usage.insert(serie, UseCheck { usage: FileUse::Used(vec!["firefox.exe".into(), "firefox.exe".into(), "vlc.exe".into()]), at: 3_000 });
    let out = run(&f, &input);
    let free = one(&out, |x| *x == Finding::Incomplete(UseStatus::Free));
    assert_eq!(free.nodes, vec![film, unconfirmed]);
    let r = &free.record;
    assert!(matches!(r.state, State::Manageable(_)));
    assert_eq!(r.action, Action::Recycle);
    let p = proof(r);
    assert_eq!((p[0].key.as_str(), &p[0].source), ("fatto-lente-nessun-processo", &Source::RestartManager));
    assert_eq!(p[0].values, vec![Value::Count(2), Value::Time(1_000)], "vale il controllo più vecchio");
    assert_eq!(r.checks[0].outcome, CheckOutcome::Passed);
    assert_eq!(r.estimate, Estimate::Exact(free.bytes));
    assert_eq!(r.risks, ["funzione-download-incompleto-rischio-1"]);
    // In uso: bloccato, con i nomi dei processi.
    let used = one(&out, |x| *x == Finding::Incomplete(UseStatus::InUse));
    assert_eq!(used.record.action, Action::None);
    assert_eq!(used.record.checks.len(), 1);
    assert_eq!((used.record.checks[0].key.as_str(), used.record.checks[0].outcome), ("controllo-restart-manager", CheckOutcome::Failed));
    let procs: Vec<&Value> = used.record.identity.0.iter().filter(|f| f.key == "fatto-lente-processo").flat_map(|f| &f.values).collect();
    assert_eq!(procs, [&Value::Text("firefox.exe".into()), &Value::Text("vlc.exe".into())]);
    // La card riceve i download non in uso.
    assert_eq!(card(&out).iter().map(|v| v.finding).collect::<Vec<_>>(), [Finding::Incomplete(UseStatus::Free)]);
    // Restart Manager non ha risposto: Decidi tu senza proposta, mai "non in uso".
    input.usage.insert(film, UseCheck { usage: FileUse::Unknown(5), at: 4_000 });
    let out = run(&f, &input);
    let unknown = one(&out, |x| *x == Finding::Incomplete(UseStatus::Unknown));
    assert_eq!(unknown.record.state, State::YouDecide);
    assert_eq!(unknown.record.identity.0[1].values, vec![Value::Text("5".into()), Value::Time(4_000)]);
    assert_eq!(one(&out, |x| *x == Finding::Incomplete(UseStatus::Free)).nodes, vec![unconfirmed]);
}

#[test]
fn findings_are_ordered_by_bytes_then_deterministically() {
    let f = downloads_folder();
    let out = run(&f, &LensInput::default());
    assert!(out.windows(2).all(|p| p[0].bytes >= p[1].bytes));
    assert_eq!(out, run(&f, &LensInput::default()));
    assert_eq!(out[0].finding, Finding::DiskImages, "6 GB di immagini disco");
    // Una lente di un altro albero non fa cadere niente.
    let other = folder("Downloads", &[("a.iso", 10, NO)]);
    let lens = downloads::build(&f.tree, f.root);
    let _ = lens::findings(&other.tree, &lens, &LensInput::default());
}

// ---------------------------------------------------------------------------------------------
// Testi (05 D13, 04 §6.8)

/// Una chiave con i segnaposto e se ha sopra un commento `# ...`.
#[derive(Debug, Default)]
struct Msg {
    vars: BTreeSet<String>,
    commented: bool,
}

fn vars(line: &str, out: &mut BTreeSet<String>) {
    for part in line.split('$').skip(1) {
        let id: String = part.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
        if !id.is_empty() {
            out.insert(id);
        }
    }
}

/// Chiavi di un testo Fluent, con i segnaposto; `from` limita alla sezione che comincia con quella riga.
fn messages(ftl: &str, from: Option<&str>) -> BTreeMap<String, Msg> {
    let mut out: BTreeMap<String, Msg> = BTreeMap::new();
    let mut inside = from.is_none();
    let mut comment = false;
    let mut last: Option<String> = None;
    for line in ftl.lines() {
        if let Some(h) = from {
            if line.starts_with(h) {
                inside = true;
                continue;
            }
            if inside && line.starts_with("## ") {
                inside = false;
            }
        }
        if !inside {
            continue;
        }
        if line.starts_with('#') {
            comment = line.starts_with("# ");
            continue;
        }
        if line.trim().is_empty() {
            comment = false;
            continue;
        }
        if !line.starts_with([' ', '\t', '[', '*', '}', '.']) && line.contains(" =") {
            let key = line.split_once(" =").map(|(k, _)| k.trim().to_owned()).unwrap_or_default();
            let mut m = Msg { commented: comment, ..Default::default() };
            vars(line, &mut m.vars);
            out.insert(key.clone(), m);
            last = Some(key);
            comment = false;
        } else if let Some(m) = last.as_ref().and_then(|k| out.get_mut(k)) {
            vars(line, &mut m.vars);
        }
    }
    out
}

const IT_HEADER: &str = "## M4: vista Download";
const EN_HEADER: &str = "## M4: Downloads view";

/// Chiavi prodotte da una voce: testi del record e fatti, con i valori di ogni fatto.
fn produced(out: &[LensFinding], into: &mut BTreeMap<String, Vec<usize>>) {
    for v in out {
        let r = &v.record;
        for k in r.text_keys() {
            into.entry(k.to_owned()).or_default();
        }
        for f in r.identity.0.iter().chain(proof(r)) {
            into.entry(f.key.clone()).or_default().push(f.values.len());
        }
    }
}

/// Ogni chiave esiste in italiano e in inglese. Con `slots`, su un insieme di voci che copre tutte le
/// varianti: un fatto usa solo `$a`, `$b`, `$c` (e `_testo`) dei valori che porta almeno una volta (con
/// le selezioni un segnaposto può mancare nelle varianti non scelte).
fn check_keys(found: &BTreeMap<String, Vec<usize>>, slots: bool) {
    let it = messages(quado_kb::TEXTS[0].1, None);
    let en = messages(quado_kb::TEXTS[1].1, None);
    let mut problems = Vec::new();
    for (k, counts) in found {
        let (Some(i), Some(e)) = (it.get(k), en.get(k)) else {
            problems.push(format!("{k}: manca in regole/it.ftl o in regole/en.ftl"));
            continue;
        };
        if i.vars != e.vars {
            problems.push(format!("{k}: segnaposto diversi"));
        }
        let Some(&most) = counts.iter().max().filter(|_| slots) else { continue };
        for var in &i.vars {
            let slot = ["a", "b", "c"].iter().position(|s| var == s || *var == format!("{s}_testo") || *var == format!("{s}_ora"));
            if slot.is_none_or(|s| s >= most) {
                problems.push(format!("{k}: usa ${var} con al massimo {most} valori"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Una lettura di ogni tipo, per coprire tutte le chiavi.
fn every_reading(f: &Folder) -> Vec<LensInput> {
    let (a, b, c) = (f.n("fattura.pdf"), f.n("fattura scaricata di nuovo.pdf"), f.n("altra fattura.pdf"));
    let mut inputs = vec![
        LensInput::default(),
        LensInput { hashed: vec![h(a, 70_000, 1, 1), h(b, 70_000, 2, 2), h(c, 70_000, 1, 3)], ..Default::default() },
        LensInput { hashed: vec![h(a, 70_000, 1, 1)], ..Default::default() },
        LensInput { hashed: vec![h(a, 70_000, 1, 1), h(b, 70_000, 1, 2), h(c, 1, 1, 3)], ..Default::default() },
    ];
    let errors = [
        ExtractError::NotPresent,
        ExtractError::Io(5),
        ExtractError::Io(32),
        ExtractError::Format(FormatIssue::Truncated),
        ExtractError::Format(FormatIssue::Unexpected),
        ExtractError::Format(FormatIssue::Empty),
        ExtractError::Format(FormatIssue::TooDeep),
        ExtractError::Format(FormatIssue::Encoding),
        ExtractError::Format(FormatIssue::TooLarge),
        ExtractError::Format(FormatIssue::MissingField(Field::Data)),
        ExtractError::Format(FormatIssue::WrongType(Field::Root)),
        ExtractError::Format(FormatIssue::BadValue(Field::Version)),
    ];
    for e in errors {
        let mut i = LensInput::default();
        i.installers.insert(f.n("setup.exe"), InstallerRead { info: Err(e), matched: None });
        i.archives.insert(f.n("Progetto.zip"), Err(e));
        inputs.push(i);
    }
    let mut i = LensInput::default();
    for (k, outcome) in [
        MatchOutcome::SameVersion { entry: 0, provenance: Provenance::FromSource },
        MatchOutcome::SameVersion { entry: 0, provenance: Provenance::Deduced(Criterion::NameOnly) },
        MatchOutcome::OtherVersion { entry: 0, provenance: Provenance::Deduced(Criterion::OneSource) },
        MatchOutcome::NotInstalled,
    ]
    .into_iter()
    .enumerate()
    {
        let node = [f.n("setup.exe"), f.n("setup (1).exe"), f.n("pacchetto.msi"), f.n("strumento.exe")][k];
        let read = info(InstallerKind::Exe, "P", "1");
        let fact = installers::match_fact(&outcome, &read, &programs());
        i.installers.insert(node, InstallerRead { info: Ok(read), matched: Some(InstallerMatch { outcome, fact }) });
    }
    i.archives.insert(f.n("Progetto.zip"), Ok(ArchiveCheck { total: 9, present: 1, different_size: 2, missing: 3, unreadable: 3 }));
    i.usage.insert(f.n("film.mp4.crdownload"), UseCheck { usage: FileUse::Free, at: 1 });
    i.usage.insert(f.n("serie.mkv.part"), UseCheck { usage: FileUse::Used(Vec::new()), at: 1 });
    i.usage.insert(f.n("Non confermato 1.crdownload"), UseCheck { usage: FileUse::Unknown(1223), at: 1 });
    inputs.push(i);
    let mut i = LensInput::default();
    i.installers.insert(f.n("setup.exe"), InstallerRead { info: Ok(InstallerInfo { product_version: Some("1".into()), ..Default::default() }), matched: None });
    i.installers.insert(f.n("pacchetto.msi"), InstallerRead { info: Ok(InstallerInfo::default()), matched: None });
    i.usage.insert(f.n("film.mp4.crdownload"), UseCheck { usage: FileUse::Used(vec!["chrome.exe".into()]), at: 1 });
    inputs.push(i);
    inputs
}

#[test]
fn every_text_key_exists_in_both_languages() {
    let f = downloads_folder();
    let mut found = BTreeMap::new();
    for input in every_reading(&f) {
        produced(&run(&f, &input), &mut found);
    }
    for func in lens::FUNCTIONS {
        found.entry(lens::name_key(func)).or_default();
    }
    // Ogni chiave della sezione della lente è prodotta dal codice (nessun testo orfano).
    let section = messages(quado_kb::TEXTS[0].1, Some(IT_HEADER));
    let orphans: Vec<&String> = section.keys().filter(|k| !found.contains_key(*k)).collect();
    assert!(orphans.is_empty(), "chiavi mai prodotte: {orphans:?}");
    check_keys(&found, true);
}

/// Ogni funzione della lente ha il suo nome in italiano e in inglese, con il commento di contesto, e ogni
/// voce prodotta viene da una di queste funzioni.
#[test]
fn every_lens_function_has_a_name() {
    let it = messages(quado_kb::TEXTS[0].1, None);
    let en = messages(quado_kb::TEXTS[1].1, None);
    for func in lens::FUNCTIONS {
        let k = lens::name_key(func);
        assert!(it.get(&k).is_some_and(|m| m.commented && m.vars.is_empty()), "{k}: manca in regole/it.ftl, senza commento o con segnaposto");
        assert!(en.get(&k).is_some_and(|m| m.vars.is_empty()), "{k}: manca in regole/en.ftl");
    }
    let f = downloads_folder();
    for input in every_reading(&f) {
        for v in run(&f, &input) {
            let Origin::Function(func) = v.record.origin else { panic!("origine") };
            assert!(lens::FUNCTIONS.contains(&func), "{func:?}");
        }
    }
}

#[test]
fn lens_texts_are_aligned_commented_and_plain() {
    let it = messages(quado_kb::TEXTS[0].1, Some(IT_HEADER));
    let en = messages(quado_kb::TEXTS[1].1, Some(EN_HEADER));
    assert!(it.len() > 30, "chiavi della lente: {}", it.len());
    assert_eq!(it.keys().collect::<Vec<_>>(), en.keys().collect::<Vec<_>>(), "stesse chiavi in it ed en");
    for (k, m) in &it {
        assert!(m.commented, "{k}: manca il commento di contesto in it.ftl");
        assert_eq!(m.vars, en[k].vars, "{k}: segnaposto diversi");
        assert!(k.starts_with("fatto-lente-") || (k.starts_with("funzione-") && !k.starts_with("funzione-installatore-")), "{k}: fuori dalla sezione");
    }
    // Parole vietate (04 §6.8), punti esclamativi e trattini lunghi, nei soli valori della sezione.
    let forbidden = [
        "sicuro da eliminare",
        "sicuro da disinstallare",
        "puoi ripristinare tutto",
        "inutilizzato",
        "mai usato",
        "non più in uso",
        "spazio sprecato",
        "spazzatura",
        "ottimizza",
        "pulizia intelligente",
        "safe to delete",
        "safe to uninstall",
        "restore everything",
        "unused",
        "never used",
        "wasted space",
        "junk",
        "optimize",
    ];
    for (text, header) in [(quado_kb::TEXTS[0].1, IT_HEADER), (quado_kb::TEXTS[1].1, EN_HEADER)] {
        let start = text.find(header).expect("sezione della lente");
        let values: String = text[start..].lines().filter(|l| !l.starts_with('#')).collect::<Vec<_>>().join("\n").to_lowercase();
        for word in forbidden {
            assert!(!values.contains(word), "{header}: «{word}»");
        }
        assert!(!values.contains('!') && !values.contains('\u{2014}') && !values.contains('\u{2013}'), "{header}");
    }
}

// ---------------------------------------------------------------------------------------------
// Proprietà su cartelle e letture casuali (06 §2.2, §3.1, §3.2)

const STEMS: [&str; 5] = ["setup", "foto", "archivio", "Relazione è", "dati"];
const EXTS: [&str; 12] = ["exe", "msi", "zip", "iso", "vhdx", "mp4", "pdf", "crdownload", "part", "xyz", "", "tar.gz"];

#[derive(Clone, Debug)]
struct Spec {
    sub: bool,
    stem: usize,
    ext: usize,
    size: u64,
    cloud: bool,
    links: bool,
    suffix: Option<u32>,
    /// Per gli zip: una cartella con lo stesso nome accanto.
    folder: bool,
}

fn spec() -> impl Strategy<Value = Spec> {
    (any::<bool>(), 0..STEMS.len(), 0..EXTS.len(), prop_oneof![Just(0u64), Just(4_096), Just(70_000), 1u64..50], 0u8..8, proptest::option::of(1u32..30), any::<bool>())
        .prop_map(|(sub, stem, ext, size, flags, suffix, folder)| Spec { sub, stem, ext, size, cloud: flags == 0, links: flags == 1, suffix, folder })
}

/// Letture casuali, applicate per posizione ai file della lente.
#[derive(Clone, Debug)]
struct Reads {
    hash: Vec<(bool, u8, i64, bool)>,
    keep: Vec<bool>,
    installer: Vec<u8>,
    archive: Vec<u8>,
    usage: Vec<u8>,
}

fn reads() -> impl Strategy<Value = Reads> {
    let n = 40;
    (
        proptest::collection::vec((any::<bool>(), 0u8..3, 0i64..5, any::<bool>()), n),
        proptest::collection::vec(any::<bool>(), n),
        proptest::collection::vec(0u8..9, n),
        proptest::collection::vec(0u8..5, n),
        proptest::collection::vec(0u8..5, n),
    )
        .prop_map(|(hash, keep, installer, archive, usage)| Reads { hash, keep, installer, archive, usage })
}

/// `Downloads` con una sottocartella; con `suffixed` i nomi (e la cartella dello zip) portano " (N)".
fn build_folder(specs: &[Spec], suffixed: bool) -> (Tree, u32) {
    let d = NodeData { flags: Flags::DIR, ..Default::default() };
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let root = t.push(Tree::ROOT, &w("Downloads"), d);
    let sub = t.push(root, &w("sotto"), d);
    for (k, s) in specs.iter().enumerate() {
        let at = if s.sub { sub } else { root };
        let sfx = match (suffixed, s.suffix) {
            (true, Some(n)) => format!(" ({n})"),
            _ => String::new(),
        };
        let stem = format!("{}{k}{sfx}", STEMS[s.stem]);
        let name = if EXTS[s.ext].is_empty() { stem.clone() } else { format!("{stem}.{}", EXTS[s.ext]) };
        let mut flags = Flags::default();
        if s.cloud {
            flags |= Flags::CLOUD;
        }
        if s.links {
            flags |= Flags::MULTI_LINK;
        }
        t.push(at, &w(&name), NodeData { logical: s.size, alloc: s.size.div_ceil(4096) * 4096, flags, ..Default::default() });
        if EXTS[s.ext] == "zip" && s.folder {
            let x = t.push(at, &w(&stem), d);
            t.push(x, &w("dentro.txt"), NodeData { logical: 5, alloc: 4096, ..Default::default() });
        }
    }
    t.aggregate();
    t.seal();
    (t, root)
}

fn random_input(t: &Tree, root: u32, r: &Reads) -> LensInput {
    let lens = downloads::build(t, root);
    let mut input = LensInput::default();
    for (k, it) in lens.items.iter().enumerate() {
        let k = k % 40;
        let (read, digest, created, changed) = r.hash[k];
        if read {
            input.hashed.push(Hashed { node: it.node, size: it.logical + u64::from(changed && digest == 0), digest: [digest; 32], created });
        }
        if r.keep[k] {
            input.keep.push(it.node);
        }
        let outcome = match r.installer[k] {
            0 => Some(Err(ExtractError::Format(FormatIssue::Truncated))),
            1 => Some(Err(ExtractError::CloudPlaceholder)),
            2 => Some(Ok(None)),
            3 => Some(Ok(Some(MatchOutcome::SameVersion { entry: 0, provenance: Provenance::FromSource }))),
            4 => Some(Ok(Some(MatchOutcome::SameVersion { entry: 0, provenance: Provenance::Deduced(Criterion::NameOnly) }))),
            5 => Some(Ok(Some(MatchOutcome::OtherVersion { entry: 0, provenance: Provenance::Deduced(Criterion::OneSource) }))),
            6 => Some(Ok(Some(MatchOutcome::NotInstalled))),
            _ => None,
        };
        if let Some(o) = outcome {
            let read = match o {
                Err(e) => InstallerRead { info: Err(e), matched: None },
                Ok(m) => {
                    let data = info(InstallerKind::Exe, "Prodotto", "1.0");
                    let matched = m.map(|outcome| InstallerMatch { outcome, fact: installers::match_fact(&outcome, &data, &programs()) });
                    InstallerRead { info: Ok(data), matched }
                }
            };
            input.installers.insert(it.node, read);
        }
        match r.archive[k] {
            0 => {
                input.archives.insert(it.node, Ok(ArchiveCheck { total: 3, present: 1, different_size: 1, missing: 1, unreadable: 0 }));
            }
            1 => {
                input.archives.insert(it.node, Err(ExtractError::Io(5)));
            }
            2 => {
                input.archives.insert(it.node, Ok(ArchiveCheck::default()));
            }
            _ => {}
        }
        let usage = match r.usage[k] {
            0 => Some(FileUse::Free),
            1 => Some(FileUse::Used(vec!["chrome.exe".into()])),
            2 => Some(FileUse::Unknown(5)),
            _ => None,
        };
        if let Some(usage) = usage {
            input.usage.insert(it.node, UseCheck { usage, at: 7 });
        }
    }
    input
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// 03 §6.1 regola 2: nessuna voce Gestibile o Da verificare senza una prova che non sia dimensione,
    /// data o nome (lo verifica `check_invariants`, chiamata da `run`), e ogni chiave esiste.
    #[test]
    fn records_from_random_folders_keep_the_rules(specs in proptest::collection::vec(spec(), 0..30), r in reads()) {
        let (t, root) = build_folder(&specs, true);
        let input = random_input(&t, root, &r);
        let f = Folder { tree: t, root, nodes: HashMap::new() };
        let out = run(&f, &input);
        let mut found = BTreeMap::new();
        produced(&out, &mut found);
        check_keys(&found, false);
        // Ogni file della lente con un ruolo compare in una voce del suo tipo.
        let lens = downloads::build(&f.tree, root);
        for it in &lens.items {
            let fam = it.family;
            let covered = out.iter().any(|v| v.nodes.contains(&it.node) && match v.finding {
                Finding::Installer(_) => fam == LensFamily::Installers,
                Finding::DiskImages => fam == LensFamily::DiskImages,
                Finding::Incomplete(_) => fam == LensFamily::Incomplete,
                _ => false,
            });
            let needs = matches!(fam, LensFamily::Installers | LensFamily::DiskImages | LensFamily::Incomplete);
            prop_assert_eq!(covered, needs, "{}", f.tree.name_lossy(it.node));
        }
    }

    /// Prima del confronto nessuna voce di copie ha una cifra recuperabile, e non ci sono copie.
    #[test]
    fn before_the_comparison_copies_have_no_figure(specs in proptest::collection::vec(spec(), 0..30), r in reads()) {
        let (t, root) = build_folder(&specs, true);
        let input = LensInput { hashed: Vec::new(), ..random_input(&t, root, &r) };
        let f = Folder { tree: t, root, nodes: HashMap::new() };
        for v in run(&f, &input) {
            let compared = matches!(v.finding, Finding::IdenticalCopies { .. } | Finding::DifferentContent);
            prop_assert!(!compared, "{:?}", v.finding);
            if v.finding == Finding::SameSize {
                prop_assert_eq!(v.record.estimate, Estimate::NotEstimable(NotEstimable::NoAction));
                prop_assert_eq!(v.record.action, Action::None);
            }
        }
    }

    /// F-DL-3: il suffisso " (N)" non cambia nessuno stato, nessuna voce, nessun record.
    #[test]
    fn the_copy_suffix_changes_nothing(specs in proptest::collection::vec(spec(), 0..30), r in reads()) {
        let (plain, root) = build_folder(&specs, false);
        let (suffixed, _) = build_folder(&specs, true);
        let input = random_input(&plain, root, &r);
        let a = run(&Folder { tree: plain, root, nodes: HashMap::new() }, &input);
        let b = run(&Folder { tree: suffixed, root, nodes: HashMap::new() }, &input);
        prop_assert_eq!(a, b);
    }
}
