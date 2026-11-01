// SPDX-License-Identifier: GPL-3.0-or-later
//! Mappa ad albero: disposizione "squarified" (Bruls, Huizing, van Wijk 2000) annidata per cartelle,
//! con l'intestazione delle cartelle abbastanza grandi.
//!
//! La geometria si calcola una volta per vista e dimensione; i riempimenti stanno in una mesh in cache,
//! così un frame con hover o selezione ridisegna solo i contorni e le etichette.

use std::sync::Arc;

use egui::epaint::{Mesh, RectShape, Shape, TessellationOptions, Tessellator};
use egui::text::{LayoutJob, TextWrapping};
use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, TextureHandle, Vec2, pos2, vec2};

/// Albero da disporre. I figli si restituiscono in ordine di dimensione decrescente.
pub trait Source {
    fn size(&self, node: u32) -> u64;
    fn children(&self, node: u32, out: &mut Vec<u32>);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    pub rect: Rect,
    pub node: u32,
    pub depth: u16,
    /// Area disegnata come blocco pieno (file, o cartella troppo piccola per mostrarne il contenuto).
    pub leaf: bool,
    /// Indice della cartella che contiene l'area in `tiles`, `u32::MAX` per la radice.
    pub parent: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Margine interno delle cartelle.
    pub padding: f32,
    /// Altezza dell'intestazione con il nome della cartella.
    pub header: f32,
    /// Una cartella mostra l'intestazione solo se è alta almeno così.
    pub min_header_rect: f32,
    /// Sotto questo lato una cartella diventa un blocco pieno.
    pub min_side: f32,
    /// Livelli disegnati sotto la radice: oltre, una cartella è un blocco pieno (F-EX-1: due livelli).
    pub max_depth: u16,
    /// Aree più piccole di così (in pixel quadrati) non si dispongono: in ordine decrescente, anche le
    /// successive lo sono, e una cartella con un milione di figli non produce un milione di aree.
    pub min_area: f32,
    /// La radice ha la sua intestazione (falso quando il nome è già nella barra del percorso).
    pub root_header: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { padding: 2.0, header: 16.0, min_header_rect: 40.0, min_side: 6.0, max_depth: u16::MAX, min_area: 0.0, root_header: true }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub tiles: Vec<Tile>,
    pub leaves: usize,
}

/// Dispone `values` (decrescenti, non negativi) dentro `rect`. Scrive un rettangolo per valore.
pub fn squarify(values: &[f64], rect: Rect, out: &mut Vec<Rect>) {
    let total: f64 = values.iter().sum();
    let start = out.len();
    out.resize(start + values.len(), Rect::NOTHING);
    if total <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        for r in &mut out[start..] {
            *r = Rect::from_min_size(rect.min, Vec2::ZERO);
        }
        return;
    }
    let (mut x, mut y) = (f64::from(rect.min.x), f64::from(rect.min.y));
    let (mut w, mut h) = (f64::from(rect.width()), f64::from(rect.height()));
    let scale = w * h / total;
    let area = |i: usize| values[i] * scale;

    let n = values.len();
    let mut i = 0;
    while i < n {
        let side = w.min(h);
        let mut sum = 0.0;
        let mut worst = f64::INFINITY;
        let mut j = i;
        while j < n {
            let a_max = area(i);
            let a_min = area(j);
            let s = sum + area(j);
            let ratio = if a_min <= 0.0 { f64::INFINITY } else { ((side * side * a_max) / (s * s)).max((s * s) / (side * side * a_min)) };
            if j > i && ratio > worst {
                break;
            }
            sum = s;
            worst = ratio;
            j += 1;
        }
        // La riga [i, j) occupa una striscia lungo il lato corto.
        if w >= h {
            let strip = if h > 0.0 { sum / h } else { 0.0 };
            let mut cy = y;
            for k in i..j {
                let len = if strip > 0.0 { area(k) / strip } else { 0.0 };
                out[start + k] = Rect::from_min_size(pos2(x as f32, cy as f32), vec2(strip as f32, len as f32));
                cy += len;
            }
            x += strip;
            w -= strip;
        } else {
            let strip = if w > 0.0 { sum / w } else { 0.0 };
            let mut cx = x;
            for k in i..j {
                let len = if strip > 0.0 { area(k) / strip } else { 0.0 };
                out[start + k] = Rect::from_min_size(pos2(cx as f32, y as f32), vec2(len as f32, strip as f32));
                cx += len;
            }
            y += strip;
            h -= strip;
        }
        i = j;
    }
}

