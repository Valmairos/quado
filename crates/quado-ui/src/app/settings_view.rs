// SPDX-License-Identifier: GPL-3.0-or-later
//! Impostazioni nella variante A: indice delle sezioni a sinistra, righe raggruppate al centro, anteprima
//! dal vivo a destra. Sezioni: Aspetto (tema, accento, colori dei dati, dimensione del testo, densità,
//! animazioni), Esplora (vista iniziale, colonne, barra laterale), Lingua (05 D13), Unità di misura
//! (F-GE-3), Dati e privacy (05 D8, F-AC-6: i file salvati, con la dimensione, e la cancellazione del
//! registro degli interventi con una conferma; la cartella dei dati di Rilievo, se c'è ancora, con la
//! dimensione e la cancellazione con una conferma), Informazioni (marchio, versione, grafica, cartella).
//!
//! Ogni scelta vale subito e si salva in `%APPDATA%\Quado\impostazioni.txt` (`App::save_settings`).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Align, Color32, CornerRadius, Id, Layout, Rect, ScrollArea, Sense, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};
use quado_actions::journal::{self, JournalError};
use quado_actions::legacy;
use quado_actions::shell;

use super::settings::{self, Columns, ExploreView, TEXT_SCALE_MAX, TEXT_SCALE_MIN, TEXT_SCALE_STEP};
use super::{App, kbtext};
use crate::brand;
use crate::format::{Formatter, Units};
use crate::i18n::{Lingue, REFERENCE, embedded};
use crate::icons::Icon;
use crate::tokens::{Accent, DataPalette, Density, Elevation, Gestore, Motion, Palette, ThemeChoice, Type, focus, radius};
use crate::widgets::{self, ButtonKind, mono};

/// Nome del file modello che si copia nella cartella delle lingue per tradurlo.
pub const TEMPLATE: &str = "modello-da-tradurre.txt";

/// Ogni quanto si rileggono i file della pagina Dati e privacy mentre è aperta.
const FILES_EVERY: Duration = Duration::from_secs(2);

/// Larghezze della pagina (`.vA .set`): indice, righe, anteprima.
const INDEX_W: f32 = 168.0;
const ROWS_MAX_W: f32 = 640.0;
const PREVIEW_W: f32 = 300.0;
const GAP: f32 = 36.0;

/// Sezioni, nell'ordine della pagina e dell'indice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Section {
    Appearance,
    Explore,
    Language,
    Units,
    Privacy,
    About,
}

impl Section {
    const ALL: [Self; 6] = [Self::Appearance, Self::Explore, Self::Language, Self::Units, Self::Privacy, Self::About];

    fn key(self) -> &'static str {
        match self {
            Self::Appearance => "impostazioni-aspetto",
            Self::Explore => "nav-esplora",
            Self::Language => "impostazioni-lingua",
            Self::Units => "impostazioni-unita",
            Self::Privacy => "privacy-titolo",
            Self::About => "impostazioni-informazioni",
        }
    }
}

/// Stato della pagina nei dati temporanei di egui: sezione corrente e sezione da raggiungere.
#[derive(Clone, Copy, Debug, Default)]
struct PageState {
    current: Option<Section>,
    scroll_to: Option<Section>,
}

fn page_id() -> Id {
    Id::new("quado_impostazioni_pagina")
}

/// Un file (o una cartella) che Quado salva in `%APPDATA%\Quado` (05 D8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DataFile {
    Settings,
    Journal,
    Languages,
}

impl DataFile {
    fn key(self) -> &'static str {
        match self {
            Self::Settings => "privacy-file-impostazioni",
            Self::Journal => "privacy-file-registro",
            Self::Languages => "privacy-file-lingue",
        }
    }
}

/// Stato della pagina Dati e privacy nei dati temporanei di egui: i file letti, la cartella di Rilievo con
/// la sua dimensione (se c'è) e le conferme aperte.
#[derive(Clone, Debug, Default)]
struct PrivacyState {
    dir: Option<PathBuf>,
    files: Vec<(DataFile, PathBuf, u64)>,
    old: Option<(PathBuf, u64)>,
    read_at: Option<Instant>,
    confirm: bool,
    confirm_old: bool,
}

/// Cancellazioni confermate nella pagina Dati e privacy in questo frame.
#[derive(Clone, Copy, Debug, Default)]
struct PrivacyAsk {
    /// Il registro degli interventi.
    journal: bool,
    /// La cartella dei dati di Rilievo.
    old: bool,
}

fn privacy_id() -> Id {
    Id::new("quado_dati_e_privacy")
}

/// Dimensione di un file, o dei file di una cartella (sottocartelle comprese). I collegamenti non si
/// attraversano. `None` se il percorso non c'è.
fn size_of_path(p: &Path) -> Option<u64> {
    let m = std::fs::symlink_metadata(p).ok()?;
    if !m.is_dir() {
        return Some(m.len());
    }
    Some(std::fs::read_dir(p).ok()?.flatten().filter_map(|e| size_of_path(&e.path())).sum())
}

