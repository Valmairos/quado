// SPDX-License-Identifier: GPL-3.0-or-later
//! Palette dei comandi (Ctrl+K, variante A, `.cmdk`): una casella e un elenco per saltare a una schermata,
//! a una cartella o a un file del modello già scansionato (con la ricerca per nome di Esplora) e
//! per i comandi più usati.
//!
//! Tastiera: la casella ha il focus; frecce su e giù scelgono la voce, Invio la apre, Esc chiude senza fare
//! niente, Tab resta nella palette. Ogni voce dichiara a AccessKit il nome (testo e seconda riga) e se è
//! scelta. La palette è modale: sotto, niente risponde finché è aperta.
//!
//! Nessun comando cambia qualcosa sul disco: «Sposta nel Cestino…» apre la stessa anteprima dell'ispettore,
//! «Ripeti come amministratore» porta alla riga dei conti della Panoramica, dove c'è il pulsante.

use std::time::{Duration, Instant};

use egui::{Align, Align2, Color32, CornerRadius, Frame, Id, Key, Layout, Margin, Modifiers, Rect, ScrollArea, Sense, Stroke, Ui, pos2, vec2};
use quado_model::{Filter, Tree};

use super::data::{self, SearchOutcome, Task};
use super::{App, Screen, explore, helper_jobs, intervention_jobs, kbtext};
use crate::icons::{self, Icon};
use crate::tokens::{self, Elevation, Gestore, Palette, Type};
use crate::widgets;

/// Cartelle e file mostrati al più.
const MAX_ITEMS: usize = 8;

#[derive(Default)]
pub struct PaletteState {
    pub open: bool,
    pub query: String,
    /// Voce scelta, nell'ordine dell'elenco mostrato.
    pub selected: usize,
    /// La casella prende il focus al prossimo frame.
    focus: bool,
    /// La scelta è cambiata da tastiera: la voce si porta in vista.
    reveal: bool,
    task: Option<Task<SearchOutcome>>,
    /// Risultati della ricerca per il testo `hits_for`.
    hits: Vec<u32>,
    hits_for: String,
    changed_at: Option<Instant>,
}

impl PaletteState {
    /// Apre la palette vuota, con il focus nella casella.
    pub fn show_palette(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.focus = true;
        self.hits.clear();
        self.hits_for.clear();
        self.changed_at = None;
    }

    /// Scrive il testo e cerca al frame successivo (test, immagini).
    pub fn set_query(&mut self, text: &str) {
        self.query = text.to_owned();
        self.selected = 0;
        self.changed_at = Some(Instant::now() - Duration::from_secs(1));
    }

    pub fn close(&mut self) {
        self.open = false;
        self.task = None;
    }

    /// La ricerca dei nomi è in corso o sta per partire.
    pub fn pending(&self) -> bool {
        self.task.is_some() || self.changed_at.is_some()
    }

    /// Aspetta la ricerca in corso (prima di modificare il modello).
    pub fn wait(&mut self) {
        if let Some(t) = self.task.take()
            && let Some(o) = t.wait()
        {
            self.hits_for = o.query.0;
            self.hits = o.hits;
        }
    }
}

/// Gruppi dell'elenco, in quest'ordine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    Views,
    Items,
    Commands,
}

impl Group {
    fn key(self) -> &'static str {
        match self {
            Self::Views => "palette-gruppo-viste",
            Self::Items => "palette-gruppo-elementi",
            Self::Commands => "palette-gruppo-comandi",
        }
    }
}

/// Cosa fa una voce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Screen(Screen),
    Node(u32),
    Rescan,
    RetryAdmin,
    View { list_only: bool },
    CopyPath(u32),
    Sidebar,
    Recycle(usize),
}

/// Una voce: segno (icona o forma del gestore), testo, seconda riga, scorciatoia.
struct Entry {
    group: Group,
    icon: Icon,
    glyph: Option<Gestore>,
    label: String,
    sub: Option<String>,
    kbd: Option<&'static str>,
    target: Target,
}

