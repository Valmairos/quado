// SPDX-License-Identifier: GPL-3.0-or-later
//! Panoramica, variante A: si apre con la risposta, le cifre grandi e leggere dell'usato e del libero, la
//! barra del bilancio con l'allocato diviso per gestore, metadati, non attribuito a tratteggio e deriva come
//! incertezza, la legenda con le forme. Sotto, affiancate, "Dove finisce lo spazio" (gli elementi più grandi
//! senza sovrapposizioni, spiegati dalle regole) e gli interventi possibili, solo da record di
//! raccomandazione, ognuno con il suo pulsante successivo. In fondo "Perché i conti non tornano del tutto"
//! come riga apribile con il non attribuito in vista: dentro, le righe dei conti, le liste "Osservato" e
//! "Possibile" e "Ripeti come amministratore". Nessuna frase causale sullo spazio mancante. Un clic su una
//! voce apre Esplora sulla voce.

use egui::{Align, Align2, CornerRadius, Layout, Rect, ScrollArea, Sense, Stroke, Ui, pos2, vec2};
use quado_model::{Flags, NONE, Tree};
use quado_scan_win::helper::pipe::PipeError;
use quado_scan_win::volumes::Support;

use quado_model::record::{Action, Estimate, NotEstimable, State};

use super::data::Loaded;
use super::downloads_view;
use super::explore;
use super::helper_jobs::{self, HelperFailure, Outcome, Phase};
use super::kbtext;
use super::{App, Screen, volume_title};
use crate::format::Formatter;
use crate::i18n::Lingue;
use crate::icons::{self, Icon};
use crate::tokens::{self, Gestore, Glyph, Palette, Type};
use crate::widgets::{self, ButtonKind, Evidence};

/// Banner "Scansione interrotta" su ogni schermata con dati parziali e avviso sul file system.
pub fn interrupted_banner(app: &App, ui: &mut Ui, p: &Palette) {
    let Some(d) = &app.data else { return };
    let l = &app.lingue;
    if d.cancelled {
        widgets::banner(ui, p, &l.t("banner-interrotta"));
        ui.add_space(12.0);
    }
    match d.volume.support {
        Support::Full => {}
        Support::Fat => {
            widgets::banner(ui, p, &l.ta("banner-fat", &[("fs", d.volume.file_system.clone().into())]));
            ui.add_space(12.0);
        }
        Support::NoGuarantees => {
            widgets::banner(ui, p, &l.ta("banner-senza-garanzie", &[("fs", d.volume.file_system.clone().into())]));
            ui.add_space(12.0);
        }
    }
    if let Some((v, code)) = &app.scan_error {
        widgets::banner(ui, p, &l.ta("banner-errore-scansione", &[("volume", volume_title(l, v).into()), ("codice", code.to_string().into())]));
        ui.add_space(12.0);
    }
}

/// Banner "N regole da riverificare", con un solo pulsante che apre l'elenco delle regole.
fn recheck_banner(app: &App, ui: &mut Ui, p: &Palette) -> bool {
    let Some(d) = &app.data else { return false };
    let n = d.cls.recheck.len();
    if n == 0 {
        return false;
    }
    let l = &app.lingue;
    let mut open = false;
    widgets::banner(ui, p, &l.ta("banner-regole-da-riverificare", &[("n", n.into()), ("testo", app.fmt.count(n as u64).into())]));
    ui.add_space(4.0);
    if widgets::button(ui, p, &l.t("banner-regole-apri"), Some(Icon::Rules), widgets::ButtonKind::Secondary, true).clicked() {
        open = true;
    }
    ui.add_space(12.0);
    open
}

/// Stato della Panoramica che resta tra i frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct OverviewState {
    /// "Perché i conti non tornano del tutto" aperta.
    pub why_open: bool,
}

/// Larghezza massima della pagina (`.page`), margini compresi.
const PAGE_MAX: f32 = 1240.0;

pub fn show(app: &mut App, ui: &mut Ui, p: &Palette) {
    let mut open: Option<u32> = None;
    let mut lens_open: Option<usize> = None;
    let mut recycle: Option<usize> = None;
    let mut rules = false;
    let mut helper = None;
    // Mentre l'helper lavora o ha un esito da dire, la riga dei conti resta aperta.
    if app.helper.phase().is_some() {
        app.overview.why_open = true;
    }
    let mut why_open = app.overview.why_open;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        widgets::centered_column(ui, PAGE_MAX, |ui| {
            egui::Frame::new().inner_margin(egui::Margin { left: 40, right: 40, top: 8, bottom: 48 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                interrupted_banner(app, ui, p);
                rules = recheck_banner(app, ui, p);
                let Some(d) = &app.data else { return };
                let (l, f) = (&app.lingue, &app.fmt);
                // Testata (`.ov-head`): il nome dell'unità e, sotto, file system, cluster e scansione.
                ui.add_space(12.0);
                widgets::typed(ui, volume_title(l, &d.volume), Type::LargeTitle, p.text);
                ui.add_space(4.0);
                let cluster = d.facts.and_then(|v| v.ntfs).map(|n| f.size(u64::from(n.bytes_per_cluster)));
                let fs = match cluster {
                    Some(c) => l.ta("panoramica-meta-cluster", &[("fs", d.volume.file_system.clone().into()), ("cluster", c.into())]),
                    None => d.volume.file_system.clone(),
                };
                // La scansione gira in questo processo: è elevata se lo è Quado.
                let scan = l.ta(
                    if app.env.elevated { "panoramica-scansione-amministratore" } else { "panoramica-scansione" },
                    &[("data", f.date(d.finished).into()), ("ora", f.time(d.finished).into()), ("durata", f.seconds(d.duration.as_secs_f64()).into())],
                );
                widgets::paragraph(ui, format!("{fs} · {scan}"), Type::Body, p.text_2);
                ui.add_space(22.0);
                // La risposta per prima: quanto è usato, quanto è libero, e come.
                usage(app, ui, p);
                ui.add_space(24.0);
                let w = ui.available_width();
                let gap = 24.0;
                let side_by_side = w >= 860.0;
                let left_w = if side_by_side { ((w - gap) * 7.0 / 12.0).floor() } else { w };
                let right_w = if side_by_side { w - gap - left_w } else { w };
                let mut cards = |ui: &mut Ui, which: u8| match which {
                    0 => {
                        ui.set_width(left_w);
                        if let Some(n) = top_list(app, ui, p) {
                            open = Some(n);
                        }
                    }
                    _ => {
                        ui.set_width(right_w);
                        match interventions_card(app, ui, p) {
                            Some(Open::Node(n)) => open = Some(n),
                            Some(Open::Lens(k)) => lens_open = Some(k),
                            Some(Open::Recycle(m)) => recycle = Some(m),
                            None => {}
                        }
                    }
                };
                if side_by_side {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        ui.allocate_ui_with_layout(vec2(left_w, 10.0), Layout::top_down(Align::Min), |ui| cards(ui, 0));
                        ui.allocate_ui_with_layout(vec2(right_w, 10.0), Layout::top_down(Align::Min), |ui| cards(ui, 1));
                    });
                } else {
                    cards(ui, 0);
                    ui.add_space(gap);
                    cards(ui, 1);
                }
                ui.add_space(24.0);
                helper = why_card(app, ui, p, &mut why_open);
            });
        });
    });
    app.overview.why_open = why_open;
    if let Some(n) = open {
        explore::show_in_folder(app, n);
    }
    if let Some(k) = lens_open {
        app.downloads.select_finding(k);
        app.go(Screen::Downloads);
    }
    if let Some(m) = recycle {
        // «Sposta nel Cestino…» dalla card: la stessa simulazione e lo stesso dialogo dell'ispettore.
        super::intervention_jobs::dispatch(app, super::intervention_jobs::ActRequest::PrepareRecycle, super::intervention_jobs::Origin::Rule(m));
    }
    if rules {
        app.rules_view.selected = None;
        app.go(Screen::Rules);
    }
    match helper {
        Some(HelperAction::Start) => helper_jobs::start(app),
        Some(HelperAction::Cancel) => helper_jobs::cancel(app),
        None => {}
    }
}