/// I file di Quado in `dir` che esistono, con la dimensione: impostazioni, registro degli interventi,
/// cartella delle lingue. Nessun log: il registro tecnico va su standard error, mai su file.
fn data_files(dir: &Path) -> Vec<(DataFile, PathBuf, u64)> {
    [
        (DataFile::Settings, dir.join(settings::FILE_NAME)),
        (DataFile::Journal, journal::path(dir)),
        (DataFile::Languages, dir.join(settings::LANG_DIR)),
    ]
    .into_iter()
    .filter_map(|(k, p)| size_of_path(&p).map(|s| (k, p, s)))
    .collect()
}

/// Righe Dati e privacy: i file di Quado e, se c'è ancora, la cartella dei dati di Rilievo in un gruppo a
/// parte. Restituisce le cancellazioni confermate.
fn privacy_rows(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, dir: Option<&Path>, old: Option<&Path>) -> PrivacyAsk {
    let mut st: PrivacyState = ui.ctx().data(|d| d.get_temp(privacy_id())).unwrap_or_default();
    if st.dir.as_deref() != dir || st.read_at.is_none_or(|t| t.elapsed() > FILES_EVERY) {
        st.files = dir.map(data_files).unwrap_or_default();
        st.old = old.and_then(|o| o.is_dir().then(|| size_of_path(o)).flatten().map(|s| (o.to_owned(), s)));
        st.read_at = Some(Instant::now());
        st.dir = dir.map(Path::to_owned);
    }
    let mut ask = PrivacyAsk::default();
    let mut delete = false;
    widgets::rows(ui, p, |rows| {
        let Some(dir) = dir else {
            rows.stacked(None, Some(&l.t("privacy-non-disponibile")), |_| {});
            return;
        };
        rows.stacked(None, Some(&l.ta("privacy-nota", &[("cartella", dir.display().to_string().into())])), |ui| {
            if st.files.is_empty() {
                widgets::paragraph(ui, l.t("privacy-niente"), Type::Callout, p.text_2);
            }
            // Il nome dentro la cartella dei dati: la cartella è già nella nota sopra.
            for (k, path, size) in &st.files {
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(egui::vec2(260.0, 20.0), Layout::left_to_right(Align::Center), |ui| {
                        widgets::typed(ui, l.ta(k.key(), &[("dimensione", f.size(*size).into())]), Type::Body, p.text);
                    });
                    let name = path.strip_prefix(dir).unwrap_or(path).display().to_string();
                    widgets::text(ui, name, mono(12.0), p.text_2);
                });
            }
        });
        let journal = st.files.iter().find(|(k, _, _)| *k == DataFile::Journal).map(|&(_, _, s)| s);
        if st.confirm {
            rows.stacked(None, None, |ui| {
                widgets::paragraph(ui, l.ta("privacy-conferma", &[("dimensione", f.size(journal.unwrap_or(0)).into())]), Type::Body, p.warn);
                ui.horizontal(|ui| {
                    if widgets::button(ui, p, &l.t("privacy-cancella-conferma"), None, ButtonKind::Primary, true).clicked() {
                        delete = true;
                        st.confirm = false;
                    }
                    if widgets::button(ui, p, &l.t("privacy-annulla"), None, ButtonKind::Secondary, true).clicked() {
                        st.confirm = false;
                    }
                });
            });
        } else {
            let note = if journal.is_none() { Some(l.t("privacy-registro-assente")) } else { None };
            rows.stacked(None, note.as_deref(), |ui| {
                if widgets::button(ui, p, &l.t("privacy-cancella-registro"), None, ButtonKind::Secondary, journal.is_some()).clicked() {
                    st.confirm = true;
                }
            });
        }
    });
    ask.journal = delete;
    // La cartella di Rilievo, la versione precedente: dimensione, percorso e cancellazione con una conferma.
    if let Some((path, size)) = st.old.clone() {
        ui.add_space(12.0);
        let size = f.size(size);
        widgets::rows(ui, p, |rows| {
            if st.confirm_old {
                rows.stacked(None, None, |ui| {
                    widgets::paragraph(ui, l.ta("privacy-rilievo-conferma", &[("dimensione", size.clone().into())]), Type::Body, p.warn);
                    ui.horizontal(|ui| {
                        if widgets::button(ui, p, &l.t("privacy-rilievo-cancella-conferma"), None, ButtonKind::Primary, true).clicked() {
                            ask.old = true;
                            st.confirm_old = false;
                        }
                        if widgets::button(ui, p, &l.t("privacy-annulla"), None, ButtonKind::Secondary, true).clicked() {
                            st.confirm_old = false;
                        }
                    });
                });
            } else {
                let note = l.ta("privacy-rilievo-nota", &[("dimensione", size.clone().into()), ("cartella", path.display().to_string().into())]);
                rows.stacked(Some(&l.t("privacy-rilievo-titolo")), Some(&note), |ui| {
                    if widgets::button(ui, p, &l.t("privacy-rilievo-cancella"), None, ButtonKind::Secondary, true).clicked() {
                        st.confirm_old = true;
                    }
                });
            }
        });
    }
    if ask.journal || ask.old {
        // I file si rileggono subito.
        st.read_at = None;
    }
    ui.ctx().data_mut(|d| d.insert_temp(privacy_id(), st));
    ask
}

