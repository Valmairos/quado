// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows e sistema (F-SY-1, F-SY-2, F-SY-3), variante A: una pagina di gruppi come Impostazioni. Le voci di
//! sistema con la misura o "non disponibile" e il motivo, la riga della regola e lo stato; un clic apre la
//! voce in Esplora. Il Cestino misurato in due modi, rientrato sotto la sua voce; la configurazione letta in
//! sola lettura; le fonti lette durante la scansione con il loro esito. Ogni riga ha il glifo della provenienza.

use egui::{CornerRadius, Rect, Sense, Ui, pos2, vec2};
use quado_kb::extract::{ConfigOrigin, ExtractError, FormatIssue, TrackProgs, UserAssist};
use quado_kb::{Classification, Match, engine};
use quado_model::Tree;
use quado_model::record::State;

use super::kbtext;
use super::{App, Screen};
use crate::format::Formatter;
use crate::i18n::Lingue;
use crate::icons::{self, Icon};
use crate::tokens::{self, Palette, Type};
use crate::widgets::{self, Evidence};

/// Motivo di un estrattore non riuscito, come testo.
pub fn extract_error(l: &Lingue, e: &ExtractError) -> String {
    match e {
        ExtractError::NotPresent => l.t("estrattore-non-presente"),
        ExtractError::Io(5) => l.t("estrattore-accesso-negato"),
        ExtractError::CloudPlaceholder => l.t("estrattore-segnaposto-cloud"),
        ExtractError::Io(c) => l.ta("estrattore-errore", &[("codice", format!("{c}").into())]),
        ExtractError::Format(f) => {
            let what = match f {
                FormatIssue::Truncated => l.t("formato-troncato"),
                FormatIssue::Unexpected => l.t("formato-inatteso"),
                FormatIssue::Empty => l.t("formato-vuoto"),
                FormatIssue::TooDeep => l.t("formato-troppo-annidato"),
                FormatIssue::Encoding => l.t("formato-codifica"),
                FormatIssue::TooLarge => l.t("formato-troppo-grande"),
                FormatIssue::MissingField(x) => l.ta("formato-campo-mancante", &[("campo", x.code().into())]),
                FormatIssue::WrongType(x) => l.ta("formato-tipo-diverso", &[("campo", x.code().into())]),
                FormatIssue::BadValue(x) => l.ta("formato-valore", &[("campo", x.code().into())]),
            };
            l.ta("estrattore-non-riuscito", &[("motivo", what.into())])
        }
    }
}

/// Una riga di un gruppo (`.ledger .lr`): glifo della provenienza, etichetta, valore; entrambi a capo se
/// serve. Un filetto rientrato la separa dalla riga sopra.
fn row(ui: &mut Ui, p: &Palette, l: &Lingue, e: Evidence, label: &str, value: &str) {
    separator(ui, p);
    egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::key_value(ui, p, Some((e, &l.t(e.key()))), label, value);
    });
}

/// La prossima riga è la prima del gruppo: niente filetto sopra.
fn first_row_id() -> egui::Id {
    egui::Id::new("sistema_prima_riga")
}

/// Filetto tra due righe di un gruppo, rientrato come le righe di Impostazioni; non sopra la prima.
fn separator(ui: &mut Ui, p: &Palette) {
    let first = ui.ctx().data_mut(|d| std::mem::take(d.get_temp_mut_or_default::<bool>(first_row_id())));
    if !first {
        let y = ui.cursor().min.y;
        widgets::inset_separator(ui.painter(), ui.max_rect(), y - 0.5, 20.0, p);
    }
}

/// Gruppo della variante A (`.srows` con la testata `.gh`): titolo, sottotitolo facoltativo, righe.
fn group(ui: &mut Ui, p: &Palette, title: &str, subtitle: Option<&str>, add: impl FnOnce(&mut Ui)) {
    widgets::bare_card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 20, top: 18, bottom: 10 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            widgets::typed(ui, title, Type::Headline, p.text);
            if let Some(s) = subtitle {
                widgets::paragraph(ui, s, Type::Callout, p.text_2);
            }
        });
        ui.ctx().data_mut(|d| d.insert_temp(first_row_id(), true));
        add(ui);
        ui.add_space(8.0);
    });
}

