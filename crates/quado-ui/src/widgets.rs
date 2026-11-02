// SPDX-License-Identifier: GPL-3.0-or-later
//! Componenti comuni con l'aspetto della variante A della proposta visiva: pulsanti (primario, secondario,
//! discreto), controllo segmentato, interruttore, casella, campioni di colore, cursore, gruppi e card
//! separati dal tono, righe di elenco e di impostazione, intestazioni di sezione, icona "i" con la
//! spiegazione a richiesta, glifi dei gestori e della provenienza, tratteggio, suggerimenti con la
//! scorciatoia.
//!
//! Regole comuni:
//! - ogni controllo raggiungibile da tastiera disegna l'anello del focus ([`focus_ring`], 2 px nel colore
//!   `ring`, 2 px fuori dal controllo; [`focus_ring_inside`] per righe e aree che toccano i bordi);
//! - l'area cliccabile è alta almeno 32 punti (`tokens::focus::MIN_TARGET`), anche dove il segno è più piccolo;
//! - ogni controllo dichiara a AccessKit tipo, nome e stato (`Response::widget_info`);
//! - nessun bordo per separare le zone: tono delle superfici, filetti `separator` solo dentro i gruppi.

use std::hash::Hash;
use std::ops::RangeInclusive;
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Galley, Id, Key, Layout, Margin, Painter, Pos2, Rect, Response, RichText, Sense, Shape,
    Stroke, StrokeKind, Ui, UiBuilder, Vec2, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::fonts;
use crate::icons::{self, Icon};
use crate::tokens::{self, Elevation, Gestore, Glyph, Palette, Type, Weight, focus, radius};

// ------------------------------------------------------------------ testo

/// Inter regular (400). Le cifre sono tabulari in tutti i pesi: vale anche per i numeri.
pub fn sans(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Inter light (300): cifre chiave grandi.
pub fn light(size: f32) -> FontId {
    FontId::new(size, fonts::light())
}

/// Inter medium (500).
pub fn medium(size: f32) -> FontId {
    FontId::new(size, fonts::medium())
}

/// Inter semibold (600).
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, fonts::semibold())
}

/// Numeri in colonna (dimensioni, conteggi, percentuali, date): Inter con le cifre tabulari.
pub fn num(size: f32) -> FontId {
    sans(size)
}

/// Numeri in colonna con il peso medium.
pub fn num_medium(size: f32) -> FontId {
    medium(size)
}

/// IBM Plex Mono: solo percorsi e comandi.
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn mono_medium(size: f32) -> FontId {
    FontId::new(size, fonts::mono_medium())
}

/// Font di un ruolo della scala tipografica.
pub fn font(t: Type) -> FontId {
    let family = match t.weight() {
        Weight::Light => fonts::light(),
        Weight::Regular => FontFamily::Proportional,
        Weight::Medium => fonts::medium(),
        Weight::SemiBold => fonts::semibold(),
    };
    FontId::new(t.size(), family)
}

/// Testo di un ruolo, con la spaziatura tra le lettere della scala.
pub fn rich(s: impl Into<String>, t: Type, color: Color32) -> RichText {
    RichText::new(s.into()).font(font(t)).color(color).extra_letter_spacing(t.tracking())
}

/// Etichetta di un ruolo su una riga (si allarga, non va a capo).
pub fn typed(ui: &mut Ui, s: impl Into<String>, t: Type, color: Color32) -> Response {
    ui.add(egui::Label::new(rich(s, t, color)).extend())
}

/// Paragrafo di un ruolo: va a capo nella larghezza disponibile.
pub fn paragraph(ui: &mut Ui, s: impl Into<String>, t: Type, color: Color32) -> Response {
    ui.add(egui::Label::new(rich(s, t, color)).wrap())
}

/// Galley di un ruolo, per chi disegna con il painter; `wrap` è la larghezza massima (None: una riga).
pub fn galley(painter: &Painter, s: impl Into<String>, t: Type, color: Color32, wrap: Option<f32>) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(s.into(), TextFormat { font_id: font(t), color, extra_letter_spacing: t.tracking(), ..Default::default() });
    if let Some(w) = wrap {
        job.wrap.max_width = w.max(8.0);
    }
    painter.layout_job(job)
}

/// Testo con un colore e un font, senza interazione.
pub fn text(ui: &mut Ui, s: impl Into<String>, font: FontId, color: Color32) -> Response {
    ui.label(RichText::new(s.into()).font(font).color(color))
}

/// Testo su una riga, troncato con "…" a `max_width`.
pub fn elided(painter: &Painter, text: String, font: FontId, color: Color32, max_width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(text, font, color);
    job.wrap = TextWrapping { max_width: max_width.max(8.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    painter.layout_job(job)
}

/// Etichetta su una riga: se non entra si tronca e il testo completo va nel suggerimento.
pub fn label_elided(ui: &mut Ui, text: &str, font: FontId, color: Color32, max_width: f32) -> Response {
    let galley = elided(ui.painter(), text.to_owned(), font, color, max_width);
    let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::hover());
    let was_elided = galley.elided;
    ui.painter().galley(rect.min, galley, color);
    if was_elided { resp.on_hover_text(text) } else { resp }
}

/// Scritta allineata a destra in uno spazio di larghezza fissa (colonne numeriche).
pub fn right_text(painter: &Painter, right: Pos2, s: String, font: FontId, color: Color32) {
    painter.text(right, Align2::RIGHT_CENTER, s, font, color);
}

/// Famiglia del font proporzionale semibold (per i test).
pub fn semibold_family() -> FontFamily {
    fonts::semibold()
}

// ------------------------------------------------------------------ superfici

/// Card: fondo `panel`, raggio 14, ombra morbida in chiaro, nessun bordo.
pub fn card(p: &Palette) -> Frame {
    Frame::new().fill(p.panel).corner_radius(CornerRadius::same(radius::CARD)).shadow(p.shadow(Elevation::Card)).inner_margin(Margin::same(20))
}

/// Gruppo di righe (Impostazioni, elenchi brevi): fondo `panel`, raggio 12, senza margini interni (le righe
/// hanno i loro). Le righe si separano con [`inset_separator`].
pub fn group(p: &Palette) -> Frame {
    Frame::new().fill(p.panel).corner_radius(CornerRadius::same(radius::GROUP)).shadow(p.shadow(Elevation::Card))
}

/// Riquadro dentro una card o un dialogo (`panel2`, raggio 12).
pub fn inset(p: &Palette) -> Frame {
    Frame::new().fill(p.panel2).corner_radius(CornerRadius::same(radius::GROUP)).inner_margin(Margin::symmetric(14, 12))
}

/// Separatore orizzontale di 1 px nel colore `separator`, su tutta la larghezza.
pub fn divider(ui: &mut Ui, p: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, p.separator));
}

