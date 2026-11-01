// SPDX-License-Identifier: GPL-3.0-or-later
//! L'applicazione (07 §1.3-1.4): Volumi, Scansione, Panoramica, Esplora, File grandi, Windows e
//! sistema, Regole, Impostazioni.
//!
//! `App::show` disegna un frame ed è la stessa funzione per eframe e per i test con `egui_kittest`.
//! Ogni testo visibile viene dai file di lingua (05 D13); numeri e date da `format` (F-GE-3).

pub mod bench;
pub mod big_files;
pub mod data;
pub mod downloads_view;
pub mod explore;
pub mod helper_jobs;
pub mod inspector;
pub mod intervention_jobs;
pub mod intervention_view;
pub mod interventions_view;
pub mod kbtext;
pub mod lens_jobs;
pub mod overview;
pub mod palette;
pub mod procedure_flow;
pub mod programs_view;
pub mod rules_view;
pub mod settings;
pub mod settings_view;
pub mod shell;
pub mod synthetic;
pub mod system_view;
pub mod table;
pub mod volumes_view;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::{CornerRadius, Key, Modifiers, Ui};
use quado_actions::legacy;
use quado_scan_win::ScanResult;
use quado_scan_win::system::RegionFormat;
use quado_scan_win::volumes::Volume;

use crate::fonts::{self, FontOptions};
use crate::format::{Formatter, UnitLabels};
use crate::i18n::{self, Lingue};
use crate::perf::Rolling;
use crate::tokens::{self, Accent, DataPalette, Motion, Palette, Theme, ThemeChoice};

use self::data::{Loaded, Notice, ScanJob, VerifyReport, VolumeInfo};
use self::settings::{ExploreView, Settings};

/// Fatti di Windows letti all'avvio (o fissati dai test).
#[derive(Clone, Debug)]
pub struct Env {
    pub region: RegionFormat,
    pub animations: bool,
    pub user_sid: Option<String>,
    pub windows_languages: Vec<String>,
    /// `%APPDATA%\Quado`; `None` nei test che non devono scrivere.
    pub data_dir: Option<PathBuf>,
    /// `%APPDATA%\Rilievo`, la cartella dei dati della versione precedente: Dati e privacy la mostra, se
    /// c'è, e la cancella dopo una conferma. `None` nei test che non la provano.
    pub old_data_dir: Option<PathBuf>,
    /// Copia dei dati di Rilievo al primo avvio, fatta prima di leggere le impostazioni (`from_system`).
    pub migration: legacy::Migration,
    /// Renderer e scheda video, per Impostazioni ▸ Informazioni.
    pub renderer: String,
    /// Carica i font di ripiego di Windows quando servono (falso nei test, per immagini stabili).
    pub system_fonts: bool,
    /// Letture su richiesta della lente Download: vere nell'app, nessuna in `fixed` (i test le iniettano).
    pub readers: lens_jobs::Readers,
    /// Azioni degli interventi (M5): vere nell'app, simulate in `fixed` (i test le iniettano).
    pub actions: intervention_jobs::Actions,
    /// Finestra di Quado (HWND), proprietaria dei dialoghi di Windows; `None` nei test.
    pub owner: Option<isize>,
    /// Helper elevato (M6, "Ripeti come amministratore"): vero nell'app, assente in `fixed` (i test lo
    /// iniettano finto).
    pub elevator: helper_jobs::Elevator,
    /// Quado è avviato come amministratore (processo elevato): letto da Windows nell'app, falso in `fixed`.
    /// Sceglie la voce della barra di stato e le frasi sulla scansione; dice com'è avviato Quado, non chi è
    /// l'utente.
    pub elevated: bool,
}

impl Env {
    /// Valori fissi per i test: formato italiano, animazioni spente, nessun file scritto.
    pub fn fixed() -> Self {
        Self {
            region: RegionFormat::default(),
            animations: false,
            user_sid: Some("S-1-5-21-1-2-3-1001".into()),
            windows_languages: vec!["it-IT".into()],
            data_dir: None,
            old_data_dir: None,
            migration: legacy::Migration::Nothing,
            renderer: String::new(),
            system_fonts: false,
            readers: lens_jobs::Readers::inert(),
            actions: intervention_jobs::Actions::inert(),
            owner: None,
            elevator: helper_jobs::Elevator::inert(),
            elevated: false,
        }
    }