fn on_off(l: &Lingue, b: bool) -> String {
    l.t(if b { "valore-attivo" } else { "valore-non-attivo" })
}

/// Voce di sistema riconosciuta da una regola: stato e spiegazione.
fn match_at<'a>(cls: &'a Classification, tree: &Tree, path: &str) -> Option<(u32, Option<&'a Match>)> {
    let n = engine::find(tree, path)?;
    Some((n, cls.match_of(n).map(|m| &cls.matches[m])))
}

/// Larghezza massima della pagina, margini compresi.
const PAGE_MAX: f32 = 1000.0;

/// Voce di sistema (`.trow`): nome, riga della regola, stato con icona e parola; a destra la misura e, se la
/// voce c'è, la freccia: tutta la riga apre la voce in Esplora. Restituisce la risposta della riga.
fn item_row(ui: &mut Ui, p: &Palette, name: &str, line: &str, state: Option<(&State, String)>, measure: &str, found: bool) -> egui::Response {
    let w = ui.available_width();
    let painter = ui.painter().clone();
    let mg = painter.layout_no_wrap(measure.to_owned(), widgets::num(13.0), if found { p.text } else { p.text_2 });
    let right_w = mg.size().x + if found { 14.0 + 14.0 } else { 0.0 };
    let tw = (w - 20.0 - 20.0 - right_w - 16.0).max(120.0);
    let ng = widgets::galley(&painter, name, Type::BodyStrong, p.text, Some(tw));
    let lg = (!line.is_empty()).then(|| widgets::galley(&painter, line, Type::Callout, p.text_2, Some(tw)));
    let tone = |s: &State| if matches!(s, State::ToVerify(_)) { p.warn } else { p.text_2 };
    let sg = state.as_ref().map(|(s, label)| (widgets::galley(&painter, label.clone(), Type::Callout, tone(s), None), kbtext::state_icon(s), tone(s)));
    let h = 12.0 + ng.size().y + lg.as_ref().map_or(0.0, |g| g.size().y + 2.0) + sg.as_ref().map_or(0.0, |g| g.0.size().y + 4.0) + 12.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), if found { Sense::click() } else { Sense::hover() });
    let inner = rect.shrink2(vec2(8.0, 2.0));
    if found && resp.hovered() {
        painter.rect_filled(inner, CornerRadius::same(tokens::radius::ROW), p.hover);
    }
    let x = rect.min.x + 20.0;
    let mut y = rect.min.y + 12.0;
    let first = ng.rows.first().map_or(ng.size().y, |r| r.height());
    let nh = ng.size().y;
    painter.galley(pos2(x, y), ng, p.text);
    let right = rect.max.x - 20.0;
    if found {
        icons::paint(&painter, Rect::from_center_size(pos2(right - 7.0, y + first / 2.0), vec2(14.0, 14.0)), Icon::ChevronRight, p.text_2);
    }
    let mw = mg.size().x;
    let mx = right - if found { 28.0 } else { 0.0 } - mw;
    painter.galley(pos2(mx, y + (first - mg.size().y) / 2.0), mg, p.text);
    y += nh + 2.0;
    if let Some(g) = lg {
        let gh = g.size().y;
        painter.galley(pos2(x, y), g, p.text_2);
        y += gh + 4.0;
    }
    if let Some((g, icon, c)) = sg {
        let gh = g.size().y;
        icons::paint(&painter, Rect::from_center_size(pos2(x + 7.0, y + gh / 2.0), vec2(14.0, 14.0)), icon, c);
        painter.galley(pos2(x + 21.0, y), g, c);
    }
    if resp.has_focus() {
        widgets::focus_ring_inside(ui.painter(), inner, p, tokens::radius::ROW);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, found, name));
    resp
}

