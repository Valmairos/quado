// SPDX-License-Identifier: GPL-3.0-or-later
//! Esplora (F-EX-1..4): barra del percorso con dimensione e quota dello spazio usato, avanti e
//! indietro, mappa ad albero su due livelli, tabella ad albero virtualizzata e sincronizzata con la
//! mappa, ricerca per nome con i filtri, tastiera completa.

use std::time::{Duration, Instant};

use egui::{
    Align, Align2, CornerRadius, Id, Key, Layout, Rect, Response, Sense, Stroke, StrokeKind, TextureHandle,
    TextureOptions, Ui, pos2, vec2,
};
use quado_model::view::{Filter, Outline};
use quado_model::{Family, Flags, Tree};

use super::data::{self, Loaded, SearchOutcome, Task};
use super::table::{self, Column, NavKeys};
use super::{App, Screen, inspector};
use crate::format::{Formatter, Units};
use crate::i18n::Lingue;
use crate::icons::{self, Icon};
use crate::index_scroll::{self, BarColors, IndexScroll};
use crate::tokens::{self, Gestore, Glyph, Palette, Type};
use crate::treemap;
use crate::widgets::{self, mono, sans, semibold};

const INDENT: f32 = 16.0;
const HATCH_PERIOD: usize = 8;

/// Geometria dell'ultimo frame, per i banchi di misura.
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub body: Rect,
    pub bar: Rect,
    pub visible: f64,
    pub map: Rect,
}

impl Default for Geometry {
    fn default() -> Self {
        Self { body: Rect::NOTHING, bar: Rect::NOTHING, visible: 0.0, map: Rect::NOTHING }
    }
}

#[derive(Default)]
pub struct MapCache {
    pub layout: treemap::Layout,
    key: Option<(Rect, u32, u64)>,
    painted: Option<treemap::Painted>,
    /// Densità di pixel delle mesh in cache: gli angoli arrotondati si tassellano per quella.
    ppp: f32,
    hatch: Option<(TextureHandle, [egui::Color32; 2])>,
    pub hovered: Option<usize>,
}

#[derive(Default)]
pub struct ExploreState {
    pub outline: Outline,
    pub scroll: IndexScroll,
    pub selected: Option<u32>,
    pub hovered_row: Option<usize>,
    pub back: Vec<u32>,
    pub forward: Vec<u32>,
    pub list_only: bool,
    pub map: MapCache,
    pub focus_pending: bool,
    pub geom: Geometry,
    generation: u64,
}

impl ExploreState {
    pub fn reset(&mut self, d: &Loaded) {
        *self = Self { outline: Outline::new(d.ord(), d.tree(), Tree::ROOT), generation: d.generation, ..Default::default() };
    }

    pub fn invalidate_map(&mut self) {
        self.map.key = None;
        self.map.painted = None;
    }

    /// Dopo una verifica: righe ricostruite, selezione sul nodo aggiornato.
    pub fn after_model_change(&mut self, d: &Loaded, node: u32) {
        let root = if d.tree().flags(self.outline.root).has(Flags::DETACHED) { Tree::ROOT } else { self.outline.root };
        self.outline.set_root(d.ord(), d.tree(), root);
        self.back.retain(|&n| !d.tree().flags(n).has(Flags::DETACHED));
        self.forward.retain(|&n| !d.tree().flags(n).has(Flags::DETACHED));
        self.generation = d.generation;
        self.invalidate_map();
        self.selected = Some(node);
        self.outline.reveal(d.ord(), d.tree(), node);
    }

    /// Mostra il contenuto di `folder` (Invio, doppio clic, barra del percorso).
    pub fn open(&mut self, d: &Loaded, folder: u32, select: Option<u32>) {
        if folder != self.outline.root {
            self.back.push(self.outline.root);
            self.forward.clear();
        }
        self.set_root(d, folder, select);
    }

    fn set_root(&mut self, d: &Loaded, folder: u32, select: Option<u32>) {
        self.outline.set_root(d.ord(), d.tree(), folder);
        self.scroll = IndexScroll::default();
        self.selected = select.or_else(|| (!self.outline.is_empty()).then(|| self.outline.row(0)));
        if let Some(s) = self.selected
            && let Some(r) = self.outline.reveal(d.ord(), d.tree(), s)
        {
            self.scroll.ensure_visible(r, self.outline.len(), self.geom.visible.max(10.0));
        }
        self.invalidate_map();
    }

    /// Posiziona l'Esplora su un elemento: apre la sua cartella e lo seleziona (dalla Panoramica, dalla ricerca).
    pub fn reveal(&mut self, d: &Loaded, node: u32) {
        let parent = d.tree().parent(node);
        let folder = if node == Tree::ROOT || parent == quado_model::NONE { Tree::ROOT } else { parent };
        self.open(d, folder, Some(node));
        self.focus_pending = true;
    }

    pub fn up(&mut self, d: &Loaded) {
        let root = self.outline.root;
        if root != Tree::ROOT {
            self.open(d, d.tree().parent(root), Some(root));
        }
    }

    pub fn go_back(&mut self, d: &Loaded) {
        if let Some(f) = self.back.pop() {
            self.forward.push(self.outline.root);
            let prev = self.outline.root;
            self.set_root(d, f, Some(prev).filter(|&n| d.tree().parent(n) == f));
        }
    }

    pub fn go_forward(&mut self, d: &Loaded) {
        if let Some(f) = self.forward.pop() {
            self.back.push(self.outline.root);
            self.set_root(d, f, None);
        }
    }
}

/// Sistema di date proposte nei filtri (F-EX-4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DatePreset {
    #[default]
    Any,
    Last30Days,
    LastYear,
    OlderThanYear,
    OlderThan3Years,
}

impl DatePreset {
    pub const ALL: [Self; 5] = [Self::Any, Self::Last30Days, Self::LastYear, Self::OlderThanYear, Self::OlderThan3Years];

    fn key(self) -> &'static str {
        match self {
            Self::Any => "filtro-data-qualsiasi",
            Self::Last30Days => "filtro-data-30-giorni",
            Self::LastYear => "filtro-data-ultimo-anno",
            Self::OlderThanYear => "filtro-data-oltre-un-anno",
            Self::OlderThan3Years => "filtro-data-oltre-tre-anni",
        }
    }
}

/// Unità della dimensione minima scritta dall'utente.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MinUnit {
    Bytes,
    Kb,
    #[default]
    Mb,
    Gb,
}

impl MinUnit {
    pub const ALL: [Self; 4] = [Self::Bytes, Self::Kb, Self::Mb, Self::Gb];

    fn power(self) -> i32 {
        match self {
            Self::Bytes => 0,
            Self::Kb => 1,
            Self::Mb => 2,
            Self::Gb => 3,
        }
    }

    fn label(self, f: &Formatter) -> String {
        match self {
            Self::Bytes => f.labels.byte_many.clone(),
            other => {
                let set = match f.units {
                    Units::Explorer => &f.labels.explorer,
                    Units::Si => &f.labels.si,
                };
                set[other.power() as usize - 1].clone()
            }
        }
    }
}

/// Filtri come li scrive l'utente.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FilterUi {
    pub min_text: String,
    pub min_unit: MinUnit,
    pub family: Option<Family>,
    pub date: DatePreset,
    /// Gestito da (M3): il gruppo di colore, "non classificato" compreso.
    pub group: Option<Gestore>,
}

impl FilterUi {
    /// Dimensione minima in byte, se il testo è un numero (virgola o punto come decimale).
    pub fn min_bytes(&self, units: Units) -> Option<u64> {
        let t = self.min_text.trim().replace(',', ".");
        if t.is_empty() {
            return None;
        }
        let v: f64 = t.parse().ok().filter(|v: &f64| v.is_finite() && *v >= 0.0)?;
        let base: f64 = match units {
            Units::Explorer => 1024.0,
            Units::Si => 1000.0,
        };
        Some((v * base.powi(self.min_unit.power())).round() as u64)
    }