    /// Valori letti da Windows. Al primo avvio dopo il cambio di nome copia i dati di Rilievo nella cartella
    /// di Quado, prima che `App::new` legga impostazioni e lingue.
    pub fn from_system(renderer: String) -> Self {
        use quado_scan_win::system as sys;
        let (data_dir, old_data_dir) = (settings::data_dir(), settings::old_data_dir());
        let migration = match (&old_data_dir, &data_dir) {
            (Some(old), Some(new)) => settings::migrate(old, new),
            _ => legacy::Migration::Nothing,
        };
        Self {
            region: sys::region_format(),
            animations: sys::animations_enabled(),
            user_sid: sys::user_sid(),
            windows_languages: sys::ui_languages(),
            data_dir,
            old_data_dir,
            migration,
            renderer,
            system_fonts: true,
            readers: lens_jobs::Readers::live(),
            actions: intervention_jobs::Actions::live(),
            owner: None,
            elevator: helper_jobs::Elevator::live(),
            // Se Windows non risponde vale la forma non elevata, quella di un avvio normale.
            elevated: quado_scan_win::process::elevated().unwrap_or(false),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Volumes,
    Scanning,
    Overview,
    Explore,
    BigFiles,
    /// Windows e sistema (F-SY-1, F-SY-3).
    System,
    /// Elenco delle regole (05 D6, 07 §5.1).
    Rules,
    /// Lente Download (F-DL).
    Downloads,
    /// Programmi (F-AP).
    Programs,
    /// Interventi: risultato dell'ultimo spostamento e storico dal registro (F-VE-5).
    Interventions,
    Settings,
}

/// Stato dei font: Inter e Plex Mono subito, i font di Windows solo se un nome li richiede e solo quelli che
/// servono (05 D13, M0).
#[derive(Default)]
struct FontState {
    installed: bool,
    /// Font di Windows installati come ripiego.
    fallbacks: Vec<fonts::SystemFont>,
    /// Caratteri dei nomi già controllati (`Loaded::name_chars`).
    checked: Option<std::sync::Arc<Vec<char>>>,
    /// Caratteri che nessun font di Windows copre: non si cercano di nuovo.
    uncovered: Vec<char>,
    /// Scelta e apertura dei font in un altro thread.
    job: Option<data::Task<fonts::Fallbacks>>,
    /// Frame dell'ultima installazione: i font valgono dal frame dopo.
    installed_at: u64,
}

pub struct App {
    pub env: Env,
    pub settings: Settings,
    pub lingue: Lingue,
    pub fmt: Formatter,
    pub volumes: Vec<Volume>,
    pub screen: Screen,
    pub job: Option<ScanJob>,
    /// Risultato della scansione che aspetta i font di ripiego dei suoi nomi: si mostra quando sono attivi,
    /// così la prima Panoramica dispone il testo una volta sola (con i font nuovi egui rifà l'atlante).
    waiting: Option<Loaded>,
    /// Ultima scansione fallita: volume e codice di Windows.
    pub scan_error: Option<(VolumeInfo, u32)>,
    pub data: Option<Loaded>,
    pub explore: explore::ExploreState,
    pub search: explore::SearchState,
    pub big: big_files::BigState,
    pub inspector: inspector::InspectorState,
    pub overview: overview::OverviewState,
    /// Palette dei comandi (Ctrl+K).
    pub palette: palette::PaletteState,
    pub rules_view: rules_view::RulesState,
    pub downloads: downloads_view::DownloadsState,
    pub programs: programs_view::ProgramsState,
    /// Spostamento nel Cestino in corso o concluso in questa sessione (M5).
    pub intervention: intervention_view::InterventionState,
    /// Procedure e comandi in attesa di "Ho eseguito la procedura" (M5).
    pub procedures: procedure_flow::ProcedureState,
    /// "Ripeti come amministratore" in corso o concluso (M6).
    pub helper: helper_jobs::HelperState,
    pub notice: Notice,
    fonts: FontState,
    styled: Option<Styled>,
    /// Barra laterale compressa aperta per la ricerca (Ctrl+F o il pulsante della ricerca).
    pub sidebar_peek: bool,
    /// Intervalli tra frame, per la barra di stato in modalità misura e per i banchi.
    pub frame_times: Rolling,
    last_frame: Option<Instant>,
    pub frame_count: u64,
    /// Mostra i tempi di frame nella barra di stato (F2).
    pub show_perf: bool,
    /// Valori di avanzamento diversi mostrati durante la scansione (≥ 4 al secondo, F-SC-4).
    pub progress_updates: Vec<(Instant, u64)>,
    /// Primo frame interattivo (font caricati e schermata disegnata).
    pub first_interactive: Option<Instant>,
    /// Frame in cui la lingua o le unità sono cambiate: le etichette si ricalcolano.
    lang_generation: u64,
    pub last_verify: Option<VerifyReport>,
    /// Base delle regole incorporata (M3).
    pub kb: std::sync::Arc<quado_kb::Kb>,
}

impl App {
    pub fn new(env: Env, volumes: Vec<Volume>) -> Self {
        let settings_path = env.data_dir.as_ref().map(|d| d.join(settings::FILE_NAME));
        let settings = settings_path.as_deref().map(Settings::load).unwrap_or_default();
        let lang_dir = env.data_dir.as_ref().map(|d| d.join(settings::LANG_DIR));
        // Le lingue disponibili servono solo al primo avvio, quando nessuna lingua è stata scelta: caricarle
        // due volte costava circa 6 ms all'avvio.
        let tag = settings.language.clone().unwrap_or_else(|| i18n::first_run_language(&env.windows_languages, &Lingue::load("it", lang_dir.as_deref()).available));
        let lingue = Lingue::load(&tag, lang_dir.as_deref());
        let fmt = Formatter { region: env.region.clone(), units: settings.units, labels: UnitLabels::from(&lingue) };
        // Primo avvio dopo il cambio di nome: l'esito della copia dei dati di Rilievo nella barra di stato.
        let notice = Notice::default();
        let data = env.data_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default();
        match &env.migration {
            legacy::Migration::Copied(_) => notice.set(lingue.ta("avviso-rilievo-copiati", &[("cartella", data.into())])),
            legacy::Migration::Failed(code) => notice.set(lingue.ta("avviso-rilievo-errore", &[("codice", code.to_string().into())])),
            legacy::Migration::Nothing => {}
        }
        Self {
            env,
            settings,
            lingue,
            fmt,
            volumes,
            screen: Screen::Volumes,
            job: None,
            waiting: None,
            scan_error: None,
            data: None,
            explore: explore::ExploreState::default(),
            search: explore::SearchState::default(),
            big: big_files::BigState::default(),
            inspector: inspector::InspectorState::default(),
            overview: overview::OverviewState::default(),
            palette: palette::PaletteState::default(),
            rules_view: rules_view::RulesState::default(),
            downloads: downloads_view::DownloadsState::default(),
            programs: programs_view::ProgramsState::default(),
            intervention: intervention_view::InterventionState::default(),
            procedures: procedure_flow::ProcedureState::default(),
            helper: helper_jobs::HelperState::default(),
            notice,
            fonts: FontState::default(),
            styled: None,
            sidebar_peek: false,
            frame_times: Rolling::new(600),
            last_frame: None,
            frame_count: 0,
            show_perf: false,
            progress_updates: Vec::new(),
            first_interactive: None,
            lang_generation: 1,
            last_verify: None,
            kb: std::sync::Arc::new(quado_kb::Kb::embedded().unwrap_or(quado_kb::Kb { rules: Vec::new() })),
        }
    }

    pub fn lang_dir(&self) -> Option<PathBuf> {
        self.env.data_dir.as_ref().map(|d| d.join(settings::LANG_DIR))
    }

    fn settings_path(&self) -> Option<PathBuf> {
        self.env.data_dir.as_ref().map(|d| d.join(settings::FILE_NAME))
    }

    /// Salva le impostazioni (solo dopo una scelta dell'utente).
    pub fn save_settings(&mut self) {
        if let Some(p) = self.settings_path()
            && let Err(e) = self.settings.save(&p)
        {
            log::warn!("impostazioni non salvate: {e}");
        }
    }

    /// Cambia lingua senza riavvio (05 D13).
    pub fn set_language(&mut self, tag: &str) {
        self.lingue = Lingue::load(tag, self.lang_dir().as_deref());
        self.fmt.labels = UnitLabels::from(&self.lingue);
        self.lang_generation += 1;
    }

    /// Ricarica i file di lingua dal disco (dopo una modifica fatta a mano).
    pub fn reload_languages(&mut self) {
        let tag = self.lingue.selected().to_owned();
        self.set_language(&tag);
    }

    pub fn theme(&self, ctx: &egui::Context) -> Theme {
        match ctx.theme() {
            egui::Theme::Light => Theme::Light,
            egui::Theme::Dark => Theme::Dark,
        }
    }

    /// Colori in uso: tema, accento e colori dei dati delle impostazioni.
    pub fn palette(&self, ctx: &egui::Context) -> Palette {
        Palette::new(self.theme(ctx), self.settings.accent, self.settings.data_palette)
    }

    /// Animazioni accese: "Effetti di animazione" di Windows e la scelta in Impostazioni.
    pub fn animations(&self) -> bool {
        self.env.animations && self.settings.motion == Motion::System
    }

    /// Avvia la scansione di un volume.
    pub fn start_scan(&mut self, volume: VolumeInfo) {
        self.wait_background();
        self.scan_error = None;
        self.progress_updates.clear();
        // Un risultato che aspettava i font lascia il posto alla scansione nuova.
        self.waiting = None;
        self.job = Some(ScanJob::start(volume, self.env.user_sid.clone(), std::sync::Arc::clone(&self.kb)));
        self.screen = Screen::Scanning;
    }

    /// Carica un risultato (dalla scansione o dai test).
    pub fn load_result(&mut self, volume: VolumeInfo, r: ScanResult, duration: Duration) {
        let mut ev = quado_kb::Evidence::empty(&volume.root);
        ev.user_sid = self.env.user_sid.clone();
        self.load_result_with(volume, r, duration, ev);
    }

    /// Come `load_result`, con le prove degli estrattori (test, immagini, scansione vera).
    pub fn load_result_with(&mut self, volume: VolumeInfo, r: ScanResult, duration: Duration, evidence: quado_kb::Evidence) {
        let loaded = Loaded::with_rules(volume, r, duration, self.env.user_sid.as_deref(), std::sync::Arc::clone(&self.kb), evidence);
        self.load(loaded);
    }

    /// Mostra un risultato già pronto.
    pub fn load(&mut self, loaded: Loaded) {
        self.wait_background();
        self.explore.reset(&loaded);
        self.explore.list_only = self.settings.explore_view == ExploreView::ListOnly;
        self.search = explore::SearchState::default();
        self.big = big_files::BigState::default();
        self.inspector = inspector::InspectorState::default();
        self.downloads.reset();
        self.last_verify = None;
        self.data = Some(loaded);
        self.screen = Screen::Overview;
    }

    /// Attende i lavori in background che tengono una copia del modello.
    pub fn wait_background(&mut self) {
        self.search.wait();
        self.palette.wait();
        self.big.wait();
        self.inspector.wait();
        self.downloads.wait();
        if let Some(t) = self.inspector.verify.take()
            && let (Some(v), Some(d)) = (t.wait(), self.data.as_mut())
        {
            self.last_verify = Some(data::apply_verified(d, v, self.env.user_sid.as_deref()));
        }
    }

    fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(job) = self.job.as_mut() else { return };
        // Avanzamento dal vivo: almeno 4 aggiornamenti al secondo (F-SC-4).
        ctx.request_repaint_after(Duration::from_millis(100));
        let files = job.progress.files.load(std::sync::atomic::Ordering::Relaxed);
        if self.progress_updates.last().is_none_or(|&(_, f)| f != files) {
            self.progress_updates.push((Instant::now(), files));
        }
        if self.waiting.is_none() {
            match job.poll() {
                Some(Ok(loaded)) => {
                    self.waiting = Some(loaded);
                    // Se i nomi chiedono font di ripiego, la scelta parte adesso in un altro thread.
                    self.check_fonts(ctx);
                }
                Some(Err(e)) => {
                    let job = self.job.take().expect("scansione in corso");
                    self.scan_error = Some((job.volume, e.0));
                    self.screen = if self.data.is_some() { Screen::Overview } else { Screen::Volumes };
                    return;
                }
                None => return,
            }
        }
        // Il risultato si mostra quando i font dei suoi nomi sono attivi (subito, se non ne servono altri).
        if self.waiting.as_ref().is_some_and(|w| self.fonts_ready(w)) {
            self.job = None;
            let loaded = self.waiting.take().expect("risultato in attesa");
            // Fine scansione (04 §6.8): durata e cartelle negate, nella barra di stato.
            if let Some(n) = volumes_view::completion_notice(&self.lingue, &self.fmt, &loaded) {
                self.notice.set(n);
            }
            self.load(loaded);
        }
    }

    /// Vero se i font di ripiego per i caratteri dei nomi di `d` sono scelti e già attivi in egui.
    fn fonts_ready(&self, d: &Loaded) -> bool {
        let f = &self.fonts;
        !self.env.system_fonts || (f.job.is_none() && f.checked.as_ref().is_some_and(|c| std::sync::Arc::ptr_eq(c, &d.name_chars)) && self.frame_count > f.installed_at)
    }

    /// Applica tema, accento, colori dei dati, animazioni, densità e dimensione del testo quando cambiano.
    fn apply_style(&mut self, ctx: &egui::Context) {
        let pref = match self.settings.theme {
            ThemeChoice::System => egui::ThemePreference::System,
            ThemeChoice::Light => egui::ThemePreference::Light,
            ThemeChoice::Dark => egui::ThemePreference::Dark,
        };
        let s = &self.settings;
        let want = Styled { theme: self.theme(ctx), choice: s.theme, animations: self.animations(), accent: s.accent, data: s.data_palette };
        if self.styled != Some(want) {
            ctx.options_mut(|o| o.theme_preference = pref);
            for t in Theme::ALL {
                tokens::apply(ctx, &Palette::new(t, s.accent, s.data_palette), want.animations);
            }
            self.styled = Some(Styled { theme: self.theme(ctx), ..want });
            self.explore.invalidate_map();
        }
        if tokens::metrics(ctx).density != self.settings.density {
            tokens::set_metrics(ctx, self.settings.density.metrics());
        }
        // Dimensione del testo: è lo zoom di egui, quindi scala testo, spazi e controlli insieme.
        let zoom = self.settings.zoom();
        if (ctx.zoom_factor() - zoom).abs() > 1e-3 {
            ctx.set_zoom_factor(zoom);
        }
    }

    /// Installa i font prima del primo frame (dal creatore dell'app): così il primo frame disegna già la
    /// schermata, invece di installarli e aspettare il successivo.
    pub fn install_fonts(&mut self, ctx: &egui::Context) {
        fonts::install(ctx, FontOptions { system_fallbacks: false, segoe_family: false });
        // Ctrl+più e Ctrl+meno cambiano la dimensione del testo delle impostazioni, non lo zoom di egui.
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        self.fonts.installed = true;
    }

    /// Font di Windows come ripiego per i caratteri dei nomi che Inter e Plex Mono non hanno. Sul thread
    /// dell'interfaccia solo il controllo dei caratteri distinti dei nomi (`Loaded::name_chars`, qualche
    /// centinaio); scelta e apertura dei file in un altro thread, poi l'installazione al frame dopo.
    fn check_fonts(&mut self, ctx: &egui::Context) {
        if !self.env.system_fonts {
            return;
        }
        if let Some(job) = self.fonts.job.as_mut() {
            let Some(r) = job.poll() else { return };
            self.fonts.job = None;
            self.fonts.uncovered.extend(r.uncovered.iter().copied());
            for (file, e) in &r.missing {
                log::warn!("font di sistema {file}: {e}");
            }
            if r.fonts != self.fonts.fallbacks {
                fonts::install_fallbacks(ctx, &r.fonts);
                let files: Vec<&str> = r.fonts.iter().map(|f| f.file).collect();
                log::info!(target: "quado", "font di ripiego di Windows: {} ({:.0} ms)", files.join(", "), r.ms);
                self.fonts.fallbacks = r.fonts;
                self.fonts.installed_at = self.frame_count;
            }
            // I font nuovi valgono dal frame dopo: il prossimo controllo li vede.
            ctx.request_repaint();
            return;
        }
        // I nomi del risultato in attesa prima di quelli già mostrati.
        let Some(d) = self.waiting.as_ref().or(self.data.as_ref()) else { return };
        if self.fonts.checked.as_ref().is_some_and(|c| std::sync::Arc::ptr_eq(c, &d.name_chars)) {
            return;
        }
        self.fonts.checked = Some(std::sync::Arc::clone(&d.name_chars));
        // egui dice "senza glifo" anche per i caratteri che cadono sul font di sostituzione: quelli coperti da
        // un ripiego già installato non si cercano di nuovo.
        let (uncovered, have) = (&self.fonts.uncovered, &self.fonts.fallbacks);
        let chars = d.name_chars.iter().copied().filter(|&c| !uncovered.contains(&c) && !have.iter().any(|f| f.covers(c)));
        let missing = fonts::missing_glyphs(ctx, chars);
        if missing.is_empty() {
            return;
        }
        let have = self.fonts.fallbacks.clone();
        let ctx = ctx.clone();
        self.fonts.job = Some(data::Task::spawn(move || {
            let r = fonts::choose_fallbacks(&missing, &have);
            ctx.request_repaint();
            r
        }));
    }

    /// Un frame.
    pub fn show(&mut self, ui: &mut Ui) {
        let now = Instant::now();
        if let Some(last) = self.last_frame {
            self.frame_times.push((now - last).as_secs_f64() * 1000.0);
        }
        self.last_frame = Some(now);
        self.frame_count += 1;
        let ctx = ui.ctx().clone();

        // Senza `install_fonts` dal creatore (test), i font si installano al primo frame e valgono dal successivo.
        if !self.fonts.installed {
            self.install_fonts(&ctx);
            ctx.request_repaint();
            return;
        }
        self.apply_style(&ctx);
        crate::widgets::track_focus_visible(&ctx);
        self.poll_job(&ctx);
        self.search.poll(&ctx);
        self.big.poll(&ctx);
        if let Some(v) = self.inspector.poll(&ctx)
            && let Some(d) = self.data.as_mut()
        {
            self.search.wait();
            self.big.wait();
            self.inspector.wait();
            self.downloads.wait();
            let r = data::apply_verified(d, v, self.env.user_sid.as_deref());
            self.explore.after_model_change(d, r.node);
            self.search.invalidate();
            self.big.invalidate();
            self.last_verify = Some(r);
        }
        // Lente Download: anche fuori dalla sua schermata, perché la card interventi ne usa le voci.
        self.downloads.poll(Some(&ctx), self.data.as_ref(), &self.env.readers);
        // Interventi: simulazione, esecuzione, misure e riscansioni in background (M5).
        intervention_view::poll(self, &ctx);
        procedure_flow::poll(self, &ctx);
        // Helper elevato: con `Done` le risposte entrano nel modello (M6).
        helper_jobs::poll(self, &ctx);
        self.check_fonts(&ctx);
        self.global_keys(ui);

        let p = self.palette(&ctx);
        // Cornice della variante A: la barra laterale prende tutta l'altezza, la barra superiore e quella di
        // stato stanno nella colonna del contenuto. L'elenco delle unità sta da solo, senza barra laterale (al
        // primo avvio e quando lo si apre dall'app, con «Torna alla Panoramica» in cima); così anche la prima
        // scansione, che continua la stessa colonna.
        if !shell::standalone(self) {
            shell::sidebar(self, ui, &p);
        }
        shell::top_bar(self, ui, &p);
        shell::status_bar(self, ui, &p);
        egui::CentralPanel::no_frame().show(ui, |ui| {
            ui.painter().rect_filled(ui.max_rect(), CornerRadius::ZERO, p.bg);
            match self.screen {
                Screen::Volumes => volumes_view::volumes(self, ui, &p),
                Screen::Scanning => volumes_view::scanning(self, ui, &p),
                Screen::Overview => overview::show(self, ui, &p),
                Screen::Explore => explore::show(self, ui, &p),
                Screen::BigFiles => big_files::show(self, ui, &p),
                Screen::System => system_view::show(self, ui, &p),
                Screen::Rules => rules_view::show(self, ui, &p),
                Screen::Downloads => downloads_view::show(self, ui, &p),
                Screen::Programs => programs_view::show(self, ui, &p),
                Screen::Interventions => interventions_view::show(self, ui, &p),
                Screen::Settings => settings_view::show(self, ui, &p),
            }
        });
        // Il dialogo di conferma dello spostamento sta sopra qualunque schermata; la palette dei comandi
        // sopra tutto.
        intervention_view::dialog(self, &ctx);
        palette::show(self, &ctx);
        if self.first_interactive.is_none() {
            self.first_interactive = Some(Instant::now());
        }
        if self.show_perf {
            ctx.request_repaint();
        }
    }

    pub fn go(&mut self, s: Screen) {
        if matches!(s, Screen::Overview | Screen::Explore | Screen::BigFiles | Screen::System | Screen::Downloads | Screen::Programs | Screen::Interventions) && self.data.is_none() {
            return;
        }
        self.screen = s;
        self.explore.focus_pending = s == Screen::Explore;
        self.big.focus_pending = s == Screen::BigFiles;
        self.downloads.focus_pending = s == Screen::Downloads;
    }

    fn global_keys(&mut self, ui: &mut Ui) {
        // Con un dialogo modale o la palette aperti i tasti sono loro: niente scorciatoie sotto.
        if self.palette.open || self.intervention.dialog_open() {
            return;
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K)) && shell::COMMAND_PALETTE {
            self.palette.show_palette();
            return;
        }
        let (k1, k2, k3, k4, k5, k6, k7, find, f2, f5, esc, comma) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::Num1),
                i.consume_key(Modifiers::COMMAND, Key::Num2),
                i.consume_key(Modifiers::COMMAND, Key::Num3),
                i.consume_key(Modifiers::COMMAND, Key::Num4),
                i.consume_key(Modifiers::COMMAND, Key::Num5),
                i.consume_key(Modifiers::COMMAND, Key::Num6),
                i.consume_key(Modifiers::COMMAND, Key::Num7),
                i.consume_key(Modifiers::COMMAND, Key::F),
                i.consume_key(Modifiers::NONE, Key::F2),
                i.consume_key(Modifiers::NONE, Key::F5),
                i.key_pressed(Key::Escape),
                i.consume_key(Modifiers::COMMAND, Key::Comma),
            )
        });
        if k1 {
            self.go(Screen::Overview);
        }
        if k2 {
            self.go(Screen::Explore);
        }
        if k3 {
            self.go(Screen::BigFiles);
        }
        if k4 {
            self.go(Screen::System);
        }
        if k5 {
            self.go(Screen::Downloads);
        }
        if k6 {
            self.go(Screen::Programs);
        }
        if k7 {
            self.go(Screen::Interventions);
        }
        if comma {
            self.go(Screen::Settings);
        }
        if find && self.data.is_some() {
            self.search.focus_box = true;
            self.sidebar_peek = self.settings.sidebar_collapsed;
        }
        if shell::text_size_keys(self, ui) {
            self.save_settings();
        }
        if f2 {
            self.show_perf = !self.show_perf;
        }
        if f5 && self.job.is_none()
            && let Some(v) = self.data.as_ref().map(|d| d.volume.clone())
        {
            self.start_scan(v);
        }
        if esc && self.screen == Screen::Scanning
            && let Some(j) = self.job.as_mut()
        {
            j.cancel();
        }
    }
}

/// "Disco locale (C:)" o "Etichetta (D:)".
pub fn volume_title(l: &Lingue, v: &VolumeInfo) -> String {
    let label = if v.label.is_empty() { l.t("volume-senza-etichetta") } else { v.label.clone() };
    l.ta("volume-titolo", &[("etichetta", label.into()), ("lettera", v.letter().into())])
}

/// Ciò che `apply_style` ha già applicato a egui.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Styled {
    theme: Theme,
    choice: ThemeChoice,
    animations: bool,
    accent: Accent,
    data: DataPalette,
}