/// Schermate della palette: icona, chiave del nome, scorciatoia; le prime chiedono una scansione.
const VIEWS: [(Screen, Icon, &str, Option<&str>); 10] = [
    (Screen::Overview, Icon::Overview, "nav-panoramica", Some("Ctrl+1")),
    (Screen::Explore, Icon::Explore, "nav-esplora", Some("Ctrl+2")),
    (Screen::BigFiles, Icon::BigFiles, "nav-file-grandi", Some("Ctrl+3")),
    (Screen::Downloads, Icon::Download, "nav-download", Some("Ctrl+5")),
    (Screen::Programs, Icon::Programs, "nav-programmi", Some("Ctrl+6")),
    (Screen::System, Icon::System, "nav-sistema", Some("Ctrl+4")),
    (Screen::Interventions, Icon::Actions, "nav-interventi", Some("Ctrl+7")),
    (Screen::Rules, Icon::Rules, "nav-regole", None),
    (Screen::Volumes, Icon::Drive, "nav-volumi", None),
    (Screen::Settings, Icon::Settings, "nav-impostazioni", Some("Ctrl+,")),
];

/// Le voci per il testo scritto: schermate e comandi che lo contengono, poi cartelle e file trovati.
fn entries(app: &App) -> Vec<Entry> {
    let l = &app.lingue;
    let q = app.palette.query.trim().to_lowercase();
    let matches = |e: &Entry| q.is_empty() || e.label.to_lowercase().contains(&q) || e.sub.as_ref().is_some_and(|s| s.to_lowercase().contains(&q));
    let has = app.data.is_some();
    let mut out: Vec<Entry> = Vec::new();
    for (screen, icon, key, kbd) in VIEWS {
        if !has && !matches!(screen, Screen::Rules | Screen::Volumes | Screen::Settings) {
            continue;
        }
        out.push(Entry { group: Group::Views, icon, glyph: None, label: l.t(key), sub: None, kbd, target: Target::Screen(screen) });
    }
    // Cartelle e file: solo con un testo, dalla ricerca per nome (i più grandi prima).
    if let Some(d) = &app.data
        && !q.is_empty()
    {
        let t = d.tree();
        for &n in app.palette.hits.iter().take(MAX_ITEMS) {
            if n as usize >= t.len() {
                continue;
            }
            let parent = t.parent(n);
            let where_ = if parent == quado_model::NONE { String::new() } else { t.path_lossy(parent) };
            let sub = if where_.is_empty() { app.fmt.size(t.alloc(n)) } else { format!("{where_} · {}", app.fmt.size(t.alloc(n))) };
            let icon = if t.is_dir(n) { Icon::Folder } else { Icon::File };
            out.push(Entry { group: Group::Items, icon, glyph: Some(kbtext::node_gestore(&d.cls, n)), label: t.name_lossy(n), sub: Some(sub), kbd: None, target: Target::Node(n) });
        }
    }
    let mut cmd = |icon, label: String, sub: Option<String>, kbd, target| out.push(Entry { group: Group::Commands, icon, glyph: None, label, sub, kbd, target });
    if let Some(d) = &app.data {
        if app.job.is_none() {
            cmd(Icon::Refresh, l.t("barra-riesamina"), None, Some("F5"), Target::Rescan);
        }
        if app.helper.phase().is_none() && helper_jobs::build_request(d).is_some() {
            cmd(Icon::Shield, l.t("helper-ripeti"), Some(l.t("conti-titolo")), None, Target::RetryAdmin);
        }
        cmd(Icon::Explore, l.t("esplora-mappa-ed-elenco"), Some(l.t("nav-esplora")), None, Target::View { list_only: false });
        cmd(Icon::List, l.t("esplora-solo-elenco"), Some(l.t("nav-esplora")), None, Target::View { list_only: true });
        if let Some(n) = app.explore.selected.filter(|&n| (n as usize) < d.tree().len()) {
            cmd(Icon::Copy, l.t("azione-copia-percorso"), Some(d.tree().path_lossy(n)), None, Target::CopyPath(n));
        }
        // Spostamenti nel Cestino di un solo elemento riconosciuto: la stessa anteprima dell'ispettore.
        for i in quado_kb::engine::interventions(&d.kb, &d.cls, d.tree()).iter().filter(|i| i.matches.len() == 1).take(6) {
            let m = &d.cls.matches[i.matches[0]];
            // Da verificare: niente spostamento diretto, come nella card (prima ipotesi e rischi nell'ispettore).
            if m.record.action == quado_model::record::Action::Recycle && !matches!(m.record.state, quado_model::record::State::ToVerify(_)) {
                cmd(Icon::Trash, l.t("azione-prepara-cestino"), Some(kbtext::rule_line(l, &app.fmt, &d.kb, m)), None, Target::Recycle(i.matches[0]));
            }
        }
    }
    if app.data.is_some() {
        let key = if app.settings.sidebar_collapsed { "barra-espandi" } else { "barra-comprimi" };
        cmd(Icon::Sidebar, l.t(key), None, None, Target::Sidebar);
    }
    out.retain(|e| e.group == Group::Items || matches(e));
    // Senza testo: schermate e comandi; con il testo, i gruppi nell'ordine della proposta.
    out.sort_by_key(|e| match e.group {
        Group::Views => 0,
        Group::Items => 1,
        Group::Commands => 2,
    });
    out
}

