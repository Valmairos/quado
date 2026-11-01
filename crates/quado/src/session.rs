// SPDX-License-Identifier: GPL-3.0-or-later
//! Sessione guidata automatica per la prova senza rete e con la rete (06 §10): scansione di C:,
//! Panoramica, Esplora (tre cartelle), ricerca, File grandi, lente Download con il confronto del contenuto
//! (un solo gruppo della stessa dimensione, al più 1 GiB), Programmi, intervento simulato (mai confermato:
//! il dialogo si chiude con Esc), Impostazioni (lingua e tema cambiati e poi rimessi com'erano). Guida
//! l'app come `shots.rs`, una macchina a stati a frame.
//!
//! Su standard output, una riga per evento, separata da tabulazioni:
//!
//! ```text
//! inizio      <passo>
//! dettaglio   <passo>  <testo>
//! errore      <passo>  <testo>
//! passo       <passo>  <ms>  ok|errore
//! riepilogo   <chiave> <valore>
//! ```
//!
//! `cargo xtask sessione-prova` la avvia, ne campiona connessioni e handle e scrive il resoconto. La
//! sessione non scrive file fuori da `%APPDATA%\Quado` (solo le impostazioni, poi rimesse com'erano) e
//! non usa "Proprietà" né altri dialoghi della shell (06 §10, limite dichiarato).
//!
//! Esclusione: con `--escludi "a;b"` nessuna lettura della lente (attributi, impronte, versioni, indici
//! degli zip, Restart Manager) raggiunge un percorso che contiene una di quelle parti: le letture passano
//! da un [`Guard`] che le rifiuta prima di aprire il file e le conta. Il gruppo da confrontare, le cartelle
//! di Esplora e l'elemento dell'intervento si scelgono fuori da quei percorsi. La scansione del volume ne
//! elenca comunque nomi e attributi, come ogni scansione di C:.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

use eframe::egui;
use quado_kb::extract::ExtractError;
use quado_kb::installers::{InstallerInfo, InstallerKind};
use quado_model::downloads::GroupComparison;
use quado_model::record::Action;
use quado_model::{Flags, Tree};
use quado_scan_win::content::{ContentError, FileIdentity};
use quado_scan_win::zip::{ZipError, ZipIndex};
use quado_ui::app::data::VolumeInfo;
use quado_ui::app::downloads_view::ReadKind;
use quado_ui::app::intervention_jobs::{Origin, find_node};
use quado_ui::app::lens_jobs::{self, LensReaders, Readers};
use quado_ui::app::{App, Screen, intervention_view, settings};
use quado_ui::tokens::{Theme, ThemeChoice};

/// Parti di percorso (senza distinzione di maiuscole) che nessuna lettura della sessione e delle misure
/// apre: quelle di `--escludi "a;b"` (per esempio cartelle in Download da non toccare durante una prova).
/// Senza l'argomento, nessuna.
static EXCLUDED: OnceLock<Vec<String>> = OnceLock::new();

/// Fissa le parti escluse, una volta all'avvio (le chiamate successive non cambiano niente).
pub fn set_excluded(parts: &[String]) {
    let _ = EXCLUDED.set(parts.iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect());
}

/// Le parti escluse, in minuscolo.
pub fn excluded_parts() -> &'static [String] {
    EXCLUDED.get().map_or(&[], Vec::as_slice)
}

/// Byte al massimo letti dal confronto del contenuto: un solo gruppo della stessa dimensione.
const HASH_LIMIT: u64 = 1 << 30;

/// Termine della ricerca: comune nei nomi di un volume di sistema.
const SEARCH_TERM: &str = "setup";

/// Tempo con il risultato di una fase a schermo prima della successiva (almeno qualche frame): il
/// campionamento di xtask (ogni 120-200 ms) vede così ogni schermata più volte.
const HOLD: Duration = Duration::from_millis(1000);

/// Il percorso contiene una delle parti escluse.
pub fn excluded(path: &str) -> bool {
    let p = path.to_lowercase();
    excluded_parts().iter().any(|x| p.contains(x.as_str()))
}

/// Letture della lente con i percorsi esclusi rifiutati prima di aprire il file (come
/// `Readers::excluding`), contati per il resoconto.
struct Guard {
    inner: Readers,
    refused: Arc<AtomicUsize>,
}

