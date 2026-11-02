// SPDX-License-Identifier: GPL-3.0-or-later
//! Programmi: programmi registrati, app di Microsoft Store, giochi di Steam ed Epic con fonte, versione,
//! spazio misurato solo con una prova del percorso, dichiarato con la sua fonte (mai sommato), ultimo uso
//! noto con la copertura della fonte e data di installazione. Il dettaglio mostra i fatti del record con
//! glifo e fonte, la prova del percorso, i dati riconosciuti dalle regole, l'avviso dei tuoi file e la scheda
//! Decidi tu.
//!
//! - Disinstallazione: "Disinstalla…" apre un dialogo che mostra prima i tuoi file e ne
//!   chiede la conferma esplicita, poi apre l'interfaccia ufficiale con `quado_actions::launch` (valori
//!   tipizzati: mai `UninstallString`). Per Epic solo il rinvio alla libreria del launcher.
//! - Possibili residui: sezione a parte con il criterio in testata, stato Non sappiamo e le sole
//!   azioni "Apri in Esplora file" e "Mostra in Esplora".
//! - Il lanciatore e "Apri in Esplora file" sono funzioni nello stato della schermata: i test le
//!   sostituiscono e registrano le chiamate senza aprire Impostazioni, Steam o Esplora file.
//!
//! Il core dà codici e valori (`quado_kb::programs`); le frasi vengono dai file di lingua.

use std::collections::HashSet;
use std::sync::Arc;

use egui::{Align, Align2, Color32, CornerRadius, Frame, Id, Key, Layout, Margin, Rect, Response, ScrollArea, Sense, Stroke, Ui, pos2, vec2};
use quado_actions::launch::{self, Launch, Pfn, SteamAppId};
use quado_actions::shell::{self, ShellError};
use quado_kb::extract::TrackProgs;
use quado_kb::programs::{Extractor, Location, PathEvidence, Program, ProgramSource, Programs, Residue, ResidueBase, RuleData, Space, UninstallVia, Usage};
use quado_model::record::{Fact, Hive, Provenance, Record, Source, State, UnknownReason};
use quado_model::Tree;

use super::data::Loaded;
use super::table::{self, Column, NavKeys};
use super::inspector::{self, Sections};
use super::{App, intervention_view, kbtext, system_view};
use crate::format::{Formatter, Units};
use crate::i18n::Lingue;
use crate::icons::{self, Icon};
use crate::index_scroll::{self, BarColors, IndexScroll};
use crate::tokens::{self, Gestore, Glyph, Palette, Type};
use crate::widgets::{self, ButtonKind, Evidence, mono};

/// Apre l'interfaccia ufficiale di disinstallazione: `quado_actions::launch::open`, finta nei test.
pub type Launcher = fn(&Launch) -> Result<(), ShellError>;
/// Apre Esplora file con l'elemento selezionato: `quado_actions::shell::reveal`, finta nei test.
pub type Revealer = fn(&str) -> Result<(), ShellError>;

/// Sezione mostrata: l'elenco dei programmi o i possibili residui.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Programs,
    Residues,
}

/// Esito del pulsante principale del dialogo di disinstallazione.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// L'interfaccia ufficiale si è aperta (per i programmi Win32 il nome è negli appunti).
    Opened,
    /// La shell non l'ha aperta: HRESULT.
    Failed(i32),
    /// PFN o appid non accettati dal lanciatore: non si apre niente.
    Invalid,
}

/// Dialogo di disinstallazione.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UninstallDialog {
    /// Indice in `Programs::items`.
    pub program: usize,
    /// Conferma esplicita che i tuoi file nella cartella sono stati copiati altrove (solo se ce ne sono).
    pub copied: bool,
    pub outcome: Option<Outcome>,
}

/// Stato della schermata.
pub struct ProgramsState {
    pub section: Section,
    /// Mostra i componenti di sistema (`SystemComponent=1`), nascosti per default.
    pub show_hidden: bool,
    /// Ricerca per nome.
    pub query: String,
    /// Programma selezionato: indice in `Programs::items`.
    pub selected: Option<usize>,
    /// Possibile residuo selezionato: indice in `Programs::residues`.
    pub selected_residue: Option<usize>,
    /// Righe della tabella nell'ultimo frame: indici in `Programs::items` per spazio misurato decrescente,
    /// filtrati dall'interruttore dei componenti di sistema e dalla ricerca.
    pub rows: Vec<usize>,
    pub dialog: Option<UninstallDialog>,
    pub launcher: Launcher,
    pub reveal: Revealer,
    /// La tabella prende il focus al prossimo frame (arrivo sulla schermata, cambio di sezione).
    pub focus_pending: bool,
    pub scroll: IndexScroll,
    pub residue_scroll: IndexScroll,
    /// Pannello del dettaglio nell'ultimo frame.
    pub detail: Rect,
    /// Sezioni del dettaglio aperte o chiuse, come nell'ispettore.
    pub sections: Sections,
    cache: Option<Cache>,
    hovered: Option<usize>,
    /// Ultimo frame in cui la schermata è stata disegnata: un salto vuol dire che ci si è appena arrivati.
    last_frame: u64,
}

