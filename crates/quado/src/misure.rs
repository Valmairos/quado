// SPDX-License-Identifier: GPL-3.0-or-later
//! Misure per la baseline delle prestazioni. Solo letture: la scansione del volume di sistema e,
//! nel giro delle schermate, le letture su richiesta della lente Download che l'utente farebbe (dati degli
//! installatori, confronto del contenuto di un gruppo piccolo), con le parti di `--escludi` escluse prima
//! di aprirle (`Readers::excluding`), come nella sessione di prova. Nessun intervento, nessuna simulazione.
//!
//! - [`avvio`]: tappe dell'avvio in millisecondi dalla creazione del processo, scritte con
//!   `--misura-avvio` insieme ai moduli caricati e alla memoria.
//! - [`Giro`] (`--misura-schermate FILE`): ogni schermata con i dati veri per qualche centinaio di frame,
//!   sincronizzazione verticale spenta, puntatore e rotella sintetici come nei banchi di `--banco`; per ogni fase
//!   il tempo di preparazione (dal comando alla schermata pronta) e i frame di entrata e a regime.
//! - [`Riposo`] (`--misura-riposo FILE`): l'app ferma, con la sincronizzazione verticale e senza ridisegni
//!   forzati: frame disegnati, CPU e memoria del processo, schermata per schermata.
//!
//! Il risultato va su standard output e nel file, una riga per valore separata da tabulazioni.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Event, Modifiers, MouseWheelUnit, PointerButton, Pos2, RawInput, Rect, TouchPhase, pos2, vec2};
use quado_model::Tree;
use quado_model::downloads::GroupComparison;
use quado_scan_win::win::process_metrics;
use quado_ui::app::data::VolumeInfo;
use quado_ui::app::downloads_view::ReadKind;
use quado_ui::app::intervention_jobs::find_node;
use quado_ui::app::{App, Screen};
use quado_ui::perf::summarize;

/// Byte al massimo letti dal confronto del contenuto: un gruppo piccolo.
const HASH_LIMIT: u64 = 256 << 20;