impl Guard {
    fn refuses(&self, path: &str) -> bool {
        let hit = excluded(path);
        if hit {
            self.refused.fetch_add(1, Relaxed);
        }
        hit
    }
}

impl LensReaders for Guard {
    fn identity(&self, path: &str) -> Result<FileIdentity, ContentError> {
        if self.refuses(path) { Err(ContentError::Io(lens_jobs::EXCLUDED)) } else { self.inner.0.identity(path) }
    }

    fn sha256(&self, path: &str, cancel: &AtomicBool, progress: &AtomicU64) -> Result<(FileIdentity, [u8; 32]), ContentError> {
        if self.refuses(path) { Err(ContentError::Io(lens_jobs::EXCLUDED)) } else { self.inner.0.sha256(path, cancel, progress) }
    }

    fn installer(&self, path: &str, kind: InstallerKind) -> Result<InstallerInfo, ExtractError> {
        if self.refuses(path) { Err(ExtractError::Io(lens_jobs::EXCLUDED)) } else { self.inner.0.installer(path, kind) }
    }

    fn zip_index(&self, path: &str) -> Result<ZipIndex, ZipError> {
        if self.refuses(path) { Err(ZipError::Io(lens_jobs::EXCLUDED)) } else { self.inner.0.zip_index(path) }
    }

    fn processes(&self, paths: &[String]) -> Result<Vec<String>, u32> {
        // Un elenco con un percorso escluso non si registra: la lente ripiega sui file uno per uno.
        if paths.iter().any(|p| self.refuses(p)) { Err(lens_jobs::EXCLUDED) } else { self.inner.0.processes(paths) }
    }

    fn now(&self) -> i64 {
        self.inner.0.now()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Scan,
    Overview,
    Explore,
    Search,
    BigFiles,
    Downloads,
    Programs,
    Intervention,
    Settings,
    End,
}

impl Step {
    fn name(self) -> &'static str {
        match self {
            Self::Scan => "scansione",
            Self::Overview => "panoramica",
            Self::Explore => "esplora",
            Self::Search => "ricerca",
            Self::BigFiles => "file-grandi",
            Self::Downloads => "download",
            Self::Programs => "programmi",
            Self::Intervention => "intervento",
            Self::Settings => "impostazioni",
            Self::End => "fine",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Scan => Self::Overview,
            Self::Overview => Self::Explore,
            Self::Explore => Self::Search,
            Self::Search => Self::BigFiles,
            Self::BigFiles => Self::Downloads,
            Self::Downloads => Self::Programs,
            Self::Programs => Self::Intervention,
            Self::Intervention => Self::Settings,
            Self::Settings | Self::End => Self::End,
        }
    }

    /// Tempo massimo del passo: oltre, il passo è un errore e la sessione va avanti.
    fn limit(self) -> Duration {
        Duration::from_secs(match self {
            Self::Scan => 900,
            Self::Downloads | Self::Intervention => 600,
            _ => 120,
        })
    }
}

/// Esito di un frame del passo.
enum Outcome {
    Wait,
    Ok(String),
    Fail(String),
}

/// Impostazioni prima del passo Impostazioni, da rimettere com'erano.
struct Original {
    language: Option<String>,
    selected: String,
    theme: ThemeChoice,
    path: Option<PathBuf>,
    file: Option<Vec<u8>>,
}

pub struct Session {
    step: Step,
    phase: u32,
    frames: u32,
    phase_started: Instant,
    step_started: Instant,
    started: Instant,
    errors: usize,
    refused: Arc<AtomicUsize>,
    opened: usize,
    hits: usize,
    group: Vec<u32>,
    asked: (usize, u64),
    rule: Option<usize>,
    escapes: u32,
    original: Option<Original>,
    pub finished: bool,
}

