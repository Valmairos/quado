// SPDX-License-Identifier: GPL-3.0-or-later
//! Banco di misura di M2 (07 §1.3 criterio 2, 06 §9), sullo schema di quello di M0.
//!
//! Gli input sono eventi sintetici iniettati prima di ogni frame (`raw_input_hook`) e passano per la
//! stessa gestione degli input di un utente. Il tempo di un frame è l'intervallo tra l'inizio di due
//! frame consecutivi con la sincronizzazione verticale spenta: interfaccia, tassellazione, disegno e
//! presentazione. Gli "effetti" provano che gli input hanno agito davvero (righe raggiunte, selezioni).
//!
//! M4 (07 §1.5 criterio 3, 06 §9): `installatori` e `hash` misurano il frame mentre la lente Download legge
//! i dati di versione o calcola le impronte su una cartella data (`--cartella`), con le letture vere e i
//! percorsi esclusi (`--escludi`) rifiutati prima di aprirli (`Readers::excluding`). La lettura si ripete
//! per `--giri` giri, dimenticando ogni volta i risultati e la cache.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use egui::{Event, Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, TouchPhase, pos2, vec2};
use quado_model::Tree;

use super::data::VolumeInfo;
use super::downloads_view::ReadKind;
use super::{App, Screen, synthetic};
use crate::perf::{Summary, summarize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Tabella da 3,5 milioni di righe (tutte le cartelle espanse).
    Table,
    /// Cartella con un milione di figli diretti.
    Children,
    /// Mappa con 20.000 aree.
    Map,
    /// Scansione vera di C: con la schermata di avanzamento.
    Scan,
    /// Lente Download: lettura dei dati di versione degli installatori di una cartella (F-DL-2).
    Installers,
    /// Lente Download: impronte dei file della stessa dimensione di una cartella (F-DL-4).
    Hash,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "tabella" => Some(Self::Table),
            "figli" => Some(Self::Children),
            "mappa" => Some(Self::Map),
            "scansione" => Some(Self::Scan),
            "installatori" => Some(Self::Installers),
            "hash" => Some(Self::Hash),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Table => "tabella",
            Self::Children => "figli",
            Self::Map => "mappa",
            Self::Scan => "scansione",
            Self::Installers => "installatori",
            Self::Hash => "hash",
        }
    }

    /// Banco della lente Download (M4).
    pub fn is_lens(self) -> bool {
        matches!(self, Self::Installers | Self::Hash)
    }
}

/// Argomenti dei banchi della lente: cartella, parti di percorso escluse, giri, thread dell'impronta.
#[derive(Clone, Debug, Default)]
pub struct LensArgs {
    pub folder: Option<String>,
    pub exclude: Vec<String>,
    /// 0: 5 giri per gli installatori, 2 per le impronte.
    pub rounds: u32,
    /// 0: `lens_jobs::HASH_THREADS`.
    pub threads: usize,
}

/// Una lettura della lente ripetuta per giri.
struct LensRun {
    read: ReadKind,
    rounds: u32,
    done: u32,
    started: Option<Instant>,
    /// File e byte annunciati prima della conferma.
    asked: Option<(usize, u64)>,
    round_ms: Vec<f64>,
    round_files: Vec<usize>,
    round_bytes: Vec<u64>,
    failed: usize,
    finished: bool,
    /// Frame da passare con il risultato a schermo prima di dimenticarlo: anche quei frame si misurano
    /// (in M4 l'elenco con migliaia di copie identiche costava secondi a frame e il banco non lo vedeva).
    hold: u32,
}

/// Frame con il risultato di un giro a schermo.
const HOLD_FRAMES: u32 = 60;

