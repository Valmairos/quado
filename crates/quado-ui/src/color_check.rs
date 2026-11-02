// SPDX-License-Identifier: GPL-3.0-or-later
//! Controlli sui colori: contrasto WCAG e distanza tra tinte in visione normale e con la simulazione delle
//! discromatopsie (Machado, Oliveira, Fernandes 2009, severità piena), misurata come ΔE in OKLab × 100.

use egui::Color32;

/// Soglia minima di ΔE OKLab × 100 tra due tinte di dati. Le coppie giudicate distinguibili stavano tutte
/// sopra 3,2; la coppia da correggere (Windows e Non classificato, chiaro, protanopia) era a 1,8.
pub const MIN_DELTA_E: f64 = 3.0;

fn lin(c: u8) -> f64 {
    let v = f64::from(c) / 255.0;
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

fn linear(c: Color32) -> [f64; 3] {
    [lin(c.r()), lin(c.g()), lin(c.b())]
}

/// Luminanza relativa WCAG 2.
pub fn luminance(c: Color32) -> f64 {
    let [r, g, b] = linear(c);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Rapporto di contrasto WCAG 2 (da 1 a 21).
pub fn contrast(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Tipo di visione dei colori simulato.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vision {
    Normal,
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

impl Vision {
    pub const ALL: [Self; 4] = [Self::Normal, Self::Protanopia, Self::Deuteranopia, Self::Tritanopia];

    /// Matrici di Machado 2009 a severità 1, su RGB lineare.
    fn matrix(self) -> [[f64; 3]; 3] {
        match self {
            Self::Normal => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            Self::Protanopia => [[0.152_286, 1.052_583, -0.204_868], [0.114_503, 0.786_281, 0.099_216], [-0.003_882, -0.048_116, 1.051_998]],
            Self::Deuteranopia => [[0.367_322, 0.860_646, -0.227_968], [0.280_085, 0.672_501, 0.047_413], [-0.011_820, 0.042_940, 0.968_881]],
            Self::Tritanopia => [[1.255_528, -0.076_749, -0.178_779], [-0.078_411, 0.930_809, 0.147_602], [0.004_733, 0.691_367, 0.303_900]],
        }
    }
}

fn simulate(c: Color32, v: Vision) -> [f64; 3] {
    let x = linear(c);
    let m = v.matrix();
    let mut out = [0.0; 3];
    for (o, row) in out.iter_mut().zip(m) {
        *o = (row[0] * x[0] + row[1] * x[1] + row[2] * x[2]).clamp(0.0, 1.0);
    }
    out
}

fn oklab([r, g, b]: [f64; 3]) -> [f64; 3] {
    let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
    let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
    let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;
    let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

/// ΔE OKLab × 100 tra due colori, visti con la visione `v`.
pub fn delta_e(a: Color32, b: Color32, v: Vision) -> f64 {
    let (x, y) = (oklab(simulate(a, v)), oklab(simulate(b, v)));
    ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt() * 100.0
}

/// La coppia peggiore di un insieme di tinte: (ΔE minimo, visione, indici).
pub fn worst_pair(colors: &[Color32]) -> (f64, Vision, usize, usize) {
    let mut worst = (f64::INFINITY, Vision::Normal, 0, 0);
    for v in Vision::ALL {
        for i in 0..colors.len() {
            for j in i + 1..colors.len() {
                let d = delta_e(colors[i], colors[j], v);
                if d < worst.0 {
                    worst = (d, v, i, j);
                }
            }
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_contrasts() {
        assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 1e-9);
        assert!((contrast(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 1e-9);
        // Programmi (chiaro) con l'etichetta bianca: 5,81:1.
        let app = Color32::from_rgb(0x34, 0x60, 0xc4);
        assert!((contrast(app, Color32::WHITE) - 5.81).abs() < 0.01);
    }

    #[test]
    fn same_numbers_as_the_proposal_script() {
        // Lo script che ha verificato i token della proposta visiva usa le stesse formule: i valori del suo
        // rapporto si ritrovano qui.
        let hex = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        // Variante A, scuro: testo su bg 15,63:1; tratteggio su track 2,87:1.
        assert!((contrast(hex(0xF5F5F7), hex(0x1C1C1E)) - 15.63).abs() < 0.01);
        assert!((contrast(hex(0x7A7A80), hex(0x353538)) - 2.87).abs() < 0.01);
        // Colori dei dati predefiniti in scuro: coppia peggiore Programmi / Windows in tritanopia, ΔE 11,7.
        let d = delta_e(hex(0x6494D6), hex(0x79C5BD), Vision::Tritanopia);
        assert!((d - 11.7).abs() < 0.06, "ΔE {d:.2}");
    }

    #[test]
    fn the_light_grey_problem_is_reproduced() {
        // La coppia troppo vicina: Windows #1A8F7C e Non classificato #8F8B84 in chiaro, protanopia, ΔE 1,8.
        let win = Color32::from_rgb(0x1a, 0x8f, 0x7c);
        let grey = Color32::from_rgb(0x8f, 0x8b, 0x84);
        let d = delta_e(win, grey, Vision::Protanopia);
        assert!((d - 1.8).abs() < 0.15, "ΔE {d:.2}");
    }
}

