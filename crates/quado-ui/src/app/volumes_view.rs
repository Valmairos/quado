// SPDX-License-Identifier: GPL-3.0-or-later
//! Unità e scansione in corso, variante A. Primo avvio: una colonna centrata con il titolo, la promessa della
//! sola lettura e una card per unità (icona, nome, tipo, la cifra grande e leggera dello spazio usato, la
//! barra e «Scansiona C:» come azione principale), poi la scansione del volume di sistema con un clic. La
//! scansione in corso tiene la stessa colonna: le quattro cifre dal vivo, «Annulla» ed Esc, e il primo
//! livello dell'unità con i totali che crescono.

use std::sync::atomic::Ordering::Relaxed;

use egui::{Align, CornerRadius, Frame, Layout, Margin, Rect, ScrollArea, Sense, Stroke, Ui, pos2, vec2};
use quado_scan_win::Problem;
use quado_scan_win::volumes::{DriveKind, Support, Volume};

use super::data::{Loaded, VolumeInfo};
use super::{App, volume_title};
use crate::format::Formatter;
use crate::i18n::Lingue;
use crate::icons::{self, Icon};
use crate::tokens::{self, Elevation, Gestore, Palette, Type, radius};
use crate::widgets::{self, ButtonKind};

/// Larghezza massima della colonna di Unità e della scansione (`.drives`), margini compresi.
const COLUMN_MAX: f32 = 780.0;

fn kind_key(k: DriveKind) -> &'static str {
    match k {
        DriveKind::Fixed => "unita-fissa",
        DriveKind::Removable => "unita-rimovibile",
        DriveKind::Network => "unita-di-rete",
        DriveKind::Optical => "unita-ottica",
        DriveKind::Ram => "unita-ram",
        DriveKind::Unknown => "unita-sconosciuta",
    }
}

/// Spazio sopra il titolo: il 13% dell'altezza come nella proposta (`padding-top: 13vh`), senza esagerare
/// nelle finestre molto alte o molto basse.
fn top_space(ui: &Ui) -> f32 {
    (ui.available_height() * 0.13).clamp(24.0, 132.0)
}

/// Titolo della colonna e la riga sotto, nel corpo da 15 punti della proposta.
fn heading(ui: &mut Ui, p: &Palette, title: &str, lead: &str) {
    widgets::typed(ui, title, Type::LargeTitle, p.text);
    ui.add_space(8.0);
    let w = ui.available_width().min(560.0);
    ui.allocate_ui_with_layout(vec2(w, 0.0), Layout::top_down(Align::Min), |ui| {
        ui.add(egui::Label::new(egui::RichText::new(lead).font(widgets::sans(15.0)).color(p.text_2)).wrap());
    });
}

/// Nota con l'icona piccola davanti (`.c2.t-callout.rowflex`): sola lettura, partizioni senza lettera.
fn note(ui: &mut Ui, p: &Palette, icon: Icon, s: &str) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(vec2(14.0, 17.0), Sense::hover());
        icons::paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 8.5), vec2(14.0, 14.0)), icon, p.text_2);
        widgets::paragraph(ui, s, Type::Callout, p.text_2);
    });
}

/// Card della variante A con il margine interno della card delle unità (`.dcard`: 24 ai lati, 22 sotto).
fn drive_card(p: &Palette) -> Frame {
    Frame::new().fill(p.panel).corner_radius(CornerRadius::same(radius::CARD)).shadow(p.shadow(Elevation::Card)).inner_margin(Margin { left: 24, right: 24, top: 24, bottom: 22 })
}