impl Session {
    pub fn new(app: &mut App) -> Self {
        let refused = Arc::new(AtomicUsize::new(0));
        app.env.readers = Readers(Arc::new(Guard { inner: app.env.readers.clone(), refused: Arc::clone(&refused) }));
        println!("sessione\tinizio\t{}", std::process::id());
        println!("esclusioni\t{}", excluded_parts().join(";"));
        let now = Instant::now();
        let mut s = Self {
            step: Step::Scan,
            phase: 0,
            frames: 0,
            phase_started: now,
            step_started: now,
            started: now,
            errors: 0,
            refused,
            opened: 0,
            hits: 0,
            group: Vec::new(),
            asked: (0, 0),
            rule: None,
            escapes: 0,
            original: None,
            finished: false,
        };
        s.begin();
        match app.volumes.iter().find(|v| v.is_system).and_then(VolumeInfo::from_volume) {
            Some(v) => app.start_scan(v),
            None => s.error("nessun volume di sistema"),
        }
        s
    }

    fn begin(&mut self) {
        println!("inizio\t{}", self.step.name());
        self.phase = 0;
        self.frames = 0;
        self.step_started = Instant::now();
        self.phase_started = self.step_started;
    }

    fn next_phase(&mut self) {
        self.phase += 1;
        self.frames = 0;
        self.phase_started = Instant::now();
    }

    /// La fase resta a schermo: meno di [`HOLD`] o meno di 5 frame.
    fn holding(&self) -> bool {
        self.frames < 5 || self.phase_started.elapsed() < HOLD
    }

    fn error(&mut self, msg: &str) {
        self.errors += 1;
        println!("errore\t{}\t{}", self.step.name(), one_line(msg));
    }

    fn detail(&self, msg: &str) {
        println!("dettaglio\t{}\t{}", self.step.name(), one_line(msg));
    }

    /// Chiamata a ogni frame, prima di `App::show`.
    pub fn frame(&mut self, app: &mut App, ctx: &egui::Context) {
        if self.finished {
            return;
        }
        ctx.request_repaint();
        self.frames += 1;
        let outcome = if self.step != Step::End && self.step_started.elapsed() > self.step.limit() {
            Outcome::Fail(format!("tempo scaduto ({} s)", self.step.limit().as_secs()))
        } else {
            match self.step {
                Step::Scan => self.scan(app),
                Step::Overview => self.show_for(app, Screen::Overview),
                Step::Explore => self.explore(app),
                Step::Search => self.search(app),
                Step::BigFiles => self.big_files(app),
                Step::Downloads => self.downloads(app),
                Step::Programs => self.programs(app),
                Step::Intervention => self.intervention(app, ctx),
                Step::Settings => self.settings(app, ctx),
                Step::End => self.end(app),
            }
        };
        let ok = match outcome {
            Outcome::Wait => return,
            Outcome::Ok(note) => {
                if !note.is_empty() {
                    self.detail(&note);
                }
                true
            }
            // La fine riassume gli errori già contati: non ne aggiunge uno.
            Outcome::Fail(why) if self.step == Step::End => {
                self.detail(&why);
                false
            }
            Outcome::Fail(why) => {
                self.error(&why);
                false
            }
        };
        let ms = if self.step == Step::End { self.started.elapsed() } else { self.step_started.elapsed() }.as_millis();
        println!("passo\t{}\t{ms}\t{}", self.step.name(), if ok { "ok" } else { "errore" });
        if self.step == Step::End {
            self.finished = true;
            return;
        }
        // Senza la scansione gli altri passi non hanno dati: la sessione finisce.
        self.step = if self.step == Step::Scan && !ok { Step::End } else { self.step.next() };
        self.begin();
    }

    fn scan(&mut self, app: &App) -> Outcome {
        if app.job.is_some() {
            return Outcome::Wait;
        }
        match (&app.data, &app.scan_error) {
            (Some(d), _) => Outcome::Ok(format!(
                "file {}, cartelle {}, non leggibili {}, scansione {} ms",
                d.tree().files(Tree::ROOT),
                d.stats.dirs,
                d.observed.unreadable.len(),
                d.duration.as_millis()
            )),
            (None, Some((_, code))) => Outcome::Fail(format!("scansione non riuscita, codice {code}")),
            (None, None) => Outcome::Fail("scansione non avviata".into()),
        }
    }

    /// Una schermata per qualche frame.
    fn show_for(&mut self, app: &mut App, screen: Screen) -> Outcome {
        if self.phase == 0 {
            app.go(screen);
            self.next_phase();
            return Outcome::Wait;
        }
        if self.holding() { Outcome::Wait } else { Outcome::Ok(String::new()) }
    }