pub fn show(app: &mut App, ui: &mut Ui, p: &Palette) {
    let mut open: Option<u32> = None;
    let see_rule: Option<usize> = None;
    widgets::page(ui, PAGE_MAX, |ui| {
        {
            let Some(d) = &app.data else { return };
            let (l, f) = (&app.lingue, &app.fmt);
            ui.add_space(12.0);
            ui.horizontal_top(|ui| {
                ui.add_space(4.0);
                widgets::paragraph(ui, l.t("sistema-sottotitolo"), Type::Body, p.text_2);
            });
            ui.add_space(16.0);
            let t = d.tree();
            let ev = &d.evidence;
            let root = d.volume.root.clone();
            let windows = ev.folders.windows.clone().unwrap_or_else(|| format!(r"{root}Windows"));
            let sid = ev.user_sid.clone().unwrap_or_default();
            // Voci di F-SY-1. Il percorso decide la misura; la regola stato, spiegazione e procedura.
            let items: [(&str, String); 9] = [
                ("sistema-voce-paging", format!(r"{root}pagefile.sys")),
                ("sistema-voce-swap", format!(r"{root}swapfile.sys")),
                ("sistema-voce-ibernazione", format!(r"{root}hiberfil.sys")),
                ("sistema-voce-winsxs", format!(r"{windows}\WinSxS")),
                ("sistema-voce-softwaredistribution", format!(r"{windows}\SoftwareDistribution")),
                ("sistema-voce-installer", format!(r"{windows}\Installer")),
                ("sistema-voce-driverstore", format!(r"{windows}\System32\DriverStore\FileRepository")),
                ("sistema-voce-cestino", format!(r"{root}$Recycle.Bin\{sid}")),
                ("sistema-voce-windows-old", format!(r"{root}Windows.old")),
            ];
            group(ui, p, &l.t("sistema-voci"), None, |ui| {
                for (key, path) in &items {
                    let found = match_at(&d.cls, t, path);
                    let name = l.t(key);
                    let measure = match &found {
                        Some((n, _)) => f.size(t.alloc(*n)),
                        None => l.t("sistema-non-presente"),
                    };
                    let m = found.as_ref().and_then(|(_, m)| *m);
                    let line = m.map(|m| kbtext::rule_line(l, f, &d.kb, m)).unwrap_or_default();
                    let state = m.map(|m| (&m.record.state, kbtext::state_label(l, &m.record.state)));
                    separator(ui, p);
                    let resp = ui.push_id(("voce", *key), |ui| item_row(ui, p, &name, &line, state, &measure, found.is_some())).inner;
                    if found.is_some() && (resp.clicked() || (resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))) {
                        open = found.as_ref().map(|(n, _)| *n);
                    }
                    if *key == "sistema-voce-cestino"
                        && let Some((n, _)) = &found
                    {
                        // F-SY-2: le due misure del Cestino e le cartelle orfane come anomalia, rientrate sotto la voce.
                        let windows_count = match &ev.recycle {
                            Ok(q) => l.ta("sistema-cestino-windows", &[("valore", f.size(q.bytes).into()), ("n", q.items.into()), ("testo", f.count(q.items).into())]),
                            Err(e) => extract_error(l, e),
                        };
                        sub_row(ui, p, l, Evidence::Source, &l.t("sistema-cestino-secondo-windows"), &windows_count);
                        sub_row(ui, p, l, Evidence::Measured, &l.t("sistema-cestino-su-disco"), &f.size(t.alloc(*n)));
                        let orphans = d.cls.matches.iter().find(|m| d.kb.rules[m.rule].id == "cestino-orfane");
                        if let Some(o) = orphans {
                            sub_row(ui, p, l, Evidence::Measured, &l.t("sistema-cestino-orfane"), &kbtext::rule_line(l, f, &d.kb, o));
                        }
                    }
                }
                // Voci non misurabili senza amministratore: mai 0 (F-SY-1).
                for key in ["sistema-voce-copie-shadow", "sistema-voce-archiviazione-riservata"] {
                    row(ui, p, l, Evidence::Unavailable, &l.t(key), &l.t("sistema-non-disponibile-admin"));
                }
                if let Some(b) = d.balance {
                    let m = b.metadata;
                    let mft = m.mft.map_or_else(|| l.t("sistema-non-disponibile-fs"), |x| f.size(x));
                    row(ui, p, l, if m.mft.is_some() { Evidence::Measured } else { Evidence::Unavailable }, &l.t("sistema-voce-mft"), &mft);
                    row(ui, p, l, Evidence::Measured, &l.t("sistema-voce-indici"), &f.size(m.dir_indexes));
                    let usn = m.usn_journal.map_or_else(|| l.t("sistema-non-disponibile-fs"), |x| f.size(x));
                    row(ui, p, l, if m.usn_journal.is_some() { Evidence::Measured } else { Evidence::Unavailable }, &l.t("sistema-voce-usn"), &usn);
                }
                egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 20, top: 10, bottom: 4 }).show(ui, |ui| {
                    widgets::paragraph(ui, l.t("sistema-nota-procedure"), Type::Callout, p.text_2);
                });
            });
            ui.add_space(24.0);
            config_card(app, ui, p, f);
            ui.add_space(24.0);
            sources_card(app, ui, p);
        }
    });
    if let Some(n) = open {
        super::explore::show_in_folder(app, n);
    }
    if let Some(r) = see_rule {
        app.rules_view.selected = Some(r);
        app.rules_view.scroll_to = true;
        app.go(Screen::Rules);
    }
}

