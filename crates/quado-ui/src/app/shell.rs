// SPDX-License-Identifier: GPL-3.0-or-later
//! Cornice dell'app nella variante A: barra laterale comprimibile e raggruppata, con il marchio in cima,
//! barra superiore (barra laterale, indietro e avanti, titolo o percorso, selettore dell'unità, «Scansiona
//! di nuovo», palette dei comandi) e barra di stato. Le zone si separano con il tono: barra laterale `sidebar`, resto `bg`, un
//! filetto solo sopra la barra di stato.
//!
//! I pezzi generici ([`nav_item`], [`drive_pill`], [`status_separator`]) servono anche alle schermate che
//! ne vogliono uno uguale; [`sidebar`], [`top_bar`] e [`status_bar`] li compongono con lo stato dell'app.

use egui::{Align, Align2, Color32, CornerRadius, Frame, Key, Layout, Margin, Rect, Response, Sense, Stroke, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};
use quado_model::Tree;

use super::data::Loaded;
use super::{App, Screen, explore, volume_title};
use crate::brand;
use crate::icons::{self, Icon};
use crate::tokens::{self, Palette, Type, focus, radius};
use crate::widgets::{self, ButtonKind, sans};

/// Larghezza della barra laterale aperta (176 + 64 nella proposta, a 100%).
pub const SIDEBAR_W: f32 = 240.0;
/// Larghezza della barra laterale con le sole icone.
pub const SIDEBAR_COLLAPSED_W: f32 = 60.0;
/// Altezza della barra superiore.
pub const TOOLBAR_H: f32 = 52.0;
/// Altezza della barra di stato.
pub const STATUS_H: f32 = 28.0;

/// La palette dei comandi (Ctrl+K, `palette`): la barra superiore mostra il suo pulsante.
pub const COMMAND_PALETTE: bool = true;

/// Voce della barra laterale (`.nav`): icona e nome, oppure la sola icona con il nome nel suggerimento
/// quando la barra è compressa. `badge` è un numero già scritto, a destra.
#[allow(clippy::too_many_arguments)]
pub fn nav_item(ui: &mut Ui, p: &Palette, icon: Icon, label: &str, shortcut: Option<&str>, badge: Option<&str>, selected: bool, enabled: bool, collapsed: bool) -> Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let w = if collapsed { 40.0 } else { ui.available_width() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, focus::MIN_TARGET), sense);
    let fill = if selected {
        p.select
    } else if enabled && resp.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(radius::ROW), fill);
    let color = if enabled { p.text } else { p.disabled };
    let icon_color = if !enabled {
        p.disabled
    } else if selected {
        p.text
    } else {
        p.text_2
    };
    let ix = if collapsed { rect.center().x } else { rect.min.x + 18.0 };
    icons::paint(ui.painter(), Rect::from_center_size(pos2(ix, rect.center().y), vec2(16.0, 16.0)), icon, icon_color);
    if !collapsed {
        let font = if selected { widgets::medium(13.0) } else { sans(13.0) };
        ui.painter().text(pos2(rect.min.x + 36.0, rect.center().y), Align2::LEFT_CENTER, label, font, color);
        if let Some(b) = badge {
            ui.painter().text(pos2(rect.max.x - 10.0, rect.center().y), Align2::RIGHT_CENTER, b, widgets::num(12.0), p.text_2);
        }
    }
    if resp.has_focus() {
        widgets::focus_ring_inside(ui.painter(), rect, p, radius::ROW);
    }
    let full = badge.map_or_else(|| label.to_owned(), |b| format!("{label} {b}"));
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, selected, &full));
    if collapsed || shortcut.is_some() { widgets::tip(resp, p, &full, shortcut) } else { resp }
}

