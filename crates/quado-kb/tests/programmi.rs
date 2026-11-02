// SPDX-License-Identifier: GPL-3.0-or-later
//! Vista Programmi: elenco, spazio con un solo proprietario per byte, dichiarato contro misurato, uso
//! noto, disinstallazione ufficiale, avviso "tuoi file", possibili residui. Fixture con la forma di un PC
//! vero (`fixture/*.albero`, `registro.reg`, `msix.txt`, Steam, Epic, CapCut) e una costruita a mano
//! (`fixture/programmi/`), più alberi e prove casuali.
//!
//! Gli helper delle fixture sono copiati da `tests/common/mod.rs` (che non si modifica) con un'aggiunta:
//! le righe `D` e `F` degli alberi portano la data di ultima modifica.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use proptest::prelude::*;
use quado_kb::engine::{self, Classification, interventions};
use quado_kb::evidence::{Evidence, RecycleQuery};
use quado_kb::extract::{self, ExtractError, FileSource, FormatIssue, MsixPackage, Power, TrackProgs, Uninstall, UninstallEntry, UserAssist, UserAssistEntry};
use quado_kb::programs::{self, Location, PathEvidence, Program, ProgramSource, Programs, ResidueBase, Space, UninstallVia, Usage};
use quado_kb::reg::{RegFile, RegistrySource, Root};
use quado_kb::{Kb, classify};
use quado_model::record::{
    Action, CheckKind, Criterion, Date, Estimate, Function, Hive, Manager, NotEstimable, Origin, Provenance, Record, Reliability, Source, State, UnknownReason, Value,
};
use quado_model::{Flags, NONE, NodeData, Tree};

const TODAY: Date = Date { year: 2026, month: 10, day: 13 };
const SID: &str = "S-1-5-21-1000-1000-1000-1001";

// ---------------------------------------------------------------------------------------------
// Fixture (copia di tests/common/mod.rs, con le date)

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture")
}

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Albero da fixture `.albero` (percorsi relativi a `fixture/`). Più fixture si uniscono nello stesso albero.
fn tree(names: &[&str]) -> Tree {
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let mut index: HashMap<String, u32> = HashMap::new();
    index.insert("c:\\".into(), Tree::ROOT);
    for name in names {
        let text = std::fs::read_to_string(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
            let (kind, rest) = line.split_once(' ').expect("riga");
            let num = |s: Option<&str>| -> u64 { s.and_then(|x| x.parse().ok()).expect("numero") };
            let (data, path) = match kind {
                "d" => (NodeData { flags: Flags::DIR, ..Default::default() }, rest),
                "n" => (NodeData { flags: Flags::DIR | Flags::DENIED, ..Default::default() }, rest),
                "D" => {
                    let (m, p) = rest.split_once(' ').expect("data");
                    (NodeData { flags: Flags::DIR, mtime: num(Some(m)) as i64, ..Default::default() }, p)
                }
                "f" => {
                    let mut it = rest.splitn(3, ' ');
                    let (alloc, logical) = (num(it.next()), num(it.next()));
                    (NodeData { alloc, logical, ..Default::default() }, it.next().expect("percorso"))
                }
                "F" => {
                    let mut it = rest.splitn(4, ' ');
                    let (alloc, logical, mtime) = (num(it.next()), num(it.next()), num(it.next()) as i64);
                    (NodeData { alloc, logical, mtime, ..Default::default() }, it.next().expect("percorso"))
                }
                other => panic!("riga sconosciuta {other}"),
            };
            if path == "C:\\" {
                continue;
            }
            let key = path.to_lowercase();
            if index.contains_key(&key) {
                continue;
            }
            let (parent, file) = path.rsplit_once('\\').expect("genitore");
            let parent_key = if parent.len() == 2 { format!("{}\\", parent.to_lowercase()) } else { parent.to_lowercase() };
            let p = *index.get(&parent_key).unwrap_or_else(|| panic!("genitore mancante per {path}"));
            let n = t.push(p, &w(file), data);
            index.insert(key, n);
        }
    }
    t.aggregate();
    t.seal();
    t
}

/// File di `fixture/file/<unità>/<percorso>`, senza distinzione di maiuscole.
struct FakeFiles {
    files: HashMap<String, String>,
}

impl FakeFiles {
    fn load() -> Self {
        let base = dir().join("file");
        let mut files = HashMap::new();
        let mut stack = vec![base.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).expect("cartella").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    let rel = p.strip_prefix(&base).expect("relativo").to_string_lossy().replace('/', "\\");
                    let (drive, rest) = rel.split_once('\\').expect("unità");
                    files.insert(format!("{drive}:\\{rest}").to_lowercase(), std::fs::read_to_string(&p).expect("testo"));
                }
            }
        }
        Self { files }
    }
}

impl FileSource for FakeFiles {
    fn read(&self, path: &str) -> Result<String, ExtractError> {
        let t = self.files.get(&extract::normalize(path).to_lowercase()).ok_or(ExtractError::NotPresent)?;
        if t.len() as u64 > extract::MAX_FILE {
            return Err(FormatIssue::TooLarge.into());
        }
        Ok(t.clone())
    }

    /// Nomi dei file e delle sottocartelle (a differenza di `common`, che elenca solo i file: qui serve
    /// `userdata\<id>` di Steam per leggere `localconfig.vdf`).
    fn list(&self, dir: &str) -> Result<Vec<String>, ExtractError> {
        let prefix = format!("{}\\", extract::normalize(dir).to_lowercase());
        let mut names: Vec<String> = self.files.keys().filter_map(|k| k.strip_prefix(&prefix)).filter_map(|r| r.split('\\').next()).map(str::to_owned).collect();
        names.sort();
        names.dedup();
        if names.is_empty() { Err(ExtractError::NotPresent) } else { Ok(names) }
    }
}

fn registry(name: &str) -> RegFile {
    RegFile::parse(&std::fs::read_to_string(dir().join(name)).expect("registro")).expect("formato del registro")
}

/// Prove dalle fixture con i lettori veri, come `common::evidence_with`.
fn evidence_with(reg: &dyn RegistrySource, files: &dyn FileSource) -> Evidence {
    let mut ev = Evidence::empty("C:\\");
    ev.folders = Evidence::default_folders("C:\\", "demo");
    ev.system_volume = true;
    ev.user_sid = Some(SID.into());
    let local = ev.folders.local.clone().expect("local");
    ev.uninstall = extract::uninstall(reg);
    ev.steam = extract::steam(reg, files, "C:\\");
    ev.epic = extract::epic(files, r"C:\ProgramData");
    ev.capcut = extract::capcut(files, &local);
    ev.capcut_drafts = extract::capcut_drafts(files, &local);
    ev.track_progs = extract::track_progs(reg);
    ev.userassist = extract::userassist(reg);
    ev.storage_sense = extract::storage_sense(reg);
    let volumes = reg.subkeys(Root::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume").unwrap_or_default();
    ev.bin = volumes.first().map_or(Err(ExtractError::NotPresent), |g| extract::bin_settings(reg, g));
    ev.power = Ok(Power { hibernate: true, hiberfile_present: true, fast_startup: Some(false), has_battery: true, modern_standby: false, critical_battery_action: Some(2) });
    ev.recycle = Ok(RecycleQuery { bytes: 1_318_890, items: 1 });
    ev.msix = extract::parse_msix_list(&std::fs::read_to_string(dir().join("msix.txt")).expect("msix")).map_err(ExtractError::Format);
    ev.restore = Err(ExtractError::Io(5));
    ev
}

/// Prove del PC delle fixture (registro, MSIX, Steam, Epic, CapCut).
fn pc_evidence() -> Evidence {
    evidence_with(&registry("registro.reg"), &FakeFiles::load())
}

/// Prove della fixture sintetica: le sue voci di registro, un pacchetto MSIX e uno framework, niente
/// Steam né Epic.
fn synthetic_evidence() -> Evidence {
    let mut ev = evidence_with(&registry("programmi/registro.reg"), &FakeFiles { files: HashMap::new() });
    ev.capcut = Err(ExtractError::NotPresent);
    ev.capcut_drafts = Err(ExtractError::NotPresent);
    ev.msix = Ok(vec![
        MsixPackage {
            family_name: "Microsoft.WindowsTerminal_8wekyb3d8bbwe".into(),
            installed_path: Some(r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_1.23.0.0_x64__8wekyb3d8bbwe".into()),
            framework: false,
        },
        MsixPackage { family_name: "Microsoft.VCLibs.140.00_8wekyb3d8bbwe".into(), installed_path: None, framework: true },
    ]);
    ev
}

fn kb() -> Kb {
    Kb::embedded().expect("regole")
}

struct View {
    tree: Tree,
    cls: Classification,
    p: Programs,
}

fn view(kb: &Kb, albero: &[&str], ev: &Evidence) -> View {
    let tree = tree(albero);
    let cls = classify(kb, &tree, ev, TODAY);
    let p = programs::build(&tree, ev, kb, &cls);
    View { tree, cls, p }
}

fn item<'a>(p: &'a Programs, name: &str) -> &'a Program {
    p.items.iter().find(|x| x.name == name).unwrap_or_else(|| panic!("programma {name} assente"))
}

fn node(t: &Tree, path: &str) -> u32 {
    engine::find(t, path).unwrap_or_else(|| panic!("{path} assente"))
}