    /// Esplora: Windows, Programmi e `AppData\Local` dell'utente, con il primo elemento selezionato.
    fn explore(&mut self, app: &mut App) -> Outcome {
        if self.phase == 0 {
            app.go(Screen::Explore);
            self.next_phase();
            return Outcome::Wait;
        }
        if self.holding() {
            return Outcome::Wait;
        }
        let Some(d) = app.data.as_ref() else { return Outcome::Fail("nessun dato".into()) };
        let f = &d.evidence.folders;
        let folders: Vec<String> = [&f.windows, &f.program_files, &f.local].into_iter().flatten().filter(|p| !excluded(p)).cloned().collect();
        let k = (self.phase - 1) as usize;
        if let Some(path) = folders.get(k) {
            let t = d.tree();
            match find_node(t, path) {
                Some(n) => {
                    let first = d.ord().children(n).iter().copied().find(|&c| !excluded(&t.path_lossy(c)));
                    app.explore.open(d, n, first);
                    self.opened += 1;
                }
                None => self.detail(&format!("cartella non trovata nella scansione: {path}")),
            }
            self.next_phase();
            return Outcome::Wait;
        }
        if self.opened == 0 { Outcome::Fail("nessuna cartella aperta".into()) } else { Outcome::Ok(format!("cartelle aperte {}", self.opened)) }
    }

    /// Ricerca per nome: i risultati compaiono in Esplora.
    fn search(&mut self, app: &mut App) -> Outcome {
        match self.phase {
            0 => {
                app.go(Screen::Explore);
                app.search.set_query(SEARCH_TERM);
                self.next_phase();
                Outcome::Wait
            }
            1 => {
                if self.frames < 3 || app.search.pending() {
                    return Outcome::Wait;
                }
                match &app.search.outcome {
                    Some(o) => {
                        self.hits = o.hits.len();
                        self.next_phase();
                        Outcome::Wait
                    }
                    None => Outcome::Wait,
                }
            }
            _ if self.holding() => Outcome::Wait,
            _ => {
                app.search.clear();
                Outcome::Ok(format!("termine {SEARCH_TERM}, risultati {}", self.hits))
            }
        }
    }

    fn big_files(&mut self, app: &mut App) -> Outcome {
        if self.phase == 0 {
            app.go(Screen::BigFiles);
            self.next_phase();
            return Outcome::Wait;
        }
        if self.holding() || app.big.pending() {
            return Outcome::Wait;
        }
        match &app.big.outcome {
            Some(o) => Outcome::Ok(format!("file elencati {}", o.files.len())),
            None => Outcome::Wait,
        }
    }

