// SPDX-License-Identifier: GPL-3.0-or-later
//! Schermate di M2 disegnate fuori schermo con `egui_kittest` (06 §7, 07 §1.3 criterio 3).
//!
//! - `pseudo_language_covers_every_text`: con la pseudo-lingua ogni testo dell'interfaccia è
//!   pseudo-localizzato (nessuna stringa scritta nel codice) e nessun testo tradotto è tagliato.
//! - `snapshots` (ignorato di default, lo lancia `cargo xtask immagini`): PNG di ogni schermata nei due
//!   temi a 100%, 150% e 200%, in italiano e in inglese, in `target/m2-immagini`. Tre variabili d'ambiente
//!   restringono o cambiano il giro: `QUADO_IMMAGINI` (schermate, `panoramica,esplora`), `QUADO_LINGUE`
//!   (lingue, `en`) e `QUADO_REGIONE` (area geografica di numeri e date: `it-IT`, quella dei test, se manca;
//!   `en-US` per `4.00 KB` e `11/1/2026`). Per le immagini del README in inglese:
//!   `QUADO_IMMAGINI=panoramica,esplora,spostamento QUADO_LINGUE=en QUADO_REGIONE=en-US cargo test -p quado-ui --test screens -- --ignored snapshots`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use egui::epaint::Shape;
use egui_kittest::Harness;
use quado_model::Tree;
use quado_scan_win::Progress;
use quado_scan_win::system::RegionFormat;
use quado_scan_win::volumes::{DriveKind, Volume};
use quado_ui::app::data::ScanJob;
use quado_ui::app::{App, Env, Screen, synthetic};
use quado_ui::i18n::PSEUDO;
use quado_ui::tokens::ThemeChoice;

const SIZE: [f32; 2] = [1600.0, 1000.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shot {
    Volumes,
    Scanning,
    Overview,
    Interrupted,
    Explore,
    Search,
    BigFiles,
    Settings,
    /// Esplora su `CapCut\Apps` con una versione precedente selezionata: record Da verificare (M3).
    Versions,
    System,
    Rules,
    /// Lente Download su `Downloads` (M4).
    Download,
    /// Vista Programmi (M4).
    Programs,
    /// Panoramica con il testo al 150% (Impostazioni › Dimensione del testo).
    OverviewText150,
    /// Esplora sulla radice con densità compatta, barra laterale compressa e colori per le discromatopsie.
    ExploreCompact,
    /// Dialogo di conferma dello spostamento delle versioni precedenti di CapCut, sopra Esplora (letture
    /// simulate: niente legge il disco vero).
    Dialog,
    /// Palette dei comandi (Ctrl+K) sopra la Panoramica, con un testo che trova CapCut.
    Palette,
    /// Vista Interventi con lo storico di un registro di prova (uno spostamento e uno svuotamento del Cestino).
    Interventions,
    /// Dialogo di disinstallazione di CapCut sopra la vista Programmi, con i tuoi file e la casella.
    Uninstall,
    /// Anteprima dello spostamento delle versioni precedenti di CapCut con «Seleziona tutti» premuto: ipotesi e
    /// rischi del consiglio Da verificare prima dell'elenco e del pulsante, in una finestra alta che mostra il
    /// dialogo intero (immagine del README).
    Move,
}

impl Shot {
    const ALL: [Self; 20] = [
        Self::Volumes,
        Self::Scanning,
        Self::Overview,
        Self::Interrupted,
        Self::Explore,
        Self::Search,
        Self::BigFiles,
        Self::Settings,
        Self::Versions,
        Self::System,
        Self::Rules,
        Self::Download,
        Self::Programs,
        Self::OverviewText150,
        Self::ExploreCompact,
        Self::Dialog,
        Self::Palette,
        Self::Interventions,
        Self::Uninstall,
        Self::Move,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Volumes => "volumi",
            Self::Scanning => "scansione",
            Self::Overview => "panoramica",
            Self::Interrupted => "panoramica-interrotta",
            Self::Explore => "esplora",
            Self::Search => "ricerca",
            Self::BigFiles => "file-grandi",
            Self::Settings => "impostazioni",
            Self::Versions => "esplora-versioni",
            Self::System => "sistema",
            Self::Rules => "regole",
            Self::Download => "download",
            Self::Programs => "programmi",
            Self::OverviewText150 => "panoramica-testo-150",
            Self::ExploreCompact => "esplora-compatta",
            Self::Dialog => "dialogo",
            Self::Palette => "palette",
            Self::Interventions => "interventi",
            Self::Uninstall => "disinstalla",
            Self::Move => "spostamento",
        }
    }

    /// Finestra della schermata: più alta per l'anteprima dello spostamento, che così non scorre.
    fn size(self) -> [f32; 2] {
        if self == Self::Move { [1600.0, 1440.0] } else { SIZE }
    }
}