/// Dispone il sottoalbero di `root` dentro `rect`.
pub fn layout(src: &impl Source, root: u32, rect: Rect, opts: &Options) -> Layout {
    let mut out = Layout::default();
    let mut kids = Vec::new();
    let mut values = Vec::new();
    let mut rects = Vec::new();
    place(src, root, rect, 0, u32::MAX, opts, &mut out, &mut kids, &mut values, &mut rects);
    out
}

#[allow(clippy::too_many_arguments)]
fn place(
    src: &impl Source,
    node: u32,
    rect: Rect,
    depth: u16,
    parent: u32,
    opts: &Options,
    out: &mut Layout,
    kids: &mut Vec<u32>,
    values: &mut Vec<f64>,
    rects: &mut Vec<Rect>,
) {
    let index = out.tiles.len() as u32;
    kids.clear();
    src.children(node, kids);
    let too_small = rect.width() < opts.min_side || rect.height() < opts.min_side || depth >= opts.max_depth;
    let mut inner = rect.shrink(opts.padding);
    if (depth > 0 || opts.root_header) && rect.height() >= opts.min_header_rect && rect.width() >= opts.min_header_rect {
        inner.min.y += opts.header - opts.padding;
    }
    if kids.is_empty() || too_small || inner.width() < 1.0 || inner.height() < 1.0 {
        out.tiles.push(Tile { rect, node, depth, leaf: true, parent });
        out.leaves += 1;
        return;
    }
    out.tiles.push(Tile { rect, node, depth, leaf: false, parent });

    let my_kids = std::mem::take(kids);
    values.clear();
    values.extend(my_kids.iter().map(|&k| src.size(k) as f64));
    let base = rects.len();
    squarify(values, inner, rects);
    for (n, &k) in my_kids.iter().enumerate() {
        let r = rects[base + n];
        let area = r.width() * r.height();
        if area <= opts.min_area.max(0.0) {
            if opts.min_area > 0.0 {
                break;
            }
            continue;
        }
        place(src, k, r, depth + 1, index, opts, out, kids, values, rects);
    }
    rects.truncate(base);
    *kids = my_kids;
}

impl Layout {
    /// Area foglia sotto il punto.
    pub fn leaf_at(&self, p: Pos2) -> Option<usize> {
        self.tiles.iter().position(|t| t.leaf && t.rect.contains(p))
    }

    /// Area foglia più vicina nella direzione `dir` (frecce), partendo da `from`.
    /// Sceglie il centro più vicino entro un cono di 45 gradi, pesando la deviazione laterale.
    pub fn neighbour(&self, from: usize, dir: Vec2) -> Option<usize> {
        let c = self.tiles[from].rect.center();
        let mut best: Option<(f32, usize)> = None;
        for (i, t) in self.tiles.iter().enumerate() {
            if !t.leaf || i == from {
                continue;
            }
            let d = t.rect.center() - c;
            let along = d.dot(dir);
            if along <= 0.5 {
                continue;
            }
            let across = (d - dir * along).length();
            if across > along * 1.5 {
                continue;
            }
            let score = along + across * 2.0;
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, i));
            }
        }
        best.map(|(_, i)| i)
    }
}

/// Colori e testo di un'area, decisi da chi usa la mappa.
pub struct TileStyle {
    pub fill: Color32,
    pub text: Color32,
    /// Area non misurata: tratteggio invece del colore pieno.
    pub hatched: bool,
}

/// Mesh dei riempimenti in cache.
pub struct Painted {
    pub fills: Arc<Mesh>,
    pub hatches: Arc<Mesh>,
}