    pub fn to_filter(&self, units: Units, now: i64) -> Filter {
        const DAY: i64 = 864_000_000_000;
        let (after, before) = match self.date {
            DatePreset::Any => (None, None),
            DatePreset::Last30Days => (Some(now - 30 * DAY), None),
            DatePreset::LastYear => (Some(now - 365 * DAY), None),
            DatePreset::OlderThanYear => (None, Some(now - 365 * DAY)),
            DatePreset::OlderThan3Years => (None, Some(now - 3 * 365 * DAY)),
        };
        Filter { min_alloc: self.min_bytes(units), family: self.family, modified_after: after, modified_before: before, group: self.group.map(|g| g as u8) }
    }

    pub fn is_empty(&self) -> bool {
        self.min_text.trim().is_empty() && self.family.is_none() && self.date == DatePreset::Any && self.group.is_none()
    }
}

/// Ricerca per nome (F-EX-4): casella nella barra del titolo, risultati in Esplora.
#[derive(Default)]
pub struct SearchState {
    pub text: String,
    pub filters: FilterUi,
    pub filter: Filter,
    task: Option<Task<SearchOutcome>>,
    pub outcome: Option<SearchOutcome>,
    pub scroll: IndexScroll,
    pub selected: Option<usize>,
    pub focus_box: bool,
    /// La casella aveva il focus nell'ultimo frame (la barra laterale compressa resta aperta).
    pub box_focused: bool,
    pub focus_results: bool,
    changed_at: Option<Instant>,
    pub geom: Geometry,
}

impl SearchState {
    /// Ricerca per nome in tutta l'unità.
    pub fn active(&self) -> bool {
        !self.text.trim().is_empty()
    }

    /// Filtri di Esplora senza un nome da cercare: l'elenco mostra gli elementi della cartella aperta che
    /// li soddisfano, a qualunque profondità.
    pub fn filtering(&self) -> bool {
        !self.active() && !self.filters.is_empty()
    }

    /// Esplora mostra l'elenco dei risultati invece di mappa e albero.
    pub fn showing(&self) -> bool {
        self.active() || self.filtering()
    }

    /// I filtri sono cambiati: si converte e si rifà la ricerca dopo la solita pausa.
    pub fn filters_changed(&mut self, units: Units) {
        self.filter = self.filters.to_filter(units, quado_scan_win::system::now_filetime());
        self.changed_at = Some(Instant::now());
    }

    pub fn pending(&self) -> bool {
        self.task.is_some() || self.changed_at.is_some()
    }

    pub fn wait(&mut self) {
        if let Some(t) = self.task.take() {
            self.outcome = t.wait();
        }
    }

    pub fn invalidate(&mut self) {
        self.outcome = None;
        self.changed_at = Some(Instant::now() - Duration::from_secs(1));
    }

    /// Imposta il testo e avvia la ricerca al frame successivo (test, immagini).
    pub fn set_query(&mut self, text: &str) {
        self.text = text.to_owned();
        self.changed_at = Some(Instant::now() - Duration::from_secs(1));
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.outcome = None;
        self.changed_at = None;
        self.selected = None;
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        if let Some(t) = self.task.as_mut() {
            ctx.request_repaint_after(Duration::from_millis(30));
            if let Some(o) = t.poll() {
                self.task = None;
                self.outcome = Some(o);
                self.scroll = IndexScroll::default();
                self.selected = None;
            }
        }
    }

    /// Avvia la ricerca dopo una breve pausa nella digitazione. Con i soli filtri la ricerca vale nella
    /// cartella aperta (`scope`); se la cartella cambia, si rifà.
    fn maybe_start(&mut self, ctx: &egui::Context, d: &Loaded, scope: u32) {
        let root = if self.active() { Tree::ROOT } else { scope };
        if self.filtering() && self.changed_at.is_none() && self.task.is_none() && self.outcome.as_ref().is_none_or(|o| !o.query.0.is_empty() || o.query.2 != root) {
            self.changed_at = Some(Instant::now() - Duration::from_secs(1));
        }
        let Some(at) = self.changed_at else { return };
        if at.elapsed() < Duration::from_millis(120) {
            ctx.request_repaint_after(Duration::from_millis(40));
            return;
        }
        if self.task.is_some() {
            return;
        }
        self.changed_at = None;
        if !self.showing() {
            self.outcome = None;
            return;
        }
        let q = (self.text.trim().to_owned(), self.filter.clone(), root);
        if self.outcome.as_ref().is_some_and(|o| o.query == q && o.generation == d.generation) {
            return;
        }
        self.task = Some(data::spawn_search(&d.model, &d.cls, q.0, q.1, q.2, d.generation));
    }
}

/// Casella di ricerca in cima alla barra laterale (`.sb-search`): fondo `field`, lente e scorciatoia.
pub fn search_box(app: &mut App, ui: &mut Ui, p: &Palette) {
    let enabled = app.data.is_some();
    let id = Id::new("casella-ricerca");
    let hint = app.lingue.t("ricerca-suggerimento");
    let w = ui.available_width();
    let kbd = app.lingue.t("ricerca-scorciatoia");
    let kg = ui.painter().layout_no_wrap(kbd, widgets::medium(11.0), p.text_3);
    // Il segnaposto non si tronca: se con la scorciatoia non entra, la scorciatoia sparisce; se non entra
    // nemmeno così (lingue più lunghe), va a capo e la casella si allunga.
    let hint_w = ui.painter().layout_no_wrap(hint.clone(), sans(13.0), p.text_3).size().x;
    let kw = if hint_w + 34.0 + 16.0 + kg.size().x + 6.0 <= w { kg.size().x } else { 0.0 };
    let text_w = w - 34.0 - 10.0 - if kw > 0.0 { kw + 6.0 } else { 0.0 };
    let empty = app.search.text.is_empty();
    let wrapped = (empty && hint_w > text_w).then(|| ui.painter().layout(hint.clone(), sans(13.0), p.text_3, text_w));
    let h = wrapped.as_ref().map_or(tokens::focus::MIN_TARGET, |g| (g.size().y + 14.0).max(tokens::focus::MIN_TARGET));
    let frame_rect = ui.allocate_exact_size(vec2(w, h), Sense::hover()).0;
    ui.painter().rect_filled(frame_rect, CornerRadius::same(tokens::radius::CONTROL), p.field);
    icons::paint(ui.painter(), Rect::from_min_size(pos2(frame_rect.min.x + 10.0, frame_rect.min.y + 8.0), vec2(16.0, 16.0)), Icon::Search, p.text_3);
    if empty && kw > 0.0 {
        ui.painter().galley(pos2(frame_rect.max.x - 10.0 - kw, frame_rect.center().y - kg.size().y / 2.0), kg, p.text_3);
    }
    if let Some(g) = wrapped {
        ui.painter().galley(pos2(frame_rect.min.x + 34.0, frame_rect.min.y + 7.0), g, p.text_3);
    }
    let edit_rect = Rect::from_min_size(pos2(frame_rect.min.x + 34.0, frame_rect.min.y + 2.0), vec2(text_w, tokens::focus::MIN_TARGET - 4.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect).layout(Layout::left_to_right(Align::Center)));
    let mut edit = egui::TextEdit::singleline(&mut app.search.text).id(id).frame(egui::Frame::NONE).font(sans(13.0)).desired_width(edit_rect.width());
    if !(empty && hint_w > text_w) {
        edit = edit.hint_text(egui::RichText::new(hint).color(p.text_3));
    }
    let resp = child.add_enabled(enabled, edit);
    app.search.box_focused = resp.has_focus();
    if resp.has_focus() {
        ui.painter().rect_stroke(frame_rect.expand(tokens::focus::OFFSET), CornerRadius::same(tokens::radius::CONTROL + 2), Stroke::new(tokens::focus::WIDTH, p.ring), StrokeKind::Outside);
    }
    if app.search.focus_box {
        resp.request_focus();
        app.search.focus_box = false;
        // Il focus vale dal frame dopo: la barra laterale aperta per la ricerca non si richiude intanto.
        app.search.box_focused = true;
    }
    if resp.changed() {
        app.search.changed_at = Some(Instant::now());
        if app.search.active() && app.screen != Screen::Explore {
            app.go(Screen::Explore);
        }
    }
    if resp.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
        app.search.clear();
    }
    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) && app.search.active() {
        app.search.focus_results = true;
        app.search.changed_at.get_or_insert(Instant::now() - Duration::from_secs(1));
    }
}