/// Cosa l'utente ha chiesto in questo frame; si applica dopo il disegno, quando `app` è libero.
#[derive(Default)]
struct Changes {
    privacy: PrivacyAsk,
    new_lang: Option<String>,
    reload: bool,
    open_dir: bool,
    template: bool,
}

pub fn show(app: &mut App, ui: &mut Ui, p: &Palette) {
    let mut ch = Changes::default();
    let mut s = app.settings.clone();
    let mut page: PageState = ui.ctx().data(|d| d.get_temp(page_id())).unwrap_or_default();
    let full = ui.max_rect();
    // Colonne visibili secondo la larghezza (04 §3, ordine di rinuncia): prima l'indice, poi l'anteprima.
    let show_index = full.width() >= 1180.0;
    let show_preview = full.width() >= 860.0;
    let side = |on: bool, w: f32| if on { w + GAP } else { 0.0 };
    let rows_w = (full.width() - 80.0 - side(show_index, INDEX_W) - side(show_preview, PREVIEW_W)).clamp(280.0, ROWS_MAX_W);
    let total = rows_w + side(show_index, INDEX_W) + side(show_preview, PREVIEW_W);
    let mut x = full.min.x + ((full.width() - total) / 2.0).max(0.0);
    let top = full.min.y + 8.0;
    let index_rect = show_index.then(|| {
        let r = Rect::from_min_size(pos2(x, top), vec2(INDEX_W, full.height() - 8.0));
        x += INDEX_W + GAP;
        r
    });
    let rows_rect = Rect::from_min_max(pos2(x, full.min.y), pos2(x + rows_w, full.max.y));
    x += rows_w + GAP;
    let preview_rect = show_preview.then(|| Rect::from_min_size(pos2(x, top), vec2(PREVIEW_W, full.height() - 8.0)));

    // Indice delle sezioni: fermo mentre le righe scorrono.
    if let Some(r) = index_rect {
        let mut iui = ui.new_child(UiBuilder::new().max_rect(r).layout(Layout::top_down(Align::Min)));
        iui.spacing_mut().item_spacing.y = 1.0;
        iui.add_space(4.0);
        let current = page.current.unwrap_or(Section::Appearance);
        for sec in Section::ALL {
            if index_item(&mut iui, p, &app.lingue.t(sec.key()), sec == current).clicked() {
                page.scroll_to = Some(sec);
                page.current = Some(sec);
            }
        }
        let resp = iui.interact(iui.min_rect(), Id::new("quado_impostazioni_indice"), Sense::hover());
        let name = app.lingue.t("impostazioni-indice");
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, &name));
    }

    // Righe raggruppate, in un'area che scorre.
    let mut rui = ui.new_child(UiBuilder::new().max_rect(rows_rect).layout(Layout::top_down(Align::Min)));
    let mut seen: Vec<(Section, f32)> = Vec::new();
    let clip_top = rows_rect.min.y;
    ScrollArea::vertical().id_salt("impostazioni_righe").auto_shrink([false, false]).show(&mut rui, |ui| {
        ui.set_width(rows_w);
        ui.add_space(8.0);
        for sec in Section::ALL {
            let head = section_title(ui, p, &app.lingue.t(sec.key()));
            if page.scroll_to == Some(sec) {
                ui.scroll_to_rect(head.expand2(vec2(0.0, 8.0)), Some(Align::TOP));
                page.scroll_to = None;
            }
            seen.push((sec, head.min.y));
            match sec {
                Section::Appearance => appearance(ui, p, app, &mut s),
                Section::Explore => explore_rows(ui, p, &app.lingue, &mut s),
                Section::Language => language_rows(ui, p, app, &mut ch),
                Section::Units => units_rows(ui, p, &app.lingue, &mut s),
                Section::Privacy => ch.privacy = privacy_rows(ui, p, &app.lingue, &app.fmt, app.env.data_dir.as_deref(), app.env.old_data_dir.as_deref()),
                Section::About => about_rows(ui, p, app),
            }
            ui.add_space(30.0);
        }
        ui.add_space(26.0);
    });
    // Sezione corrente: l'ultima il cui titolo è salito oltre il bordo alto dell'area.
    if let Some(&(first, _)) = seen.first() {
        page.current = Some(seen.iter().rev().find(|(_, y)| *y <= clip_top + 48.0).map_or(first, |&(s, _)| s));
    }
    ui.ctx().data_mut(|d| d.insert_temp(page_id(), page));

    // Anteprima dal vivo dei colori, della densità e del testo scelti.
    if let Some(r) = preview_rect {
        let mut pui = ui.new_child(UiBuilder::new().max_rect(r).layout(Layout::top_down(Align::Min)));
        preview(&mut pui, app, &s);
    }
    apply(app, s, ch);
}

/// Titolo di una sezione (`.sgrp > h2`), con lo spazio della proposta. Restituisce il suo rettangolo.
fn section_title(ui: &mut Ui, p: &Palette, title: &str) -> Rect {
    let r = ui
        .horizontal(|ui| {
            ui.add_space(4.0);
            widgets::section_header(ui, p, title)
        })
        .response
        .rect;
    ui.add_space(10.0);
    r
}