pub mod avvio {
    //! Tappe dell'avvio: millisecondi dalla creazione del processo (`GetProcessTimes`) al momento della
    //! prima tappa, poi `Instant` per le successive.

    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    static BASE: OnceLock<(Instant, f64)> = OnceLock::new();
    static MARKS: Mutex<Vec<(&'static str, f64)>> = Mutex::new(Vec::new());

    /// Prima tappa (`main`), dopo `harden_dll_search`.
    pub fn start() {
        let _ = BASE.set((Instant::now(), quado_scan_win::process::ms_since_start()));
        mark("main");
    }

    /// Millisecondi dalla creazione del processo.
    pub fn now() -> f64 {
        BASE.get().map_or(f64::NAN, |(t, b)| b + t.elapsed().as_secs_f64() * 1000.0)
    }

    pub fn mark(name: &'static str) {
        let ms = now();
        if let Ok(mut m) = MARKS.lock() {
            m.push((name, ms));
        }
    }

    /// Tappe, memoria del processo e moduli caricati, una riga per valore.
    pub fn report() -> String {
        let mut out = String::new();
        if let Ok(m) = MARKS.lock() {
            for (name, ms) in m.iter() {
                out.push_str(&format!("tappa\t{name}\t{ms:.1}\n"));
            }
        }
        let p = quado_scan_win::win::process_metrics();
        out.push_str(&format!("memoria\tworking-set\t{}\n", p.working_set));
        out.push_str(&format!("memoria\tpicco\t{}\n", p.peak_working_set));
        out.push_str(&format!("memoria\tprivata\t{}\n", p.private));
        out.push_str(&format!("cpu\tutente-ms\t{:.0}\n", p.user.as_secs_f64() * 1000.0));
        out.push_str(&format!("cpu\tkernel-ms\t{:.0}\n", p.kernel.as_secs_f64() * 1000.0));
        out.push_str(&format!("handle\t{}\n", p.handles));
        for m in quado_scan_win::process::loaded_modules() {
            out.push_str(&format!("modulo\t{m}\n"));
        }
        out
    }
}

use crate::block_on;

/// `--misura-wgpu`: quanto costano istanza, elenco degli adattatori (wgpu crea un dispositivo Direct3D 12
/// su ciascuno per provarlo) e dispositivo sull'adattatore scelto, con i moduli caricati da ogni passo.
pub fn wgpu_diagnostic(want: Option<quado_scan_win::system::DisplayAdapter>, choice: crate::DeviceChoice) {
    use eframe::egui_wgpu::wgpu;
    let modules = || quado_scan_win::process::loaded_modules();
    let mem = || process_metrics();
    let before = modules();
    let m0 = mem();
    let t = Instant::now();
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::DX12, ..wgpu::InstanceDescriptor::new_without_display_handle() });
    println!("wgpu\tistanza-ms\t{:.1}", t.elapsed().as_secs_f64() * 1000.0);
    let t = Instant::now();
    let adapters = block_on(instance.enumerate_adapters(wgpu::Backends::DX12));
    println!("wgpu\telenco-ms\t{:.1}", t.elapsed().as_secs_f64() * 1000.0);
    let m1 = mem();
    for a in &adapters {
        let i = a.get_info();
        println!("adattatore\t{:?}\t{}\t{:04x}:{:04x}", i.device_type, i.name, i.vendor, i.device);
    }
    let after_list = modules();
    for m in after_list.iter().filter(|m| !before.contains(m)) {
        println!("modulo-elenco\t{m}");
    }
    let chosen = adapters
        .iter()
        .find(|a| {
            let i = a.get_info();
            i.device_type != wgpu::DeviceType::Cpu && want.as_ref().is_some_and(|w| i.vendor == w.vendor && i.device == w.device)
        })
        .or_else(|| adapters.iter().find(|a| a.get_info().device_type != wgpu::DeviceType::Cpu));
    // Il dispositivo che chiede l'app (o quello di egui, con `--wgpu-limiti egui --wgpu-memoria prestazioni`).
    println!("wgpu\tdispositivo-scelta\t{choice:?}");
    let device = chosen.map(|a| {
        let t = Instant::now();
        let r = block_on(a.request_device(&crate::device_descriptor(choice, a)));
        println!("wgpu\tdispositivo-ms\t{:.1}\t{}", t.elapsed().as_secs_f64() * 1000.0, a.get_info().name);
        let after_dev = modules();
        for m in after_dev.iter().filter(|m| !after_list.contains(m)) {
            println!("modulo-dispositivo\t{m}");
        }
        r
    });
    let m2 = mem();
    println!("memoria\tdopo-elenco-working-set\t{}", m1.working_set.saturating_sub(m0.working_set));
    println!("memoria\tdopo-elenco-privata\t{}", m1.private.saturating_sub(m0.private));
    println!("memoria\tdopo-dispositivo-working-set\t{}", m2.working_set.saturating_sub(m0.working_set));
    println!("memoria\tdopo-dispositivo-privata\t{}", m2.private.saturating_sub(m0.private));
    // Un secondo elenco con il driver già caricato (quanto costerebbe a egui-wgpu dopo un elenco anticipato).
    let t = Instant::now();
    let again = block_on(wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::DX12, ..wgpu::InstanceDescriptor::new_without_display_handle() }).enumerate_adapters(wgpu::Backends::DX12));
    println!("wgpu\telenco-di-nuovo-ms\t{:.1}", t.elapsed().as_secs_f64() * 1000.0);
    // E un secondo dispositivo sulla stessa scheda, da quel secondo elenco.
    let name = chosen.map(|a| a.get_info().name);
    if let Some(a) = again.iter().find(|a| Some(a.get_info().name) == name) {
        let t = Instant::now();
        let second = block_on(a.request_device(&crate::device_descriptor(choice, a)));
        println!("wgpu\tdispositivo-di-nuovo-ms\t{:.1}", t.elapsed().as_secs_f64() * 1000.0);
        drop(second);
    }
    drop(again);
    // Le schede non usate tengono il loro dispositivo Direct3D 12 finché vive l'adattatore.
    let keep = chosen.map(|a| a.get_info().name);
    let mut adapters = adapters;
    adapters.retain(|a| Some(a.get_info().name) == keep);
    let m3 = mem();
    println!("memoria\tsenza-adattatori-non-usati-privata\t{}", m3.private.saturating_sub(m0.private));
    println!("memoria\tsenza-adattatori-non-usati-working-set\t{}", m3.working_set.saturating_sub(m0.working_set));
    println!("handle\t{}\t{}", m0.handles, m2.handles);
    drop(device);
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn system_volume(app: &App) -> Option<VolumeInfo> {
    app.volumes.iter().find(|v| v.is_system).and_then(VolumeInfo::from_volume)
}

/// Il percorso contiene una delle parti di `--escludi`, le stesse della sessione di prova.
fn excluded(path: &str) -> bool {
    crate::session::excluded(path)
}

fn excluding_readers(app: &mut App) {
    app.env.readers = app.env.readers.clone().excluding(crate::session::excluded_parts());
}

// ---------------------------------------------------------------------------------------------
// Giro delle schermate

/// Zona su cui agiscono gli input sintetici.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Zone {
    /// Area centrale della finestra, senza barre.
    Content,
    Map,
    ExploreBody,
    SearchBody,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Input {
    Sweep(Zone),
    Wheel(Zone),
    /// Trascinamento della barra di scorrimento della tabella di Esplora, dall'alto in basso.
    Drag,
}