/// Albero e figli ordinati come sorgente della mappa.
struct Src<'a>(&'a Loaded);

impl treemap::Source for Src<'_> {
    fn size(&self, node: u32) -> u64 {
        self.0.tree().alloc(node)
    }
    fn children(&self, node: u32, out: &mut Vec<u32>) {
        out.extend_from_slice(self.0.ord().children(node));
    }
}

/// Non misurato: cartella non leggibile o cartella cloud non elencata (tratteggio, 04 §6.1).
pub fn unmeasured(t: &Tree, n: u32) -> bool {
    t.flags(n).any(Flags::DENIED | Flags::ERROR | Flags::CLOUD_NOT_LISTED | Flags::PARTIAL)
}

/// Larghezza dell'ispettore di Esplora (272 + 72 nella proposta, a 100%: la dimensione del testo è lo zoom).
pub const INSPECTOR_W: f32 = 344.0;

pub fn show(app: &mut App, ui: &mut Ui, p: &Palette) {
    let Some(d) = app.data.as_ref() else { return };
    if app.explore.generation != d.generation {
        let node = app.explore.selected.unwrap_or(Tree::ROOT);
        app.explore.after_model_change(d, node);
    }
    app.search.maybe_start(ui.ctx(), d, app.explore.outline.root);
    // Ispettore a destra su tutta l'altezza, nel tono della barra laterale (`.insp`).
    let full = ui.max_rect();
    let right = Rect::from_min_max(pos2(full.max.x - INSPECTOR_W, full.min.y), full.max);
    ui.painter().rect_filled(right, CornerRadius::ZERO, p.sidebar);
    let outer = Rect::from_min_max(pos2(full.min.x + 20.0, full.min.y), pos2(right.min.x - 20.0, full.max.y - 16.0));
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(outer).layout(Layout::top_down(Align::Min)));
    let ui = &mut ui;
    super::overview::interrupted_banner(app, ui, p);
    let searching = app.search.active();
    let listing = app.search.showing();
    if searching {
        results_header(app, ui, p);
    } else {
        toolbar(app, ui, p);
    }
    ui.add_space(12.0);
    let left = ui.available_rect_before_wrap();
    let right = Rect::from_min_max(right.min + vec2(22.0, 22.0), right.max - vec2(22.0, 16.0));
    let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(left).layout(Layout::top_down(Align::Min)));
    if listing {
        results_table(app, &mut lui, p);
    } else if app.explore.list_only {
        tree_table(app, &mut lui, p);
    } else {
        let map_h = (left.height() * 0.46).clamp(160.0, 520.0);
        let map_rect = Rect::from_min_size(left.min, vec2(left.width(), map_h));
        map(app, &mut lui, p, map_rect);
        let table_rect = Rect::from_min_max(pos2(left.min.x, map_rect.max.y + 12.0), left.max);
        let mut tui = lui.new_child(egui::UiBuilder::new().max_rect(table_rect).layout(Layout::top_down(Align::Min)));
        tree_table(app, &mut tui, p);
    }
    let node = if listing {
        app.search.selected.and_then(|r| app.search.outcome.as_ref().and_then(|o| o.hits.get(r).copied()))
    } else {
        app.explore.selected.or(Some(app.explore.outline.root))
    };
    let mut rui = ui.new_child(egui::UiBuilder::new().max_rect(right).layout(Layout::top_down(Align::Min)));
    inspector::show(app, &mut rui, p, node);
}

/// Percorso a briciole della cartella aperta, con dimensione e quota dello spazio usato, nella barra
/// superiore (`.crumbs`). Se non entra, le cartelle più vicine alla radice diventano "…".
pub fn crumbs(app: &mut App, ui: &mut Ui, p: &Palette) {
    let Some(d) = app.data.as_ref() else { return };
    let t = d.tree();
    let root = app.explore.outline.root;
    let mut chain = vec![root];
    let mut n = root;
    while t.parent(n) != quado_model::NONE {
        n = t.parent(n);
        chain.push(n);
    }
    chain.reverse();
    let l = &app.lingue;
    let share = t.alloc(root) as f64 / d.used() as f64;
    let size_line = l.ta("esplora-dimensione-quota", &[("dimensione", app.fmt.size(t.alloc(root)).into()), ("quota", app.fmt.percent(share).into())]);
    let name = |c: u32| if c == Tree::ROOT { d.volume.letter() } else { t.name_lossy(c) };
    // Quante briciole iniziali (dopo la radice) nascondere perché il resto entri.
    let width = |s: String, last: bool| ui.painter().layout_no_wrap(s, if last { semibold(13.0) } else { sans(13.0) }, p.text).size().x + 12.0 + 14.0;
    let widths: Vec<f32> = chain.iter().enumerate().map(|(k, &c)| width(name(c), k + 1 == chain.len())).collect();
    let line_w = ui.painter().layout_no_wrap(size_line.clone(), widgets::font(Type::Callout), p.text_2).size().x + 12.0;
    let avail = ui.available_width() - line_w;
    let mut hidden = 0;
    while hidden + 2 < chain.len() && widths.iter().sum::<f32>() - widths[1..=hidden].iter().sum::<f32>() + if hidden > 0 { 26.0 } else { 0.0 } > avail {
        hidden += 1;
    }
    let mut target = None;
    ui.spacing_mut().item_spacing.x = 2.0;
    for (k, &c) in chain.iter().enumerate() {
        if k >= 1 && k <= hidden {
            if k == 1 {
                widgets::text(ui, "…", sans(13.0), p.text_3);
                let (cr, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                icons::paint(ui.painter(), cr, Icon::ChevronRight, p.text_3);
            }
            continue;
        }
        let label = name(c);
        let last = k + 1 == chain.len();
        let g = ui.painter().layout_no_wrap(label.clone(), if last { semibold(13.0) } else { sans(13.0) }, p.text_2);
        let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 12.0, tokens::focus::MIN_TARGET), if last { Sense::hover() } else { Sense::click() });
        if resp.hovered() && !last {
            ui.painter().rect_filled(r, CornerRadius::same(6), p.hover);
        }
        let color = if last || resp.hovered() { p.text } else { p.text_2 };
        ui.painter().galley(pos2(r.min.x + 6.0, r.center().y - g.size().y / 2.0), g, color);
        widgets::focus_ring(ui, &resp, p, 6);
        resp.widget_info(|| egui::WidgetInfo::labeled(if last { egui::WidgetType::Label } else { egui::WidgetType::Button }, true, &label));
        if resp.clicked() {
            target = Some(c);
        }
        if !last {
            let (cr, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
            icons::paint(ui.painter(), cr, Icon::ChevronRight, p.text_3);
        }
    }
    ui.add_space(10.0);
    widgets::typed(ui, size_line, Type::Callout, p.text_2);
    let Some(d) = app.data.as_ref() else { return };
    if let Some(c) = target {
        let child_on_path = chain.iter().position(|&x| x == c).and_then(|k| chain.get(k + 1).copied());
        app.explore.open(d, c, child_on_path);
    }
}

