// SPDX-License-Identifier: GPL-3.0-or-later
//! Marchio di Quado, segno «Metà» (`docs/brand`): un riquadro diviso in una metà e due quarti.
//! Qui la geometria del segno sulla griglia di 24 unità e sul disegno per 16 px, la tessera dell'icona e il
//! disegno con il painter di egui. La stessa geometria genera l'icona dell'eseguibile (`cargo xtask icona`).

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};

/// Nome del programma: titolo della finestra, barra laterale, Informazioni. Non si traduce.
pub const NAME: &str = "Quado";
/// Titolare del diritto d'autore, nella riga di copyright di Informazioni (GPLv3 §0, "Appropriate Legal
/// Notices"): il nome utente GitHub del progetto, scelto dal proprietario il 01/11/2026.
pub const COPYRIGHT: &str = "Copyright (C) 2026 Valmairos";
/// Dove si trova il codice sorgente. Quado non apre la pagina: mostra l'indirizzo e lo copia.
pub const SOURCE_URL: &str = "https://github.com/Valmairos/quado";
/// Testo della licenza (GPL-3.0-or-later), incorporato: `quado.exe` scaricato da solo non ha `LICENSE` accanto.
pub const LICENSE_TEXT: &str = include_str!("../../../LICENSE");

/// Colori del segno, quelli dell'accento Grafite della variante A: grafite su fondo chiaro, chiaro su fondo
/// scuro. Un solo colore per segno.
pub const GRAPHITE: [u8; 3] = [0x1D, 0x1D, 0x1F];
pub const LIGHT: [u8; 3] = [0xF5, 0xF5, 0xF7];

/// Una parte del segno: il rettangolo da `(x0, y0)` a `(x1, y1)` in unità della griglia, con i raggi degli
/// angoli in senso orario da quello in alto a sinistra: alto-sinistra, alto-destra, basso-destra,
/// basso-sinistra.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub r: [f32; 4],
}

/// Un disegno del segno: le parti e il lato della griglia.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub grid: f32,
    pub parts: [Part; 3],
}

/// Griglia di 24 unità (`segno-meta.svg`): area utile da 2 a 22, tagli di 2, raggi 4 sugli angoli esterni e
/// 1,5 lungo i tagli.
pub const MARK_24: Mark = Mark {
    grid: 24.0,
    parts: [
        Part { x0: 2.0, y0: 2.0, x1: 22.0, y1: 11.0, r: [4.0, 4.0, 1.5, 1.5] },
        Part { x0: 2.0, y0: 13.0, x1: 11.0, y1: 22.0, r: [1.5, 1.5, 1.5, 4.0] },
        Part { x0: 13.0, y0: 13.0, x1: 22.0, y1: 22.0, r: [1.5, 1.5, 4.0, 1.5] },
    ],
};

/// Disegno per 16 e 32 px (`segno-meta-16px.svg`): griglia di 16, area utile da 1 a 14, asse a 7,5, tagli
/// di un pixel esatto. Il disegno a 24 ridotto a 16 px darebbe tagli di 1,33 px, cioè grigi.
pub const MARK_16: Mark = Mark {
    grid: 16.0,
    parts: [
        Part { x0: 1.0, y0: 1.0, x1: 14.0, y1: 7.0, r: [2.5, 2.5, 1.0, 1.0] },
        Part { x0: 1.0, y0: 8.0, x1: 7.0, y1: 14.0, r: [1.0, 1.0, 1.0, 2.5] },
        Part { x0: 8.0, y0: 8.0, x1: 14.0, y1: 14.0, r: [1.0, 1.0, 2.5, 1.0] },
    ],
};

/// Tessera dell'icona (`icona-meta-grafite.svg`): superellisse di esponente 5, grafite, con il segno chiaro
/// al centro e un bordo interno chiaro. È l'icona a tutte le misure: su fondo chiaro si vede la tessera, su
/// fondo scuro il segno chiaro e il bordo, qualunque sia il tema di Windows.
pub const TILE_EXPONENT: f32 = 5.0;

