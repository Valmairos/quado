// SPDX-License-Identifier: GPL-3.0-or-later
//! Elenco delle regole (05 D6, 07 §5.1), variante A: un gruppo di righe come Impostazioni, una per regola con
//! nome, identificativo, data di verifica, intervallo di versioni e scadenza. Una riga si apre (la freccia, un
//! clic o Invio) e mostra le fonti con URL ed estratto, «Copia indirizzo» e il caso di prova. Si apre dal
//! banner "regole da riverificare" e da "Apri la regola" nell'ispettore, che la seleziona e la porta in vista.

use egui::{CornerRadius, Frame, Margin, Rect, Sense, Ui, pos2, vec2};
use quado_kb::Rule;
use quado_model::record::Expiry;

use super::App;
use super::data::today;
use crate::icons::{self, Icon};
use crate::tokens::{Palette, Type, radius};
use crate::widgets::{self, ButtonKind, mono};

#[derive(Default)]
pub struct RulesState {
    /// Regola aperta (indice nella base delle regole).
    pub selected: Option<usize>,
    /// La regola selezionata va portata in vista al prossimo frame.
    pub scroll_to: bool,
}

/// Testo della scadenza di una regola (07 §5.1), vuoto se è fresca.
pub fn expiry_text(app: &App, rule: &Rule) -> Option<String> {
    let (l, f) = (&app.lingue, &app.fmt);
    let verified = f.day(rule.verified);
    match Expiry::at(rule.verified, today()) {
        Expiry::Fresh => None,
        Expiry::Recheck => Some(l.ta("regole-scadenza-riverificare", &[("data", verified.into())])),
        Expiry::Announce => Some(l.ta("regole-scadenza-annuncio", &[("data", verified.into()), ("limite", f.day(rule.verified.add_months(12)).into())])),
        Expiry::Expired => Some(l.ta("regole-scadenza-scaduta", &[("data", verified.into())])),
    }
}

/// Larghezza massima della pagina, margini compresi.
const PAGE_MAX: f32 = 1000.0;

pub fn show(app: &mut App, ui: &mut Ui, p: &Palette) {
    let kb = std::sync::Arc::clone(&app.kb);
    let mut select: Option<Option<usize>> = None;
    let mut copy: Option<String> = None;
    let selected = app.rules_view.selected;
    let mut scroll_to = app.rules_view.scroll_to;
    widgets::page(ui, PAGE_MAX, |ui| {
        let (l, f) = (&app.lingue, &app.fmt);
        ui.add_space(12.0);
        ui.horizontal_top(|ui| {
            ui.add_space(4.0);
            let n = kb.rules.len();
            widgets::paragraph(ui, l.ta("regole-sottotitolo", &[("n", n.into()), ("testo", f.count(n as u64).into())]), Type::Body, p.text_2);
        });
        ui.add_space(16.0);
        // Stato vuoto: la base delle regole incorporata non si è caricata.
        if kb.rules.is_empty() {
            widgets::card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::paragraph(ui, l.t("regole-vuoto"), Type::Body, p.text_2);
            });
            return;
        }
        widgets::bare_card(p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut prev_open = false;
            for (i, r) in kb.rules.iter().enumerate() {
                let open = selected == Some(i);
                // Filetto rientrato tra le righe; una regola aperta ha già il suo riquadro.
                if i > 0 && !open && !prev_open {
                    let y = ui.cursor().min.y;
                    widgets::inset_separator(ui.painter(), ui.max_rect(), y - 0.5, 44.0, p);
                }
                prev_open = open;
                let (resp, rect) = ui.push_id(("regola", i), |ui| rule_row(app, ui, p, r, open)).inner;
                if open && scroll_to {
                    ui.scroll_to_rect(rect, Some(egui::Align::Min));
                    scroll_to = false;
                }
                if resp.clicked() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    select = Some(if open { None } else { Some(i) });
                }
                if open && let Some(u) = ui.push_id(("fonti", i), |ui| sources(app, ui, p, r)).inner {
                    copy = Some(u);
                }
            }
        });
    });
    app.rules_view.scroll_to = scroll_to;
    if let Some(s) = select {
        app.rules_view.selected = s;
    }
    if let Some(u) = copy {
        ui.ctx().copy_text(u);
        app.notice.set(app.lingue.t("avviso-link-copiato"));
    }
}