/// Barra di Esplora (`.exbar`): mappa ed elenco o solo elenco, i filtri in linea; sotto, la legenda dei
/// gestori con le forme.
fn toolbar(app: &mut App, ui: &mut Ui, p: &Palette) {
    ui.horizontal_wrapped(|ui| {
        // I filtri hanno il loro margine interno: tra l'uno e l'altro basta poco.
        ui.spacing_mut().item_spacing = vec2(2.0, 6.0);
        let l = &app.lingue;
        let mut list_only = app.explore.list_only;
        let options = [(false, l.t("esplora-mappa-ed-elenco"), Some(Icon::Explore)), (true, l.t("esplora-solo-elenco"), Some(Icon::List))];
        if widgets::segmented(ui, p, "vista_esplora", &mut list_only, &options) {
            app.explore.list_only = list_only;
        }
        ui.add_space(6.0);
        if filter_chips(ui, p, &app.lingue, &app.fmt, &mut app.search.filters, "esplora") {
            app.search.filters_changed(app.fmt.units);
        }
        // Con i soli filtri: quanti elementi della cartella li soddisfano.
        if app.search.filtering() {
            ui.add_space(8.0);
            widgets::typed(ui, filter_status(app), Type::Callout, p.text_2);
        }
    });
    ui.add_space(6.0);
    legend(ui, p, &app.lingue);
}

/// Stato della ricerca o del filtro: in corso, oppure quanti elementi e in quanto tempo.
fn filter_status(app: &App) -> String {
    let l = &app.lingue;
    match (&app.search.outcome, app.search.pending()) {
        (_, true) => l.t("ricerca-in-corso"),
        (Some(o), false) => {
            let n = o.hits.len();
            l.ta("ricerca-risultati", &[("n", n.into()), ("testo", app.fmt.count(n as u64).into()), ("ms", app.fmt.decimal(o.elapsed.as_secs_f64() * 1000.0, 0).into())])
        }
        (None, false) => String::new(),
    }
}

/// Legenda dei gestori (`.exlegend`): il colore e la forma sono il gestore (04 §6.2); il tratteggio è il
/// non leggibile.
fn legend(ui: &mut Ui, p: &Palette, l: &Lingue) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        let item = |ui: &mut Ui, shape: Glyph, color: egui::Color32, key: &str| {
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 16.0), Sense::hover());
            widgets::glyph(ui.painter(), r.center(), 9.0, shape, color, p);
            widgets::typed(ui, l.t(key), Type::Callout, p.text_2);
            ui.add_space(8.0);
        };
        for (g, key) in [
            (Gestore::App, Gestore::App.key()),
            (Gestore::Mine, Gestore::Mine.key()),
            (Gestore::Windows, Gestore::Windows.key()),
            (Gestore::Unknown, "legenda-non-classificato-ipotesi"),
        ] {
            item(ui, g.shape(), p.data_color(g), key);
        }
        item(ui, Glyph::Hatch, p.hatch, "legenda-non-leggibile");
    });
}

/// Filtro in linea (`.flt`): nome in `text_2`, valore scelto, freccia; apre il suo menu. Nome accessibile
/// "nome valore".
pub fn filter_chip(ui: &mut Ui, p: &Palette, label: &str, value: &str, on: bool) -> Response {
    let lg = ui.painter().layout_no_wrap(label.to_owned(), sans(13.0), p.text_2);
    let vg = ui.painter().layout_no_wrap(value.to_owned(), widgets::medium(13.0), p.text);
    let w = 8.0 + lg.size().x + 4.0 + vg.size().x + 3.0 + 14.0 + 5.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, tokens::focus::MIN_TARGET), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let fill = if open || on {
        p.select
    } else if resp.hovered() {
        p.hover
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(tokens::radius::CONTROL), fill);
    let cy = rect.center().y;
    let mut x = rect.min.x + 8.0;
    let lw = lg.size().x;
    ui.painter().galley(pos2(x, cy - lg.size().y / 2.0), lg, p.text_2);
    x += lw + 4.0;
    let vw = vg.size().x;
    ui.painter().galley(pos2(x, cy - vg.size().y / 2.0), vg, p.text);
    x += vw + 3.0;
    icons::paint(ui.painter(), Rect::from_min_size(pos2(x, cy - 7.0), vec2(14.0, 14.0)), Icon::Caret, p.text_2);
    widgets::focus_ring(ui, &resp, p, tokens::radius::CONTROL);
    let name = format!("{label} {value}");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, &name));
    resp
}

/// Voce di un menu dei filtri: scelta con il segno di spunta, area alta 32.
pub fn menu_choice(ui: &mut Ui, p: &Palette, label: &str, on: bool) -> Response {
    let w = ui.available_width().max(200.0);
    let g = ui.painter().layout_no_wrap(label.to_owned(), sans(13.0), p.text);
    let (rect, resp) = ui.allocate_exact_size(vec2(w.max(g.size().x + 44.0), tokens::focus::MIN_TARGET), Sense::click());
    if resp.hovered() || resp.has_focus() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), p.hover);
    }
    if on {
        icons::paint(ui.painter(), Rect::from_center_size(pos2(rect.min.x + 14.0, rect.center().y), vec2(14.0, 14.0)), Icon::Check, p.text);
    }
    ui.painter().galley(pos2(rect.min.x + 30.0, rect.center().y - g.size().y / 2.0), g, p.text);
    if resp.has_focus() {
        widgets::focus_ring_inside(ui.painter(), rect, p, 6);
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, on, label));
    resp
}

/// Filtri in linea di Esplora e della ricerca (F-EX-4): dimensioni minime, tipo, ultima modifica, gestore,
/// ciascuno con il suo menu; "Togli i filtri" quando ce n'è uno. Restituisce `true` se un filtro è cambiato.
pub fn filter_chips(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, fu: &mut FilterUi, id: &str) -> bool {
    let before = fu.clone();
    // Niente scope intorno ai filtri: in una riga che va a capo, il capo torna al margine della barra.
    // Dimensioni minime: il numero e l'unità, scritti nel menu.
    let min = if fu.min_text.trim().is_empty() { "0".to_owned() } else { fu.min_text.trim().to_owned() };
    let r = filter_chip(ui, p, &l.t("filtro-dimensione-minima"), &format!("{min} {}", fu.min_unit.label(f)), !fu.min_text.trim().is_empty());
    egui::Popup::menu(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        ui.set_min_width(260.0);
        ui.horizontal(|ui| {
            let te = ui.add(egui::TextEdit::singleline(&mut fu.min_text).desired_width(72.0).font(widgets::num(13.0)).id(Id::new((id, "minimo"))));
            widgets::focus_ring(ui, &te, p, 4);
            let units: Vec<(MinUnit, String, Option<Icon>)> = MinUnit::ALL.iter().map(|&u| (u, u.label(f), None)).collect();
            widgets::segmented(ui, p, (id, "unita"), &mut fu.min_unit, &units);
        });
        // Una dimensione minima che non è un numero non filtra niente: lo si dice, invece di ignorarla.
        if !fu.min_text.trim().is_empty() && fu.min_bytes(f.units).is_none() {
            ui.set_max_width(320.0);
            widgets::paragraph(ui, l.t("filtro-dimensione-non-valida"), Type::Callout, p.warn);
        }
    });
    // Tipo secondo l'estensione.
    let fam_text = fu.family.map_or_else(|| l.t("filtro-tipo-tutti"), |fam| l.t(&format!("famiglia-{}", fam.code())));
    let r = filter_chip(ui, p, &l.t("filtro-tipo"), &fam_text, fu.family.is_some());
    egui::Popup::menu(&r).show(|ui| {
        if menu_choice(ui, p, &l.t("filtro-tipo-tutti"), fu.family.is_none()).clicked() {
            fu.family = None;
        }
        for fam in Family::ALL {
            if menu_choice(ui, p, &l.t(&format!("famiglia-{}", fam.code())), fu.family == Some(fam)).clicked() {
                fu.family = Some(fam);
            }
        }
        ui.add_space(4.0);
        ui.set_max_width(300.0);
        widgets::paragraph(ui, l.t("filtro-nota-estensione"), Type::Callout, p.text_2);
    });
    // Ultima modifica.
    let r = filter_chip(ui, p, &l.t("filtro-modificato"), &l.t(fu.date.key()), fu.date != DatePreset::Any);
    egui::Popup::menu(&r).show(|ui| {
        for dp in DatePreset::ALL {
            if menu_choice(ui, p, &l.t(dp.key()), fu.date == dp).clicked() {
                fu.date = dp;
            }
        }
    });
    // Gestito da: i quattro gruppi di colore, "non classificato" compreso.
    let g_text = fu.group.map_or_else(|| l.t("filtro-gestore-tutti"), |g| l.t(g.key()));
    let r = filter_chip(ui, p, &l.t("filtro-gestore"), &g_text, fu.group.is_some());
    egui::Popup::menu(&r).show(|ui| {
        if menu_choice(ui, p, &l.t("filtro-gestore-tutti"), fu.group.is_none()).clicked() {
            fu.group = None;
        }
        for g in Gestore::ALL {
            if menu_choice(ui, p, &l.t(g.key()), fu.group == Some(g)).clicked() {
                fu.group = Some(g);
            }
        }
    });
    if !fu.is_empty() {
        ui.add_space(4.0);
        if widgets::button(ui, p, &l.t("filtro-azzera"), Some(Icon::Close), widgets::ButtonKind::Quiet, true).clicked() {
            *fu = FilterUi { min_unit: fu.min_unit, ..FilterUi::default() };
        }
    }
    *fu != before
}