    /// Lente Download sulla cartella Download dell'utente, poi il confronto del contenuto di un solo gruppo
    /// della stessa dimensione (il più grande entro [`HASH_LIMIT`]), fuori dai percorsi esclusi.
    fn downloads(&mut self, app: &mut App) -> Outcome {
        let readers = app.env.readers.clone();
        match self.phase {
            0 => {
                app.downloads.choose(None);
                app.go(Screen::Downloads);
                self.next_phase();
                Outcome::Wait
            }
            1 => {
                let Some(d) = app.data.as_ref() else { return Outcome::Fail("nessun dato".into()) };
                if self.frames < 3 {
                    return Outcome::Wait;
                }
                if app.downloads.root().is_none() {
                    return if self.frames > 120 { Outcome::Fail("cartella Download non trovata nella scansione".into()) } else { Outcome::Wait };
                }
                let st = &mut app.downloads;
                if st.pending() {
                    return Outcome::Wait;
                }
                let Some(v) = st.current(d) else { return Outcome::Wait };
                let t = d.tree();
                let ok_node = |n: u32| !t.flags(n).has(Flags::CLOUD) && !excluded(&t.path_lossy(n));
                let best = v
                    .candidates
                    .iter()
                    .zip(&v.compared)
                    .filter(|(_, c)| matches!(c, GroupComparison::NotRead { .. }))
                    .map(|(g, _)| (g.logical.saturating_mul(g.nodes.len() as u64), g))
                    .filter(|&(bytes, g)| bytes <= HASH_LIMIT && g.nodes.len() >= 2 && g.nodes.iter().all(|&n| ok_node(n)))
                    .max_by_key(|&(bytes, g)| (bytes, std::cmp::Reverse(g.nodes[0])));
                self.detail(&format!("voci della lente {}, gruppi della stessa dimensione {}", v.findings.len(), v.candidates.len()));
                let Some((_, g)) = best else { return Outcome::Ok("nessun gruppo da confrontare entro 1 GiB".into()) };
                self.group = g.nodes.clone();
                st.request(ReadKind::Hash, d, &readers);
                let group = &self.group;
                let Some(p) = st.prompt.as_mut() else { return Outcome::Fail("il confronto non ha chiesto conferma".into()) };
                p.retain_nodes(|n| group.contains(&n));
                self.asked = (p.files, p.bytes);
                self.next_phase();
                Outcome::Wait
            }
            // La richiesta di conferma resta a schermo qualche frame, poi si conferma.
            2 if self.holding() => Outcome::Wait,
            2 => {
                let Some(d) = app.data.as_ref() else { return Outcome::Fail("nessun dato".into()) };
                if self.asked.0 != self.group.len() {
                    app.downloads.dismiss();
                    return Outcome::Fail(format!("la richiesta ha {} file invece di {}", self.asked.0, self.group.len()));
                }
                app.downloads.confirm(d, &readers);
                self.next_phase();
                Outcome::Wait
            }
            _ => {
                let Some(d) = app.data.as_ref() else { return Outcome::Fail("nessun dato".into()) };
                let st = &app.downloads;
                if st.progress().is_some() || st.pending() {
                    return Outcome::Wait;
                }
                let Some(o) = st.outcome else { return Outcome::Wait };
                let result = st
                    .current(d)
                    .and_then(|v| v.candidates.iter().position(|g| g.nodes == self.group).map(|k| &v.compared[k]))
                    .map_or_else(
                        || "gruppo non più nella lente".to_owned(),
                        |c| match c {
                            GroupComparison::Compared { identical, unique } => format!("gruppi di copie identiche {}, file senza copie {}", identical.len(), unique.len()),
                            GroupComparison::Changed { changed } => format!("file cambiati dalla scansione {changed}"),
                            GroupComparison::NotRead { read } => format!("non confrontato, letti {read}"),
                        },
                    );
                let note = format!("impronte di un gruppo: file {}, byte {}; letti {}, non letti {}; {result}", self.asked.0, self.asked.1, o.read, o.failed);
                if o.read + o.failed != self.asked.0 || o.interrupted {
                    return Outcome::Fail(format!("lettura incompleta: {note}"));
                }
                Outcome::Ok(note)
            }
        }
    }

    fn programs(&mut self, app: &mut App) -> Outcome {
        if self.phase == 0 {
            app.go(Screen::Programs);
            if app.data.as_ref().is_some_and(|d| !d.programs.items.is_empty()) {
                app.programs.selected = Some(0);
            }
            self.next_phase();
            return Outcome::Wait;
        }
        if self.holding() {
            return Outcome::Wait;
        }
        let n = app.data.as_ref().map_or(0, |d| d.programs.items.len());
        Outcome::Ok(format!("programmi {n}"))
    }