impl Default for ProgramsState {
    fn default() -> Self {
        Self {
            section: Section::Programs,
            show_hidden: false,
            query: String::new(),
            selected: None,
            selected_residue: None,
            rows: Vec::new(),
            dialog: None,
            launcher: launch::open,
            reveal: shell::reveal,
            focus_pending: false,
            scroll: IndexScroll::default(),
            residue_scroll: IndexScroll::default(),
            detail: Rect::NOTHING,
            sections: Sections::default(),
            cache: None,
            hovered: None,
            last_frame: 0,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Testi delle celle

/// Testi di una riga, scritti una volta per dati, lingua e unità.
struct Cells {
    source: String,
    version: String,
    measured: String,
    /// Il misurato è una cifra (Plex Mono) e non un testo come "posizione non dichiarata".
    figure: bool,
    declared: String,
    usage: String,
    installed: String,
    /// Indicazione accanto al nome: componente di sistema, cartella condivisa.
    tag: Option<String>,
    /// Nome in minuscolo, per la ricerca.
    lower: String,
}

/// Testi e ordine dei programmi e dei possibili residui per i dati, la lingua e le unità correnti.
struct Cache {
    programs: Arc<Programs>,
    generation: u64,
    lang: String,
    units: Units,
    /// Tutti i programmi per spazio misurato decrescente, poi non misurati, poi senza spazio; a parità per nome.
    order: Vec<usize>,
    cells: Vec<Cells>,
    /// Testo di ogni possibile residuo, con la data.
    residue_texts: Vec<String>,
    /// Larghezze delle colonne fisse misurate sui testi (al primo frame dopo la ricostruzione).
    widths: Option<Vec<f32>>,
}

/// Fonte breve, per la tabella.
fn source_short(l: &Lingue, p: &Program) -> String {
    l.t(match &p.source {
        ProgramSource::Uninstall { hive: Hive::Machine, .. } => "programmi-fonte-hklm",
        ProgramSource::Uninstall { hive: Hive::Machine32, .. } => "programmi-fonte-wow",
        ProgramSource::Uninstall { hive: Hive::User, .. } => "programmi-fonte-hkcu",
        ProgramSource::Msix { .. } => "programmi-fonte-msix",
        ProgramSource::Steam { .. } => "programmi-fonte-steam",
        ProgramSource::Epic { .. } => "programmi-fonte-epic",
    })
}

/// Fonte per esteso, sotto il nome nel dettaglio.
fn source_long(l: &Lingue, p: &Program) -> String {
    l.t(match &p.source {
        ProgramSource::Uninstall { hive: Hive::Machine, .. } => "programmi-tipo-hklm",
        ProgramSource::Uninstall { hive: Hive::Machine32, .. } => "programmi-tipo-wow",
        ProgramSource::Uninstall { hive: Hive::User, .. } => "programmi-tipo-hkcu",
        ProgramSource::Msix { .. } => "programmi-tipo-msix",
        ProgramSource::Steam { .. } => "programmi-tipo-steam",
        ProgramSource::Epic { .. } => "programmi-tipo-epic",
    })
}

/// Spazio misurato nella tabella: la cifra, oppure perché non c'è. Mai 0 senza misura.
fn measured_cell(l: &Lingue, f: &Formatter, p: &Program) -> (String, bool) {
    match (p.space, &p.location) {
        (Space::Measured(b), _) => (f.size(b), true),
        (Space::NotMeasured, _) => (l.t("programmi-cella-non-misurato"), false),
        (Space::NotAttributed, Location::Implausible { .. }) => (l.t("programmi-cella-non-plausibile"), false),
        (Space::NotAttributed, _) => (l.t("programmi-cella-non-dichiarata"), false),
    }
}

/// Dimensione dichiarata con la fonte breve; vuota se la fonte non ne dichiara una.
fn declared_cell(l: &Lingue, f: &Formatter, p: &Program) -> String {
    let Some(d) = &p.declared else { return String::new() };
    let source = match d.source {
        Source::Uninstall(_) => l.t("programmi-dichiarata-registro"),
        Source::SteamManifest | Source::SteamLocalConfig => l.t("programmi-dichiarata-steam"),
        ref other => l.t(&format!("origine-{}", other.code())),
    };
    l.ta("programmi-cella-dichiarata", &[("dimensione", f.size(d.bytes).into()), ("fonte", source.into())])
}

/// Ultimo uso noto nella tabella: Steam, UserAssist solo con il tracciamento attivo, altrimenti
/// "nessun dato d'uso"; il perché (tracciamento spento, piattaforma che non registra) sta nella testata e
/// nel dettaglio. Mai "inutilizzato" o "mai usato".
fn usage_cell(l: &Lingue, f: &Formatter, p: &Program) -> String {
    match p.usage {
        Usage::Steam { manifest, local_config } => match manifest.or(local_config) {
            Some(t) => l.ta("programmi-uso-steam", &[("data", f.date(t).into())]),
            None => l.t("programmi-uso-steam-nessuna"),
        },
        Usage::UserAssist(Some(x)) => l.ta("programmi-uso-userassist", &[("data", f.date(x.last).into())]),
        Usage::UserAssist(None) => l.t("programmi-uso-userassist-nessuno"),
        Usage::NotTracked(_) | Usage::Unreadable(_) | Usage::NoFolder | Usage::NotRecorded => l.t("programmi-uso-nessun-dato"),
    }
}

/// Ordine della tabella: spazio misurato decrescente; poi i non misurati e quelli senza spazio, per nome.
fn sort_order(p: &Programs) -> Vec<usize> {
    let mut order: Vec<usize> = (0..p.items.len()).collect();
    let key = |i: usize| {
        let x = &p.items[i];
        let (rank, bytes) = match x.space {
            Space::Measured(b) => (0u8, b),
            Space::NotMeasured => (1, 0),
            Space::NotAttributed => (2, 0),
        };
        (rank, std::cmp::Reverse(bytes), x.name.to_lowercase(), i)
    };
    order.sort_by_cached_key(|&i| key(i));
    order
}

impl Cache {
    fn build(programs: Arc<Programs>, generation: u64, l: &Lingue, f: &Formatter) -> Self {
        let cells = programs
            .items
            .iter()
            .map(|p| {
                let (measured, figure) = measured_cell(l, f, p);
                let tag = if p.hidden {
                    Some(l.t("programmi-tag-componente"))
                } else if !p.shared_with.is_empty() {
                    Some(l.t("programmi-tag-condivisa"))
                } else {
                    None
                };
                Cells {
                    source: source_short(l, p),
                    version: p.version.clone().unwrap_or_default(),
                    measured,
                    figure,
                    declared: declared_cell(l, f, p),
                    usage: usage_cell(l, f, p),
                    installed: p.install_date.map(|d| f.day(d)).unwrap_or_default(),
                    tag,
                    lower: p.name.to_lowercase(),
                }
            })
            .collect();
        let residue_texts = programs.residues.iter().map(|r| r.record.identity.0.first().map(|fa| kbtext::fact_text(l, f, fa)).unwrap_or_default()).collect();
        Self { order: sort_order(&programs), programs, generation, lang: l.selected().to_owned(), units: f.units, cells, residue_texts, widths: None }
    }

    fn fresh(&self, d: &Loaded, l: &Lingue, f: &Formatter) -> bool {
        Arc::ptr_eq(&self.programs, &d.programs) && self.generation == d.generation && self.lang == l.selected() && self.units == f.units
    }
}

/// Spazio misurato dei programmi elencati: una cartella condivisa conta una volta, come in
/// `Programs::measured_total`, con cui coincide quando sono elencati tutti i programmi.
pub fn listed_total(p: &Programs, listed: &[usize]) -> u64 {
    let set: HashSet<usize> = listed.iter().copied().collect();
    listed
        .iter()
        .filter(|&&i| p.items.get(i).is_some_and(|x| x.shared_with.iter().all(|&j| j > i || !set.contains(&j))))
        .map(|&i| p.items[i].space.bytes())
        .sum()
}

// ---------------------------------------------------------------------------------------------
// Schermata

const PROGRAM_COLS: [Column; 7] = [
    Column { key: "colonna-nome", width: 0.0, right: false, drop: 0 },
    Column { key: "programmi-colonna-fonte", width: 110.0, right: false, drop: 3 },
    Column { key: "programmi-colonna-versione", width: 90.0, right: false, drop: 5 },
    Column { key: "colonna-dimensioni-su-disco", width: 118.0, right: true, drop: 0 },
    Column { key: "programmi-colonna-dichiarata", width: 110.0, right: true, drop: 2 },
    Column { key: "programmi-colonna-uso", width: 130.0, right: false, drop: 1 },
    Column { key: "programmi-colonna-installazione", width: 96.0, right: false, drop: 4 },
];

const RESIDUE_COLS: [Column; 5] = [
    Column { key: "colonna-cartella", width: 0.0, right: false, drop: 0 },
    Column { key: "programmi-colonna-base", width: 96.0, right: false, drop: 3 },
    Column { key: "colonna-ultima-modifica", width: 112.0, right: false, drop: 1 },
    Column { key: "colonna-dimensioni-su-disco", width: 118.0, right: true, drop: 0 },
    Column { key: "programmi-colonna-stato", width: 130.0, right: false, drop: 2 },
];

/// Richieste raccolte durante il disegno ed eseguite alla fine del frame.
enum Request {
    /// Mostra il nodo nell'Esplora di Quado.
    Explore(u32),
    /// Apre Esplora file selezionando il percorso.
    Reveal(String),
    /// Copia un comando o un URI del record negli appunti.
    Copy(String),
    /// Apre il dialogo di disinstallazione del programma.
    Uninstall(usize),
}

/// Programmi (variante A): come Esplora, con il dettaglio a destra su tutta l'altezza nel tono della barra
/// laterale. A sinistra prima la risposta (quanti programmi, quanto spazio misurato, quanti senza posizione) in
/// una fila di cifre, poi le fonti dell'uso, la barra con la sezione, la ricerca e i componenti di sistema, e
/// la tabella.
pub fn show(app: &mut App, ui: &mut Ui, p: &Palette) {
    let Some(d) = app.data.as_ref() else { return };
    let programs = Arc::clone(&d.programs);
    // Dati nuovi (altra scansione o "Rileggi dal disco"): selezioni e dialogo non valgono più.
    let st = &mut app.programs;
    match &st.cache {
        Some(c) if !Arc::ptr_eq(&c.programs, &programs) => {
            st.selected = None;
            st.selected_residue = None;
            st.dialog = None;
            st.scroll = IndexScroll::default();
            st.residue_scroll = IndexScroll::default();
        }
        Some(c) if c.generation != d.generation => {
            // Stesse prove: gli indici dei programmi non cambiano, quelli dei residui sì.
            st.dialog = None;
            st.selected_residue = None;
        }
        _ => {}
    }
    // Una selezione fatta prima del primo disegno (test, immagini) vale se l'indice esiste.
    if st.selected.is_some_and(|i| i >= programs.items.len()) {
        st.selected = None;
    }
    if st.selected_residue.is_some_and(|i| i >= programs.residues.len()) {
        st.selected_residue = None;
    }
    if st.dialog.as_ref().is_some_and(|x| x.program >= programs.items.len()) {
        st.dialog = None;
    }
    if !st.cache.as_ref().is_some_and(|c| c.fresh(d, &app.lingue, &app.fmt)) {
        st.cache = Some(Cache::build(Arc::clone(&programs), d.generation, &app.lingue, &app.fmt));
    }
    // Arrivo sulla schermata: la tabella prende il focus come in Esplora e File grandi; un dialogo rimasto
    // aperto quando si è usciti si chiude senza fare niente.
    if app.frame_count != st.last_frame + 1 {
        st.focus_pending = true;
        st.dialog = None;
    }
    st.last_frame = app.frame_count;

    let split = super::shell::split(ui, p);
    app.programs.detail = split.detail_full;
    let mut requests: Vec<Request> = Vec::new();
    let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(split.content).layout(Layout::top_down(Align::Min)));
    {
        let ui = &mut lui;
        super::overview::interrupted_banner(app, ui, p);
        summary(app, ui, p, &programs);
        ui.add_space(14.0);
        toolbar(app, ui, p, &programs);
        ui.add_space(12.0);
    }
    let rest = lui.available_rect_before_wrap();
    let mut tui = lui.new_child(egui::UiBuilder::new().max_rect(rest).layout(Layout::top_down(Align::Min)));
    let mut rui = ui.new_child(egui::UiBuilder::new().max_rect(split.detail).layout(Layout::top_down(Align::Min)));
    match app.programs.section {
        Section::Programs => {
            program_table(app, &mut tui, p, &programs, &mut requests);
            program_detail(app, &mut rui, p, &programs, &mut requests);
        }
        Section::Residues => {
            residue_table(app, &mut tui, p, &programs, &mut requests);
            residue_detail(app, &mut rui, p, &programs, &mut requests);
        }
    }
    dialog(app, ui.ctx(), p, &programs);

    for r in requests {
        match r {
            Request::Explore(n) => super::explore::show_in_folder(app, n),
            Request::Reveal(path) => {
                if let Err(e) = (app.programs.reveal)(&path) {
                    app.notice.set(app.lingue.ta("avviso-shell-errore", &[("codice", format!("0x{:08X}", e.0 as u32).into())]));
                }
            }
            Request::Copy(c) => {
                ui.ctx().copy_text(c);
                app.notice.set(app.lingue.t("avviso-comando-copiato"));
            }
            Request::Uninstall(i) => app.programs.dialog = Some(UninstallDialog { program: i, copied: false, outcome: None }),
        }
    }
}

/// Programmi elencati dall'interruttore dei componenti di sistema (senza la ricerca).
fn listed(programs: &Programs, show_hidden: bool) -> Vec<usize> {
    (0..programs.items.len()).filter(|&i| show_hidden || !programs.items[i].hidden).collect()
}

/// La risposta in testa (`.nums`): programmi elencati, spazio misurato, posizione non dichiarata, ognuno con
/// la sua nota; sotto, da dove viene l'uso noto e le fonti non lette.
fn summary(app: &App, ui: &mut Ui, p: &Palette, programs: &Programs) {
    let (l, f) = (&app.lingue, &app.fmt);
    let shown = listed(programs, app.programs.show_hidden);
    let not_declared = shown.iter().filter(|&&i| programs.items[i].location == Location::NotDeclared).count() as u64;
    let note = l.t(if app.programs.show_hidden { "programmi-tessera-con-componenti" } else { "programmi-tessera-senza-componenti" });
    let items: Vec<widgets::Figure> = vec![
        (f.count(shown.len() as u64), l.t("programmi-tessera-programmi"), Some(note)),
        (f.size(listed_total(programs, &shown)), l.t("programmi-tessera-spazio"), Some(l.t("programmi-tessera-spazio-nota"))),
        (f.count(not_declared), l.t("programmi-tessera-non-dichiarata"), Some(l.t("programmi-tessera-non-dichiarata-nota"))),
    ];
    widgets::bare_card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        Frame::new().inner_margin(Margin { left: 4, right: 4, top: 6, bottom: 4 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::figures(ui, p, p.panel, 28.0, &items);
        });
    });
    ui.add_space(10.0);
    // Uso noto: con il tracciamento spento il testo che lo dice e nessun valore UserAssist.
    let tracking = match programs.tracking {
        TrackProgs::Off => l.t("programmi-tracciamento-spento"),
        TrackProgs::On => l.t("programmi-tracciamento-attivo"),
        _ => l.t("programmi-tracciamento-non-letto"),
    };
    let e = if matches!(programs.tracking, TrackProgs::On | TrackProgs::Off) { Evidence::Source } else { Evidence::Unavailable };
    fact(ui, p, l, e, &tracking);
    fact(ui, p, l, Evidence::Source, &l.t("programmi-uso-copertura"));
    // Fonti non lette o elementi saltati: il motivo, mai dati ricostruiti.
    for s in &programs.errors {
        let source = l.t(match s.extractor {
            Extractor::Uninstall => "fonti-disinstallazione",
            Extractor::Msix => "fonti-msix",
            Extractor::Steam => "fonti-steam",
            Extractor::Epic => "fonti-epic",
            Extractor::UserAssist => "fonti-userassist",
        });
        let why = system_view::extract_error(l, &s.error);
        let text = match &s.path {
            Some(path) => l.ta("programmi-fonte-errore-elemento", &[("fonte", source.into()), ("motivo", why.into()), ("percorso", path.clone().into())]),
            None => l.ta("programmi-fonte-errore", &[("fonte", source.into()), ("motivo", why.into())]),
        };
        fact(ui, p, l, Evidence::Unavailable, &text);
    }
}