pub fn volumes(app: &mut App, ui: &mut Ui, p: &Palette) {
    let mut start: Option<VolumeInfo> = None;
    let mut back = false;
    // Con più di due unità lo spazio sopra si riduce, così le note in fondo restano in vista.
    let many = app.volumes.iter().filter(|v| VolumeInfo::from_volume(v).is_some()).count() > 2;
    let top = top_space(ui) * if many { 0.4 } else { 1.0 };
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        widgets::centered_column(ui, COLUMN_MAX, |ui| {
            Frame::new().inner_margin(Margin { left: 40, right: 40, top: 0, bottom: 40 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add_space(top);
                super::overview::interrupted_banner(app, ui, p);
                let (l, f) = (&app.lingue, &app.fmt);
                // Dall'app si torna alla Panoramica; al primo avvio non c'è niente dietro.
                if app.data.is_some() {
                    if widgets::button(ui, p, &l.t("volumi-torna"), Some(Icon::Back), ButtonKind::Quiet, true).clicked() {
                        back = true;
                    }
                    ui.add_space(12.0);
                }
                heading(ui, p, &l.t("volumi-titolo"), &l.t("volumi-meta"));
                ui.add_space(28.0);
                // Stato vuoto: nessun volume con una lettera di unità (l'elenco si legge all'avvio).
                if app.volumes.iter().all(|v| VolumeInfo::from_volume(v).is_none()) {
                    drive_card(p).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        widgets::paragraph(ui, l.t("volumi-nessuno"), Type::Body, p.text_2);
                    });
                    ui.add_space(12.0);
                }
                // L'azione principale è una sola: la scansione dell'unità di sistema.
                let primary = app.volumes.iter().position(|v| v.is_system && v.scannable() && VolumeInfo::from_volume(v).is_some());
                for (k, v) in app.volumes.iter().enumerate() {
                    let Some(info) = VolumeInfo::from_volume(v) else { continue };
                    let enabled = app.job.is_none() && v.scannable();
                    if ui.push_id(("unita", k), |ui| volume_card(ui, p, l, f, v, &info, primary == Some(k), enabled)).inner {
                        start = Some(info);
                    }
                    ui.add_space(12.0);
                }
                ui.add_space(6.0);
                note(ui, p, Icon::Lock, &l.t("nav-nota-sola-lettura"));
                ui.add_space(4.0);
                note(ui, p, Icon::Info, &l.t("volumi-nota"));
            });
        });
    });
    if back {
        app.go(super::Screen::Overview);
    }
    if let Some(v) = start {
        app.start_scan(v);
    }
}

/// Una unità (`.dcard`): icona grande, nome e tipo, il pulsante della scansione; sotto la cifra dello spazio
/// usato con la quota e la barra. Restituisce vero se il pulsante è stato premuto.
#[allow(clippy::too_many_arguments)]
fn volume_card(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, v: &Volume, info: &VolumeInfo, primary: bool, enabled: bool) -> bool {
    let mut clicked = false;
    drive_card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        // Testata: icona, nome e tipo centrati sull'altezza del blocco, il pulsante a destra. Il pulsante si
        // misura prima: nome e tipo vanno a capo nello spazio che resta.
        let label = l.ta("volumi-scansiona", &[("lettera", info.letter().into())]);
        let font = if primary { widgets::semibold(13.0) } else { widgets::sans(13.0) };
        let bw = ui.painter().layout_no_wrap(label.clone(), font, p.text).size().x + 24.0;
        let w = ui.available_width();
        let text_w = (w - 32.0 - 20.0 - bw - 20.0).max(120.0);
        let title = volume_title(l, info);
        let mut meta = format!("{} · {}", info.file_system, l.t(kind_key(info.kind)));
        if info.is_system {
            meta = format!("{meta} · {}", l.t("volumi-di-sistema"));
        }
        let tg = widgets::galley(ui.painter(), title.clone(), Type::Headline, p.text, Some(text_w));
        let mg = widgets::galley(ui.painter(), meta.clone(), Type::Callout, p.text_2, Some(text_w));
        let block = tg.size().y + 2.0 + mg.size().y;
        let (head, resp) = ui.allocate_exact_size(vec2(w, block.max(tokens::focus::MIN_TARGET)), Sense::hover());
        let cy = head.center().y;
        icons::paint_stroke(ui.painter(), Rect::from_center_size(pos2(head.min.x + 16.0, cy), vec2(32.0, 32.0)), Icon::Drive, p.text_2, 1.25);
        let top = cy - block / 2.0;
        let th = tg.size().y;
        ui.painter().galley(pos2(head.min.x + 52.0, top), tg, p.text);
        ui.painter().galley(pos2(head.min.x + 52.0, top + th + 2.0), mg, p.text_2);
        let name = format!("{title}, {meta}");
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
        let b = Rect::from_min_max(pos2(head.max.x - bw, cy - tokens::focus::MIN_TARGET / 2.0), pos2(head.max.x, cy + tokens::focus::MIN_TARGET / 2.0));
        let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(b).layout(Layout::left_to_right(Align::Center)));
        let kind = if primary { ButtonKind::Primary } else { ButtonKind::Secondary };
        clicked = widgets::button(&mut bui, p, &label, None, kind, enabled).clicked();
        // File system con misure diverse da NTFS: la nota, in avviso se non ci sono garanzie.
        match info.support {
            Support::Full => {}
            Support::Fat => {
                ui.add_space(10.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(52.0);
                    widgets::paragraph(ui, l.ta("volumi-nota-fat", &[("fs", info.file_system.clone().into())]), Type::Callout, p.text_2);
                });
            }
            Support::NoGuarantees => {
                ui.add_space(10.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(52.0);
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 16.0), Sense::hover());
                    icons::paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 8.0), vec2(14.0, 14.0)), Icon::Warning, p.warn);
                    widgets::paragraph(ui, l.t("volumi-senza-garanzie"), Type::Callout, p.warn);
                });
            }
        }
        let Some(s) = v.space else { return };
        let used = s.used();
        let ratio = if s.capacity > 0 { used as f64 / s.capacity as f64 } else { 0.0 };
        ui.add_space(18.0);
        // La cifra grande e leggera, con l'etichetta sulla stessa linea di base; a destra la quota.
        figure_line(ui, p, &f.size(used), &l.ta("bilancio-usati-su", &[("totale", f.size(s.capacity).into())]), &f.percent(ratio));
        ui.add_space(12.0);
        let (bar, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
        widgets::share_bar(ui.painter(), bar, ratio, p.text_2, p.track);
        let name = l.ta("volumi-uso", &[("usato", f.size(used).into()), ("totale", f.size(s.capacity).into())]);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
    });
    clicked
}

