// SPDX-License-Identifier: GPL-3.0-or-later
//! Conferma del piano (03 F-AC-2, F-AC-4, F-AC-7; 05 D7; 06 §5.2) con letture simulate (`fake`):
//! `proptest` su alberi casuali con radici protette e selezioni casuali; l'eccezione di F-AC-4 (03 §6.1,
//! caso CapCut) con un caso positivo e un caso negativo per ciascun controllo mancante; gli altri motivi
//! di rifiuto. I test di compilazione del piano sono nella documentazione di `plan.rs`. Nessun file vero.

use proptest::prelude::*;
use quado_actions::fake::{self, FakeProbe};
use quado_actions::plan::{Block, CapacityCheck, Rifiuto, Scelta, Simulazione, check, conferma};
use quado_actions::probe::VolumeBin;
use quado_actions::restart::RmError;
use quado_actions::simulate::{FixedReason, Request, RequestItem, SimProgress, simulate};
use quado_model::intervention::{BinCount, ProtectedKind, ProtectedRoot, ReferenceKind};
use quado_model::record::{Action, CheckOutcome, Expiry, Record, State, UnknownReason};

use ProtectedKind::*;

fn sim_of(probe: &FakeProbe, items: Vec<RequestItem>, record: Record, protected: Vec<ProtectedRoot>) -> Simulazione {
    simulate(Request { items, record, protected }, probe, &SimProgress::default()).expect("simulazione")
}

fn item(path: &str) -> RequestItem {
    RequestItem { path: path.to_owned(), fixed: None }
}

/// Tutti gli elementi che si possono scegliere, con tutti i controlli manuali spuntati.
fn ticked(sim: &Simulazione) -> Scelta {
    let mut s = sim.default_choice();
    s.selected = sim.selectable();
    s.ticked_rule.fill(true);
    s.ticked_items.iter_mut().for_each(|t| t.fill(true));
    s
}

/// Tutti gli elementi scelti, tutti i controlli manuali spuntati.
fn everything(sim: &Simulazione) -> Scelta {
    let mut s = ticked(sim);
    s.selected.fill(true);
    s
}

// ---------------------------------------------------------------------------------------------
// Selezioni casuali con elementi protetti (06 §5.2)

const ROOTS: [(&str, ProtectedKind); 9] = [
    ("C:\\Windows", WindowsDir),
    ("C:\\Windows\\WinSxS", WinSxS),
    ("C:\\Windows\\Installer", WindowsInstaller),
    ("C:\\System Volume Information", SystemVolumeInformation),
    ("C:\\pagefile.sys", PagingFile),
    ("C:\\hiberfil.sys", Hibernation),
    ("C:\\swapfile.sys", SwapFile),
    ("C:\\Program Files\\Él Programma", RegisteredProgram),
    ("C:\\Users\\u\\AppData\\Local\\CapCut", RegisteredProgram),
];

/// Cartelle in cui l'albero casuale mette i suoi file.
const DIRS: [&str; 15] = [
    "C:\\Windows\\System32",
    "C:\\Windows\\WinSxS\\amd64_prova",
    "C:\\Windows\\Installer",
    "C:\\System Volume Information",
    "C:\\Program Files\\Él Programma",
    "C:\\Program Files\\Él Programma\\dati",
    "C:\\Program Files\\Él Programma 2",
    "C:\\Program Files\\Altro",
    "C:\\Users\\u\\AppData\\Local\\CapCut\\Apps\\9.1.0",
    "C:\\Users\\u\\AppData\\Local\\CapCut\\Apps\\9.2.0",
    "C:\\Users\\u\\AppData\\Local\\CapCut\\User Data\\Projects",
    "C:\\Users\\u\\AppData\\Local\\Temp",
    "C:\\Users\\u\\Documents",
    "C:\\Users\\u\\Downloads",
    "C:\\Windows2",
];

/// Elementi proposti: radici, elementi dentro, contenitori e elementi fuori (anche con un nome che
/// comincia come una radice).
const CANDIDATES: [&str; 26] = [
    "C:\\",
    "C:\\Windows",
    "C:\\Windows\\System32",
    "C:\\Windows\\WinSxS",
    "C:\\Windows\\WinSxS\\amd64_prova",
    "C:\\Windows\\Installer",
    "C:\\System Volume Information",
    "C:\\pagefile.sys",
    "C:\\hiberfil.sys",
    "C:\\swapfile.sys",
    "C:\\Program Files",
    "C:\\Program Files\\Él Programma",
    "C:\\Program Files\\Él Programma\\dati",
    "C:\\Program Files\\Él Programma 2",
    "C:\\Program Files\\Altro",
    "C:\\Users",
    "C:\\Users\\u\\AppData",
    "C:\\Users\\u\\AppData\\Local\\CapCut",
    "C:\\Users\\u\\AppData\\Local\\CapCut\\Apps",
    "C:\\Users\\u\\AppData\\Local\\CapCut\\Apps\\9.1.0",
    "C:\\Users\\u\\AppData\\Local\\CapCut\\User Data\\Projects",
    "C:\\Users\\u\\AppData\\Local\\Temp",
    "C:\\Users\\u\\Documents",
    "C:\\Users\\u\\Downloads",
    "C:\\Windows2",
    "C:\\Users\\u\\AppData\\Local\\CapCut2",
];