/// Fatto con il glifo della provenienza e l'etichetta nel suggerimento, come nell'ispettore.
fn fact(ui: &mut Ui, p: &Palette, l: &Lingue, e: Evidence, s: &str) {
    inspector::fact(ui, p, e, s, l);
}

/// Testo che va a capo nella larghezza disponibile, nel ruolo Callout.
fn note(ui: &mut Ui, s: impl Into<String>, color: Color32) {
    widgets::paragraph(ui, s, Type::Callout, color);
}

/// Sezione (programmi o residui) come controllo segmentato, ricerca e componenti di sistema.
fn toolbar(app: &mut App, ui: &mut Ui, p: &Palette, programs: &Programs) {
    let hidden = programs.items.iter().filter(|x| x.hidden).count() as u64;
    let dialog_open = app.programs.dialog.is_some();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(10.0, 8.0);
        let (l, f) = (&app.lingue, &app.fmt);
        let st = &mut app.programs;
        let n = listed(programs, st.show_hidden).len() as u64;
        let r = programs.residues.len() as u64;
        let options = [
            (Section::Programs, l.ta("programmi-sezione-programmi", &[("testo", f.count(n).into())]), Some(Icon::Programs)),
            (Section::Residues, l.ta("programmi-sezione-residui", &[("testo", f.count(r).into())]), Some(Icon::Folder)),
        ];
        if widgets::segmented(ui, p, ("programmi", "sezione"), &mut st.section, &options) {
            st.focus_pending = true;
        }
        if st.section != Section::Programs {
            return;
        }
        ui.add_space(6.0);
        // Il segnaposto entra intero anche in una traduzione più lunga.
        let hint = l.t("programmi-cerca");
        let hint_w = ui.painter().layout_no_wrap(hint.clone(), widgets::sans(13.0), p.text_3).size().x;
        let te = widgets::search_field(ui, p, Id::new(("programmi", "ricerca")), &mut st.query, &hint, hint_w.max(220.0) + 56.0);
        if te.has_focus() && !dialog_open && ui.input(|i| i.key_pressed(Key::Escape)) {
            st.query.clear();
        }
        if hidden > 0 {
            let label = l.ta("programmi-mostra-componenti", &[("testo", f.count(hidden).into())]);
            // Una casella: dice da sola se i componenti sono mostrati.
            let mut on = st.show_hidden;
            if widgets::checkbox(ui, p, &mut on, &label, true).changed() {
                st.show_hidden = on;
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Tabella dei programmi

/// Larghezze delle colonne fisse: abbastanza per l'intestazione e per ogni testo della colonna, così
/// nessun testo tradotto si tronca. La colonna del nome prende il resto.
fn program_widths(ui: &Ui, l: &Lingue, cache: &Cache) -> Vec<f32> {
    let painter = ui.painter();
    let base = table::fit_widths(painter, l, &PROGRAM_COLS);
    let callout = widgets::font(Type::Callout);
    let measure = |s: &str, font: egui::FontId| painter.layout_no_wrap(s.to_owned(), font, Color32::WHITE).size().x;
    let mut out: Vec<f32> = base.iter().map(|c| c.width).collect();
    for c in &cache.cells {
        out[1] = out[1].max(measure(&c.source, callout.clone()) + 20.0);
        out[2] = out[2].max(measure(&c.version, callout.clone()) + 20.0);
        out[3] = out[3].max(measure(&c.measured, if c.figure { widgets::num(13.0) } else { callout.clone() }) + 20.0);
        out[4] = out[4].max(measure(&c.declared, callout.clone()) + 20.0);
        out[5] = out[5].max(measure(&c.usage, callout.clone()) + 20.0);
        out[6] = out[6].max(measure(&c.installed, callout.clone()) + 20.0);
    }
    out.iter_mut().for_each(|w| *w = w.ceil());
    out
}

/// Tabella come quella di Esplora (`.grid`): intestazione in caption con il filetto sotto, righe con il fondo
/// della selezione e del passaggio del mouse arrotondato, nessun bordo.
fn program_table(app: &mut App, ui: &mut Ui, p: &Palette, programs: &Programs, requests: &mut Vec<Request>) {
    let l = &app.lingue;
    let st = &mut app.programs;
    let Some(cache) = st.cache.as_mut() else { return };
    // Righe: interruttore dei componenti di sistema e ricerca per nome.
    let q = st.query.trim().to_lowercase();
    st.rows = cache.order.iter().copied().filter(|&i| (st.show_hidden || !programs.items[i].hidden) && (q.is_empty() || cache.cells[i].lower.contains(&q))).collect();
    if st.selected.is_some_and(|s| !st.rows.contains(&s)) {
        st.selected = None;
    }
    let full = ui.available_rect_before_wrap();
    if st.rows.is_empty() {
        let any_source = programs.errors.iter().filter(|e| e.path.is_none() && e.extractor != Extractor::UserAssist).count() < 4;
        let msg = if !programs.items.is_empty() {
            l.t("programmi-vuoto-ricerca")
        } else if any_source {
            l.t("programmi-vuoto-nessuno")
        } else {
            l.t("programmi-vuoto-senza-fonti")
        };
        super::shell::empty_state(ui, p, full, &msg, None);
        return;
    }
    if cache.widths.is_none() {
        cache.widths = Some(program_widths(ui, l, cache));
    }
    let widths = cache.widths.clone().unwrap_or_default();
    let cols: Vec<Column> = PROGRAM_COLS.iter().zip(&widths).map(|(c, &w)| Column { width: if c.width == 0.0 { 0.0 } else { w }, ..*c }).collect();
    let header = Rect::from_min_size(full.min, vec2(full.width(), table::HEADER_H));
    let xs = table::layout(&cols, header.min.x, header.max.x - index_scroll::BAR_WIDTH, 200.0);
    // Le righe sono in ordine di dimensioni su disco, decrescente.
    table::header_sorted(ui.painter(), p, l, &cols, &xs, header, Some(3));
    let body = Rect::from_min_max(pos2(full.min.x, header.max.y), full.max);
    let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(Layout::top_down(Align::Min)));
    let id = Id::new(("programmi", "tabella"));
    let colors = BarColors { track: p.bg, thumb: p.border, thumb_active: p.text_3 };
    let rows = &st.rows;
    let sel = st.selected;
    let hovered_prev = st.hovered;
    let row_h = table::row_height(ui);
    let callout = widgets::font(Type::Callout);
    let out = index_scroll::show_rows(&mut bui, id, &mut st.scroll, rows.len(), row_h, colors, |painter, row, rect| {
        let i = rows[row];
        let x = &programs.items[i];
        let c = &cache.cells[i];
        if Some(i) == sel {
            painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.select);
        } else if hovered_prev == Some(row) {
            painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.hover);
        }
        let cy = rect.center().y;
        if let Some((a, b)) = xs[0] {
            // Forma dei programmi se c'è una misura, tratteggio se no.
            if c.figure {
                widgets::manager_glyph(painter, pos2(a + 15.0, cy), Gestore::App, p);
            } else {
                widgets::glyph(painter, pos2(a + 15.0, cy), 10.0, Glyph::Hatch, p.hatch, p);
            }
            let nx = a + 28.0;
            let room = b - nx - 8.0;
            // L'indicazione si disegna intera o non si disegna; il nome (un dato) si tronca.
            let tag = c.tag.as_ref().map(|t| painter.layout_no_wrap(t.clone(), callout.clone(), p.text_2)).filter(|g| g.size().x + 90.0 < room);
            let tag_w = tag.as_ref().map_or(0.0, |g| g.size().x + 10.0);
            let g = widgets::elided(painter, x.name.clone(), widgets::sans(13.0), p.text, room - tag_w);
            let gw = g.size().x;
            painter.galley(pos2(nx, cy - g.size().y / 2.0), g, p.text);
            if let Some(t) = tag {
                painter.galley(pos2(nx + gw + 10.0, cy - t.size().y / 2.0), t, p.text_2);
            }
        }
        if let Some((a, _)) = xs[1] {
            painter.text(pos2(a + 10.0, cy), Align2::LEFT_CENTER, &c.source, callout.clone(), p.text_2);
        }
        if let Some((a, _)) = xs[2] {
            painter.text(pos2(a + 10.0, cy), Align2::LEFT_CENTER, &c.version, callout.clone(), p.text_2);
        }
        if let Some((_, b)) = xs[3] {
            let (font, color) = if c.figure { (widgets::num(13.0), p.text) } else { (callout.clone(), p.text_2) };
            widgets::right_text(painter, pos2(b - 10.0, cy), c.measured.clone(), font, color);
        }
        if let Some((_, b)) = xs[4] {
            widgets::right_text(painter, pos2(b - 10.0, cy), c.declared.clone(), callout.clone(), p.text_2);
        }
        if let Some((a, _)) = xs[5] {
            painter.text(pos2(a + 10.0, cy), Align2::LEFT_CENTER, &c.usage, callout.clone(), p.text_2);
        }
        if let Some((a, _)) = xs[6] {
            painter.text(pos2(a + 10.0, cy), Align2::LEFT_CENTER, &c.installed, callout.clone(), p.text_2);
        }
    });
    st.hovered = out.hovered_row;
    let resp = out.response.clone();
    let a11y = l.t("programmi-accessibile-tabella");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &a11y));
    let dialog_open = st.dialog.is_some();
    keyboard_table(ui, id, &resp, full, p, &mut st.focus_pending, dialog_open);
    if let Some(r) = out.clicked_row {
        st.selected = st.rows.get(r).copied();
    }
    let mut activate = resp.double_clicked().then_some(out.hovered_row).flatten().and_then(|r| st.rows.get(r).copied());
    if resp.has_focus() && !dialog_open {
        let k = NavKeys::read(ui);
        let page = (out.visible.floor() as usize).saturating_sub(1).max(1);
        let cur = st.selected.and_then(|s| st.rows.iter().position(|&x| x == s));
        if let Some(r) = k.apply(cur, st.rows.len(), page)
            && (Some(r) != cur || k.moves())
        {
            st.selected = st.rows.get(r).copied();
            st.scroll.ensure_visible(r, st.rows.len(), out.visible);
        }
        if k.enter {
            activate = st.selected;
        }
    }
    // Invio e doppio clic: la cartella del programma nell'Esplora, come Invio in File grandi.
    if let Some(n) = activate.and_then(|i| programs.items[i].location.node()) {
        requests.push(Request::Explore(n));
    }
}