/// Filetto tra due righe di un gruppo, rientrato a sinistra di `left` (come `.srow+.srow::before`).
pub fn inset_separator(painter: &Painter, rect: Rect, y: f32, left: f32, p: &Palette) {
    painter.hline((rect.min.x + left)..=rect.max.x, y + 0.5, Stroke::new(1.0, p.separator));
}

/// Spazio vuoto di `size`.
pub fn gap(ui: &mut Ui, size: Vec2) {
    ui.allocate_exact_size(size, Sense::hover());
}

// ------------------------------------------------------------------ focus e suggerimenti

fn focus_visible_id() -> Id {
    Id::new("quado_focus_visibile")
}

/// Il focus si vede dopo un tasto e non dopo un clic, come `:focus-visible` della proposta: chi usa la
/// tastiera vede sempre dove si trova; chi usa il mouse non vede l'anello sul controllo appena cliccato né
/// sul focus dato dal programma (la tabella di Esplora quando si apre). Si chiama a ogni frame (`App::show`).
pub fn track_focus_visible(ctx: &egui::Context) {
    let (key, pointer) = ctx.input(|i| (i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. })), i.pointer.any_pressed()));
    if key || pointer {
        ctx.data_mut(|d| d.insert_temp(focus_visible_id(), key && !pointer));
    }
}

/// L'ultimo gesto è stato un tasto: gli anelli del focus si disegnano.
pub fn focus_visible(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(focus_visible_id())).unwrap_or(false)
}

/// Anello del focus da tastiera fuori dal controllo.
pub fn focus_ring(ui: &Ui, resp: &Response, p: &Palette, radius: u8) {
    if resp.has_focus() && focus_visible(ui.ctx()) {
        let r = radius.saturating_add(focus::OFFSET as u8);
        ui.painter().rect_stroke(resp.rect.expand(focus::OFFSET), CornerRadius::same(r), Stroke::new(focus::WIDTH, p.ring), StrokeKind::Outside);
    }
}

/// Anello del focus dentro `rect`: righe di elenchi, mappa, tabelle, dove fuori non c'è spazio.
pub fn focus_ring_inside(painter: &Painter, rect: Rect, p: &Palette, radius: u8) {
    if focus_visible(painter.ctx()) {
        painter.rect_stroke(rect, CornerRadius::same(radius), Stroke::new(focus::WIDTH, p.ring), StrokeKind::Inside);
    }
}

/// Scorciatoia da tastiera in un riquadro (`.kbd`): "Ctrl+K".
pub fn kbd(ui: &mut Ui, p: &Palette, s: &str) -> Response {
    let g = ui.painter().layout_no_wrap(s.to_owned(), medium(11.0), p.text_2);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 10.0, 18.0), Sense::hover());
    ui.painter().rect(rect, CornerRadius::same(4), p.panel2, Stroke::new(1.0, p.border), StrokeKind::Inside);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, p.text_2);
    resp
}

/// Suggerimento al passaggio del mouse e al focus da tastiera, con la scorciatoia accanto se c'è.
pub fn tip(resp: Response, p: &Palette, text: &str, shortcut: Option<&str>) -> Response {
    let contents = |ui: &mut Ui| {
        ui.set_max_width(ui.spacing().tooltip_width);
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::Label::new(RichText::new(text).font(sans(12.0)).color(p.text)).wrap());
            if let Some(k) = shortcut {
                kbd(ui, p, k);
            }
        });
    };
    if resp.has_focus() && !resp.hovered() {
        resp.show_tooltip_ui(contents);
        resp
    } else {
        resp.on_hover_ui(contents)
    }
}

/// Icona "i" con la spiegazione a richiesta: si legge al passaggio del mouse o con il focus da tastiera.
/// Area cliccabile di 32 × 32, nome accessibile uguale alla spiegazione.
pub fn info(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(focus::MIN_TARGET, focus::MIN_TARGET), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(radius::CONTROL), p.hover);
    }
    icons::paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(16.0, 16.0)), Icon::Info, p.text_2);
    focus_ring(ui, &resp, p, radius::CONTROL);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, text));
    tip(resp, p, text, None)
}

// ------------------------------------------------------------------ pulsanti

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// Pieno nel colore dell'accento: l'azione principale di un dialogo o di una schermata.
    Primary,
    /// Fondo `field` (`.btn.sec`).
    Secondary,
    /// Senza fondo finché non ci passa il mouse (`.btn.quiet`).
    Quiet,
}

/// Mescola due colori (0 = `a`, 1 = `b`).
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Pulsante con icona facoltativa, alto 32. Il primario si usa una volta per vista.
pub fn button(ui: &mut Ui, p: &Palette, label: &str, icon: Option<Icon>, kind: ButtonKind, enabled: bool) -> Response {
    let h = tokens::metrics(ui.ctx()).control;
    let font = if kind == ButtonKind::Primary { semibold(13.0) } else { sans(13.0) };
    // Il colore si sceglie quando si disegna (tipo e stato): il testo si compone con il segnaposto.
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER);
    let icon_w = if icon.is_some() { 16.0 + 7.0 } else { 0.0 };
    let size = vec2((galley.size().x + icon_w + 24.0).max(h), h);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    if ui.is_rect_visible(rect) {
        let hovered = enabled && resp.hovered();
        let down = enabled && resp.is_pointer_button_down_on();
        let (fill, color) = match kind {
            ButtonKind::Primary if !enabled => (p.field, p.disabled),
            ButtonKind::Primary => (if down { mix(p.accent, p.bg, 0.22) } else if hovered { mix(p.accent, p.bg, 0.12) } else { p.accent }, p.on_accent),
            ButtonKind::Secondary => (if down { p.select } else if hovered { p.hover } else { p.field }, if enabled { p.text } else { p.disabled }),
            ButtonKind::Quiet => (if down { p.select } else if hovered { p.hover } else { Color32::TRANSPARENT }, if enabled { p.text } else { p.disabled }),
        };
        ui.painter().rect_filled(rect, CornerRadius::same(radius::CONTROL), fill);
        let content_w = galley.size().x + icon_w;
        let mut x = rect.center().x - content_w / 2.0;
        if let Some(i) = icon {
            icons::paint(ui.painter(), Rect::from_min_size(pos2(x, rect.center().y - 8.0), vec2(16.0, 16.0)), i, color);
            x += icon_w;
        }
        ui.painter().galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, color);
    }
    focus_ring(ui, &resp, p, radius::CONTROL);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    resp
}