/// Selettore dell'unità della barra superiore (`.drive-pill`): disco, nome, capacità, freccia.
pub fn drive_pill(ui: &mut Ui, p: &Palette, label: &str, capacity: Option<&str>) -> Response {
    let g1 = ui.painter().layout_no_wrap(label.to_owned(), sans(13.0), p.text);
    let g2 = capacity.map(|c| ui.painter().layout_no_wrap(c.to_owned(), widgets::num(13.0), p.text_2));
    let w = 10.0 + 16.0 + 8.0 + g1.size().x + g2.as_ref().map_or(0.0, |g| g.size().x + 8.0) + 8.0 + 14.0 + 10.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, focus::MIN_TARGET), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(radius::CONTROL), p.hover);
    }
    let cy = rect.center().y;
    let mut x = rect.min.x + 10.0;
    icons::paint(ui.painter(), Rect::from_min_size(pos2(x, cy - 8.0), vec2(16.0, 16.0)), Icon::Drive, p.text);
    x += 24.0;
    let w1 = g1.size().x;
    ui.painter().galley(pos2(x, cy - g1.size().y / 2.0), g1, p.text);
    x += w1 + 8.0;
    if let Some(g) = g2 {
        let w2 = g.size().x;
        ui.painter().galley(pos2(x, cy - g.size().y / 2.0), g, p.text_2);
        x += w2 + 8.0;
    }
    icons::paint(ui.painter(), Rect::from_min_size(pos2(x, cy - 7.0), vec2(14.0, 14.0)), Icon::Caret, p.text_2);
    widgets::focus_ring(ui, &resp, p, radius::CONTROL);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    resp
}

/// Spazio tra due voci della barra di stato.
pub fn status_separator(ui: &mut Ui) {
    ui.add_space(18.0);
}

// ------------------------------------------------------------------ barra laterale

/// Una voce di navigazione: schermata, icona, chiave del nome, scorciatoia.
type NavEntry = (Screen, Icon, &'static str, Option<&'static str>);

/// Voci di navigazione, in tre gruppi come nella proposta: quadro e dettaglio; viste dedicate; interventi
/// e regole.
const NAV: [&[NavEntry]; 3] = [
    &[
        (Screen::Overview, Icon::Overview, "nav-panoramica", Some("Ctrl+1")),
        (Screen::Explore, Icon::Explore, "nav-esplora", Some("Ctrl+2")),
        (Screen::BigFiles, Icon::BigFiles, "nav-file-grandi", Some("Ctrl+3")),
    ],
    &[
        (Screen::Downloads, Icon::Download, "nav-download", Some("Ctrl+5")),
        (Screen::Programs, Icon::Programs, "nav-programmi", Some("Ctrl+6")),
        (Screen::System, Icon::System, "nav-sistema", Some("Ctrl+4")),
    ],
    &[(Screen::Interventions, Icon::Actions, "nav-interventi", Some("Ctrl+7")), (Screen::Rules, Icon::Rules, "nav-regole", None)],
];

/// Schermata senza barra laterale (`.nosb` della proposta): l'elenco delle unità e la prima scansione, quando
/// non ci sono ancora dati da navigare.
pub fn standalone(app: &App) -> bool {
    app.screen == Screen::Volumes || (app.screen == Screen::Scanning && app.data.is_none())
}

/// La barra laterale è compressa (per scelta) e non aperta per la ricerca.
pub fn collapsed(app: &App) -> bool {
    app.settings.sidebar_collapsed && !app.sidebar_peek
}