/// Esito del controllo "pronto" di una fase.
enum Ready {
    Wait,
    Go,
    /// La fase non si può fare su questo PC: la si salta con il motivo.
    Skip(String),
}

/// Una fase del giro: un comando, l'attesa della schermata pronta, poi `frames` frame con gli input.
struct Item {
    name: &'static str,
    prepare: fn(&mut App, &mut Notes),
    ready: fn(&mut App, &mut Notes) -> Ready,
    frames: u32,
    input: Input,
}

/// Note di una fase (testo libero) e stato condiviso tra le fasi.
#[derive(Default)]
struct Notes {
    text: Vec<String>,
    group: Vec<u32>,
    started: Option<Instant>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Enter,
    Steady,
}

struct Done {
    name: &'static str,
    ready_ms: f64,
    first_ms: f64,
    enter: Vec<f64>,
    steady: Vec<f64>,
    notes: Vec<String>,
}

pub struct Giro {
    out: PathBuf,
    items: Vec<Item>,
    index: usize,
    /// Fase in corso: dopo `prepare`, istante del comando; poi frame contati a regime.
    prepared: Option<Instant>,
    ready_at: Option<Instant>,
    frames_in: u32,
    enter: Vec<f64>,
    steady: Vec<f64>,
    notes: Notes,
    first_ms: Option<f64>,
    last: Option<Instant>,
    prev: Option<Part>,
    rect: Rect,
    pointer: Pos2,
    scan_started: Option<Instant>,
    scan_seen: bool,
    after_load: Vec<f64>,
    lines: String,
    done: Vec<Done>,
    pub finished: bool,
    started: Instant,
}

fn ready_now(_: &mut App, _: &mut Notes) -> Ready {
    Ready::Go
}

fn nothing(_: &mut App, _: &mut Notes) {}

fn go(app: &mut App, s: Screen) {
    app.go(s);
}