/// Pulsante con la sola icona, 32 × 32, discreto, con il suggerimento (e la scorciatoia, se c'è).
pub fn icon_button(ui: &mut Ui, p: &Palette, icon: Icon, tip_text: &str, enabled: bool) -> Response {
    icon_button_kbd(ui, p, icon, tip_text, None, enabled)
}

/// Come [`icon_button`], con la scorciatoia nel suggerimento.
pub fn icon_button_kbd(ui: &mut Ui, p: &Palette, icon: Icon, tip_text: &str, shortcut: Option<&str>, enabled: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(focus::MIN_TARGET, focus::MIN_TARGET), sense);
    if enabled && resp.hovered() {
        let fill = if resp.is_pointer_button_down_on() { p.select } else { p.hover };
        ui.painter().rect_filled(rect, CornerRadius::same(radius::CONTROL), fill);
    }
    icons::paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(16.0, 16.0)), icon, if enabled { p.text } else { p.disabled });
    focus_ring(ui, &resp, p, radius::CONTROL);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, tip_text));
    tip(resp, p, tip_text, shortcut)
}

/// Pulsante a pillola per una scelta tra poche (vista, filtro): acceso con il fondo `select`.
pub fn chip(ui: &mut Ui, p: &Palette, label: &str, on: bool) -> Response {
    let font = if on { medium(13.0) } else { sans(13.0) };
    let color = if on { p.text } else { p.text_2 };
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    let size = vec2(galley.size().x + 24.0, focus::MIN_TARGET);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let fill = if on {
        p.select
    } else if resp.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(radius::CONTROL), fill);
    ui.painter().galley(pos2(rect.min.x + 12.0, rect.center().y - galley.size().y / 2.0), galley, color);
    focus_ring(ui, &resp, p, radius::CONTROL);
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, label));
    resp
}

/// Menu a tendina con la freccia della proposta al posto del triangolo pieno di egui. Il fondo è `field`,
/// il focus da tastiera è l'anello degli stili di egui (`tokens::apply`).
pub fn combo(id_salt: impl Hash + std::fmt::Debug, width: f32, selected: impl Into<egui::WidgetText>) -> egui::ComboBox {
    egui::ComboBox::from_id_salt(id_salt).width(width).height(320.0).selected_text(selected).icon(|ui, rect, visuals, _open| {
        let r = Rect::from_center_size(rect.center(), vec2(14.0, 14.0));
        icons::paint(ui.painter(), r, Icon::Caret, visuals.fg_stroke.color);
    })
}

// ------------------------------------------------------------------ controlli di scelta

/// Controllo segmentato (`.seg`): fondo `field`, voce scelta in rilievo (`raised` e ombra in chiaro).
/// Ogni voce è un pulsante raggiungibile con Tab; con il focus, frecce sinistra e destra cambiano la
/// scelta. Restituisce vero se la scelta è cambiata.
pub fn segmented<T: Copy + PartialEq>(ui: &mut Ui, p: &Palette, id_salt: impl Hash + std::fmt::Debug, value: &mut T, options: &[(T, String, Option<Icon>)]) -> bool {
    let base = ui.make_persistent_id(id_salt);
    let pad = 2.0;
    let h = focus::MIN_TARGET;
    let galleys: Vec<_> = options
        .iter()
        .map(|(v, label, _)| {
            let selected = *v == *value;
            ui.painter().layout_no_wrap(label.clone(), if selected { medium(13.0) } else { sans(13.0) }, Color32::PLACEHOLDER)
        })
        .collect();
    let widths: Vec<f32> = options.iter().zip(&galleys).map(|((_, _, icon), g)| g.size().x + 24.0 + if icon.is_some() { 22.0 } else { 0.0 }).collect();
    let total = widths.iter().sum::<f32>() + pad * 2.0 + pad * (options.len().saturating_sub(1)) as f32;
    let (outer, _) = ui.allocate_exact_size(vec2(total, h), Sense::hover());
    ui.painter().rect_filled(outer, CornerRadius::same(radius::CONTROL), p.field);
    let mut changed = false;
    let mut x = outer.min.x + pad;
    let mut move_to: Option<usize> = None;
    for (k, (((v, label, icon), g), w)) in options.iter().zip(galleys).zip(&widths).enumerate() {
        let r = Rect::from_min_size(pos2(x, outer.min.y + pad), vec2(*w, h - pad * 2.0));
        x += w + pad;
        let id = base.with(k);
        // Area cliccabile alta 32 anche se il fondo della voce è alto 28.
        let resp = ui.interact(r.expand2(vec2(0.0, pad)), id, Sense::click());
        let selected = *v == *value;
        if selected {
            let sh = p.shadow(Elevation::Segment);
            if sh != egui::epaint::Shadow::NONE {
                ui.painter().add(sh.as_shape(r, CornerRadius::same(radius::CONTROL - 2)));
            }
            ui.painter().rect_filled(r, CornerRadius::same(radius::CONTROL - 2), p.raised);
        }
        let color = if selected || resp.hovered() { p.text } else { p.text_2 };
        let content = g.size().x + if icon.is_some() { 22.0 } else { 0.0 };
        let mut cx = r.center().x - content / 2.0;
        if let Some(i) = icon {
            icons::paint(ui.painter(), Rect::from_min_size(pos2(cx, r.center().y - 7.0), vec2(14.0, 14.0)), *i, color);
            cx += 22.0;
        }
        ui.painter().galley(pos2(cx, r.center().y - g.size().y / 2.0), g, color);
        if resp.has_focus() {
            focus_ring_inside(ui.painter(), r.expand(1.0), p, radius::CONTROL);
            let (left, right) = ui.input(|i| (i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight)));
            if left && k > 0 {
                move_to = Some(k - 1);
            } else if right && k + 1 < options.len() {
                move_to = Some(k + 1);
            }
        }
        if resp.clicked() && !selected {
            *value = *v;
            changed = true;
        }
        resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, selected, label));
    }
    if let Some(k) = move_to {
        *value = options[k].0;
        changed = true;
        ui.memory_mut(|m| m.request_focus(base.with(k)));
    }
    // Le frecce orizzontali restano al controllo invece di spostare il focus.
    ui.memory_mut(|m| {
        for k in 0..options.len() {
            m.set_focus_lock_filter(base.with(k), egui::EventFilter { horizontal_arrows: true, ..Default::default() });
        }
    });
    changed
}