impl LensRun {
    /// Un passo per frame: chiede la lettura, conferma, aspetta la fine, dimentica e riparte.
    fn step(&mut self, app: &mut App) {
        if self.finished {
            return;
        }
        let readers = app.env.readers.clone();
        let Some(d) = app.data.as_ref() else {
            self.finished = true;
            return;
        };
        let st = &mut app.downloads;
        if let Some(p) = &st.prompt {
            self.asked = Some((p.files, p.bytes));
            st.confirm(d, &readers);
            return;
        }
        if st.progress().is_some() {
            return;
        }
        if self.hold > 0 {
            // Il risultato resta a schermo, con la lente ricostruita (`pending` finché non è pronta).
            if !st.pending() {
                self.hold -= 1;
                if self.hold == 0 {
                    if self.done >= self.rounds {
                        self.finished = true;
                    } else {
                        st.forget(self.read);
                    }
                }
            }
            return;
        }
        match self.started {
            None => {
                // Dopo `forget` la lente si ricostruisce prima di chiedere di nuovo.
                if st.pending() {
                    return;
                }
                st.request(self.read, d, &readers);
                if st.prompt.is_none() && st.progress().is_none() {
                    self.finished = true;
                    return;
                }
                self.started = Some(Instant::now());
            }
            Some(t) => {
                if let Some(o) = st.outcome {
                    self.round_ms.push(t.elapsed().as_secs_f64() * 1000.0);
                    self.round_files.push(o.read);
                    self.round_bytes.push(if o.cached { 0 } else { self.asked.map_or(0, |a| a.1) });
                    self.failed += o.failed;
                    self.done += 1;
                    self.started = None;
                    self.asked = None;
                    self.hold = HOLD_FRAMES;
                }
            }
        }
    }

    fn nothing(&self) -> bool {
        self.round_files.iter().all(|&n| n == 0)
    }

    fn describe(&self) -> String {
        let list = |v: Vec<String>| v.join(", ");
        format!(
            "\"giri\": {}, \"file_letti\": [{}], \"byte_letti\": [{}], \"durata_giro_ms\": [{}], \"non_letti\": {}, \"niente_da_leggere\": {}",
            self.done,
            list(self.round_files.iter().map(|x| x.to_string()).collect()),
            list(self.round_bytes.iter().map(|x| x.to_string()).collect()),
            list(self.round_ms.iter().map(|x| format!("{x:.0}")).collect()),
            self.failed,
            self.nothing()
        )
    }
}

/// Carica la cartella del banco della lente: scansione della sola cartella, voci di disinstallazione dal
/// registro (per la corrispondenza degli installatori), letture vere con le esclusioni.
fn prepare_lens(kind: Kind, app: &mut App, args: &LensArgs) -> (String, LensRun) {
    let folder = args.folder.clone().unwrap_or_default();
    let t0 = Instant::now();
    let scanned = quado_scan_win::scan(&folder, quado_scan_win::Options::default(), &quado_scan_win::Progress::default());
    let scan_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let lower = folder.to_lowercase();
    let volume = app.volumes.iter().filter_map(VolumeInfo::from_volume).find(|v| lower.starts_with(&v.root.to_lowercase())).unwrap_or_else(|| VolumeInfo {
        root: folder.chars().take(3).collect(),
        label: String::new(),
        file_system: String::new(),
        kind: quado_scan_win::volumes::DriveKind::Fixed,
        support: quado_scan_win::volumes::Support::Full,
        is_system: false,
    });
    let mut ev = quado_kb::Evidence::empty(&volume.root);
    ev.folders = quado_kb::win::known_folders();
    ev.user_sid = app.env.user_sid.clone();
    ev.uninstall = quado_kb::extract::uninstall(&quado_kb::win::LiveRegistry);
    let files = match scanned {
        Ok(r) => {
            let files = r.stats.files;
            app.load_result_with(volume, r, t0.elapsed(), ev);
            files
        }
        Err(e) => {
            eprintln!("scansione di {folder}: errore {}", e.0);
            0
        }
    };
    app.env.readers = app.env.readers.clone().excluding(&args.exclude);
    app.downloads.hash_threads = args.threads;
    app.downloads.choose(Some(folder.clone()));
    app.go(Screen::Downloads);
    let read = if kind == Kind::Hash { ReadKind::Hash } else { ReadKind::Installers };
    let rounds = if args.rounds > 0 { args.rounds } else if kind == Kind::Hash { 2 } else { 5 };
    let threads = if args.threads > 0 { args.threads } else { super::lens_jobs::HASH_THREADS };
    let info = format!(
        "\"cartella\": \"{}\", \"file\": {files}, \"scansione_ms\": {scan_ms:.0}, \"esclusioni\": {}, \"thread_impronta\": {threads}, \"giri\": {rounds}",
        folder.replace('\\', "\\\\").replace('"', "'"),
        args.exclude.len()
    );
    let run = LensRun {
        read,
        rounds,
        done: 0,
        started: None,
        asked: None,
        round_ms: Vec::new(),
        round_files: Vec::new(),
        round_bytes: Vec::new(),
        failed: 0,
        finished: false,
        hold: 0,
    };
    (info, run)
}