/// Cifra grande e leggera con l'etichetta accanto sulla stessa linea di base e, a destra, un valore piccolo
/// (`402 GB  usati su 476 GB ……… 84,4%`). Tre testi separati: ognuno si legge da solo.
fn figure_line(ui: &mut Ui, p: &Palette, value: &str, label: &str, right: &str) {
    let painter = ui.painter().clone();
    let size = 32.0;
    let vg = painter.layout_no_wrap(value.to_owned(), widgets::light(size), p.text);
    let lg = widgets::galley(&painter, label, Type::Body, p.text_2, None);
    let rg = painter.layout_no_wrap(right.to_owned(), widgets::num(13.0), p.text_2);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), vg.size().y), Sense::hover());
    // Linea di base comune: il basso dei testi meno la discesa del loro font (circa 0,24 em per Inter).
    let base = rect.max.y - 0.24 * size;
    let small = |h: f32| base - h + 0.24 * 13.0;
    let vw = vg.size().x;
    let (lh, rh, rw) = (lg.size().y, rg.size().y, rg.size().x);
    painter.galley(rect.min, vg, p.text);
    painter.galley(pos2(rect.min.x + vw + 10.0, small(lh)), lg, p.text_2);
    painter.galley(pos2(rect.max.x - rw, small(rh)), rg, p.text_2);
    let name = format!("{value} {label}, {right}");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
}

/// Avviso di fine scansione, per la barra di stato: durata e cartelle con l'accesso negato, il cui contenuto
/// resta nel non attribuito. Nessun avviso per una scansione annullata: lì vale il banner "Scansione interrotta".
pub fn completion_notice(l: &Lingue, f: &Formatter, d: &Loaded) -> Option<String> {
    if d.cancelled {
        return None;
    }
    let denied = d.observed.unreadable.iter().filter(|(_, pr)| *pr == Problem::AccessDenied).count();
    Some(l.ta("scansione-completata", &[("n", denied.into()), ("testo", f.count(denied as u64).into()), ("durata", f.seconds(d.duration.as_secs_f64()).into())]))
}