/// Segmenti della barra del bilancio: (chiave dell'etichetta, byte, stile).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Seg {
    Data(Gestore),
    Meta,
    Hatch,
    Free,
}

/// Card dell'uso (`.usage`): le due cifre grandi e leggere, la nota sulle unità, la barra del bilancio con
/// l'allocato diviso per gestore, la legenda con le forme e i valori, le note su deriva e quota.
fn usage(app: &App, ui: &mut Ui, p: &Palette) {
    let Some(d) = &app.data else { return };
    let (l, f) = (&app.lingue, &app.fmt);
    let frame = egui::Frame::new()
        .fill(p.panel)
        .corner_radius(CornerRadius::same(tokens::radius::CARD))
        .shadow(p.shadow(tokens::Elevation::Card))
        .inner_margin(egui::Margin { left: 32, right: 32, top: 28, bottom: 24 });
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        let units = l.t(match app.fmt.units {
            crate::format::Units::Explorer => "bilancio-unita-esplora",
            crate::format::Units::Si => "bilancio-unita-si",
        });
        let Some(b) = d.balance else {
            widgets::typed(ui, l.t("bilancio-titolo"), Type::Headline, p.text);
            ui.add_space(8.0);
            widgets::paragraph(ui, l.t("bilancio-non-disponibile"), Type::Body, p.text_2);
            return;
        };
        let free = b.capacity.saturating_sub(b.used);
        // Cifre (`.figs`): usato e libero; a destra, in alto, le unità con la spiegazione a richiesta.
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 44.0;
            let fig = |ui: &mut Ui, value: String, value_color: egui::Color32, label: String| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.add(egui::Label::new(widgets::rich(value, Type::Display, value_color)).extend());
                    widgets::typed(ui, label, Type::Body, p.text_2);
                });
            };
            fig(ui, f.size(b.used), p.text, l.ta("bilancio-usati-su", &[("totale", f.size(b.capacity).into())]));
            fig(ui, f.size(free), p.text_2, l.t("bilancio-liberi"));
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                widgets::info(ui, p, &l.t("impostazioni-unita-nota"));
                ui.add_space(0.0);
                ui.allocate_ui_with_layout(vec2(ui.available_width().min(360.0), tokens::focus::MIN_TARGET), Layout::right_to_left(Align::Center), |ui| {
                    widgets::paragraph(ui, &units, Type::Callout, p.text_2);
                });
            });
        });
        ui.add_space(22.0);
        let n = b.unattributed.max(0) as u64;
        // L'allocato letto diviso per gestore: le ipotesi restano nel grigio.
        let g = d.cls.by_manager;
        let segs = [
            (Seg::Data(Gestore::App), Gestore::App.key(), g.programs),
            (Seg::Data(Gestore::Mine), Gestore::Mine.key(), g.you),
            (Seg::Data(Gestore::Windows), Gestore::Windows.key(), g.windows),
            (Seg::Data(Gestore::Unknown), "bilancio-non-classificato-ipotesi", g.unclassified),
            // Metadati e spazio riservato: stesso colore, voci separate come nella card dei conti.
            (Seg::Meta, "bilancio-metadati", b.metadata.total() - b.metadata.reserved.unwrap_or(0)),
            (Seg::Meta, "bilancio-riservato", b.metadata.reserved.unwrap_or(0)),
            (Seg::Hatch, "bilancio-non-attribuito", n),
            (Seg::Free, "bilancio-libero", free),
        ];
        let total = b.capacity.max(1) as f32;
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 14.0), Sense::hover());
        let visible: Vec<_> = segs.iter().filter(|s| s.2 > 0).collect();
        let gaps = 2.0 * (visible.len().saturating_sub(1)) as f32;
        let mut x = rect.min.x;
        for (k, (seg, key, bytes)) in visible.iter().enumerate() {
            let w = ((*bytes as f32 / total) * (rect.width() - gaps)).max(2.0);
            let r = Rect::from_min_size(pos2(x, rect.min.y), vec2(w, rect.height()));
            let end = tokens::radius::BAR;
            let radius = CornerRadius {
                nw: if k == 0 { end } else { 0 },
                sw: if k == 0 { end } else { 0 },
                ne: if k + 1 == visible.len() { end } else { 0 },
                se: if k + 1 == visible.len() { end } else { 0 },
            };
            match seg {
                Seg::Data(g) => {
                    ui.painter().rect_filled(r, radius, p.data_color(*g));
                }
                Seg::Meta => {
                    ui.painter().rect_filled(r, radius, p.meta);
                }
                Seg::Hatch => {
                    widgets::unattributed(ui.painter(), r, p);
                }
                Seg::Free => {
                    ui.painter().rect_filled(r, radius, p.track);
                }
            }
            let tip = format!("{}: {} · {}", l.t(key), f.size(*bytes), l.ta("bilancio-quota-volume", &[("quota", f.percent(*bytes as f64 / total as f64).into())]));
            let id = ui.id().with(("segmento", k));
            ui.interact(r, id, Sense::hover()).on_hover_text(tip);
            x += w + 2.0;
        }
        ui.add_space(20.0);
        // Legenda in quattro colonne (`.legend`): forma, nome e, sotto, il valore. Il nome va a capo nella
        // sua colonna invece di troncarsi (testi tradotti più lunghi).
        let cols = if ui.available_width() >= 640.0 { 4 } else { 2 };
        let col_gap = 28.0;
        let col_w = ((ui.available_width() - col_gap * (cols - 1) as f32) / cols as f32).floor();
        for row in segs.chunks(cols) {
            let items: Vec<_> = row
                .iter()
                .map(|&(seg, key, bytes)| {
                    let mut v = f.size(bytes);
                    if seg == Seg::Hatch && b.drift != 0 {
                        v = l.ta("bilancio-deriva", &[("valore", v.into()), ("deriva", f.size(b.drift.unsigned_abs()).into())]);
                    }
                    let lg = widgets::galley(ui.painter(), l.t(key), Type::Body, p.text, Some(col_w - 20.0));
                    let vg = widgets::galley(ui.painter(), v, Type::Body, p.text_2, Some(col_w - 20.0));
                    (seg, lg, vg)
                })
                .collect();
            let h = items.iter().map(|(_, lg, vg)| lg.size().y + 2.0 + vg.size().y).fold(0.0, f32::max);
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
            for (k, (seg, lg, vg)) in items.into_iter().enumerate() {
                let x = rect.min.x + k as f32 * (col_w + col_gap);
                let first = lg.rows.first().map_or(lg.size().y, |r| r.height());
                let c = pos2(x + 5.0, rect.min.y + first / 2.0);
                let (shape, color) = match seg {
                    Seg::Data(g) => (g.shape(), p.data_color(g)),
                    Seg::Meta => (Glyph::Bar, p.meta),
                    Seg::Hatch => (Glyph::Hatch, p.hatch),
                    Seg::Free => (Glyph::Square, p.text_3),
                };
                widgets::glyph(ui.painter(), c, 10.0, shape, color, p);
                // Nome e valore insieme per AccessKit: la legenda si legge anche senza vederla.
                let name = format!("{}: {}", lg.text(), vg.text());
                let item = Rect::from_min_size(pos2(x, rect.min.y), vec2(col_w, rect.height()));
                ui.interact(item, ui.id().with(("legenda", row[0].1, k)), Sense::hover()).widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
                let lh = lg.size().y;
                ui.painter().galley(pos2(x + 20.0, rect.min.y), lg, p.text);
                ui.painter().galley(pos2(x + 20.0, rect.min.y + lh + 2.0), vg, p.text_2);
            }
            ui.add_space(12.0);
        }
        if b.drift != 0 || b.quota.is_some() {
            ui.add_space(4.0);
        }
        if b.drift != 0 {
            widgets::paragraph(ui, l.ta("bilancio-nota-deriva", &[("deriva", f.size(b.drift.unsigned_abs()).into())]), Type::Callout, p.text_2);
        }
        if let Some(q) = b.quota {
            widgets::paragraph(ui, l.ta("bilancio-nota-quota", &[("quota", f.size(q).into())]), Type::Callout, p.text_2);
        }
    });
}