pub fn sidebar(app: &mut App, ui: &mut Ui, p: &Palette) {
    let collapsed = collapsed(app);
    let width = if collapsed { SIDEBAR_COLLAPSED_W } else { SIDEBAR_W };
    let margin = Margin { left: 10, right: 10, top: 12, bottom: 10 };
    egui::Panel::left("barra-laterale").resizable(false).show_separator_line(false).exact_size(width).frame(Frame::new().fill(p.sidebar).inner_margin(margin)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.add_space(2.0);
        brand_row(ui, p, collapsed);
        ui.add_space(12.0);
        let has = app.data.is_some();
        // Ricerca sotto il marchio: con la barra compressa è un pulsante che apre la barra e la casella.
        if has {
            if collapsed {
                let l = app.lingue.t("ricerca-suggerimento");
                let k = app.lingue.t("ricerca-scorciatoia");
                if widgets::icon_button_kbd(ui, p, Icon::Search, &l, Some(&k), true).clicked() {
                    app.sidebar_peek = true;
                    app.search.focus_box = true;
                }
            } else {
                explore::search_box(app, ui, p);
                if !app.search.box_focused && app.sidebar_peek && app.search.text.trim().is_empty() && !app.search.focus_box {
                    app.sidebar_peek = false;
                }
            }
            ui.add_space(14.0);
        }
        for (k, group) in NAV.iter().enumerate() {
            if k > 0 {
                ui.add_space(14.0);
            }
            for &(screen, icon, key, shortcut) in group.iter() {
                let label = app.lingue.t(key);
                let on = app.screen == screen;
                let needs_data = screen != Screen::Rules;
                // Interventi: il numero di interventi registrati.
                let n = if screen == Screen::Interventions { app.procedures.history.count() as u64 } else { 0 };
                let badge = (n > 0).then(|| app.fmt.count(n));
                if nav_item(ui, p, icon, &label, shortcut, badge.as_deref(), on, has || !needs_data, collapsed).clicked() {
                    if screen == Screen::Rules {
                        app.rules_view.selected = None;
                    }
                    app.go(screen);
                }
            }
        }
        if let Some(d) = &app.data {
            if !collapsed {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    widgets::caption_header(ui, p, &app.lingue.t("nav-volumi"));
                });
                ui.add_space(6.0);
            } else {
                ui.add_space(14.0);
            }
            if drive_item(ui, p, app, d, collapsed).clicked() {
                app.screen = Screen::Volumes;
            }
        }
        // In fondo: la nota sulla sola lettura, poi Impostazioni.
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            let label = app.lingue.t("nav-impostazioni");
            if nav_item(ui, p, Icon::Settings, &label, Some("Ctrl+,"), None, app.screen == Screen::Settings, true, collapsed).clicked() {
                app.go(Screen::Settings);
            }
            ui.add_space(10.0);
            let note = app.lingue.t("nav-nota-sola-lettura");
            if collapsed {
                let (r, resp) = ui.allocate_exact_size(vec2(40.0, 24.0), Sense::hover());
                icons::paint(ui.painter(), Rect::from_center_size(r.center(), vec2(14.0, 14.0)), Icon::Lock, p.text_2);
                resp.on_hover_text(note);
            } else {
                // Lucchetto e nota a capo, appoggiati sopra Impostazioni.
                let w = ui.available_width();
                let g = widgets::galley(ui.painter(), note.clone(), Type::Callout, p.text_2, Some(w - 38.0));
                let (r, resp) = ui.allocate_exact_size(vec2(w, g.size().y), Sense::hover());
                icons::paint(ui.painter(), Rect::from_min_size(pos2(r.min.x + 10.0, r.min.y + 1.0), vec2(14.0, 14.0)), Icon::Lock, p.text_2);
                ui.painter().galley(pos2(r.min.x + 32.0, r.min.y), g, p.text_2);
                resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &note));
            }
        });
    });
}

/// Lato del segno nella barra laterale.
const BRAND_MARK: f32 = 18.0;

/// Marchio in cima alla barra laterale (`.brandrow` della proposta del marchio): segno di 18 punti e nome
/// in Inter SemiBold 14 con le lettere avvicinate dell'1%, a 8 punti dal segno; il nome comincia dove
/// cominciano i nomi delle voci. Con la barra compressa resta il segno, al centro.
fn brand_row(ui: &mut Ui, p: &Palette, collapsed: bool) {
    let w = if collapsed { 40.0 } else { ui.available_width() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, BRAND_MARK), Sense::hover());
    let x = if collapsed { rect.center().x - BRAND_MARK / 2.0 } else { rect.min.x + 10.0 };
    brand::paint(ui.painter(), Rect::from_min_size(pos2(x, rect.min.y), vec2(BRAND_MARK, BRAND_MARK)), p.text);
    if !collapsed {
        let size = 14.0;
        let job = egui::text::LayoutJob::single_section(brand::NAME.to_owned(), egui::TextFormat { font_id: widgets::semibold(size), color: p.text, extra_letter_spacing: -0.01 * size, ..Default::default() });
        let g = ui.painter().layout_job(job);
        // Il centro della riga di Inter coincide con il centro delle maiuscole: segno e nome hanno lo stesso centro.
        ui.painter().galley(pos2(x + BRAND_MARK + 8.0, rect.center().y - g.size().y / 2.0), g, p.text);
    }
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, brand::NAME));
}

