// SPDX-License-Identifier: GPL-3.0-or-later
//! Scorrimento per indice di riga (05 D2).
//!
//! `ScrollArea` di egui tiene l'offset in pixel come `f32`: oltre circa 2 milioni di righe la
//! precisione non basta più e lo scorrimento salta (issue egui #1391). Qui la posizione è la prima
//! riga visibile in `f64`, con la frazione già scorsa; si disegnano solo le righe visibili e la
//! barra di scorrimento mappa la posizione del cursore su un indice di riga.

use egui::{Color32, CornerRadius, Id, Painter, Pos2, Rangef, Rect, Response, Sense, Ui, Vec2, pos2, vec2};

/// Larghezza della barra di scorrimento, in punti.
pub const BAR_WIDTH: f32 = 12.0;
/// Lunghezza minima del cursore, perché resti afferrabile anche con milioni di righe.
pub const MIN_THUMB: f32 = 28.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct IndexScroll {
    /// Prima riga visibile, con la frazione di riga già scorsa.
    pub top: f64,
    /// Durante il trascinamento: distanza tra il puntatore e l'inizio del cursore.
    grab: Option<f32>,
}

impl IndexScroll {
    pub fn max_top(rows: usize, visible: f64) -> f64 {
        (rows as f64 - visible).max(0.0)
    }

    pub fn clamp(&mut self, rows: usize, visible: f64) {
        self.top = self.top.clamp(0.0, Self::max_top(rows, visible));
    }

    pub fn scroll_rows(&mut self, delta_rows: f64, rows: usize, visible: f64) {
        self.top += delta_rows;
        self.clamp(rows, visible);
    }

    /// Porta `row` dentro la vista, a righe intere, muovendo il meno possibile.
    pub fn ensure_visible(&mut self, row: usize, rows: usize, visible: f64) {
        let whole = visible.floor().max(1.0);
        let row = row as f64;
        if row < self.top {
            self.top = row;
        } else if row + 1.0 > self.top + whole {
            self.top = row + 1.0 - whole;
        }
        self.clamp(rows, visible);
    }

    /// Posizione del cursore della barra, nell'intervallo `track`.
    pub fn thumb(&self, rows: usize, visible: f64, track: Rangef) -> Rangef {
        let len = Self::thumb_len(rows, visible, track);
        let max_top = Self::max_top(rows, visible);
        let t = if max_top > 0.0 { (self.top / max_top) as f32 } else { 0.0 };
        let start = track.min + (track.span() - len) * t;
        Rangef::new(start, start + len)
    }

    fn thumb_len(rows: usize, visible: f64, track: Rangef) -> f32 {
        if rows == 0 {
            return track.span();
        }
        let frac = (visible / rows as f64).min(1.0) as f32;
        (track.span() * frac).max(MIN_THUMB).min(track.span())
    }

    /// Riga corrispondente a un inizio del cursore in `thumb_start`.
    pub fn top_for_thumb(thumb_start: f32, rows: usize, visible: f64, track: Rangef) -> f64 {
        let len = Self::thumb_len(rows, visible, track);
        let free = (track.span() - len).max(f32::EPSILON);
        let t = ((thumb_start - track.min) / free).clamp(0.0, 1.0);
        f64::from(t) * Self::max_top(rows, visible)
    }
}

/// Cosa ha disegnato [`show_rows`].
pub struct RowsOutput {
    /// Area delle righe (senza la barra).
    pub body: Rect,
    /// Barra di scorrimento.
    pub bar: Rect,
    /// Righe visibili, intere: `first..end`.
    pub first: usize,
    pub end: usize,
    /// Righe che stanno nell'area, con la frazione.
    pub visible: f64,
    /// Interazione con l'area delle righe (clic, focus).
    pub response: Response,
    /// Riga sotto il puntatore, se c'è.
    pub hovered_row: Option<usize>,
    /// Riga cliccata in questo frame.
    pub clicked_row: Option<usize>,
}

/// Colori della barra.
#[derive(Clone, Copy, Debug)]
pub struct BarColors {
    pub track: Color32,
    pub thumb: Color32,
    pub thumb_active: Color32,
}