/// Costruisce le mesh dei riempimenti: cartelle come cornice scura, foglie come blocchi separati da
/// una riga di sfondo di un pixel.
pub fn build_meshes(
    layout: &Layout,
    hatch: &TextureHandle,
    hatch_period: f32,
    folder_fill: Color32,
    style: impl Fn(&Tile) -> TileStyle,
) -> Painted {
    let mut fills = Mesh::default();
    fills.reserve_vertices(layout.tiles.len() * 4);
    fills.reserve_triangles(layout.tiles.len() * 2);
    let mut hatches = Mesh::with_texture(hatch.id());
    for t in &layout.tiles {
        if t.leaf {
            let r = t.rect.shrink(0.5);
            if r.width() <= 0.0 || r.height() <= 0.0 {
                continue;
            }
            let s = style(t);
            if s.hatched {
                let uv = Rect::from_min_max(
                    pos2(r.min.x / hatch_period, r.min.y / hatch_period),
                    pos2(r.max.x / hatch_period, r.max.y / hatch_period),
                );
                hatches.add_rect_with_uv(r, uv, Color32::WHITE);
            } else {
                fills.add_colored_rect(r, s.fill);
            }
        } else {
            fills.add_colored_rect(t.rect.shrink(0.5), folder_fill);
        }
    }
    Painted { fills: Arc::new(fills), hatches: Arc::new(hatches) }
}

/// Come [`build_meshes`], con le aree arrotondate e staccate tra loro (variante A): `shape` dà per ogni
/// area il rettangolo da riempire (già ristretto dello spazio tra le aree), il raggio e lo stile, oppure
/// `None` per non riempirla (le cartelle aperte stanno sul fondo della mappa). Gli angoli si tassellano
/// una volta qui, con l'antialiasing di egui alla densità di pixel `pixels_per_point`.
pub fn build_rounded_meshes(layout: &Layout, hatch: &TextureHandle, hatch_period: f32, pixels_per_point: f32, shape: impl Fn(&Tile) -> Option<(Rect, u8, TileStyle)>) -> Painted {
    let mut tess = Tessellator::new(pixels_per_point, TessellationOptions::default(), [1, 1], Vec::new());
    let mut fills = Mesh::default();
    let mut hatches = Mesh::default();
    for t in &layout.tiles {
        let Some((r, radius, s)) = shape(t) else { continue };
        if r.width() < 0.5 || r.height() < 0.5 {
            continue;
        }
        // Il raggio non supera metà del lato corto: le aree strette restano rettangoli.
        let rr = CornerRadius::same(radius.min((r.width().min(r.height()) / 2.0).floor() as u8));
        if s.hatched {
            let start = hatches.vertices.len();
            tess.tessellate_rect(&RectShape::filled(r, rr, Color32::WHITE), &mut hatches);
            for v in &mut hatches.vertices[start..] {
                v.uv = pos2(v.pos.x / hatch_period, v.pos.y / hatch_period);
            }
        } else {
            tess.tessellate_rect(&RectShape::filled(r, rr, s.fill), &mut fills);
        }
    }
    hatches.texture_id = hatch.id();
    Painted { fills: Arc::new(fills), hatches: Arc::new(hatches) }
}