struct Phase {
    name: &'static str,
    frames: u32,
    criterion: bool,
}

struct Sample {
    phase: &'static str,
    interval_ms: f64,
}

pub struct Bench {
    kind: Kind,
    phases: Vec<Phase>,
    phase: usize,
    frame: u32,
    prev: Option<&'static str>,
    last: Option<Instant>,
    samples: Vec<Sample>,
    rng: synthetic::Rng,
    out_dir: PathBuf,
    label: String,
    pointer: Pos2,
    pub finished: bool,
    started: Instant,
    data_info: String,
    effects: Vec<(&'static str, String)>,
    fx: Effects,
    scan_started: Option<Instant>,
    lens: Option<LensRun>,
}

impl Bench {
    /// Prepara i dati della misura nell'app. `lens` vale per i banchi della lente.
    pub fn new(kind: Kind, out_dir: PathBuf, label: &str, app: &mut App, lens: &LensArgs) -> Self {
        let p = |name, frames, criterion| Phase { name, frames, criterion };
        let t0 = Instant::now();
        let mut data_info = String::new();
        let mut lens_run = None;
        let phases = match kind {
            Kind::Installers | Kind::Hash => {
                let (info, run) = prepare_lens(kind, app, lens);
                data_info = info;
                lens_run = Some(run);
                vec![p("riscaldamento", 120, false), p("lettura", u32::MAX, true), p("dopo", 60, false)]
            }
            Kind::Table | Kind::Children => {
                let wide = if kind == Kind::Table {
                    let (v, r) = synthetic::big_tree(3_500_000, 0x4d32);
                    app.load_result(v, r, std::time::Duration::ZERO);
                    None
                } else {
                    let (v, r, folder) = synthetic::wide_folder(1_000_000);
                    app.load_result(v, r, std::time::Duration::ZERO);
                    Some(folder)
                };
                let gen_ms = t0.elapsed().as_secs_f64() * 1000.0;
                if let Some(d) = app.data.as_ref() {
                    match wide {
                        None => app.explore.outline.expand_all(d.ord(), d.tree()),
                        Some(f) => app.explore.open(d, f, None),
                    }
                }
                app.go(Screen::Explore);
                let _ = write!(data_info, "\"righe\": {}, \"preparazione_ms\": {gen_ms:.0}", app.explore.outline.len());
                vec![p("riscaldamento", 120, false), p("trascinamento", 2400, true), p("rotella", 1200, true), p("salti", 600, true), p("tastiera", 600, true)]
            }
            Kind::Map => {
                let (v, r) = synthetic::map_tree(40, 500);
                app.load_result(v, r, std::time::Duration::ZERO);
                app.go(Screen::Explore);
                vec![p("riscaldamento", 120, false), p("hover", 2400, true), p("selezione", 1200, true)]
            }
            Kind::Scan => {
                let v = app.volumes.iter().find(|v| v.is_system).and_then(VolumeInfo::from_volume);
                if let Some(v) = v {
                    app.start_scan(v);
                }
                vec![p("scansione", u32::MAX, true)]
            }
        };
        app.show_perf = true;
        Self {
            kind,
            phases,
            phase: 0,
            frame: 0,
            prev: None,
            last: None,
            samples: Vec::new(),
            rng: synthetic::Rng::new(0x4d32),
            out_dir,
            label: label.to_owned(),
            pointer: Pos2::ZERO,
            finished: false,
            started: Instant::now(),
            data_info,
            effects: Vec::new(),
            fx: Effects::default(),
            scan_started: None,
            lens: lens_run,
        }
    }