/// Riga di una misura che appartiene alla voce sopra (le due misure del Cestino): rientrata sotto il nome.
fn sub_row(ui: &mut Ui, p: &Palette, l: &Lingue, e: Evidence, label: &str, value: &str) {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(20, 0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::key_value_indented(ui, p, Some((e, &l.t(e.key()))), label, value, 20.0);
    });
}

/// Configurazione letta in sola lettura (F-SY-3).
fn config_card(app: &App, ui: &mut Ui, p: &Palette, f: &Formatter) {
    let Some(d) = &app.data else { return };
    let l = &app.lingue;
    let ev = &d.evidence;
    group(ui, p, &l.t("configurazione-titolo"), Some(&l.t("configurazione-sottotitolo")), |ui| {
        match &ev.power {
            Ok(pw) => {
                row(ui, p, l, Evidence::Source, &l.t("configurazione-ibernazione"), &on_off(l, pw.hibernate));
                let fs = pw.fast_startup.map_or_else(|| l.t("valore-non-registrato"), |b| on_off(l, b));
                row(ui, p, l, Evidence::Source, &l.t("configurazione-avvio-rapido"), &fs);
                let crit = match pw.critical_battery_action {
                    Some(a) => l.t(&format!("configurazione-batteria-{}", a.min(4))),
                    None => l.t("valore-non-registrato"),
                };
                row(ui, p, l, Evidence::Source, &l.t("configurazione-batteria-critica"), &crit);
            }
            Err(e) => row(ui, p, l, Evidence::Unavailable, &l.t("configurazione-alimentazione"), &extract_error(l, e)),
        }
        match &ev.storage_sense {
            Ok(s) if s.configured => {
                row(ui, p, l, Evidence::Source, &l.t("configurazione-sensore"), &on_off(l, s.enabled));
                let freq = match s.frequency_days {
                    Some(0) => l.t("configurazione-sensore-poco-spazio"),
                    Some(n) => l.ta("configurazione-giorni", &[("n", u64::from(n).into()), ("testo", f.count(u64::from(n)).into())]),
                    None => l.t("valore-non-registrato"),
                };
                row(ui, p, l, Evidence::Source, &l.t("configurazione-sensore-frequenza"), &freq);
                let days = |v: Option<u32>| v.map_or_else(|| l.t("configurazione-mai"), |n| l.ta("configurazione-giorni", &[("n", u64::from(n).into()), ("testo", f.count(u64::from(n)).into())]));
                row(ui, p, l, Evidence::Source, &l.t("configurazione-sensore-cestino"), &days(s.recycle_days));
                row(ui, p, l, Evidence::Source, &l.t("configurazione-sensore-download"), &days(s.downloads_days));
            }
            Ok(_) => row(ui, p, l, Evidence::Source, &l.t("configurazione-sensore"), &l.t("configurazione-mai-configurato")),
            Err(e) => row(ui, p, l, Evidence::Unavailable, &l.t("configurazione-sensore"), &extract_error(l, e)),
        }
        match &ev.bin {
            Ok(b) => {
                let cap = b.max_capacity_mib.map_or_else(|| l.t("configurazione-capacita-predefinita"), |m| f.size(u64::from(m) << 20));
                row(ui, p, l, Evidence::Source, &l.t("configurazione-cestino-capacita"), &cap);
                row(ui, p, l, Evidence::Source, &l.t("configurazione-cestino-elimina-subito"), &on_off(l, b.nuke_on_delete));
            }
            Err(e) => row(ui, p, l, Evidence::Unavailable, &l.t("configurazione-cestino-capacita"), &extract_error(l, e)),
        }
        let track = match ev.track_progs {
            TrackProgs::On => l.t("valore-attivo"),
            TrackProgs::Off => l.t("configurazione-tracciamento-spento"),
            TrackProgs::Absent => l.t("valore-non-registrato"),
            TrackProgs::Unexpected => l.t("formato-inatteso"),
            TrackProgs::Unreadable(c) => l.ta("estrattore-errore", &[("codice", format!("{c}").into())]),
        };
        row(ui, p, l, Evidence::Source, &l.t("configurazione-tracciamento"), &track);
        match &ev.restore {
            Ok(b) => row(ui, p, l, Evidence::Source, &l.t("configurazione-ripristino"), &on_off(l, *b)),
            Err(e) => row(ui, p, l, Evidence::Unavailable, &l.t("configurazione-ripristino"), &extract_error(l, e)),
        }
        for (key, tp) in [("configurazione-cache-npm", &ev.npm_cache), ("configurazione-store-pnpm", &ev.pnpm_store)] {
            let origin = l.t(match tp.origin {
                ConfigOrigin::Environment => "configurazione-origine-ambiente",
                ConfigOrigin::Npmrc => "configurazione-origine-npmrc",
                ConfigOrigin::Default => "configurazione-origine-predefinito",
            });
            row(ui, p, l, Evidence::Source, &l.t(key), &format!("{} ({origin})", tp.path));
        }
    });
}