fn items() -> Vec<Item> {
    let it = |name, prepare: fn(&mut App, &mut Notes), ready: fn(&mut App, &mut Notes) -> Ready, frames, input| Item { name, prepare, ready, frames, input };
    vec![
        it("panoramica", |a, _| go(a, Screen::Overview), ready_now, 300, Input::Sweep(Zone::Content)),
        it("esplora-radice-mappa", |a, _| go(a, Screen::Explore), ready_now, 300, Input::Sweep(Zone::Map)),
        it("esplora-radice-tabella", nothing, ready_now, 300, Input::Wheel(Zone::ExploreBody)),
        it(
            "esplora-appdata-local",
            |a, n| {
                let Some(d) = a.data.as_ref() else { return };
                match d.evidence.folders.local.as_deref().and_then(|p| find_node(d.tree(), p)) {
                    Some(f) => a.explore.open(d, f, None),
                    None => n.text.push("cartella non trovata".into()),
                }
            },
            ready_now,
            300,
            Input::Sweep(Zone::Map),
        ),
        it(
            "tabella-grande-trascinamento",
            |a, n| {
                let Some(d) = a.data.as_ref() else { return };
                a.explore.open(d, Tree::ROOT, None);
                let t = Instant::now();
                a.explore.outline.expand_all(d.ord(), d.tree());
                n.text.push(format!("expand_all {:.0} ms, righe {}", ms(t.elapsed()), a.explore.outline.len()));
                a.explore.scroll.top = 0.0;
                go(a, Screen::Explore);
            },
            ready_now,
            300,
            Input::Drag,
        ),
        it("tabella-grande-rotella", nothing, ready_now, 300, Input::Wheel(Zone::ExploreBody)),
        it(
            "ricerca-a",
            |a, n| {
                if let Some(d) = a.data.as_ref() {
                    a.explore.reset(d);
                }
                go(a, Screen::Explore);
                a.search.set_query("a");
                n.started = Some(Instant::now());
            },
            search_ready,
            300,
            Input::Wheel(Zone::SearchBody),
        ),
        it(
            "ricerca-setup",
            |a, n| {
                a.search.set_query("setup");
                n.started = Some(Instant::now());
            },
            search_ready,
            200,
            Input::Sweep(Zone::SearchBody),
        ),
        it(
            "file-grandi",
            |a, _| {
                a.search.clear();
                go(a, Screen::BigFiles);
            },
            |a, n| {
                if a.big.pending() || a.big.outcome.is_none() {
                    return Ready::Wait;
                }
                n.text.push(format!("file {}", a.big.outcome.as_ref().map_or(0, |o| o.files.len())));
                Ready::Go
            },
            200,
            Input::Sweep(Zone::Content),
        ),
        it("file-grandi-rotella", nothing, ready_now, 200, Input::Wheel(Zone::Content)),
        it("sistema", |a, _| go(a, Screen::System), ready_now, 200, Input::Sweep(Zone::Content)),
        it("regole", |a, _| go(a, Screen::Rules), ready_now, 200, Input::Sweep(Zone::Content)),
        it(
            "download",
            |a, _| {
                a.downloads.choose(None);
                go(a, Screen::Downloads);
            },
            |a, n| {
                let Some(d) = a.data.as_ref() else { return Ready::Skip("nessun dato".into()) };
                if a.downloads.root().is_none() || a.downloads.pending() {
                    return Ready::Wait;
                }
                match a.downloads.current(d) {
                    Some(v) => {
                        n.text.push(format!("voci {}, gruppi della stessa dimensione {}, costruzione {:.0} ms", v.findings.len(), v.candidates.len(), ms(v.elapsed)));
                        Ready::Go
                    }
                    None => Ready::Wait,
                }
            },
            300,
            Input::Sweep(Zone::Content),
        ),
        it("download-rotella", nothing, ready_now, 200, Input::Wheel(Zone::Content)),
        it(
            "download-installatori",
            |a, n| {
                let readers = a.env.readers.clone();
                let Some(d) = a.data.as_ref() else { return };
                a.downloads.request(ReadKind::Installers, d, &readers);
                n.started = Some(Instant::now());
            },
            |a, n| read_ready(a, n, ReadKind::Installers),
            120,
            Input::Sweep(Zone::Content),
        ),
        it(
            "download-confronto",
            |a, n| {
                let readers = a.env.readers.clone();
                let Some(d) = a.data.as_ref() else { return };
                let st = &mut a.downloads;
                let Some(v) = st.current(d) else {
                    n.text.push("lente non pronta".into());
                    return;
                };
                let t = d.tree();
                let chosen = Instant::now();
                let ok_node = |x: u32| !t.flags(x).has(quado_model::Flags::CLOUD) && !excluded(&t.path_lossy(x));
                let best = v
                    .candidates
                    .iter()
                    .zip(&v.compared)
                    .filter(|(_, c)| matches!(c, GroupComparison::NotRead { .. }))
                    .map(|(g, _)| (g.logical.saturating_mul(g.nodes.len() as u64), g))
                    .filter(|&(bytes, g)| bytes <= HASH_LIMIT && g.nodes.len() >= 2 && g.nodes.iter().all(|&x| ok_node(x)))
                    .max_by_key(|&(bytes, g)| (bytes, std::cmp::Reverse(g.nodes[0])));
                let Some((_, g)) = best else {
                    n.text.push("nessun gruppo entro il limite".into());
                    return;
                };
                n.group = g.nodes.clone();
                let chosen = chosen.elapsed();
                let t = Instant::now();
                st.request(ReadKind::Hash, d, &readers);
                let asked = t.elapsed();
                let group = n.group.clone();
                if let Some(p) = st.prompt.as_mut() {
                    n.text.push(format!("richiesta come l'utente {} file, {} byte", p.files, p.bytes));
                    p.retain_nodes(|x| group.contains(&x));
                    n.text.push(format!("gruppo di {} file, {} byte", p.files, p.bytes));
                }
                let t = Instant::now();
                st.confirm(d, &readers);
                n.text.push(format!("scelta del gruppo (misura) {:.0} ms, request {:.0} ms, confirm {:.0} ms", ms(chosen), ms(asked), ms(t.elapsed())));
                n.started = Some(Instant::now());
            },
            |a, n| read_ready(a, n, ReadKind::Hash),
            120,
            Input::Sweep(Zone::Content),
        ),
        it(
            "programmi",
            |a, _| {
                go(a, Screen::Programs);
                if a.data.as_ref().is_some_and(|d| !d.programs.items.is_empty()) {
                    a.programs.selected = Some(0);
                }
            },
            ready_now,
            300,
            Input::Sweep(Zone::Content),
        ),
        it("programmi-rotella", nothing, ready_now, 200, Input::Wheel(Zone::Content)),
        it("interventi", |a, _| go(a, Screen::Interventions), ready_now, 200, Input::Sweep(Zone::Content)),
        it("impostazioni", |a, _| go(a, Screen::Settings), ready_now, 200, Input::Sweep(Zone::Content)),
        it("panoramica-di-nuovo", |a, _| go(a, Screen::Overview), ready_now, 200, Input::Sweep(Zone::Content)),
        // "Rileggi dal disco" su una cartella piccola (la cartella `docs` del repository, sola lettura): il
        // modello si aggiorna sul thread dell'interfaccia (`apply_verified`), il frame più lungo dell'entrata
        // è quel lavoro.
        it(
            "rileggi-cartella",
            |a, n| {
                let Some(d) = a.data.as_ref() else { return };
                let path = std::env::current_dir().map(|p| p.join("docs").to_string_lossy().into_owned()).unwrap_or_default();
                let Some(node) = find_node(d.tree(), &path) else {
                    n.text.push(format!("cartella non trovata: {path}"));
                    return;
                };
                a.explore.reveal(d, node);
                go(a, Screen::Explore);
                a.inspector.verifying = Some(node);
                a.inspector.verify = Some(quado_ui::app::data::spawn_verify(path, node, true));
                n.started = Some(Instant::now());
            },
            |a, n| {
                if n.started.is_none() {
                    return Ready::Skip(n.text.join("; "));
                }
                if a.inspector.verify.is_some() || a.last_verify.is_none() {
                    return Ready::Wait;
                }
                Ready::Go
            },
            120,
            Input::Sweep(Zone::Content),
        ),
    ]
}