/// Voce dell'indice (`.idx button`).
fn index_item(ui: &mut Ui, p: &Palette, label: &str, current: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), focus::MIN_TARGET), Sense::click());
    let fill = if current {
        p.select
    } else if resp.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(radius::ROW), fill);
    let font = if current { widgets::font(Type::BodyStrong) } else { widgets::font(Type::Body) };
    let color = if current || resp.hovered() { p.text } else { p.text_2 };
    ui.painter().text(pos2(rect.min.x + 10.0, rect.center().y), egui::Align2::LEFT_CENTER, label, font, color);
    if resp.has_focus() {
        widgets::focus_ring_inside(ui.painter(), rect, p, radius::ROW);
    }
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, current, label));
    resp
}

fn appearance(ui: &mut Ui, p: &Palette, app: &App, s: &mut settings::Settings) {
    let l = &app.lingue;
    widgets::rows(ui, p, |rows| {
        rows.row("tema", &l.t("impostazioni-tema"), Some(&l.t("impostazioni-tema-nota")), |ui| {
            let options: Vec<_> = [(ThemeChoice::System, "tema-sistema"), (ThemeChoice::Light, "tema-chiaro"), (ThemeChoice::Dark, "tema-scuro")].into_iter().map(|(t, k)| (t, l.t(k), None)).collect();
            widgets::segmented(ui, p, "scelta_tema", &mut s.theme, &options);
        });
        rows.row("accento", &l.t("impostazioni-accento"), Some(&l.t("impostazioni-accento-nota")), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for a in Accent::ALL {
                let color = a.colors(p.theme).fill;
                if widgets::swatch(ui, p, color, s.accent == a, &l.t(a.key())).clicked() {
                    s.accent = a;
                }
            }
        });
        rows.stacked(Some(&l.t("impostazioni-colori-dati")), Some(&l.t("impostazioni-colori-dati-nota")), |ui| {
            data_palette_choice(ui, p, l, &mut s.data_palette);
        });
        rows.row("testo", &l.t("impostazioni-testo"), Some(&l.t("impostazioni-testo-nota")), |ui| {
            // Il cursore scrive un valore temporaneo; lo zoom cambia quando si lascia il pomello, perché uno
            // zoom che cambia sotto il puntatore sposterebbe il cursore stesso.
            let id = Id::new("quado_testo_trascinato");
            let mut v: f32 = ui.data(|d| d.get_temp(id)).unwrap_or(f32::from(s.text_scale));
            let r = widgets::slider(ui, p, "cursore_testo", &mut v, f32::from(TEXT_SCALE_MIN)..=f32::from(TEXT_SCALE_MAX), f32::from(TEXT_SCALE_STEP), 180.0, &l.t("impostazioni-testo"));
            if r.dragged() {
                ui.data_mut(|d| d.insert_temp(id, v));
            } else {
                ui.data_mut(|d| d.remove::<f32>(id));
                if r.changed() || r.drag_stopped() {
                    s.text_scale = v.round() as u16;
                }
            }
            ui.add_space(8.0);
            widgets::typed(ui, format!("{}%", app.fmt.count(v.round() as u64)), Type::Body, p.text);
        });
        rows.row("densita", &l.t("impostazioni-densita"), Some(&l.t("impostazioni-densita-nota")), |ui| {
            let options: Vec<_> = [(Density::Comfortable, "densita-comoda"), (Density::Compact, "densita-compatta")].into_iter().map(|(d, k)| (d, l.t(k), None)).collect();
            widgets::segmented(ui, p, "scelta_densita", &mut s.density, &options);
        });
        rows.row("animazioni", &l.t("impostazioni-animazioni"), Some(&l.t("impostazioni-animazioni-nota")), |ui| {
            let options: Vec<_> = [(Motion::System, "animazioni-windows"), (Motion::Off, "animazioni-spente")].into_iter().map(|(m, k)| (m, l.t(k), None)).collect();
            widgets::segmented(ui, p, "scelta_animazioni", &mut s.motion, &options);
        });
    });
}