/// "Dove finisce lo spazio". Restituisce l'elemento da aprire in Esplora.
fn top_list(app: &App, ui: &mut Ui, p: &Palette) -> Option<u32> {
    let d = app.data.as_ref()?;
    let (l, f) = (&app.lingue, &app.fmt);
    let t = d.tree();
    let mut open = None;
    widgets::bare_card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::card_header(ui, p, &l.t("top-titolo"), Some(&l.t("top-sottotitolo")), None);
        egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 8, top: 0, bottom: 4 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            let max = d.top.first().map_or(1, |&n| t.alloc(n)).max(1);
            let row_min = tokens::metrics(ui.ctx()).row_two_line;
            let mut prev_hovered = false;
            for (k, &n) in d.top.iter().enumerate() {
                let w = ui.available_width();
                let size_g = ui.painter().layout_no_wrap(f.size(t.alloc(n)), widgets::num(13.0), p.text);
                let bar_w = (w * 0.12).clamp(48.0, 88.0);
                // Colonne (`.trow`): forma 12, testo, barretta, dimensione, freccia 14; 14 tra l'una e l'altra.
                let text_w = (w - 24.0 - 12.0 - 14.0 - bar_w - 14.0 - size_g.size().x - 14.0 - 14.0 - 14.0).max(80.0);
                let own = d.cls.own(n).map(|k| &d.cls.matches[k]);
                let gest = kbtext::node_gestore(&d.cls, n);
                // Riga di spiegazione: la regola che riconosce l'elemento, o dove sta e cosa contiene. Va a
                // capo invece di troncarsi (testi tradotti più lunghi).
                let parent = t.parent(n);
                let where_ = if parent == NONE { String::new() } else { t.path_lossy(parent) };
                let why = if let Some(m) = own {
                    kbtext::rule_line(l, f, &d.kb, m)
                } else if t.is_dir(n) {
                    let files = u64::from(t.files(n));
                    l.ta("top-spiegazione-cartella", &[("percorso", where_.into()), ("file", l.ta("conteggio-file", &[("n", files.into()), ("testo", f.count(files).into())]).into())])
                } else {
                    l.ta("top-spiegazione-file", &[("percorso", where_.into())])
                };
                let wg = widgets::galley(ui.painter(), why.clone(), Type::Callout, p.text_2, Some(text_w));
                // Il Cestino si chiama Cestino, non con il SID della sua cartella.
                let name = match own {
                    _ if n == Tree::ROOT => d.volume.letter(),
                    Some(m) if d.kb.rules[m.rule].id == "cestino" => kbtext::rule_name(l, &d.kb, m.rule),
                    _ => t.name_lossy(n),
                };
                let ng = widgets::elided(ui.painter(), name.clone(), widgets::font(Type::BodyStrong), p.text, text_w - 40.0);
                let h = (16.0 + ng.size().y + 2.0 + wg.size().y).max(row_min);
                let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
                let painter = ui.painter();
                let hovered = resp.hovered();
                if hovered {
                    painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.hover);
                }
                // Filetto tra le righe, rientrato dopo la forma; sparisce accanto alla riga sotto il puntatore.
                if k > 0 && !hovered && !prev_hovered {
                    painter.hline((rect.min.x + 38.0)..=(rect.max.x - 12.0), rect.min.y + 0.5, Stroke::new(1.0, p.separator));
                }
                prev_hovered = hovered;
                let cy = rect.center().y;
                let block_h = ng.size().y + 2.0 + wg.size().y;
                let top = rect.center().y - block_h / 2.0;
                let x = rect.min.x + 12.0;
                widgets::manager_glyph(painter, pos2(x + 6.0, top + ng.size().y / 2.0), gest, p);
                let text_x = x + 12.0 + 14.0;
                let nw = ng.size().x;
                let nh = ng.size().y;
                painter.galley(pos2(text_x, top), ng, p.text);
                let ev = own.map_or(Evidence::Measured, |m| kbtext::evidence(m.attribution));
                widgets::evidence_glyph(painter, pos2(text_x + nw + 10.0, top + nh / 2.0), ev, p);
                if own.is_some_and(kbtext::is_hypothesis) {
                    painter.text(pos2(text_x + nw + 20.0, top + nh / 2.0), Align2::LEFT_CENTER, l.t("etichetta-ipotesi"), widgets::font(Type::Caption), p.text_2);
                }
                painter.galley(pos2(text_x, top + nh + 2.0), wg, p.text_2);
                let right = rect.max.x - 12.0;
                icons::paint(painter, Rect::from_center_size(pos2(right - 7.0, cy), vec2(14.0, 14.0)), Icon::ChevronRight, p.text_2);
                let size_right = right - 14.0 - 14.0;
                let sw = size_g.size().x;
                painter.galley(pos2(size_right - sw, cy - size_g.size().y / 2.0), size_g, p.text);
                let bar = Rect::from_min_size(pos2(size_right - sw - 14.0 - bar_w, cy - 2.0), vec2(bar_w, 4.0));
                widgets::share_bar(painter, bar, t.alloc(n) as f64 / max as f64, p.data_color(gest), p.track);
                if resp.has_focus() {
                    widgets::focus_ring_inside(ui.painter(), rect, p, tokens::radius::ROW);
                }
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
                // La spiegazione e la dimensione nella descrizione: il nome resta quello visibile.
                let desc = format!("{why} · {}", f.size(t.alloc(n)));
                ui.ctx().accesskit_node_builder(resp.id, |node| node.set_description(desc));
                let quota = f.percent(t.alloc(n) as f64 / d.used() as f64);
                let resp = resp.on_hover_text(l.ta("top-suggerimento", &[("quota", quota.into())]));
                if resp.clicked() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    open = Some(n);
                }
            }
            if d.top.is_empty() {
                widgets::paragraph(ui, l.t("top-vuoto"), Type::Body, p.text_2);
            }
        });
        // Legenda della provenienza (`.srcl`).
        egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 20, top: 6, bottom: 16 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                widgets::typed(ui, l.t("top-legenda-fonte"), Type::Callout, p.text_2);
                ui.add_space(6.0);
                for e in [Evidence::Measured, Evidence::Source, Evidence::Inferred, Evidence::Unavailable] {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 16.0), Sense::hover());
                    widgets::evidence_glyph(ui.painter(), r.center(), e, p);
                    widgets::typed(ui, l.t(e.key()), Type::Callout, p.text_2);
                    ui.add_space(10.0);
                }
            });
        });
    });
    open
}

