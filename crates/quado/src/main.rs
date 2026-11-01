// SPDX-License-Identifier: GPL-3.0-or-later
//! Quado: analizzatore dello spazio su disco. La scansione legge soltanto; Quado sposta file nel Cestino solo dopo
//! l'anteprima e la conferma.
//!
//! ```text
//! quado [--renderer auto|warp|gl]
//! quado --misura-avvio FILE        primo frame interattivo (06 §9), scritto in FILE, poi esce
//! quado --banco TIPO [--out DIR]   banco di misura di M2 (tabella, figli, mappa, scansione); uscita
//!                                   predefinita `target/misure`
//! quado --banco installatori|hash --cartella DIR [--escludi "a;b"] [--giri N] [--thread N] [--out DIR]
//!                                   banco della lente Download (M4): frame durante le letture
//! quado --immagini DIR             scansiona il volume di sistema e salva le schermate
//! quado --diagnostica-moduli       moduli caricati (prova delle DLL, 07 §3 riga 5)
//! quado --misura-schermate FILE    giro delle schermate su C: con i tempi di frame (baseline delle prestazioni)
//! quado --misura-riposo FILE [--secondi N]
//!                                   app ferma dopo la scansione di C:: frame, CPU e memoria
//! quado --misura-wgpu              costo di istanza, elenco degli adattatori e dispositivo di wgpu
//! quado ... --wgpu-limiti egui|app|N --wgpu-memoria prestazioni|uso
//!                                   dispositivo wgpu diverso da quello dell'app, per le misure
//! quado --sessione-prova           sessione guidata automatica per la prova senza rete (06 §10):
//!                                   passi su standard output, nessun file scritto fuori da %APPDATA%\Quado
//! quado --helper PIPE PID          helper elevato (05 D3): lo avvia Quado stesso con UAC
//! ```
//!
//! Con `--sessione-prova`, `--misura-schermate` e `--misura-riposo`, `--escludi "a;b"` nomina parti di
//! percorso che le letture della lente non aprono mai (senza distinzione di maiuscole).
//!
//! Le opzioni di misura e di prova (tutte tranne `--renderer` e `--helper`) esistono solo con la feature
//! `misure`, che `cargo xtask` attiva quando misura o prova. L'eseguibile distribuito (`cargo xtask
//! installa`, profilo `dist`, release) non le ha: scrive solo in `%APPDATA%\Quado` e risponde
//! «argomento sconosciuto» a ogni altra opzione.
//!
//! Catena del renderer (05 D2): wgpu su DX12 con la scheda video che guida il monitor principale
//! (su un portatile con due schede video, disegnare con l'altra costava 7-9 ms di copia per frame);
//! `warp` forza l'adattatore software; se wgpu non parte, OpenGL (glow).

#[cfg(feature = "misure")]
mod misure;
#[cfg(feature = "misure")]
mod session;
#[cfg(feature = "misure")]
mod shots;

/// Senza la feature `misure` le tappe dell'avvio non registrano niente.
#[cfg(not(feature = "misure"))]
mod misure {
    pub mod avvio {
        pub fn start() {}
        pub fn mark(_name: &'static str) {}
    }
}

use std::cell::Cell;
#[cfg(feature = "misure")]
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use eframe::egui;
use eframe::egui_wgpu::{WgpuSetup, wgpu};
use quado_scan_win::{process, system, volumes, window};
#[cfg(feature = "misure")]
use quado_ui::app::bench::{Bench, Kind, LensArgs};
use quado_ui::app::{App, Env};

/// Esegue un future di wgpu nativo, che è già pronto (nessun lavoro asincrono vero su Windows).
pub fn block_on<F: std::future::Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Renderer {
    Auto,
    Warp,
    Gl,
}

/// Il dispositivo wgpu che si chiede: limiti e strategia di memoria. Quello dell'app per default;
/// `--wgpu-limiti egui|N` e `--wgpu-memoria prestazioni|uso` servono alle misure (`--misura-avvio`,
/// `--misura-wgpu`) per confrontarlo con quello predefinito di egui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceChoice {
    /// `max_non_sampler_bindings`: descrittori nell'heap visibile agli shader di Direct3D 12, che wgpu crea
    /// tutto insieme con il dispositivo. `None`: il valore predefinito di wgpu (un milione).
    pub views: Option<u32>,
    /// Blocchi di memoria piccoli (`MemoryHints::MemoryUsage`) invece di quelli da 64-256 MiB.
    pub small_blocks: bool,
}