/// Disegna `rows` righe alte `row_height` nello spazio disponibile, con la barra a destra.
/// `paint_row(painter, riga, rettangolo)` disegna una riga visibile.
pub fn show_rows(
    ui: &mut Ui,
    id: Id,
    state: &mut IndexScroll,
    rows: usize,
    row_height: f32,
    colors: BarColors,
    mut paint_row: impl FnMut(&Painter, usize, Rect),
) -> RowsOutput {
    let outer = ui.available_rect_before_wrap();
    ui.allocate_rect(outer, Sense::hover());
    let body = Rect::from_min_max(outer.min, pos2(outer.max.x - BAR_WIDTH, outer.max.y));
    let bar = Rect::from_min_max(pos2(body.max.x, outer.min.y), outer.max);
    let visible = f64::from(body.height() / row_height);

    let response = ui.interact(body, id, Sense::click());

    // Rotella e touchpad: in righe, con la frazione.
    if response.hovered() {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            state.scroll_rows(-f64::from(dy / row_height), rows, visible);
        }
    }

    // Barra: trascinamento del cursore e clic sulla traccia (salta lì).
    let track = Rangef::new(bar.min.y + 2.0, bar.max.y - 2.0);
    // Non prende il focus: la tastiera resta alle righe anche dopo un clic sulla barra.
    let bar_resp = ui.interact(bar, id.with("bar"), Sense::CLICK | Sense::DRAG);
    let pointer = ui.input(|i| i.pointer.interact_pos());
    match (bar_resp.is_pointer_button_down_on(), pointer) {
        (true, Some(p)) => {
            let thumb = state.thumb(rows, visible, track);
            let grab = *state.grab.get_or_insert(if thumb.contains(p.y) { p.y - thumb.min } else { thumb.span() / 2.0 });
            state.top = IndexScroll::top_for_thumb(p.y - grab, rows, visible, track);
        }
        _ => state.grab = None,
    }
    // egui toglie il focus a chi lo ha quando si clicca altrove: dopo un clic sulla barra la tastiera
    // deve restare alle righe, se le aveva o se nessuno lo ha.
    if (bar_resp.is_pointer_button_down_on() || bar_resp.clicked() || bar_resp.drag_stopped())
        && ui.memory(|m| m.focused().is_none_or(|f| f == id))
    {
        ui.memory_mut(|m| m.request_focus(id));
    }
    state.clamp(rows, visible);

    let painter = ui.painter_at(outer);
    let first = state.top.floor() as usize;
    let offset = ((state.top - first as f64) * f64::from(row_height)) as f32;
    let end = (first + visible.ceil() as usize + 1).min(rows);
    let body_painter = painter.with_clip_rect(body);
    for row in first..end {
        let y = body.min.y - offset + (row - first) as f32 * row_height;
        let rect = Rect::from_min_size(pos2(body.min.x, y), vec2(body.width(), row_height));
        paint_row(&body_painter, row, rect);
    }

    // Barra disegnata dopo le righe.
    painter.rect_filled(bar, CornerRadius::ZERO, colors.track);
    if rows as f64 > visible {
        let thumb = state.thumb(rows, visible, track);
        let active = bar_resp.dragged() || bar_resp.hovered();
        let r = Rect::from_x_y_ranges(Rangef::new(bar.min.x + 3.0, bar.max.x - 3.0), thumb);
        painter.rect_filled(r, CornerRadius::same(3), if active { colors.thumb_active } else { colors.thumb });
    }

    let row_at = |p: Pos2| -> Option<usize> {
        if !body.contains(p) {
            return None;
        }
        let r = state.top + f64::from((p.y - body.min.y) / row_height);
        let r = r.floor() as usize;
        (r < rows).then_some(r)
    };
    let hovered_row = response.hover_pos().and_then(row_at);
    let clicked_row = if response.clicked() { response.interact_pointer_pos().and_then(row_at) } else { None };

    RowsOutput { body, bar, first, end, visible, response, hovered_row, clicked_row }
}

/// Spostamento della rotella in righe, utile ai test.
pub fn wheel_rows(delta: Vec2, row_height: f32) -> f64 {
    -f64::from(delta.y / row_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROWS: usize = 3_500_000;

    #[test]
    fn clamps_to_last_page() {
        let mut s = IndexScroll::default();
        s.scroll_rows(1e12, ROWS, 40.0);
        assert_eq!(s.top, (ROWS - 40) as f64);
        s.scroll_rows(-1e12, ROWS, 40.0);
        assert_eq!(s.top, 0.0);
    }

    #[test]
    fn fractional_scroll_is_exact_far_down() {
        // Con un offset in pixel f32 questa differenza andrebbe persa (05 D2).
        let mut s = IndexScroll { top: 3_400_000.0, grab: None };
        s.scroll_rows(0.25, ROWS, 40.0);
        assert_eq!(s.top, 3_400_000.25);
    }

    #[test]
    fn thumb_round_trip() {
        let track = Rangef::new(0.0, 1000.0);
        for top in [0.0, 1.0, 1234.0, 1_750_000.0, (ROWS - 40) as f64] {
            let s = IndexScroll { top, grab: None };
            let t = s.thumb(ROWS, 40.0, track);
            assert!(t.span() >= MIN_THUMB);
            let back = IndexScroll::top_for_thumb(t.min, ROWS, 40.0, track);
            // Un pixel di traccia vale circa 3.500 righe: il ritorno è esatto a meno di quello.
            assert!((back - top).abs() <= ROWS as f64 / 900.0, "{top} -> {back}");
        }
    }

    #[test]
    fn ensure_visible_moves_minimally() {
        let mut s = IndexScroll { top: 100.0, grab: None };
        s.ensure_visible(120, ROWS, 40.5);
        assert_eq!(s.top, 100.0);
        s.ensure_visible(150, ROWS, 40.5);
        assert_eq!(s.top, 111.0);
        s.ensure_visible(5, ROWS, 40.5);
        assert_eq!(s.top, 5.0);
    }
}