    /// Spostamento nel Cestino simulato sull'elemento più piccolo che una regola propone di spostare: la
    /// simulazione legge solo attributi, uso dei file e riferimenti; il dialogo di conferma si chiude con
    /// Esc e il piano non si costruisce.
    fn intervention(&mut self, app: &mut App, ctx: &egui::Context) -> Outcome {
        match self.phase {
            0 => {
                let Some(d) = app.data.as_ref() else { return Outcome::Fail("nessun dato".into()) };
                let t = d.tree();
                let best = d
                    .cls
                    .matches
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| m.record.action == Action::Recycle && m.record.state.proposes() && !m.nodes.is_empty())
                    .filter(|(_, m)| m.nodes.iter().all(|&n| !excluded(&t.path_lossy(n))))
                    .map(|(k, m)| (m.nodes.iter().map(|&n| t.alloc(n)).sum::<u64>(), k))
                    .filter(|&(bytes, _)| bytes > 0)
                    .min();
                let Some((bytes, k)) = best else { return Outcome::Fail("nessun elemento da spostare proposto da una regola".into()) };
                let m = &d.cls.matches[k];
                self.detail(&format!("regola {}, elementi {}, byte {bytes}", d.kb.rules[m.rule].id, m.nodes.len()));
                // Come l'utente: l'elemento selezionato in Esplora, poi "Sposta nel Cestino…".
                let node = m.nodes[0];
                app.explore.open(d, t.parent(node), Some(node));
                app.go(Screen::Explore);
                self.rule = Some(k);
                self.next_phase();
                Outcome::Wait
            }
            1 if self.holding() => Outcome::Wait,
            1 => {
                if let Some(k) = self.rule {
                    intervention_view::open(app, Origin::Rule(k));
                }
                self.next_phase();
                Outcome::Wait
            }
            2 => {
                if app.intervention.simulating() {
                    return Outcome::Wait;
                }
                match app.intervention.simulation() {
                    Some(sim) => {
                        self.detail(&format!("simulazione: elementi {}", sim.items().len()));
                        self.next_phase();
                        Outcome::Wait
                    }
                    None if self.frames > 5 => Outcome::Fail("la simulazione non ha aperto il dialogo di conferma".into()),
                    None => Outcome::Wait,
                }
            }
            // Il dialogo resta a schermo, poi Esc (come l'utente), ripetuto se serve.
            _ if self.holding() => Outcome::Wait,
            _ => {
                if !app.intervention.dialog_open() {
                    let n = app.intervention.confirmations();
                    return if n == 0 { Outcome::Ok("dialogo chiuso con Esc, nessuna conferma".into()) } else { Outcome::Fail(format!("piani confermati: {n}")) };
                }
                if self.escapes >= 10 {
                    return Outcome::Fail("il dialogo di conferma non si chiude con Esc".into());
                }
                if self.frames.is_multiple_of(10) {
                    self.escapes += 1;
                    ctx.input_mut(|i| {
                        i.events.push(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
                    });
                }
                Outcome::Wait
            }
        }
    }

    /// Lingua e tema cambiati come dalla pagina Impostazioni (con il salvataggio in `%APPDATA%\Quado`),
    /// poi rimessi com'erano, file delle impostazioni compreso.
    fn settings(&mut self, app: &mut App, ctx: &egui::Context) -> Outcome {
        match self.phase {
            0 => {
                let path = app.env.data_dir.as_ref().map(|d| d.join(settings::FILE_NAME));
                let file = path.as_ref().and_then(|p| std::fs::read(p).ok());
                self.original = Some(Original { language: app.settings.language.clone(), selected: app.lingue.selected().to_owned(), theme: app.settings.theme, path, file });
                app.go(Screen::Settings);
                self.next_phase();
                Outcome::Wait
            }
            1..=3 if self.holding() => Outcome::Wait,
            1 => {
                let Some(o) = &self.original else { return Outcome::Fail("stato iniziale mancante".into()) };
                let other = if o.selected.starts_with("it") { "en" } else { "it" };
                app.set_language(other);
                app.settings.language = Some(other.to_owned());
                app.save_settings();
                self.detail(&format!("lingua {} poi {other}", o.selected));
                self.next_phase();
                Outcome::Wait
            }
            2 => {
                let other = if app.theme(ctx) == Theme::Dark { ThemeChoice::Light } else { ThemeChoice::Dark };
                app.settings.theme = other;
                app.save_settings();
                let before = self.original.as_ref().map_or_else(String::new, |o| format!("{:?}", o.theme));
                self.detail(&format!("tema {before} poi {other:?}"));
                self.next_phase();
                Outcome::Wait
            }
            3 => {
                let Some(o) = self.original.take() else { return Outcome::Fail("stato iniziale mancante".into()) };
                app.set_language(&o.selected);
                app.settings.language = o.language.clone();
                app.settings.theme = o.theme;
                let restored = match (&o.path, &o.file) {
                    (Some(p), Some(bytes)) => std::fs::write(p, bytes).map_err(|e| e.to_string()),
                    (Some(p), None) if p.exists() => std::fs::remove_file(p).map_err(|e| e.to_string()),
                    _ => Ok(()),
                };
                let same = o.path.as_ref().is_none_or(|p| std::fs::read(p).ok() == o.file);
                self.original = Some(o);
                self.next_phase();
                match restored {
                    Err(e) => Outcome::Fail(format!("impostazioni non rimesse: {e}")),
                    Ok(()) if !same => Outcome::Fail("il file delle impostazioni non è tornato com'era".into()),
                    Ok(()) => Outcome::Wait,
                }
            }
            _ if self.holding() => Outcome::Wait,
            _ => {
                let o = self.original.as_ref();
                let back = o.is_some_and(|o| app.lingue.selected() == o.selected && app.settings.theme == o.theme && app.settings.language == o.language);
                if back { Outcome::Ok("lingua, tema e file delle impostazioni rimessi com'erano".into()) } else { Outcome::Fail("impostazioni in memoria diverse da prima".into()) }
            }
        }
    }