    fn current(&self) -> Option<&Phase> {
        self.phases.get(self.phase)
    }

    /// Eventi sintetici per il frame che sta per cominciare.
    pub fn input(&mut self, app: &App, raw: &mut RawInput) {
        let Some(phase) = self.current() else { return };
        let (name, frames, f) = (phase.name, phase.frames, self.frame);
        raw.focused = true;
        let mut ev = Vec::new();
        let press = |pos: Pos2, pressed: bool| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        let key = |k: Key| {
            [
                Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE },
                Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE },
            ]
        };
        let g = app.explore.geom;
        match name {
            "trascinamento" => {
                let (top, bottom) = (g.bar.min.y + 12.0, g.bar.max.y - 2.0);
                let x = g.bar.center().x;
                if f == 0 {
                    self.pointer = pos2(x, top);
                    ev.push(Event::PointerMoved(self.pointer));
                    ev.push(press(self.pointer, true));
                } else if f + 1 < frames {
                    let t = f as f32 / (frames - 2) as f32;
                    self.pointer = pos2(x, top + (bottom - top) * t);
                    ev.push(Event::PointerMoved(self.pointer));
                } else {
                    ev.push(press(self.pointer, false));
                }
            }
            "rotella" => {
                self.pointer = g.body.center();
                ev.push(Event::PointerMoved(self.pointer));
                ev.push(Event::MouseWheel { unit: MouseWheelUnit::Line, delta: vec2(0.0, -3.0), phase: TouchPhase::Move, modifiers: Modifiers::NONE });
            }
            "salti" => {
                if f % 2 == 0 {
                    let y = g.bar.min.y + 2.0 + (g.bar.height() - 4.0) * (self.rng.below(10_000) as f32 / 10_000.0);
                    self.pointer = pos2(g.bar.center().x, y);
                    ev.push(Event::PointerMoved(self.pointer));
                    ev.push(press(self.pointer, true));
                } else {
                    ev.push(press(self.pointer, false));
                }
            }
            "tastiera" => {
                if f == 0 {
                    // Un clic su una riga dà il focus alla tabella.
                    self.pointer = pos2(g.body.min.x + 200.0, g.body.min.y + 40.0);
                    ev.push(Event::PointerMoved(self.pointer));
                    ev.push(press(self.pointer, true));
                    ev.push(press(self.pointer, false));
                } else {
                    ev.push(Event::PointerMoved(g.body.center()));
                    ev.extend(key(Key::PageDown));
                }
            }
            "lettura" | "dopo" => {
                // Il puntatore passa sull'elenco delle voci, come chi guarda la schermata durante la lettura.
                self.pointer = pos2(420.0 + (f % 900) as f32, 520.0 + ((f / 900) % 400) as f32);
                ev.push(Event::PointerMoved(self.pointer));
            }
            "hover" => {
                let r = g.map;
                let sweeps = 24.0;
                let s = f as f32 / frames as f32 * sweeps;
                let row = s.floor();
                let along = s - row;
                let x = if (row as u32).is_multiple_of(2) { r.min.x + along * r.width() } else { r.max.x - along * r.width() };
                let y = r.min.y + (row + 0.5) / sweeps * r.height();
                self.pointer = pos2(x, y);
                ev.push(Event::PointerMoved(self.pointer));
            }
            "selezione" => {
                let r = g.map;
                if f % 2 == 0 {
                    self.pointer = pos2(r.min.x + r.width() * (self.rng.below(1000) as f32 / 1000.0), r.min.y + r.height() * (self.rng.below(1000) as f32 / 1000.0));
                    ev.push(Event::PointerMoved(self.pointer));
                    ev.push(press(self.pointer, true));
                } else {
                    ev.push(press(self.pointer, false));
                    let k = [Key::ArrowRight, Key::ArrowDown, Key::ArrowLeft, Key::ArrowUp][(f / 2 % 4) as usize];
                    ev.extend(key(k));
                }
            }
            _ => {}
        }
        raw.events.extend(ev);
    }

    /// Inizio di un frame: registra il tempo del frame precedente e avanza la fase.
    pub fn frame(&mut self, app: &mut App, ctx: &egui::Context) {
        let now = Instant::now();
        if let (Some(prev), Some(last)) = (self.prev, self.last) {
            self.samples.push(Sample { phase: prev, interval_ms: (now - last).as_secs_f64() * 1000.0 });
        }
        self.last = Some(now);
        let Some(phase) = self.current() else {
            if !self.finished {
                self.finished = true;
                match self.write(app) {
                    Ok(p) => eprintln!("risultati: {}", p.display()),
                    Err(e) => eprintln!("risultati non scritti: {e}"),
                }
            }
            return;
        };
        let (name, frames) = (phase.name, phase.frames);
        if self.frame == 0 {
            match name {
                "trascinamento" | "rotella" => app.explore.scroll.top = 0.0,
                "scansione" => self.scan_started = Some(now),
                _ => {}
            }
            self.fx = Effects::start(app);
        } else {
            self.fx.observe(app);
        }
        self.prev = Some(name);
        self.frame += 1;
        let scan_done = self.kind == Kind::Scan && self.frame > 2 && app.job.is_none();
        let mut lens_done = false;
        if name == "lettura"
            && let Some(run) = self.lens.as_mut()
        {
            run.step(app);
            lens_done = run.finished;
        }
        if self.frame >= frames || scan_done || lens_done {
            let fx = match &self.lens {
                Some(run) if name == "lettura" => run.describe(),
                Some(_) => String::new(),
                None => self.fx.describe(self.kind, app, self.scan_started),
            };
            self.effects.push((name, fx));
            self.phase += 1;
            self.frame = 0;
        }
        ctx.request_repaint();
    }

    fn summary_of(&self, filter: impl Fn(&Sample) -> bool) -> Summary {
        let v: Vec<f64> = self.samples.iter().filter(|s| filter(s)).map(|s| s.interval_ms).collect();
        summarize(&v)
    }

    fn write(&self, app: &App) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.out_dir)?;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let prefix = if self.kind.is_lens() { "m4" } else { "m2" };
        let path = self.out_dir.join(format!("{prefix}-{}-{stamp}.json", self.kind.name()));
        let sj = |s: &Summary| format!("{{\"n\": {}, \"p50\": {:.3}, \"p95\": {:.3}, \"p99\": {:.3}, \"max\": {:.3}, \"media\": {:.3}}}", s.n, s.p50, s.p95, s.p99, s.max, s.mean);
        let mut phases = Vec::new();
        for ph in &self.phases {
            let s = self.summary_of(|x| x.phase == ph.name);
            let fx = self.effects.iter().find(|(n, _)| *n == ph.name).map_or(String::new(), |(_, d)| d.clone());
            phases.push(format!("    {{\"nome\": \"{}\", \"criterio\": {}, \"effetti\": {{{fx}}}, \"intervallo_ms\": {}}}", ph.name, ph.criterion, sj(&s)));
        }
        let crit: Vec<&str> = self.phases.iter().filter(|p| p.criterion).map(|p| p.name).collect();
        let ci = self.summary_of(|s| crit.contains(&s.phase));
        let (rule, pass) = match self.kind {
            Kind::Table | Kind::Children => ("p95 <= 16 ms e p99 <= 33 ms", ci.p95 <= 16.0 && ci.p99 <= 33.0),
            Kind::Map => ("p95 <= 16 ms con 20.000 aree", ci.p95 <= 16.0 && app.explore.map.layout.leaves >= 20_000),
            Kind::Scan => ("p95 <= 16 ms e almeno 4 aggiornamenti al secondo", ci.p95 <= 16.0 && updates_per_second(app, self.scan_started) >= 4.0),
            Kind::Installers | Kind::Hash => ("p95 <= 16 ms durante la lettura", ci.p95 <= 16.0 && self.lens.as_ref().is_some_and(|r| !r.nothing())),
        };
        let data = match self.kind {
            Kind::Map => format!("\"aree\": {}, \"foglie\": {}", app.explore.map.layout.tiles.len(), app.explore.map.layout.leaves),
            Kind::Scan => format!("\"file\": {}", app.data.as_ref().map_or(0, |d| d.tree().files(Tree::ROOT))),
            _ => self.data_info.clone(),
        };
        let json = format!(
            "{{\n  \"tipo\": \"{}\",\n  \"renderer\": \"{}\",\n  \"durata_s\": {:.1},\n  \"dati\": {{{data}}},\n  \"fasi\": [\n{}\n  ],\n  \"criterio\": {{\"fasi\": {:?}, \"regola\": \"{rule}\", \"intervallo_ms\": {}, \"superato\": {pass}}}\n}}\n",
            self.kind.name(),
            self.label.replace('"', "'"),
            self.started.elapsed().as_secs_f64(),
            phases.join(",\n"),
            crit,
            sj(&ci),
        );
        std::fs::write(&path, json)?;
        Ok(path)
    }
}