/// Focus della tabella: richiesta all'arrivo, al clic; l'anello dentro l'area quando ce l'ha (come in Esplora).
/// Le frecce restano alla tabella.
fn keyboard_table(ui: &Ui, id: Id, resp: &Response, full: Rect, p: &Palette, focus_pending: &mut bool, dialog_open: bool) {
    ui.memory_mut(|m| m.set_focus_lock_filter(id, egui::EventFilter { horizontal_arrows: false, vertical_arrows: true, escape: false, ..Default::default() }));
    if *focus_pending && !dialog_open {
        resp.request_focus();
        *focus_pending = false;
    }
    if resp.clicked() {
        resp.request_focus();
    }
    if resp.has_focus() {
        widgets::focus_ring_inside(ui.painter(), full, p, tokens::radius::ROW);
    }
}

// ---------------------------------------------------------------------------------------------
// Dettaglio di un programma

/// Dove va un fatto del record nel dettaglio: ogni fatto compare una volta, nella sua sezione.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Identity,
    Location,
    Space,
    Usage,
    YourFiles,
}

fn part_of(key: &str) -> Part {
    match key {
        "fatto-programma-tuoi-file" => Part::YourFiles,
        "fatto-programma-posizione-non-dichiarata" => Part::Location,
        k if k.starts_with("fatto-programma-cartella") => Part::Location,
        "fatto-programma-su-disco" | "fatto-programma-non-misurato" | "fatto-programma-nessuno-spazio" | "fatto-programma-solo-dati" | "fatto-programma-dati-regole" | "fatto-dimensione-dichiarata" => {
            Part::Space
        }
        "fatto-ultima-sessione-steam" | "fatto-uso-non-registrato" | "fatto-epic-nessun-dato-uso" => Part::Usage,
        k if k.starts_with("fatto-uso-") => Part::Usage,
        _ => Part::Identity,
    }
}

fn facts_of(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, r: &Record, part: Part) {
    for fa in r.identity.0.iter().filter(|fa| part_of(&fa.key) == part) {
        inspector::fact_in(ui, p, kbtext::evidence(fa.provenance), &kbtext::fact_with_source(l, f, fa), l, p.text);
    }
}

fn evidence_label(l: &Lingue, e: PathEvidence) -> String {
    l.t(match e {
        PathEvidence::InstallLocation => "programmi-prova-installlocation",
        PathEvidence::UninstallHint => "programmi-prova-indizio",
        PathEvidence::MsixInstalled => "programmi-prova-msix",
        PathEvidence::SteamManifest => "programmi-prova-steam",
        PathEvidence::EpicManifest => "programmi-prova-epic",
    })
}

/// Testata del dettaglio, come l'ispettore: nome, tipo, la cifra grande e leggera (o perché non c'è) e la nota.
fn detail_head(ui: &mut Ui, p: &Palette, name: &str, kind: &str, figure: Result<String, String>, note_text: Option<&str>) {
    let w = ui.available_width();
    let title = widgets::label_elided(ui, name, widgets::font(Type::Title), p.text, w);
    title.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, name));
    widgets::paragraph(ui, kind, Type::Callout, p.text_2);
    ui.add_space(10.0);
    match figure {
        Ok(v) => {
            ui.add(egui::Label::new(egui::RichText::new(v).font(widgets::light(34.0)).color(p.text).extra_letter_spacing(-0.8)));
        }
        Err(s) => {
            widgets::paragraph(ui, s, Type::Title, p.text_2);
        }
    }
    if let Some(n) = note_text {
        note(ui, n, p.text_2);
    }
}