/// Fonti lette durante la scansione, con il loro esito (06 §2.3).
fn sources_card(app: &App, ui: &mut Ui, p: &Palette) {
    let Some(d) = &app.data else { return };
    let (l, f) = (&app.lingue, &app.fmt);
    let ev = &d.evidence;
    let n = |x: usize| -> (crate::i18n::Arg<'static>, crate::i18n::Arg<'static>) { (x.into(), f.count(x as u64).into()) };
    group(ui, p, &l.t("fonti-titolo"), None, |ui| {
        let item = |ui: &mut Ui, key: &str, r: Result<String, &ExtractError>| match r {
            Ok(s) => row(ui, p, l, Evidence::Source, &l.t(key), &s),
            Err(e) => row(ui, p, l, Evidence::Unavailable, &l.t(key), &extract_error(l, e)),
        };
        item(
            ui,
            "fonti-disinstallazione",
            ev.uninstall.as_ref().map(|u| {
                let (a, b) = n(u.entries.len());
                let (c, e) = n(u.skipped.len());
                l.ta("fonti-voci", &[("n", a), ("testo", b), ("saltate", c), ("saltate_testo", e)])
            }),
        );
        item(ui, "fonti-msix", ev.msix.as_ref().map(|m| {
            let (a, b) = n(m.len());
            l.ta("fonti-pacchetti", &[("n", a), ("testo", b)])
        }));
        item(ui, "fonti-steam", ev.steam.as_ref().map(|s| {
            let (a, b) = n(s.apps.len());
            l.ta("fonti-giochi", &[("n", a), ("testo", b)])
        }));
        item(ui, "fonti-epic", ev.epic.as_ref().map(|e| {
            let (a, b) = n(e.items.len());
            l.ta("fonti-installazioni", &[("n", a), ("testo", b)])
        }));
        item(ui, "fonti-capcut", ev.capcut.as_ref().map(|v| l.ta("fonti-versione", &[("versione", v.clone().into())])));
        let ua = match &ev.userassist {
            Ok(UserAssist::NotTracked(_)) => Ok(l.t("fonti-userassist-non-usato")),
            Ok(UserAssist::Entries(v)) => {
                let (a, b) = n(v.len());
                Ok(l.ta("fonti-voci-semplici", &[("n", a), ("testo", b)]))
            }
            Err(e) => Err(e),
        };
        item(ui, "fonti-userassist", ua);
    });
}