/// Cosa chiede la parte "Ripeti come amministratore" della card dei conti.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HelperAction {
    Start,
    Cancel,
}

/// "Perché i conti non tornano del tutto", come riga apribile su tutta la larghezza (`.why`): chiusa
/// mostra il non attribuito; aperta, a sinistra le righe dei conti, a destra i file con più nomi, le liste
/// "Osservato" e "Possibile" e "Ripeti come amministratore". Dopo l'helper "Osservato" mostra i
/// valori misurati e "Possibile" perde le voci misurate. Restituisce il pulsante premuto.
fn why_card(app: &App, ui: &mut Ui, p: &Palette, open: &mut bool) -> Option<HelperAction> {
    let d = app.data.as_ref()?;
    let (l, f) = (&app.lingue, &app.fmt);
    let mut action = None;
    widgets::bare_card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let title = l.t("conti-titolo");
        let value = d.balance.map(|b| if b.unattributed >= 0 { f.size(b.unattributed as u64) } else { format!("-{}", f.size(b.unattributed.unsigned_abs())) });
        // La riga (`.why .dis`): freccia, titolo; a destra "Non attribuito" e il valore.
        let w = ui.available_width();
        let label = l.t("conti-non-attribuito");
        let vg = value.as_ref().map(|v| ui.painter().layout_no_wrap(v.clone(), widgets::font(Type::Headline), p.text));
        let lg = ui.painter().layout_no_wrap(label.clone(), widgets::font(Type::Body), p.text_2);
        let right_w = vg.as_ref().map_or(0.0, |g| g.size().x + 10.0 + lg.size().x + 20.0);
        let tg = widgets::galley(ui.painter(), title.clone(), Type::Headline, p.text, Some(w - 44.0 - right_w - 16.0));
        let h = (tg.size().y + 32.0).max(56.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
        if resp.clicked() {
            *open = !*open;
        }
        if resp.hovered() {
            ui.painter().rect_filled(rect, CornerRadius::same(tokens::radius::CARD), p.hover);
        }
        let cy = rect.center().y;
        let icon = if *open { Icon::ChevronDown } else { Icon::ChevronRight };
        icons::paint(ui.painter(), Rect::from_center_size(pos2(rect.min.x + 28.0, cy), vec2(16.0, 16.0)), icon, p.text_2);
        ui.painter().galley(pos2(rect.min.x + 44.0, cy - tg.size().y / 2.0), tg, p.text);
        if let Some(g) = vg {
            let gw = g.size().x;
            ui.painter().galley(pos2(rect.max.x - 20.0 - gw, cy - g.size().y / 2.0), g, p.text);
            let lw = lg.size().x;
            ui.painter().galley(pos2(rect.max.x - 20.0 - gw - 10.0 - lw, cy - lg.size().y / 2.0), lg, p.text_2);
        }
        if resp.has_focus() {
            widgets::focus_ring_inside(ui.painter(), rect, p, tokens::radius::CARD);
        }
        let state = *open;
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::CollapsingHeader, true, &title));
        ui.ctx().accesskit_node_builder(resp.id, |n| n.set_expanded(state));
        if !*open {
            return;
        }
        egui::Frame::new().inner_margin(egui::Margin { left: 44, right: 20, top: 4, bottom: 22 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let Some(b) = d.balance else {
                widgets::paragraph(ui, l.t("bilancio-non-disponibile"), Type::Body, p.text_2);
                return;
            };
            let w = ui.available_width();
            let gap = 28.0;
            let two = w >= 720.0;
            let col = if two { ((w - gap) / 2.0).floor() } else { w };
            let ledger_col = |ui: &mut Ui| {
                ui.set_width(col);
                ledger_rows(ui, p, l, f, &b);
            };
            let mut notes_col = |ui: &mut Ui| {
                ui.set_width(col);
                ui.spacing_mut().item_spacing.y = 4.0;
                if d.links.extra_names > 0 {
                    let s = l.ta("conti-hard-link", &[("n", d.links.extra_names.into()), ("testo", f.count(d.links.extra_names).into()), ("byte", f.size(d.links.extra_alloc).into())]);
                    widgets::paragraph(ui, s, Type::Callout, p.text_2);
                    ui.add_space(12.0);
                }
                widgets::typed(ui, l.t("conti-osservato"), Type::BodyStrong, p.text);
                for s in observed_items(d, l, f) {
                    bullet(ui, p, &s);
                }
                ui.add_space(12.0);
                widgets::typed(ui, l.t("conti-possibile"), Type::BodyStrong, p.text);
                for s in possible_items(d, b.metadata.reserved.is_some(), l) {
                    bullet(ui, p, &s);
                }
                action = helper_block(app, d, ui, p);
            };
            if two {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    ui.allocate_ui_with_layout(vec2(col, 10.0), Layout::top_down(Align::Min), |ui| ledger_col(ui));
                    ui.allocate_ui_with_layout(vec2(col, 10.0), Layout::top_down(Align::Min), |ui| notes_col(ui));
                });
            } else {
                ledger_col(ui);
                ui.add_space(16.0);
                notes_col(ui);
            }
        });
    });
    action
}

