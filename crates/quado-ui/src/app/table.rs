// SPDX-License-Identifier: GPL-3.0-or-later
//! Parti comuni delle tabelle virtualizzate (F-EX-2, F-EX-5): colonne, intestazione, tasti,
//! percorsi troncati all'inizio.

use std::sync::Arc;

use egui::{Color32, FontId, Galley, Key, Painter, Rect, Stroke, Ui, pos2};

use crate::i18n::Lingue;
use crate::tokens::{self, Palette, Type};
use crate::widgets;

/// Altezza dell'intestazione delle colonne. L'altezza delle righe viene dalla densità: [`row_height`].
pub const HEADER_H: f32 = 30.0;

/// Altezza di una riga di tabella con la densità in uso (40 comoda, 32 compatta).
pub fn row_height(ui: &Ui) -> f32 {
    tokens::metrics(ui.ctx()).row
}

/// Una colonna: chiave dell'intestazione, larghezza (0 = prende il resto), allineamento a destra.
#[derive(Clone, Copy, Debug)]
pub struct Column {
    pub key: &'static str,
    pub width: f32,
    pub right: bool,
    /// Ordine di rinuncia nelle finestre strette: prima i numeri più alti (0 = mai).
    pub drop: u8,
}

/// Larghezze delle colonne fisse: almeno quella scritta, e abbastanza per l'intestazione tradotta
/// (una traduzione più lunga non si tronca, RS30).
pub fn fit_widths(painter: &Painter, l: &Lingue, cols: &[Column]) -> Vec<Column> {
    cols.iter()
        .map(|c| {
            if c.width == 0.0 {
                return *c;
            }
            let w = painter.layout_no_wrap(l.t(c.key), widgets::font(Type::Caption), Color32::WHITE).size().x + 24.0;
            Column { width: c.width.max(w.ceil()), ..*c }
        })
        .collect()
}

/// Posizione orizzontale delle colonne visibili; `None` per quelle nascoste.
pub fn layout(cols: &[Column], left: f32, right: f32, min_flex: f32) -> Vec<Option<(f32, f32)>> {
    layout_with(cols, &vec![false; cols.len()], left, right, min_flex)
}

/// Come [`layout`], con alcune colonne già nascoste per scelta (Impostazioni › Colonne della tabella).
pub fn layout_with(cols: &[Column], chosen_hidden: &[bool], left: f32, right: f32, min_flex: f32) -> Vec<Option<(f32, f32)>> {
    let mut hidden: Vec<bool> = (0..cols.len()).map(|k| chosen_hidden.get(k).copied().unwrap_or(false)).collect();
    loop {
        let fixed: f32 = cols.iter().zip(&hidden).filter(|(c, h)| !**h && c.width > 0.0).map(|(c, _)| c.width).sum();
        if right - left - fixed >= min_flex {
            break;
        }
        // Si nasconde la colonna visibile con il numero di rinuncia più alto.
        let Some((k, _)) = cols.iter().enumerate().filter(|(k, c)| !hidden[*k] && c.drop > 0).max_by_key(|(_, c)| c.drop) else { break };
        hidden[k] = true;
    }
    let fixed: f32 = cols.iter().zip(&hidden).filter(|(c, h)| !**h && c.width > 0.0).map(|(c, _)| c.width).sum();
    let flex = (right - left - fixed).max(min_flex.min(right - left));
    let mut x = left;
    cols.iter()
        .zip(&hidden)
        .map(|(c, h)| {
            if *h {
                return None;
            }
            let w = if c.width > 0.0 { c.width } else { flex };
            let r = (x, x + w);
            x += w;
            Some(r)
        })
        .collect()
}

/// Intestazione delle colonne (`.grid .hdr`): caption 11/500 in `text_2`, filetto sotto, nessun fondo.
pub fn header(painter: &Painter, p: &Palette, l: &Lingue, cols: &[Column], xs: &[Option<(f32, f32)>], rect: Rect) {
    header_sorted(painter, p, l, cols, xs, rect, None);
}

/// Come [`header`], con la freccia dell'ordine accanto alla colonna `sorted` (in ordine decrescente).
pub fn header_sorted(painter: &Painter, p: &Palette, l: &Lingue, cols: &[Column], xs: &[Option<(f32, f32)>], rect: Rect, sorted: Option<usize>) {
    painter.hline(rect.x_range(), rect.max.y - 0.5, Stroke::new(1.0, p.separator));
    let font = widgets::font(Type::Caption);
    for (k, (c, x)) in cols.iter().zip(xs).enumerate() {
        let Some((a, b)) = *x else { continue };
        let label = l.t(c.key);
        // La freccia dell'ordine sta dopo l'intestazione, nel margine della colonna: `fit_widths` lascia
        // 24 punti oltre il testo, abbastanza per la freccia di 12 senza allargare la colonna.
        let arrow = sorted == Some(k);
        let g = widgets::elided(painter, label, font.clone(), p.text_2, b - a - if arrow { 24.0 } else { 16.0 });
        let y = rect.center().y - g.size().y / 2.0;
        let w = g.size().x;
        let x = match (c.right, arrow) {
            (true, true) => b - 16.0 - w,
            (true, false) => b - 10.0 - w,
            (false, _) => a + 10.0,
        };
        painter.galley(pos2(x, y), g, p.text_2);
        if arrow {
            crate::icons::paint(painter, Rect::from_center_size(pos2(x + w + 8.0, rect.center().y), egui::vec2(12.0, 12.0)), crate::icons::Icon::ChevronDown, p.text_2);
        }
    }
}