impl DeviceChoice {
    /// Quello di egui (limiti predefiniti, memoria per le prestazioni): l'app prima delle misure dell'avvio.
    pub const EGUI: Self = Self { views: None, small_blocks: false };
    /// Quello dell'app: egui disegna con una manciata di texture, e ogni texture usa un descrittore.
    pub const APP: Self = Self { views: Some(APP_VIEWS), small_blocks: true };
}

/// Descrittori per l'app: margine di centinaia di volte sulle texture che egui tiene insieme.
const APP_VIEWS: u32 = 16_384;

/// Elenco anticipato degli adattatori, in un altro thread dall'inizio di `main`: carica i driver Direct3D 12
/// (circa 400 ms quello della NVIDIA) mentre eframe crea la finestra. L'elenco che egui-wgpu fa poi, sempre
/// su tutte le schede, trova i driver già in memoria: 4 ms invece di 420 (`--misura-wgpu`, elenco di nuovo).
/// Gli adattatori anticipati tengono vivi i driver fino alla creazione dell'app, poi si rilasciano.
fn early_adapters() -> Option<std::thread::JoinHandle<Vec<wgpu::Adapter>>> {
    std::thread::Builder::new()
        .name("quado-driver-video".into())
        .spawn(|| {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::DX12, ..wgpu::InstanceDescriptor::new_without_display_handle() });
            let adapters = block_on(instance.enumerate_adapters(wgpu::Backends::DX12));
            misure::avvio::mark("adattatori-anticipati");
            adapters
        })
        .ok()
}

/// Descrittore del dispositivo: limiti di egui (texture fino a 8192 per i monitor 4K e oltre) con le scelte
/// di `choice`.
pub fn device_descriptor(choice: DeviceChoice, adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
    let base = if adapter.get_info().backend == wgpu::Backend::Gl { wgpu::Limits::downlevel_webgl2_defaults() } else { wgpu::Limits::default() };
    let mut required_limits = wgpu::Limits { max_texture_dimension_2d: 8192, ..base };
    if let Some(v) = choice.views {
        required_limits.max_non_sampler_bindings = v.min(adapter.limits().max_non_sampler_bindings);
    }
    let memory_hints = if choice.small_blocks { wgpu::MemoryHints::MemoryUsage } else { wgpu::MemoryHints::Performance };
    wgpu::DeviceDescriptor { label: Some("egui wgpu device"), required_limits, memory_hints, ..Default::default() }
}

struct Args {
    renderer: Renderer,
    device: DeviceChoice,
    /// Elenco anticipato degli adattatori (`early_adapters`); `--senza-elenco-anticipato` per le misure.
    early: bool,
    /// Opzioni di misura e di prova: solo con la feature `misure`.
    #[cfg(feature = "misure")]
    m: Measure,
}

/// Opzioni di misura e di prova (feature `misure`).
#[cfg(feature = "misure")]
struct Measure {
    measure_start: Option<PathBuf>,
    bench: Option<Kind>,
    out: PathBuf,
    shots: Option<PathBuf>,
    session: bool,
    lens: LensArgs,
    screens: Option<PathBuf>,
    rest: Option<PathBuf>,
    rest_secs: u64,
    wgpu_diag: bool,
}