/// Colore del bordo interno della tessera: disegna la sagoma della tessera dove la grafite si confonde con lo
/// sfondo (barra delle applicazioni, menu Start, Esplora file e Alt+Tab col tema scuro).
pub const RIM: [u8; 3] = [0x4A, 0x4A, 0x4D];

/// Una tessera disegnata per una misura in pixel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    /// Lato della tessera in pixel, dall'angolo in alto a sinistra dell'immagine.
    pub side: f32,
    /// Spessore del bordo interno in pixel.
    pub rim: f32,
    /// Il segno in pixel dell'immagine (`grid` è il lato dell'immagine).
    pub mark: Mark,
}

/// Disegni della tessera per misura: lato dell'immagine, lato della tessera, inizio del segno, altezza di una
/// metà, taglio. I bordi delle parti cadono su pixel interi. Dove il taglio è di un pixel il segno ha lato
/// dispari: la tessera ha lato dispari anche lei e lascia libere l'ultima riga e l'ultima colonna, così i
/// margini del segno sono uguali sui quattro lati. A 40 px è la tessera della proposta (1 unità = 1 px). Sotto
/// i 32 px il segno occupa più della metà della tessera, per restare leggibile.
const TILES: [(u32, f32, f32, f32, f32); 8] = [
    (16, 15.0, 3.0, 4.0, 1.0),
    (20, 19.0, 4.0, 5.0, 1.0),
    (24, 23.0, 5.0, 6.0, 1.0),
    (32, 32.0, 7.0, 8.0, 2.0),
    (40, 40.0, 10.0, 9.0, 2.0),
    (48, 48.0, 12.0, 11.0, 2.0),
    (64, 63.0, 15.0, 15.0, 3.0),
    (256, 256.0, 64.0, 58.0, 12.0),
];

/// Il segno nel quadrato da `(x0, x0)` di lato `2 * half + cut`, su una griglia di lato `grid`: raggi 0,2 del
/// lato sugli angoli esterni e 0,075 lungo i tagli, come sulla griglia di 24.
fn mark_in(grid: f32, x0: f32, half: f32, cut: f32) -> Mark {
    let side = 2.0 * half + cut;
    let (outer, inner) = (0.2 * side, 0.075 * side);
    let (x1, a, b) = (x0 + side, x0 + half, x0 + half + cut);
    Mark {
        grid,
        parts: [
            Part { x0, y0: x0, x1, y1: a, r: [outer, outer, inner, inner] },
            Part { x0, y0: b, x1: a, y1: x1, r: [inner, inner, inner, outer] },
            Part { x0: b, y0: b, x1, y1: x1, r: [inner, inner, outer, inner] },
        ],
    }
}

impl Tile {
    /// La tessera per un'immagine di `px` pixel: il disegno di [`TILES`], o per le altre misure la proposta
    /// in scala. Bordo di un pixel fino a 128 px, poi in proporzione.
    pub fn for_pixels(px: u32) -> Self {
        let s = px as f32;
        let (side, x0, half, cut) = TILES.iter().find(|t| t.0 == px).map_or((s, s / 4.0, 0.225 * s, 0.05 * s), |t| (t.1, t.2, t.3, t.4));
        Tile { side, rim: (s / 128.0).max(1.0), mark: mark_in(s, x0, half, cut) }
    }

    /// Valore della superellisse in `(x, y)` pixel, con la tessera ristretta di `inset`: 0 sul bordo, negativo
    /// dentro, in valore assoluto mai più grande della distanza dal bordo.
    pub fn distance(&self, x: f32, y: f32, inset: f32) -> f32 {
        let c = self.side / 2.0;
        let (dx, dy) = ((x - c).abs(), (y - c).abs());
        (dx.powf(TILE_EXPONENT) + dy.powf(TILE_EXPONENT)).powf(1.0 / TILE_EXPONENT) - (c - inset)
    }
}

impl Part {
    /// Raggio dell'angolo dalla parte di `(px, py)`, relativo al centro.
    fn radius(&self, px: f32, py: f32) -> f32 {
        match (px < 0.0, py < 0.0) {
            (true, true) => self.r[0],
            (false, true) => self.r[1],
            (false, false) => self.r[2],
            (true, false) => self.r[3],
        }
    }