/// Avvia la ricerca dei nomi dopo una breve pausa nella digitazione.
fn search(app: &mut App, ctx: &egui::Context) {
    let st = &mut app.palette;
    if let Some(t) = st.task.as_mut() {
        ctx.request_repaint_after(Duration::from_millis(30));
        if let Some(o) = t.poll() {
            st.task = None;
            st.hits_for = o.query.0;
            st.hits = o.hits;
        }
    }
    let Some(at) = st.changed_at else { return };
    if at.elapsed() < Duration::from_millis(120) {
        ctx.request_repaint_after(Duration::from_millis(40));
        return;
    }
    if st.task.is_some() {
        return;
    }
    st.changed_at = None;
    let q = st.query.trim().to_owned();
    if q.is_empty() {
        st.hits.clear();
        st.hits_for.clear();
        return;
    }
    if q == st.hits_for {
        return;
    }
    if let Some(d) = &app.data {
        st.task = Some(data::spawn_search(&d.model, &d.cls, q, Filter::default(), Tree::ROOT, d.generation));
    }
}

/// La palette, sopra ogni schermata, quando è aperta.
pub fn show(app: &mut App, ctx: &egui::Context) {
    if !app.palette.open {
        return;
    }
    search(app, ctx);
    let p = app.palette(ctx);
    // Le frecce, Invio e Tab sono della palette, prima della casella: la casella su una riga li userebbe
    // per spostare il cursore o per perdere il focus.
    let (down, up, enter, _tab) = ctx.input_mut(|i| {
        (
            i.count_and_consume_key(Modifiers::NONE, Key::ArrowDown),
            i.count_and_consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Tab) || i.consume_key(Modifiers::SHIFT, Key::Tab),
        )
    });
    let list = entries(app);
    let n = list.len();
    let st = &mut app.palette;
    if n == 0 {
        st.selected = 0;
    } else {
        let before = st.selected.min(n - 1);
        st.selected = (before + down).saturating_sub(up).min(n - 1);
        if down + up > 0 {
            st.reveal = true;
        }
    }
    let mut run: Option<Target> = None;
    if enter && n > 0 {
        run = Some(list[st.selected].target);
    }
    let screen = ctx.content_rect();
    let width = (screen.width() - 48.0).min(600.0);
    let max_list = ((screen.height() - 120.0).min(520.0) - 52.0 - 40.0).max(120.0);
    let area = egui::Modal::default_area(Id::new("palette_comandi")).anchor(Align2::CENTER_TOP, vec2(0.0, 76.0));
    let frame = Frame::new().fill(p.raised).corner_radius(CornerRadius::same(14)).shadow(p.shadow(Elevation::Popover)).stroke(Stroke::new(1.0, p.border));
    let l = &app.lingue;
    let pending = app.palette.pending();
    let resp = egui::Modal::new(Id::new("palette_comandi")).area(area).frame(frame).backdrop_color(p.scrim).show(ctx, |ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let st = &mut app.palette;
        // Casella (`.cmdk .q`): lente, testo, «Esc».
        Frame::new().inner_margin(Margin { left: 16, right: 14, top: 4, bottom: 4 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_min_height(44.0);
                ui.spacing_mut().item_spacing.x = 10.0;
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                icons::paint(ui.painter(), r, Icon::Search, p.text_2);
                let kw = 30.0;
                let hint = egui::RichText::new(l.t("palette-segnaposto")).color(p.text_3).font(widgets::sans(15.0));
                let te = egui::TextEdit::singleline(&mut st.query).id(Id::new("palette_casella")).frame(Frame::NONE).font(widgets::sans(15.0)).hint_text(hint).desired_width(ui.available_width() - kw - 10.0);
                let r = ui.add(te);
                let name = l.t("barra-palette");
                ui.ctx().accesskit_node_builder(r.id, |node| node.set_label(name));
                if st.focus {
                    r.request_focus();
                    st.focus = false;
                }
                // Il focus resta nella casella: un clic sull'elenco non lo porta via.
                if !r.has_focus() && !r.gained_focus() {
                    r.request_focus();
                }
                if r.changed() {
                    st.selected = 0;
                    st.changed_at = Some(Instant::now());
                }
                widgets::kbd(ui, &p, &l.t("tasto-esc"));
            });
        });
        widgets::divider(ui, &p);
        // Risultati (`.cmdk .res`), raggruppati.
        let selected = st.selected;
        let reveal = std::mem::take(&mut st.reveal);
        ScrollArea::vertical().id_salt("palette_elenco").max_height(max_list).auto_shrink([false, true]).show(ui, |ui| {
            Frame::new().inner_margin(Margin::same(6)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let mut last = None;
                for (k, e) in list.iter().enumerate() {
                    if last != Some(e.group) {
                        last = Some(e.group);
                        ui.add_space(if k == 0 { 4.0 } else { 8.0 });
                        ui.horizontal(|ui| {
                            ui.add_space(10.0);
                            widgets::typed(ui, l.t(e.group.key()), Type::Caption, p.text_2);
                        });
                        ui.add_space(4.0);
                    }
                    let r = ui.push_id(("voce", k), |ui| entry_row(ui, &p, e, k == selected)).inner;
                    if k == selected && reveal {
                        r.scroll_to_me(None);
                    }
                    if r.clicked() {
                        run = Some(e.target);
                    }
                }
                if list.iter().all(|e| e.group != Group::Items) && !app.palette.query.trim().is_empty() && app.data.is_some() && pending {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        widgets::typed(ui, l.t("ricerca-in-corso"), Type::Callout, p.text_2);
                    });
                }
                if list.is_empty() && !pending {
                    Frame::new().inner_margin(Margin::symmetric(12, 14)).show(ui, |ui| {
                        widgets::paragraph(ui, l.t("palette-nessuno"), Type::Body, p.text_2);
                    });
                }
            });
        });
        widgets::divider(ui, &p);
        // Piede (`.cmdk .ft`): i tasti e cosa fanno.
        Frame::new().inner_margin(Margin::symmetric(16, 8)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                kbd_arrow(ui, &p, true);
                kbd_arrow(ui, &p, false);
                widgets::typed(ui, l.t("palette-sposta"), Type::Caption, p.text_2);
                ui.add_space(12.0);
                for (key, what) in [("tasto-invio", "palette-apri"), ("tasto-esc", "palette-chiudi")] {
                    widgets::kbd(ui, &p, &l.t(key));
                    widgets::typed(ui, l.t(what), Type::Caption, p.text_2);
                    ui.add_space(12.0);
                }
            });
        });
    });
    if resp.should_close() {
        app.palette.close();
        return;
    }
    if let Some(t) = run {
        app.palette.close();
        perform(app, ctx, t);
    }
}