/// Spazio tra le aree della mappa: 3 punti tra le cartelle del primo livello, 2 tra le aree dentro.
const MAP_GAP_TOP: f32 = 1.5;
const MAP_GAP_INNER: f32 = 1.0;
/// Intestazione di un gruppo della mappa (`.mgh`).
const MAP_HEADER: f32 = 22.0;

/// Rettangolo disegnato di un'area: quello della disposizione ristretto della metà dello spazio tra le aree.
fn drawn_rect(tile: &treemap::Tile) -> Rect {
    tile.rect.shrink(if tile.depth <= 1 { MAP_GAP_TOP } else { MAP_GAP_INNER })
}

/// Raggio di un'area: i gruppi come `.g`, le aree piene come `.tl`.
fn tile_radius(tile: &treemap::Tile) -> u8 {
    if tile.leaf { tokens::radius::TILE_INNER } else { tokens::radius::TILE }
}

fn map(app: &mut App, ui: &mut Ui, p: &Palette, rect: Rect) {
    let Some(d) = app.data.as_ref() else { return };
    let st = &mut app.explore;
    st.geom.map = rect;
    let id = Id::new("mappa");
    let resp = ui.interact(rect, id, Sense::click());
    let a11y = app.lingue.t("accessibile-mappa");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &a11y));
    // La mappa sta su un riquadro arrotondato (`.mapwrap`); le cartelle aperte non hanno fondo proprio,
    // si leggono dall'intestazione e dallo spazio intorno (`.g`).
    ui.painter().rect_filled(rect, CornerRadius::same(tokens::radius::GROUP), p.panel);
    let inner = rect.shrink(2.0);
    let root = st.outline.root;
    let m = &mut st.map;
    // Due livelli sotto la cartella aperta (F-EX-1): i gruppi e le loro aree.
    let opts = treemap::Options { padding: 2.0, header: MAP_HEADER, min_header_rect: 56.0, min_side: 4.0, max_depth: 2, min_area: 2.0, root_header: false };
    if m.key != Some((inner, root, d.generation)) {
        m.layout = treemap::layout(&Src(d), root, inner, &opts);
        m.key = Some((inner, root, d.generation));
        m.painted = None;
        m.hovered = None;
    }
    if m.hatch.as_ref().is_none_or(|(_, c)| *c != [p.hatch, p.track]) {
        let img = treemap::hatch_image(HATCH_PERIOD, p.hatch, p.track);
        let to = TextureOptions { magnification: egui::TextureFilter::Nearest, minification: egui::TextureFilter::Nearest, wrap_mode: egui::TextureWrapMode::Repeat, mipmap_mode: None };
        m.hatch = Some((ui.ctx().load_texture("tratteggio", img, to), [p.hatch, p.track]));
        m.painted = None;
    }
    let ppp = ui.ctx().pixels_per_point();
    if (m.ppp - ppp).abs() > 1e-3 {
        m.ppp = ppp;
        m.painted = None;
    }
    if m.painted.is_none() {
        let hatch = &m.hatch.as_ref().expect("texture appena creata").0;
        let t = d.tree();
        let cls = &d.cls;
        m.painted = Some(treemap::build_rounded_meshes(&m.layout, hatch, HATCH_PERIOD as f32, ppp, |tile| {
            if !tile.leaf || tile.depth == 0 {
                return None;
            }
            let (fill, text) = p.tile_color(super::kbtext::node_gestore(cls, tile.node));
            Some((drawn_rect(tile), tile_radius(tile), treemap::TileStyle { fill, text, hatched: unmeasured(t, tile.node) }))
        }));
    }
    let painter = ui.painter_at(inner);
    let painted = m.painted.as_ref().expect("mesh appena create");
    painter.add(egui::Shape::Mesh(std::sync::Arc::clone(&painted.fills)));
    if !painted.hatches.is_empty() {
        painter.add(egui::Shape::Mesh(std::sync::Arc::clone(&painted.hatches)));
    }
    let t = d.tree();
    let f = &app.fmt;
    for tile in &m.layout.tiles {
        if tile.depth == 0 {
            continue;
        }
        let (_, tile_text) = p.tile_color(super::kbtext::node_gestore(&d.cls, tile.node));
        let r = drawn_rect(tile);
        let g = super::kbtext::node_gestore(&d.cls, tile.node);
        if !tile.leaf {
            // Intestazione del gruppo (`.mgh`): forma del gestore, nome, dimensione; nessun bordo.
            if tile.rect.height() >= opts.min_header_rect && tile.rect.width() >= opts.min_header_rect {
                let name = t.name_lossy(tile.node);
                let size = f.size(t.alloc(tile.node));
                let cy = tile.rect.min.y + MAP_HEADER / 2.0 + 0.5;
                widgets::glyph(&painter, pos2(r.min.x + 10.0, cy), 8.0, g.shape(), p.data_color(g), p);
                // Il nome prima della dimensione: se non entrano tutti e due, resta il nome.
                let sg = painter.layout_no_wrap(size, widgets::num(11.0), p.text_2);
                let full = painter.layout_no_wrap(name.clone(), semibold(11.0), p.text).size().x;
                let with_size = full + sg.size().x + 34.0 <= r.width();
                let ng = widgets::elided(&painter, name, semibold(11.0), p.text, r.width() - 26.0);
                let nw = ng.size().x;
                painter.galley(pos2(r.min.x + 19.0, cy - ng.size().y / 2.0), ng, p.text);
                if with_size {
                    painter.galley(pos2(r.min.x + 25.0 + nw, cy - sg.size().y / 2.0), sg, p.text_2);
                }
            }
            continue;
        }
        let hatched = unmeasured(t, tile.node);
        let color = if hatched { p.text } else { tile_text };
        if r.width() >= 46.0 && r.height() >= 22.0 {
            // Il glifo del gestore precede il nome nelle aree abbastanza larghe (`.tl .n`).
            let top = r.min.y + 5.0;
            let gx = if r.width() > 70.0 && !hatched {
                widgets::glyph(&painter, pos2(r.min.x + 11.0, top + 7.0), 8.0, g.shape(), color, p);
                11.0
            } else {
                0.0
            };
            let ng = widgets::elided(&painter, t.name_lossy(tile.node), semibold(11.0), color, r.width() - 14.0 - gx);
            let h = ng.size().y;
            painter.galley(pos2(r.min.x + 7.0 + gx, top), ng, color);
            // La dimensione si mostra intera o non si mostra: un numero troncato inganna.
            let sg = painter.layout_no_wrap(f.size(t.alloc(tile.node)), widgets::num(11.0), color);
            if sg.size().x <= r.width() - 14.0 && r.height() >= 40.0 {
                painter.galley(pos2(r.min.x + 7.0, top + h), sg, color);
            }
        } else if r.width() > 30.0 && r.height() > 16.0 {
            let ng = widgets::elided(&painter, t.name_lossy(tile.node), sans(10.0), color, r.width() - 8.0);
            painter.galley(pos2(r.min.x + 4.0, r.min.y + 2.0), ng, color);
        }
    }

    let sel_tile = st.selected.and_then(|s| selected_tile(&m.layout, t, s));
    let hover_tile = resp.hover_pos().and_then(|pos| m.layout.leaf_at(pos)).filter(|&k| m.layout.tiles[k].depth > 0);
    m.hovered = hover_tile;
    // Contorno sull'area sotto il puntatore, nel colore della sua etichetta.
    if let Some(h) = hover_tile {
        let tile = &m.layout.tiles[h];
        let (_, tile_text) = p.tile_color(super::kbtext::node_gestore(&d.cls, tile.node));
        painter.rect_stroke(drawn_rect(tile), CornerRadius::same(tile_radius(tile)), Stroke::new(1.5, tile_text), StrokeKind::Inside);
    }
    // Anello sulla selezione: 2 punti nel colore `ring`, con un filetto del fondo dentro sulle aree piene.
    if let Some(s) = sel_tile {
        let tile = &m.layout.tiles[s];
        let r = drawn_rect(tile);
        let rad = tile_radius(tile);
        painter.rect_stroke(r, CornerRadius::same(rad), Stroke::new(tokens::focus::WIDTH, p.ring), StrokeKind::Inside);
        if tile.leaf && r.width() > 8.0 && r.height() > 8.0 {
            painter.rect_stroke(r.shrink(tokens::focus::WIDTH), CornerRadius::same(rad.saturating_sub(2)), Stroke::new(1.0, p.panel), StrokeKind::Inside);
        }
    }
    if resp.has_focus() {
        widgets::focus_ring_inside(ui.painter(), rect, p, tokens::radius::GROUP);
    }

    let hovered_node = hover_tile.map(|k| m.layout.tiles[k].node);
    let clicked = if resp.clicked() { resp.interact_pointer_pos().and_then(|pos| m.layout.leaf_at(pos)).filter(|&k| m.layout.tiles[k].depth > 0) } else { None };
    let double = resp.double_clicked();
    let (l, used) = (&app.lingue, d.used());
    let resp = if let Some(n) = hovered_node {
        let name = t.name_lossy(n);
        let s = l.ta("mappa-suggerimento", &[("dimensione", f.size(t.alloc(n)).into()), ("quota", f.percent(t.alloc(n) as f64 / used as f64).into())]);
        resp.on_hover_ui_at_pointer(|ui| {
            widgets::text(ui, name, semibold(12.0), p.text);
            widgets::text(ui, s, widgets::num(12.0), p.text_2);
        })
    } else {
        resp
    };

    // Interazione.
    let st = &mut app.explore;
    if let Some(k) = clicked {
        let node = st.map.layout.tiles[k].node;
        st.selected = Some(node);
        st.outline.reveal(d.ord(), t, node);
        if double {
            if t.is_dir(node) && d.ord().has_children(node) {
                st.open(d, node, None);
            }
        } else if let Some(r) = st.outline.row_of(node) {
            st.scroll.ensure_visible(r, st.outline.len(), st.geom.visible.max(1.0));
        }
    }
    ui.memory_mut(|mem| mem.set_focus_lock_filter(id, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, escape: false, ..Default::default() }));
    if resp.clicked() {
        resp.request_focus();
    }
    if resp.has_focus() {
        map_keys(app, ui);
    }
}