fn search_ready(a: &mut App, n: &mut Notes) -> Ready {
    if a.search.pending() {
        return Ready::Wait;
    }
    match &a.search.outcome {
        Some(o) if a.search.text == o.query.0 => {
            n.text.push(format!("risultati {}, ricerca {:.0} ms nel thread", o.hits.len(), ms(o.elapsed)));
            Ready::Go
        }
        _ => Ready::Wait,
    }
}

fn read_ready(a: &mut App, n: &mut Notes, kind: ReadKind) -> Ready {
    if n.started.is_none() {
        return Ready::Skip(n.text.join("; "));
    }
    // Gli installatori chiedono conferma dopo il controllo degli attributi: si conferma come l'utente.
    if a.downloads.prompt.is_some() {
        let readers = a.env.readers.clone();
        if let (Some(d), Some(p)) = (a.data.as_ref(), a.downloads.prompt.as_ref()) {
            n.text.push(format!("conferma dopo {:.0} ms: {} file, {} byte", n.started.map_or(0.0, |t| ms(t.elapsed())), p.files, p.bytes));
            a.downloads.confirm(d, &readers);
        }
        return Ready::Wait;
    }
    let st = &a.downloads;
    if st.progress().is_some() || st.busy() || st.pending() {
        return Ready::Wait;
    }
    match st.outcome {
        Some(o) if o.kind == kind => {
            n.text.push(format!("letti {}, non letti {}, su {}, dalla cache {}", o.read, o.failed, o.total, o.cached));
            Ready::Go
        }
        _ => Ready::Wait,
    }
}

impl Giro {
    pub fn new(app: &mut App, out: PathBuf) -> Self {
        excluding_readers(app);
        let mut g = Self {
            out,
            items: items(),
            index: 0,
            prepared: None,
            ready_at: None,
            frames_in: 0,
            enter: Vec::new(),
            steady: Vec::new(),
            notes: Notes::default(),
            first_ms: None,
            last: None,
            prev: None,
            rect: Rect::ZERO,
            pointer: Pos2::ZERO,
            scan_started: None,
            scan_seen: false,
            after_load: Vec::new(),
            lines: String::new(),
            done: Vec::new(),
            finished: false,
            started: Instant::now(),
        };
        match system_volume(app) {
            Some(v) => {
                app.start_scan(v);
                g.scan_started = app.job.as_ref().map(|j| j.started);
            }
            None => g.line("errore\tnessun volume di sistema"),
        }
        g
    }

    fn line(&mut self, s: &str) {
        println!("{s}");
        self.lines.push_str(s);
        self.lines.push('\n');
    }

    fn zone(&self, app: &App, z: Zone) -> Rect {
        let r = self.rect;
        let content = Rect::from_min_max(pos2(r.min.x + 240.0, r.min.y + 70.0), pos2(r.max.x - 30.0, r.max.y - 40.0));
        let pick = |x: Rect| if x.width() > 10.0 && x.height() > 10.0 { x } else { content };
        match z {
            Zone::Content => content,
            Zone::Map => pick(app.explore.geom.map),
            Zone::ExploreBody => pick(app.explore.geom.body),
            Zone::SearchBody => pick(app.search.geom.body),
        }
    }

    /// Eventi sintetici del frame che sta per cominciare (solo a regime).
    pub fn input(&mut self, app: &App, raw: &mut RawInput) {
        if self.ready_at.is_none() {
            return;
        }
        let Some(item) = self.items.get(self.index) else { return };
        raw.focused = true;
        let (f, n) = (self.frames_in, item.frames.max(2));
        let press = |pos: Pos2, pressed: bool| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        match item.input {
            Input::Sweep(z) => {
                let r = self.zone(app, z);
                let rows = 12.0;
                let s = f as f32 / n as f32 * rows;
                let row = s.floor();
                let along = s - row;
                let x = if (row as u32).is_multiple_of(2) { r.min.x + along * r.width() } else { r.max.x - along * r.width() };
                let y = r.min.y + (row + 0.5) / rows * r.height();
                self.pointer = pos2(x, y);
                raw.events.push(Event::PointerMoved(self.pointer));
            }
            Input::Wheel(z) => {
                self.pointer = self.zone(app, z).center();
                raw.events.push(Event::PointerMoved(self.pointer));
                raw.events.push(Event::MouseWheel { unit: MouseWheelUnit::Line, delta: vec2(0.0, -3.0), phase: TouchPhase::Move, modifiers: Modifiers::NONE });
            }
            Input::Drag => {
                let g = app.explore.geom;
                let (top, bottom) = (g.bar.min.y + 12.0, g.bar.max.y - 2.0);
                let x = g.bar.center().x;
                if f == 0 {
                    self.pointer = pos2(x, top);
                    raw.events.push(Event::PointerMoved(self.pointer));
                    raw.events.push(press(self.pointer, true));
                } else if f + 1 < n {
                    let t = f as f32 / (n - 2) as f32;
                    self.pointer = pos2(x, top + (bottom - top) * t);
                    raw.events.push(Event::PointerMoved(self.pointer));
                } else {
                    raw.events.push(press(self.pointer, false));
                }
            }
        }
    }