#[cfg(feature = "misure")]
impl Default for Measure {
    fn default() -> Self {
        Self {
            measure_start: None,
            bench: None,
            out: PathBuf::from("target/misure"),
            shots: None,
            session: false,
            lens: LensArgs::default(),
            screens: None,
            rest: None,
            rest_secs: 60,
            wgpu_diag: false,
        }
    }
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        renderer: Renderer::Auto,
        device: DeviceChoice::APP,
        early: true,
        #[cfg(feature = "misure")]
        m: Measure::default(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{arg}: manca il valore"));
        match arg.as_str() {
            "--renderer" => {
                a.renderer = match value()?.as_str() {
                    "auto" | "wgpu" => Renderer::Auto,
                    "warp" => Renderer::Warp,
                    "gl" | "glow" => Renderer::Gl,
                    v => return Err(format!("--renderer {v}: valori ammessi auto, warp, gl")),
                }
            }
            #[cfg(feature = "misure")]
            "--misura-avvio" => a.m.measure_start = Some(PathBuf::from(value()?)),
            #[cfg(feature = "misure")]
            "--banco" => a.m.bench = Some(Kind::parse(&value()?).ok_or("--banco: tabella, figli, mappa, scansione, installatori, hash")?),
            #[cfg(feature = "misure")]
            "--cartella" => a.m.lens.folder = Some(value()?),
            #[cfg(feature = "misure")]
            "--escludi" => a.m.lens.exclude = value()?.split(';').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect(),
            #[cfg(feature = "misure")]
            "--giri" => a.m.lens.rounds = value()?.parse().map_err(|_| "--giri: un numero")?,
            #[cfg(feature = "misure")]
            "--thread" => a.m.lens.threads = value()?.parse().map_err(|_| "--thread: un numero")?,
            #[cfg(feature = "misure")]
            "--out" => a.m.out = PathBuf::from(value()?),
            #[cfg(feature = "misure")]
            "--immagini" => a.m.shots = Some(PathBuf::from(value()?)),
            #[cfg(feature = "misure")]
            "--sessione-prova" => a.m.session = true,
            #[cfg(feature = "misure")]
            "--misura-schermate" => a.m.screens = Some(PathBuf::from(value()?)),
            #[cfg(feature = "misure")]
            "--misura-riposo" => a.m.rest = Some(PathBuf::from(value()?)),
            #[cfg(feature = "misure")]
            "--secondi" => a.m.rest_secs = value()?.parse().map_err(|_| "--secondi: un numero")?,
            #[cfg(feature = "misure")]
            "--misura-wgpu" => a.m.wgpu_diag = true,
            #[cfg(feature = "misure")]
            "--senza-elenco-anticipato" => a.early = false,
            #[cfg(feature = "misure")]
            "--wgpu-limiti" => {
                a.device.views = match value()?.as_str() {
                    "egui" => None,
                    "app" => Some(APP_VIEWS),
                    v => Some(v.parse().map_err(|_| "--wgpu-limiti: egui, app o un numero di descrittori")?),
                }
            }
            #[cfg(feature = "misure")]
            "--wgpu-memoria" => {
                a.device.small_blocks = match value()?.as_str() {
                    "prestazioni" => false,
                    "uso" => true,
                    v => return Err(format!("--wgpu-memoria {v}: valori ammessi prestazioni, uso")),
                }
            }
            #[cfg(feature = "misure")]
            "--diagnostica-moduli" => {}
            other => return Err(format!("argomento sconosciuto: {other}")),
        }
    }
    #[cfg(feature = "misure")]
    if a.m.bench.is_some_and(Kind::is_lens) && a.m.lens.folder.is_none() {
        return Err("--banco installatori e hash: manca --cartella".into());
    }
    Ok(a)
}

/// Registro minimo su standard error.
struct StderrLog;

impl log::Log for StderrLog {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Warn || (m.level() <= log::Level::Info && m.target().starts_with("quado"))
    }
    fn log(&self, r: &log::Record<'_>) {
        if self.enabled(r.metadata()) {
            eprintln!("[{}] {}: {}", r.level(), r.target(), r.args());
        }
    }
    fn flush(&self) {}
}

static LOGGER: StderrLog = StderrLog;

/// Scheda video del monitor principale, dove si apre la finestra.
fn display_adapter() -> Option<system::DisplayAdapter> {
    system::adapter_for_point(0, 0)
}