/// Interruttore (`.sw`): 38 × 22 dentro un'area di 32 di altezza; acceso nel colore dell'accento.
pub fn switch(ui: &mut Ui, p: &Palette, on: &mut bool, label: &str) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(38.0, focus::MIN_TARGET), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, tokens::motion::BASE);
    let track = Rect::from_center_size(rect.center(), vec2(38.0, 22.0));
    let fill = mix(p.border, p.accent, t);
    ui.painter().rect_filled(track, CornerRadius::same(11), fill);
    let knob = pos2(track.min.x + 11.0 + 16.0 * t, track.center().y);
    ui.painter().circle_filled(knob + vec2(0.0, 1.0), 9.0, Color32::from_black_alpha(60));
    ui.painter().circle_filled(knob, 9.0, Color32::WHITE);
    focus_ring(ui, &resp, p, 11);
    let state = *on;
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, state, label));
    resp
}

/// Casella con l'etichetta (`.cb`): spuntata nel colore dell'accento. Riga alta 32.
pub fn checkbox(ui: &mut Ui, p: &Palette, on: &mut bool, label: &str, enabled: bool) -> Response {
    let g = ui.painter().layout_no_wrap(label.to_owned(), sans(13.0), Color32::PLACEHOLDER);
    let size = vec2(16.0 + 10.0 + g.size().x, focus::MIN_TARGET);
    let (rect, mut resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    if enabled && resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let b = Rect::from_min_size(pos2(rect.min.x, rect.center().y - 8.0), vec2(16.0, 16.0));
    let r = CornerRadius::same(4);
    match (enabled, *on) {
        (true, true) => {
            ui.painter().rect_filled(b, r, p.accent);
        }
        (true, false) => {
            ui.painter().rect_stroke(b, r, Stroke::new(1.5, p.text_3), StrokeKind::Inside);
        }
        (false, _) => {
            ui.painter().rect(b, r, p.field, Stroke::new(1.5, p.border), StrokeKind::Inside);
        }
    }
    if *on {
        let c = if enabled { p.on_accent } else { p.text_3 };
        let s = |x: f32, y: f32| pos2(b.min.x + x * 16.0 / 12.0, b.min.y + y * 16.0 / 12.0);
        ui.painter().add(Shape::line(vec![s(2.5, 6.2), s(5.0, 8.6), s(9.6, 3.6)], Stroke::new(2.0, c)));
    }
    ui.painter().galley(pos2(b.max.x + 10.0, rect.center().y - g.size().y / 2.0), g, if enabled { p.text } else { p.text_2 });
    focus_ring(ui, &resp, p, radius::ROW);
    let state = *on;
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, state, label));
    resp
}

/// Campione di colore tondo (accento): scelto con l'anello `ring` staccato dal campione.
pub fn swatch(ui: &mut Ui, p: &Palette, color: Color32, selected: bool, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(focus::MIN_TARGET, focus::MIN_TARGET), Sense::click());
    let c = rect.center();
    if selected {
        ui.painter().circle_stroke(c, 15.0, Stroke::new(2.0, p.ring));
    }
    ui.painter().circle_filled(c, 13.0, color);
    ui.painter().circle_stroke(c, 12.5, Stroke::new(1.0, Color32::from_rgba_unmultiplied(127, 127, 127, 90)));
    if resp.has_focus() {
        ui.painter().circle_stroke(c, 18.0, Stroke::new(focus::WIDTH, p.ring));
    }
    resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, selected, label));
    tip(resp, p, label, None)
}

/// Cursore orizzontale (`.range`): binario di 4 px, parte scelta nell'accento, pomello bianco di 20 px.
/// Frecce, Pagina su e giù, Home e Fine con il focus. `changed()` è vero quando il valore cambia.
#[allow(clippy::too_many_arguments)]
pub fn slider(ui: &mut Ui, p: &Palette, id_salt: impl Hash + std::fmt::Debug, value: &mut f32, range: RangeInclusive<f32>, step: f32, width: f32, label: &str) -> Response {
    let id = ui.make_persistent_id(id_salt);
    let (rect, _) = ui.allocate_exact_size(vec2(width, focus::MIN_TARGET), Sense::hover());
    let mut resp = ui.interact(rect, id, Sense::click_and_drag());
    let (lo, hi) = (*range.start(), *range.end());
    let snap = |v: f32| ((v - lo) / step).round().mul_add(step, lo).clamp(lo, hi);
    let track = Rect::from_min_max(pos2(rect.min.x + 10.0, rect.center().y - 2.0), pos2(rect.max.x - 10.0, rect.center().y + 2.0));
    let before = *value;
    if let Some(pos) = resp.interact_pointer_pos()
        && (resp.dragged() || resp.clicked())
    {
        let t = ((pos.x - track.min.x) / track.width()).clamp(0.0, 1.0);
        *value = snap(lo + t * (hi - lo));
    }
    if resp.has_focus() {
        let (l, r, home, end) = ui.input(|i| {
            (
                i.num_presses(Key::ArrowLeft) + i.num_presses(Key::ArrowDown) + 3 * i.num_presses(Key::PageDown),
                i.num_presses(Key::ArrowRight) + i.num_presses(Key::ArrowUp) + 3 * i.num_presses(Key::PageUp),
                i.key_pressed(Key::Home),
                i.key_pressed(Key::End),
            )
        });
        *value = snap(*value + (r as f32 - l as f32) * step);
        if home {
            *value = lo;
        }
        if end {
            *value = hi;
        }
        ui.memory_mut(|m| m.set_focus_lock_filter(id, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() }));
    }
    if (*value - before).abs() > f32::EPSILON {
        resp.mark_changed();
    }
    let t = ((*value - lo) / (hi - lo)).clamp(0.0, 1.0);
    let x = track.min.x + t * track.width();
    ui.painter().rect_filled(track, CornerRadius::same(2), p.border);
    ui.painter().rect_filled(Rect::from_min_max(track.min, pos2(x, track.max.y)), CornerRadius::same(2), p.accent);
    let knob = pos2(x, rect.center().y);
    ui.painter().circle_filled(knob + vec2(0.0, 1.0), 10.5, Color32::from_black_alpha(70));
    ui.painter().circle_filled(knob, 10.0, Color32::WHITE);
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(focus::OFFSET), CornerRadius::same(radius::CONTROL), Stroke::new(focus::WIDTH, p.ring), StrokeKind::Outside);
    }
    let v = f64::from(*value);
    resp.widget_info(|| WidgetInfo::slider(true, v, label));
    resp
}