/// Scansione in corso, nella colonna di Unità: titolo, cosa succede, le quattro cifre dal vivo in una card con
/// «Annulla» ed Esc in fondo, poi il primo livello dell'unità con i totali che crescono.
pub fn scanning(app: &mut App, ui: &mut Ui, p: &Palette) {
    let Some(job) = app.job.as_ref() else { return };
    let (l, f) = (&app.lingue, &app.fmt);
    let pr = &job.progress;
    let mut cancel = false;
    let top = top_space(ui);
    widgets::centered_column(ui, COLUMN_MAX, |ui| {
        Frame::new().inner_margin(Margin { left: 40, right: 40, top: 0, bottom: 24 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add_space(top);
            let meta = l.t(if app.env.elevated { "scansione-meta-amministratore" } else { "scansione-meta" });
            heading(ui, p, &l.ta("scansione-titolo", &[("volume", volume_title(l, &job.volume).into())]), &meta);
            ui.add_space(28.0);
            let files = pr.files.load(Relaxed);
            let dirs = pr.dirs.load(Relaxed);
            let alloc = pr.alloc.load(Relaxed);
            widgets::bare_card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let items: Vec<widgets::Figure> = vec![
                    (f.count(files), l.t("scansione-file"), None),
                    (f.count(dirs), l.t("scansione-cartelle"), None),
                    (f.size(alloc), l.t("scansione-letti"), None),
                    (f.seconds(job.started.elapsed().as_secs_f64()), l.t("scansione-tempo"), None),
                ];
                Frame::new().inner_margin(Margin { left: 6, right: 6, top: 8, bottom: 4 }).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    widgets::figures(ui, p, p.panel, 30.0, &items);
                });
                // Piede (`.dlg .foot`): la scorciatoia a sinistra, «Annulla» a destra.
                let y = ui.cursor().min.y;
                let r = ui.max_rect();
                ui.painter().hline(r.x_range(), y + 0.5, Stroke::new(1.0, p.separator));
                Frame::new().inner_margin(Margin { left: 24, right: 16, top: 12, bottom: 12 }).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        widgets::typed(ui, l.t("scansione-esc"), Type::Callout, p.text_2);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let label = if job.cancel_requested.is_some() { l.t("scansione-annullo") } else { l.t("scansione-annulla") };
                            if widgets::button(ui, p, &label, None, ButtonKind::Secondary, job.cancel_requested.is_none()).clicked() {
                                cancel = true;
                            }
                        });
                    });
                });
            });
            ui.add_space(24.0);
            widgets::bare_card(p).show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::card_header(ui, p, &l.t("scansione-primo-livello"), Some(&l.t("scansione-primo-livello-nota")), None);
                Frame::new().inner_margin(Margin { left: 8, right: 8, top: 0, bottom: 10 }).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let Some(top) = pr.top() else {
                        Frame::new().inner_margin(Margin::symmetric(12, 6)).show(ui, |ui| {
                            widgets::paragraph(ui, l.t("scansione-attesa"), Type::Body, p.text_2);
                        });
                        return;
                    };
                    let mut rows: Vec<(String, u64)> = top.iter().map(|e| (String::from_utf16_lossy(&e.name), e.alloc.load(Relaxed))).collect();
                    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                    let max = rows.first().map_or(1, |r| r.1).max(1);
                    let row_h = tokens::metrics(ui.ctx()).row;
                    // Le righe che entrano nella finestra; le altre si contano sotto.
                    let room = ui.available_height() - 40.0;
                    let shown = ((room / row_h).floor() as usize).max(4);
                    for (k, (name, a)) in rows.iter().take(shown).enumerate() {
                        top_row(ui, p, f, name, *a, max, row_h, k > 0);
                    }
                    if rows.len() > shown {
                        let rest = (rows.len() - shown) as u64;
                        Frame::new().inner_margin(Margin { left: 12, right: 12, top: 8, bottom: 0 }).show(ui, |ui| {
                            widgets::paragraph(ui, l.ta("scansione-altri", &[("n", rest.into()), ("testo", f.count(rest).into())]), Type::Callout, p.text_2);
                        });
                    }
                });
            });
        });
    });
    if cancel && let Some(j) = app.job.as_mut() {
        j.cancel();
    }
}

/// Riga del primo livello (`.trow` senza freccia): l'anello del non classificato (la scansione non ha ancora
/// le regole), il nome, la barretta della quota e la dimensione; un filetto rientrato tra le righe.
#[allow(clippy::too_many_arguments)]
fn top_row(ui: &mut Ui, p: &Palette, f: &Formatter, name: &str, bytes: u64, max: u64, h: f32, separator: bool) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let painter = ui.painter();
    if separator {
        painter.hline((rect.min.x + 38.0)..=(rect.max.x - 12.0), rect.min.y + 0.5, Stroke::new(1.0, p.separator));
    }
    let cy = rect.center().y;
    widgets::manager_glyph(painter, pos2(rect.min.x + 18.0, cy), Gestore::Unknown, p);
    let sg = painter.layout_no_wrap(f.size(bytes), widgets::num(13.0), p.text);
    let sw = sg.size().x;
    painter.galley(pos2(rect.max.x - 12.0 - sw, cy - sg.size().y / 2.0), sg, p.text);
    let bar_w = (w * 0.14).clamp(48.0, 88.0);
    let bar = Rect::from_min_size(pos2(rect.max.x - 12.0 - sw - 14.0 - bar_w, cy - 2.0), vec2(bar_w, 4.0));
    widgets::share_bar(painter, bar, bytes as f64 / max as f64, p.data_color(Gestore::Unknown), p.track);
    let ng = widgets::elided(painter, name.to_owned(), widgets::font(Type::BodyStrong), p.text, (bar.min.x - rect.min.x - 38.0 - 14.0).max(40.0));
    painter.galley(pos2(rect.min.x + 38.0, cy - ng.size().y / 2.0), ng, p.text);
}