fn native_options(args: &Args, renderer: Renderer, bench: bool) -> eframe::NativeOptions {
    // Icona vuota: niente icona di egui. Le icone della finestra vengono dalle risorse dell'eseguibile
    // (`Shell::window_icons`), con un'immagine per ogni misura invece di una sola ridimensionata.
    let mut viewport = egui::ViewportBuilder::default().with_title(quado_ui::brand::NAME).with_icon(Arc::new(egui::IconData::default())).with_min_inner_size([960.0, 600.0]);
    viewport = if bench {
        viewport.with_position([0.0, 0.0]).with_inner_size([1920.0, 1080.0]).with_decorations(false)
    } else {
        viewport.with_inner_size([1600.0, 1000.0])
    };
    let mut native = eframe::NativeOptions {
        viewport,
        renderer: if renderer == Renderer::Gl { eframe::Renderer::Glow } else { eframe::Renderer::Wgpu },
        ..Default::default()
    };
    native.wgpu_options.surface.present_mode = if bench { wgpu::PresentMode::AutoNoVsync } else { wgpu::PresentMode::AutoVsync };
    native.glow_options.vsync = !bench;
    if let WgpuSetup::CreateNew(setup) = &mut native.wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = wgpu::Backends::DX12;
        let choice = args.device;
        setup.device_descriptor = Arc::new(move |adapter| {
            // Tappa dell'avvio: adattatore scelto, egui-wgpu chiede il dispositivo.
            misure::avvio::mark("dispositivo");
            device_descriptor(choice, adapter)
        });
        let want = display_adapter();
        let warp = renderer == Renderer::Warp;
        setup.native_adapter_selector = Some(Arc::new(move |adapters, _surface| {
            // Tappa dell'avvio: elenco degli adattatori finito (egui-wgpu lo fa prima di chiamare la scelta).
            misure::avvio::mark("adattatori");
            if warp {
                return adapters
                    .iter()
                    .find(|a| a.get_info().device_type == wgpu::DeviceType::Cpu)
                    .cloned()
                    .ok_or_else(|| "nessun adattatore software (WARP)".to_owned());
            }
            let hw = |a: &&wgpu::Adapter| a.get_info().device_type != wgpu::DeviceType::Cpu;
            if let Some(w) = &want
                && let Some(a) = adapters.iter().filter(hw).find(|a| {
                    let i = a.get_info();
                    i.vendor == w.vendor && i.device == w.device
                })
            {
                return Ok(a.clone());
            }
            adapters.iter().find(hw).or_else(|| adapters.first()).cloned().ok_or_else(|| "nessun adattatore".to_owned())
        }));
    }
    native
}

struct Shell {
    app: App,
    /// Frame disegnati, per le tappe dell'avvio.
    frames: u32,
    /// Icone della finestra messe: scala del monitor (`window_icons`).
    icons: Option<u32>,
    /// Misure e prove in corso (feature `misure`).
    #[cfg(feature = "misure")]
    m: Running,
}

/// Misure e prove in corso, con la feature `misure`.
#[cfg(feature = "misure")]
struct Running {
    measure_start: Option<PathBuf>,
    bench: Option<Bench>,
    shots: Option<shots::Shots>,
    session: Option<session::Session>,
    giro: Option<misure::Giro>,
    riposo: Option<misure::Riposo>,
    /// Tappe dell'avvio da scrivere al frame dopo il primo frame interattivo.
    marks_pending: bool,
}

impl Shell {
    /// Icone della finestra dalle risorse dell'eseguibile: di nuovo quando cambia la scala del monitor (le
    /// misure delle icone).
    fn window_icons(&mut self, ctx: &egui::Context) {
        let Some(hwnd) = self.app.env.owner else { return };
        let scale = ctx.native_pixels_per_point().map_or(100, |p| (p * 100.0).round() as u32);
        if self.icons != Some(scale) {
            self.icons = Some(scale);
            if !window::set_icons(hwnd) {
                log::warn!("icone della finestra non caricate dalle risorse dell'eseguibile");
            }
        }
    }
}