// ------------------------------------------------------------------ righe, sezioni, gruppi

/// Titolo di sezione o di gruppo (`.sgrp > h2`): headline 15/600.
pub fn section_header(ui: &mut Ui, p: &Palette, title: &str) -> Response {
    typed(ui, title, Type::Headline, p.text)
}

/// Titolo piccolo di un gruppo della barra laterale o di una legenda: caption 11/500 in `text_2`.
pub fn caption_header(ui: &mut Ui, p: &Palette, title: &str) -> Response {
    typed(ui, title, Type::Caption, p.text_2)
}

/// Riga apribile (`.dis`): freccia e titolo nel ruolo `t`; tutta la riga è il pulsante e un clic (o Invio,
/// Spazio con il focus) apre e chiude. Il nome accessibile è il titolo, con lo stato aperto o chiuso.
pub fn disclosure(ui: &mut Ui, p: &Palette, label: &str, t: Type, open: &mut bool) -> Response {
    let w = ui.available_width();
    let g = galley(ui.painter(), label, t, p.text, Some(w - 30.0));
    let h = (g.size().y + 10.0).max(focus::MIN_TARGET);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    if resp.clicked() {
        *open = !*open;
        resp.mark_changed();
    }
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(radius::ROW), p.hover);
    }
    let first = g.rows.first().map_or(g.size().y, |r| r.height());
    let top = rect.center().y - g.size().y / 2.0;
    let icon = if *open { Icon::ChevronDown } else { Icon::ChevronRight };
    icons::paint(ui.painter(), Rect::from_center_size(pos2(rect.min.x + 11.0, top + first / 2.0), vec2(14.0, 14.0)), icon, p.text_2);
    ui.painter().galley(pos2(rect.min.x + 26.0, top), g, p.text);
    if resp.has_focus() {
        focus_ring_inside(ui.painter(), rect, p, radius::ROW);
    }
    let state = *open;
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::CollapsingHeader, true, label));
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_expanded(state));
    resp
}

/// Titolo di sezione dell'ispettore: corpo 13 semibold.
pub fn section_heading(ui: &mut Ui, p: &Palette, s: &str) {
    ui.add_space(16.0);
    text(ui, s, semibold(13.0), p.text);
    ui.add_space(4.0);
}

/// Riga di un elenco: fondo `hover` al passaggio del mouse e `select` se scelta, raggio 8, focus dentro.
/// L'altezza viene di solito da `tokens::metrics(ctx).row` o `.row_two_line`.
pub fn list_row(ui: &mut Ui, p: &Palette, height: f32, selected: bool) -> (Rect, Response) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let fill = if selected {
        p.select
    } else if resp.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(radius::ROW), fill);
    if resp.has_focus() {
        focus_ring_inside(ui.painter(), rect, p, radius::ROW);
    }
    (rect, resp)
}

/// Righe di un gruppo (`.srows`): ogni riga ha l'etichetta e la descrizione a sinistra e il controllo a
/// destra, centrato in verticale; un filetto rientrato separa le righe.
pub struct Rows<'a> {
    pub ui: &'a mut Ui,
    p: Palette,
    first: bool,
}

/// Gruppo di righe su fondo `panel`. `add` riceve le [`Rows`].
pub fn rows(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Rows<'_>)) -> Response {
    group(p)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut r = Rows { ui, p: *p, first: true };
            add(&mut r);
        })
        .response
}