/// L'unità scansionata nella barra laterale (`.drv`): nome, uso e barretta; apre l'elenco delle unità.
fn drive_item(ui: &mut Ui, p: &Palette, app: &App, d: &Loaded, collapsed: bool) -> Response {
    let (l, f) = (&app.lingue, &app.fmt);
    let (used, cap) = d.balance.map_or((d.tree().alloc(Tree::ROOT), 0), |b| (b.used, b.capacity));
    let ratio = if cap > 0 { used as f64 / cap as f64 } else { 0.0 };
    let title = volume_title(l, &d.volume);
    let meta = (cap > 0).then(|| l.ta("volume-uso", &[("usato", f.size(used).into()), ("totale", f.size(cap).into()), ("fs", d.volume.file_system.clone().into())]));
    let w = if collapsed { 40.0 } else { ui.available_width() };
    // Nome e uso vanno a capo invece di troncarsi (testi tradotti più lunghi).
    let tg = (!collapsed).then(|| ui.painter().layout(title.clone(), widgets::font(Type::BodyStrong), p.text, w - 46.0));
    let mg = if collapsed { None } else { meta.as_ref().map(|m| ui.painter().layout(m.clone(), widgets::font(Type::Callout), p.text_2, w - 20.0)) };
    let h = if collapsed { 40.0 } else { 9.0 + tg.as_ref().map_or(16.0, |g| g.size().y.max(16.0)) + 4.0 + mg.as_ref().map_or(0.0, |g| g.size().y + 6.0) + 4.0 + 9.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(radius::ROW), p.hover);
    }
    let painter = ui.painter();
    if collapsed {
        icons::paint(painter, Rect::from_center_size(pos2(rect.center().x, rect.min.y + 14.0), vec2(16.0, 16.0)), Icon::Drive, p.text);
        widgets::share_bar(painter, Rect::from_min_size(pos2(rect.min.x + 8.0, rect.max.y - 10.0), vec2(24.0, 4.0)), ratio, p.text_2, p.track);
    } else {
        let x = rect.min.x + 10.0;
        let mut y = rect.min.y + 9.0;
        icons::paint(painter, Rect::from_min_size(pos2(x, y), vec2(16.0, 16.0)), Icon::Drive, p.text);
        if let Some(g) = tg {
            let gh = g.size().y.max(16.0);
            painter.galley(pos2(x + 24.0, y + 8.0 - g.rows.first().map_or(g.size().y, |r| r.height()) / 2.0), g, p.text);
            y += gh + 4.0;
        }
        if let Some(g) = mg {
            let gh = g.size().y;
            painter.galley(pos2(x, y), g, p.text_2);
            y += gh + 6.0;
        }
        widgets::share_bar(painter, Rect::from_min_size(pos2(x, y), vec2(rect.width() - 20.0, 4.0)), ratio, p.text_2, p.track);
    }
    if resp.has_focus() {
        widgets::focus_ring_inside(ui.painter(), rect, p, radius::ROW);
    }
    let name = meta.as_ref().map_or_else(|| title.clone(), |m| format!("{title}, {m}"));
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &name));
    widgets::tip(resp, p, &l.t("barra-cambia-volume"), None)
}

// ------------------------------------------------------------------ barra superiore

pub fn top_bar(app: &mut App, ui: &mut Ui, p: &Palette) {
    egui::Panel::top("barra-titolo").resizable(false).show_separator_line(false).exact_size(TOOLBAR_H).frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 12, right: 16, top: 0, bottom: 0 })).show(ui, |ui| {
        let full = ui.max_rect();
        // A destra: unità, «Scansiona di nuovo», palette. Si disegnano per primi per sapere quanto resta.
        let mut right = ui.new_child(UiBuilder::new().max_rect(full).layout(Layout::right_to_left(Align::Center)));
        right.spacing_mut().item_spacing.x = 4.0;
        right_items(app, &mut right, p);
        let right_w = right.min_rect().width();
        let left_rect = Rect::from_min_max(full.min, pos2(full.max.x - right_w - 16.0, full.max.y));
        let mut left = ui.new_child(UiBuilder::new().max_rect(left_rect).layout(Layout::left_to_right(Align::Center)));
        left.spacing_mut().item_spacing.x = 4.0;
        left.set_clip_rect(left_rect.intersect(ui.clip_rect()));
        left_items(app, &mut left, p);
    });
}