/// Righe dei conti (`.ledger`): usato secondo Windows, file letti, metadati e spazio riservato rientrati, il
/// non attribuito in fondo con il tratteggio. L'etichetta va a capo invece di troncarsi.
fn ledger_rows(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, b: &quado_model::balance::Balance) {
    ui.spacing_mut().item_spacing.y = 0.0;
    let line = |ui: &mut Ui, label: String, value: String, total: bool, indent: bool| {
        let w = ui.available_width();
        let t = if total { Type::BodyStrong } else { Type::Body };
        let color = if indent { p.text_2 } else { p.text };
        let vg = widgets::galley(ui.painter(), value, t, p.text, None);
        let vw = vg.size().x;
        let x0 = if indent { 16.0 } else if total { 18.0 } else { 0.0 };
        let lg = widgets::galley(ui.painter(), label, t, color, Some((w - vw - 16.0 - x0).max(40.0)));
        let h = lg.size().y + 18.0;
        let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let name = format!("{}: {}", lg.text(), vg.text());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
        let first = lg.rows.first().map_or(lg.size().y, |r| r.height());
        ui.painter().galley(pos2(rect.max.x - vw, rect.min.y + 9.0 + (first - vg.size().y) / 2.0), vg, p.text);
        if total {
            widgets::glyph(ui.painter(), pos2(rect.min.x + 5.0, rect.min.y + 9.0 + first / 2.0), 10.0, Glyph::Hatch, p.hatch, p);
        }
        ui.painter().galley(pos2(rect.min.x + x0, rect.min.y + 9.0), lg, color);
        if !total {
            ui.painter().hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, p.separator));
        }
    };
    line(ui, l.t("conti-usato-windows"), f.size(b.used), false, false);
    line(ui, l.t("conti-file-letti"), f.size(b.allocated), false, false);
    let m = b.metadata;
    let detail = l.ta(
        "conti-metadati",
        &[
            ("mft", f.size(m.mft.unwrap_or(0)).into()),
            ("indici", f.size(m.dir_indexes).into()),
            ("journal", f.size(m.usn_journal.unwrap_or(0)).into()),
        ],
    );
    // Lo spazio riservato da Windows ha una riga propria tra i metadati misurati.
    line(ui, detail, f.size(m.total() - m.reserved.unwrap_or(0)), false, true);
    if let Some(r) = m.reserved {
        line(ui, l.t("conti-riservato"), f.size(r), false, true);
    }
    let n = if b.unattributed >= 0 { f.size(b.unattributed as u64) } else { format!("-{}", f.size(b.unattributed.unsigned_abs())) };
    line(ui, l.t("conti-non-attribuito"), n, true, false);
}

/// Voci di "Osservato": cartelle ancora non leggibili, Cestini e profili altrui non letti, poi le misure
/// dell'helper (cartelle lette come amministratore, Cestini per SID, profili come totale senza nomi, copie
/// shadow). Solo valori misurati, nessuna causa.
fn observed_items(d: &Loaded, l: &Lingue, f: &Formatter) -> Vec<String> {
    let o = &d.observed;
    let t = d.tree();
    let mut items: Vec<String> = Vec::new();
    if !o.unreadable.is_empty() {
        let examples: Vec<String> = o.unreadable.iter().take(2).map(|&(n, _)| t.name_lossy(n)).collect();
        items.push(l.ta(
            "conti-non-leggibili",
            &[("n", o.unreadable.len().into()), ("testo", f.count(o.unreadable.len() as u64).into()), ("esempi", examples.join(", ").into())],
        ));
    }
    if o.other_bins > 0 {
        items.push(l.ta("conti-cestini-altri", &[("n", o.other_bins.into()), ("testo", f.count(o.other_bins as u64).into())]));
    }
    if o.other_profiles > 0 {
        items.push(l.ta("conti-profili-altri", &[("n", o.other_profiles.into()), ("testo", f.count(o.other_profiles as u64).into())]));
    }
    // Misure dell'helper: solo i nodi ancora nel modello (una riscansione successiva li stacca).
    let e = &d.elevated;
    let alive = |n: u32| (n as usize) < t.len() && !t.flags(n).has(Flags::DETACHED);
    let measured = |n: u32| alive(n) && t.flags(n).has(Flags::MEASURED);
    let folders: Vec<u32> = e.folders.iter().copied().filter(|&n| alive(n)).collect();
    if !folders.is_empty() {
        let bytes: u64 = folders.iter().map(|&n| t.alloc(n)).sum();
        items.push(l.ta(
            "conti-lette-amministratore",
            &[("n", folders.len().into()), ("testo", f.count(folders.len() as u64).into()), ("dimensione", f.size(bytes).into())],
        ));
    }
    for (n, sid, orphan) in e.bins.iter().filter(|b| measured(b.0)) {
        let key = if *orphan { "conti-cestino-orfano" } else { "conti-cestino-altro" };
        items.push(l.ta(key, &[("sid", sid.as_str().into()), ("dimensione", f.size(t.alloc(*n)).into())]));
    }
    let profiles: Vec<u32> = e.profiles.iter().copied().filter(|&n| measured(n)).collect();
    if !profiles.is_empty() {
        let bytes: u64 = profiles.iter().map(|&n| t.alloc(n)).sum();
        let files: u64 = profiles.iter().map(|&n| u64::from(t.files(n))).sum();
        let file = l.ta("conteggio-file", &[("n", files.into()), ("testo", f.count(files).into())]);
        items.push(l.ta(
            "conti-profili-misurati",
            &[("n", profiles.len().into()), ("testo", f.count(profiles.len() as u64).into()), ("dimensione", f.size(bytes).into()), ("file", file.into())],
        ));
    }
    // Copie shadow: spiegano una parte di System Volume Information e non si sommano ai conti.
    match e.shadow {
        // `Win32_ShadowStorage` scrive il massimo "senza limite" come il valore più grande.
        Some(Ok(Some(s))) if s.max == u64::MAX => {
            items.push(l.ta("conti-copie-shadow-senza-limite", &[("usato", f.size(s.used).into()), ("allocato", f.size(s.allocated).into())]));
        }
        Some(Ok(Some(s))) => {
            items.push(l.ta("conti-copie-shadow-misurate", &[("usato", f.size(s.used).into()), ("allocato", f.size(s.allocated).into()), ("massimo", f.size(s.max).into())]));
        }
        Some(Ok(None)) => items.push(l.t("conti-copie-shadow-nessuna")),
        Some(Err(SERVICE_DISABLED)) => items.push(l.t("conti-copie-shadow-servizio-spento")),
        _ => {}
    }
    if items.is_empty() {
        items.push(l.t("conti-osservato-nulla"));
    }
    items
}