    /// Inizio di un frame, prima di `App::show`.
    pub fn frame(&mut self, app: &mut App, ctx: &egui::Context) {
        let now = Instant::now();
        let interval = self.last.map(|l| ms(now - l));
        self.last = Some(now);
        self.rect = ctx.content_rect();
        if let (Some(i), Some(part)) = (interval, self.prev) {
            match part {
                Part::Enter => {
                    if self.first_ms.is_none() {
                        self.first_ms = Some(i);
                    }
                    self.enter.push(i);
                }
                Part::Steady => self.steady.push(i),
            }
        }
        self.prev = None;
        if self.finished {
            return;
        }
        // Scansione: nessun ridisegno forzato, l'app ridisegna ogni 100 ms.
        if !self.scan_seen {
            if app.job.is_some() {
                return;
            }
            let Some(d) = app.data.as_ref() else {
                self.line("errore\tscansione non riuscita");
                self.finish(app);
                return;
            };
            self.scan_seen = true;
            let scan = d.duration;
            let since = self.scan_started.map_or(f64::NAN, |s| ms(now - s));
            let line = format!(
                "scansione\tdurata-ms\t{:.0}\nscansione\tdall-avvio-ai-dati-ms\t{since:.0}\nscansione\tfine-scansione-ai-dati-ms\t{:.0}\nscansione\tframe-del-caricamento-ms\t{:.1}\nscansione\tnodi\t{}",
                ms(scan),
                since - ms(scan),
                interval.unwrap_or(f64::NAN),
                d.tree().len()
            );
            self.line(&line);
            ctx.request_repaint();
            return;
        }
        // Primi frame dopo il caricamento: controllo dei font e prima Panoramica.
        if self.after_load.len() < 6 {
            if let Some(i) = interval {
                self.after_load.push(i);
            }
            if self.after_load.len() == 6 {
                let s: Vec<String> = self.after_load.iter().map(|x| format!("{x:.1}")).collect();
                let l = format!("scansione\tframe-dopo-il-caricamento-ms\t{}", s.join(" "));
                self.line(&l);
            }
            ctx.request_repaint();
            return;
        }
        ctx.request_repaint();
        let Some(item) = self.items.get(self.index) else {
            self.finish(app);
            return;
        };
        let (prepare, ready, frames) = (item.prepare, item.ready, item.frames);
        match (self.prepared, self.ready_at) {
            (None, _) => {
                self.notes.text.clear();
                self.notes.started = None;
                prepare(app, &mut self.notes);
                self.prepared = Some(Instant::now());
                self.prev = Some(Part::Enter);
            }
            (Some(p), None) => {
                // Almeno tre frame di entrata: la schermata nuova si disegna e i lavori partono.
                let outcome = if self.enter.len() < 3 { Ready::Wait } else { ready(app, &mut self.notes) };
                match outcome {
                    Ready::Wait if p.elapsed() > Duration::from_secs(600) => {
                        self.notes.text.push("tempo scaduto".into());
                        self.close_item(f64::NAN);
                    }
                    Ready::Wait => self.prev = Some(Part::Enter),
                    Ready::Skip(why) => {
                        self.notes.text.push(format!("saltata: {why}"));
                        self.close_item(f64::NAN);
                    }
                    Ready::Go => {
                        self.ready_at = Some(now);
                        self.frames_in = 0;
                        self.prev = Some(Part::Steady);
                    }
                }
            }
            (Some(p), Some(r)) => {
                self.frames_in += 1;
                if self.frames_in >= frames {
                    self.close_item(ms(r - p));
                } else {
                    self.prev = Some(Part::Steady);
                }
            }
        }
    }