fn right_items(app: &mut App, ui: &mut Ui, p: &Palette) {
    let l = &app.lingue;
    if COMMAND_PALETTE && widgets::icon_button_kbd(ui, p, Icon::Palette, &l.t("barra-palette"), Some("Ctrl+K"), true).clicked() {
        app.palette.show_palette();
    }
    let l = &app.lingue;
    // L'elenco delle unità ha i suoi pulsanti di scansione: qui solo il selettore dell'unità.
    if app.data.is_some() && !standalone(app) {
        let can = app.job.is_none();
        let label = l.t("barra-riesamina");
        let r = widgets::button(ui, p, &label, Some(Icon::Refresh), ButtonKind::Quiet, can);
        let tip = app.data.as_ref().map(|d| l.ta("barra-riesamina-suggerimento", &[("volume", d.volume.letter().into())])).unwrap_or_default();
        let r = widgets::tip(r, p, &tip, None);
        if r.clicked()
            && let Some(v) = app.data.as_ref().map(|d| d.volume.clone())
        {
            app.start_scan(v);
            return;
        }
    }
    let l = &app.lingue;
    let vol_label = match (&app.data, &app.job) {
        (_, Some(j)) => volume_title(l, &j.volume),
        (Some(d), _) => volume_title(l, &d.volume),
        _ => l.t("barra-volumi"),
    };
    let cap = app.data.as_ref().and_then(|d| d.balance).map(|b| app.fmt.size(b.capacity));
    let tip = l.t("barra-cambia-volume");
    let resp = drive_pill(ui, p, &vol_label, cap.as_deref());
    let resp = widgets::tip(resp, p, &tip, None);
    if resp.clicked() {
        app.screen = Screen::Volumes;
    }
}

fn left_items(app: &mut App, ui: &mut Ui, p: &Palette) {
    // Senza barra laterale la colonna ha il suo titolo grande: a sinistra non c'è niente.
    if standalone(app) {
        return;
    }
    if app.data.is_some() {
        let c = collapsed(app);
        let tip = app.lingue.t(if c { "barra-espandi" } else { "barra-comprimi" });
        if widgets::icon_button(ui, p, Icon::Sidebar, &tip, true).clicked() {
            app.settings.sidebar_collapsed = !app.settings.sidebar_collapsed;
            app.sidebar_peek = false;
            app.save_settings();
        }
        ui.add_space(12.0);
        // Indietro, avanti e cartella superiore: la cronologia delle cartelle di Esplora.
        let explore = app.screen == Screen::Explore && !app.search.active();
        let l = &app.lingue;
        let root = app.explore.outline.root;
        let mut nav = None;
        if widgets::icon_button_kbd(ui, p, Icon::Back, &l.t("esplora-indietro"), None, explore && !app.explore.back.is_empty()).clicked() {
            nav = Some(0);
        }
        if widgets::icon_button_kbd(ui, p, Icon::Forward, &l.t("esplora-avanti"), None, explore && !app.explore.forward.is_empty()).clicked() {
            nav = Some(1);
        }
        if explore && widgets::icon_button(ui, p, Icon::ArrowUp, &l.t("esplora-su"), root != Tree::ROOT).clicked() {
            nav = Some(2);
        }
        if let Some(d) = app.data.as_ref() {
            match nav {
                Some(0) => app.explore.go_back(d),
                Some(1) => app.explore.go_forward(d),
                Some(2) => app.explore.up(d),
                _ => {}
            }
        }
        ui.add_space(10.0);
        if explore {
            explore::crumbs(app, ui, p);
            return;
        }
    }
    if let Some(t) = title(app) {
        widgets::typed(ui, t, Type::Headline, p.text);
    }
}

/// Titolo della schermata nella barra superiore (Esplora mostra il percorso).
fn title(app: &App) -> Option<String> {
    let l = &app.lingue;
    let key = match app.screen {
        // Unità e scansione hanno il titolo grande nella colonna.
        Screen::Volumes | Screen::Scanning => return None,
        Screen::Overview => "nav-panoramica",
        Screen::Explore => "nav-esplora",
        Screen::BigFiles => "nav-file-grandi",
        Screen::System => "nav-sistema",
        Screen::Rules => "nav-regole",
        Screen::Downloads => "nav-download",
        Screen::Programs => "nav-programmi",
        Screen::Interventions => "nav-interventi",
        Screen::Settings => "impostazioni-titolo",
    };
    Some(l.t(key))
}

// ------------------------------------------------------------------ barra di stato