/// Percorso troncato all'inizio (`…\AppData\Local`), perché la parte utile è la fine.
pub fn path_elided_start(painter: &Painter, path: &str, font: FontId, color: Color32, max_width: f32) -> Arc<Galley> {
    let g = painter.layout_no_wrap(path.to_owned(), font.clone(), color);
    if g.size().x <= max_width {
        return g;
    }
    let parts: Vec<&str> = path.split('\\').collect();
    for k in 1..parts.len() {
        let s = format!("…\\{}", parts[k..].join("\\"));
        let g = painter.layout_no_wrap(s, font.clone(), color);
        if g.size().x <= max_width {
            return g;
        }
    }
    widgets::elided(painter, path.to_owned(), font, color, max_width)
}

/// Tasti di navigazione di una tabella, letti una volta per frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct NavKeys {
    pub up: usize,
    pub down: usize,
    pub page_up: usize,
    pub page_down: usize,
    pub home: bool,
    pub end: bool,
    pub left: usize,
    pub right: usize,
    pub enter: bool,
    pub back: bool,
    pub star: bool,
    pub alt_left: bool,
    pub alt_right: bool,
}

impl NavKeys {
    pub fn read(ui: &Ui) -> Self {
        ui.input(|i| {
            let alt = i.modifiers.alt;
            Self {
                up: i.num_presses(Key::ArrowUp),
                down: i.num_presses(Key::ArrowDown),
                page_up: i.num_presses(Key::PageUp),
                page_down: i.num_presses(Key::PageDown),
                home: i.key_pressed(Key::Home),
                end: i.key_pressed(Key::End),
                left: if alt { 0 } else { i.num_presses(Key::ArrowLeft) },
                right: if alt { 0 } else { i.num_presses(Key::ArrowRight) },
                enter: i.key_pressed(Key::Enter),
                back: i.key_pressed(Key::Backspace),
                star: i.events.iter().any(|e| matches!(e, egui::Event::Text(t) if t == "*")),
                alt_left: alt && i.key_pressed(Key::ArrowLeft),
                alt_right: alt && i.key_pressed(Key::ArrowRight),
            }
        })
    }

    pub fn moves(&self) -> bool {
        self.up + self.down + self.page_up + self.page_down > 0 || self.home || self.end
    }

    /// Nuova riga dopo i tasti di movimento verticale.
    pub fn apply(&self, row: Option<usize>, rows: usize, page: usize) -> Option<usize> {
        if rows == 0 {
            return None;
        }
        let Some(mut r) = row else {
            // Senza selezione il primo tasto seleziona la prima riga, Fine l'ultima.
            return if self.end { Some(rows - 1) } else { self.moves().then_some(0) };
        };
        r = r.saturating_sub(self.up).saturating_sub(self.page_up * page);
        r = (r + self.down + self.page_down * page).min(rows - 1);
        if self.home {
            r = 0;
        }
        if self.end {
            r = rows - 1;
        }
        Some(r)
    }
}

/// Testo centrato in un'area vuota (nessun risultato, lavoro in corso). Va a capo se non entra: gli
/// stati vuoti dicono cosa è successo e cosa fare, e una traduzione più lunga non si taglia.
pub fn empty_message(painter: &Painter, p: &Palette, rect: Rect, s: &str) {
    let g = painter.layout(s.to_owned(), widgets::sans(13.0), p.text_3, (rect.width() - 48.0).clamp(120.0, 640.0));
    painter.galley(rect.center() - g.size() / 2.0, g, p.text_3);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_windows_drop_columns_in_order() {
        let cols = [
            Column { key: "a", width: 0.0, right: false, drop: 0 },
            Column { key: "b", width: 100.0, right: true, drop: 0 },
            Column { key: "c", width: 100.0, right: false, drop: 2 },
            Column { key: "d", width: 100.0, right: false, drop: 1 },
        ];
        let wide = layout(&cols, 0.0, 600.0, 200.0);
        assert!(wide.iter().all(Option::is_some));
        let narrow = layout(&cols, 0.0, 450.0, 200.0);
        assert!(narrow[2].is_none() && narrow[3].is_some());
        let tiny = layout(&cols, 0.0, 320.0, 200.0);
        assert!(tiny[2].is_none() && tiny[3].is_none() && tiny[1].is_some());
    }

    #[test]
    fn keys_move_within_bounds() {
        let k = NavKeys { down: 3, ..Default::default() };
        assert_eq!(k.apply(Some(8), 10, 5), Some(9));
        let k = NavKeys { page_up: 1, ..Default::default() };
        assert_eq!(k.apply(Some(3), 10, 5), Some(0));
        assert_eq!(NavKeys { end: true, ..Default::default() }.apply(None, 10, 5), Some(9));
    }
}
