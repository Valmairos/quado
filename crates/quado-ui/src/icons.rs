// SPDX-License-Identifier: GPL-3.0-or-later
//! Icone a tratto disegnate con il painter, sugli stessi simboli della proposta visiva (variante A:
//! griglia di 16 unità, tratto 1,5, estremi arrotondati). Un solo stile, nessuna icona colorata: il colore
//! appartiene ai dati.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// Segno del marchio (`brand`), pieno.
    Logo,
    /// Unità (disco).
    Drive,
    Search,
    Refresh,
    Overview,
    Explore,
    BigFiles,
    /// Impostazioni (cursori).
    Settings,
    ChevronRight,
    ChevronDown,
    /// Freccia piccola dei selettori a tendina.
    Caret,
    /// Indietro e avanti della barra superiore.
    Back,
    Forward,
    ArrowLeft,
    ArrowRight,
    /// Cartella superiore.
    ArrowUp,
    External,
    Copy,
    Info,
    Verify,
    Close,
    Warning,
    /// Stati di valutazione: sempre icona più parola, mai il solo colore.
    StateManageable,
    StateToVerify,
    StateYouDecide,
    StateManagedBy,
    StateUnknown,
    /// Vista Windows e sistema.
    System,
    /// Elenco delle regole.
    Rules,
    /// Vista Download.
    Download,
    /// Vista Programmi.
    Programs,
    /// Vista Interventi (storico).
    Actions,
    /// Chiudi o apri la barra laterale.
    Sidebar,
    /// Palette dei comandi.
    Palette,
    /// Privilegi di amministratore.
    Shield,
    /// Sola lettura.
    Lock,
    Check,
    Folder,
    File,
    /// Proprietà di Windows.
    Properties,
    /// Solo elenco.
    List,
    /// Cestino.
    Trash,
}

/// Disegna `icon` nel quadrato `rect`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    paint_stroke(painter, rect, icon, color, 1.5 * rect.width().min(rect.height()) / 16.0);
}