fn roots() -> Vec<ProtectedRoot> {
    ROOTS.iter().map(|(p, k)| ProtectedRoot { path: (*p).to_owned(), kind: *k }).collect()
}

/// Lo stesso percorso scritto in un altro modo: maiuscole, `/`, `\\?\`, `\` finale.
fn variant(path: &str, v: u8) -> String {
    match v {
        0 => path.to_owned(),
        1 => path.to_uppercase(),
        2 => path.to_lowercase(),
        3 => path.replace('\\', "/"),
        4 => format!("\\\\?\\{path}"),
        _ if path.ends_with('\\') => path.to_owned(),
        _ => format!("{path}\\"),
    }
}

/// Oracolo indipendente dalla simulazione: l'elemento è protetto (uguale, dentro o contenitore di una
/// radice) e, se lo è, se l'eccezione di 03 §6.1 può valere (solo radici di programmi registrati, solo
/// strettamente dentro, regola Da verificare con l'eccezione).
fn oracle(path: &str, exception: bool, to_verify: bool) -> (bool, bool) {
    let p = path.to_lowercase();
    let (mut protected, mut only_inside_programs) = (false, true);
    for (root, kind) in ROOTS {
        let r = root.to_lowercase();
        let equal = p == r;
        let inside = p.starts_with(&format!("{r}\\"));
        let contains = if p.ends_with('\\') { r.starts_with(&p) } else { r.starts_with(&format!("{p}\\")) };
        if equal || inside || contains {
            protected = true;
            if !(kind == RegisteredProgram && inside) {
                only_inside_programs = false;
            }
        }
    }
    (protected, protected && only_inside_programs && exception && to_verify)
}

fn random_world(files: &[(usize, u64)]) -> FakeProbe {
    let mut p = FakeProbe::new().volume("C:\\");
    for d in DIRS {
        p = p.dir(d);
    }
    for f in ["C:\\pagefile.sys", "C:\\hiberfil.sys", "C:\\swapfile.sys"] {
        p = p.file(f, 1 << 20, 1 << 20);
    }
    p = p.dir("C:\\Users\\u\\AppData\\Local\\CapCut2");
    for (k, (d, size)) in files.iter().enumerate() {
        let dir = DIRS[d % DIRS.len()];
        let name = if k % 5 == 0 { format!("f{k}.dll") } else { format!("f{k}.dat") };
        p = p.file(&format!("{dir}\\{name}"), *size, size.div_ceil(4_096) * 4_096);
    }
    p
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// Ogni selezione con un elemento protetto è respinta con `Blocked(_, Protected(_))`, salvo
    /// l'eccezione; nessun piano contiene un elemento protetto senza l'eccezione; gli elementi non
    /// protetti non hanno mai quel blocco.
    #[test]
    fn selections_with_protected_items_are_rejected(
        files in proptest::collection::vec((0usize..64, 0u64..(1 << 20)), 0..40),
        selected in proptest::collection::vec(any::<bool>(), CANDIDATES.len()),
        variants in proptest::collection::vec(0u8..6, CANDIDATES.len()),
        exception in any::<bool>(),
        state in 0u8..3,
    ) {
        let p = random_world(&files);
        let mut record = fake::record(match state {
            0 => State::Manageable(fake::proof()),
            1 => State::ToVerify(fake::proof()),
            _ => State::YouDecide,
        });
        record.protected_exception = exception;
        let items = CANDIDATES.iter().zip(&variants).map(|(c, v)| item(&variant(c, *v))).collect();
        let sim = sim_of(&p, items, record, roots());
        let mut scelta = everything(&sim);
        scelta.selected = selected.clone();
        let reasons = check(&sim, &scelta);
        let mut must_reject = false;
        for (i, c) in CANDIDATES.iter().enumerate() {
            let (protected, excepted) = oracle(c, exception, state == 1);
            let has_block = sim.items()[i].blocks.iter().any(|b| matches!(b, Block::Protected(_)));
            prop_assert_eq!(has_block, protected && !excepted, "{} (variante {})", c, variants[i]);
            if !selected[i] {
                continue;
            }
            let rejected = reasons.iter().any(|r| matches!(r, Rifiuto::Blocked(j, Block::Protected(_)) if *j == i));
            prop_assert_eq!(rejected, protected && !excepted, "{}", c);
            must_reject |= protected && !excepted;
        }
        match conferma(&sim, &scelta) {
            Ok(piano) => {
                prop_assert!(!must_reject);
                for it in piano.items() {
                    let i = sim.items().iter().position(|s| s.path == it.path()).expect("elemento della simulazione");
                    let (protected, excepted) = oracle(CANDIDATES[i], exception, state == 1);
                    prop_assert!(!protected || excepted, "{} nel piano", it.path());
                }
            }
            Err(r) => prop_assert_eq!(r, reasons),
        }
    }
}