    fn end(&mut self, app: &App) -> Outcome {
        let confirmations = app.intervention.confirmations();
        if confirmations > 0 {
            self.error(&format!("piani confermati: {confirmations}"));
        }
        println!("riepilogo\terrori\t{}", self.errors);
        println!("riepilogo\tletture-rifiutate\t{}", self.refused.load(Relaxed));
        println!("riepilogo\tconferme\t{confirmations}");
        if self.errors == 0 { Outcome::Ok(String::new()) } else { Outcome::Fail(format!("errori nella sessione: {}", self.errors)) }
    }
}

/// Una riga sola: niente tabulazioni né a capo dentro un campo.
fn one_line(s: &str) -> String {
    s.replace(['\t', '\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le parti escluse dei test, le stesse in ogni test (si fissano una volta sola).
    fn exclude_for_tests() {
        set_excluded(&["copia-a-".into(), " Copia-B- ".into(), String::new()]);
    }

    #[test]
    fn the_excluded_parts_are_recognized_in_any_case() {
        exclude_for_tests();
        assert_eq!(excluded_parts(), ["copia-a-", "copia-b-"]);
        assert!(excluded(r"C:\Users\demo\Downloads\copia-a-0.4.0-windows-x64-portable\app.exe"));
        assert!(excluded(r"C:\Users\demo\Downloads\Copia-B-0.5.0.zip"));
        assert!(excluded(r"c:\users\demo\downloads\COPIA-A-0.7.0"));
        assert!(!excluded(r"C:\Users\demo\Downloads\copia-a.txt"));
        assert!(!excluded(r"C:\Users\demo\Downloads\setup.exe"));
    }

    #[test]
    fn the_guard_refuses_excluded_paths_before_the_inner_reader() {
        exclude_for_tests();
        let refused = Arc::new(AtomicUsize::new(0));
        // `inert` non legge niente: un percorso escluso deve fermarsi prima, con il codice dell'esclusione.
        let g = Guard { inner: Readers::inert(), refused: Arc::clone(&refused) };
        let cancel = AtomicBool::new(false);
        let progress = AtomicU64::new(0);
        assert_eq!(g.sha256(r"C:\D\copia-a-1\a.bin", &cancel, &progress).err(), Some(ContentError::Io(lens_jobs::EXCLUDED)));
        assert_eq!(g.zip_index(r"C:\D\copia-b-1.zip").err(), Some(ZipError::Io(lens_jobs::EXCLUDED)));
        assert_eq!(g.processes(&[r"C:\D\a.part".into(), r"C:\D\copia-a-2\b.part".into()]).err(), Some(lens_jobs::EXCLUDED));
        assert_eq!(g.sha256(r"C:\D\altro.bin", &cancel, &progress).err(), Some(ContentError::Io(lens_jobs::NOT_READ)));
        assert_eq!(refused.load(Relaxed), 3);
    }

    #[test]
    fn the_steps_run_in_order_and_end() {
        let mut s = Step::Scan;
        let mut names = vec![s.name()];
        while s != Step::End {
            s = s.next();
            names.push(s.name());
        }
        assert_eq!(names, ["scansione", "panoramica", "esplora", "ricerca", "file-grandi", "download", "programmi", "intervento", "impostazioni", "fine"]);
    }
}