/// `ERROR_SERVICE_DISABLED` dalla lettura delle copie shadow: il servizio VSS è disattivato.
const SERVICE_DISABLED: u32 = 1058;

/// Voci di "Possibile, misurabile solo come amministratore", senza quelle ormai misurate: le copie shadow
/// dopo l'helper, l'archiviazione riservata quando `GetDiskSpaceInformationW` l'ha letta.
fn possible_items(d: &Loaded, reserved_measured: bool, l: &Lingue) -> Vec<String> {
    let mut items = Vec::new();
    match d.elevated.shadow {
        Some(Ok(_)) | Some(Err(SERVICE_DISABLED)) => {}
        Some(Err(code)) => items.push(l.ta("conti-copie-shadow-errore", &[("codice", helper_jobs::code_text(code).into())])),
        None => items.push(l.t("conti-copie-shadow")),
    }
    if !reserved_measured {
        items.push(l.t("conti-archiviazione-riservata"));
    }
    items.push(l.t("conti-altri-metafile"));
    items
}

/// "Ripeti come amministratore": con cartelle negate da chiedere, cosa succede e il pulsante;
/// durante il lavoro la fase e "Annulla"; dopo, l'esito. Restituisce il pulsante premuto.
fn helper_block(app: &App, d: &Loaded, ui: &mut Ui, p: &Palette) -> Option<HelperAction> {
    let (l, f) = (&app.lingue, &app.fmt);
    let phase = app.helper.phase();
    let outcome = app.helper.outcome_for(d);
    let request = if phase.is_none() { helper_jobs::build_request(d) } else { None };
    if phase.is_none() && outcome.is_none() && request.is_none() {
        return None;
    }
    let mut action = None;
    ui.add_space(14.0);
    if let Some(ph) = phase {
        let s = match ph {
            Phase::Consent => l.t("helper-attesa"),
            Phase::Reading { done, total } => {
                l.ta("helper-lettura", &[("n", total.into()), ("fatte", f.count(done as u64).into()), ("testo", f.count(total as u64).into())])
            }
        };
        widgets::paragraph(ui, s, Type::Callout, p.text);
        ui.add_space(8.0);
        if widgets::button(ui, p, &l.t("helper-annulla"), None, ButtonKind::Secondary, true).clicked() {
            action = Some(HelperAction::Cancel);
        }
        return action;
    }
    if let Some(o) = outcome {
        for s in outcome_lines(l, f, o) {
            widgets::paragraph(ui, s, Type::Callout, p.text);
        }
        ui.add_space(8.0);
    }
    if let Some(r) = &request {
        let n = r.nodes.len();
        widgets::paragraph(ui, l.ta("helper-spiegazione", &[("n", n.into()), ("testo", f.count(n as u64).into())]), Type::Callout, p.text_2);
        ui.add_space(10.0);
        if widgets::button(ui, p, &l.t("helper-ripeti"), Some(Icon::Shield), ButtonKind::Secondary, true).clicked() {
            action = Some(HelperAction::Start);
        }
    }
    action
}

/// Righe dell'esito dell'ultimo lavoro dell'helper.
fn outcome_lines(l: &Lingue, f: &Formatter, o: &Outcome) -> Vec<String> {
    match o {
        Outcome::Declined => vec![l.t("helper-rifiutato")],
        Outcome::Cancelled => vec![l.t("helper-annullato")],
        // Riprovare non serve: il testo dice dove spostare l'eseguibile.
        Outcome::Failed(e) if e.is_location() => vec![l.t("helper-posizione")],
        Outcome::Failed(e) => vec![l.ta("helper-errore", &[("motivo", failure_text(l, f, *e).into())])],
        Outcome::Finished(s) => {
            let mut v = vec![l.ta("helper-fine", &[("n", s.asked.into()), ("lette", f.count(s.applied as u64).into()), ("testo", f.count(s.asked as u64).into())])];
            if s.backup_privilege == Some(false) {
                v.push(l.t("helper-senza-privilegio"));
            }
            if !s.failed.is_empty() {
                let mut codes: Vec<u32> = s.failed.iter().map(|x| x.1).collect();
                codes.sort_unstable();
                codes.dedup();
                let codici = codes.iter().map(|&c| helper_jobs::code_text(c)).collect::<Vec<_>>().join(", ");
                v.push(l.ta("helper-fine-non-lette", &[("n", s.failed.len().into()), ("testo", f.count(s.failed.len() as u64).into()), ("codici", codici.into())]));
            }
            if s.ignored > 0 {
                v.push(l.ta("helper-fine-ignorate", &[("n", s.ignored.into()), ("testo", f.count(s.ignored as u64).into())]));
            }
            if s.code != 0 {
                v.push(l.ta("helper-fine-codice", &[("codice", helper_jobs::code_text(s.code).into())]));
            }
            v
        }
    }
}

/// Il motivo di un errore dell'helper, da inserire in `helper-errore`.
fn failure_text(l: &Lingue, f: &Formatter, e: HelperFailure) -> String {
    let code = |c: u32| l.ta("helper-motivo-codice", &[("codice", helper_jobs::code_text(c).into())]);
    match e {
        HelperFailure::Launch(c) | HelperFailure::Pipe(PipeError::Win(c)) => code(c),
        // Non arrivano qui (sono esiti propri), ma hanno un codice di Windows: ERROR_CANCELLED.
        HelperFailure::Declined | HelperFailure::Pipe(PipeError::Cancelled) => code(1223),
        HelperFailure::Exited(c) => l.ta("helper-motivo-uscita", &[("codice", helper_jobs::code_text(c).into())]),
        HelperFailure::Pipe(PipeError::Timeout) => l.ta("helper-motivo-tempo", &[("secondi", f.count(helper_jobs::ACCEPT_TIMEOUT.as_secs()).into())]),
        HelperFailure::Pipe(PipeError::WrongPeer(_)) => l.t("helper-motivo-processo"),
        HelperFailure::Pipe(PipeError::Proto(_)) => l.t("helper-motivo-protocollo"),
        HelperFailure::Pipe(PipeError::Closed) => l.t("helper-motivo-chiuso"),
        // Non arriva qui: ha il suo esito, `helper-posizione`.
        HelperFailure::Location => l.t("helper-posizione"),
    }
}