impl Rows<'_> {
    fn separator(&mut self) {
        if !self.first {
            let r = self.ui.max_rect();
            let y = self.ui.cursor().min.y;
            inset_separator(self.ui.painter(), Rect::from_x_y_ranges(r.x_range(), y..=y), y - 0.5, 16.0, &self.p);
        }
        self.first = false;
    }

    /// Riga con etichetta, descrizione facoltativa e controllo a destra. Il controllo si dispone da
    /// sinistra a destra in un'area larga quanto al frame precedente; se la misura cambia, egui rifà il frame
    /// prima di mostrarlo (`request_discard`), quindi non si vede mai una riga sbagliata.
    pub fn row(&mut self, id_salt: impl Hash + std::fmt::Debug, label: &str, desc: Option<&str>, control: impl FnOnce(&mut Ui)) {
        self.separator();
        let p = self.p;
        let ui = &mut *self.ui;
        let id = ui.make_persistent_id(id_salt);
        let (prev_w, prev_h): (f32, f32) = ui.data(|d| d.get_temp(id)).unwrap_or((0.0, focus::MIN_TARGET));
        let w = ui.available_width();
        let text_w = (w - 32.0 - 20.0 - prev_w).max(120.0);
        let lg = galley(ui.painter(), label, Type::Body, p.text, Some(text_w));
        let dg = desc.map(|d| galley(ui.painter(), d, Type::Callout, p.text_2, Some(text_w)));
        let th = lg.size().y + dg.as_ref().map_or(0.0, |g| g.size().y + 3.0);
        let h = (th.max(prev_h) + 24.0).max(52.0);
        let row = Rect::from_min_size(ui.cursor().min, vec2(w, h));
        // L'area del controllo è alta quanto il controllo al frame precedente e centrata nella riga: così la
        // misura del frame dopo non dipende dall'altezza della riga (che a sua volta dipende dal controllo).
        let ctl = Rect::from_min_max(pos2(row.max.x - 16.0 - prev_w, row.center().y - prev_h / 2.0), pos2(row.max.x - 16.0, row.center().y + prev_h / 2.0));
        let mut cui = ui.new_child(UiBuilder::new().id_salt(id).max_rect(ctl).layout(Layout::top_down(Align::Min)));
        // I controlli si dispongono su una fascia alta 32 e centrati su di essa; un controllo più alto (una
        // griglia di caselle) cresce verso il basso. La misura è quindi esatta e non dipende dalla riga.
        cui.horizontal(control);
        let used = cui.min_rect();
        let ty = row.center().y - th / 2.0;
        let lh = lg.size().y;
        ui.painter().galley(pos2(row.min.x + 16.0, ty), lg, p.text);
        if let Some(g) = dg {
            ui.painter().galley(pos2(row.min.x + 16.0, ty + lh + 3.0), g, p.text_2);
        }
        let resp = ui.allocate_rect(row, Sense::hover());
        let name = desc.map_or_else(|| label.to_owned(), |d| format!("{label}. {d}"));
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &name));
        let now = (used.width(), used.height());
        if (now.0 - prev_w).abs() > 0.5 || (now.1 - prev_h).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(id, now));
            ui.ctx().request_discard("misura di una riga di impostazione");
        }
    }

    /// Riga con il contenuto sotto l'etichetta (controlli larghi: colori dei dati, pulsanti).
    pub fn stacked(&mut self, label: Option<&str>, desc: Option<&str>, contents: impl FnOnce(&mut Ui)) {
        self.separator();
        let p = self.p;
        Frame::new().inner_margin(Margin::symmetric(16, 12)).show(self.ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 3.0;
            if let Some(l) = label {
                paragraph(ui, l, Type::Body, p.text);
            }
            if let Some(d) = desc {
                paragraph(ui, d, Type::Callout, p.text_2);
            }
            ui.add_space(if label.is_some() || desc.is_some() { 7.0 } else { 0.0 });
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            contents(ui);
        });
    }
}

// ------------------------------------------------------------------ glifi dei dati

/// Glifo di una forma (gestori, metadati, non attribuito, libero) nel quadrato di lato `size` centrato in
/// `center`. Il colore dei gestori viene da `p.data_color`; tratteggio e libero usano i colori del tema.
pub fn glyph(painter: &Painter, center: Pos2, size: f32, shape: Glyph, color: Color32, p: &Palette) {
    let h = size / 2.0;
    let pt = |x: f32, y: f32| pos2(center.x + (x - 0.5) * size, center.y + (y - 0.5) * size);
    match shape {
        Glyph::Circle => {
            painter.circle_filled(center, h * 0.84, color);
        }
        Glyph::Diamond => {
            painter.add(Shape::convex_polygon(vec![pt(0.5, 0.04), pt(0.96, 0.5), pt(0.5, 0.96), pt(0.04, 0.5)], color, Stroke::NONE));
        }
        Glyph::Triangle => {
            painter.add(Shape::convex_polygon(vec![pt(0.5, 0.08), pt(0.97, 0.92), pt(0.03, 0.92)], color, Stroke::NONE));
        }
        Glyph::Ring => {
            painter.circle_stroke(center, h * 0.68, Stroke::new(size * 0.2, color));
        }
        Glyph::Bar => {
            painter.rect_filled(Rect::from_min_max(pt(0.05, 0.25), pt(0.95, 0.75)), CornerRadius::same(1), color);
        }
        Glyph::Hatch => {
            let r = Rect::from_center_size(center, vec2(size, size));
            hatch(painter, r, p.hatch, p.track, 3.0);
        }
        Glyph::Square => {
            painter.rect_stroke(Rect::from_center_size(center, vec2(size, size)), CornerRadius::same(2), Stroke::new(1.25, p.text_3), StrokeKind::Inside);
        }
    }
}

/// Glifo del gestore (forma e colore dei dati), lato 10.
pub fn manager_glyph(painter: &Painter, center: Pos2, g: Gestore, p: &Palette) {
    glyph(painter, center, 10.0, g.shape(), p.data_color(g), p);
}

/// Voce di legenda: glifo, etichetta e, sotto o accanto, il valore. Restituisce la risposta della voce.
pub fn legend_item(ui: &mut Ui, p: &Palette, shape: Glyph, color: Color32, label: &str, value: Option<&str>) -> Response {
    let lg = ui.painter().layout_no_wrap(label.to_owned(), sans(13.0), p.text);
    let vg = value.map(|v| ui.painter().layout_no_wrap(v.to_owned(), num(13.0), p.text_2));
    let w = 12.0 + 8.0 + lg.size().x.max(vg.as_ref().map_or(0.0, |g| g.size().x));
    let h = lg.size().y + vg.as_ref().map_or(0.0, |g| g.size().y + 2.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    glyph(ui.painter(), pos2(rect.min.x + 5.0, rect.min.y + lg.size().y / 2.0), 10.0, shape, color, p);
    let ly = lg.size().y;
    ui.painter().galley(pos2(rect.min.x + 20.0, rect.min.y), lg, p.text);
    if let Some(g) = vg {
        ui.painter().galley(pos2(rect.min.x + 20.0, rect.min.y + ly + 2.0), g, p.text_2);
    }
    resp
}

// ------------------------------------------------------------------ provenienza e tratteggio

/// Classe di provenienza di un fatto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evidence {
    Measured,
    Source,
    Inferred,
    Unavailable,
}

impl Evidence {
    pub fn key(self) -> &'static str {
        match self {
            Self::Measured => "fonte-misurato",
            Self::Source => "fonte-da-fonte",
            Self::Inferred => "fonte-dedotto",
            Self::Unavailable => "fonte-non-disponibile",
        }
    }
}