/// Il dettaglio a destra (`.insp`): testata con la misura, l'avviso dei tuoi file, "Cosa puoi fare" per primo
/// con «Disinstalla…» e la scheda apribile, poi le sezioni apribili (Cos'è, Posizione, Spazio, Dati dalle
/// regole, Uso noto, Funzione) e in fondo le azioni sempre disponibili.
fn program_detail(app: &mut App, ui: &mut Ui, p: &Palette, programs: &Programs, requests: &mut Vec<Request>) {
    let (l, f) = (&app.lingue, &app.fmt);
    let Some((i, x)) = app.programs.selected.and_then(|i| programs.items.get(i).map(|x| (i, x))) else {
        widgets::paragraph(ui, l.t("programmi-nessuna-selezione"), Type::Body, p.text_2);
        return;
    };
    let Some(d) = app.data.as_ref() else { return };
    let t = d.tree();
    let dialog_open = app.programs.dialog.is_some();
    let mut sections = std::mem::take(&mut app.programs.sections);
    ScrollArea::vertical().id_salt(("programmi", "dettaglio")).auto_shrink([false, false]).show(ui, |ui| {
        // Misura: la cifra solo con una prova del percorso o dati dalle regole.
        let figure = match (x.space, &x.location) {
            (Space::Measured(b), _) => Ok(f.size(b)),
            (Space::NotMeasured, _) => Err(l.t("programmi-titolo-non-misurato")),
            (Space::NotAttributed, Location::Implausible { .. }) => Err(l.t("programmi-titolo-non-plausibile")),
            (Space::NotAttributed, _) => Err(l.t("programmi-titolo-non-dichiarata")),
        };
        let measured_note = figure.is_ok().then(|| l.t("programmi-misurato-nota"));
        detail_head(ui, p, &x.name, &source_long(l, x), figure, measured_note.as_deref());
        // Tuoi file nella cartella: prima di tutto il resto.
        for fa in x.record.identity.0.iter().filter(|fa| part_of(&fa.key) == Part::YourFiles) {
            ui.add_space(12.0);
            widgets::banner(ui, p, &kbtext::fact_text(l, f, fa));
        }

        // Cosa puoi fare, per primo: stato, «Disinstalla…» e la scheda del record apribile.
        let mut details = sections.is_open("scheda");
        inspector::section(ui, p, &mut sections, "cosa", &l.t("ispettore-cosa-puoi-fare"), |ui| {
            todo_box(ui, p, |ui| {
                inspector::state_line(ui, p, l, &x.record.state);
                if let State::Unknown(reason) = x.record.state {
                    note(ui, kbtext::unknown_reason(l, reason, None), p.text_2);
                }
                note(ui, kbtext::estimate_text(l, f, &x.record.estimate), p.text_2);
                if x.uninstall.is_some() {
                    ui.add_space(10.0);
                    let w = ui.available_width();
                    if inspector::wide_button(ui, p, &l.t("programmi-disinstalla"), None, w, true, !dialog_open).clicked() {
                        requests.push(Request::Uninstall(i));
                    }
                }
                ui.add_space(6.0);
                ui.push_id("scheda_programma", |ui| widgets::disclosure(ui, p, &l.t("spostamento-scheda"), Type::Callout, &mut details));
                if details {
                    record_card(ui, p, l, f, &x.record, requests);
                }
            });
        });
        sections.set("scheda", details);

        inspector::section(ui, p, &mut sections, "cose", &l.t("ispettore-cos-e"), |ui| facts_of(ui, p, l, f, &x.record, Part::Identity));
        inspector::section(ui, p, &mut sections, "posizione", &l.t("programmi-sezione-posizione"), |ui| {
            facts_of(ui, p, l, f, &x.record, Part::Location);
            if let Location::Folder { evidence, .. } | Location::Implausible { evidence, .. } = &x.location {
                note(ui, evidence_label(l, *evidence), p.text_2);
            }
            if let Location::Folder { provenance, .. } = &x.location {
                let class = l.t(kbtext::evidence(*provenance).key());
                note(ui, l.ta("programmi-provenienza", &[("classe", class.into())]), p.text_2);
                if matches!(provenance, Provenance::Deduced(_)) {
                    note(ui, kbtext::reliability_label(l, x.record.reliability), p.text_2);
                }
            }
            if !x.shared_with.is_empty() {
                let names: Vec<&str> = x.shared_with.iter().filter_map(|&j| programs.items.get(j)).map(|o| o.name.as_str()).collect();
                note(ui, l.ta("programmi-condivisa-con", &[("nomi", names.join(", ").into())]), p.text_2);
            }
        });
        inspector::section(ui, p, &mut sections, "spazio", &l.t("programmi-sezione-spazio"), |ui| {
            facts_of(ui, p, l, f, &x.record, Part::Space);
            if x.declared.is_some() {
                note(ui, l.t("programmi-dichiarato-nota"), p.text_2);
            }
        });
        if !x.data.is_empty() {
            inspector::section(ui, p, &mut sections, "dati", &l.t("programmi-sezione-dati"), |ui| rule_data(ui, p, l, f, d, t, &x.data, requests));
        }
        inspector::section(ui, p, &mut sections, "uso", &l.t("programmi-sezione-uso"), |ui| {
            facts_of(ui, p, l, f, &x.record, Part::Usage);
            if matches!(x.usage, Usage::Steam { .. }) {
                note(ui, l.t("programmi-copertura-steam"), p.text_2);
            }
        });
        inspector::section(ui, p, &mut sections, "funzione", &l.t("programmi-sezione-funzione"), |ui| {
            widgets::paragraph(ui, l.t("funzione-programma"), Type::BodyStrong, p.text);
            note(ui, l.t("funzione-programma-descrizione"), p.text_2);
        });

        // Azioni sempre disponibili, in due colonne come nell'ispettore.
        let folder = match &x.location {
            Location::Folder { path, .. } => Some(path.clone()),
            _ => None,
        };
        let node = x.location.node();
        if folder.is_some() || node.is_some() {
            ui.add_space(18.0);
            widgets::typed(ui, l.t("ispettore-azioni"), Type::BodyStrong, p.text);
            ui.add_space(8.0);
            ui.push_id("programma_azioni", |ui| {
                if let Some(r) = action_pair(ui, p, l, folder.is_some(), node.is_some()) {
                    match (r, &folder, node) {
                        (0, Some(path), _) => requests.push(Request::Reveal(path.clone())),
                        (1, _, Some(n)) => requests.push(Request::Explore(n)),
                        _ => {}
                    }
                }
            });
        }
        ui.add_space(12.0);
    });
    app.programs.sections = sections;
}

/// Riquadro di "Cosa puoi fare" (`.todo`): fondo `panel`, raggio 12, ombra in chiaro, margine 14.
fn todo_box(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui)) {
    Frame::new().fill(p.panel).corner_radius(CornerRadius::same(tokens::radius::GROUP)).shadow(p.shadow(tokens::Elevation::Card)).inner_margin(Margin::same(14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 3.0;
        add(ui);
    });
}

/// «Apri in Esplora file» e «Mostra in Esplora» affiancati (`.insp .acts`), o uno sotto l'altro se i nomi
/// tradotti non ci stanno. Restituisce quale è stato premuto (0 o 1).
fn action_pair(ui: &mut Ui, p: &Palette, l: &Lingue, reveal: bool, explore: bool) -> Option<usize> {
    let gap = 6.0;
    let col = ((ui.available_width() - gap) / 2.0).floor();
    let buttons: Vec<(usize, Icon, String)> = [(0, Icon::External, l.t("azione-apri-in-esplora"), reveal), (1, Icon::Explore, l.t("programmi-mostra-in-esplora"), explore)]
        .into_iter()
        .filter(|b| b.3)
        .map(|(k, i, s, _)| (k, i, s))
        .collect();
    let fits = buttons.iter().all(|b| ui.painter().layout_no_wrap(b.2.clone(), widgets::medium(13.0), p.text).size().x + 24.0 + 23.0 <= col);
    let (per_row, col) = if fits { (2, col) } else { (1, ui.available_width()) };
    let mut out = None;
    for pair in buttons.chunks(per_row) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (k, icon, label) in pair {
                if inspector::wide_button(ui, p, label, Some(*icon), col, false, true).clicked() {
                    out = Some(*k);
                }
            }
        });
    }
    out
}