#[test]
fn every_kind_of_protected_root_blocks_equal_inside_and_containing_items() {
    let p = random_world(&[]).file("C:\\Windows\\System32\\kernel32.dll", 10, 4_096);
    for (root, kind) in ROOTS {
        // Uguale, scritto in un altro modo.
        let sim = sim_of(&p, vec![item(&root.to_uppercase())], fake::record(State::YouDecide), roots());
        assert!(sim.items()[0].blocks.contains(&Block::Protected(kind)), "{root}");
        assert!(conferma(&sim, &everything(&sim)).is_err());
    }
    // Contenitore di tutte le radici.
    let sim = sim_of(&p, vec![item("C:\\")], fake::record(State::YouDecide), roots());
    for (_, kind) in ROOTS {
        assert!(sim.items()[0].blocks.contains(&Block::Protected(kind)), "{kind:?}");
    }
    // Dentro: un file di System32.
    let sim = sim_of(&p, vec![item("c:/windows/system32/KERNEL32.DLL")], fake::record(State::YouDecide), roots());
    assert!(sim.items()[0].blocks.contains(&Block::Protected(WindowsDir)));
    // Un nome che comincia come una radice non è protetto.
    let sim = sim_of(&p, vec![item("C:\\Windows2"), item("C:\\Program Files\\Él Programma 2")], fake::record(State::YouDecide), roots());
    assert!(sim.items().iter().all(|it| it.blocks.is_empty()));
    assert!(conferma(&sim, &everything(&sim)).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Eccezione di F-AC-4 (03 §6.1): versioni precedenti di CapCut

const CAPCUT: &str = "C:\\Users\\u\\AppData\\Local\\CapCut";
const OLD: &str = "C:\\Users\\u\\AppData\\Local\\CapCut\\Apps\\9.1.0.3500";
const KEPT: &str = "C:\\Users\\u\\AppData\\Local\\CapCut\\Apps\\9.3.0.3970";
const USED: &str = "C:\\Users\\u\\AppData\\Local\\CapCut\\Apps\\9.4.0.4015";
const RULE_MANUAL: &str = "regola-capcut-versioni-controllo-prova-capcut";

fn capcut_probe() -> FakeProbe {
    FakeProbe::new()
        .volume("C:\\")
        .file(&format!("{OLD}\\CapCut.exe"), 300_000_000, 300_003_328)
        .file(&format!("{OLD}\\avcodec.dll"), 50_000_000, 50_003_968)
        .file(&format!("{OLD}\\res\\a.dat"), 1_000, 4_096)
        .file(&format!("{KEPT}\\CapCut.exe"), 310_000_000, 310_001_664)
        .file(&format!("{USED}\\CapCut.exe"), 320_000_000, 320_000_000)
        .file(&format!("{CAPCUT}\\User Data\\Projects\\p.json"), 100, 4_096)
}

fn capcut_record() -> Record {
    let mut r = fake::record(State::ToVerify(fake::proof()));
    r.protected_exception = true;
    r.checks = vec![
        fake::automatic("controllo-restart-manager"),
        fake::automatic("controllo-riferimenti"),
        fake::automatic("controllo-capacita-cestino"),
        fake::manual(RULE_MANUAL),
    ];
    r
}

fn capcut_items() -> Vec<RequestItem> {
    vec![
        item(OLD),
        RequestItem { path: KEPT.into(), fixed: Some(FixedReason::KeptVersion) },
        RequestItem { path: USED.into(), fixed: Some(FixedReason::InUseVersion) },
    ]
}

fn capcut_roots(kind: ProtectedKind) -> Vec<ProtectedRoot> {
    vec![ProtectedRoot { path: CAPCUT.into(), kind }]
}

/// Simulazione del caso CapCut e motivi di rifiuto della scelta proposta con tutto spuntato; verifica
/// che `conferma` rifiuti esattamente quei motivi.
fn capcut(probe: &FakeProbe, record: Record, kind: ProtectedKind, items: Vec<RequestItem>) -> (Simulazione, Vec<Rifiuto>) {
    let sim = sim_of(probe, items, record, capcut_roots(kind));
    let mut scelta = ticked(&sim);
    scelta.selected = sim.items().iter().map(|it| it.fixed.is_none()).collect();
    let reasons = check(&sim, &scelta);
    match conferma(&sim, &scelta) {
        Ok(_) => assert!(reasons.is_empty()),
        Err(r) => assert_eq!(r, reasons),
    }
    (sim, reasons)
}

const EXCEPTION_FAILS: Rifiuto = Rifiuto::Blocked(0, Block::Protected(RegisteredProgram));

#[test]
fn exception_holds_when_every_check_is_satisfied() {
    let p = capcut_probe();
    let (sim, reasons) = capcut(&p, capcut_record(), RegisteredProgram, capcut_items());
    assert_eq!(reasons, vec![]);
    assert!(sim.items()[0].blocks.is_empty());
    // Da verificare: le caselle partono vuote; si può scegliere solo la versione da spostare.
    assert_eq!(sim.default_selection(), vec![false, false, false]);
    assert_eq!(sim.selectable(), vec![true, false, false]);
    let piano = conferma(&sim, &ticked(&sim)).expect("piano");
    assert_eq!(piano.items().iter().map(|i| i.path()).collect::<Vec<_>>(), [OLD]);
    // La stima esclude le versioni tenute.
    assert_eq!(piano.estimate().estimate, quado_model::record::Estimate::Exact(300_003_328 + 50_003_968 + 4_096));
    // Restart Manager con eseguibili e DLL, riferimenti sulla sola versione da spostare.
    assert_eq!(p.rm_calls().len(), 1);
    assert!(p.rm_calls()[0].iter().any(|f| f.ends_with("CapCut.exe")));
    assert_eq!(p.reference_calls(), vec![vec![OLD.to_owned()]]);
    let outcomes: Vec<CheckOutcome> = sim.rule_checks().iter().map(|c| c.outcome).collect();
    assert_eq!(outcomes, [CheckOutcome::Passed, CheckOutcome::Passed, CheckOutcome::Passed, CheckOutcome::ToTick]);
}

#[test]
fn exception_needs_a_rule_in_to_verify() {
    for state in [State::Manageable(fake::proof()), State::YouDecide] {
        let mut r = capcut_record();
        r.state = state;
        let (_, reasons) = capcut(&capcut_probe(), r, RegisteredProgram, capcut_items());
        assert_eq!(reasons, vec![EXCEPTION_FAILS]);
    }
}

#[test]
fn exception_needs_the_rule_to_declare_it() {
    let mut r = capcut_record();
    r.protected_exception = false;
    let (_, reasons) = capcut(&capcut_probe(), r, RegisteredProgram, capcut_items());
    assert_eq!(reasons, vec![EXCEPTION_FAILS]);
}

#[test]
fn exception_holds_only_for_registered_programs() {
    for kind in [WindowsDir, WinSxS, WindowsInstaller, SystemVolumeInformation, PagingFile, Hibernation, SwapFile] {
        let (_, reasons) = capcut(&capcut_probe(), capcut_record(), kind, capcut_items());
        assert_eq!(reasons, vec![Rifiuto::Blocked(0, Block::Protected(kind))], "{kind:?}");
    }
}

#[test]
fn exception_holds_only_strictly_inside_the_root() {
    // La radice stessa, scritta in un altro modo.
    let (_, reasons) = capcut(&capcut_probe(), capcut_record(), RegisteredProgram, vec![item(&format!("{}\\", CAPCUT.to_lowercase()))]);
    assert_eq!(reasons, vec![EXCEPTION_FAILS]);
    // Un contenitore della radice.
    let (_, reasons) = capcut(&capcut_probe(), capcut_record(), RegisteredProgram, vec![item("C:\\Users\\u\\AppData\\Local")]);
    assert_eq!(reasons, vec![EXCEPTION_FAILS]);
}

#[test]
fn exception_fails_with_a_process_using_the_version() {
    let p = capcut_probe().in_use(&format!("{OLD}\\CapCut.exe"), 1234, "CapCut");
    let (sim, reasons) = capcut(&p, capcut_record(), RegisteredProgram, capcut_items());
    assert!(matches!(&sim.items()[0].blocks[0], Block::InUse(u) if u[0].app_name == "CapCut"));
    assert_eq!(reasons.len(), 2);
    assert!(reasons.contains(&EXCEPTION_FAILS));
    assert_eq!(sim.rule_checks()[0].outcome, CheckOutcome::Failed);
}

#[test]
fn exception_fails_when_restart_manager_does_not_answer() {
    let p = capcut_probe().rm_error(&format!("{OLD}\\avcodec.dll"), RmError::Session(5));
    let (_, reasons) = capcut(&p, capcut_record(), RegisteredProgram, capcut_items());
    assert_eq!(reasons, vec![Rifiuto::Blocked(0, Block::RestartManager(RmError::Session(5))), EXCEPTION_FAILS]);
}

#[test]
fn exception_fails_with_a_reference_to_the_version() {
    let p = capcut_probe().reference(ReferenceKind::DisplayIcon, "HKCU\\...\\Uninstall\\CapCut\\DisplayIcon", &format!("{OLD}\\CapCut.exe"));
    let (sim, reasons) = capcut(&p, capcut_record(), RegisteredProgram, capcut_items());
    assert!(matches!(&reasons[0], Rifiuto::Blocked(0, Block::Referenced(r)) if r.len() == 1 && r[0].kind == ReferenceKind::DisplayIcon));
    assert_eq!(reasons[1..], [EXCEPTION_FAILS]);
    assert_eq!(sim.references().found.len(), 1);
    // Un riferimento a un'altra versione (tenuta) non blocca questa: non viene nemmeno chiesto.
    let q = capcut_probe().reference(ReferenceKind::Shortcut, "Start\\CapCut.lnk", &format!("{USED}\\CapCut.exe"));
    let (_, reasons) = capcut(&q, capcut_record(), RegisteredProgram, capcut_items());
    assert_eq!(reasons, vec![]);
}

#[test]
fn partial_references_do_not_lift_the_exception_and_are_reported() {
    // Per un utente standard le attività pianificate di sistema non sono mai leggibili: il controllo è
    // sempre parziale. F-AC-9 vuole che la conferma lo dica, non che blocchi.
    let p = capcut_probe().partial(ReferenceKind::ScheduledTask, 5);
    let (sim, reasons) = capcut(&p, capcut_record(), RegisteredProgram, capcut_items());
    assert_eq!(reasons, vec![]);
    assert!(sim.items()[0].blocks.is_empty());
    assert_eq!(sim.references().partial, vec![(ReferenceKind::ScheduledTask, 5)]);
    assert_eq!(sim.rule_checks()[1].outcome, CheckOutcome::Passed);
    // Parziale e un riferimento trovato: il riferimento blocca.
    let q = capcut_probe().partial(ReferenceKind::ScheduledTask, 5).reference(ReferenceKind::ScheduledTask, "\\CapCut\\Aggiorna", &format!("{OLD}\\CapCut.exe"));
    let (_, reasons) = capcut(&q, capcut_record(), RegisteredProgram, capcut_items());
    assert!(matches!(&reasons[0], Rifiuto::Blocked(0, Block::Referenced(_))));
    assert_eq!(reasons[1..], [EXCEPTION_FAILS]);
    // Anche per un elemento senza eccezione "parziale" è solo un'informazione.
    let q = FakeProbe::new().volume("R:\\").file("R:\\a.bin", 1, 4_096).partial(ReferenceKind::ScheduledTask, 5);
    let mut r = fake::record(State::Manageable(fake::proof()));
    r.checks = vec![fake::automatic("controllo-riferimenti")];
    let sim = sim_of(&q, vec![item("R:\\a.bin")], r, Vec::new());
    assert!(sim.items()[0].blocks.is_empty());
    assert_eq!(sim.references().partial.len(), 1);
}

#[test]
fn exception_checks_references_even_if_the_rule_does_not_ask() {
    let mut r = capcut_record();
    r.checks.retain(|c| c.key != "controllo-riferimenti");
    let p = capcut_probe().reference(ReferenceKind::RunUser, "HKCU\\...\\Run\\CapCut", &format!("{OLD}\\CapCut.exe"));
    let (_, reasons) = capcut(&p, r, RegisteredProgram, capcut_items());
    assert!(matches!(&reasons[0], Rifiuto::Blocked(0, Block::Referenced(_))));
    assert_eq!(reasons[1..], [EXCEPTION_FAILS]);
    assert_eq!(p.reference_calls(), vec![vec![OLD.to_owned()]]);
}

#[test]
fn exception_does_not_lift_the_capacity_check() {
    let p = capcut_probe().bin("C:\\", |b| b.content = Some(BinCount { items: 3, bytes: fake::CAPACITY - 100_000_000 }));
    let (sim, reasons) = capcut(&p, capcut_record(), RegisteredProgram, capcut_items());
    assert!(sim.items()[0].blocks.is_empty());
    let total = fake::CAPACITY - 100_000_000 + 350_001_000;
    assert_eq!(reasons, vec![Rifiuto::Capacity(CapacityCheck::Exceeded { capacity: fake::CAPACITY, total })]);
    assert_eq!(sim.rule_checks()[2].outcome, CheckOutcome::Failed);
}

#[test]
fn exception_needs_the_manual_check_of_the_rule() {
    let p = capcut_probe();
    let sim = sim_of(&p, capcut_items(), capcut_record(), capcut_roots(RegisteredProgram));
    assert_eq!(sim.rule_manual(), [RULE_MANUAL]);
    let scelta = Scelta { selected: sim.selectable(), ..sim.default_choice() };
    assert_eq!(check(&sim, &scelta), vec![Rifiuto::RuleNotTicked(0)]);
    assert_eq!(conferma(&sim, &scelta).err(), Some(vec![Rifiuto::RuleNotTicked(0)]));
}

#[test]
fn exception_fails_with_any_other_block() {
    let p = capcut_probe().bin("C:\\", |b| b.nuke_on_delete = true);
    let (_, reasons) = capcut(&p, capcut_record(), RegisteredProgram, capcut_items());
    assert_eq!(reasons, vec![Rifiuto::Blocked(0, Block::NukeOnDelete), EXCEPTION_FAILS]);
}

#[test]
fn exception_ends_with_its_rule() {
    let mut r = capcut_record();
    r.expiry = Expiry::Expired;
    let (_, reasons) = capcut(&capcut_probe(), r, RegisteredProgram, capcut_items());
    assert_eq!(reasons, vec![Rifiuto::NotARecycle]);
}

// ---------------------------------------------------------------------------------------------
// Altri motivi di rifiuto

fn simple(probe: &FakeProbe, paths: &[&str], record: Record) -> Simulazione {
    sim_of(probe, paths.iter().map(|p| item(p)).collect(), record, Vec::new())
}

fn manageable() -> Record {
    fake::record(State::Manageable(fake::proof()))
}

fn small_world() -> FakeProbe {
    FakeProbe::new().volume("R:\\").file("R:\\a.bin", 1_000, 4_096).file("R:\\b\\c.bin", 2_000, 4_096)
}

#[test]
fn only_records_that_propose_a_recycle_give_a_plan() {
    let p = small_world();
    let mut not_recycle = Vec::new();
    for action in [Action::Procedure, Action::Command, Action::Uninstall, Action::None] {
        let mut r = manageable();
        r.action = action;
        not_recycle.push(r);
    }
    for state in [State::Unknown(UnknownReason::NoRule), State::ManagedBy { by: "Windows".into(), procedure: true }, State::ManagedBy { by: "Windows".into(), procedure: false }] {
        not_recycle.push(fake::record(state));
    }
    let mut expired = manageable();
    expired.expiry = Expiry::Expired;
    not_recycle.push(expired);
    for r in not_recycle {
        let sim = simple(&p, &["R:\\a.bin"], r);
        assert_eq!(conferma(&sim, &ticked(&sim)).err(), Some(vec![Rifiuto::NotARecycle]));
    }
    for state in [State::Manageable(fake::proof()), State::ToVerify(fake::proof()), State::YouDecide] {
        let mut r = fake::record(state);
        r.expiry = Expiry::Announce;
        let sim = simple(&p, &["R:\\a.bin"], r);
        assert!(conferma(&sim, &ticked(&sim)).is_ok());
    }
}

#[test]
fn choices_with_the_wrong_shape_are_rejected() {
    let p = small_world().synced("R:\\b", "OneDrive");
    let mut r = manageable();
    r.checks = vec![fake::manual("regola-prova-controllo")];
    let sim = simple(&p, &["R:\\a.bin", "R:\\b"], r);
    let good = ticked(&sim);
    assert!(check(&sim, &good).is_empty());
    let mut wrong = Vec::new();
    let mut s = good.clone();
    s.selected.pop();
    wrong.push(s);
    let mut s = good.clone();
    s.ticked_rule.push(true);
    wrong.push(s);
    let mut s = good.clone();
    s.ticked_items[1].clear();
    wrong.push(s);
    let mut s = good.clone();
    s.ticked_items.push(Vec::new());
    wrong.push(s);
    wrong.push(Scelta::default());
    for s in wrong {
        assert_eq!(conferma(&sim, &s).err(), Some(vec![Rifiuto::Shape]), "{s:?}");
    }
}

#[test]
fn empty_and_fixed_choices_are_rejected() {
    let p = small_world();
    let req = vec![item("R:\\a.bin"), RequestItem { path: "R:\\b".into(), fixed: Some(FixedReason::KeptCopy) }];
    let sim = sim_of(&p, req, manageable(), Vec::new());
    let mut s = ticked(&sim);
    s.selected = vec![false, false];
    assert_eq!(check(&sim, &s), vec![Rifiuto::Empty]);
    s.selected = vec![false, true];
    assert_eq!(check(&sim, &s), vec![Rifiuto::Fixed(1)]);
    s.selected = vec![true, true];
    assert_eq!(check(&sim, &s), vec![Rifiuto::Fixed(1)]);
    s.selected = vec![true, false];
    assert!(conferma(&sim, &s).is_ok());
}

#[test]
fn unticked_manual_checks_are_rejected() {
    let p = small_world().synced("R:\\b", "Dropbox");
    let mut r = manageable();
    r.checks = vec![fake::manual("regola-prova-uno"), fake::automatic("controllo-lockfile"), fake::manual("regola-prova-due")];
    let sim = simple(&p, &["R:\\a.bin", "R:\\b\\c.bin"], r);
    assert_eq!(sim.rule_manual(), ["regola-prova-uno", "regola-prova-due"]);
    let mut s = sim.default_choice();
    assert_eq!(check(&sim, &s), vec![Rifiuto::ItemNotTicked(1, 0), Rifiuto::RuleNotTicked(0), Rifiuto::RuleNotTicked(1)]);
    s.ticked_rule = vec![true, false];
    s.ticked_items[1][0] = true;
    assert_eq!(check(&sim, &s), vec![Rifiuto::RuleNotTicked(1)]);
    // Il controllo dell'elemento non scelto non conta.
    s.ticked_rule = vec![true, true];
    s.ticked_items[1][0] = false;
    s.selected[1] = false;
    assert!(conferma(&sim, &s).is_ok());
}

#[test]
fn capacity_exceeded_or_unknown_is_rejected() {
    // 03 F-AC-7: contenuto più selezione (logica) oltre la capacità.
    let p = small_world().bin("R:\\", |b| {
        b.capacity = Some(10_000);
        b.content = Some(BinCount { items: 4, bytes: 8_000 });
    });
    let sim = simple(&p, &["R:\\a.bin", "R:\\b"], manageable());
    assert!(sim.items().iter().all(|it| it.blocks.is_empty()));
    let s = ticked(&sim);
    assert_eq!(check(&sim, &s), vec![Rifiuto::Capacity(CapacityCheck::Exceeded { capacity: 10_000, total: 11_000 })]);
    let mut one = s.clone();
    one.selected = vec![false, true];
    assert_eq!(sim.capacity(&one.selected), CapacityCheck::Fits { capacity: 10_000, total: 10_000 });
    assert!(conferma(&sim, &one).is_ok());
    // Contenuto del Cestino non letto.
    let q = small_world().bin("R:\\", |b| b.content = None);
    let sim = simple(&q, &["R:\\a.bin"], manageable());
    assert_eq!(check(&sim, &ticked(&sim)), vec![Rifiuto::Capacity(CapacityCheck::Unknown)]);
}

/// Modifica del Cestino di un volume simulato.
type BinChange = fn(&mut VolumeBin);

#[test]
fn volumes_without_a_usable_bin_block_every_item() {
    let cases: [(BinChange, Block); 3] = [
        (|b| b.has_bin = false, Block::NoRecycleBin),
        (|b| b.nuke_on_delete = true, Block::NukeOnDelete),
        (|b| b.capacity = None, Block::CapacityUnknown),
    ];
    for (f, block) in cases {
        let p = small_world().bin("R:\\", f);
        let sim = simple(&p, &["R:\\a.bin", "R:\\b"], manageable());
        for it in sim.items() {
            assert_eq!(it.blocks, vec![block.clone()]);
        }
        let reasons = check(&sim, &everything(&sim));
        assert!(reasons.contains(&Rifiuto::Blocked(0, block.clone())) && reasons.contains(&Rifiuto::Blocked(1, block.clone())));
        assert!(conferma(&sim, &everything(&sim)).is_err());
    }
}

#[test]
fn long_paths_block_the_item_and_the_folders_that_contain_them() {
    // Dalle prove su VHDX: da 260 caratteri UTF-16 la shell chiede di eliminare per sempre.
    let dir = format!("R:\\corta\\{}", "d".repeat(240));
    assert_eq!(dir.encode_utf16().count(), 249);
    let at_259 = format!("{dir}\\{}", "f".repeat(9));
    let at_260 = format!("{dir}\\{}", "f".repeat(10));
    assert_eq!((at_259.encode_utf16().count(), at_260.encode_utf16().count()), (259, 260));
    let p = small_world()
        .file(&at_259, 1, 4_096)
        .file("R:\\lunga\\x.bin", 1, 4_096)
        .file(&format!("R:\\lunga\\{}\\y.bin", "è".repeat(250)), 1, 4_096)
        .dir(&format!("R:\\negata\\{}", "n".repeat(260)))
        .denied(&format!("R:\\negata\\{}", "n".repeat(260)))
        .file(&at_260, 1, 4_096)
        // 259 unità UTF-16, 507 byte: si contano le unità, non i byte.
        .file(&format!("R:\\accenti\\{}", "è".repeat(248)), 1, 4_096);
    let items = [at_259.as_str(), at_260.as_str(), "R:\\lunga", "R:\\negata", "R:\\a.bin", "R:\\accenti"];
    let sim = simple(&p, &items, manageable());
    assert!(sim.items()[0].blocks.is_empty(), "259 caratteri: ammesso");
    assert_eq!(sim.items()[1].blocks, vec![Block::LongPath]);
    assert_eq!(sim.items()[2].blocks, vec![Block::ContainsLongPath]);
    // Anche una cartella negata con un nome lungo: il suo nome si vede.
    assert_eq!(sim.items()[3].blocks, vec![Block::ContainsLongPath]);
    assert!(sim.items()[4].blocks.is_empty());
    assert!(sim.items()[5].blocks.is_empty());
    let reasons = check(&sim, &everything(&sim));
    assert_eq!(reasons, vec![Rifiuto::Blocked(1, Block::LongPath), Rifiuto::Blocked(2, Block::ContainsLongPath), Rifiuto::Blocked(3, Block::ContainsLongPath)]);
    // La cartella che contiene il file da 260 caratteri, e lo stesso file scritto con `\\?\`.
    let sim = simple(&p, &["R:\\corta", &format!("\\\\?\\{at_260}")], manageable());
    assert_eq!(sim.items()[0].blocks, vec![Block::ContainsLongPath]);
    assert_eq!(sim.items()[1].blocks, vec![Block::LongPath]);
    // Un elemento lungo che non c'è: entrambi i motivi.
    let sim = simple(&p, &[&format!("{dir}\\{}", "m".repeat(20))], manageable());
    assert_eq!(sim.items()[0].blocks, vec![Block::Missing, Block::LongPath]);
}

#[test]
fn items_larger_than_the_bin_are_blocked() {
    // 05 "Aggiornamento dopo M1": Windows conta la logica; un elemento più grande della capacità
    // sarebbe eliminato per sempre.
    let p = small_world().file("R:\\grande.vhdx", 50_000, 4_096).bin("R:\\", |b| b.capacity = Some(10_000));
    let sim = simple(&p, &["R:\\grande.vhdx", "R:\\a.bin"], manageable());
    assert_eq!(sim.items()[0].blocks, vec![Block::LargerThanBin { logical: 50_000, capacity: 10_000 }]);
    assert!(sim.items()[1].blocks.is_empty());
    let reasons = check(&sim, &everything(&sim));
    assert_eq!(reasons[0], Rifiuto::Blocked(0, Block::LargerThanBin { logical: 50_000, capacity: 10_000 }));
    assert!(conferma(&sim, &ticked(&sim)).is_ok());
}

#[test]
fn items_on_another_volume_are_blocked() {
    // Un'altra unità.
    let p = small_world().volume("S:\\").file("S:\\x.bin", 1, 4_096);
    let sim = simple(&p, &["R:\\a.bin", "s:/x.bin", "R:\\b"], manageable());
    assert_eq!(sim.volume().root, "R:\\");
    assert_eq!(sim.items()[1].blocks, vec![Block::OtherVolume]);
    assert!(sim.items()[0].blocks.is_empty() && sim.items()[2].blocks.is_empty());
    assert_eq!(sim.default_selection(), vec![true, false, true]);
    // Un volume montato in una cartella.
    let q = small_world().volume("R:\\Montato\\").file("R:\\Montato\\y.bin", 1, 4_096);
    let sim = simple(&q, &["R:\\a.bin", "R:\\Montato\\y.bin"], manageable());
    assert_eq!(sim.items()[1].blocks, vec![Block::OtherVolume]);
    let reasons = check(&sim, &everything(&sim));
    assert_eq!(reasons, vec![Rifiuto::Blocked(1, Block::OtherVolume)]);
}

#[test]
fn unreadable_volume_and_missing_items_are_blocked() {
    let p = small_world().volume_error("R:\\", 21);
    let sim = simple(&p, &["R:\\a.bin", "R:\\b"], manageable());
    assert!(sim.items().iter().all(|it| it.blocks == vec![Block::Unreadable(21)]));
    assert_eq!(sim.capacity(&[true, true]), CapacityCheck::Unknown);
    let q = small_world().file("R:\\negato.bin", 1, 4_096).attributes_error("R:\\negato.bin", 5);
    let sim = simple(&q, &["R:\\manca.bin", "R:\\negato.bin", "R:\\a.bin"], manageable());
    assert_eq!(sim.items()[0].blocks, vec![Block::Missing]);
    assert_eq!(sim.items()[1].blocks, vec![Block::Unreadable(5)]);
    let reasons = check(&sim, &everything(&sim));
    assert_eq!(reasons, vec![Rifiuto::Blocked(0, Block::Missing), Rifiuto::Blocked(1, Block::Unreadable(5))]);
}

#[test]
fn boxes_start_empty_for_to_verify_and_for_more_than_one_item_to_decide() {
    // Valutazione dei consigli (01/11/2026): per Da verificare, e per Decidi tu con più elementi, la scelta la
    // fa l'utente; con un solo elemento il clic che ha aperto l'anteprima era già la scelta.
    let p = small_world();
    let two = [r"R:\a.bin", r"R:\b"];
    let selection = |paths: &[&str], state: State| simple(&p, paths, fake::record(state)).default_selection();
    assert_eq!(selection(&two, State::ToVerify(fake::proof())), vec![false, false]);
    assert_eq!(selection(&[r"R:\a.bin"], State::ToVerify(fake::proof())), vec![false]);
    assert_eq!(selection(&two, State::YouDecide), vec![false, false]);
    assert_eq!(selection(&[r"R:\a.bin"], State::YouDecide), vec![true]);
    assert_eq!(selection(&two, State::Manageable(fake::proof())), vec![true, true]);
    // «Seleziona tutti» sceglie quello che si può scegliere, e il piano si costruisce.
    let sim = simple(&p, &two, fake::record(State::ToVerify(fake::proof())));
    assert_eq!(sim.selectable(), vec![true, true]);
    assert_eq!(conferma(&sim, &sim.default_choice()).err(), Some(vec![Rifiuto::Empty]));
    assert!(conferma(&sim, &ticked(&sim)).is_ok());
}