/// Come [`paint`], con il tratto di `width` punti: le icone grandi (`.i.xl`, 32 punti) restano sottili
/// (1,25) invece di ingrossare il tratto con la dimensione.
pub fn paint_stroke(painter: &Painter, rect: Rect, icon: Icon, color: Color32, width: f32) {
    if icon == Icon::Logo {
        logo(painter, rect, color);
        return;
    }
    let s = rect.width().min(rect.height()) / 16.0;
    let o = rect.center() - egui::vec2(8.0 * s, 8.0 * s);
    let p = |x: f32, y: f32| pos2(o.x + x * s, o.y + y * s);
    let stroke = Stroke::new(width.max(1.0), color);
    let line = |pts: &[(f32, f32)]| Shape::line(pts.iter().map(|&(x, y)| p(x, y)).collect(), stroke);
    let rrect = |x: f32, y: f32, w: f32, h: f32, r: f32| Shape::rect_stroke(Rect::from_min_max(p(x, y), p(x + w, y + h)), r * s, stroke, StrokeKind::Middle);
    let circle = |x: f32, y: f32, r: f32| Shape::circle_stroke(p(x, y), r * s, stroke);
    let dot = |x: f32, y: f32| Shape::circle_filled(p(x, y), 0.85 * s, color);
    // Arco da `from` a `to` gradi (0 = destra, 90 = basso), come gli archi SVG della proposta.
    let arc = |cx: f32, cy: f32, r: f32, from: f32, to: f32| -> Vec<Pos2> {
        let n = ((to - from).abs() / 12.0).ceil().max(2.0) as usize;
        (0..=n)
            .map(|k| {
                let a = (from + (to - from) * k as f32 / n as f32).to_radians();
                p(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect()
    };
    // Spezzata con angoli arrotondati: ogni vertice con raggio > 0 diventa una curva tra i due lati.
    let rounded_pts = |pts: &[(f32, f32, f32)], closed: bool| -> Vec<Pos2> {
        let mut out: Vec<Pos2> = Vec::new();
        let n = pts.len();
        for i in 0..n {
            let (x, y, r) = pts[i];
            if r <= 0.0 || (!closed && (i == 0 || i == n - 1)) {
                out.push(p(x, y));
                continue;
            }
            let (px, py, _) = pts[(i + n - 1) % n];
            let (nx, ny, _) = pts[(i + 1) % n];
            let a = egui::vec2(px - x, py - y).normalized();
            let b = egui::vec2(nx - x, ny - y).normalized();
            for k in 0..=6 {
                let t = k as f32 / 6.0;
                // Bézier quadratica tra i due punti di tangenza, con il vertice come controllo.
                let s0 = egui::vec2(x, y) + a * r;
                let s1 = egui::vec2(x, y) + b * r;
                let c = egui::vec2(x, y);
                let q = s0 * (1.0 - t) * (1.0 - t) + c * 2.0 * t * (1.0 - t) + s1 * t * t;
                out.push(p(q.x, q.y));
            }
        }
        out
    };
    let rounded = |pts: &[(f32, f32, f32)], closed: bool| -> Shape {
        let out = rounded_pts(pts, closed);
        if closed { Shape::closed_line(out, stroke) } else { Shape::line(out, stroke) }
    };
    let doc = || rounded(&[(4.0, 2.0, 0.5), (9.0, 2.0, 0.0), (12.0, 5.0, 0.0), (12.0, 14.0, 0.5), (4.0, 14.0, 0.5)], true);
    let shapes: Vec<Shape> = match icon {
        Icon::Logo => Vec::new(),
        Icon::Drive => vec![rrect(2.0, 4.0, 12.0, 8.0, 2.0), line(&[(2.5, 9.0), (13.5, 9.0)]), line(&[(11.0, 10.6), (11.5, 10.6)])],
        Icon::Search => vec![circle(7.0, 7.0, 4.25), line(&[(10.2, 10.2), (13.5, 13.5)])],
        Icon::Refresh => vec![Shape::line(arc(8.0, 8.0, 5.0, 0.0, 315.0), stroke), line(&[(13.0, 2.5), (13.0, 5.5), (10.0, 5.5)])],
        Icon::Overview => vec![rrect(2.0, 2.5, 12.0, 11.0, 2.5), line(&[(5.0, 10.5), (5.0, 8.5)]), line(&[(8.0, 10.5), (8.0, 5.5)]), line(&[(11.0, 10.5), (11.0, 7.0)])],
        Icon::Explore => vec![rrect(2.0, 2.5, 12.0, 11.0, 2.5), line(&[(7.5, 2.5), (7.5, 13.5)]), line(&[(7.5, 8.25), (14.0, 8.25)])],
        Icon::BigFiles => vec![doc(), line(&[(9.0, 2.0), (9.0, 5.0), (12.0, 5.0)]), line(&[(6.0, 9.0), (10.0, 9.0)]), line(&[(6.0, 11.5), (10.0, 11.5)])],
        Icon::File => vec![doc(), line(&[(9.0, 2.0), (9.0, 5.0), (12.0, 5.0)])],
        Icon::Settings => vec![
            line(&[(2.5, 5.0), (8.5, 5.0)]),
            line(&[(11.5, 5.0), (13.5, 5.0)]),
            line(&[(2.5, 11.0), (4.5, 11.0)]),
            line(&[(7.5, 11.0), (13.5, 11.0)]),
            circle(10.0, 5.0, 1.5),
            circle(6.0, 11.0, 1.5),
        ],
        Icon::ChevronRight => vec![line(&[(6.5, 4.0), (10.5, 8.0), (6.5, 12.0)])],
        Icon::ChevronDown => vec![line(&[(4.0, 6.5), (8.0, 10.5), (12.0, 6.5)])],
        Icon::Caret => vec![line(&[(5.0, 6.75), (8.0, 9.75), (11.0, 6.75)])],
        Icon::Back => vec![line(&[(10.0, 3.5), (5.5, 8.0), (10.0, 12.5)])],
        Icon::Forward => vec![line(&[(6.0, 3.5), (10.5, 8.0), (6.0, 12.5)])],
        Icon::ArrowLeft => vec![line(&[(13.0, 8.0), (3.0, 8.0)]), line(&[(7.0, 4.0), (3.0, 8.0), (7.0, 12.0)])],
        Icon::ArrowRight => vec![line(&[(3.0, 8.0), (13.0, 8.0)]), line(&[(9.0, 4.0), (13.0, 8.0), (9.0, 12.0)])],
        Icon::ArrowUp => vec![line(&[(8.0, 13.0), (8.0, 3.5)]), line(&[(4.0, 7.5), (8.0, 3.5), (12.0, 7.5)])],
        Icon::External => vec![rrect(2.0, 2.0, 12.0, 12.0, 2.0), line(&[(6.0, 10.0), (10.5, 5.5)]), line(&[(6.5, 5.5), (10.5, 5.5), (10.5, 9.5)])],
        Icon::Copy => vec![
            rrect(5.0, 5.0, 8.5, 8.5, 1.5),
            rounded(&[(11.0, 5.0, 0.0), (11.0, 2.0, 1.5), (2.0, 2.0, 1.5), (2.0, 11.0, 1.5), (5.0, 11.0, 0.0)], false),
        ],
        Icon::Info => vec![circle(8.0, 8.0, 6.0), line(&[(8.0, 7.25), (8.0, 11.0)]), dot(8.0, 5.0)],
        Icon::Verify | Icon::StateManageable => vec![circle(8.0, 8.0, 6.0), line(&[(5.4, 8.2), (7.2, 10.0), (10.7, 6.3)])],
        Icon::Close => vec![line(&[(4.0, 4.0), (12.0, 12.0)]), line(&[(12.0, 4.0), (4.0, 12.0)])],
        Icon::Warning | Icon::StateToVerify => vec![
            rounded(&[(8.0, 1.8, 1.0), (14.5, 13.5, 1.0), (1.5, 13.5, 1.0)], true),
            line(&[(8.0, 6.5), (8.0, 9.5)]),
            dot(8.0, 11.4),
        ],
        Icon::StateYouDecide => vec![circle(8.0, 8.0, 6.0), Shape::circle_filled(p(8.0, 8.0), 1.9 * s, color)],
        Icon::StateManagedBy => vec![rrect(2.5, 2.5, 11.0, 11.0, 2.0), line(&[(6.25, 9.75), (10.0, 6.0)]), line(&[(7.0, 6.0), (10.0, 6.0), (10.0, 9.0)])],
        Icon::StateUnknown => Shape::dashed_line(&arc(8.0, 8.0, 6.0, 0.0, 360.0), stroke, 2.2 * s, 2.2 * s),
        Icon::System => vec![rrect(2.0, 2.5, 12.0, 8.5, 1.5), line(&[(5.5, 14.0), (10.5, 14.0)]), line(&[(8.0, 11.0), (8.0, 14.0)])],
        Icon::Rules => {
            // Libro: copertina con il dorso curvo in basso a sinistra, poi le pagine sotto.
            let mut cover = rounded_pts(&[(3.0, 13.0, 0.0), (3.0, 2.0, 1.5), (13.0, 2.0, 0.0), (13.0, 11.5, 0.0), (4.5, 11.5, 0.0)], false);
            cover.extend(arc(4.5, 13.0, 1.5, 270.0, 180.0));
            vec![
                Shape::closed_line(cover, stroke),
                rounded(&[(3.0, 13.0, 0.0), (3.0, 14.0, 1.0), (13.0, 14.0, 0.0), (13.0, 11.5, 0.0)], false),
                line(&[(6.0, 5.0), (10.0, 5.0)]),
            ]
        }
        Icon::Download => vec![
            line(&[(8.0, 2.5), (8.0, 9.5)]),
            line(&[(5.0, 7.0), (8.0, 10.0), (11.0, 7.0)]),
            rounded(&[(2.5, 10.5, 0.0), (2.5, 14.0, 1.5), (13.5, 14.0, 1.5), (13.5, 10.5, 0.0)], false),
        ],
        Icon::Programs => vec![rrect(2.5, 2.5, 4.5, 4.5, 1.2), rrect(9.0, 2.5, 4.5, 4.5, 1.2), rrect(2.5, 9.0, 4.5, 4.5, 1.2), rrect(9.0, 9.0, 4.5, 4.5, 1.2)],
        Icon::Actions => vec![
            // Cerchio quasi chiuso che parte da sinistra, con la freccia in alto: lo storico.
            Shape::line(arc(8.0, 8.0, 5.4, 180.0, -134.6), stroke),
            line(&[(2.5, 2.5), (2.5, 5.0), (5.0, 5.0)]),
            line(&[(8.0, 5.25), (8.0, 8.0), (10.0, 9.5)]),
        ],
        Icon::Sidebar => vec![rrect(2.0, 2.5, 12.0, 11.0, 2.5), line(&[(6.5, 2.5), (6.5, 13.5)])],
        Icon::Palette => vec![rrect(2.0, 2.5, 12.0, 11.0, 2.5), line(&[(5.0, 6.25), (7.0, 8.0), (5.0, 9.75)]), line(&[(8.5, 10.0), (11.0, 10.0)])],
        Icon::Shield => {
            let cubic = |a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)| -> Vec<(f32, f32)> {
                (1..=8)
                    .map(|k| {
                        let t = k as f32 / 8.0;
                        let u = 1.0 - t;
                        (u * u * u * a.0 + 3.0 * u * u * t * b.0 + 3.0 * u * t * t * c.0 + t * t * t * d.0, u * u * u * a.1 + 3.0 * u * u * t * b.1 + 3.0 * u * t * t * c.1 + t * t * t * d.1)
                    })
                    .collect()
            };
            let mut pts = vec![(8.0, 1.8), (13.0, 3.8), (13.0, 7.4)];
            pts.extend(cubic((13.0, 7.4), (13.0, 10.4), (10.9, 12.8), (8.0, 14.2)));
            pts.extend(cubic((8.0, 14.2), (5.1, 12.8), (3.0, 10.4), (3.0, 7.4)));
            pts.push((3.0, 3.8));
            vec![Shape::closed_line(pts.iter().map(|&(x, y)| p(x, y)).collect(), stroke)]
        }
        Icon::Lock => {
            let mut shackle = vec![p(5.5, 7.0)];
            shackle.extend(arc(8.0, 5.0, 2.5, 180.0, 360.0));
            shackle.push(p(10.5, 7.0));
            vec![rrect(3.5, 7.0, 9.0, 6.5, 1.5), Shape::line(shackle, stroke)]
        }
        Icon::Check => vec![line(&[(3.5, 8.5), (6.5, 11.5), (12.5, 4.5)])],
        Icon::Folder => vec![rounded(&[(2.0, 3.0, 1.5), (6.5, 3.0, 0.0), (8.0, 4.5, 0.0), (14.0, 4.5, 1.5), (14.0, 13.0, 1.5), (2.0, 13.0, 1.5)], true)],
        Icon::Properties => vec![rrect(2.5, 2.5, 11.0, 11.0, 2.0), line(&[(5.5, 6.0), (10.5, 6.0)]), line(&[(5.5, 8.5), (10.5, 8.5)]), line(&[(5.5, 11.0), (8.5, 11.0)])],
        Icon::List => vec![line(&[(5.5, 4.0), (13.5, 4.0)]), line(&[(5.5, 8.0), (13.5, 8.0)]), line(&[(5.5, 12.0), (13.5, 12.0)]), dot(2.5, 4.0), dot(2.5, 8.0), dot(2.5, 12.0)],
        Icon::Trash => vec![
            line(&[(3.0, 4.5), (13.0, 4.5)]),
            line(&[(6.5, 4.5), (6.5, 3.0), (9.5, 3.0), (9.5, 4.5)]),
            rounded(&[(4.5, 4.5, 0.0), (5.1, 14.0, 1.2), (10.9, 14.0, 1.2), (11.5, 4.5, 0.0)], false),
        ],
    };
    painter.extend(shapes);
}

/// Marchio: il segno «Metà» di Quado (`brand`), pieno nel colore dell'icona.
fn logo(painter: &Painter, rect: Rect, color: Color32) {
    crate::brand::paint(painter, rect, color);
}