/// Dati dalle regole (dentro e fuori dalla cartella), raggruppati per regola: nome della regola, numero di
/// elementi, dimensione e "Mostra in Esplora" sull'elemento più grande.
#[allow(clippy::too_many_arguments)]
fn rule_data(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, d: &Loaded, t: &Tree, data: &[RuleData], requests: &mut Vec<Request>) {
    let mut groups: Vec<(usize, bool, Vec<&RuleData>)> = Vec::new();
    for x in data {
        match groups.iter_mut().find(|g| g.0 == x.rule && g.1 == x.inside) {
            Some(g) => g.2.push(x),
            None => groups.push((x.rule, x.inside, vec![x])),
        }
    }
    groups.sort_by_key(|g| std::cmp::Reverse(g.2.iter().map(|x| x.bytes).sum::<u64>()));
    for (k, (rule, inside, items)) in groups.into_iter().enumerate() {
        let bytes: u64 = items.iter().map(|x| x.bytes).sum();
        let n = items.len() as u64;
        let sub = l.ta(if inside { "programmi-dati-dentro" } else { "programmi-dati-fuori" }, &[("n", n.into()), ("n_testo", f.count(n).into())]);
        let name = kbtext::rule_name(l, &d.kb, rule);
        let w = ui.available_width();
        let painter = ui.painter().clone();
        let sg = painter.layout_no_wrap(f.size(bytes), widgets::num(12.0), p.text);
        let name_w = (w - 20.0 - sg.size().x - 12.0).max(60.0);
        let ng = widgets::galley(&painter, name.clone(), Type::Callout, p.text, Some(name_w));
        let bg = widgets::galley(&painter, sub, Type::Callout, p.text_2, Some(name_w));
        let h = 10.0 + ng.size().y + 1.0 + bg.size().y;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h.max(28.0)), Sense::hover());
        if k > 0 {
            painter.hline((rect.min.x + 20.0)..=rect.max.x, rect.min.y + 0.5, Stroke::new(1.0, p.separator));
        }
        let top = rect.min.y + 5.0;
        let line_h = ng.rows.first().map_or(ng.size().y, |r| r.height());
        let cy = top + line_h / 2.0;
        widgets::manager_glyph(&painter, pos2(rect.min.x + 5.0, cy), Gestore::App, p);
        let sw = sg.size();
        painter.galley(pos2(rect.max.x - sw.x, cy - sw.y / 2.0), sg, p.text);
        let nh = ng.size().y;
        painter.galley(pos2(rect.min.x + 20.0, top), ng, p.text);
        painter.galley(pos2(rect.min.x + 20.0, top + nh + 1.0), bg, p.text_2);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
        // Il più grande, se è ancora un nodo dell'albero.
        if let Some(top_node) = items.iter().max_by_key(|x| x.bytes).map(|x| x.node).filter(|&n| (n as usize) < t.len()) {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                if ui.push_id(("dati_regola", k), |ui| widgets::button(ui, p, &l.t("programmi-mostra-in-esplora"), Some(Icon::Explore), ButtonKind::Quiet, true)).inner.clicked() {
                    requests.push(Request::Explore(top_node));
                }
            });
        }
    }
}

/// Scheda del record, sotto lo stato di "Cosa puoi fare": azione, metodo ufficiale e alternative
/// con il comando da copiare, controlli, privilegi, ipotesi, rischi, reversibilità, errore, verifica.
fn record_card(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, r: &Record, requests: &mut Vec<Request>) {
    let _ = f;
    let line = |ui: &mut Ui, label: &str, value: &str| note(ui, format!("{label}: {value}"), p.text_2);
    line(ui, &l.t("record-azione"), &kbtext::action_label(l, &r.action));
    let step = |ui: &mut Ui, s: &quado_model::record::Step, requests: &mut Vec<Request>| {
        note(ui, l.t(&s.key), p.text);
        if let Some(c) = s.command.as_ref().or(s.uri.as_ref()) {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::Label::new(egui::RichText::new(c).font(mono(12.0)).color(p.text)).wrap());
                if widgets::button(ui, p, &l.t("record-copia"), Some(Icon::Copy), ButtonKind::Quiet, true).clicked() {
                    requests.push(Request::Copy(c.clone()));
                }
            });
        }
    };
    if let Some(s) = &r.preferred {
        widgets::typed(ui, l.t("record-metodo"), Type::Caption, p.text_2);
        step(ui, s, requests);
    }
    if !r.alternatives.is_empty() {
        widgets::typed(ui, l.t("record-alternative"), Type::Caption, p.text_2);
        for s in &r.alternatives {
            step(ui, s, requests);
        }
    }
    if !r.checks.is_empty() {
        widgets::typed(ui, l.t("record-controlli"), Type::Caption, p.text_2);
        for c in &r.checks {
            note(ui, format!("{} · {}", l.t(&c.key), kbtext::check_outcome(l, c.outcome)), p.text_2);
        }
    }
    line(ui, &l.t("record-privilegi"), &kbtext::privileges_label(l, r.privileges));
    for h in &r.hypotheses {
        note(ui, l.t(h), p.text_2);
    }
    for k in &r.risks {
        note(ui, l.t(k), p.warn);
    }
    line(ui, &l.t("record-reversibilita"), &l.t(&r.reversibility));
    line(ui, &l.t("record-errore"), &l.t(&r.on_error));
    line(ui, &l.t("record-verifica"), &l.t(&r.after));
}

// ---------------------------------------------------------------------------------------------
// Possibili residui

fn base_name(b: ResidueBase) -> &'static str {
    match b {
        ResidueBase::Local => "Local",
        ResidueBase::Roaming => "Roaming",
        ResidueBase::LocalLow => "LocalLow",
        ResidueBase::ProgramData => "ProgramData",
    }
}

fn residue_table(app: &mut App, ui: &mut Ui, p: &Palette, programs: &Programs, requests: &mut Vec<Request>) {
    let Some(d) = app.data.as_ref() else { return };
    let t = d.tree();
    let (l, f) = (&app.lingue, &app.fmt);
    let st = &mut app.programs;
    let Some(cache) = st.cache.as_ref() else { return };
    // Testata con il criterio: residuo solo come possibilità, nessuna proposta.
    note(ui, l.t("programmi-residui-regola"), p.text_2);
    ui.add_space(10.0);
    let full = ui.available_rect_before_wrap();
    let residues = &programs.residues;
    if st.selected_residue.is_some_and(|s| s >= residues.len()) {
        st.selected_residue = None;
    }
    if residues.is_empty() {
        super::shell::empty_state(ui, p, full, &l.t("programmi-residui-nessuno"), None);
        return;
    }
    let state = State::Unknown(UnknownReason::NameOnly);
    let state_text = kbtext::state_label(l, &state);
    let callout = widgets::font(Type::Callout);
    let cols = table::fit_widths(ui.painter(), l, &RESIDUE_COLS);
    let cols: Vec<Column> = cols
        .iter()
        .enumerate()
        .map(|(k, c)| {
            // Lo stato con l'icona deve entrare intero.
            let w = if k == 4 { c.width.max(ui.painter().layout_no_wrap(state_text.clone(), callout.clone(), Color32::WHITE).size().x + 44.0) } else { c.width };
            Column { width: w.ceil(), ..*c }
        })
        .collect();
    let header = Rect::from_min_size(full.min, vec2(full.width(), table::HEADER_H));
    let xs = table::layout(&cols, header.min.x, header.max.x - index_scroll::BAR_WIDTH, 200.0);
    table::header(ui.painter(), p, l, &cols, &xs, header);
    // Seconda riga: il testo onesto con la data, a capo e mai troncato. L'altezza delle righe segue il testo
    // più lungo alla larghezza di questo frame.
    let text_left = xs[0].map_or(header.min.x, |(a, _)| a + 28.0);
    let text_right = xs.iter().rev().find_map(|x| x.map(|(_, b)| b)).unwrap_or(header.max.x) - 10.0;
    let text_w = (text_right - text_left).max(80.0);
    let text_h = cache.residue_texts.iter().map(|s| widgets::galley(ui.painter(), s.clone(), Type::Callout, p.text_2, Some(text_w)).size().y).fold(0.0, f32::max);
    let row_h = (12.0 + 18.0 + 3.0 + text_h + 11.0).ceil();
    let body = Rect::from_min_max(pos2(full.min.x, header.max.y), full.max);
    let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(Layout::top_down(Align::Min)));
    let id = Id::new(("programmi", "residui"));
    let colors = BarColors { track: p.bg, thumb: p.border, thumb_active: p.text_3 };
    let sel = st.selected_residue;
    let hovered_prev = st.hovered;
    let out = index_scroll::show_rows(&mut bui, id, &mut st.residue_scroll, residues.len(), row_h, colors, |painter, row, rect| {
        let r = &residues[row];
        if Some(row) == sel {
            painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.select);
        } else if hovered_prev == Some(row) {
            painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.hover);
        }
        let cy = rect.min.y + 12.0 + 9.0;
        if let Some((a, b)) = xs[0] {
            // Anello del non classificato: nessun gestore attribuito (Non sappiamo).
            widgets::manager_glyph(painter, pos2(a + 15.0, cy), Gestore::Unknown, p);
            let g = table::path_elided_start(painter, &t.path_lossy(r.node), mono(12.5), p.text, b - a - 38.0);
            painter.galley(pos2(a + 28.0, cy - g.size().y / 2.0), g, p.text);
        }
        if let Some((a, _)) = xs[1] {
            painter.text(pos2(a + 10.0, cy), Align2::LEFT_CENTER, base_name(r.base), callout.clone(), p.text_2);
        }
        if let Some((a, _)) = xs[2] {
            let date = r.last_modified.map(|m| f.date(m)).unwrap_or_default();
            painter.text(pos2(a + 10.0, cy), Align2::LEFT_CENTER, date, widgets::num(13.0), p.text_2);
        }
        if let Some((_, b)) = xs[3] {
            widgets::right_text(painter, pos2(b - 10.0, cy), f.size(t.alloc(r.node)), widgets::num(13.0), p.text);
        }
        if let Some((a, _)) = xs[4] {
            icons::paint(painter, Rect::from_center_size(pos2(a + 18.0, cy), vec2(14.0, 14.0)), kbtext::state_icon(&state), p.text_2);
            painter.text(pos2(a + 32.0, cy), Align2::LEFT_CENTER, &state_text, callout.clone(), p.text_2);
        }
        let g = widgets::galley(painter, cache.residue_texts[row].clone(), Type::Callout, p.text_2, Some(text_w));
        painter.galley(pos2(text_left, rect.min.y + 12.0 + 18.0 + 3.0), g, p.text_2);
    });
    st.hovered = out.hovered_row;
    let resp = out.response.clone();
    let a11y = l.t("programmi-accessibile-residui");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &a11y));
    let dialog_open = st.dialog.is_some();
    keyboard_table(ui, id, &resp, full, p, &mut st.focus_pending, dialog_open);
    if let Some(r) = out.clicked_row {
        st.selected_residue = Some(r);
    }
    let mut activate = resp.double_clicked().then_some(out.hovered_row).flatten();
    if resp.has_focus() && !dialog_open {
        let k = NavKeys::read(ui);
        let page = (out.visible.floor() as usize).saturating_sub(1).max(1);
        if let Some(r) = k.apply(st.selected_residue, residues.len(), page)
            && (Some(r) != st.selected_residue || k.moves())
        {
            st.selected_residue = Some(r);
            st.residue_scroll.ensure_visible(r, residues.len(), out.visible);
        }
        if k.enter {
            activate = st.selected_residue;
        }
    }
    if let Some(n) = activate.and_then(|r| residues.get(r)).map(|r| r.node) {
        requests.push(Request::Explore(n));
    }
}