fn volumes() -> Vec<Volume> {
    use quado_model::Space;
    let gib = 1u64 << 30;
    vec![
        Volume {
            guid_path: String::new(),
            paths: vec!["C:\\".into()],
            label: String::new(),
            file_system: "NTFS".into(),
            kind: DriveKind::Fixed,
            space: Some(Space { capacity: 476 * gib, free: 118 * gib, free_to_caller: 118 * gib }),
            is_system: true,
        },
        Volume {
            guid_path: String::new(),
            paths: vec!["E:\\".into()],
            label: "CHIAVETTA".into(),
            file_system: "exFAT".into(),
            kind: DriveKind::Removable,
            space: Some(Space { capacity: 58 * gib, free: 41 * gib, free_to_caller: 41 * gib }),
            is_system: false,
        },
        Volume {
            guid_path: String::new(),
            paths: vec!["Z:\\".into()],
            label: "Archivio".into(),
            file_system: "ReFS".into(),
            kind: DriveKind::Fixed,
            space: Some(Space { capacity: 1800 * gib, free: 900 * gib, free_to_caller: 900 * gib }),
            is_system: false,
        },
    ]
}

fn new_app(lang: &str, theme: ThemeChoice) -> App {
    new_app_in(lang, theme, Env::fixed().region)
}

/// Come `new_app`, con l'area geografica data per numeri e date.
fn new_app_in(lang: &str, theme: ThemeChoice, region: RegionFormat) -> App {
    let mut env = Env::fixed();
    env.region = region;
    env.renderer = "wgpu Dx12 · prova".into();
    // Cartella dei dati inventata, come quella di un utente qualsiasi: le immagini di Impostazioni non
    // mostrano la cartella di chi le genera. Nessun test cambia le impostazioni, quindi non si scrive niente.
    env.data_dir = Some(PathBuf::from(r"C:\Users\demo\AppData\Roaming\Quado"));
    env.windows_languages = vec![if lang == "en" { "en-US".into() } else { "it-IT".into() }];
    let mut app = App::new(env, volumes());
    if lang == PSEUDO {
        app.set_language(PSEUDO);
    }
    app.settings.theme = theme;
    app
}