fn has_fact(r: &Record, key: &str) -> bool {
    r.identity.0.iter().any(|f| f.key == key)
}

/// Gruppi di programmi con la stessa cartella: indici ordinati.
fn groups(p: &Programs) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    for (i, x) in p.items.iter().enumerate() {
        let mut g: Vec<usize> = x.shared_with.clone();
        g.push(i);
        g.sort_unstable();
        if !out.contains(&g) {
            out.push(g);
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Invarianti

/// Oracolo del proprietario di ogni file: la cartella di programma, il dato fuori dalla cartella o il tuo
/// file più vicino risalendo (prefisso più lungo). Somma per gruppo confrontata con lo spazio misurato.
fn check_one_owner_per_byte(t: &Tree, cls: &Classification, p: &Programs) {
    let gs = groups(p);
    let group_of = |i: usize| gs.iter().position(|g| g.contains(&i)).expect("gruppo");
    let mut delim: HashMap<u32, Option<usize>> = HashMap::new();
    for (n, m) in cls.anchors() {
        if cls.matches[m].manager == Manager::You {
            delim.insert(n, None);
        }
    }
    for (i, x) in p.items.iter().enumerate() {
        if let Some(n) = x.location.node() {
            delim.insert(n, Some(group_of(i)));
        }
    }
    for (i, x) in p.items.iter().enumerate() {
        for d in x.data.iter().filter(|d| !d.inside) {
            delim.insert(d.node, Some(group_of(i)));
        }
    }
    let mut by_group = vec![0u64; gs.len()];
    let mut counted = 0u64;
    for f in 0..t.len() as u32 {
        let fl = t.flags(f);
        if fl.has(Flags::DIR) || fl.any(Flags::SECONDARY_LINK | Flags::DETACHED) {
            continue;
        }
        let mut n = f;
        while n != NONE {
            if let Some(owner) = delim.get(&n) {
                if let Some(g) = owner {
                    by_group[*g] += t.alloc(f);
                    counted += t.alloc(f);
                }
                break;
            }
            n = t.parent(n);
        }
    }
    for (k, g) in gs.iter().enumerate() {
        let first = &p.items[g[0]];
        for &i in g {
            assert_eq!(p.items[i].space, first.space, "{}: stesso spazio nel gruppo", p.items[i].name);
        }
        if let Space::Measured(b) = first.space {
            assert_eq!(b, by_group[k], "{}: byte del gruppo", first.name);
        } else {
            assert_eq!(by_group[k], 0, "{}: byte senza misura", first.name);
        }
    }
    assert_eq!(p.measured_total(), counted, "totale: ogni byte una volta");
    assert!(p.measured_total() <= t.alloc(Tree::ROOT));
}

/// Proprietà dei record della vista, compresi uso noto e possibili residui.
fn check_records(t: &Tree, cls: &Classification, p: &Programs) {
    for x in &p.items {
        let r = &x.record;
        assert_eq!(r.state, State::YouDecide, "{}", x.name);
        assert_eq!(r.origin, Origin::Function(Function::Program));
        assert!(!r.identity.0.is_empty());
        assert!(r.text_keys().all(|k| !k.trim().is_empty()));
        assert_eq!(r.action == Action::Uninstall, x.uninstall.is_some(), "{}", x.name);
        assert_eq!(r.preferred.is_some(), x.uninstall.is_some());
        assert_ne!(r.reliability.level(), Reliability::Low, "{}: Decidi tu con affidabilità bassa", x.name);
        if x.hidden {
            assert_eq!(r.action, Action::None);
        }
        // Mai spazio per somiglianza del nome: senza prova del percorso né dati, 0 byte.
        if matches!(x.location, Location::NotDeclared | Location::Implausible { .. }) && x.data.iter().all(|d| d.inside) {
            assert_eq!(x.space, Space::NotAttributed, "{}", x.name);
        }
        // Il dichiarato non entra nel misurato: è un fatto da fonte a parte.
        if let Some(d) = &x.declared {
            assert!(r.identity.0.iter().any(|f| f.key == "fatto-dimensione-dichiarata" && f.provenance == Provenance::FromSource && f.source == d.source));
        }
        // UserAssist solo con il tracciamento attivo.
        if p.tracking != TrackProgs::On {
            assert!(!matches!(x.usage, Usage::UserAssist(_)), "{}", x.name);
            assert!(r.identity.0.iter().all(|f| f.key != "fatto-uso-userassist"), "{}", x.name);
        }
        // Tuoi file: mai contati, sempre con l'avviso e il controllo manuale.
        for y in &x.your_files {
            assert_eq!(cls.matches[y.matched].manager, Manager::You);
            assert!(r.identity.0.iter().any(|f| f.key == "fatto-programma-tuoi-file"));
            if r.action == Action::Uninstall {
                assert!(r.checks.iter().any(|c| c.key == "funzione-programma-controllo-tuoi-file" && c.kind == CheckKind::Manual));
            }
        }
    }
    let folders: Vec<u32> = p.items.iter().filter_map(|x| x.location.node()).collect();
    let card: HashSet<u32> = interventions(&quado_kb::Kb::embedded().expect("regole"), cls, t).iter().flat_map(|i| i.matches.iter().flat_map(|&k| cls.matches[k].nodes.clone())).collect();
    for res in &p.residues {
        let r = &res.record;
        assert_eq!(r.action, Action::None);
        assert_eq!(r.state, State::Unknown(UnknownReason::NameOnly));
        assert_eq!(r.origin, Origin::Function(Function::PossibleResidue));
        assert!(r.preferred.is_none() && r.alternatives.is_empty() && r.checks.is_empty());
        assert_eq!(r.estimate, Estimate::NotEstimable(NotEstimable::NoAction));
        assert!(!card.contains(&res.node), "residuo nella card interventi");
        // Nessun residuo è un programma, sta dentro la cartella di un programma o ne contiene una.
        for &f in &folders {
            assert!(!is_ancestor(t, f, res.node) && !is_ancestor(t, res.node, f), "{}: residuo e programma", t.path_lossy(res.node));
        }
        // Nessuna regola sulla cartella, sotto o sopra (fino alla base).
        assert!(cls.own(res.node).is_none());
        assert!(!cls.anchors().any(|(n, _)| is_ancestor(t, res.node, n)));
    }
}

fn is_ancestor(t: &Tree, a: u32, mut b: u32) -> bool {
    while b != NONE {
        if a == b {
            return true;
        }
        b = t.parent(b);
    }
    false
}

/// Somma per programma contro la classificazione: i byte della cartella sono quelli degli elementi
/// riconosciuti al suo interno che non sono di altri programmi o tuoi (`own_bytes` di ciascuno).
fn check_matches_classification(t: &Tree, cls: &Classification, p: &Programs) -> usize {
    let anchors: Vec<(u32, usize)> = cls.anchors().collect();
    let anchor_set: HashSet<u32> = anchors.iter().map(|a| a.0).collect();
    let mut delims: HashSet<u32> = anchors.iter().filter(|&&(_, m)| cls.matches[m].manager == Manager::You).map(|a| a.0).collect();
    delims.extend(p.items.iter().filter_map(|x| x.location.node()));
    delims.extend(p.items.iter().flat_map(|x| x.data.iter().filter(|d| !d.inside).map(|d| d.node)));
    let mut checked = 0;
    for g in groups(p) {
        let x = &p.items[g[0]];
        let Space::Measured(bytes) = x.space else { continue };
        let tops: Vec<u32> = x.location.node().into_iter().chain(x.data.iter().filter(|d| !d.inside).map(|d| d.node)).collect();
        // L'uguaglianza vale quando tutti i delimitatori interni sono elementi riconosciuti.
        if !tops.iter().all(|n| anchor_set.contains(n)) || delims.iter().any(|&d| tops.iter().any(|&top| d != top && is_ancestor(t, top, d)) && !anchor_set.contains(&d)) {
            continue;
        }
        let mut sum = 0u64;
        for &top in &tops {
            for &(a, _) in &anchors {
                if !is_ancestor(t, top, a) {
                    continue;
                }
                // Il primo delimitatore risalendo da `a` (incluso) dev'essere `top`.
                let mut n = a;
                while n != top && !delims.contains(&n) {
                    n = t.parent(n);
                }
                if n == top {
                    sum += cls.own_bytes(t, a);
                }
            }
        }
        assert_eq!(bytes, sum, "{}: misurato diverso dalla classificazione", x.name);
        checked += 1;
    }
    checked
}

// ---------------------------------------------------------------------------------------------
// Elenco

#[test]
fn list_comes_from_every_source() {
    let kb = kb();
    let ev = pc_evidence();
    let v = view(&kb, &["giochi.albero"], &ev);
    let count = |f: &dyn Fn(&ProgramSource) -> bool| v.p.items.iter().filter(|x| f(&x.source)).count();
    let entries = ev.uninstall.as_ref().expect("voci").entries.len();
    assert_eq!(count(&|s| matches!(s, ProgramSource::Uninstall { .. })), entries, "ogni voce con DisplayName");
    let msix = ev.msix.as_ref().expect("msix");
    assert_eq!(count(&|s| matches!(s, ProgramSource::Msix { .. })), msix.iter().filter(|p| !p.framework).count(), "MSIX non framework");
    assert!(v.p.items.iter().all(|x| !x.name.contains("Framework")));
    assert_eq!(count(&|s| matches!(s, ProgramSource::Steam { .. })), 3);
    assert_eq!(count(&|s| matches!(s, ProgramSource::Epic { .. })), 3);
    assert!(v.p.errors.is_empty(), "{:?}", v.p.errors);
    // Ogni voce porta la sua fonte: hive e chiave, PFN, appid.
    let capcut = item(&v.p, "CapCut");
    assert!(matches!(&capcut.source, ProgramSource::Uninstall { hive: Hive::User, key, .. } if key == "CapCut"));
    let steam_entry = ev.uninstall.as_ref().expect("voci").entries.iter().position(|e| e.display_name == "Steam").expect("Steam");
    assert_eq!(item(&v.p, "Steam").source, ProgramSource::Uninstall { hive: Hive::Machine32, key: "Steam".into(), entry: steam_entry });
    assert_eq!(item(&v.p, "Hollow Peaks").source, ProgramSource::Steam { appid: 3_999_910 });
    assert_eq!(item(&v.p, "Microsoft.WindowsTerminal_8wekyb3d8bbwe").source, ProgramSource::Msix { family_name: "Microsoft.WindowsTerminal_8wekyb3d8bbwe".into() });
    assert_eq!(item(&v.p, "Unreal Engine").source, ProgramSource::Epic { app_name: "UE_5.5".into() });
    check_records(&v.tree, &v.cls, &v.p);
    check_one_owner_per_byte(&v.tree, &v.cls, &v.p);
}

#[test]
fn system_components_are_hidden_and_have_no_method() {
    let kb = kb();
    let v = view(&kb, &["programmi/programmi.albero"], &synthetic_evidence());
    let c = item(&v.p, "Esempio Componente");
    assert!(c.hidden);
    assert!(c.uninstall.is_none() && c.record.action == Action::None && c.record.preferred.is_none());
    assert!(has_fact(&c.record, "fatto-programma-componente-di-sistema"));
    assert!(v.p.visible().all(|x| !x.hidden));
    assert_eq!(v.p.visible().count(), v.p.items.len() - 1);
    // Il pacchetto framework non è nell'elenco.
    assert!(v.p.items.iter().all(|x| !x.name.starts_with("Microsoft.VCLibs")));
    check_records(&v.tree, &v.cls, &v.p);
}

#[test]
fn extractor_failures_arrive_as_codes_without_data() {
    let kb = kb();
    let mut ev = pc_evidence();
    ev.uninstall = Err(ExtractError::Io(5));
    ev.msix = Err(ExtractError::Format(FormatIssue::BadValue(extract::Field::PackageFamilyName)));
    if let Ok(s) = &mut ev.steam {
        s.skipped.push((r"C:\Program Files (x86)\Steam\steamapps\appmanifest_1.acf".into(), FormatIssue::Truncated));
    }
    ev.epic = Err(ExtractError::NotPresent);
    let v = view(&kb, &["giochi.albero"], &ev);
    assert!(v.p.items.iter().all(|x| matches!(x.source, ProgramSource::Steam { .. })), "nessun dato ricostruito");
    let e = |x: programs::Extractor| v.p.errors.iter().filter(|s| s.extractor == x).map(|s| (s.error, s.path.clone())).collect::<Vec<_>>();
    assert_eq!(e(programs::Extractor::Uninstall), vec![(ExtractError::Io(5), None)]);
    assert_eq!(e(programs::Extractor::Msix), vec![(ExtractError::Format(FormatIssue::BadValue(extract::Field::PackageFamilyName)), None)]);
    let manifest = r"C:\Program Files (x86)\Steam\steamapps\appmanifest_1.acf".to_owned();
    assert_eq!(e(programs::Extractor::Steam), vec![(ExtractError::Format(FormatIssue::Truncated), Some(manifest))]);
    assert_eq!(e(programs::Extractor::Epic), vec![(ExtractError::NotPresent, None)]);
    // Voce del registro con un tipo inatteso: saltata, con la chiave.
    let mut ev = pc_evidence();
    if let Ok(u) = &mut ev.uninstall {
        u.skipped.push((Hive::User, "Rotta".into(), FormatIssue::WrongType(extract::Field::EstimatedSize)));
    }
    let v = view(&kb, &["giochi.albero"], &ev);
    let s = v.p.errors.iter().find(|s| s.extractor == programs::Extractor::Uninstall).expect("voce saltata");
    assert_eq!(s.path.as_deref(), Some(r"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Uninstall\Rotta"));
}

// ---------------------------------------------------------------------------------------------
// Posizione e spazio

#[test]
fn without_a_path_proof_location_is_not_declared_and_zero_bytes() {
    let kb = kb();
    let v = view(&kb, &["programmi/programmi.albero"], &synthetic_evidence());
    // "Senza Prova" ha una cartella con lo stesso nome in Program Files: nessuno spazio per somiglianza.
    let s = item(&v.p, "Senza Prova");
    assert_eq!(s.location, Location::NotDeclared);
    assert_eq!(s.space, Space::NotAttributed);
    assert_eq!(s.space.bytes(), 0);
    assert!(has_fact(&s.record, "fatto-programma-posizione-non-dichiarata") && has_fact(&s.record, "fatto-programma-nessuno-spazio"));
    // Percorso dichiarato non plausibile (una cartella nota): come nel motore, niente spazio.
    let bad = item(&v.p, "Voce Sbagliata");
    assert!(matches!(&bad.location, Location::Implausible { evidence: PathEvidence::InstallLocation, .. }));
    assert_eq!(bad.space, Space::NotAttributed);
    // Altro volume: cartella dichiarata ma non misurata, mai 0.
    let other = item(&v.p, "Altro Volume");
    assert!(matches!(&other.location, Location::Folder { node: None, .. }));
    assert_eq!(other.space, Space::NotMeasured);
    assert!(has_fact(&other.record, "fatto-programma-non-misurato"));
    // Fixture del PC: 7-Zip ha solo MsiExec e InstallLocation vuota.
    let pc = view(&kb, &["profilo.albero"], &pc_evidence());
    let z = item(&pc.p, "7-Zip 26.03 (x64 edition)");
    assert_eq!((z.location.clone(), z.space), (Location::NotDeclared, Space::NotAttributed));
}

#[test]
fn location_provenance_follows_the_engine() {
    let kb = kb();
    let v = view(&kb, &["programmi/programmi.albero"], &synthetic_evidence());
    // HKLM con InstallLocation: da fonte, affidabilità alta.
    let e = item(&v.p, "Esempio");
    assert!(matches!(&e.location, Location::Folder { evidence: PathEvidence::InstallLocation, provenance: Provenance::FromSource, node: Some(_), .. }));
    assert_eq!(e.record.reliability, Criterion::OfficialWithStructure);
    // HKCU, scrivibile da qualunque programma dell'utente: dedotto media.
    let u = item(&v.p, "Utente App");
    assert!(matches!(&u.location, Location::Folder { provenance: Provenance::Deduced(Criterion::OneSource), node: Some(_), .. }));
    assert_eq!(u.record.reliability, Criterion::OneSource);
    assert_eq!(u.space, Space::Measured(65_536));
    // Fixture del PC: CapCut dall'indizio di HKCU con ProductInfo.xml concorde (due fonti, come il motore);
    // Steam dall'indizio di HKLM con `uninstall.exe` presente nella cartella (fonte di Windows con la struttura).
    let pc = view(&kb, &["capcut.albero", "profilo.albero"], &pc_evidence());
    let c = item(&pc.p, "CapCut");
    let Location::Folder { evidence, provenance, node: Some(_), path } = &c.location else { panic!("{:?}", c.location) };
    assert_eq!((*evidence, *provenance), (PathEvidence::UninstallHint, Provenance::Deduced(Criterion::TwoSources)));
    assert!(path.ends_with(r"CapCut\Apps"), "{path}");
    assert!(c.record.hypotheses.iter().any(|h| h == "funzione-programma-ipotesi-indizio"));
    let s = item(&pc.p, "Steam");
    let Location::Folder { evidence, provenance, .. } = &s.location else { panic!("{:?}", s.location) };
    assert_eq!((*evidence, *provenance), (PathEvidence::UninstallHint, Provenance::Deduced(Criterion::OfficialWithStructure)));
    // Lo stesso nodo ha la regola dei programmi registrati con la stessa attribuzione.
    let n = c.location.node().expect("nodo");
    let m = &pc.cls.matches[pc.cls.match_of(n).expect("corrispondenza")];
    assert_eq!(m.attribution, Provenance::Deduced(Criterion::TwoSources));
}

#[test]
fn shared_folders_are_counted_once() {
    // Unreal Engine, Quixel Bridge e il plugin Fab dichiarano la stessa InstallLocation.
    let kb = kb();
    let v = view(&kb, &["giochi.albero"], &pc_evidence());
    let ue = node(&v.tree, r"C:\Program Files\Epic Games\UE_5.5");
    let epic: Vec<usize> = v.p.items.iter().enumerate().filter(|(_, x)| matches!(x.source, ProgramSource::Epic { .. })).map(|(i, _)| i).collect();
    assert_eq!(epic.len(), 3);
    for &i in &epic {
        let x = &v.p.items[i];
        assert_eq!(x.location.node(), Some(ue));
        assert_eq!(x.space, Space::Measured(v.tree.alloc(ue)));
        let mut others = x.shared_with.clone();
        others.sort_unstable();
        assert_eq!(others, epic.iter().copied().filter(|&j| j != i).collect::<Vec<_>>());
        assert!(x.record.identity.0.iter().any(|f| f.key == "fatto-programma-cartella-condivisa" && f.values == vec![Value::Count(2)]));
        assert!(matches!(x.uninstall, Some(UninstallVia::EpicLibrary)));
        assert!(x.record.preferred.as_ref().is_some_and(|s| s.uri.is_none() && s.command.is_none()));
    }
    // Nel totale la cartella conta una volta (gli altri programmi della fixture non condividono cartelle).
    let others: u64 = v.p.items.iter().filter(|x| !matches!(x.source, ProgramSource::Epic { .. })).map(|x| x.space.bytes()).sum();
    assert!(v.p.items.iter().filter(|x| !matches!(x.source, ProgramSource::Epic { .. })).all(|x| x.shared_with.is_empty()));
    assert_eq!(v.p.measured_total(), others + v.tree.alloc(ue));
    // Sintetica: due voci del registro (HKLM e WOW6432Node) sulla stessa cartella, una con la barra finale.
    let s = view(&kb, &["programmi/programmi.albero"], &synthetic_evidence());
    let (a, b) = (item(&s.p, "Esempio"), item(&s.p, "Esempio Plugin"));
    assert_eq!(a.location.node(), b.location.node());
    assert_eq!(a.space, b.space);
    assert_eq!(a.shared_with.len(), 1);
    check_one_owner_per_byte(&v.tree, &v.cls, &v.p);
    check_one_owner_per_byte(&s.tree, &s.cls, &s.p);
}

#[test]
fn nested_programs_and_your_files_are_not_counted() {
    let kb = kb();
    let v = view(&kb, &["programmi/programmi.albero"], &synthetic_evidence());
    let t = &v.tree;
    let esempio = node(t, r"C:\Program Files\Esempio");
    let comp = node(t, r"C:\Program Files\Esempio\Componente");
    let nm = node(t, r"C:\Program Files\Esempio\resources\node_modules");
    let e = item(&v.p, "Esempio");
    // Prefisso più lungo: il componente nascosto ha la sua cartella. La node_modules con il lockfile accanto
    // sta nella cartella di un programma (un'app Electron): fa parte del programma, non è un tuo progetto.
    // I tuoi file dentro un programma si provano con CapCut.
    assert_eq!(e.space, Space::Measured(t.alloc(esempio) - t.alloc(comp)));
    assert_eq!(item(&v.p, "Esempio Componente").space, Space::Measured(t.alloc(comp)));
    assert!(e.your_files.is_empty());
    assert!(v.cls.own(nm).is_none(), "nessuna regola propone la node_modules di un programma");
    assert!(!e.record.identity.0.iter().any(|f| f.key == "fatto-programma-tuoi-file"));
    check_one_owner_per_byte(t, &v.cls, &v.p);
    assert!(check_matches_classification(t, &v.cls, &v.p) >= 2);
}

#[test]
fn rule_data_outside_the_folder_needs_the_exact_name() {
    let kb = kb();
    let v = view(&kb, &["programmi/programmi.albero"], &synthetic_evidence());
    let t = &v.tree;
    // cache-brave dichiara "Brave" come gestore: i suoi byte sono di Brave, fuori dalla cartella.
    let b = item(&v.p, "Brave");
    let app = node(t, r"C:\Program Files\BraveSoftware\Brave-Browser\Application");
    let cache = node(t, r"C:\Users\demo\AppData\Local\BraveSoftware\Brave-Browser\User Data\Default\Cache");
    assert_eq!(b.data.len(), 1);
    assert_eq!((b.data[0].node, b.data[0].inside), (cache, false));
    assert_eq!(kb.rules[b.data[0].rule].id, "cache-brave");
    assert_eq!(b.space, Space::Measured(t.alloc(app) + t.alloc(cache)));
    assert_eq!(b.area, None, "cartella e cache non hanno una cartella comune sotto una cartella nota");
    assert!(has_fact(&b.record, "fatto-programma-dati-regole"));
    // Un nome solo simile non basta: con "Brave Browser" la cache non è di nessun programma.
    let mut ev = synthetic_evidence();
    if let Ok(u) = &mut ev.uninstall {
        for e in &mut u.entries {
            if e.display_name == "Brave" {
                e.display_name = "Brave Browser".into();
            }
        }
    }
    let v2 = view(&kb, &["programmi/programmi.albero"], &ev);
    let b2 = item(&v2.p, "Brave Browser");
    assert!(b2.data.is_empty());
    assert_eq!(b2.space, Space::Measured(t.alloc(app)));
    // MSIX: InstalledLocation non leggibile (WindowsApps), cartella dati `Packages\<PFN>` dalla regola.
    let m = item(&v.p, "Microsoft.WindowsTerminal_8wekyb3d8bbwe");
    let data = node(t, r"C:\Users\demo\AppData\Local\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe");
    assert!(matches!(&m.location, Location::Folder { evidence: PathEvidence::MsixInstalled, node: None, .. }));
    assert_eq!(m.space, Space::Measured(t.alloc(data)));
    assert_eq!(kb.rules[m.data[0].rule].id, "pacchetti-msix");
    assert!(has_fact(&m.record, "fatto-programma-solo-dati"), "il misurato dice che la cartella non è letta");
    assert!(!has_fact(&b.record, "fatto-programma-solo-dati"));
    check_one_owner_per_byte(t, &v.cls, &v.p);
}

#[test]
fn capcut_warns_about_your_files() {
    // Con la fixture di CapCut compare l'avviso sui tuoi file. La cartella del programma è `Apps`
    // (indizio del programma di disinstallazione); la cache di CapCut, gestita da CapCut, sta in
    // `User Data`: la cartella comune `Local\CapCut` è l'ambito in cui stanno anche Projects e Videos.
    let kb = kb();
    let v = view(&kb, &["capcut.albero"], &pc_evidence());
    let t = &v.tree;
    let c = item(&v.p, "CapCut");
    let apps = node(t, r"C:\Users\demo\AppData\Local\CapCut\Apps");
    let root = node(t, r"C:\Users\demo\AppData\Local\CapCut");
    let cache = node(t, r"C:\Users\demo\AppData\Local\CapCut\User Data\Cache");
    let projects = node(t, r"C:\Users\demo\AppData\Local\CapCut\User Data\Projects");
    let videos = node(t, r"C:\Users\demo\AppData\Local\CapCut\Videos");
    assert_eq!(c.area, Some(root));
    let mut yours: Vec<(&str, u64)> = c.your_files.iter().map(|y| (y.relative.as_str(), y.bytes)).collect();
    yours.sort_unstable();
    assert_eq!(yours, vec![(r"User Data\Projects", t.alloc(projects)), ("Videos", t.alloc(videos))]);
    // Tuoi file mai contati: misurato = Apps (con le versioni) più la cache.
    assert_eq!(c.space, Space::Measured(t.alloc(apps) + t.alloc(cache)));
    let ids: Vec<&str> = c.data.iter().map(|d| kb.rules[d.rule].id.as_str()).collect();
    assert_eq!(ids.iter().filter(|&&i| i == "capcut-versioni").count(), 9, "le versioni precedenti, dentro la cartella");
    assert!(c.data.iter().any(|d| !d.inside && d.node == cache && kb.rules[d.rule].id == "capcut-cache"));
    let warn: Vec<&quado_model::record::Fact> = c.record.identity.0.iter().filter(|f| f.key == "fatto-programma-tuoi-file").collect();
    assert_eq!(warn.len(), 2);
    assert!(warn.iter().any(|f| f.values == vec![Value::Text("CapCut".into()), Value::Bytes(t.alloc(projects)), Value::Text(r"User Data\Projects".into())]));
    assert!(c.record.hypotheses.iter().any(|h| h == "funzione-programma-ipotesi-ambito"));
    assert!(c.record.checks.iter().any(|k| k.key == "funzione-programma-controllo-tuoi-file"));
    assert_eq!(c.uninstall, Some(UninstallVia::Settings { name: "CapCut".into() }));
    check_records(t, &v.cls, &v.p);
    check_one_owner_per_byte(t, &v.cls, &v.p);
    assert!(check_matches_classification(t, &v.cls, &v.p) >= 1);
}

#[test]
fn measured_is_coherent_with_the_classification_on_every_fixture() {
    let kb = kb();
    let ev = pc_evidence();
    let mut checked = 0;
    for f in ["capcut.albero", "giochi.albero", "profilo.albero", "radice.albero", "sviluppo.albero", "browser.albero"] {
        let v = view(&kb, &[f], &ev);
        checked += check_matches_classification(&v.tree, &v.cls, &v.p);
        check_one_owner_per_byte(&v.tree, &v.cls, &v.p);
        check_records(&v.tree, &v.cls, &v.p);
    }
    assert!(checked >= 5, "programmi confrontati: {checked}");
}

// ---------------------------------------------------------------------------------------------
// Dichiarato contro misurato

#[test]
fn declared_sizes_keep_their_source_and_stay_out_of_the_measure() {
    let kb = kb();
    let v = view(&kb, &["giochi.albero"], &pc_evidence());
    let game = item(&v.p, "Hollow Peaks");
    let folder = node(&v.tree, r"C:\Program Files (x86)\Steam\steamapps\common\hollow peaks");
    assert_eq!(game.declared.as_ref().map(|d| (d.bytes, d.source.clone())), Some((61_874_502_118, Source::SteamManifest)));
    assert_eq!(game.space, Space::Measured(v.tree.alloc(folder)));
    assert_ne!(game.space.bytes(), 61_874_502_118);
    let fusion = item(&v.p, "Autodesk Fusion");
    assert_eq!(fusion.declared.as_ref().map(|d| (d.bytes, d.source.clone())), Some((0x0059_a794 * 1024, Source::Uninstall(Hive::User))));
    let e = view(&kb, &["programmi/programmi.albero"], &synthetic_evidence());
    let x = item(&e.p, "Esempio");
    assert_eq!(x.declared.as_ref().map(|d| d.bytes), Some(0x2800 * 1024));
    assert_ne!(x.space.bytes(), 0x2800 * 1024);
}

// ---------------------------------------------------------------------------------------------
// Uso noto

#[test]
fn steam_last_played_from_manifest_and_localconfig_agree() {
    let kb = kb();
    let v = view(&kb, &["giochi.albero"], &pc_evidence());
    let mut both = 0;
    for x in v.p.items.iter().filter(|x| matches!(x.source, ProgramSource::Steam { .. })) {
        let Usage::Steam { manifest, local_config } = x.usage else { panic!("{}: uso di Steam", x.name) };
        if let (Some(a), Some(b)) = (manifest, local_config) {
            assert!((a - b).abs() <= 86_400 * 10_000_000, "{}: le due fonti differiscono di più di un giorno", x.name);
            both += 1;
        }
        let f = x.record.identity.0.iter().find(|f| f.key == "fatto-ultima-sessione-steam").expect("ultima sessione");
        assert_eq!((f.provenance, f.source.clone()), (Provenance::FromSource, Source::SteamManifest));
    }
    assert_eq!(both, 3);
    // Senza LastPlayed nel manifest vale localconfig.vdf, con la sua fonte.
    let mut ev = pc_evidence();
    if let Ok(s) = &mut ev.steam {
        for a in &mut s.apps {
            a.last_played = None;
        }
    }
    let v = view(&kb, &["giochi.albero"], &ev);
    let f = item(&v.p, "Brinewood").record.identity.0.iter().find(|f| f.key == "fatto-ultima-sessione-steam").cloned().expect("ultima sessione");
    assert_eq!(f.source, Source::SteamLocalConfig);
}

#[test]
fn userassist_needs_tracking_on() {
    let kb = kb();
    // Questo PC: Start_TrackProgs = 0 e la chiave con dati vecchi. Nessun valore UserAssist.
    let ev = pc_evidence();
    assert_eq!(ev.track_progs, TrackProgs::Off);
    let v = view(&kb, &["profilo.albero"], &ev);
    assert_eq!(v.p.tracking, TrackProgs::Off);
    for x in v.p.items.iter().filter(|x| matches!(x.source, ProgramSource::Uninstall { .. })) {
        assert_eq!(x.usage, Usage::NotTracked(TrackProgs::Off));
        assert!(has_fact(&x.record, "fatto-uso-tracciamento-disattivato"));
        assert!(x.record.identity.0.iter().all(|f| f.source != Source::UserAssist || f.provenance == Provenance::Unavailable));
    }
    // Prove incoerenti (tracciamento spento ma voci UserAssist presenti): i valori non si usano.
    let mut ev = synthetic_evidence();
    let entries = match &ev.userassist {
        Ok(UserAssist::Entries(e)) => e.clone(),
        other => panic!("UserAssist della fixture: {other:?}"),
    };
    ev.track_progs = TrackProgs::Off;
    let v = view(&kb, &["programmi/programmi.albero"], &ev);
    assert_eq!(item(&v.p, "Esempio").usage, Usage::NotTracked(TrackProgs::Off));
    // Tracciamento attivo: abbinamento per cartella, il più recente e gli avvii sommati.
    let v = view(&kb, &["programmi/programmi.albero"], &synthetic_evidence());
    let last = |name: &str| entries.iter().find(|e| e.name.ends_with(name)).and_then(|e| e.last_run).expect("avvio");
    let e = item(&v.p, "Esempio");
    assert_eq!(e.usage, Usage::UserAssist(Some(programs::Launches { last: last("esempio.exe").max(last("strumento.exe")), runs: 6 })));
    let f = e.record.identity.0.iter().find(|f| f.key == "fatto-uso-userassist").expect("ultimo avvio");
    assert_eq!((f.provenance, f.source.clone(), f.values.clone()), (Provenance::FromSource, Source::UserAssist, vec![Value::Time(last("esempio.exe"))]));
    // Prefisso più lungo: l'eseguibile nella cartella del componente è del componente.
    assert_eq!(item(&v.p, "Esempio Componente").usage, Usage::UserAssist(Some(programs::Launches { last: last("comp.exe"), runs: 2 })));
    assert_eq!(item(&v.p, "Altro Volume").usage, Usage::UserAssist(None));
    assert!(has_fact(&item(&v.p, "Altro Volume").record, "fatto-uso-userassist-nessun-avvio"));
    assert_eq!(item(&v.p, "Senza Prova").usage, Usage::NoFolder);
    // Epic e MSIX: nessun dato d'uso.
    assert_eq!(item(&v.p, "Microsoft.WindowsTerminal_8wekyb3d8bbwe").usage, Usage::NotRecorded);
    assert!(has_fact(&item(&v.p, "Microsoft.WindowsTerminal_8wekyb3d8bbwe").record, "fatto-uso-msix-nessun-dato"));
    let pc = view(&kb, &["giochi.albero"], &pc_evidence());
    assert_eq!(item(&pc.p, "Unreal Engine").usage, Usage::NotRecorded);
    assert!(has_fact(&item(&pc.p, "Unreal Engine").record, "fatto-epic-nessun-dato-uso"));
    // UserAssist non letto con il tracciamento attivo: codice, nessun valore.
    let mut ev = synthetic_evidence();
    ev.userassist = Err(ExtractError::Format(FormatIssue::BadValue(extract::Field::Data)));
    let v = view(&kb, &["programmi/programmi.albero"], &ev);
    assert_eq!(item(&v.p, "Esempio").usage, Usage::Unreadable(ExtractError::Format(FormatIssue::BadValue(extract::Field::Data))));
    assert!(v.p.errors.iter().any(|s| s.extractor == programs::Extractor::UserAssist));
}

#[test]
fn install_date_is_validated() {
    let kb = kb();
    let pc = view(&kb, &["profilo.albero"], &pc_evidence());
    let nv = item(&pc.p, "NVIDIA Graphics Driver 617.42");
    assert_eq!(nv.install_date, Some(Date::new(2026, 10, 8)));
    assert!(has_fact(&nv.record, "fatto-programma-data-installazione"));
    let v = view(&kb, &["programmi/programmi.albero"], &synthetic_evidence());
    assert_eq!(item(&v.p, "Esempio").install_date, Some(Date::new(2025, 3, 14)));
    // "20251399" non è una data: si ignora.
    let plugin = item(&v.p, "Esempio Plugin");
    assert_eq!(plugin.install_date, None);
    assert!(!has_fact(&plugin.record, "fatto-programma-data-installazione"));
}

// ---------------------------------------------------------------------------------------------
// Disinstallazione

#[test]
fn uninstall_methods_are_the_official_ones() {
    let kb = kb();
    let v = view(&kb, &["giochi.albero"], &pc_evidence());
    let uri = |x: &Program| x.record.preferred.as_ref().and_then(|s| s.uri.clone());
    let game = item(&v.p, "Hollow Peaks");
    assert_eq!(game.uninstall, Some(UninstallVia::Steam { appid: 3_999_910 }));
    assert_eq!(uri(game).as_deref(), Some("steam://uninstall/3999910"));
    let msix = item(&v.p, "Microsoft.WindowsTerminal_8wekyb3d8bbwe");
    assert_eq!(msix.uninstall, Some(UninstallVia::SettingsApp { family_name: "Microsoft.WindowsTerminal_8wekyb3d8bbwe".into() }));
    assert_eq!(uri(msix).as_deref(), Some("ms-settings:appsfeatures-app?Microsoft.WindowsTerminal_8wekyb3d8bbwe"));
    let w = item(&v.p, "Autodesk Fusion");
    assert_eq!(w.uninstall, Some(UninstallVia::Settings { name: "Autodesk Fusion".into() }));
    assert_eq!(uri(w).as_deref(), Some("ms-settings:appsfeatures"));
    assert_eq!(w.record.alternatives.first().and_then(|s| s.command.clone()).as_deref(), Some("appwiz.cpl"));
    // Mai UninstallString né QuietUninstallString nei metodi.
    let ev = synthetic_evidence();
    let quiet = r#""C:\Program Files\Esempio\uninstall.exe" /S /quiet"#;
    assert!(ev.uninstall.as_ref().is_ok_and(|u| u.entries.iter().any(|e| e.uninstall_string.is_some())));
    let s = view(&kb, &["programmi/programmi.albero"], &ev);
    for p in [&v.p, &s.p] {
        for x in &p.items {
            let r = &x.record;
            assert_eq!(r.state, State::YouDecide);
            for st in r.preferred.iter().chain(&r.alternatives) {
                for lit in st.command.iter().chain(&st.uri) {
                    assert!(!lit.to_lowercase().contains("uninstall.exe") && !lit.contains(quiet) && !lit.to_lowercase().contains("msiexec"), "{}: {lit}", x.name);
                }
            }
            if r.action == Action::Uninstall {
                assert_eq!(r.estimate, Estimate::NotEstimable(NotEstimable::ExternalProcedure));
                assert_eq!(r.reversibility, "reversibilita-disinstallazione");
                assert_eq!(r.risks, vec!["funzione-programma-rischio-1".to_owned()]);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Possibili residui

#[test]
fn residues_are_unclaimed_folders_with_their_date() {
    let kb = kb();
    let v = view(&kb, &["programmi/programmi.albero"], &synthetic_evidence());
    let t = &v.tree;
    let got: BTreeMap<String, (ResidueBase, Option<i64>)> = v.p.residues.iter().map(|r| (t.path_lossy(r.node), (r.base, r.last_modified))).collect();
    let want: BTreeMap<String, (ResidueBase, Option<i64>)> = [
        (r"C:\Users\demo\AppData\Local\Orfana", (ResidueBase::Local, Some(133_957_584_000_000_000))),
        (r"C:\Users\demo\AppData\Local\Vuota", (ResidueBase::Local, None)),
        (r"C:\Users\demo\AppData\LocalLow\Studio Abbandonato", (ResidueBase::LocalLow, Some(133_223_076_000_000_000))),
        (r"C:\Users\demo\AppData\Roaming\Orfana Roaming", (ResidueBase::Roaming, Some(134_152_200_000_000_000))),
        (r"C:\ProgramData\Residuo Di Sistema", (ResidueBase::ProgramData, Some(133_143_516_000_000_000))),
        (r"C:\ProgramData\Negata", (ResidueBase::ProgramData, None)),
    ]
    .into_iter()
    .map(|(p, x)| (p.to_owned(), x))
    .collect();
    assert_eq!(got, want);
    for r in &v.p.residues {
        let f = &r.record.identity.0[0];
        match r.last_modified {
            Some(d) => assert_eq!((f.key.as_str(), f.provenance, f.values.clone()), ("fatto-residuo-possibile", Provenance::Measured, vec![Value::Time(d)])),
            None => assert_eq!((f.key.as_str(), f.provenance), ("fatto-residuo-possibile-senza-data", Provenance::Unavailable)),
        }
        assert_eq!(r.record.reliability, Criterion::NameOnly);
    }
    // Rivendicate: dalla cartella di un programma (Programs), da una regola (Temp, BraveSoftware, Packages),
    // dalle parole di un nome (editore, una parola del nome del programma, parte nome del PFN).
    for claimed in ["Programs", "Temp", "BraveSoftware", "Packages", "Esempio Software Srl", "Fantasma", "Utente", "Microsoft.WindowsTerminal"] {
        assert!(!got.contains_key(&format!(r"C:\Users\demo\AppData\Local\{claimed}")), "{claimed}");
    }
    check_records(t, &v.cls, &v.p);
}

#[test]
fn residues_on_this_pc_fixture() {
    let kb = kb();
    let v = view(&kb, &["profilo.albero"], &pc_evidence());
    let t = &v.tree;
    let names: HashSet<String> = v.p.residues.iter().map(|r| t.path_lossy(r.node)).collect();
    // CapCut, Autodesk, NVIDIA e Steam hanno un programma installato con quel nome (CapCut anche la cartella
    // dichiarata); npm-cache, Packages e Temp hanno una regola.
    for claimed in ["CapCut", "Autodesk", "NVIDIA", "Steam", "npm-cache", "Packages", "Temp"] {
        assert!(!names.contains(&format!(r"C:\Users\demo\AppData\Local\{claimed}")), "{claimed}");
    }
    // Esempi senza alcun programma corrispondente nella fixture: possibili residui.
    for maybe in ["Local\\VecchioStrumento", "Local\\AppDismessa"] {
        assert!(names.contains(&format!(r"C:\Users\demo\AppData\{maybe}")), "{maybe}");
    }
    check_records(t, &v.cls, &v.p);
}

#[test]
fn residues_compare_the_words_of_one_installed_name() {
    // Falsi positivi del PC vero (14/10/2026): cartelle di programmi installati con un nome un po' diverso.
    // Una cartella è spiegata se tutte le sue parole stanno nel nome (o nell'editore) di uno stesso programma.
    let d = |m: i64| NodeData { flags: Flags::DIR, mtime: m, ..Default::default() };
    let f = |m: i64| NodeData { alloc: 4096, logical: 100, mtime: m, ..Default::default() };
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let users = t.push(Tree::ROOT, &w("Users"), d(0));
    let u = t.push(users, &w("demo"), d(0));
    let appdata = t.push(u, &w("AppData"), d(0));
    let local = t.push(appdata, &w("Local"), d(0));
    let roaming = t.push(appdata, &w("Roaming"), d(0));
    t.push(Tree::ROOT, &w("ProgramData"), d(0));
    for (base, name) in [(local, "Epic Games"), (roaming, "Thunderbird"), (roaming, "Code"), (local, "CopperKite"), (local, "Mozilla Pen"), (roaming, "VecchioEditor"), (local, "Epic Games Store Data")] {
        let x = t.push(base, &w(name), d(133_000_000_000_000_000));
        t.push(x, &w("dati.bin"), f(133_000_000_000_000_000));
    }
    t.aggregate();
    t.seal();
    let mut ev = Evidence::empty("C:\\");
    ev.folders = Evidence::default_folders("C:\\", "demo");
    let entry = |name: &str, publisher: Option<&str>| UninstallEntry {
        hive: Hive::Machine,
        key: name.into(),
        display_name: name.into(),
        version: None,
        publisher: publisher.map(str::to_owned),
        install_location: None,
        uninstall_string: None,
        display_icon: None,
        install_date: None,
        estimated_kb: None,
        system_component: false,
    };
    ev.uninstall = Ok(Uninstall {
        entries: vec![
            entry("Epic Games Launcher", None),
            entry("Mozilla Thunderbird (x64 it)", Some("Mozilla")),
            entry("Microsoft Visual Studio Code", Some("Microsoft Corporation")),
            entry("Epic Pen", None),
        ],
        skipped: Vec::new(),
    });
    ev.steam = Ok(extract::Steam {
        root: r"C:\Program Files (x86)\Steam".into(),
        libraries: Vec::new(),
        apps: vec![extract::SteamApp {
            appid: 3_999_960,
            name: "Copper Kite".into(),
            installdir: "Copper Kite".into(),
            library: r"C:\Program Files (x86)\Steam".into(),
            size_on_disk: None,
            last_played: None,
        }],
        local_last_played: Vec::new(),
        skipped: Vec::new(),
    });
    let kb = kb();
    let cls = classify(&kb, &t, &ev, TODAY);
    let p = programs::build(&t, &ev, &kb, &cls);
    let residues: BTreeMap<String, ResidueBase> = p.residues.iter().map(|r| (t.name_lossy(r.node), r.base)).collect();
    // Spiegate: "Epic Games Launcher", "Mozilla Thunderbird (x64 it)", "Microsoft Visual Studio Code", il gioco
    // di Steam scritto in CamelCase.
    for claimed in ["Epic Games", "Thunderbird", "Code", "CopperKite"] {
        assert!(!residues.contains_key(claimed), "{claimed}: rivendicata per nome");
    }
    // Non spiegate: una parola che nessun programma ha, parole di due programmi diversi ("Mozilla" ed "Epic Pen").
    let want: BTreeMap<String, ResidueBase> =
        [("Mozilla Pen", ResidueBase::Local), ("VecchioEditor", ResidueBase::Roaming), ("Epic Games Store Data", ResidueBase::Local)].into_iter().map(|(n, b)| (n.to_owned(), b)).collect();
    assert_eq!(residues, want);
    check_records(&t, &cls, &p);
}

// ---------------------------------------------------------------------------------------------
// Alberi e prove casuali (come tests/proprieta.rs)

const NAMES: [&str; 12] = ["Alfa", "Beta Tools", "Gamma", "Delta Studio", "Epsilon", "Zeta", "Eta Labs", "Theta", "Iota", "Kappa", "Lambda", "Mu"];

/// Albero casuale: programmi in Program Files (anche annidati), cartelle con node_modules e lockfile,
/// cartelle di primo livello nelle quattro basi, con date; le prove dichiarano cartelle a caso (anche la
/// stessa, annidate, assenti, in HKCU, nascoste).
fn random_case(seed: u64) -> (Tree, Evidence) {
    let mut rng = seed | 1;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let d = |m: i64| NodeData { flags: Flags::DIR, mtime: m, ..Default::default() };
    let f = |n: u64, m: i64| NodeData { alloc: n, logical: n, mtime: m, ..Default::default() };
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let pf = t.push(Tree::ROOT, &w("Program Files"), d(0));
    let mut dirs: Vec<(u32, String)> = Vec::new();
    for k in 0..(3 + next() % 6) {
        let name = format!("{} {k}", NAMES[(next() % 12) as usize]);
        let a = t.push(pf, &w(&name), d(0));
        t.push(a, &w("app.exe"), f(next() % (1 << 24), 0));
        dirs.push((a, format!(r"C:\Program Files\{name}")));
        if next() % 2 == 0 {
            let s = t.push(a, &w("sub"), d(0));
            t.push(s, &w("lib.dll"), f(next() % (1 << 20), 0));
            dirs.push((s, format!(r"C:\Program Files\{name}\sub")));
        }
        if next() % 3 == 0 {
            t.push(a, &w("package-lock.json"), f(4096, 0));
            let nm = t.push(a, &w("node_modules"), d(0));
            t.push(nm, &w("x.js"), f(next() % (1 << 22), 0));
        }
    }
    let users = t.push(Tree::ROOT, &w("Users"), d(0));
    let u = t.push(users, &w("demo"), d(0));
    let appdata = t.push(u, &w("AppData"), d(0));
    let mut bases = Vec::new();
    for name in ["Local", "Roaming", "LocalLow"] {
        bases.push(t.push(appdata, &w(name), d(0)));
    }
    bases.push(t.push(Tree::ROOT, &w("ProgramData"), d(0)));
    for &b in &bases {
        for k in 0..(next() % 5) {
            let name = if next() % 2 == 0 { NAMES[(next() % 12) as usize].to_owned() } else { format!("Cartella{k}") };
            if t.children(b).any(|c| t.name_lossy(c).eq_ignore_ascii_case(&name)) {
                continue;
            }
            let x = t.push(b, &w(&name), d((next() % 1000) as i64));
            t.push(x, &w("dati.bin"), f(next() % (1 << 20), (next() % 1000) as i64));
            if next() % 3 == 0 {
                dirs.push((x, t.path_lossy(x)));
            }
        }
    }
    let temp = t.push(bases[0], &w("Temp"), d(0));
    t.push(temp, &w("x.tmp"), f(100, 0));
    t.aggregate();
    t.seal();

    let mut ev = Evidence::empty("C:\\");
    ev.folders = Evidence::default_folders("C:\\", "demo");
    let mut entries = Vec::new();
    for k in 0..(next() % 10) {
        let name = format!("{} {}", NAMES[(next() % 12) as usize], next() % 3);
        let pick = &dirs[(next() as usize) % dirs.len().max(1)];
        let (install_location, display_icon) = match next() % 5 {
            0 => (None, None),
            1 => (None, Some(format!(r"{}\app.exe,0", pick.1))),
            2 => (Some(r"C:\Users\demo\AppData\Local".to_owned()), None),
            3 => (Some(r"D:\Altro".to_owned()), None),
            _ => (Some(pick.1.clone()), None),
        };
        entries.push(UninstallEntry {
            hive: [Hive::Machine, Hive::Machine32, Hive::User][(next() % 3) as usize],
            key: format!("k{k}"),
            display_name: name,
            version: Some("1.0".into()),
            publisher: Some(format!("{} Srl", NAMES[(next() % 12) as usize])),
            install_location,
            uninstall_string: None,
            display_icon,
            install_date: Some(format!("2025{:02}{:02}", 1 + next() % 14, 1 + next() % 31)),
            estimated_kb: Some((next() % 100_000) as u32),
            system_component: next() % 4 == 0,
        });
    }
    ev.uninstall = Ok(Uninstall { entries, skipped: Vec::new() });
    if next() % 2 == 0 {
        ev.track_progs = TrackProgs::On;
        let ua: Vec<UserAssistEntry> =
            dirs.iter().map(|(_, p)| UserAssistEntry { name: format!(r"{p}\app.exe"), runs: 1, last_run: Some((next() % 1_000_000) as i64 + 1) }).collect();
        ev.userassist = Ok(UserAssist::Entries(ua));
    } else {
        ev.track_progs = TrackProgs::Off;
        ev.userassist = Ok(UserAssist::NotTracked(TrackProgs::Off));
    }
    (t, ev)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 200, ..ProptestConfig::default() })]

    #[test]
    fn random_cases_keep_one_owner_and_no_residue_action(seed in any::<u64>()) {
        let kb = kb();
        let (t, ev) = random_case(seed);
        let cls = classify(&kb, &t, &ev, TODAY);
        let p = programs::build(&t, &ev, &kb, &cls);
        check_records(&t, &cls, &p);
        check_one_owner_per_byte(&t, &cls, &p);
        check_matches_classification(&t, &cls, &p);
        prop_assert!(p.residues.iter().all(|r| r.record.action == Action::None));
    }
}

// ---------------------------------------------------------------------------------------------
// Testi

/// Messaggi di un file Fluent: chiave -> testo (righe di continuazione comprese), senza i commenti.
fn messages(ftl: &str) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let mut cur: Option<String> = None;
    for line in ftl.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            cur = None;
            continue;
        }
        if !line.starts_with([' ', '\t']) && let Some((k, v)) = line.split_once(" =") {
            out.insert(k.trim().to_owned(), v.to_owned());
            cur = Some(k.trim().to_owned());
        } else if let Some(k) = &cur {
            out.entry(k.clone()).or_default().push_str(line);
        }
    }
    out
}