/// Dettaglio di un possibile residuo: misure, testo onesto, stato Non sappiamo con il motivo e le sole azioni
/// ammesse: "Apri in Esplora file" e "Mostra in Esplora". Niente sezioni apribili: nel dettaglio di un residuo
/// gli unici pulsanti sono quei due.
fn residue_detail(app: &App, ui: &mut Ui, p: &Palette, programs: &Programs, requests: &mut Vec<Request>) {
    let (l, f) = (&app.lingue, &app.fmt);
    let Some(r) = app.programs.selected_residue.and_then(|i| programs.residues.get(i)) else {
        widgets::paragraph(ui, l.t("programmi-residuo-nessuna-selezione"), Type::Body, p.text_2);
        return;
    };
    let Some(d) = app.data.as_ref() else { return };
    let t = d.tree();
    if r.node as usize >= t.len() {
        return;
    }
    let path = t.path_lossy(r.node);
    ScrollArea::vertical().id_salt(("programmi", "residuo")).auto_shrink([false, false]).show(ui, |ui| {
        let name = t.name_lossy(r.node);
        let w = ui.available_width();
        let title = widgets::label_elided(ui, &name, widgets::font(Type::Title), p.text, w);
        title.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
        ui.add(egui::Label::new(egui::RichText::new(&path).font(mono(12.0)).color(p.text_2)).wrap());
        ui.add_space(10.0);
        ui.add(egui::Label::new(egui::RichText::new(f.size(t.alloc(r.node))).font(widgets::light(34.0)).color(p.text).extra_letter_spacing(-0.8)));
        ui.add_space(10.0);
        residue_facts(ui, p, l, f, t, r);
        ui.add_space(16.0);
        widgets::typed(ui, l.t("ispettore-cosa-puoi-fare"), Type::BodyStrong, p.text);
        ui.add_space(6.0);
        todo_box(ui, p, |ui| {
            inspector::state_line(ui, p, l, &r.record.state);
            if let State::Unknown(reason) = r.record.state {
                note(ui, kbtext::unknown_reason(l, reason, None), p.text_2);
            }
            note(ui, format!("{}: {}", l.t("record-azione"), kbtext::action_label(l, &r.record.action)), p.text_2);
        });
        ui.add_space(18.0);
        widgets::typed(ui, l.t("ispettore-azioni"), Type::BodyStrong, p.text);
        ui.add_space(8.0);
        match ui.push_id("residuo_azioni", |ui| action_pair(ui, p, l, true, true)).inner {
            Some(0) => requests.push(Request::Reveal(path.clone())),
            Some(_) => requests.push(Request::Explore(r.node)),
            None => {}
        }
        ui.add_space(18.0);
        widgets::typed(ui, l.t("programmi-sezione-funzione"), Type::BodyStrong, p.text);
        ui.add_space(4.0);
        widgets::paragraph(ui, l.t("funzione-possibile-residuo"), Type::Callout, p.text);
        note(ui, l.t("funzione-possibile-residuo-descrizione"), p.text_2);
        ui.add_space(12.0);
    });
}

/// Fatti di un possibile residuo: il testo onesto con la sua provenienza, la base e le misure.
fn residue_facts(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, t: &Tree, r: &Residue) {
    for fa in &r.record.identity.0 {
        inspector::fact_in(ui, p, kbtext::evidence(fa.provenance), &kbtext::fact_text(l, f, fa), l, p.text);
    }
    // La base è una cartella nota di Windows (SHGetKnownFolderPath): da fonte.
    inspector::fact_in(ui, p, Evidence::Source, &l.ta("programmi-residuo-base", &[("base", base_name(r.base).into())]), l, p.text);
    let alloc = t.alloc(r.node);
    inspector::fact_in(ui, p, Evidence::Measured, &l.ta("fatto-dimensioni-su-disco", &[("dimensione", f.size(alloc).into()), ("byte", f.count(alloc).into())]), l, p.text);
}

// ---------------------------------------------------------------------------------------------
// Dialogo di disinstallazione

/// L'interfaccia ufficiale di `via` con valori validati; `None` per Epic e per PFN o appid non validi.
fn official(via: &UninstallVia) -> Option<Launch> {
    match via {
        UninstallVia::Settings { .. } => Some(Launch::AppsFeatures),
        UninstallVia::SettingsApp { family_name } => Pfn::new(family_name).map(Launch::AppsFeaturesApp),
        UninstallVia::Steam { appid } => SteamAppId::new(*appid).map(Launch::SteamUninstall),
        UninstallVia::EpicLibrary => None,
    }
}

/// Apre l'interfaccia ufficiale con valori tipizzati. Per Win32 copia prima il nome negli appunti, come
/// "Copia percorso". Mai `UninstallString`: il lanciatore non accetta stringhe.
fn launch_official(ctx: &egui::Context, launcher: Launcher, via: &UninstallVia) -> Outcome {
    if let UninstallVia::Settings { name } = via {
        ctx.copy_text(name.clone());
    }
    match official(via) {
        None => Outcome::Invalid,
        Some(t) => match launcher(&t) {
            Ok(()) => Outcome::Opened,
            Err(e) => Outcome::Failed(e.0),
        },
    }
}