/// Dove porta un clic su una voce della card.
enum Open {
    /// L'elemento in Esplora.
    Node(u32),
    /// La voce della vista Download (indice in `LensView::findings`).
    Lens(usize),
    /// «Sposta nel Cestino…» per la corrispondenza: simulazione e dialogo di conferma.
    Recycle(usize),
}

/// Il pulsante dell'azione successiva di una voce: testo e dove porta.
struct Next {
    key: &'static str,
    open: Open,
}

/// Una voce della card: da una regola o dalla vista Download.
struct CardItem<'a> {
    title: String,
    state: &'a State,
    /// Il record sposta nel Cestino: lo spazio si libera svuotando il Cestino, non dopo una procedura.
    recycle: bool,
    estimate: Estimate,
    bytes: u64,
    open: Option<Open>,
    next: Option<Next>,
}

/// Quanto si libera, detto nella card sotto lo stato: per uno spostamento nel Cestino la stima vale dopo lo
/// svuotamento; per una procedura o un comando lo misura Quado dopo. La cifra grande della voce resta quanto
/// occupa l'elemento, con l'etichetta «Occupa».
pub(crate) fn card_estimate(l: &Lingue, f: &Formatter, recycle: bool, e: &Estimate) -> String {
    if !recycle {
        return l.t("interventi-libera-procedura");
    }
    match e {
        Estimate::Exact(b) => l.ta("interventi-libera-cestino", &[("valore", f.size(*b).into())]),
        Estimate::Range { min, max } => l.ta("interventi-libera-cestino-intervallo", &[("minimo", f.size(*min).into()), ("massimo", f.size(*max).into())]),
        Estimate::NotEstimable(NotEstimable::NoAction) => l.t("stima-nessuna-azione"),
        Estimate::NotEstimable(_) => l.t("interventi-libera-cestino-anteprima"),
    }
}