/// Glifo della provenienza in un quadrato di lato 9: pieno, mezzo pieno, vuoto, tratteggiato.
pub fn evidence_glyph(painter: &Painter, center: Pos2, e: Evidence, p: &Palette) {
    let r = Rect::from_center_size(center, vec2(8.0, 8.0));
    let c = p.text_2;
    match e {
        Evidence::Measured => {
            painter.rect_filled(r, CornerRadius::same(1), c);
        }
        Evidence::Source => {
            painter.rect_stroke(r, CornerRadius::same(1), Stroke::new(1.2, c), StrokeKind::Inside);
            painter.rect_filled(Rect::from_min_max(r.min, pos2(r.center().x, r.max.y)), CornerRadius::same(1), c);
        }
        Evidence::Inferred => {
            painter.rect_stroke(r, CornerRadius::same(1), Stroke::new(1.2, c), StrokeKind::Inside);
        }
        Evidence::Unavailable => {
            let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
            painter.extend(Shape::dashed_line(&pts, Stroke::new(1.2, c), 1.6, 1.4));
        }
    }
}

/// Tratteggio a 45°: fondo `b`, linee `a` ogni `period` punti, spesse 1,25.
pub fn hatch(painter: &Painter, rect: Rect, a: Color32, b: Color32, period: f32) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let clip = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    clip.rect_filled(rect, CornerRadius::ZERO, b);
    let w = (period * 0.28).clamp(1.0, 1.5);
    let span = rect.width() + rect.height();
    let mut t = -rect.height();
    while t < span {
        let x0 = rect.min.x + t;
        clip.line_segment([pos2(x0, rect.max.y), pos2(x0 + rect.height(), rect.min.y)], Stroke::new(w, a));
        t += period;
    }
}

/// Spazio non attribuito o non leggibile: il tratteggio della proposta (linee di 1,25 ogni 4,5).
pub fn unattributed(painter: &Painter, rect: Rect, p: &Palette) {
    hatch(painter, rect, p.hatch, p.track, 4.5);
}

/// Barretta di quota: traccia `track`, riempimento nel colore dei dati, estremi arrotondati.
pub fn share_bar(painter: &Painter, rect: Rect, ratio: f64, fill: Color32, track: Color32) {
    let r = CornerRadius::same((rect.height() / 2.0).round() as u8);
    painter.rect_filled(rect, r, track);
    let w = (rect.width() * ratio.clamp(0.0, 1.0) as f32).max(if ratio > 0.0 { 2.0 } else { 0.0 });
    if w > 0.0 {
        painter.rect_filled(Rect::from_min_size(rect.min, vec2(w, rect.height())), r, fill);
    }
}

/// Riquadro di avviso (`.callout`): fondo `warn_bg`, icona nel colore di avviso, un fatto, al massimo un'azione.
pub fn banner(ui: &mut Ui, p: &Palette, message: &str) {
    Frame::new().fill(p.warn_bg).corner_radius(CornerRadius::same(radius::CARD)).inner_margin(Margin::symmetric(14, 11)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            icons::paint(ui.painter(), r, Icon::Warning, p.warn);
            ui.add(egui::Label::new(RichText::new(message).font(sans(13.0)).color(p.text)).wrap());
        });
    });
}

/// Cifra con l'etichetta sulla stessa linea di base. Da 28 punti in su la cifra è in Inter light, come le
/// cifre chiave della proposta (`.t-display`).
pub fn big_figure(ui: &mut Ui, value: &str, label: &str, value_color: Color32, label_color: Color32, size: f32) -> Response {
    let mut job = LayoutJob::default();
    let vf = if size >= 28.0 { light(size) } else { semibold(size) };
    job.append(value, 0.0, TextFormat { font_id: vf, color: value_color, valign: Align::BOTTOM, extra_letter_spacing: -0.02 * size, ..Default::default() });
    job.append(label, 6.0, TextFormat { font_id: sans(13.0), color: label_color, valign: Align::BOTTOM, ..Default::default() });
    let g = ui.painter().layout_job(job);
    let (rect, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(rect.min, g, value_color);
    resp
}

/// Contenuto in una colonna centrata di larghezza massima `max` (pagine della variante A).
pub fn centered_column<R>(ui: &mut Ui, max: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width();
    let col = w.min(max);
    let left = ((w - col) / 2.0).max(0.0);
    let rect = Rect::from_min_size(ui.cursor().min + vec2(left, 0.0), vec2(col, ui.available_height()));
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)));
    let r = add(&mut child);
    let used = child.min_rect();
    ui.allocate_rect(Rect::from_min_max(ui.cursor().min, pos2(ui.max_rect().max.x, used.max.y)), Sense::hover());
    r
}

/// Pagina della variante A (`.page`): scorre in verticale, colonna centrata larga al più `max`, margini
/// 40 ai lati, 8 sopra e 48 sotto. Panoramica, Interventi, Regole, Windows e sistema.
pub fn page<R>(ui: &mut Ui, max: f32, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let mut out = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        centered_column(ui, max, |ui| {
            Frame::new().inner_margin(Margin { left: 40, right: 40, top: 8, bottom: 48 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                out = Some(add(ui));
            });
        });
    });
    out
}

/// Card senza margini interni (`.card` con le righe che arrivano ai bordi con i loro): fondo `panel`,
/// raggio 14, ombra morbida solo in chiaro.
pub fn bare_card(p: &Palette) -> Frame {
    Frame::new().fill(p.panel).corner_radius(CornerRadius::same(radius::CARD)).shadow(p.shadow(Elevation::Card))
}

/// Testata di una card (`.gh`): titolo e, sulla stessa linea, un sottotitolo breve; a destra l'icona "i"
/// con la spiegazione a richiesta, se c'è.
pub fn card_header(ui: &mut Ui, p: &Palette, title: &str, subtitle: Option<&str>, info_text: Option<&str>) {
    Frame::new().inner_margin(Margin { left: 20, right: 14, top: 18, bottom: 8 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            typed(ui, title, Type::Headline, p.text);
            if let Some(s) = subtitle {
                typed(ui, s, Type::Callout, p.text_2);
            }
            if let Some(i) = info_text {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    info(ui, p, i);
                });
            }
        });
    });
}