/// Tasto freccia su o giù nel riquadro dei tasti (come `widgets::kbd`), con la freccia disegnata.
fn kbd_arrow(ui: &mut Ui, p: &Palette, up: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(4), p.panel2, Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
    let c = rect.center();
    let s = if up { -1.0 } else { 1.0 };
    let stroke = Stroke::new(1.3, p.text_2);
    ui.painter().line_segment([pos2(c.x, c.y - 4.0 * s), pos2(c.x, c.y + 4.0 * s)], stroke);
    ui.painter().add(egui::Shape::line(vec![pos2(c.x - 3.0, c.y + 1.0 * s), pos2(c.x, c.y + 4.0 * s), pos2(c.x + 3.0, c.y + 1.0 * s)], stroke));
}

/// Una voce (`.cmdk .it`): segno, testo e seconda riga, scorciatoia; fondo `select` se è la scelta.
fn entry_row(ui: &mut Ui, p: &Palette, e: &Entry, selected: bool) -> egui::Response {
    let w = ui.available_width();
    let kw = e.kbd.map_or(0.0, |k| ui.painter().layout_no_wrap(k.to_owned(), widgets::medium(11.0), p.text_2).size().x + 10.0 + 10.0);
    let tw = w - 10.0 - 20.0 - 10.0 - kw - 10.0;
    let lg = widgets::elided(ui.painter(), e.label.clone(), widgets::font(Type::Body), p.text, tw);
    let sg = e.sub.as_ref().map(|s| widgets::elided(ui.painter(), s.clone(), widgets::font(Type::Callout), p.text_2, tw));
    let h = (lg.size().y + sg.as_ref().map_or(0.0, |g| g.size().y) + 12.0).max(36.0);
    // Le voci non prendono il focus: la scelta si muove con le frecce dalla casella (Tab resta lì).
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::CLICK);
    let fill = if selected {
        p.select
    } else if resp.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(tokens::radius::ROW), fill);
    let cx = rect.min.x + 10.0 + 10.0;
    match e.glyph {
        Some(g) => widgets::manager_glyph(ui.painter(), pos2(cx, rect.center().y), g, p),
        None => icons::paint(ui.painter(), Rect::from_center_size(pos2(cx, rect.center().y), vec2(16.0, 16.0)), e.icon, p.text_2),
    }
    let block = lg.size().y + sg.as_ref().map_or(0.0, |g| g.size().y);
    let mut y = rect.center().y - block / 2.0;
    let x = rect.min.x + 10.0 + 20.0 + 10.0;
    let lh = lg.size().y;
    ui.painter().galley(pos2(x, y), lg, p.text);
    y += lh;
    if let Some(g) = sg {
        ui.painter().galley(pos2(x, y), g, p.text_2);
    }
    if let Some(k) = e.kbd {
        let mut kui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(rect.max.x - kw, rect.min.y), pos2(rect.max.x - 10.0, rect.max.y))).layout(Layout::right_to_left(Align::Center)));
        widgets::kbd(&mut kui, p, k);
    }
    let name = match &e.sub {
        Some(s) => format!("{}, {s}", e.label),
        None => e.label.clone(),
    };
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, &name));
    resp
}