/// Aggiornamenti dell'avanzamento al secondo: valori diversi mostrati diviso la durata (F-SC-4).
pub fn updates_per_second(app: &App, since: Option<Instant>) -> f64 {
    let u = &app.progress_updates;
    let (Some(first), Some(last)) = (u.first(), u.last()) else { return 0.0 };
    let start = since.unwrap_or(first.0);
    let secs = (last.0 - start).as_secs_f64();
    if secs <= 0.0 { 0.0 } else { u.len() as f64 / secs }
}

#[derive(Clone, Debug, Default)]
struct Effects {
    top: f64,
    max_top: f64,
    moved_frames: u32,
    selected: Option<u32>,
    selection_changes: u32,
    hovered: Option<usize>,
    hover_changes: u32,
}

impl Effects {
    fn start(app: &App) -> Self {
        Self { top: app.explore.scroll.top, max_top: app.explore.scroll.top, selected: app.explore.selected, hovered: app.explore.map.hovered, ..Default::default() }
    }

    fn observe(&mut self, app: &App) {
        let top = app.explore.scroll.top;
        if top != self.top {
            self.moved_frames += 1;
            self.top = top;
        }
        self.max_top = self.max_top.max(top);
        if app.explore.selected != self.selected {
            self.selection_changes += 1;
            self.selected = app.explore.selected;
        }
        if app.explore.map.hovered != self.hovered {
            self.hover_changes += 1;
            self.hovered = app.explore.map.hovered;
        }
    }

    fn describe(&self, kind: Kind, app: &App, scan_started: Option<Instant>) -> String {
        match kind {
            Kind::Table | Kind::Children => {
                format!("\"riga_massima_raggiunta\": {:.0}, \"frame_con_scorrimento\": {}, \"cambi_di_selezione\": {}", self.max_top, self.moved_frames, self.selection_changes)
            }
            Kind::Map => format!(
                "\"aree\": {}, \"foglie\": {}, \"cambi_di_hover\": {}, \"cambi_di_selezione\": {}",
                app.explore.map.layout.tiles.len(),
                app.explore.map.layout.leaves,
                self.hover_changes,
                self.selection_changes
            ),
            Kind::Scan => format!(
                "\"aggiornamenti_al_secondo\": {:.1}, \"valori_mostrati\": {}, \"durata_s\": {:.1}",
                updates_per_second(app, scan_started),
                app.progress_updates.len(),
                scan_started.map_or(0.0, |s| s.elapsed().as_secs_f64())
            ),
            Kind::Installers | Kind::Hash => String::new(),
        }
    }
}