/// Una cifra della fila di [`figures`]: valore, etichetta e una nota facoltativa sotto.
pub type Figure = (String, String, Option<String>);

/// Fila di cifre (`.dlg .nums`): ogni colonna ha la cifra grande e leggera (Inter light di `size` punti),
/// l'etichetta nel testo principale e la nota in `text_2`; un filetto separa le colonne. Il riquadro ha il
/// fondo `fill` (`panel2` dentro card e dialoghi) e il raggio dei gruppi. Valore, etichetta e nota sono
/// testi separati: ognuno si legge (e si cerca) da solo.
pub fn figures(ui: &mut Ui, p: &Palette, fill: Color32, size: f32, items: &[Figure]) -> Response {
    let n = items.len().max(1);
    let r = Frame::new().fill(fill).corner_radius(CornerRadius::same(radius::GROUP)).inner_margin(Margin::symmetric(0, 16)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let col = (ui.available_width() / n as f32).floor();
        // Una cifra non va a capo: se la più larga non entra nella colonna, tutte si rimpiccioliscono insieme
        // (fino a 18 punti), così restano della stessa dimensione.
        let widest = items.iter().map(|(v, _, _)| ui.painter().layout_no_wrap(v.clone(), light(size), p.text).size().x).fold(0.0, f32::max);
        let size = if widest > col - 36.0 && widest > 0.0 { (size * (col - 36.0) / widest).floor().max(18.0) } else { size };
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (k, (value, label, sub)) in items.iter().enumerate() {
                ui.push_id(("cifra", k), |ui| {
                    ui.allocate_ui_with_layout(vec2(col, 0.0), Layout::top_down(Align::Min), |ui| {
                        Frame::new().inner_margin(Margin::symmetric(18, 0)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.add(egui::Label::new(RichText::new(value).font(light(size)).color(p.text).extra_letter_spacing(-0.02 * size)).wrap());
                            ui.add_space(4.0);
                            paragraph(ui, label, Type::Callout, p.text);
                            if let Some(s) = sub {
                                paragraph(ui, s, Type::Callout, p.text_2);
                            }
                        });
                    });
                });
            }
        });
        col
    });
    let rect = r.response.rect;
    for k in 1..n {
        let x = rect.min.x + r.inner * k as f32;
        ui.painter().vline(x, (rect.min.y + 16.0)..=(rect.max.y - 16.0), Stroke::new(1.0, p.separator));
    }
    r.response
}

/// Riga di un elenco a due colonne in un gruppo (`.ledger .lr`, righe di Windows e sistema): glifo della
/// provenienza facoltativo, etichetta a sinistra, valore a destra in `text_2`; entrambi vanno a capo. Il
/// nome accessibile è "etichetta: valore"; il suggerimento del glifo dice la provenienza.
pub fn key_value(ui: &mut Ui, p: &Palette, evidence: Option<(Evidence, &str)>, label: &str, value: &str) -> Response {
    key_value_indented(ui, p, evidence, label, value, 0.0)
}

/// Come [`key_value`], con l'etichetta rientrata di `indent` (una misura che appartiene alla riga sopra): la
/// colonna dei valori resta allineata con le altre righe.
pub fn key_value_indented(ui: &mut Ui, p: &Palette, evidence: Option<(Evidence, &str)>, label: &str, value: &str, indent: f32) -> Response {
    let w = ui.available_width();
    let painter = ui.painter().clone();
    let x0 = indent + if evidence.is_some() { 20.0 } else { 0.0 };
    let value_x = (w * 0.42).max(180.0);
    let label_w = (value_x - x0 - 16.0).max(80.0);
    let value_w = (w - value_x).max(80.0);
    let lg = galley(&painter, label, Type::Body, p.text, Some(label_w));
    let vg = galley(&painter, value, Type::Body, p.text_2, Some(value_w));
    let h = lg.size().y.max(vg.size().y) + 18.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let first = lg.rows.first().map_or(lg.size().y, |r| r.height());
    if let Some((e, tip_text)) = evidence {
        let c = pos2(rect.min.x + indent + 6.0, rect.min.y + 9.0 + first / 2.0);
        evidence_glyph(&painter, c, e, p);
        let g = ui.interact(Rect::from_center_size(c, vec2(14.0, 18.0)), resp.id.with("fonte"), Sense::hover());
        g.on_hover_text(tip_text);
    }
    painter.galley(pos2(rect.min.x + x0, rect.min.y + 9.0), lg, p.text);
    painter.galley(pos2(rect.min.x + value_x, rect.min.y + 9.0), vg, p.text_2);
    let name = format!("{label}: {value}");
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &name));
    resp
}

/// Campo di ricerca (`.sb-search`): fondo `field`, raggio dei controlli, lente a sinistra, segnaposto in
/// `text_3`, alto 32; l'anello del focus fuori dal campo quando si scrive. Restituisce la risposta del testo.
pub fn search_field(ui: &mut Ui, p: &Palette, id: Id, text: &mut String, hint: &str, width: f32) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(width, focus::MIN_TARGET), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(radius::CONTROL), p.field);
    icons::paint(ui.painter(), Rect::from_center_size(pos2(rect.min.x + 18.0, rect.center().y), vec2(16.0, 16.0)), Icon::Search, p.text_3);
    let edit_rect = Rect::from_min_max(pos2(rect.min.x + 34.0, rect.min.y + 2.0), pos2(rect.max.x - 10.0, rect.max.y - 2.0));
    let mut child = ui.new_child(UiBuilder::new().max_rect(edit_rect).layout(Layout::left_to_right(Align::Center)));
    let edit = egui::TextEdit::singleline(text).id(id).frame(Frame::NONE).font(sans(13.0)).desired_width(edit_rect.width()).hint_text(RichText::new(hint).font(sans(13.0)).color(p.text_3));
    let resp = child.add(edit);
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(focus::OFFSET), CornerRadius::same(radius::CONTROL + 2), Stroke::new(focus::WIDTH, p.ring), StrokeKind::Outside);
    }
    resp
}

/// Identificatore stabile per lo stato di un componente nei dati temporanei di egui.
pub fn state_id(name: &str) -> Id {
    Id::new(("quado_componente", name))
}
