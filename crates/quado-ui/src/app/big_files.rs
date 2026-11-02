// SPDX-License-Identifier: GPL-3.0-or-later
//! File grandi, variante A: come Esplora, con l'ispettore a destra su tutta l'altezza nel tono della
//! barra laterale. A sinistra i filtri in linea (gli stessi di Esplora e della ricerca), la riga che
//! dice cosa c'è in elenco e la tabella dei file più grandi del volume per dimensioni su disco.

use std::time::Duration;

use egui::{Align, Id, Layout, Ui, vec2};
use quado_model::view::Filter;

use super::data::{self, LargestOutcome, Task};
use super::explore::{self, FilterUi, Geometry};
use super::{App, inspector, shell};
use crate::index_scroll::IndexScroll;
use crate::tokens::{Palette, Type};
use crate::widgets;

#[derive(Default)]
pub struct BigState {
    pub filters: FilterUi,
    pub filter: Filter,
    task: Option<Task<LargestOutcome>>,
    pub outcome: Option<LargestOutcome>,
    pub scroll: IndexScroll,
    pub selected: Option<usize>,
    pub focus_pending: bool,
    pub geom: Geometry,
}

impl BigState {
    pub fn wait(&mut self) {
        if let Some(t) = self.task.take() {
            self.outcome = t.wait();
        }
    }

    pub fn invalidate(&mut self) {
        self.outcome = None;
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

    pub fn pending(&self) -> bool {
        self.task.is_some()
    }
}

pub fn show(app: &mut App, ui: &mut Ui, p: &Palette) {
    let Some(d) = app.data.as_ref() else { return };
    let st = &mut app.big;
    let fresh = st.outcome.as_ref().is_some_and(|o| o.filter == st.filter && o.generation == d.generation);
    if !fresh && st.task.is_none() {
        st.task = Some(data::spawn_largest(&d.model, &d.cls, st.filter.clone(), d.generation));
    }
    let split = shell::split(ui, p);
    let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(split.content).layout(Layout::top_down(Align::Min)));
    let ui = &mut lui;
    super::overview::interrupted_banner(app, ui, p);
    // Barra (`.exbar`): i filtri in linea, poi cosa c'è in elenco.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 6.0);
        if explore::filter_chips(ui, p, &app.lingue, &app.fmt, &mut app.big.filters, "file-grandi") {
            app.big.filter = app.big.filters.to_filter(app.fmt.units, quado_scan_win::system::now_filetime());
        }
    });
    ui.add_space(6.0);
    let (l, f) = (&app.lingue, &app.fmt);
    let n = app.big.outcome.as_ref().map_or(0, |o| o.files.len());
    let meta = l.ta("file-grandi-meta", &[("volume", d.volume.letter().into()), ("n", n.into()), ("testo", f.count(n as u64).into())]);
    widgets::paragraph(ui, meta, Type::Callout, p.text_2);
    // Una dimensione minima che non è un numero non filtra niente: lo si dice qui, non solo nel suo menu.
    let fu = &app.big.filters;
    if !fu.min_text.trim().is_empty() && fu.min_bytes(f.units).is_none() {
        widgets::paragraph(ui, l.t("filtro-dimensione-non-valida"), Type::Callout, p.warn);
    }
    ui.add_space(12.0);
    let rest = ui.available_rect_before_wrap();
    let mut tui = ui.new_child(egui::UiBuilder::new().max_rect(rest).layout(Layout::top_down(Align::Min)));
    let Some(d) = app.data.as_ref() else { return };
    let empty: Vec<u32> = Vec::new();
    let st = &mut app.big;
    let rows = st.outcome.as_ref().map_or(&empty, |o| &o.files);
    let mut activated = None;
    if rows.is_empty() {
        // Senza filtri l'elenco vuoto vuol dire nessun file misurato; con i filtri, che i filtri escludono tutto
        // (e l'azione è toglierli).
        let key = match (st.task.is_some(), st.filters.is_empty()) {
            (true, _) => "file-grandi-calcolo",
            (false, true) => "file-grandi-vuoto",
            (false, false) => "file-grandi-nessuno",
        };
        let action = (st.task.is_none() && !st.filters.is_empty()).then(|| app.lingue.t("filtro-azzera"));
        let msg = app.lingue.t(key);
        if shell::empty_state(&mut tui, p, rest, &msg, action.as_deref()) {
            let unit = app.big.filters.min_unit;
            app.big.filters = FilterUi { min_unit: unit, ..FilterUi::default() };
            app.big.filter = app.big.filters.to_filter(app.fmt.units, quado_scan_win::system::now_filetime());
        }
    } else {
        let (_, a) = explore::flat_table(&mut tui, p, &app.lingue, &app.fmt, d, Id::new("tabella-file-grandi"), rows, &mut st.scroll, &mut st.selected, &mut st.geom, &mut st.focus_pending);
        activated = a.and_then(|r| rows.get(r).copied());
    }
    let node = app.big.selected.and_then(|r| app.big.outcome.as_ref().and_then(|o| o.files.get(r).copied()));
    let mut rui = ui.new_child(egui::UiBuilder::new().max_rect(split.detail).layout(Layout::top_down(Align::Min)));
    inspector::show(app, &mut rui, p, node);
    if let Some(n) = activated {
        explore::show_in_folder(app, n);
    }
}