/// Scelta dei colori dei dati: come un controllo segmentato, ogni voce con le quattro tinte del suo tema.
fn data_palette_choice(ui: &mut Ui, p: &Palette, l: &Lingue, value: &mut DataPalette) {
    let pad = 2.0;
    let items: Vec<(DataPalette, String)> = DataPalette::ALL.iter().map(|d| (*d, l.t(d.key()))).collect();
    let galleys: Vec<_> = items.iter().map(|(d, t)| ui.painter().layout_no_wrap(t.clone(), if *d == *value { widgets::medium(13.0) } else { widgets::sans(13.0) }, p.text)).collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 24.0 + 34.0).collect();
    let total = widths.iter().sum::<f32>() + pad * (items.len() + 1) as f32;
    let (outer, _) = ui.allocate_exact_size(vec2(total, focus::MIN_TARGET), Sense::hover());
    ui.painter().rect_filled(outer, CornerRadius::same(radius::CONTROL), p.field);
    let mut x = outer.min.x + pad;
    for (k, ((d, label), g)) in items.iter().zip(galleys).enumerate() {
        let r = Rect::from_min_size(pos2(x, outer.min.y + pad), vec2(widths[k], focus::MIN_TARGET - pad * 2.0));
        x += widths[k] + pad;
        let resp = ui.interact(r, Id::new(("quado_colori_dati", k)), Sense::click());
        let selected = *d == *value;
        if selected {
            let sh = p.shadow(Elevation::Segment);
            if sh != egui::epaint::Shadow::NONE {
                ui.painter().add(sh.as_shape(r, CornerRadius::same(radius::CONTROL - 2)));
            }
            ui.painter().rect_filled(r, CornerRadius::same(radius::CONTROL - 2), p.raised);
        }
        for (i, c) in d.colors(p.theme).data.iter().enumerate() {
            let b = Rect::from_min_size(pos2(r.min.x + 12.0 + i as f32 * 8.0, r.center().y - 6.0), vec2(6.0, 12.0));
            ui.painter().rect_filled(b, CornerRadius::same(2), *c);
        }
        let color = if selected || resp.hovered() { p.text } else { p.text_2 };
        ui.painter().galley(pos2(r.min.x + 12.0 + 34.0, r.center().y - g.size().y / 2.0), g, color);
        if resp.has_focus() {
            widgets::focus_ring_inside(ui.painter(), r.expand(1.0), p, radius::CONTROL);
        }
        if resp.clicked() {
            *value = *d;
        }
        resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, selected, label));
    }
}

fn explore_rows(ui: &mut Ui, p: &Palette, l: &Lingue, s: &mut settings::Settings) {
    widgets::rows(ui, p, |rows| {
        rows.row("esplora_vista", &l.t("impostazioni-esplora-vista"), None, |ui| {
            let options = [(ExploreView::MapAndList, l.t("esplora-mappa-ed-elenco"), Some(Icon::Explore)), (ExploreView::ListOnly, l.t("esplora-solo-elenco"), Some(Icon::List))];
            widgets::segmented(ui, p, "scelta_vista_esplora", &mut s.explore_view, &options);
        });
        rows.row("colonne", &l.t("impostazioni-colonne"), Some(&l.t("impostazioni-colonne-nota")), |ui| {
            let c: &mut Columns = &mut s.columns;
            egui::Grid::new("quado_colonne").num_columns(2).spacing(vec2(18.0, 0.0)).show(ui, |ui| {
                widgets::checkbox(ui, p, &mut c.share, &l.t("colonna-quota"), true);
                widgets::checkbox(ui, p, &mut c.files, &l.t("colonna-file"), true);
                ui.end_row();
                widgets::checkbox(ui, p, &mut c.date, &l.t("colonna-ultima-modifica"), true);
                widgets::checkbox(ui, p, &mut c.managed, &l.t("colonna-gestito-da"), true);
                ui.end_row();
            });
        });
        rows.row("barra_laterale", &l.t("impostazioni-barra-laterale"), None, |ui| {
            let options = [(false, l.t("barra-laterale-espansa"), None), (true, l.t("barra-laterale-compressa"), None)];
            widgets::segmented(ui, p, "scelta_barra_laterale", &mut s.sidebar_collapsed, &options);
        });
    });
}

fn language_rows(ui: &mut Ui, p: &Palette, app: &App, ch: &mut Changes) {
    let l = &app.lingue;
    let dir = app.lang_dir().map(|d| d.display().to_string()).unwrap_or_default();
    let note = l.ta("impostazioni-lingua-nota", &[("cartella", dir.into())]);
    widgets::rows(ui, p, |rows| {
        rows.row("lingua", &l.t("impostazioni-lingua"), Some(&note), |ui| {
            let current = l.available.iter().find(|a| a.tag == l.selected()).map_or_else(|| l.selected().to_owned(), |a| a.name.clone());
            widgets::combo("lingua", 160.0, current).show_ui(ui, |ui| {
                for a in &l.available {
                    let label = if a.user_file { format!("{}  ({})", a.name, l.t("impostazioni-lingua-file-utente")) } else { a.name.clone() };
                    if ui.selectable_label(a.tag == l.selected(), label).clicked() {
                        ch.new_lang = Some(a.tag.clone());
                    }
                }
            });
        });
        rows.stacked(None, None, |ui| {
            ui.horizontal_wrapped(|ui| {
                let has_dir = app.env.data_dir.is_some();
                if widgets::button(ui, p, &l.t("impostazioni-lingua-apri-cartella"), Some(Icon::Folder), ButtonKind::Secondary, has_dir).clicked() {
                    ch.open_dir = true;
                }
                if widgets::button(ui, p, &l.t("impostazioni-lingua-modello"), Some(Icon::Copy), ButtonKind::Secondary, has_dir).clicked() {
                    ch.template = true;
                }
                if widgets::button(ui, p, &l.t("impostazioni-lingua-ricarica"), Some(Icon::Refresh), ButtonKind::Secondary, true).clicked() {
                    ch.reload = true;
                }
            });
            if !l.errors.is_empty() {
                widgets::paragraph(ui, l.ta("impostazioni-lingua-errori", &[("n", l.errors.len().into())]), Type::BodyStrong, p.warn);
                for e in l.errors.iter().take(20) {
                    let name = e.file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    widgets::text(ui, format!("{name}:{}  {}", e.line, e.detail), mono(12.0), p.text_2);
                }
            }
        });
    });
}