/// "Interventi possibili": una voce per regola, solo da record di raccomandazione con un'azione; mai Non
/// sappiamo, mai "Nessuna procedura", mai le cartelle orfane del Cestino, mai le regole fuori dalla card
/// (ibernazione, WinSxS, cache dei browser). Le disinstallazioni stanno nella vista Programmi. Con le voci
/// della vista Download che hanno un'azione, una per funzione: copie identiche dopo il confronto e download
/// incompleti non in uso (mai le copie non confrontate). Ogni voce ha il suo pulsante successivo: «Sposta
/// nel Cestino…» per uno spostamento di un solo elemento riconosciuto che non è Da verificare, la vista
/// Download per le sue voci, altrimenti l'elemento in Esplora, dove l'ispettore mostra ipotesi e rischi sopra
/// il pulsante. Restituisce dove portare il clic.
fn interventions_card(app: &App, ui: &mut Ui, p: &Palette) -> Option<Open> {
    let d = app.data.as_ref()?;
    let (l, f) = (&app.lingue, &app.fmt);
    let rules = quado_kb::engine::interventions(&d.kb, &d.cls, d.tree());
    let mut items: Vec<CardItem<'_>> = rules
        .iter()
        .map(|i| {
            let m = &d.cls.matches[i.matches[0]];
            let title = if i.matches.len() == 1 {
                kbtext::rule_line(l, f, &d.kb, m)
            } else {
                l.ta("interventi-gruppo", &[("nome", kbtext::rule_name(l, &d.kb, i.rule).into()), ("n", i.matches.len().into()), ("testo", f.count(i.matches.len() as u64).into())])
            };
            let node = m.nodes.first().copied();
            // Da verificare: niente spostamento diretto dalla card, prima l'ispettore con ipotesi e rischi.
            let direct = i.matches.len() == 1 && m.record.action == Action::Recycle && !matches!(m.record.state, State::ToVerify(_));
            let next = if direct {
                Some(Next { key: "azione-prepara-cestino", open: Open::Recycle(i.matches[0]) })
            } else {
                node.map(|n| Next { key: "interventi-cosa-puoi-fare", open: Open::Node(n) })
            };
            CardItem { title, state: &m.record.state, recycle: m.record.action == Action::Recycle, estimate: i.estimate, bytes: i.bytes, open: node.map(Open::Node), next }
        })
        .collect();
    // Voci mostrate: tutte quelle della lente (al più una per funzione: non stanno nell'ispettore degli
    // elementi, quindi non possono finire tra "le altre voci"), poi le regole per byte fino a `SHOWN`.
    const SHOWN: usize = 6;
    let lens_items = app.downloads.current(d).map(downloads_view::interventions).unwrap_or_default();
    let from_lens = !lens_items.is_empty();
    let hidden = items.len().saturating_sub(SHOWN.saturating_sub(lens_items.len()));
    items.truncate(items.len() - hidden);
    for (func, voci, bytes, estimate, record) in lens_items {
        let name = l.t(&quado_kb::lens::name_key(func));
        let title = if voci.len() == 1 {
            name
        } else {
            l.ta("interventi-gruppo", &[("nome", name.into()), ("n", voci.len().into()), ("testo", f.count(voci.len() as u64).into())])
        };
        let first = voci.first().copied();
        let next = first.map(|k| Next { key: "download-apri-lente", open: Open::Lens(k) });
        items.push(CardItem { title, state: &record.state, recycle: record.action == Action::Recycle, estimate, bytes, open: first.map(Open::Lens), next });
    }
    items.sort_by_key(|i| std::cmp::Reverse(i.bytes));
    let mut open = None;
    widgets::bare_card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        // La spiegazione della card sta nella "i": si legge al passaggio del mouse o con il focus.
        let subtitle = l.t(if from_lens { "interventi-sottotitolo-lente" } else { "interventi-sottotitolo" });
        widgets::card_header(ui, p, &l.t("interventi-titolo"), None, Some(&subtitle));
        egui::Frame::new().inner_margin(egui::Margin { left: 8, right: 8, top: 0, bottom: 4 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut prev_hovered = false;
            for (k, i) in items.iter_mut().enumerate() {
                let state = i.state;
                // Da verificare è l'unico stato con un tono proprio.
                let tone = if matches!(state, State::ToVerify(_)) { p.warn } else { p.text_2 };
                let w = ui.available_width();
                let painter = ui.painter().clone();
                // La cifra grande è quanto occupa l'elemento, con l'etichetta davanti: non quanto si libera.
                let sg = painter.layout_no_wrap(f.size(i.bytes), widgets::num_medium(15.0), p.text);
                let lg = widgets::galley(&painter, l.t("interventi-occupa"), Type::Callout, p.text_2, None);
                let tw = (w - 14.0 - 12.0 - sg.size().x - 6.0 - lg.size().x - 16.0).max(80.0);
                let tg = widgets::galley(&painter, i.title.clone(), Type::BodyStrong, p.text, Some(tw));
                // Stato e stima (`.meta`): icona e parola nel suo tono, poi "·" e la stima; la stima va sulla
                // riga dopo se accanto non ha posto.
                let stg = widgets::galley(&painter, kbtext::state_label(l, state), Type::Callout, tone, None);
                let beside = tw - 21.0 - stg.size().x - 20.0;
                let est_w = if beside >= 160.0 { beside } else { tw - 21.0 };
                let eg = widgets::galley(&painter, card_estimate(l, f, i.recycle, &i.estimate), Type::Callout, p.text_2, Some(est_w));
                let meta_h = if beside >= 160.0 { stg.size().y.max(eg.size().y) } else { stg.size().y + 2.0 + eg.size().y };
                let label = i.next.as_ref().map(|n| l.t(n.key));
                let bh = if label.is_some() { tokens::focus::MIN_TARGET + 10.0 } else { 0.0 };
                let h = 12.0 + tg.size().y + 5.0 + meta_h + bh + 12.0;
                let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
                let hovered = resp.hovered();
                if hovered {
                    painter.rect_filled(rect, CornerRadius::same(tokens::radius::ROW), p.hover);
                }
                if k > 0 && !hovered && !prev_hovered {
                    painter.hline((rect.min.x + 14.0)..=(rect.max.x - 12.0), rect.min.y + 0.5, Stroke::new(1.0, p.separator));
                }
                prev_hovered = hovered;
                let x = rect.min.x + 14.0;
                let mut y = rect.min.y + 12.0;
                let th = tg.size().y;
                painter.galley(pos2(x, y), tg, p.text);
                let first = th.min(22.0);
                let (sw_num, sh_num) = (sg.size().x, sg.size().y);
                painter.galley(pos2(rect.max.x - 12.0 - sw_num, y + (first - sh_num) / 2.0), sg, p.text);
                // L'etichetta sulla stessa linea di base della cifra.
                painter.galley(pos2(rect.max.x - 12.0 - sw_num - 6.0 - lg.size().x, y + (first - sh_num) / 2.0 + (sh_num - lg.size().y) - 1.0), lg, p.text_2);
                y += th + 5.0;
                let line = stg.size().y;
                icons::paint(&painter, Rect::from_center_size(pos2(x + 7.0, y + line / 2.0), vec2(14.0, 14.0)), kbtext::state_icon(state), tone);
                let sw = stg.size().x;
                painter.galley(pos2(x + 21.0, y), stg, tone);
                if beside >= 160.0 {
                    // Il punto tra stato e stima è un segno, non un testo.
                    painter.circle_filled(pos2(x + 21.0 + sw + 10.0, y + line / 2.0), 1.3, p.text_2);
                    painter.galley(pos2(x + 21.0 + sw + 20.0, y), eg, p.text_2);
                } else {
                    painter.galley(pos2(x + 21.0, y + line + 2.0), eg, p.text_2);
                }
                y += meta_h;
                if resp.has_focus() {
                    widgets::focus_ring_inside(ui.painter(), rect, p, tokens::radius::ROW);
                }
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &i.title));
                let desc = format!("{} · {} · {} {}", kbtext::state_label(l, state), card_estimate(l, f, i.recycle, &i.estimate), l.t("interventi-occupa"), f.size(i.bytes));
                ui.ctx().accesskit_node_builder(resp.id, |node| node.set_description(desc));
                if resp.clicked() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    open = i.open.take();
                }
                // Il pulsante successivo (`.arow .act`), sopra la riga: è il controllo più in alto e prende
                // il clic.
                if let Some(label) = label {
                    let b = Rect::from_min_size(pos2(x, y + 10.0), vec2(rect.max.x - 12.0 - x, tokens::focus::MIN_TARGET));
                    let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(b).layout(Layout::left_to_right(Align::Center)));
                    let r = bui.push_id(("azione_successiva", k), |ui| next_button(ui, p, &label, &i.title)).inner;
                    if r.clicked() {
                        open = i.next.take().map(|n| n.open);
                    }
                }
            }
            if items.is_empty() {
                egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                    widgets::paragraph(ui, l.t("interventi-vuoto"), Type::Body, p.text_2);
                });
            }
        });
        if hidden > 0 {
            egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 20, top: 6, bottom: 18 }).show(ui, |ui| {
                widgets::paragraph(ui, l.ta("interventi-altri", &[("n", hidden.into()), ("testo", f.count(hidden as u64).into())]), Type::Callout, p.text_2);
            });
        } else {
            ui.add_space(8.0);
        }
    });
    open
}

/// Pulsante dell'azione successiva (`.arow .act .btn`): fondo `field`, testo medio. Il nome accessibile è
/// il testo visibile; la voce a cui si riferisce è nella descrizione.
fn next_button(ui: &mut Ui, p: &Palette, label: &str, item: &str) -> egui::Response {
    let g = ui.painter().layout_no_wrap(label.to_owned(), widgets::medium(13.0), p.text);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 24.0, tokens::focus::MIN_TARGET), Sense::click());
    let fill = if resp.is_pointer_button_down_on() {
        p.select
    } else if resp.hovered() {
        p.hover
    } else {
        p.field
    };
    ui.painter().rect_filled(rect, CornerRadius::same(tokens::radius::CONTROL), fill);
    if resp.hovered() {
        ui.painter().rect_stroke(rect, CornerRadius::same(tokens::radius::CONTROL), Stroke::new(1.0, p.border), egui::StrokeKind::Inside);
    }
    ui.painter().galley(rect.center() - g.size() / 2.0, g, p.text);
    widgets::focus_ring(ui, &resp, p, tokens::radius::CONTROL);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_description(item.to_owned()));
    resp
}

/// Voce di un elenco: pallino e testo a capo (`.why ul`).
fn bullet(ui: &mut Ui, p: &Palette, s: &str) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(vec2(10.0, 18.0), Sense::hover());
        ui.painter().circle_filled(pos2(r.center().x, r.min.y + 9.0), 1.8, p.text_2);
        widgets::paragraph(ui, s, Type::Callout, p.text_2);
    });
}