/// Disegna mesh in cache ed etichette. Le etichette entrano solo dove c'è spazio, troncate con "…".
pub fn paint(
    painter: &Painter,
    layout: &Layout,
    painted: &Painted,
    font: &FontId,
    header_text: Color32,
    label: impl Fn(&Tile) -> Option<(String, Color32)>,
    opts: &Options,
) {
    painter.add(Shape::Mesh(Arc::clone(&painted.fills)));
    if !painted.hatches.is_empty() {
        painter.add(Shape::Mesh(Arc::clone(&painted.hatches)));
    }
    let line = font.size + 2.0;
    for t in &layout.tiles {
        let has_header = !t.leaf && t.rect.height() >= opts.min_header_rect && t.rect.width() >= opts.min_header_rect;
        let fits_leaf = t.leaf && t.rect.width() >= 40.0 && t.rect.height() >= line + 4.0;
        if !(has_header || fits_leaf) {
            continue;
        }
        let Some((text, color)) = label(t) else { continue };
        let color = if has_header { header_text } else { color };
        let max_width = t.rect.width() - 8.0;
        let mut job = LayoutJob::simple_singleline(text, font.clone(), color);
        job.wrap = TextWrapping { max_width, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
        let galley = painter.layout_job(job);
        let pos = pos2(t.rect.min.x + 4.0, t.rect.min.y + if has_header { (opts.header - galley.size().y) / 2.0 + 1.0 } else { 3.0 });
        painter.galley(pos, galley, color);
    }
}

/// Contorno di un'area (hover o selezione).
pub fn outline(painter: &Painter, rect: Rect, outer: Color32, inner: Color32) {
    painter.rect_stroke(rect, 0.0, Stroke::new(2.0, outer), StrokeKind::Inside);
    painter.rect_stroke(rect.shrink(2.0), 0.0, Stroke::new(1.0, inner), StrokeKind::Inside);
}

/// Immagine del tratteggio diagonale, ripetibile: `period` pixel per lato.
pub fn hatch_image(period: usize, a: Color32, b: Color32) -> egui::ColorImage {
    let mut pixels = Vec::with_capacity(period * period);
    for y in 0..period {
        for x in 0..period {
            // Righe a 45 gradi spesse un quarto del periodo.
            let on = (x + y) % period < period / 4;
            pixels.push(if on { a } else { b });
        }
    }
    egui::ColorImage::new([period, period], pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(r: &Rect) -> f64 {
        f64::from(r.width()) * f64::from(r.height())
    }

    #[test]
    fn squarify_preserves_area_and_order() {
        let values = [6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0];
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(600.0, 400.0));
        let mut out = Vec::new();
        squarify(&values, rect, &mut out);
        let total: f64 = values.iter().sum();
        for (v, r) in values.iter().zip(&out) {
            let expected = v / total * 240_000.0;
            assert!((area(r) - expected).abs() < 1.0, "{r:?} {expected}");
            assert!(rect.expand(0.01).contains_rect(*r));
        }
        // Nessuna sovrapposizione.
        for a in 0..out.len() {
            for b in a + 1..out.len() {
                let i = out[a].intersect(out[b]);
                assert!(i.width() <= 0.01 || i.height() <= 0.01, "{a} {b}");
            }
        }
    }

    #[test]
    fn squarify_aspect_is_reasonable() {
        let values: Vec<f64> = (1..=200).rev().map(f64::from).collect();
        let mut out = Vec::new();
        squarify(&values, Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0)), &mut out);
        let worst = out.iter().map(|r| (r.width() / r.height()).max(r.height() / r.width())).fold(0.0, f32::max);
        assert!(worst < 6.0, "{worst}");
    }

    struct Flat(Vec<u64>);
    impl Source for Flat {
        fn size(&self, node: u32) -> u64 {
            if node == 0 { self.0.iter().sum() } else { self.0[node as usize - 1] }
        }
        fn children(&self, node: u32, out: &mut Vec<u32>) {
            if node == 0 {
                out.extend(1..=self.0.len() as u32);
            }
        }
    }

    #[test]
    fn layout_and_navigation() {
        let src = Flat(vec![40, 30, 20, 10]);
        let l = layout(&src, 0, Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0)), &Options::default());
        assert_eq!(l.leaves, 4);
        assert!(!l.tiles[0].leaf);
        let first = l.leaf_at(pos2(10.0, 30.0)).unwrap();
        assert_eq!(l.tiles[first].node, 1);
        // Da ogni foglia almeno una direzione porta altrove.
        for i in 1..l.tiles.len() {
            let dirs = [Vec2::RIGHT, Vec2::LEFT, Vec2::UP, Vec2::DOWN];
            assert!(dirs.iter().any(|d| l.neighbour(i, *d).is_some()), "{i}");
        }
    }
}