fn units_rows(ui: &mut Ui, p: &Palette, l: &Lingue, s: &mut settings::Settings) {
    widgets::rows(ui, p, |rows| {
        rows.stacked(Some(&l.t("impostazioni-unita")), Some(&l.t("impostazioni-unita-nota")), |ui| {
            let options = [(Units::Explorer, l.t("unita-scelta-esplora"), None), (Units::Si, l.t("unita-scelta-si"), None)];
            widgets::segmented(ui, p, "scelta_unita", &mut s.units, &options);
        });
    });
}

fn about_rows(ui: &mut Ui, p: &Palette, app: &App) {
    let l = &app.lingue;
    let renderer = app.env.renderer.clone();
    let data = app.env.data_dir.as_ref().map(|d| d.display().to_string()).unwrap_or_default();
    widgets::rows(ui, p, |rows| {
        // Il marchio, con il nome di prima: la copia installata, il collegamento e la cartella dei dati della
        // versione precedente si chiamano ancora Quado.
        rows.stacked(None, None, |ui| {
            brand::lockup(ui, 20.0, p.text);
            ui.add_space(4.0);
            widgets::paragraph(ui, l.t("info-nome-nota"), Type::Callout, p.text_2);
        });
        rows.row("versione", &l.t("info-versione"), None, |ui| {
            widgets::typed(ui, env!("CARGO_PKG_VERSION"), Type::Body, p.text_2);
        });
        rows.row("grafica", &l.t("info-grafica"), None, |ui| {
            widgets::typed(ui, renderer, Type::Body, p.text_2);
        });
        rows.row("cartella_dati", &l.t("info-dati"), None, |ui| {
            widgets::text(ui, data, mono(12.0), p.text_2);
        });
        // Avvisi legali (GPLv3 §0 e §5(d), "Appropriate Legal Notices"): copyright, licenza e come leggerla,
        // nessuna garanzia, dove sta il codice sorgente. Il testo della licenza è nell'eseguibile.
        rows.stacked(Some(&l.t("info-licenza-titolo")), None, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            widgets::paragraph(ui, brand::COPYRIGHT, Type::Body, p.text);
            widgets::paragraph(ui, l.t("info-licenza"), Type::Callout, p.text_2);
            widgets::paragraph(ui, l.t("info-garanzia"), Type::Callout, p.text_2);
            widgets::paragraph(ui, l.t("info-terze-parti"), Type::Callout, p.text_2);
            let id = Id::new("quado_testo_licenza");
            let mut open = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
            ui.push_id("testo_licenza", |ui| widgets::disclosure(ui, p, &l.t("info-testo-licenza"), Type::Callout, &mut open));
            ui.ctx().data_mut(|d| d.insert_temp(id, open));
            if open {
                egui::Frame::new().fill(p.panel2).corner_radius(egui::CornerRadius::same(crate::tokens::radius::GROUP)).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ScrollArea::vertical().id_salt("quado_licenza").max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(brand::LICENSE_TEXT).font(mono(11.5)).color(p.text_2)).wrap());
                    });
                });
            }
        });
        rows.row("sorgente", &l.t("info-sorgente"), None, |ui| {
            ui.horizontal_wrapped(|ui| {
                widgets::text(ui, brand::SOURCE_URL, mono(12.0), p.text_2);
                if widgets::button(ui, p, &l.t("regole-copia-link"), Some(Icon::Copy), ButtonKind::Quiet, true).clicked() {
                    ui.ctx().copy_text(brand::SOURCE_URL.to_owned());
                    app.notice.set(l.t("avviso-comando-copiato"));
                }
            });
        });
    });
}