pub fn status_bar(app: &mut App, ui: &mut Ui, p: &Palette) {
    egui::Panel::bottom("barra-stato").resizable(false).show_separator_line(false).exact_size(STATUS_H).frame(Frame::new().fill(p.bg).inner_margin(Margin::symmetric(20, 0))).show(ui, |ui| {
        let r = ui.max_rect();
        ui.painter().hline((r.min.x - 20.0)..=(r.max.x + 20.0), r.min.y + 0.5, Stroke::new(1.0, p.separator));
        let l = &app.lingue;
        let f = &app.fmt;
        let small = widgets::font(Type::Callout);
        let painter = ui.painter().clone();
        let width = |s: &str, font: egui::FontId| painter.layout_no_wrap(s.to_owned(), font, p.text_2).size().x;
        // A destra, sempre intera, la voce sui privilegi con lo scudo (e i tempi di frame con F2): si misura per
        // prima, così le voci a sinistra non ci finiscono sopra con il testo più grande o la finestra stretta.
        // Dice com'è avviato Quado: con UAC anche un amministratore avvia i programmi senza elevazione.
        let admin = l.t(if app.env.elevated { "stato-amministratore" } else { "stato-senza-privilegi" });
        let perf = app.show_perf.then(|| {
            let s = app.frame_times.summary();
            format!("p50 {:.1}  p95 {:.1}  p99 {:.1} ms", s.p50, s.p95, s.p99)
        });
        let right_w = 14.0 + 6.0 + width(&admin, small.clone()) + perf.as_ref().map_or(0.0, |s| width(s, widgets::mono(12.0)) + 12.0);
        let right = Rect::from_min_max(pos2(r.max.x - right_w, r.min.y), r.max);
        // A sinistra le misure, con un ordine di rinuncia (`.hide-n` della proposta): se non entrano si tolgono
        // prima il metodo, poi le cartelle; file e cartelle non leggibili restano.
        let mut items: Vec<(String, u8)> = Vec::new();
        if let Some(j) = &app.job {
            let files = j.progress.files.load(std::sync::atomic::Ordering::Relaxed);
            items.push((l.ta("stato-scansione-in-corso", &[("file", f.count(files).into())]), 0));
        } else if let Some(d) = &app.data {
            let t = d.tree();
            let files = u64::from(t.files(Tree::ROOT));
            items.push((l.ta("conteggio-file", &[("n", files.into()), ("testo", f.count(files).into())]), 0));
            items.push((l.ta("conteggio-cartelle", &[("n", d.stats.dirs.into()), ("testo", f.count(d.stats.dirs).into())]), 1));
            let n = d.observed.unreadable.len();
            items.push((l.ta("stato-non-leggibili", &[("n", n.into()), ("testo", f.count(n as u64).into())]), 0));
            items.push((l.t("stato-metodo"), 2));
        }
        let gap = 18.0;
        let notice = app.notice.current();
        // L'avviso (percorso copiato, fine scansione) ha almeno un po' di posto, troncato se serve.
        let notice_min = notice.as_ref().map_or(0.0, |n| width(n, small.clone()).min(200.0) + 12.0);
        let left_room = right.min.x - r.min.x - 12.0 - notice_min;
        let total = |v: &[(String, u8)]| v.iter().map(|(s, _)| width(s, small.clone())).sum::<f32>() + gap * v.len().saturating_sub(1) as f32;
        while total(&items) > left_room {
            let Some(k) = items.iter().enumerate().filter(|(_, (_, d))| *d > 0).max_by_key(|(_, (_, d))| *d).map(|(k, _)| k) else { break };
            items.remove(k);
        }
        let left_rect = Rect::from_min_max(r.min, pos2((right.min.x - 12.0).max(r.min.x), r.max.y));
        let mut left = ui.new_child(UiBuilder::new().max_rect(left_rect).layout(Layout::left_to_right(Align::Center)));
        left.set_clip_rect(left_rect.intersect(ui.clip_rect()));
        left.spacing_mut().item_spacing.x = 0.0;
        for (k, (s, _)) in items.iter().enumerate() {
            if k > 0 {
                status_separator(&mut left);
            }
            // Una voce che non entra nemmeno da sola si tronca, con il testo intero nel suggerimento.
            let w = left.available_width();
            let r = widgets::label_elided(&mut left, s, small.clone(), p.text_2, w);
            r.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, s));
        }
        if let Some(n) = notice {
            // L'avviso sta accanto allo scudo, allineato a destra; un avviso lungo si tronca, con il testo intero
            // nel suggerimento e per AccessKit.
            let end = right.min.x - 12.0;
            let start = left.min_rect().max.x + gap;
            let w = width(&n, small.clone()).min(end - start);
            if w >= 40.0 {
                let rect = Rect::from_min_max(pos2(end - w, r.min.y), pos2(end, r.max.y));
                let mut nui = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
                let resp = widgets::label_elided(&mut nui, &n, small.clone(), p.text, w);
                resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &n));
            }
        }
        let mut rui = ui.new_child(UiBuilder::new().max_rect(right).layout(Layout::right_to_left(Align::Center)));
        rui.spacing_mut().item_spacing.x = 6.0;
        if let Some(s) = perf {
            widgets::text(&mut rui, s, widgets::mono(12.0), p.text_3);
            rui.add_space(6.0);
        }
        widgets::text(&mut rui, admin, small, p.text_2);
        let (ir, _) = rui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        icons::paint(rui.painter(), ir, Icon::Shield, p.text_2);
    });
}