/// Esegue la voce scelta.
fn perform(app: &mut App, ctx: &egui::Context, t: Target) {
    match t {
        Target::Screen(s) => {
            if s == Screen::Rules {
                app.rules_view.selected = None;
            }
            if s == Screen::Volumes {
                app.screen = Screen::Volumes;
            } else {
                app.go(s);
            }
        }
        Target::Node(n) => explore::show_in_folder(app, n),
        Target::Rescan => {
            if let Some(v) = app.data.as_ref().map(|d| d.volume.clone())
                && app.job.is_none()
            {
                app.start_scan(v);
            }
        }
        Target::RetryAdmin => {
            app.overview.why_open = true;
            app.go(Screen::Overview);
        }
        Target::View { list_only } => {
            app.explore.list_only = list_only;
            app.go(Screen::Explore);
        }
        Target::CopyPath(n) => {
            if let Some(d) = &app.data {
                ctx.copy_text(d.tree().path_lossy(n));
                app.notice.set(app.lingue.t("avviso-percorso-copiato"));
            }
        }
        Target::Sidebar => {
            app.settings.sidebar_collapsed = !app.settings.sidebar_collapsed;
            app.sidebar_peek = false;
            app.save_settings();
        }
        Target::Recycle(m) => intervention_jobs::dispatch(app, intervention_jobs::ActRequest::PrepareRecycle, intervention_jobs::Origin::Rule(m)),
    }
}