/// Il dialogo (variante A, come la conferma dello spostamento): titolo-domanda, sottotitolo, prima i tuoi
/// file con la casella della conferma esplicita, poi cosa succede e il rischio; nel piede Esc a sinistra e i
/// pulsanti a destra, con il principale nel colore dell'accento.
fn dialog(app: &mut App, ctx: &egui::Context, p: &Palette, programs: &Programs) {
    let Some(dlg) = app.programs.dialog.clone() else { return };
    let Some((x, via)) = programs.items.get(dlg.program).and_then(|x| x.uninstall.clone().map(|v| (x, v))) else {
        app.programs.dialog = None;
        return;
    };
    let (l, f) = (&app.lingue, &app.fmt);
    let name = x.name.clone();
    let your_files: Vec<&Fact> = x.record.identity.0.iter().filter(|fa| part_of(&fa.key) == Part::YourFiles).collect();
    let mut copied = dlg.copied;
    let (mut primary, mut close, mut performed) = (false, false, false);
    // Dopo l'apertura: misura di partenza, misura in corso ed esito di "Ho eseguito la procedura".
    let (busy, status) = super::procedure_flow::program_status(app, &name);
    let width = (ctx.content_rect().width() - 64.0).min(560.0);
    let resp = egui::Modal::new(Id::new(("programmi", "dialogo"))).frame(intervention_view::dialog_frame(p)).backdrop_color(p.scrim).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = 6.0;
        Frame::new().inner_margin(Margin { left: 28, right: 28, top: 24, bottom: 6 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Il titolo chiede quello che fa il pulsante principale (docs/testi.md §4): Quado apre l'interfaccia
            // ufficiale e non disinstalla. Senza niente da aprire (Epic, dopo l'apertura, dopo un errore) il titolo
            // nomina il compito.
            let title = match (&dlg.outcome, &via) {
                (None, UninstallVia::Settings { .. } | UninstallVia::SettingsApp { .. }) => "programmi-dialogo-titolo",
                (None, UninstallVia::Steam { .. }) => "programmi-dialogo-titolo-steam",
                _ => "programmi-dialogo-titolo-guida",
            };
            widgets::paragraph(ui, l.ta(title, &[("programma", name.clone().into())]), Type::Title, p.text);
            ui.add_space(2.0);
            widgets::paragraph(ui, l.t("programmi-dialogo-sottotitolo"), Type::Body, p.text_2);
        });
        Frame::new().inner_margin(Margin { left: 28, right: 28, top: 10, bottom: 16 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            match dlg.outcome {
                None => {
                    // Prima i tuoi file e la conferma esplicita, poi cosa succede.
                    for fa in &your_files {
                        widgets::banner(ui, p, &kbtext::fact_text(l, f, fa));
                    }
                    if !your_files.is_empty() {
                        widgets::checkbox(ui, p, &mut copied, &l.t("funzione-programma-controllo-tuoi-file"), true);
                        ui.add_space(4.0);
                    }
                    let what = match &via {
                        UninstallVia::Settings { name } => l.ta("programmi-dialogo-impostazioni", &[("programma", name.clone().into())]),
                        UninstallVia::SettingsApp { .. } => l.ta("programmi-dialogo-msix", &[("programma", name.clone().into())]),
                        UninstallVia::Steam { .. } => l.ta("programmi-dialogo-steam", &[("programma", name.clone().into())]),
                        UninstallVia::EpicLibrary => l.ta("programmi-dialogo-epic", &[("programma", name.clone().into())]),
                    };
                    widgets::paragraph(ui, what, Type::Body, p.text);
                    widgets::paragraph(ui, l.t("funzione-programma-rischio-1"), Type::Callout, p.warn);
                }
                Some(Outcome::Opened) => {
                    let done = match &via {
                        UninstallVia::Settings { name } => l.ta("programmi-dialogo-aperto-impostazioni", &[("programma", name.clone().into())]),
                        UninstallVia::SettingsApp { .. } => l.ta("programmi-dialogo-aperto-msix", &[("programma", name.clone().into())]),
                        _ => l.ta("programmi-dialogo-aperto-steam", &[("programma", name.clone().into())]),
                    };
                    widgets::paragraph(ui, done, Type::Body, p.text);
                    note(ui, l.t("procedura-disinstallazione-dopo"), p.text_2);
                    super::procedure_flow::lines(ui, p, &status);
                }
                Some(Outcome::Failed(code)) => {
                    widgets::paragraph(ui, l.ta("programmi-dialogo-errore", &[("codice", format!("0x{:08X}", code as u32).into())]), Type::Body, p.warn);
                }
                Some(Outcome::Invalid) => {
                    widgets::paragraph(ui, l.t("programmi-dialogo-non-valido"), Type::Body, p.warn);
                }
            }
        });
        intervention_view::dialog_foot(ui, p, &l.t("programmi-dialogo-esc"), p.text_2, |ui| {
            // Dopo l'apertura "Ho eseguito la procedura" e Chiudi; per Epic, che non ha niente da aprire, solo Chiudi.
            if dlg.outcome.is_some() || via == UninstallVia::EpicLibrary {
                close |= widgets::button(ui, p, &l.t("programmi-dialogo-chiudi"), None, ButtonKind::Secondary, true).clicked();
                if dlg.outcome == Some(Outcome::Opened) {
                    performed |= widgets::button(ui, p, &l.t("azione-ho-eseguito"), None, ButtonKind::Secondary, !busy).clicked();
                }
                return;
            }
            let label = match &via {
                UninstallVia::Settings { .. } | UninstallVia::SettingsApp { .. } => Some(l.t("programmi-dialogo-apri-impostazioni")),
                UninstallVia::Steam { .. } => Some(l.t("programmi-dialogo-apri-steam")),
                UninstallVia::EpicLibrary => None,
            };
            if let Some(label) = label {
                primary |= widgets::button(ui, p, &label, None, ButtonKind::Primary, your_files.is_empty() || copied).clicked();
            }
            close |= widgets::button(ui, p, &l.t("programmi-dialogo-annulla"), None, ButtonKind::Secondary, true).clicked();
        });
    });
    // Esc, clic fuori dal dialogo, Annulla, Chiudi: si chiude senza fare niente.
    if close || resp.should_close() {
        app.programs.dialog = None;
        return;
    }
    // "Ho eseguito la procedura": nuova misura, riscansione della cartella del programma, registro.
    if performed {
        super::procedure_flow::done(app, super::intervention_jobs::Origin::Program(dlg.program));
        return;
    }
    if primary && dlg.outcome.is_none() && (your_files.is_empty() || copied) {
        // Misura di partenza prima di aprire l'interfaccia ufficiale.
        let start = super::procedure_flow::uninstall_start(app, dlg.program);
        let outcome = launch_official(ctx, app.programs.launcher, &via);
        if outcome == Outcome::Opened
            && let Some(t) = official(&via)
        {
            super::procedure_flow::uninstall_opened(app, dlg.program, start, t.uri());
        }
        let notice = match (outcome, &via) {
            (Outcome::Opened, UninstallVia::Settings { name }) => Some(app.lingue.ta("programmi-dialogo-aperto-impostazioni", &[("programma", name.clone().into())])),
            (Outcome::Failed(code), _) => Some(app.lingue.ta("avviso-shell-errore", &[("codice", format!("0x{:08X}", code as u32).into())])),
            _ => None,
        };
        if let Some(n) = notice {
            app.notice.set(n);
        }
        app.programs.dialog = Some(UninstallDialog { outcome: Some(outcome), copied, ..dlg });
        return;
    }
    if let Some(open) = app.programs.dialog.as_mut() {
        open.copied = copied;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::app::synthetic;

    fn sample() -> Loaded {
        let (v, r) = synthetic::sample_c(false);
        let kb = Arc::new(quado_kb::Kb::embedded().unwrap_or(quado_kb::Kb { rules: Vec::new() }));
        Loaded::with_rules(v, r, Duration::ZERO, None, kb, synthetic::sample_c_evidence())
    }

    #[test]
    fn listed_total_counts_a_shared_folder_once() {
        let d = sample();
        let mut p: Programs = (*d.programs).clone();
        let all: Vec<usize> = (0..p.items.len()).collect();
        assert_eq!(listed_total(&p, &all), p.measured_total());
        // Due programmi sulla stessa cartella: nel totale una volta, anche se ne è elencato uno solo.
        let a = p.items.iter().position(|x| x.name == "Hollow Peaks").expect("Hollow Peaks");
        let b = p.items.iter().position(|x| x.name == "Quillfire").expect("Quillfire");
        p.items[b].space = p.items[a].space;
        p.items[a].shared_with = vec![b];
        p.items[b].shared_with = vec![a];
        assert_eq!(listed_total(&p, &all), p.measured_total());
        assert_eq!(listed_total(&p, &[b]), p.items[a].space.bytes());
        assert_eq!(listed_total(&p, &[a, b]), p.items[a].space.bytes());
        assert_eq!(listed_total(&p, &[]), 0);
    }

    #[test]
    fn order_is_by_measured_space_then_unmeasured() {
        let d = sample();
        let order = sort_order(&d.programs);
        let items = &d.programs.items;
        assert_eq!(order.len(), items.len());
        let rank = |s: Space| match s {
            Space::Measured(_) => 0,
            Space::NotMeasured => 1,
            Space::NotAttributed => 2,
        };
        for w in order.windows(2) {
            let (a, b) = (&items[w[0]], &items[w[1]]);
            assert!(rank(a.space) < rank(b.space) || (rank(a.space) == rank(b.space) && a.space.bytes() >= b.space.bytes()), "{} prima di {}", a.name, b.name);
        }
    }

    #[test]
    fn every_fact_key_has_one_place() {
        // Le chiavi dei fatti della vista finiscono nella sezione giusta del dettaglio.
        assert!(part_of("fatto-programma-tuoi-file") == Part::YourFiles);
        assert!(part_of("fatto-programma-cartella-indizio") == Part::Location);
        assert!(part_of("fatto-programma-cartella-condivisa") == Part::Location);
        assert!(part_of("fatto-dimensione-dichiarata") == Part::Space);
        assert!(part_of("fatto-uso-tracciamento-disattivato") == Part::Usage);
        assert!(part_of("fatto-ultima-sessione-steam") == Part::Usage);
        assert!(part_of("fatto-programma-installato") == Part::Identity);
        assert!(part_of("fatto-gioco-steam") == Part::Identity);
    }
}