fn placeholders(text: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    let b = text.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if c == b'$' {
            let name: String = text[i + 1..].chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
            out.insert(name);
        }
    }
    out
}

fn texts() -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    (messages(quado_kb::TEXTS[0].1), messages(quado_kb::TEXTS[1].1))
}

/// Tutti i record della vista sulle fixture.
fn all_views() -> Vec<View> {
    let kb = kb();
    let pc = pc_evidence();
    let mut out: Vec<View> = ["capcut.albero", "giochi.albero", "profilo.albero", "radice.albero"].iter().map(|f| view(&kb, &[f], &pc)).collect();
    out.push(view(&kb, &["programmi/programmi.albero"], &synthetic_evidence()));
    let mut ev = synthetic_evidence();
    ev.userassist = Err(ExtractError::Io(5));
    out.push(view(&kb, &["programmi/programmi.albero"], &ev));
    ev.track_progs = TrackProgs::Absent;
    out.push(view(&kb, &["programmi/programmi.albero"], &ev));
    out
}

fn record_keys(r: &Record) -> Vec<String> {
    r.identity.0.iter().map(|f| f.key.clone()).chain(r.text_keys().map(str::to_owned)).collect()
}

#[test]
fn every_key_of_the_view_exists_in_both_languages() {
    let (it, en) = texts();
    let mut keys: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // Chiavi scritte nel sorgente del modulo: ogni stringa che comincia con un prefisso di testo.
    let src = include_str!("../src/programs.rs");
    for part in src.split('"').skip(1).step_by(2) {
        if ["fatto-", "funzione-", "reversibilita-", "errore-", "verifica-"].iter().any(|p| part.starts_with(p)) && !part.contains(' ') {
            keys.insert(part.to_owned());
        }
    }
    assert!(keys.len() >= 40, "chiavi trovate nel sorgente: {}", keys.len());
    // Chiavi dei record prodotti sulle fixture e titoli delle due funzioni.
    for v in all_views() {
        for r in v.p.items.iter().map(|x| &x.record).chain(v.p.residues.iter().map(|x| &x.record)) {
            keys.extend(record_keys(r));
        }
    }
    for f in [Function::Program, Function::PossibleResidue] {
        keys.insert(format!("funzione-{}", f.code()));
        keys.insert(format!("funzione-{}-descrizione", f.code()));
    }
    let mut problems = Vec::new();
    for k in &keys {
        match (it.get(k), en.get(k)) {
            (Some(a), Some(b)) => {
                if placeholders(a) != placeholders(b) {
                    problems.push(format!("{k}: segnaposto diversi"));
                }
            }
            (a, b) => problems.push(format!("{k}: it {} en {}", a.is_some(), b.is_some())),
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // In it.ftl ogni chiave della sezione ha sopra un commento di contesto.
    let src_it = quado_kb::TEXTS[0].1;
    let lines: Vec<&str> = src_it.lines().collect();
    let mine = ["funzione-programma", "funzione-possibile", "fatto-uso-", "fatto-residuo-"];
    for k in keys.iter().filter(|k| mine.iter().any(|p| k.starts_with(p))) {
        let at = lines.iter().position(|l| l.starts_with(&format!("{k} ="))).expect("chiave");
        assert!(at > 0 && lines[at - 1].starts_with("# "), "{k}: manca il commento di contesto");
    }
}

/// Parole vietate sull'uso dei programmi (docs/testi.md §3), senza distinzione di maiuscole.
const FORBIDDEN: [&str; 8] = ["inutilizzato", "mai usato", "non usato da", "sicuro da disinstallare", "unused", "never used", "not used", "safe to uninstall"];


#[test]
fn no_catalog_string_and_no_record_uses_forbidden_words() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut problems = Vec::new();
    for file in ["crates/quado-kb/regole/it.ftl", "crates/quado-kb/regole/en.ftl", "crates/quado-ui/lingue/it.ftl", "crates/quado-ui/lingue/en.ftl"] {
        let path = crates.join("..").join(file);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{file}: {e}"));
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with('#') {
                continue;
            }
            let low = line.to_lowercase();
            for bad in FORBIDDEN {
                if low.contains(bad) {
                    problems.push(format!("{file}:{}: «{bad}»", n + 1));
                }
            }
        }
    }
    // Record: chiavi, testi delle chiavi e valori di testo.
    let (it, en) = texts();
    for v in all_views() {
        for r in v.p.items.iter().map(|x| &x.record).chain(v.p.residues.iter().map(|x| &x.record)) {
            let mut strings: Vec<String> = record_keys(r);
            for k in record_keys(r) {
                strings.extend(it.get(&k).cloned());
                strings.extend(en.get(&k).cloned());
            }
            for f in &r.identity.0 {
                for val in &f.values {
                    if let Value::Text(s) = val {
                        strings.push(s.clone());
                    }
                }
            }
            for s in strings {
                let low = s.to_lowercase();
                for bad in FORBIDDEN {
                    if low.contains(bad) {
                        problems.push(format!("record {}: «{bad}» in {s}", r.origin_id()));
                    }
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

// ---------------------------------------------------------------------------------------------
// Prestazioni e PC vero

/// Albero sintetico grande: `n` nodi in cartelle di programmi, profili e dati.
fn big_tree(n: usize) -> Tree {
    let d = NodeData { flags: Flags::DIR, ..Default::default() };
    let mut t = Tree::with_root(&w("C:\\"), n);
    let pf = t.push(Tree::ROOT, &w("Program Files"), d);
    let users = t.push(Tree::ROOT, &w("Users"), d);
    let u = t.push(users, &w("demo"), d);
    let appdata = t.push(u, &w("AppData"), d);
    let local = t.push(appdata, &w("Local"), d);
    let mut parents = Vec::new();
    for k in 0..300 {
        parents.push(t.push(pf, &w(&format!("App {k}")), d));
        parents.push(t.push(local, &w(&format!("Dati {k}")), d));
    }
    let mut i = 0usize;
    while t.len() < n {
        let p = parents[i % parents.len()];
        if i.is_multiple_of(50) {
            let s = t.push(p, &w(&format!("s{i}")), d);
            parents.push(s);
        } else {
            t.push(p, &w(&format!("f{i}.bin")), NodeData { alloc: 4096, logical: 4000, mtime: i as i64, ..Default::default() });
        }
        i += 1;
    }
    t.aggregate();
    t.seal();
    t
}

#[test]
#[ignore = "misura dei tempi su 3,4 milioni di nodi: cargo test --release -p quado-kb --test programmi -- --ignored --nocapture tempi"]
fn tempi_su_tre_milioni_di_nodi() {
    let kb = kb();
    let t = big_tree(3_400_000);
    let mut ev = Evidence::empty("C:\\");
    ev.folders = Evidence::default_folders("C:\\", "demo");
    let entries = (0..300)
        .map(|k| UninstallEntry {
            hive: Hive::Machine,
            key: format!("k{k}"),
            display_name: format!("App {k}"),
            version: None,
            publisher: None,
            install_location: Some(format!(r"C:\Program Files\App {k}")),
            uninstall_string: None,
            display_icon: None,
            install_date: None,
            estimated_kb: None,
            system_component: false,
        })
        .collect();
    ev.uninstall = Ok(Uninstall { entries, skipped: Vec::new() });
    let cls = classify(&kb, &t, &ev, TODAY);
    let start = std::time::Instant::now();
    let p = programs::build(&t, &ev, &kb, &cls);
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    println!("{} nodi, {} programmi, {} possibili residui: {ms:.1} ms", t.len(), p.items.len(), p.residues.len());
}

#[cfg(windows)]
#[test]
#[ignore = "legge il registro e i nomi delle cartelle di questo PC: controllo manuale con data"]
fn conteggi_su_questo_pc() {
    let ev = quado_kb::win::gather("C:\\", None);
    // Albero con i soli nomi del primo livello delle quattro basi: niente scansione, niente date.
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let mut examined = 0;
    for base in [&ev.folders.local, &ev.folders.roaming, &ev.folders.local_low, &ev.folders.program_data].into_iter().flatten() {
        let rest = base.strip_prefix("C:\\").expect("percorso su C:");
        let mut at = Tree::ROOT;
        for seg in rest.split('\\') {
            at = engine::child(&t, at, seg).unwrap_or_else(|| t.push(at, &w(seg), NodeData { flags: Flags::DIR, ..Default::default() }));
        }
        if let Ok(rd) = std::fs::read_dir(base) {
            for e in rd.flatten() {
                if e.file_type().is_ok_and(|ft| ft.is_dir() && !ft.is_symlink()) {
                    t.push(at, &w(&e.file_name().to_string_lossy()), NodeData { flags: Flags::DIR, ..Default::default() });
                    examined += 1;
                }
            }
        }
    }
    t.aggregate();
    t.seal();
    let kb = kb();
    let cls = classify(&kb, &t, &ev, TODAY);
    let p = programs::build(&t, &ev, &kb, &cls);
    let n = |f: &dyn Fn(&Program) -> bool| p.items.iter().filter(|x| f(x)).count();
    println!("voci di disinstallazione: {} (nascoste {})", n(&|x| matches!(x.source, ProgramSource::Uninstall { .. })), n(&|x| x.hidden));
    for h in [Hive::Machine, Hive::Machine32, Hive::User] {
        println!("  {h:?}: {}", n(&|x| matches!(&x.source, ProgramSource::Uninstall { hive, .. } if *hive == h)));
    }
    let in_view = n(&|x| matches!(x.source, ProgramSource::Msix { .. }));
    match &ev.msix {
        Ok(m) => println!("msix: {} pacchetti, {} non framework, {in_view} nella vista", m.len(), m.iter().filter(|x| !x.framework).count()),
        Err(e) => println!("msix: {e:?}"),
    }
    println!("steam: {}; epic: {}", n(&|x| matches!(x.source, ProgramSource::Steam { .. })), n(&|x| matches!(x.source, ProgramSource::Epic { .. })));
    println!(
        "posizione: cartella {}, indizio {}, non dichiarata {}, non plausibile {}",
        n(&|x| matches!(x.location, Location::Folder { evidence: PathEvidence::InstallLocation, .. })),
        n(&|x| matches!(x.location, Location::Folder { evidence: PathEvidence::UninstallHint, .. })),
        n(&|x| x.location == Location::NotDeclared),
        n(&|x| matches!(x.location, Location::Implausible { .. }))
    );
    println!(
        "metodi: impostazioni {}, pagina MSIX {}, steam {}, epic {}, nessuno {}",
        n(&|x| matches!(x.uninstall, Some(UninstallVia::Settings { .. }))),
        n(&|x| matches!(x.uninstall, Some(UninstallVia::SettingsApp { .. }))),
        n(&|x| matches!(x.uninstall, Some(UninstallVia::Steam { .. }))),
        n(&|x| matches!(x.uninstall, Some(UninstallVia::EpicLibrary))),
        n(&|x| x.uninstall.is_none())
    );
    println!("tracciamento: {:?}; uso UserAssist: {}", p.tracking, n(&|x| matches!(x.usage, Usage::UserAssist(Some(_)))));
    println!("errori: {:?}", p.errors);
    println!("cartelle di primo livello esaminate: {examined}; possibili residui: {}", p.residues.len());
    if std::env::var_os("QUADO_STAMPA_RESIDUI").is_some() {
        for r in &p.residues {
            println!("  {:?} {}", r.base, t.name_lossy(r.node));
        }
    }
}

/// Le voci "Steam App N" del registro e i manifest di Steam sono lo stesso gioco: una riga sola, quella
/// di Steam, con il fatto della voce di disinstallazione: lo stesso gioco non compare in due righe.
#[test]
fn steam_uninstall_entries_join_their_game() {
    for v in all_views() {
        let steam: HashSet<u32> = v.p.items.iter().filter_map(|x| if let ProgramSource::Steam { appid } = x.source { Some(appid) } else { None }).collect();
        for x in &v.p.items {
            if let ProgramSource::Uninstall { key, .. } = &x.source
                && let Some(id) = key.strip_prefix("Steam App ").and_then(|s| s.parse::<u32>().ok())
            {
                assert!(!steam.contains(&id), "{key}: il gioco compare due volte");
            }
        }
    }
    // Hollow Peaks dalla fixture di Steam più la voce che Steam scrive nel registro: una riga, con il fatto della
    // voce e la sua data di installazione.
    let mut ev = pc_evidence();
    if let Ok(u) = &mut ev.uninstall {
        u.entries.push(UninstallEntry {
            hive: Hive::Machine,
            key: "Steam App 3999910".into(),
            display_name: "Hollow Peaks".into(),
            version: None,
            publisher: Some("Studio di Prova".into()),
            install_location: Some(r"C:\Program Files (x86)\Steam\steamapps\common\hollow peaks".into()),
            uninstall_string: Some(r#""C:\Program Files (x86)\Steam\steam.exe" steam://uninstall/3999910"#.into()),
            display_icon: None,
            install_date: Some("20250101".into()),
            estimated_kb: None,
            system_component: false,
        });
    }
    let v = view(&kb(), &["giochi.albero"], &ev);
    let game: Vec<&Program> = v.p.items.iter().filter(|x| x.name == "Hollow Peaks").collect();
    assert_eq!(game.len(), 1, "una riga sola per Hollow Peaks");
    assert!(matches!(game[0].source, ProgramSource::Steam { appid: 3_999_910 }));
    assert!(game[0].record.identity.0.iter().any(|f| f.key == "fatto-programma-installato"), "il fatto della voce passa al gioco");
    assert!(game[0].install_date.is_some());
}