/// Area della mappa che mostra `node`: la sua, o quella del suo antenato più profondo (F-EX-2).
pub fn selected_tile(layout: &treemap::Layout, t: &Tree, node: u32) -> Option<usize> {
    let mut best: Option<(usize, u16)> = None;
    for (k, tile) in layout.tiles.iter().enumerate() {
        if tile.depth > 0 && quado_model::view::is_ancestor(t, tile.node, node) && best.is_none_or(|(_, dd)| tile.depth > dd) {
            best = Some((k, tile.depth));
        }
    }
    best.map(|(k, _)| k)
}

fn map_keys(app: &mut App, ui: &mut Ui) {
    let Some(d) = app.data.as_ref() else { return };
    let k = NavKeys::read(ui);
    let st = &mut app.explore;
    let layout = &st.map.layout;
    let current = st.selected.and_then(|s| layout.tiles.iter().position(|t| t.leaf && t.node == s));
    let first = layout.tiles.iter().position(|t| t.leaf && t.depth > 0);
    let mut sel = current;
    for (n, dir) in [(k.left, egui::Vec2::LEFT), (k.right, egui::Vec2::RIGHT), (k.up, egui::Vec2::UP), (k.down, egui::Vec2::DOWN)] {
        for _ in 0..n {
            sel = match sel {
                None => first,
                Some(s) => layout.neighbour(s, dir).or(Some(s)),
            };
        }
    }
    if sel != current
        && let Some(s) = sel
    {
        let node = layout.tiles[s].node;
        st.selected = Some(node);
        if let Some(r) = st.outline.reveal(d.ord(), d.tree(), node) {
            st.scroll.ensure_visible(r, st.outline.len(), st.geom.visible.max(1.0));
        }
    }
    if k.enter
        && let Some(node) = st.selected
    {
        let t = d.tree();
        let target = if t.is_dir(node) && d.ord().has_children(node) { node } else { t.parent(node) };
        if target != st.outline.root {
            st.open(d, target, None);
        }
    }
    if k.back {
        st.up(d);
    }
    if k.alt_left {
        st.go_back(d);
    }
    if k.alt_right {
        st.go_forward(d);
    }
}

const TREE_COLS: [Column; 6] = [
    Column { key: "colonna-nome", width: 0.0, right: false, drop: 0 },
    Column { key: "colonna-dimensioni-su-disco", width: 118.0, right: true, drop: 0 },
    Column { key: "colonna-quota", width: 150.0, right: false, drop: 3 },
    Column { key: "colonna-file", width: 92.0, right: true, drop: 4 },
    Column { key: "colonna-ultima-modifica", width: 112.0, right: false, drop: 2 },
    Column { key: "colonna-gestito-da", width: 140.0, right: false, drop: 5 },
];