    fn close_item(&mut self, ready_ms: f64) {
        let name = self.items[self.index].name;
        let d = Done {
            name,
            ready_ms,
            first_ms: self.first_ms.unwrap_or(f64::NAN),
            enter: std::mem::take(&mut self.enter),
            steady: std::mem::take(&mut self.steady),
            notes: std::mem::take(&mut self.notes.text),
        };
        let e = summarize(&d.enter);
        let s = summarize(&d.steady);
        let line = format!(
            "fase\t{}\tpronta-ms\t{:.0}\tprimo-frame-ms\t{:.1}\tentrata-n\t{}\tentrata-max-ms\t{:.1}\tregime-n\t{}\tp50\t{:.2}\tp95\t{:.2}\tp99\t{:.2}\tmax\t{:.1}\tmedia\t{:.2}\tnote\t{}",
            d.name,
            d.ready_ms,
            d.first_ms,
            e.n,
            e.max,
            s.n,
            s.p50,
            s.p95,
            s.p99,
            s.max,
            s.mean,
            d.notes.join("; ")
        );
        self.line(&line);
        self.done.push(d);
        self.index += 1;
        self.prepared = None;
        self.ready_at = None;
        self.first_ms = None;
        self.frames_in = 0;
    }

    fn finish(&mut self, app: &App) {
        if self.finished {
            return;
        }
        self.finished = true;
        let p = process_metrics();
        let l = format!(
            "fine\tdurata-s\t{:.1}\nmemoria\tworking-set\t{}\nmemoria\tpicco\t{}\nmemoria\tprivata\t{}\ncpu\tutente-ms\t{:.0}\ncpu\tkernel-ms\t{:.0}\nhandle\t{}\nframe\ttotali\t{}",
            self.started.elapsed().as_secs_f64(),
            p.working_set,
            p.peak_working_set,
            p.private,
            p.user.as_secs_f64() * 1000.0,
            p.kernel.as_secs_f64() * 1000.0,
            p.handles,
            app.frame_count
        );
        self.line(&l);
        if let Some(dir) = self.out.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&self.out, &self.lines) {
            eprintln!("risultati non scritti: {e}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Riposo

#[derive(Clone, Copy, Debug)]
struct Snapshot {
    at: Instant,
    frames: u64,
    cpu_ms: f64,
    user_ms: f64,
}

fn snapshot(app: &App) -> Snapshot {
    let p = process_metrics();
    Snapshot {
        at: Instant::now(),
        frames: app.frame_count,
        cpu_ms: (p.user + p.kernel).as_secs_f64() * 1000.0,
        user_ms: p.user.as_secs_f64() * 1000.0,
    }
}

struct Rest {
    name: &'static str,
    screen: Option<Screen>,
    settle: Duration,
    idle: Duration,
}

enum RestState {
    /// Fase da avviare.
    Start,
    /// Attesa che i lavori della schermata finiscano.
    Settling(Instant),
    /// Misura in corso.
    Idle(Snapshot),
    /// Scansione in corso.
    Scanning(Snapshot),
}

pub struct Riposo {
    out: PathBuf,
    phases: Vec<Rest>,
    index: usize,
    state: RestState,
    lines: String,
    pub finished: bool,
    scanned: bool,
    /// Frame della misura in corso in cui ciascun lavoro in background era attivo (chi chiede i ridisegni).
    busy: [u32; 8],
}

/// Lavori in background che chiedono ridisegni a intervalli: lente Download (lente o Restart Manager),
/// procedure e Cestino, intervento, File grandi, ricerca, scansione; più i frame con eventi di input
/// (mouse o tastiera dell'utente durante la misura).
const BUSY: [&str; 8] = ["lente-download", "uso-dei-download", "procedure-o-cestino", "intervento", "file-grandi", "ricerca", "scansione", "input-utente"];

fn busy_now(app: &App, ctx: &egui::Context) -> [bool; 8] {
    [
        app.downloads.pending(),
        app.downloads.checking_usage(),
        app.procedures.busy(),
        app.intervention.busy(),
        app.big.pending(),
        app.search.pending(),
        app.job.is_some(),
        ctx.input(|i| !i.events.is_empty()),
    ]
}

impl Riposo {
    pub fn new(app: &mut App, out: PathBuf, seconds: u64) -> Self {
        excluding_readers(app);
        let s = Duration::from_secs;
        let short = s(seconds.clamp(5, 15));
        let phases = vec![
            Rest { name: "unita-prima-della-scansione", screen: None, settle: s(3), idle: short },
            Rest { name: "panoramica", screen: Some(Screen::Overview), settle: s(5), idle: s(seconds) },
            Rest { name: "esplora", screen: Some(Screen::Explore), settle: s(3), idle: short },
            Rest { name: "file-grandi", screen: Some(Screen::BigFiles), settle: s(3), idle: short },
            Rest { name: "download", screen: Some(Screen::Downloads), settle: s(5), idle: short },
            Rest { name: "programmi", screen: Some(Screen::Programs), settle: s(3), idle: short },
            Rest { name: "interventi", screen: Some(Screen::Interventions), settle: s(3), idle: short },
            Rest { name: "sistema", screen: Some(Screen::System), settle: s(3), idle: short },
            Rest { name: "impostazioni", screen: Some(Screen::Settings), settle: s(3), idle: short },
        ];
        Self { out, phases, index: 0, state: RestState::Start, lines: String::new(), finished: false, scanned: false, busy: [0; 8] }
    }

    fn line(&mut self, s: &str) {
        println!("{s}");
        self.lines.push_str(s);
        self.lines.push('\n');
    }

    pub fn frame(&mut self, app: &mut App, ctx: &egui::Context) {
        if self.finished {
            return;
        }
        let now = Instant::now();
        match std::mem::replace(&mut self.state, RestState::Start) {
            RestState::Scanning(s0) => {
                if app.job.is_some() {
                    self.state = RestState::Scanning(s0);
                    return;
                }
                let s1 = snapshot(app);
                let p = process_metrics();
                let l = format!(
                    "scansione\tdurata-ms\t{:.0}\tframe\t{}\tcpu-ms\t{:.0}\tworking-set\t{}\tpicco\t{}\tprivata\t{}",
                    ms(s1.at - s0.at),
                    s1.frames - s0.frames,
                    s1.cpu_ms - s0.cpu_ms,
                    p.working_set,
                    p.peak_working_set,
                    p.private
                );
                self.line(&l);
                self.scanned = true;
                self.index += 1;
                self.state = RestState::Start;
                ctx.request_repaint();
            }
            RestState::Start => {
                let Some(ph) = self.phases.get(self.index) else {
                    self.finish();
                    return;
                };
                if let Some(s) = ph.screen {
                    app.go(s);
                }
                let settle = ph.settle;
                self.state = RestState::Settling(now + settle);
                ctx.request_repaint_after(settle);
            }
            RestState::Settling(until) => {
                if now < until {
                    self.state = RestState::Settling(until);
                    ctx.request_repaint_after(until - now);
                    return;
                }
                let idle = self.phases[self.index].idle;
                self.state = RestState::Idle(snapshot(app));
                self.busy = [0; 8];
                ctx.request_repaint_after(idle);
            }
            RestState::Idle(s0) => {
                let (name, idle) = (self.phases[self.index].name, self.phases[self.index].idle);
                for (k, b) in busy_now(app, ctx).into_iter().enumerate() {
                    self.busy[k] += u32::from(b);
                }
                let elapsed = now - s0.at;
                if elapsed < idle {
                    self.state = RestState::Idle(s0);
                    ctx.request_repaint_after(idle - elapsed);
                    return;
                }
                let s1 = snapshot(app);
                let secs = (s1.at - s0.at).as_secs_f64();
                // Il frame di adesso è quello del risveglio: non conta.
                let frames = s1.frames.saturating_sub(s0.frames + 1);
                let p = process_metrics();
                let l = format!(
                    "riposo\t{}\tsecondi\t{secs:.1}\tframe\t{frames}\tframe-al-secondo\t{:.2}\tcpu-ms\t{:.0}\tcpu-percento-di-un-core\t{:.3}\tutente-ms\t{:.0}\tworking-set\t{}\tprivata\t{}\thandle\t{}",
                    name,
                    frames as f64 / secs,
                    s1.cpu_ms - s0.cpu_ms,
                    (s1.cpu_ms - s0.cpu_ms) / (secs * 1000.0) * 100.0,
                    s1.user_ms - s0.user_ms,
                    p.working_set,
                    p.private,
                    p.handles
                );
                self.line(&l);
                let who: Vec<String> = BUSY.iter().zip(self.busy).filter(|(_, n)| *n > 0).map(|(k, n)| format!("{k} {n}")).collect();
                let l = format!("lavori\t{}\tframe-con-lavori-attivi\t{}", name, if who.is_empty() { "nessuno".to_owned() } else { who.join(", ") });
                self.line(&l);
                if !self.scanned {
                    // Dopo la prima fase (finestra senza dati): la scansione del volume di sistema.
                    match system_volume(app) {
                        Some(v) => {
                            app.start_scan(v);
                            self.state = RestState::Scanning(snapshot(app));
                        }
                        None => {
                            self.line("errore\tnessun volume di sistema");
                            self.finish();
                        }
                    }
                    return;
                }
                self.index += 1;
                self.state = RestState::Start;
                ctx.request_repaint();
            }
        }
    }

    fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        let p = process_metrics();
        let mut l = String::new();
        let _ = write!(l, "fine\tworking-set\t{}\tpicco\t{}\tprivata\t{}\thandle\t{}", p.working_set, p.peak_working_set, p.private, p.handles);
        self.line(&l);
        if let Some(dir) = self.out.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&self.out, &self.lines) {
            eprintln!("risultati non scritti: {e}");
        }
    }
}
