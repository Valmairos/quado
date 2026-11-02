// SPDX-License-Identifier: GPL-3.0-or-later
//! Vista Interventi: in cima il risultato dell'ultimo spostamento di questa sessione
//! (`intervention_view::result_panel`), sotto lo storico dal registro degli interventi, una voce per
//! intervento: data e ora, tipo, origine, elementi con gli esiti, metodo, stima con il suo stato, variazione
//! del volume, differenza e spiegazione o "non spiegata". Le righe illeggibili del registro si dichiarano.
//!
//! Il registro si rilegge quando il file cambia (dimensione o data di modifica, controllate al più una volta
//! al secondo), non a ogni frame: [`History`] sta in `ProcedureState` e dà anche il numero di interventi
//! della barra laterale.

use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use egui::{Frame, Margin, Ui};
use quado_actions::journal::{self, Journal, JournalError};
use quado_actions::launch::Launch;
use quado_actions::procedures::EMPTY_RECYCLE_BIN;
use quado_model::intervention::{Comparison, EXPLAIN_THRESHOLD_BP, Explanation, InterventionKind, ItemOutcome, JournalEntry};
use quado_model::record::Estimate;

use super::App;
use super::kbtext;
use super::procedure_flow::{self, Tone};
use crate::format::Formatter;
use crate::i18n::Lingue;
use crate::tokens::{Palette, Type};
use crate::widgets::{self, ButtonKind, mono};

/// Ogni quanto si guarda se il file del registro è cambiato.
const CHECK_EVERY: Duration = Duration::from_secs(1);

/// Il registro degli interventi letto, con l'impronta del file (dimensione e data di modifica).
#[derive(Debug, Default)]
pub struct History {
    /// `None` senza cartella dei dati; altrimenti il registro o il codice dell'errore di lettura.
    journal: Option<Result<Journal, JournalError>>,
    /// Impronta all'ultima lettura: `None` se il file non c'era.
    stamp: Option<Option<(u64, Option<SystemTime>)>>,
    checked: Option<Instant>,
}

impl History {
    /// Rilegge il registro se il file è cambiato. Al più un controllo al secondo, salvo dopo
    /// [`History::invalidate`].
    pub fn refresh(&mut self, data_dir: Option<&Path>) {
        let Some(dir) = data_dir else {
            *self = Self::default();
            return;
        };
        if self.checked.is_some_and(|t| t.elapsed() < CHECK_EVERY) {
            return;
        }
        self.checked = Some(Instant::now());
        let stamp = std::fs::metadata(journal::path(dir)).ok().map(|m| (m.len(), m.modified().ok()));
        if self.journal.is_some() && self.stamp.as_ref() == Some(&stamp) {
            return;
        }
        self.stamp = Some(stamp);
        self.journal = Some(journal::read(dir));
    }

    /// Il registro è cambiato (intervento scritto, registro cancellato): al prossimo frame si rilegge.
    pub fn invalidate(&mut self) {
        self.checked = None;
        self.stamp = None;
    }

    /// Interventi registrati.
    pub fn count(&self) -> usize {
        match &self.journal {
            Some(Ok(j)) => j.entries.len(),
            _ => 0,
        }
    }

    pub fn journal(&self) -> Option<&Result<Journal, JournalError>> {
        self.journal.as_ref()
    }
}

// ---------------------------------------------------------------------------------------------
// Testi della verifica, comuni allo storico e all'esito delle procedure

/// Variazione con il segno: `+1,20 GB`, `-300 MB`, `0 byte`.
pub fn signed(l: &Lingue, f: &Formatter, v: i64) -> String {
    let s = f.size(v.unsigned_abs());
    match v.cmp(&0) {
        std::cmp::Ordering::Greater => l.ta("storico-piu", &[("valore", s.into())]),
        std::cmp::Ordering::Less => l.ta("storico-meno", &[("valore", s.into())]),
        std::cmp::Ordering::Equal => s,
    }
}

/// Punti base come percentuale con due decimali, come `Formatter::percent`: 512 → `5,12%`. Con un
/// decimale 501 punti base si leggerebbero "5,0%", sotto la soglia che superano.
fn percent(f: &Formatter, bp: u64) -> String {
    format!("{}%", f.decimal(bp as f64 / 100.0, 2))
}

/// Il valore della stima per la riga "Stimato"; `None` se non c'è o vale 0: si mostra solo la
/// variazione.
fn estimate_value(l: &Lingue, f: &Formatter, e: &Estimate) -> Option<String> {
    match *e {
        Estimate::Exact(x) if x > 0 => Some(f.size(x)),
        Estimate::Range { min, max } if min.max(max) > 0 => Some(l.ta("storico-intervallo", &[("minimo", f.size(min.min(max)).into()), ("massimo", f.size(min.max(max)).into())])),
        _ => None,
    }
}