/// Riga di una regola (`.srow` apribile): freccia, nome e identificativo; sotto la verifica con le versioni,
/// la scadenza nel tono di avviso e, chiusa, quante fonti ha. Tutta la riga è il pulsante che la apre.
fn rule_row(app: &App, ui: &mut Ui, p: &Palette, r: &Rule, open: bool) -> (egui::Response, Rect) {
    let (l, f) = (&app.lingue, &app.fmt);
    let w = ui.available_width();
    let painter = ui.painter().clone();
    let name = l.t(&r.key("nome"));
    let id_g = painter.layout_no_wrap(r.id.clone(), mono(12.0), p.text_2);
    let text_w = (w - 44.0 - 16.0 - id_g.size().x - 20.0).max(120.0);
    let ng = widgets::galley(&painter, name.clone(), Type::BodyStrong, p.text, Some(text_w));
    let mut line = l.ta("regole-verificata", &[("data", f.day(r.verified).into())]);
    if let Some(v) = &r.versions {
        line.push_str(" · ");
        line.push_str(&l.ta("regole-versioni", &[("programma", v.programma.clone().into()), ("min", v.min.clone().into()), ("max", v.max.clone().into())]));
    }
    // Chiusa, la riga dice anche quante fonti ha: aperta, le fonti sono sotto.
    if !open {
        line.push_str(" · ");
        line.push_str(&l.ta("regole-fonti", &[("n", r.sources.len().into()), ("testo", f.count(r.sources.len() as u64).into())]));
    }
    let lg = widgets::galley(&painter, line, Type::Callout, p.text_2, Some(text_w));
    let expiry = expiry_text(app, r).map(|e| widgets::galley(&painter, e, Type::Callout, p.warn, Some(text_w - 20.0)));
    let h = 14.0 + ng.size().y + 3.0 + lg.size().y + expiry.as_ref().map_or(0.0, |g| g.size().y + 3.0) + 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let fill = if open {
        p.select
    } else if resp.hovered() {
        p.hover
    } else {
        egui::Color32::TRANSPARENT
    };
    painter.rect_filled(rect.shrink2(vec2(8.0, 4.0)), CornerRadius::same(radius::ROW), fill);
    let x = rect.min.x + 44.0;
    let mut y = rect.min.y + 14.0;
    let first = ng.rows.first().map_or(ng.size().y, |row| row.height());
    let icon = if open { Icon::ChevronDown } else { Icon::ChevronRight };
    icons::paint(&painter, Rect::from_center_size(pos2(rect.min.x + 26.0, y + first / 2.0), vec2(14.0, 14.0)), icon, p.text_2);
    let nh = ng.size().y;
    painter.galley(pos2(x, y), ng, p.text);
    painter.galley(pos2(rect.max.x - 20.0 - id_g.size().x, y + (first - id_g.size().y) / 2.0), id_g, p.text_2);
    y += nh + 3.0;
    let lh = lg.size().y;
    painter.galley(pos2(x, y), lg, p.text_2);
    y += lh + 3.0;
    if let Some(g) = expiry {
        icons::paint(&painter, Rect::from_min_size(pos2(x, y + 1.0), vec2(13.0, 13.0)), Icon::Warning, p.warn);
        painter.galley(pos2(x + 20.0, y), g, p.warn);
    }
    if resp.has_focus() {
        widgets::focus_ring_inside(ui.painter(), rect.shrink2(vec2(8.0, 4.0)), p, radius::ROW);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::CollapsingHeader, true, &name));
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_expanded(open));
    (resp, rect)
}

/// Le fonti di una regola aperta, ognuna in un riquadro (`.inset`): titolo e tipo, URL, estratto e «Copia
/// indirizzo»; in fondo il caso di prova. Restituisce l'URL da copiare.
fn sources(app: &App, ui: &mut Ui, p: &Palette, r: &Rule) -> Option<String> {
    let l = &app.lingue;
    let mut copy = None;
    Frame::new().inner_margin(Margin { left: 44, right: 20, top: 6, bottom: 16 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 8.0;
        for (k, s) in r.sources.iter().enumerate() {
            widgets::inset(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                let kind = l.t(if s.ufficiale { "regole-fonte-ufficiale" } else { "regole-fonte-non-ufficiale" });
                widgets::paragraph(ui, format!("{} · {kind}", s.titolo), Type::BodyStrong, p.text);
                let (label, shown, copyable) = super::kbtext::source_place(l, &s.url);
                if let Some(label) = label {
                    widgets::paragraph(ui, label, Type::Callout, p.text_2);
                }
                ui.add(egui::Label::new(egui::RichText::new(shown).font(mono(12.0)).color(p.text_2)).wrap());
                widgets::paragraph(ui, format!("«{}»", s.estratto), Type::Body, p.text);
                if copyable && ui.push_id(("copia", k), |ui| widgets::button(ui, p, &l.t("regole-copia-link"), Some(Icon::Copy), ButtonKind::Quiet, true)).inner.clicked() {
                    copy = Some(s.url.clone());
                }
            });
        }
        widgets::paragraph(ui, l.ta("regole-fixture", &[("nome", r.fixture.clone().into())]), Type::Callout, p.text_2);
    });
    copy
}