    /// Distanza con segno dal bordo (negativa dentro), in unità della griglia.
    pub fn distance(&self, x: f32, y: f32) -> f32 {
        let (hx, hy) = ((self.x1 - self.x0) / 2.0, (self.y1 - self.y0) / 2.0);
        let (px, py) = (x - (self.x0 + hx), y - (self.y0 + hy));
        let r = self.radius(px, py);
        let (qx, qy) = (px.abs() - hx + r, py.abs() - hy + r);
        qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
    }

    /// Contorno in senso orario: i lati dritti e `steps` segmenti per ogni angolo.
    pub fn outline(&self, steps: usize) -> Vec<(f32, f32)> {
        let [tl, tr, br, bl] = self.r;
        // (centro dell'arco, raggio, angolo di partenza in gradi; 0 = destra, 90 = basso)
        let corners = [(self.x0 + tl, self.y0 + tl, tl, 180.0f32), (self.x1 - tr, self.y0 + tr, tr, 270.0), (self.x1 - br, self.y1 - br, br, 0.0), (self.x0 + bl, self.y1 - bl, bl, 90.0)];
        let mut out = Vec::with_capacity(4 * (steps + 1));
        for (cx, cy, r, from) in corners {
            for k in 0..=steps {
                let a = (from + 90.0 * k as f32 / steps as f32).to_radians();
                out.push((cx + r * a.cos(), cy + r * a.sin()));
            }
        }
        out
    }
}

impl Mark {
    /// Distanza con segno dal segno intero (negativa dentro), in unità della griglia.
    pub fn distance(&self, x: f32, y: f32) -> f32 {
        self.parts.iter().map(|p| p.distance(x, y)).fold(f32::INFINITY, f32::min)
    }

    /// Il disegno per un segno di `px` pixel di lato: quello per 16 px a 16 e 32 (raddoppiato), altrimenti
    /// quello sulla griglia di 24.
    pub fn for_pixels(px: f32) -> Self {
        if (px - 16.0).abs() < 0.5 || (px - 32.0).abs() < 0.5 { MARK_16 } else { MARK_24 }
    }
}

/// Disegna il segno nel quadrato `rect` (il lato più corto), nel colore `color`. Il disegno dipende dai pixel
/// veri (`Mark::for_pixels`); l'angolo in alto a sinistra cade su un pixel intero, così a 16 e 32 px i tagli
/// restano netti.
pub fn paint(painter: &Painter, rect: Rect, color: Color32) {
    let ppp = painter.pixels_per_point();
    let side = rect.width().min(rect.height());
    let mark = Mark::for_pixels(side * ppp);
    let origin = rect.center() - egui::vec2(side / 2.0, side / 2.0);
    let origin = pos2((origin.x * ppp).round() / ppp, (origin.y * ppp).round() / ppp);
    let s = side / mark.grid;
    let to_screen = |(x, y): (f32, f32)| -> Pos2 { pos2(origin.x + x * s, origin.y + y * s) };
    let shapes = mark.parts.iter().map(|p| Shape::convex_polygon(p.outline(8).into_iter().map(to_screen).collect(), color, Stroke::NONE));
    painter.extend(shapes);
}