impl eframe::App for Shell {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.frames += 1;
        if let Some(&name) = ["frame-1", "frame-2", "frame-3"].get(self.frames as usize - 1) {
            misure::avvio::mark(name);
        }
        #[cfg(feature = "misure")]
        if self.m.marks_pending {
            self.m.marks_pending = false;
            print!("{}", misure::avvio::report());
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // La finestra di Quado è la proprietaria dei dialoghi di Windows degli interventi (M5).
        if self.app.env.owner.is_none()
            && let Ok(h) = raw_window_handle::HasWindowHandle::window_handle(frame)
            && let raw_window_handle::RawWindowHandle::Win32(w) = h.as_raw()
        {
            self.app.env.owner = Some(w.hwnd.get());
        }
        self.window_icons(ui.ctx());
        #[cfg(feature = "misure")]
        self.measures_before(ui);
        self.app.show(ui);
        if self.frames == 1 {
            misure::avvio::mark("frame-1-fine");
        }
        #[cfg(feature = "misure")]
        self.measures_after(ui);
    }

    #[cfg(feature = "misure")]
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(mut b) = self.m.bench.take() {
            b.input(&self.app, raw);
            self.m.bench = Some(b);
        }
        if let Some(g) = self.m.giro.as_mut() {
            g.input(&self.app, raw);
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

#[cfg(feature = "misure")]
impl Shell {
    /// Banchi, schermate, sessione di prova e misure prima di disegnare l'app.
    fn measures_before(&mut self, ui: &mut egui::Ui) {
        let m = &mut self.m;
        if let Some(mut b) = m.bench.take() {
            b.frame(&mut self.app, ui.ctx());
            m.bench = Some(b);
        }
        if let Some(mut s) = m.shots.take() {
            s.frame(&mut self.app, ui.ctx());
            if s.finished {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            m.shots = Some(s);
        }
        if let Some(mut s) = m.session.take() {
            s.frame(&mut self.app, ui.ctx());
            if s.finished {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            m.session = Some(s);
        }
        if let Some(mut g) = m.giro.take() {
            g.frame(&mut self.app, ui.ctx());
            if g.finished {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            m.giro = Some(g);
        }
        if let Some(mut r) = m.riposo.take() {
            r.frame(&mut self.app, ui.ctx());
            if r.finished {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            m.riposo = Some(r);
        }
    }

    /// Fine dei banchi e misura del primo frame interattivo, dopo aver disegnato l'app.
    fn measures_after(&mut self, ui: &mut egui::Ui) {
        let m = &mut self.m;
        if let Some(b) = &m.bench {
            ui.ctx().request_repaint();
            if b.finished {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if let Some(path) = m.measure_start.take() {
            if self.app.first_interactive.is_some() {
                let ms = process::ms_since_start();
                let line = format!("{}\t{:.0}\n", system::now_filetime(), ms);
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                use std::io::Write as _;
                let ok = std::fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| f.write_all(line.as_bytes()));
                if let Err(e) = ok {
                    log::warn!("misura dell'avvio non scritta: {e}");
                }
                println!("primo-frame-ms\t{ms:.0}");
                // Le tappe dell'avvio al frame dopo, che misura anche il disegno di questo.
                misure::avvio::mark("interattivo");
                m.marks_pending = true;
                ui.ctx().request_repaint();
            } else {
                m.measure_start = Some(path);
            }
        }
    }
}

fn creator(args: &Args, started: Rc<Cell<bool>>, renderer: Renderer, early: Option<std::thread::JoinHandle<Vec<wgpu::Adapter>>>) -> eframe::AppCreator<'static> {
    #[cfg(feature = "misure")]
    let (measure, bench, shots_dir, session, screens, rest) = (
        args.m.measure_start.clone(),
        args.m.bench.map(|k| (k, args.m.out.clone(), args.m.lens.clone())),
        args.m.shots.clone(),
        args.m.session,
        args.m.screens.clone(),
        args.m.rest.clone().map(|p| (p, args.m.rest_secs)),
    );
    #[cfg(not(feature = "misure"))]
    let _ = args;
    Box::new(move |cc| {
        misure::avvio::mark("creatore");
        started.set(true);
        // Il dispositivo dell'app c'è: i driver restano caricati anche senza gli adattatori anticipati.
        drop(early.and_then(|h| h.join().ok()));
        let label = match &cc.wgpu_render_state {
            Some(state) => {
                let i = state.adapter.get_info();
                format!("wgpu {:?} · {}", i.backend, i.name)
            }
            None => "OpenGL (glow)".to_owned(),
        };
        log::info!(target: "quado", "renderer: {label}");
        let env = Env::from_system(label.clone());
        misure::avvio::mark("ambiente");
        let mut app = App::new(env, volumes::list());
        // I font prima del primo frame: il primo frame disegna già la schermata.
        app.install_fonts(&cc.egui_ctx);
        misure::avvio::mark("app");
        #[cfg(feature = "misure")]
        let m = Running {
            bench: bench.map(|(k, out, lens)| Bench::new(k, out, &label, &mut app, &lens)),
            shots: shots_dir.map(|d| shots::Shots::new(d, &mut app)),
            session: session.then(|| session::Session::new(&mut app)),
            giro: screens.map(|out| misure::Giro::new(&mut app, out)),
            riposo: rest.map(|(out, secs)| misure::Riposo::new(&mut app, out, secs)),
            measure_start: measure,
            marks_pending: false,
        };
        let _ = renderer;
        Ok(Box::new(Shell {
            app,
            frames: 0,
            icons: None,
            #[cfg(feature = "misure")]
            m,
        }))
    })
}

fn main() -> eframe::Result {
    // Prima chiamata (06 §11). L'esito lo stampa solo `--diagnostica-moduli` (feature `misure`).
    let hardened = process::harden_dll_search();
    #[cfg(not(feature = "misure"))]
    let _ = hardened;
    // Modalità helper (05 D3): niente finestra, niente registro su stderr; il codice di uscita dice l'esito.
    let all: Vec<String> = std::env::args().collect();
    if all.get(1).is_some_and(|a| a == "--helper") {
        std::process::exit(quado_scan_win::helper::serve::run(&all[2..]));
    }
    #[cfg(feature = "misure")]
    if std::env::args().any(|a| a == "--diagnostica-moduli") {
        let _ = process::file_version_info_size(r"C:\Windows\System32\kernel32.dll");
        println!("dll-search-hardened\t{hardened}");
        for m in process::loaded_modules() {
            println!("modulo\t{m}");
        }
        return Ok(());
    }
    misure::avvio::start();
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Info));
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    // Parti di percorso escluse dalle letture della sessione di prova e delle misure (`--escludi`).
    #[cfg(feature = "misure")]
    session::set_excluded(&args.m.lens.exclude);
    misure::avvio::mark("argomenti");
    #[cfg(feature = "misure")]
    let wgpu_diag = args.m.wgpu_diag;
    #[cfg(not(feature = "misure"))]
    let wgpu_diag = false;
    let early = (args.early && !wgpu_diag && args.renderer != Renderer::Gl).then(early_adapters).flatten();
    #[cfg(feature = "misure")]
    if wgpu_diag {
        misure::wgpu_diagnostic(display_adapter(), args.device);
        return Ok(());
    }
    // Il giro delle schermate usa la finestra dei banchi (1920x1080, sincronizzazione verticale spenta).
    #[cfg(feature = "misure")]
    let bench = args.m.bench.is_some() || args.m.screens.is_some();
    #[cfg(not(feature = "misure"))]
    let bench = false;
    let started = Rc::new(Cell::new(false));
    let options = native_options(&args, args.renderer, bench);
    misure::avvio::mark("opzioni");
    let result = eframe::run_native(quado_ui::brand::NAME, options, creator(&args, Rc::clone(&started), args.renderer, early));
    match result {
        // Solo se wgpu non è partito prima di creare l'app: allora OpenGL può ancora funzionare.
        Err(e @ eframe::Error::Wgpu(_)) if args.renderer == Renderer::Auto && !started.get() => {
            log::error!("wgpu non è partito: {e}. Riprovo con OpenGL.");
            eframe::run_native(quado_ui::brand::NAME, native_options(&args, Renderer::Gl, bench), creator(&args, started, Renderer::Gl, None))
        }
        other => other,
    }
}