fn tree_table(app: &mut App, ui: &mut Ui, p: &Palette) {
    let Some(d) = app.data.as_ref() else { return };
    let t = d.tree();
    let ord = d.ord();
    let full = ui.available_rect_before_wrap();
    let inner = full;
    let header = Rect::from_min_size(inner.min, vec2(inner.width(), table::HEADER_H));
    let cols = table::fit_widths(ui.painter(), &app.lingue, &TREE_COLS);
    // Colonne facoltative scelte in Impostazioni: Nome e Dimensioni su disco restano sempre.
    let c = app.settings.columns;
    let hidden = [false, false, !c.share, !c.files, !c.date, !c.managed];
    let xs = table::layout_with(&cols, &hidden, header.min.x, header.max.x - index_scroll::BAR_WIDTH, 220.0);
    let row_h = tokens::metrics(ui.ctx()).row;
    // L'albero è sempre in ordine di dimensioni su disco, decrescente.
    table::header_sorted(ui.painter(), p, &app.lingue, &cols, &xs, header, Some(1));
    let body = Rect::from_min_max(pos2(inner.min.x, header.max.y), inner.max);
    // Stato vuoto: la cartella è stata letta e non contiene niente, oppure non è stata letta.
    if app.explore.outline.is_empty() {
        let root = app.explore.outline.root;
        let key = if unmeasured(t, root) || d.problem(root).is_some() { "esplora-vuoto-non-letta" } else { "esplora-vuoto" };
        table::empty_message(ui.painter(), p, body, &app.lingue.t(key));
        return;
    }
    let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(Layout::top_down(Align::Min)));
    let id = Id::new("tabella-esplora");
    let st = &mut app.explore;
    let outline = &st.outline;
    let selected = st.selected;
    let hovered_prev = st.hovered_row;
    let colors = BarColors { track: p.bg, thumb: p.border, thumb_active: p.text_3 };
    let (l, f) = (&app.lingue, &app.fmt);
    let parent_alloc = |n: u32| t.alloc(t.parent(n)).max(1);
    let not_readable = l.t("tabella-non-leggibile");
    let unclassified = l.t("tabella-non-classificato");
    let hypothesis = l.t("etichetta-ipotesi");
    let out = index_scroll::show_rows(&mut bui, id, &mut st.scroll, outline.len(), row_h, colors, |painter, row, rect| {
        let node = outline.row(row);
        if Some(node) == selected {
            painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.select);
        } else if hovered_prev == Some(row) {
            painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.hover);
        }
        let cy = rect.center().y;
        let un = unmeasured(t, node);
        if let Some((a, b)) = xs[0] {
            let x = a + 10.0 + f32::from(outline.depth(row)) * INDENT;
            if ord.has_children(node) {
                let icon = if outline.is_expanded(node) { Icon::ChevronDown } else { Icon::ChevronRight };
                icons::paint(painter, Rect::from_center_size(pos2(x + 5.0, cy), vec2(12.0, 12.0)), icon, p.text_2);
            }
            // Glifo del gestore (forma e colore), o il tratteggio del non misurato.
            if un {
                widgets::glyph(painter, pos2(x + 19.0, cy), 10.0, Glyph::Hatch, p.hatch, p);
            } else {
                widgets::manager_glyph(painter, pos2(x + 19.0, cy), super::kbtext::node_gestore(&d.cls, node), p);
            }
            let nx = x + 30.0;
            let mut name = t.name_lossy(node);
            if let Some(tag) = name_tag(l, t, node) {
                name = format!("{name}  {tag}");
            }
            let g = widgets::elided(painter, name, sans(13.0), p.text, b - nx - 8.0);
            painter.galley(pos2(nx, cy - g.size().y / 2.0), g, p.text);
        }
        if let Some((_, b)) = xs[1] {
            if un {
                widgets::right_text(painter, pos2(b - 10.0, cy), not_readable.clone(), sans(12.0), p.text_3);
            } else {
                widgets::right_text(painter, pos2(b - 10.0, cy), f.size(t.alloc(node)), widgets::num(13.0), p.text);
            }
        }
        if let Some((a, _)) = xs[2] {
            let share = t.alloc(node) as f64 / parent_alloc(node) as f64;
            widgets::share_bar(painter, Rect::from_min_size(pos2(a + 10.0, cy - 2.0), vec2(44.0, 4.0)), share, p.text_2, p.track);
            painter.text(pos2(a + 62.0, cy), Align2::LEFT_CENTER, f.percent(share), widgets::num(13.0), p.text_2);
        }
        if let Some((_, b)) = xs[3]
            && t.is_dir(node)
        {
            widgets::right_text(painter, pos2(b - 10.0, cy), f.count(u64::from(t.files(node))), widgets::num(13.0), p.text_2);
        }
        if let Some((a, _)) = xs[4] {
            painter.text(pos2(a + 10.0, cy), Align2::LEFT_CENTER, f.date(t.mtime(node)), widgets::num(13.0), p.text_2);
        }
        if let Some((a, b)) = xs[5] {
            // Gestito da (F-EX-2): gruppo con il glifo della provenienza; "ipotesi" se non colora.
            let m = d.cls.match_of(node).map(|k| &d.cls.matches[k]);
            match m {
                Some(m) => {
                    widgets::evidence_glyph(painter, pos2(a + 14.0, cy), super::kbtext::evidence(m.attribution), p);
                    let label = if super::kbtext::is_hypothesis(m) { hypothesis.clone() } else { l.t(super::kbtext::short_key(super::kbtext::manager_gestore(m.manager))) };
                    let g = widgets::elided(painter, label, sans(12.5), p.text_2, b - a - 34.0);
                    painter.galley(pos2(a + 24.0, cy - g.size().y / 2.0), g, p.text_2);
                }
                None => {
                    painter.text(pos2(a + 10.0, cy), Align2::LEFT_CENTER, &unclassified, sans(12.5), p.text_3);
                }
            }
        }
    });
    st.geom.body = out.body;
    st.geom.bar = out.bar;
    st.geom.visible = out.visible;
    st.hovered_row = out.hovered_row;
    let resp = out.response.clone();
    let a11y = l.t("accessibile-tabella");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &a11y));
    ui.memory_mut(|m| m.set_focus_lock_filter(id, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, escape: false, ..Default::default() }));
    if st.focus_pending {
        resp.request_focus();
        st.focus_pending = false;
    }
    if resp.clicked() {
        resp.request_focus();
    }
    if let Some(row) = out.clicked_row {
        let node = st.outline.row(row);
        st.selected = Some(node);
        if let (Some((a, _)), Some(pos)) = (xs[0], resp.interact_pointer_pos()) {
            let x = a + 10.0 + f32::from(st.outline.depth(row)) * INDENT + 5.0;
            if ord.has_children(node) && (pos.x - x).abs() <= 9.0 {
                let open = !st.outline.is_expanded(node);
                st.outline.set_expanded(ord, row, open);
            }
        }
    }
    if resp.double_clicked()
        && let Some(row) = out.hovered_row
    {
        let node = st.outline.row(row);
        if t.is_dir(node) && ord.has_children(node) {
            st.open(d, node, None);
        }
    }
    if resp.has_focus() {
        widgets::focus_ring_inside(ui.painter(), full, p, tokens::radius::ROW);
        table_keys(app, ui);
    }
}

/// Indicazione accanto al nome: collegamento non seguito, cartella non leggibile, cloud.
fn name_tag(l: &Lingue, t: &Tree, n: u32) -> Option<String> {
    let fl = t.flags(n);
    if fl.has(Flags::NOT_FOLLOWED) {
        Some(l.t("tag-collegamento"))
    } else if fl.any(Flags::DENIED | Flags::ERROR) {
        Some(l.t("tag-non-leggibile"))
    } else if fl.has(Flags::CLOUD_NOT_LISTED) {
        Some(l.t("tag-cloud-non-elencata"))
    } else if fl.has(Flags::CLOUD) {
        Some(l.t("tag-cloud"))
    } else {
        None
    }
}

fn table_keys(app: &mut App, ui: &mut Ui) {
    let Some(d) = app.data.as_ref() else { return };
    let k = NavKeys::read(ui);
    let st = &mut app.explore;
    let ord = d.ord();
    let t = d.tree();
    let rows = st.outline.len();
    let page = (st.geom.visible.floor() as usize).saturating_sub(1).max(1);
    let cur = st.selected.and_then(|s| st.outline.row_of(s));
    let mut row = k.apply(cur, rows, page);
    if let Some(r) = row {
        let node = st.outline.row(r);
        for _ in 0..k.right {
            let r = st.outline.row_of(node).unwrap_or(r);
            if ord.has_children(node) {
                if st.outline.is_expanded(node) {
                    row = Some(r + 1);
                } else {
                    st.outline.set_expanded(ord, r, true);
                }
            }
        }
        for _ in 0..k.left {
            let Some(r) = row else { break };
            let node = st.outline.row(r);
            if st.outline.is_expanded(node) {
                st.outline.set_expanded(ord, r, false);
            } else if st.outline.depth(r) > 0 {
                let parent = t.parent(node);
                row = st.outline.row_of(parent);
            }
        }
    }
    if k.star {
        st.outline.expand_all(ord, t);
        row = st.selected.and_then(|s| st.outline.row_of(s));
    }
    if let Some(r) = row
        && r < st.outline.len()
    {
        let node = st.outline.row(r);
        if st.selected != Some(node) || k.moves() {
            st.selected = Some(node);
            st.scroll.ensure_visible(r, st.outline.len(), st.geom.visible);
        }
    }
    if k.enter
        && let Some(node) = st.selected
        && t.is_dir(node)
        && ord.has_children(node)
    {
        st.open(d, node, None);
    }
    if k.back {
        st.up(d);
    }
    if k.alt_left {
        st.go_back(d);
    }
    if k.alt_right {
        st.go_forward(d);
    }
}