/// Tasti della cornice: Ctrl+più, Ctrl+meno e Ctrl+0 cambiano la dimensione del testo (vale anche fuori da
/// Impostazioni). Restituisce vero se la dimensione è cambiata.
pub fn text_size_keys(app: &mut App, ui: &Ui) -> bool {
    let (bigger, smaller, reset) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::COMMAND, Key::Plus) || i.consume_key(egui::Modifiers::COMMAND, Key::Equals),
            i.consume_key(egui::Modifiers::COMMAND, Key::Minus),
            i.consume_key(egui::Modifiers::COMMAND, Key::Num0),
        )
    });
    let before = app.settings.text_scale;
    if bigger {
        app.settings.step_text(true);
    }
    if smaller {
        app.settings.step_text(false);
    }
    if reset {
        app.settings.text_scale = 100;
    }
    app.settings.text_scale != before
}

/// Misure della densità in uso (scorciatoia per le schermate).
pub fn metrics(ui: &Ui) -> tokens::Metrics {
    tokens::metrics(ui.ctx())
}

/// Larghezza della colonna di dettaglio a destra: la stessa dell'ispettore di Esplora.
pub const DETAIL_W: f32 = explore::INSPECTOR_W;

/// Aree di una schermata con la colonna di dettaglio (`.ex`): contenuto a sinistra e dettaglio a destra,
/// già senza i margini interni.
pub struct Split {
    pub content: Rect,
    pub detail: Rect,
    /// La colonna di dettaglio intera, fino ai bordi.
    pub detail_full: Rect,
}

/// Divide la schermata come Esplora: la colonna di dettaglio prende tutta l'altezza a destra nel tono della
/// barra laterale (File grandi, Download, Programmi); il contenuto ha 20 punti di margine ai lati.
pub fn split(ui: &Ui, p: &Palette) -> Split {
    let full = ui.max_rect();
    let right = Rect::from_min_max(pos2(full.max.x - DETAIL_W, full.min.y), full.max);
    ui.painter().rect_filled(right, CornerRadius::ZERO, p.sidebar);
    let content = Rect::from_min_max(pos2(full.min.x + 20.0, full.min.y), pos2(right.min.x - 20.0, full.max.y - 16.0));
    let detail = Rect::from_min_max(right.min + vec2(22.0, 22.0), right.max - vec2(22.0, 16.0));
    Split { content, detail, detail_full: right }
}

/// Stato vuoto: cosa manca e perché, centrato nell'area, con al più un'azione sotto. Restituisce
/// vero se l'azione è stata scelta.
pub fn empty_state(ui: &mut Ui, p: &Palette, rect: Rect, message: &str, action: Option<&str>) -> bool {
    let w = (rect.width() - 48.0).clamp(120.0, 480.0);
    let g = widgets::galley(ui.painter(), message, Type::Body, p.text_2, Some(w));
    let bh = if action.is_some() { focus::MIN_TARGET + 14.0 } else { 0.0 };
    let top = rect.center().y - (g.size().y + bh) / 2.0;
    let gs = g.size();
    let r = Rect::from_min_size(pos2(rect.center().x - gs.x / 2.0, top), gs);
    ui.painter().galley(r.min, g, p.text_2);
    let resp = ui.interact(r, ui.id().with(("stato_vuoto", message)), Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, message));
    let Some(label) = action else { return false };
    let bw = ui.painter().layout_no_wrap(label.to_owned(), widgets::font(Type::Body), p.text).size().x + 24.0;
    let b = Rect::from_min_size(pos2(rect.center().x - bw / 2.0, r.max.y + 14.0), vec2(bw, focus::MIN_TARGET));
    let mut bui = ui.new_child(UiBuilder::new().max_rect(b).layout(Layout::left_to_right(Align::Center)));
    widgets::button(&mut bui, p, label, None, ButtonKind::Secondary, true).clicked()
}