/// Estremo della stima rispetto a cui si calcola la percentuale (il più vicino all'ottenuto).
fn reference(e: &Estimate, obtained: i64) -> Option<u64> {
    match *e {
        Estimate::Exact(x) if x > 0 => Some(x),
        Estimate::Range { min, max } => {
            let (lo, hi) = (min.min(max), min.max(max));
            Some(if obtained < i64::try_from(lo).unwrap_or(i64::MAX) { lo } else { hi }).filter(|&r| r > 0)
        }
        _ => None,
    }
}

fn explanation(l: &Lingue, f: &Formatter, e: &Explanation) -> String {
    let n = |k: u64| -> [(&str, crate::i18n::Arg<'static>); 2] { [("n", k.into()), ("testo", f.count(k).into())] };
    match *e {
        Explanation::SkippedOrFailed { items, bytes } => {
            let [a, b] = n(items);
            l.ta("storico-spiegazione-saltati", &[a, b, ("dimensione", f.size(bytes).into())])
        }
        Explanation::ExcludedNames { files, bytes } => {
            let [a, b] = n(files);
            l.ta("storico-spiegazione-altri-nomi", &[a, b, ("dimensione", f.size(bytes).into())])
        }
        Explanation::StillInBin { bytes } => l.ta("storico-spiegazione-nel-cestino", &[("dimensione", f.size(bytes).into())]),
        Explanation::OtherBinItems { items, bytes } => {
            let [a, b] = n(items);
            l.ta("storico-spiegazione-altri-elementi", &[a, b, ("dimensione", f.size(bytes).into())])
        }
        Explanation::StillOpen { files } => {
            let [a, b] = n(files);
            l.ta("storico-spiegazione-file-aperti", &[a, b])
        }
        Explanation::OtherActivity { bytes } => l.ta("storico-spiegazione-altri-processi", &[("variazione", signed(l, f, bytes).into())]),
    }
}

/// Stimato e variazione del volume, poi la differenza: entro il 5%, oppure spiegata voce per voce, oppure
/// "non spiegata". Con una stima non stimabile o pari a 0 c'è solo la variazione.
pub fn comparison_lines(l: &Lingue, f: &Formatter, c: &Comparison) -> Vec<(String, Tone)> {
    let y = signed(l, f, c.obtained);
    let mut out = vec![match estimate_value(l, f, &c.estimated) {
        Some(x) => (l.ta("storico-verifica", &[("stimato", x.into()), ("variazione", y.into())]), Tone::Text),
        None => (l.ta("storico-verifica-solo-variazione", &[("variazione", y.into())]), Tone::Text),
    }];
    match (c.difference, c.basis_points) {
        (Some(d), Some(bp)) if bp > EXPLAIN_THRESHOLD_BP => {
            out.push((l.ta("storico-differenza-oltre", &[("differenza", signed(l, f, d).into()), ("percentuale", percent(f, bp).into())]), Tone::Text));
            for e in &c.explanations {
                out.push((explanation(l, f, e), Tone::Text));
            }
            if let Some(u) = c.unexplained {
                let p = reference(&c.estimated, c.obtained).map(|r| (u128::from(u.unsigned_abs()) * 10_000).div_ceil(u128::from(r))).map_or(0, |bp| u64::try_from(bp).unwrap_or(u64::MAX));
                out.push((l.ta("storico-non-spiegata", &[("differenza", signed(l, f, u).into()), ("percentuale", percent(f, p).into())]), Tone::Warn));
            }
        }
        (Some(d), Some(bp)) => out.push((l.ta("storico-differenza-entro", &[("differenza", signed(l, f, d).into()), ("percentuale", percent(f, bp).into())]), Tone::Note)),
        (Some(d), None) => out.push((l.ta("storico-differenza", &[("differenza", signed(l, f, d).into())]), Tone::Note)),
        (None, _) => {}
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Una voce dello storico

/// Nome di una procedura dal suo codice nel registro; `None` per le procedure dentro un'app (il codice è
/// la regola).
fn procedure_key(code: &str) -> Option<&'static str> {
    let is = |l: Launch| l.procedure_code() == Some(code);
    if is(Launch::RecycleBin) {
        Some("storico-procedura-cestino-apri")
    } else if code == EMPTY_RECYCLE_BIN {
        Some("storico-procedura-cestino-svuota")
    } else if is(Launch::StorageSense) {
        Some("storico-procedura-archiviazione")
    } else if is(Launch::TempFiles) {
        Some("storico-procedura-file-temporanei")
    } else if code == procedure_flow::UNINSTALL {
        Some("storico-procedura-disinstallazione")
    } else {
        None
    }
}

/// Tipo di intervento.
pub fn kind_text(l: &Lingue, k: &InterventionKind) -> String {
    match k {
        InterventionKind::Recycle => l.t("storico-tipo-spostamento"),
        InterventionKind::Procedure(code) => match procedure_key(code) {
            Some(key) => l.ta("storico-tipo-procedura", &[("nome", l.t(key).into())]),
            None => l.t("storico-tipo-procedura-app"),
        },
        InterventionKind::Command(_) => l.t("storico-tipo-comando"),
    }
}

/// Nome della regola o della funzione dai file di lingua; il codice se non c'è un testo.
pub fn origin_text(l: &Lingue, origin: &str) -> String {
    [format!("regola-{origin}-nome"), format!("funzione-{origin}-nome"), format!("funzione-{origin}")]
        .into_iter()
        .find(|k| l.has(k))
        .map_or_else(|| origin.to_owned(), |k| l.t(&k))
}

/// Elementi: quanti, con gli esiti.
fn items_text(l: &Lingue, f: &Formatter, e: &JournalEntry) -> String {
    if e.items.is_empty() {
        return l.t(if e.kind == InterventionKind::Recycle { "storico-elementi-nessuno" } else { "storico-elementi-procedura" });
    }
    let mut counts = [0u64; 5];
    for i in &e.items {
        let k = match i.outcome {
            ItemOutcome::Recycled => 0,
            ItemOutcome::Skipped(_) => 1,
            ItemOutcome::Failed(_) => 2,
            ItemOutcome::NotInRecycleBin(_) => 3,
            ItemOutcome::NotReached => 4,
        };
        counts[k] += 1;
    }
    let keys = ["storico-esito-nel-cestino", "storico-esito-saltati", "storico-esito-falliti", "storico-esito-non-nel-cestino", "storico-esito-non-raggiunti"];
    let outcomes: Vec<String> = keys.iter().zip(counts).filter(|(_, c)| *c > 0).map(|(k, c)| l.ta(k, &[("n", c.into()), ("testo", f.count(c).into())])).collect();
    let n = e.items.len() as u64;
    l.ta("storico-elementi", &[("n", n.into()), ("testo", f.count(n).into()), ("esiti", outcomes.join(", ").into())])
}

/// Righe di una voce dello storico, nell'ordine in cui si mostrano (la prima è il titolo).
pub fn entry_lines(l: &Lingue, f: &Formatter, e: &JournalEntry) -> Vec<(String, Tone)> {
    let at = e.measures.first().map_or(e.id, |m| m.at);
    let mut out = vec![(
        l.ta("storico-voce", &[("data", f.date(at).into()), ("ora", f.time(at).into()), ("tipo", kind_text(l, &e.kind).into()), ("origine", origin_text(l, &e.origin).into())]),
        Tone::Text,
    )];
    out.push((items_text(l, f, e), Tone::Text));
    let method = if e.method.is_empty() { l.t("storico-metodo-nessuno") } else { e.method.clone() };
    out.push((l.ta("storico-metodo", &[("metodo", method.into()), ("volume", e.volume.clone().into())]), Tone::Text));
    out.push((kbtext::estimate_text(l, f, &e.estimate), Tone::Text));
    match (&e.comparison, e.measures.as_slice()) {
        (Some(c), _) => out.extend(comparison_lines(l, f, c)),
        (None, [m]) => out.push((l.ta("storico-una-misura", &[("ora", f.time(m.at).into()), ("libero", f.size(m.free).into())]), Tone::Note)),
        (None, []) => out.push((l.t("storico-nessuna-misura"), Tone::Note)),
        (None, _) => out.push((l.t("storico-senza-confronto"), Tone::Note)),
    }
    if let Some(b) = e.measures.last().and_then(|m| m.still_present) {
        out.push((l.ta("storico-ancora-presenti", &[("dimensione", f.size(b).into())]), Tone::Text));
    }
    out
}

/// Una voce dello storico (`.arow`): il titolo con data, tipo e origine; sotto le righe con il loro tono. Il
/// metodo è un dato (URI o comando): IBM Plex Mono.
fn entry_block(ui: &mut Ui, p: &Palette, l: &Lingue, f: &Formatter, e: &JournalEntry) {
    // A destra la variazione dello spazio libero, la risposta della voce (`.arow .sz`), se c'è un confronto.
    let figure = e.comparison.as_ref().map(|c| signed(l, f, c.obtained));
    Frame::new().inner_margin(Margin { left: 20, right: 20, top: 14, bottom: 14 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let w = ui.available_width();
        let fg = figure.map(|s| ui.painter().layout_no_wrap(s, widgets::num_medium(15.0), p.text));
        let fw = fg.as_ref().map_or(0.0, |g| g.size().x + 24.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(egui::vec2(w - fw, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                let lines = entry_lines(l, f, e);
                if let Some((title, _)) = lines.first() {
                    widgets::paragraph(ui, title, Type::BodyStrong, p.text);
                    ui.add_space(2.0);
                }
                for (k, (s, tone)) in lines.iter().enumerate().skip(1) {
                    let color = match tone {
                        Tone::Text => p.text_2,
                        Tone::Note => p.text_3,
                        Tone::Warn => p.warn,
                    };
                    if k == 2 {
                        ui.add(egui::Label::new(egui::RichText::new(s).font(mono(12.0)).color(color)).wrap());
                    } else {
                        widgets::paragraph(ui, s, Type::Callout, color);
                    }
                }
            });
            if let Some(g) = fg {
                let (r, _) = ui.allocate_exact_size(egui::vec2(fw, g.size().y), egui::Sense::hover());
                let gw = g.size().x;
                ui.painter().galley(egui::pos2(r.max.x - gw, r.min.y), g, p.text);
            }
        });
    });
}

/// Larghezza massima della pagina degli interventi, margini compresi: si legge come un elenco.
const PAGE_MAX: f32 = 1000.0;

/// Interventi (variante A): in cima il risultato dell'ultimo spostamento, poi lo storico come un gruppo di voci
/// separate da filetti, dal più recente. Senza interventi, lo stato vuoto con una sola azione: gli interventi
/// possibili della Panoramica.
pub fn show(app: &mut App, ui: &mut Ui, p: &Palette) {
    app.procedures.history.refresh(app.env.data_dir.as_deref());
    let mut to_overview = false;
    widgets::page(ui, PAGE_MAX, |ui| {
        ui.add_space(12.0);
        let had_result = super::intervention_view::result_panel(app, ui, p);
        if had_result {
            ui.add_space(32.0);
        }
        let (l, f) = (&app.lingue, &app.fmt);
        // Testata dello storico (`.sgrp > h2`): titolo, poi dove sta il registro.
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            widgets::section_header(ui, p, &l.t("storico-titolo"));
        });
        if let Some(dir) = &app.env.data_dir {
            let path = journal::path(dir).display().to_string();
            ui.horizontal_top(|ui| {
                ui.add_space(4.0);
                widgets::paragraph(ui, l.ta("storico-sottotitolo", &[("percorso", path.into())]), Type::Callout, p.text_2);
            });
        }
        ui.add_space(12.0);
        match app.procedures.history.journal() {
            None => {
                empty_card(ui, p, &l.t("storico-non-disponibile"), None);
            }
            Some(Err(JournalError::Io(code))) => {
                widgets::banner(ui, p, &l.ta("storico-errore", &[("codice", code.to_string().into())]));
            }
            Some(Ok(j)) => {
                if j.unreadable_lines > 0 {
                    let n = j.unreadable_lines as u64;
                    widgets::banner(ui, p, &l.ta("storico-illeggibili", &[("n", n.into()), ("testo", f.count(n).into())]));
                    ui.add_space(12.0);
                }
                if j.entries.is_empty() {
                    // Con il risultato in vista l'azione c'è già: lo stato vuoto dice solo cosa manca.
                    let action = (!had_result).then(|| l.t("storico-vuoto-azione"));
                    to_overview = empty_card(ui, p, &l.t("storico-vuoto"), action.as_deref());
                } else {
                    let n = j.entries.len() as u64;
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        widgets::typed(ui, l.ta("storico-numero", &[("n", n.into()), ("testo", f.count(n).into())]), Type::Caption, p.text_2);
                    });
                    ui.add_space(6.0);
                    widgets::bare_card(p).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 0.0;
                        // Dal più recente; un filetto rientrato tra una voce e l'altra.
                        for (k, e) in j.entries.iter().rev().enumerate() {
                            if k > 0 {
                                let y = ui.cursor().min.y;
                                let r = ui.max_rect();
                                widgets::inset_separator(ui.painter(), r, y - 0.5, 20.0, p);
                            }
                            ui.push_id(("storico_voce", k), |ui| entry_block(ui, p, l, f, e));
                        }
                    });
                }
            }
        }
    });
    if to_overview {
        app.go(super::Screen::Overview);
    }
}

/// Card dello stato vuoto: il messaggio e, sotto, al più un'azione. Vero se l'azione è stata scelta.
fn empty_card(ui: &mut Ui, p: &Palette, message: &str, action: Option<&str>) -> bool {
    let mut clicked = false;
    widgets::card(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::paragraph(ui, message, Type::Body, p.text_2);
        if let Some(a) = action {
            ui.add_space(12.0);
            clicked = widgets::button(ui, p, a, None, ButtonKind::Secondary, true).clicked();
        }
    });
    clicked
}