/// Testata dei risultati della ricerca per nome: titolo, stato, «Chiudi la ricerca»; sotto, gli stessi
/// filtri in linea di Esplora.
fn results_header(app: &mut App, ui: &mut Ui, p: &Palette) {
    ui.horizontal(|ui| {
        let l = &app.lingue;
        let q = app.search.text.trim().to_owned();
        widgets::typed(ui, l.ta("ricerca-titolo", &[("testo", q.into())]), Type::Headline, p.text);
        ui.add_space(10.0);
        widgets::typed(ui, filter_status(app), Type::Callout, p.text_2);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::button(ui, p, &app.lingue.t("ricerca-chiudi"), Some(Icon::Close), widgets::ButtonKind::Quiet, true).clicked() {
                app.search.clear();
            }
        });
    });
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 6.0);
        if filter_chips(ui, p, &app.lingue, &app.fmt, &mut app.search.filters, "ricerca") {
            app.search.filters_changed(app.fmt.units);
        }
    });
}

const RESULT_COLS: [Column; 4] = [
    Column { key: "colonna-nome", width: 0.0, right: false, drop: 0 },
    Column { key: "colonna-cartella", width: 260.0, right: false, drop: 2 },
    Column { key: "colonna-dimensioni-su-disco", width: 118.0, right: true, drop: 0 },
    Column { key: "colonna-ultima-modifica", width: 112.0, right: false, drop: 1 },
];

/// Tabella di righe piatte (ricerca, File grandi): nome, cartella, dimensione, data.
#[allow(clippy::too_many_arguments)]
pub fn flat_table(
    ui: &mut Ui,
    p: &Palette,
    l: &Lingue,
    f: &Formatter,
    d: &Loaded,
    id: Id,
    rows: &[u32],
    scroll: &mut IndexScroll,
    selected: &mut Option<usize>,
    geom: &mut Geometry,
    focus: &mut bool,
) -> (Response, Option<usize>) {
    let t = d.tree();
    let full = ui.available_rect_before_wrap();
    let inner = full;
    let header = Rect::from_min_size(inner.min, vec2(inner.width(), table::HEADER_H));
    let cols = table::fit_widths(ui.painter(), l, &RESULT_COLS);
    let xs = table::layout(&cols, header.min.x, header.max.x - index_scroll::BAR_WIDTH, 200.0);
    table::header(ui.painter(), p, l, &cols, &xs, header);
    let body = Rect::from_min_max(pos2(inner.min.x, header.max.y), inner.max);
    let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(Layout::top_down(Align::Min)));
    let colors = BarColors { track: p.bg, thumb: p.border, thumb_active: p.text_3 };
    let sel = *selected;
    let row_h = tokens::metrics(ui.ctx()).row;
    let hovered_prev = ui.memory(|m| m.data.get_temp::<usize>(id.with("hover")));
    let out = index_scroll::show_rows(&mut bui, id, scroll, rows.len(), row_h, colors, |painter, row, rect| {
        let node = rows[row];
        if Some(row) == sel {
            painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.select);
        } else if hovered_prev == Some(row) {
            painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.hover);
        }
        let cy = rect.center().y;
        if let Some((a, b)) = xs[0] {
            widgets::manager_glyph(painter, pos2(a + 15.0, cy), super::kbtext::node_gestore(&d.cls, node), p);
            let g = widgets::elided(painter, t.name_lossy(node), sans(13.0), p.text, b - a - 36.0);
            painter.galley(pos2(a + 26.0, cy - g.size().y / 2.0), g, p.text);
        }
        if let Some((a, b)) = xs[1] {
            let parent = t.parent(node);
            let path = if parent == quado_model::NONE { String::new() } else { t.path_lossy(parent) };
            let g = table::path_elided_start(painter, &path, mono(12.0), p.text_2, b - a - 20.0);
            painter.galley(pos2(a + 10.0, cy - g.size().y / 2.0), g, p.text_2);
        }
        if let Some((_, b)) = xs[2] {
            widgets::right_text(painter, pos2(b - 10.0, cy), f.size(t.alloc(node)), widgets::num(13.0), p.text);
        }
        if let Some((a, _)) = xs[3] {
            painter.text(pos2(a + 10.0, cy), Align2::LEFT_CENTER, f.date(t.mtime(node)), widgets::num(13.0), p.text_2);
        }
    });
    ui.memory_mut(|m| {
        match out.hovered_row {
            Some(h) => {
                m.data.insert_temp(id.with("hover"), h);
            }
            None => {
                m.data.remove::<usize>(id.with("hover"));
            }
        }
        m.set_focus_lock_filter(id, egui::EventFilter { horizontal_arrows: false, vertical_arrows: true, escape: false, ..Default::default() });
    });
    geom.body = out.body;
    geom.bar = out.bar;
    geom.visible = out.visible;
    let resp = out.response.clone();
    let a11y = l.t("accessibile-elenco");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &a11y));
    if *focus {
        resp.request_focus();
        *focus = false;
    }
    if resp.clicked() {
        resp.request_focus();
    }
    if let Some(r) = out.clicked_row {
        *selected = Some(r);
    }
    let mut activated = None;
    if resp.double_clicked() {
        activated = out.hovered_row;
    }
    if resp.has_focus() {
        widgets::focus_ring_inside(ui.painter(), full, p, tokens::radius::ROW);
        let k = NavKeys::read(ui);
        let page = (out.visible.floor() as usize).saturating_sub(1).max(1);
        if let Some(r) = k.apply(*selected, rows.len(), page)
            && (Some(r) != *selected || k.moves())
        {
            *selected = Some(r);
            scroll.ensure_visible(r, rows.len(), out.visible);
        }
        if k.enter {
            activated = *selected;
        }
    }
    (resp, activated)
}

fn results_table(app: &mut App, ui: &mut Ui, p: &Palette) {
    let Some(d) = app.data.as_ref() else { return };
    let empty: Vec<u32> = Vec::new();
    let s = &mut app.search;
    let rows = s.outcome.as_ref().map_or(&empty, |o| &o.hits);
    if rows.is_empty() {
        let r = ui.available_rect_before_wrap();
        let key = match (s.pending(), s.filtering(), s.filters.is_empty()) {
            (true, _, _) => "ricerca-in-corso",
            (false, true, _) => "esplora-filtri-nessuno",
            (false, false, true) => "ricerca-nessun-risultato",
            (false, false, false) => "ricerca-nessun-risultato-filtri",
        };
        let msg = app.lingue.t(key);
        table::empty_message(ui.painter(), p, r, &msg);
        return;
    }
    let filtering = s.filtering();
    let (_, activated) = flat_table(ui, p, &app.lingue, &app.fmt, d, Id::new("tabella-ricerca"), rows, &mut s.scroll, &mut s.selected, &mut s.geom, &mut s.focus_results);
    if let Some(r) = activated
        && let Some(&node) = rows.get(r)
    {
        if filtering {
            // Con i soli filtri: Invio su una cartella la apre e i filtri valgono lì.
            if d.tree().is_dir(node) && d.ord().has_children(node) {
                app.search.selected = None;
                app.explore.open(d, node, None);
            }
        } else {
            // "Mostra nella cartella": chiude la ricerca e posiziona l'Esplora sull'elemento.
            app.search.clear();
            app.explore.reveal(d, node);
        }
    }
}

/// Seleziona `node` e lo mostra nella sua cartella (da Panoramica, File grandi).
pub fn show_in_folder(app: &mut App, node: u32) {
    if let Some(d) = app.data.as_ref() {
        app.search.clear();
        app.explore.reveal(d, node);
        app.screen = Screen::Explore;
    }
}