/// Segno e nome insieme, con le regole della proposta: lato del segno 1,1 volte il corpo del testo, distanza
/// 0,32 volte il corpo, nome in Inter SemiBold con le lettere avvicinate del 2%, centro del segno sul centro
/// delle maiuscole (in Inter coincide con il centro della riga).
pub fn lockup(ui: &mut Ui, size: f32, color: Color32) -> Response {
    let mark = (1.1 * size).round();
    let gap = 0.32 * size;
    let format = TextFormat { font_id: FontId::new(size, crate::fonts::semibold()), color, extra_letter_spacing: -0.02 * size, ..Default::default() };
    let g = ui.painter().layout_job(LayoutJob::single_section(NAME.to_owned(), format));
    let (rect, resp) = ui.allocate_exact_size(vec2(mark + gap + g.size().x, g.size().y.max(mark)), Sense::hover());
    let cy = rect.center().y;
    paint(ui.painter(), Rect::from_min_size(pos2(rect.min.x, cy - mark / 2.0), vec2(mark, mark)), color);
    ui.painter().galley(pos2(rect.min.x + mark + gap, cy - g.size().y / 2.0), g, color);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, NAME));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_match_the_svg_of_the_proposal() {
        // Tagli di 2 unità (24) e di 1 px (16), asse di simmetria a 12 e a 7,5.
        for (m, cut, axis) in [(MARK_24, 2.0, 12.0), (MARK_16, 1.0, 7.5)] {
            let [top, left, right] = m.parts;
            assert_eq!(left.y0 - top.y1, cut);
            assert_eq!(right.x0 - left.x1, cut);
            assert_eq!((top.x0 + top.x1) / 2.0, axis);
            assert_eq!((left.x0 + right.x1) / 2.0, axis);
            // Simmetria sull'asse verticale: i raggi dei due quarti sono speculari.
            assert_eq!([left.r[1], left.r[0], left.r[3], left.r[2]], right.r);
        }
        // Dentro, fuori e sul bordo.
        assert!(MARK_24.distance(12.0, 6.0) < 0.0);
        assert!(MARK_24.distance(12.0, 12.0) > 0.0, "il taglio orizzontale");
        assert!(MARK_24.distance(12.0, 18.0) > 0.0, "il taglio verticale");
        assert!(MARK_24.distance(2.3, 2.3) > 0.0, "fuori dall'angolo di raggio 4");
        assert!((MARK_24.parts[0].distance(12.0, 2.0)).abs() < 1e-5);
        assert_eq!(Mark::for_pixels(16.0), MARK_16);
        assert_eq!(Mark::for_pixels(32.0), MARK_16);
        assert_eq!(Mark::for_pixels(18.0), MARK_24);
    }

    #[test]
    fn every_tile_has_whole_pixel_cuts_and_centered_mark() {
        // A 40 px la tessera è quella della proposta: il segno sulla griglia di 24 spostato di 8.
        let t = Tile::for_pixels(40);
        for (p, q) in t.mark.parts.iter().zip(MARK_24.parts) {
            let got = [p.x0, p.y0, p.x1, p.y1, p.r[0], p.r[1], p.r[2], p.r[3]];
            let want = [q.x0 + 8.0, q.y0 + 8.0, q.x1 + 8.0, q.y1 + 8.0, q.r[0], q.r[1], q.r[2], q.r[3]];
            assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-5), "{got:?} {want:?}");
        }
        for (px, ..) in TILES {
            let t = Tile::for_pixels(px);
            let [top, left, right] = t.mark.parts;
            // Il segno al centro della tessera, la tessera dentro l'immagine.
            assert_eq!(top.x0 + top.x1, t.side, "{px}");
            assert_eq!(left.y0 - top.y1, right.x0 - left.x1, "{px}");
            assert!(t.side <= px as f32 && t.side >= px as f32 - 1.0, "{px}");
            if px < 256 {
                for v in [top.x0, top.x1, top.y1, left.y0, left.x1, right.x0] {
                    assert_eq!(v.fract(), 0.0, "{px}: bordo fuori dai pixel");
                }
            }
            assert!(t.distance(t.side / 2.0, 0.0, 0.0).abs() < 1e-4);
            assert!(t.distance(0.5, 0.5, 0.0) > 0.0, "{px}: l'angolo della tessera è arrotondato");
            assert!(t.distance(t.side / 2.0, 0.5, t.rim) > 0.0, "{px}: il bordo interno");
        }
    }

    #[test]
    fn the_outline_follows_the_corners() {
        let p = MARK_24.parts[0];
        let o = p.outline(8);
        assert_eq!(o.len(), 36);
        for &(x, y) in &o {
            assert!(p.distance(x, y).abs() < 1e-4, "({x}, {y})");
        }
    }
}