/// Anteprima (`.pvw`): barra del bilancio, tre righe e una piccola mappa con i colori e la densità scelti.
/// Le dimensioni sono quelle dell'unità scansionata, se c'è.
fn preview(ui: &mut Ui, app: &App, s: &settings::Settings) {
    let p = &Palette::new(app.palette(ui.ctx()).theme, s.accent, s.data_palette);
    let (l, f) = (&app.lingue, &app.fmt);
    let m = s.density.metrics();
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        widgets::section_header(ui, p, &l.t("impostazioni-anteprima"));
    });
    ui.add_space(10.0);
    let by = app.data.as_ref().map(|d| d.cls.by_manager);
    let values: [u64; 4] = by.map_or([40, 25, 6, 16], |g| [g.programs, g.you, g.windows, g.unclassified]);
    let used = app.data.as_ref().map(|d| d.used()).filter(|&u| u > 0);
    widgets::card(p).inner_margin(egui::Margin::same(18)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        // Barra del bilancio in piccolo: i quattro gestori, il non attribuito, il libero.
        let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 10.0), Sense::hover());
        let sum = values.iter().sum::<u64>().max(1) as f32;
        let (free_w, hatch_w) = (bar.width() * 0.1, bar.width() * 0.02);
        let avail = bar.width() - free_w - hatch_w - 2.0 * 5.0;
        let mut x = bar.min.x;
        for (g, v) in Gestore::ALL.iter().zip(values) {
            let w = (v as f32 / sum * avail).max(2.0);
            ui.painter().rect_filled(Rect::from_min_size(pos2(x, bar.min.y), vec2(w, bar.height())), CornerRadius::same(2), p.data_color(*g));
            x += w + 2.0;
        }
        widgets::unattributed(ui.painter(), Rect::from_min_size(pos2(x, bar.min.y), vec2(hatch_w, bar.height())), p);
        x += hatch_w + 2.0;
        ui.painter().rect_filled(Rect::from_min_max(pos2(x, bar.min.y), bar.max), CornerRadius::same(2), p.track);
        ui.add_space(12.0);
        // Righe con la densità scelta; la prima è selezionata, nel colore dell'accento.
        for (k, g) in [Gestore::App, Gestore::Mine, Gestore::Windows].into_iter().enumerate() {
            let w = ui.available_width();
            let v = values[g as usize];
            let sg = by.map(|_| ui.painter().layout_no_wrap(f.size(v), widgets::num(13.0), p.text));
            let sw = sg.as_ref().map_or(0.0, |g| g.size().x + 10.0);
            // I nomi vanno a capo: la riga cresce invece di troncarli.
            let name = widgets::galley(ui.painter(), l.t(g.key()), Type::BodyStrong, p.text, Some(w - 34.0 - sw));
            let sub = used.map(|u| {
                let t = l.ta("esplora-dimensione-quota", &[("dimensione", f.size(v).into()), ("quota", f.percent(v as f64 / u as f64).into())]);
                widgets::galley(ui.painter(), t, Type::Callout, p.text_2, Some(w - 34.0 - sw))
            });
            let content = name.size().y + sub.as_ref().map_or(0.0, |g| g.size().y + 1.0);
            let (rect, _) = ui.allocate_exact_size(vec2(w, m.row_two_line.max(content + 12.0)), Sense::hover());
            if k == 0 {
                ui.painter().rect_filled(rect, CornerRadius::same(radius::ROW), p.select);
            }
            widgets::manager_glyph(ui.painter(), pos2(rect.min.x + 12.0, rect.center().y), g, p);
            let nh = name.size().y;
            let h = nh + sub.as_ref().map_or(0.0, |g| g.size().y + 1.0);
            let y = rect.center().y - h / 2.0;
            ui.painter().galley(pos2(rect.min.x + 24.0, y), name, p.text);
            if let Some(g) = sub {
                ui.painter().galley(pos2(rect.min.x + 24.0, y + nh + 1.0), g, p.text_2);
            }
            if let Some(g) = sg {
                ui.painter().galley(pos2(rect.max.x - 8.0 - g.size().x, rect.center().y - g.size().y / 2.0), g, p.text);
            }
        }
        ui.add_space(12.0);
        // Mappa in piccolo: un'area per gestore, con l'etichetta e la forma nel colore dell'etichetta.
        let (map, _) = ui.allocate_exact_size(vec2(ui.available_width(), 90.0), Sense::hover());
        let w = map.width();
        let cells = [
            (Gestore::Mine, Rect::from_min_size(map.min, vec2(w * 0.46 - 1.0, 90.0))),
            (Gestore::App, Rect::from_min_size(pos2(map.min.x + w * 0.46 + 1.0, map.min.y), vec2(w * 0.30 - 2.0, 53.0))),
            (Gestore::Windows, Rect::from_min_size(pos2(map.min.x + w * 0.76 + 1.0, map.min.y), vec2(w * 0.24 - 1.0, 53.0))),
            (Gestore::Unknown, Rect::from_min_max(pos2(map.min.x + w * 0.46 + 1.0, map.min.y + 55.0), map.max)),
        ];
        // Nomi delle aree: l'elemento più grande di quel gestore nell'unità scansionata (sono dati, si possono
        // troncare), altrimenti il nome breve del gestore.
        let data_name = |g: Gestore| -> Option<String> {
            let d = app.data.as_ref()?;
            d.top.iter().find(|&&n| kbtext::node_gestore(&d.cls, n) == g).map(|&n| d.tree().name_lossy(n))
        };
        for (g, r) in cells {
            let (fill, text) = p.tile_color(g);
            if let Some(n) = data_name(g) {
                ui.painter().rect_filled(r, CornerRadius::same(radius::TILE_INNER), fill);
                widgets::glyph(ui.painter(), pos2(r.min.x + 10.0, r.min.y + 12.0), 8.0, g.shape(), text, p);
                let name = widgets::elided(ui.painter(), n, widgets::font(Type::Caption), text, r.width() - 24.0);
                ui.painter().galley(pos2(r.min.x + 18.0, r.min.y + 12.0 - name.size().y / 2.0), name, text);
                continue;
            }
            ui.painter().rect_filled(r, CornerRadius::same(radius::TILE_INNER), fill);
            widgets::glyph(ui.painter(), pos2(r.min.x + 11.0, r.min.y + 12.0), 8.0, g.shape(), text, p);
            // Come nella mappa vera, il nome resta dentro l'area: va a capo e ciò che non entra non si vede.
            let name = widgets::galley(ui.painter(), l.t(kbtext::short_key(g)), Type::Caption, text, Some(r.width() - 28.0));
            let first = name.rows.first().map_or(name.size().y, |row| row.height());
            ui.painter().with_clip_rect(r).galley(pos2(r.min.x + 20.0, r.min.y + 12.0 - first / 2.0), name, text);
        }
    });
}