/// Porta l'app alla schermata e aspetta i lavori in background.
fn prepare(h: &mut Harness<'_, App>, shot: Shot) {
    {
        let app = h.state_mut();
        match shot {
            Shot::Volumes => {}
            Shot::Scanning => {
                let top = [("Users", 151u64), ("Program Files (x86)", 88), ("Windows", 19), ("Program Files", 24), ("ProgramData", 6), ("$Recycle.Bin", 9)]
                    .iter()
                    .map(|(n, g)| (n.encode_utf16().collect(), g << 30))
                    .collect();
                let p = Progress::preset(784_512, 126_430, 297 << 30, top);
                app.job = Some(ScanJob::preview(synthetic::volume_c(), p, Duration::from_millis(6_300)));
                app.screen = Screen::Scanning;
            }
            _ => {
                let (v, r) = synthetic::sample_c(shot == Shot::Interrupted);
                app.load_result_with(v, r, Duration::from_millis(14_700), synthetic::sample_c_evidence());
            }
        }
        match shot {
            Shot::Explore | Shot::Dialog | Shot::Move => {
                // La cartella di CapCut con Apps selezionata, come nella proposta.
                app.go(Screen::Explore);
                let d = app.data.as_ref().expect("dati");
                let t = d.tree();
                let capcut = (0..t.len() as u32).find(|&i| t.name_lossy(i) == "CapCut").expect("CapCut");
                let apps = t.children(capcut).find(|&c| t.name_lossy(c) == "Apps").expect("Apps");
                app.explore.open(d, capcut, Some(apps));
                if shot == Shot::Dialog || shot == Shot::Move {
                    // Letture simulate sullo stesso albero, poi la simulazione e il dialogo di conferma.
                    let probe = mirror(t);
                    probe.bin("C:\\", |b| b.capacity = Some(24_380 << 20));
                    let m = d.cls.matches.iter().position(|m| d.kb.rules[m.rule].id == "capcut-versioni").expect("regola");
                    app.env.actions = quado_ui::app::intervention_jobs::Actions::fake(&probe);
                    quado_ui::app::intervention_jobs::dispatch(app, quado_ui::app::intervention_jobs::ActRequest::PrepareRecycle, quado_ui::app::intervention_jobs::Origin::Rule(m));
                }
            }
            Shot::Palette => {
                app.palette.show_palette();
                app.palette.set_query("cap");
            }
            Shot::Search => {
                app.search.set_query("download");
                app.go(Screen::Explore);
            }
            Shot::BigFiles => app.go(Screen::BigFiles),
            Shot::Settings => app.go(Screen::Settings),
            Shot::Versions => {
                app.go(Screen::Explore);
                let d = app.data.as_ref().expect("dati");
                let t = d.tree();
                let old = (0..t.len() as u32).find(|&i| t.name_lossy(i) == "8.1.0.3402").expect("versione");
                let apps = t.parent(old);
                app.explore.open(d, apps, Some(old));
            }
            Shot::System => app.go(Screen::System),
            Shot::OverviewText150 => app.settings.text_scale = 150,
            Shot::ExploreCompact => {
                app.settings.density = quado_ui::tokens::Density::Compact;
                app.settings.sidebar_collapsed = true;
                app.settings.data_palette = quado_ui::tokens::DataPalette::ColorBlind;
                app.go(Screen::Explore);
            }
            Shot::Download => {
                // Lente con letture finte (nessun file letto): un confronto e i dati degli installatori,
                // con la riga delle copie identiche selezionata.
                use quado_ui::app::downloads_view::ReadKind;
                let fake = std::sync::Arc::new(synthetic::FakeReaders::for_tree(app.data.as_ref().expect("dati").tree()));
                app.env.readers = quado_ui::app::lens_jobs::Readers(fake);
                app.go(Screen::Downloads);
                let readers = app.env.readers.clone();
                let d = app.data.as_ref().expect("dati");
                let dl = &mut app.downloads;
                dl.settle(Some(d), &readers);
                dl.request(ReadKind::Hash, d, &readers);
                dl.confirm(d, &readers);
                dl.settle(Some(d), &readers);
                dl.request(ReadKind::Installers, d, &readers);
                dl.settle(Some(d), &readers);
                dl.confirm(d, &readers);
                dl.settle(Some(d), &readers);
                let copies = dl.view.as_ref().and_then(|v| v.rows.iter().position(|r| matches!(r.finding, quado_kb::lens::Finding::IdenticalCopies { .. })));
                if let Some(r) = copies {
                    dl.select(r);
                }
            }
            Shot::Programs => {
                // CapCut selezionato: dettaglio con l'avviso dei tuoi file, i dati dalle regole e la scheda.
                app.go(Screen::Programs);
                let d = app.data.as_ref().expect("dati");
                app.programs.selected = d.programs.items.iter().position(|x| x.name == "CapCut");
                assert!(app.programs.selected.is_some(), "CapCut tra i programmi");
            }
            Shot::Interventions => {
                // Registro di prova in una cartella propria, riscritto a ogni immagine. Percorso relativo alla
                // cartella del crate (quella dei test): l'immagine non mostra la cartella di chi la genera.
                let dir = PathBuf::from(r"..\..\target\m2-dati-interventi");
                std::fs::remove_dir_all(&dir).ok();
                std::fs::create_dir_all(&dir).expect("cartella");
                for e in journal_fixture() {
                    quado_actions::journal::append(&dir, &e).expect("registro di prova");
                }
                app.env.data_dir = Some(dir);
                // Il registro si rilegge al più una volta al secondo: con la cartella nuova, subito.
                app.procedures.history.invalidate();
                app.go(Screen::Interventions);
            }
            Shot::Uninstall => {
                app.go(Screen::Programs);
                let d = app.data.as_ref().expect("dati");
                let i = d.programs.items.iter().position(|x| x.name == "CapCut").expect("CapCut");
                app.programs.selected = Some(i);
                app.programs.dialog = Some(quado_ui::app::programs_view::UninstallDialog { program: i, copied: false, outcome: None });
            }
            Shot::Rules => {
                app.rules_view.selected = app.kb.index("capcut-versioni");
                app.rules_view.scroll_to = true;
                app.go(Screen::Rules);
            }
            _ => {}
        }
    }
    for _ in 0..400 {
        h.step();
        let a = h.state();
        let busy = a.search.pending() || a.big.pending() || a.intervention.simulating() || a.palette.pending();
        if !busy && h.state().frame_count > 6 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // Ancora qualche frame: le misure dell'ispettore arrivano in background.
    for _ in 0..20 {
        h.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    // Anteprima dello spostamento: per Da verificare le caselle partono vuote; «Seleziona tutti» come farebbe
    // l'utente, poi il dialogo dall'inizio.
    if shot == Shot::Move {
        use egui_kittest::kittest::Queryable as _;
        let all = h.state().lingue.t("spostamento-seleziona-tutti");
        h.get_by_label(&all).click();
        h.run_steps(3);
        // Il puntatore esce dalla finestra: niente freccia né evidenziazione nell'immagine.
        h.input_mut().events.push(egui::Event::PointerGone);
        h.run_steps(3);
    }
    // La vista Programmi chiude un dialogo rimasto da prima dell'arrivo: si apre dopo i primi frame.
    if shot == Shot::Uninstall {
        let app = h.state_mut();
        let i = app.programs.selected.expect("CapCut");
        app.programs.dialog = Some(quado_ui::app::programs_view::UninstallDialog { program: i, copied: false, outcome: None });
    }
    h.run_steps(3);
}

/// Registro di prova per la vista Interventi: uno spostamento nel Cestino con un elemento saltato e uno
/// svuotamento del Cestino, con le misure prima e dopo.
fn journal_fixture() -> Vec<quado_model::intervention::JournalEntry> {
    use quado_model::intervention::{BinCount, InterventionKind, ItemOutcome, JournalEntry, JournalItem, Measure, MeasurePoint, SkipReason};
    use quado_model::record::{Estimate, NotEstimable};
    use quado_model::verification::{self, ExplainFacts};
    const GIB: u64 = 1 << 30;
    const MINUTE: i64 = 600_000_000;
    let t1 = quado_actions::fake::NOW + 10 * MINUTE;
    let m = |point, at, free, bin, still| Measure { point, at, free, bin, still_present: still };
    let recycle_measures = vec![
        m(MeasurePoint::Start, t1, 100 * GIB, Some(BinCount { items: 1, bytes: GIB }), None),
        m(MeasurePoint::AfterEmpty, t1 + MINUTE, 108 * GIB, Some(BinCount::default()), Some(0)),
    ];
    let facts = ExplainFacts { skipped_or_failed: (1, GIB), recycled_bytes: 8 * GIB, ..ExplainFacts::default() };
    let recycle = JournalEntry {
        id: t1,
        volume: "C:\\".into(),
        kind: InterventionKind::Recycle,
        origin: "capcut-versioni".into(),
        method: "cestino".into(),
        estimate: Estimate::Exact(9 * GIB),
        items: vec![
            JournalItem { path: r"C:\Users\demo\AppData\Local\CapCut\Apps\6.4.1.2547".into(), bytes: 4 * GIB, outcome: ItemOutcome::Recycled },
            JournalItem { path: r"C:\Users\demo\AppData\Local\CapCut\Apps\7.0.0.2833".into(), bytes: 4 * GIB, outcome: ItemOutcome::Recycled },
            JournalItem { path: r"C:\Users\demo\AppData\Local\CapCut\Apps\8.4.1.3561".into(), bytes: GIB, outcome: ItemOutcome::Skipped(SkipReason::InUse) },
        ],
        comparison: verification::compare(&Estimate::Exact(9 * GIB), &recycle_measures, &facts),
        measures: recycle_measures,
    };
    let t2 = t1 + 20 * MINUTE;
    let empty_measures = vec![m(MeasurePoint::Start, t2, 50 * GIB, None, None), m(MeasurePoint::AfterEmpty, t2 + MINUTE, 52 * GIB, None, Some(4096))];
    let empty = JournalEntry {
        id: t2,
        volume: "C:\\".into(),
        kind: InterventionKind::Procedure("cestino-svuota".into()),
        origin: "cestino".into(),
        method: "cestino-svuota".into(),
        estimate: Estimate::NotEstimable(NotEstimable::ExternalProcedure),
        items: Vec::new(),
        comparison: verification::compare(&Estimate::NotEstimable(NotEstimable::ExternalProcedure), &empty_measures, &ExplainFacts::default()),
        measures: empty_measures,
    };
    vec![recycle, empty]
}

/// Letture simulate con lo stesso albero di `tree`: cartelle, file con le loro dimensioni, cartelle negate.
fn mirror(tree: &Tree) -> quado_actions::fake::FakeProbe {
    let p = quado_actions::fake::FakeProbe::new().volume("C:\\");
    for n in 1..tree.len() as u32 {
        let path = tree.path_lossy(n);
        if tree.is_dir(n) {
            if tree.flags(n).has(quado_model::Flags::DENIED) {
                p.denied(&path);
            } else {
                p.dir(&path);
            }
        } else {
            p.file(&path, tree.logical(n), tree.alloc(n));
        }
    }
    p
}

/// Area geografica delle immagini da `QUADO_REGIONE`: se manca, quella fissa dei test (italiana). I valori
/// sono quelli che Windows restituisce per l'area (`LOCALE_SDECIMAL`, `LOCALE_STHOUSAND`, `LOCALE_SGROUPING`,
/// `LOCALE_SSHORTDATE`).
fn snapshot_region() -> RegionFormat {
    match std::env::var("QUADO_REGIONE").ok().as_deref().map(str::trim) {
        None | Some("") => Env::fixed().region,
        Some(tag) => region(tag).unwrap_or_else(|| panic!("QUADO_REGIONE={tag}: aree note it-IT, en-US")),
    }
}

fn region(tag: &str) -> Option<RegionFormat> {
    match tag {
        "it-IT" => Some(RegionFormat { decimal: ",".into(), thousands: ".".into(), grouping: "3;0".into(), short_date: "dd/MM/yyyy".into() }),
        "en-US" => Some(RegionFormat { decimal: ".".into(), thousands: ",".into(), grouping: "3;0".into(), short_date: "M/d/yyyy".into() }),
        _ => None,
    }
}

#[test]
fn snapshot_regions_write_numbers_and_dates_like_windows() {
    // L'area italiana è quella fissa dei test; quella degli Stati Uniti scrive punto decimale e data M/g/aaaa.
    assert_eq!(region("it-IT"), Some(Env::fixed().region));
    let app = new_app_in("en", ThemeChoice::Dark, region("en-US").expect("en-US"));
    assert_eq!(app.fmt.size(4096), "4.00 KB");
    assert_eq!(app.fmt.count(1_734_052), "1,734,052");
    assert_eq!(quado_ui::format::short_date(&app.fmt.region.short_date, 2026, 11, 1), "11/1/2026");
    assert_eq!(region("fr-FR"), None);
}

fn harness(app: App, ppp: f32, wgpu: bool) -> Harness<'static, App> {
    harness_sized(app, ppp, wgpu, SIZE)
}

fn harness_sized(app: App, ppp: f32, wgpu: bool, size: [f32; 2]) -> Harness<'static, App> {
    let mut b = Harness::builder().with_size(size).with_pixels_per_point(ppp).with_max_steps(10_000);
    if wgpu {
        b = b.wgpu();
    }
    b.build_ui_state(|ui, app: &mut App| app.show(ui), app)
}

fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, bool)> {
    fn walk(s: &Shape, out: &mut Vec<(String, bool)>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            Shape::Text(t) => out.push((t.galley.text().to_owned(), t.galley.elided)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// Nomi e percorsi del volume di prova: sono dati, non testi da tradurre.
fn data_strings(app: &App) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Some(d) = &app.data {
        let t = d.tree();
        for i in 0..t.len() as u32 {
            set.insert(t.name_lossy(i));
            set.insert(t.path_lossy(i));
        }
        set.insert(d.volume.letter());
    }
    for v in volumes() {
        set.insert(v.file_system.clone());
        set.insert(v.label.clone());
    }
    set.insert("Quado".into());
    // Avvisi legali di Informazioni: nome del titolare e indirizzo, uguali in ogni lingua.
    set.insert(quado_ui::brand::COPYRIGHT.into());
    set.insert(quado_ui::brand::SOURCE_URL.into());
    set.insert("download".into());
    // Il testo scritto nella palette dei comandi.
    set.insert("cap".into());
    set.insert(PSEUDO.into());
    if let Some(d) = &app.env.data_dir {
        set.insert(d.display().to_string());
    }
    set.insert("wgpu Dx12 · prova".into());
    // Regole: identificativi, fonti ed estratti (in lingua originale), comandi: dati, non testi da tradurre.
    for r in &app.kb.rules {
        set.insert(r.id.clone());
        for src in &r.sources {
            set.insert(src.url.clone());
            set.insert(quado_ui::app::kbtext::source_place(&app.lingue, &src.url).1);
            set.insert(src.titolo.clone());
            set.insert(format!("«{}»", src.estratto));
        }
        for st in r.method.iter().chain(&r.alternatives) {
            set.extend(st.comando.iter().cloned());
            set.extend(st.uri.iter().cloned());
        }
    }
    if let Some(d) = &app.data {
        for m in &d.cls.matches {
            for st in m.record.preferred.iter().chain(&m.record.alternatives) {
                set.extend(st.command.iter().cloned());
                set.extend(st.uri.iter().cloned());
            }
        }
        // Programmi (M4): nomi, editori, versioni e cartelle dichiarate dalle fonti, URI e comandi dei metodi.
        for x in &d.programs.items {
            set.insert(x.name.clone());
            set.extend(x.publisher.iter().cloned());
            set.extend(x.version.iter().cloned());
            if let quado_kb::programs::Location::Folder { path, .. } | quado_kb::programs::Location::Implausible { path, .. } = &x.location {
                set.insert(path.clone());
            }
            for st in x.record.preferred.iter().chain(&x.record.alternatives) {
                set.extend(st.command.iter().cloned());
                set.extend(st.uri.iter().cloned());
            }
        }
    }
    for n in ["Users", "Program Files (x86)", "Program Files", "Windows", "ProgramData", "$Recycle.Bin"] {
        set.insert(n.into());
    }
    set
}

fn is_data(s: &str, data: &HashSet<String>) -> bool {
    let t = s.trim();
    if t.is_empty() || data.contains(t) {
        return true;
    }
    // Numeri, separatori, percorsi troncati all'inizio.
    if t.chars().all(|c| c.is_ascii_digit() || " .,:%/±-…".contains(c)) {
        return true;
    }
    if let Some(rest) = t.strip_prefix("…\\") {
        return data.iter().any(|d| d.ends_with(rest));
    }
    false
}

#[test]
fn pseudo_language_covers_every_text() {
    let mut problems = Vec::new();
    for shot in Shot::ALL {
        let mut h = harness(new_app(PSEUDO, ThemeChoice::Light), 1.0, false);
        prepare(&mut h, shot);
        let data = data_strings(h.state());
        for (text, elided) in texts(&h.output().shapes) {
            let pseudo = text.contains('[') && text.contains(']');
            if text.trim_start().starts_with('[') && text.trim_end().ends_with(']') && !text.contains('~') {
                problems.push(format!("{}: chiave mancante {text:?}", shot.name()));
            } else if !pseudo && !is_data(&text, &data) {
                problems.push(format!("{}: testo non tradotto {text:?}", shot.name()));
            } else if pseudo && elided {
                problems.push(format!("{}: testo tagliato {text:?}", shot.name()));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_screen_renders_in_both_languages_without_missing_keys() {
    for lang in ["it", "en"] {
        for shot in Shot::ALL {
            let mut h = harness(new_app(lang, ThemeChoice::Dark), 1.0, false);
            prepare(&mut h, shot);
            for (text, _) in texts(&h.output().shapes) {
                let t = text.trim();
                let looks_like_key = t.starts_with('[') && t.ends_with(']') && t[1..t.len() - 1].chars().all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit());
                assert!(!looks_like_key, "{lang} {}: chiave mancante {t}", shot.name());
            }
        }
    }
}

#[test]
#[ignore = "immagini: cargo xtask immagini"]
fn snapshots() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/m2-immagini");
    std::fs::create_dir_all(&out).expect("cartella");
    let only: Option<String> = std::env::var("QUADO_IMMAGINI").ok();
    // QUADO_LINGUE: una lingua o più, separate da virgole (en); QUADO_REGIONE: l'area geografica.
    let langs: Option<String> = std::env::var("QUADO_LINGUE").ok().filter(|l| !l.trim().is_empty());
    let region = snapshot_region();
    for lang in ["it", "en"] {
        if langs.as_deref().is_some_and(|l| !l.split(',').any(|x| x.trim() == lang)) {
            continue;
        }
        for (theme, tname) in [(ThemeChoice::Light, "chiaro"), (ThemeChoice::Dark, "scuro")] {
            for ppp in [1.0f32, 1.5, 2.0] {
                if lang == "en" && ppp != 1.0 {
                    continue;
                }
                for shot in Shot::ALL {
                    // QUADO_IMMAGINI: una schermata o più, separate da virgole (panoramica,esplora).
                    if only.as_deref().is_some_and(|o| !o.split(',').any(|x| x.trim() == shot.name())) {
                        continue;
                    }
                    let mut h = harness_sized(new_app_in(lang, theme, region.clone()), ppp, true, shot.size());
                    prepare(&mut h, shot);
                    let img = h.render().expect("disegno");
                    let name = format!("{}-{lang}-{tname}-{:.0}.png", shot.name(), ppp * 100.0);
                    img.save(out.join(&name)).expect("png");
                }
            }
        }
    }
    let _ = Tree::ROOT;
}