/// Applica le scelte del frame: impostazioni salvate, lingua, file delle lingue, registro, cartella di
/// Rilievo.
fn apply(app: &mut App, s: settings::Settings, ch: Changes) {
    // Cancellazione del registro degli interventi, dopo la conferma (F-AC-6): il file non c'è più e la vista
    // Interventi resta vuota. Impostazioni e lingue restano.
    if ch.privacy.journal && let Some(dir) = app.env.data_dir.clone() {
        match journal::delete(&dir) {
            Ok(()) => app.notice.set(app.lingue.t("privacy-registro-cancellato")),
            Err(JournalError::Io(code)) => app.notice.set(app.lingue.ta("privacy-registro-errore", &[("codice", code.to_string().into())])),
        }
        app.procedures.history.invalidate();
    }
    // Cancellazione della cartella dei dati di Rilievo, dopo la conferma: quella di Quado resta com'è.
    if ch.privacy.old && let Some(old) = app.env.old_data_dir.clone() {
        match legacy::delete_old(&old) {
            Ok(()) => app.notice.set(app.lingue.t("privacy-rilievo-cancellata")),
            Err(code) => app.notice.set(app.lingue.ta("privacy-rilievo-errore", &[("codice", code.to_string().into())])),
        }
    }
    if let Some(tag) = ch.new_lang {
        app.set_language(&tag);
        app.settings.language = Some(tag);
        app.save_settings();
    }
    // La lingua scelta in questo frame resta quella appena salvata.
    let s = settings::Settings { language: app.settings.language.clone(), ..s };
    if s != app.settings {
        let units_changed = s.units != app.settings.units;
        let view_changed = s.explore_view != app.settings.explore_view;
        app.settings = s;
        if view_changed {
            app.explore.list_only = app.settings.explore_view == ExploreView::ListOnly;
        }
        if units_changed {
            let units = app.settings.units;
            app.fmt.units = units;
            app.search.filter = app.search.filters.to_filter(units, quado_scan_win::system::now_filetime());
            app.big.filter = app.big.filters.to_filter(units, quado_scan_win::system::now_filetime());
        }
        app.save_settings();
    }
    if ch.reload {
        app.reload_languages();
        app.notice.set(app.lingue.t("avviso-lingue-ricaricate"));
    }
    if let Some(dir) = app.lang_dir() {
        if ch.template || ch.open_dir {
            let _ = std::fs::create_dir_all(&dir);
        }
        if ch.template {
            let path = dir.join(TEMPLATE);
            match std::fs::write(&path, embedded(REFERENCE).unwrap_or_default()) {
                Ok(()) => {
                    // Il modello c'è anche se Esplora file non si apre: la cartella è scritta nella nota.
                    let _ = shell::reveal(&path.display().to_string());
                    app.notice.set(app.lingue.t("avviso-modello-creato"));
                }
                Err(e) => app.notice.set(app.lingue.ta("avviso-errore-file", &[("codice", e.raw_os_error().unwrap_or(0).to_string().into())])),
            }
        } else if ch.open_dir && let Err(e) = shell::reveal(&dir.display().to_string()) {
            // Se Esplora file non si apre lo si dice, con il codice, invece di non fare niente.
            app.notice.set(app.lingue.ta("avviso-shell-errore", &[("codice", format!("0x{:08X}", e.0 as u32).into())]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_and_privacy_lists_only_the_files_quado_writes() {
        // Nessun «Log tecnico»: Quado non scrive log su file. Una cartella `log` lasciata da altri non compare.
        let dir = std::env::temp_dir().join(format!("quado-privacy-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("log")).expect("cartella");
        std::fs::write(dir.join("log").join("a.txt"), b"x").expect("file");
        std::fs::write(dir.join(settings::FILE_NAME), b"tema=scuro\n").expect("impostazioni");
        let kinds: Vec<DataFile> = data_files(&dir).into_iter().map(|(k, _, _)| k).collect();
        assert_eq!(kinds, vec![DataFile::Settings]);
        for tag in ["it", "en"] {
            let text = embedded(tag).expect("catalogo");
            assert!(!text.contains(&format!("privacy-file-{}", "log")), "{tag}");
            // Il riquadro di Windows non si cita alla lettera (revisione dei testi, R15): vale con e senza firma.
            assert!(!text.contains("«Autore sconosciuto»") && !text.contains("\"Unknown publisher\""), "{tag}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
