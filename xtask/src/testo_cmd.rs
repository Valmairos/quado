// SPDX-License-Identifier: GPL-3.0-or-later
//! `confronto-testo`: testo di Quado affiancato a ClearType (06 §7, 07 §1.7 criterio 4). Agente P di M6.
//!
//! Come la prova del testo di M0 (05 D2, criterio 3), ma con i font e i colori di oggi
//! (`quado_ui::fonts`, `quado_ui::tokens`) e con frasi prese dal catalogo italiano di oggi: titoli,
//! corpo, stati, numeri, etichette piccole. Per ogni riga, sopra la resa di egui, disegnata fuori schermo
//! con `egui_kittest` e wgpu come negli snapshot dell'interfaccia; sotto la resa di GDI con ClearType, lo
//! stesso motore dei controlli di Windows. Stessa dimensione em, stessi colori, nessun ridimensionamento:
//! i pezzi si copiano pixel per pixel.
//!
//! GDI accetta solo altezze intere. Dove egui usa una dimensione frazionaria (12,5 px, o 13 px al 150%,
//! cioè 19,5 px) Windows usa il pixel intero più vicino, e l'etichetta della riga lo dice.
//!
//! Uscita, in `target/testi-immagini` (o `--uscita DIR`):
//! - `confronto-testo-<100|150>-<chiaro|scuro>.png`;
//! - `confronto-testo.tsv`: per ogni riga la quota di pixel colorati tra quelli del testo (le frange di
//!   ClearType), come `confronto.tsv` di M0, e l'inchiostro delle due rese (il peso del testo) con il loro
//!   rapporto.
//!
//! Uso: `cargo xtask confronto-testo [--uscita DIR]`.

mod gdi;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use egui::{FontId, Id, LayerId, Order, Rect, pos2};
use egui_kittest::Harness;
use image::{Rgb, RgbImage};
use quado_ui::fonts::{self, FontOptions};
use quado_ui::format::Formatter;
use quado_ui::i18n::Lingue;
use quado_ui::tokens::{self, Palette, Theme};
use quado_ui::widgets;

/// Font di una riga, come lo usa l'interfaccia (`quado_ui::widgets`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Face {
    Sans,
    Medium,
    Semibold,
    Mono,
}

impl Face {
    fn egui(self, px: f32) -> FontId {
        match self {
            Self::Sans => widgets::sans(px),
            Self::Medium => widgets::medium(px),
            Self::Semibold => widgets::semibold(px),
            Self::Mono => widgets::mono(px),
        }
    }

    /// Nome di famiglia per GDI (ID 1 della tabella `name` dei file di `assets/fonts`) e peso.
    fn gdi(self) -> (&'static str, i32) {
        match self {
            Self::Sans => ("Inter", 400),
            Self::Medium => ("Inter Medium", 500),
            Self::Semibold => ("Inter SemiBold", 600),
            Self::Mono => ("IBM Plex Mono", 400),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Sans => "Inter",
            Self::Medium => "Inter medium",
            Self::Semibold => "Inter semibold",
            Self::Mono => "Plex Mono",
        }
    }
}

/// Colore del testo, dai token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    Text,
    Text2,
    Text3,
}

impl Tone {
    fn color(self, p: &Palette) -> egui::Color32 {
        match self {
            Self::Text => p.text,
            Self::Text2 => p.text_2,
            Self::Text3 => p.text_3,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Text2 => "text-2",
            Self::Text3 => "text-3",
        }
    }
}

/// Una riga del confronto: dove compare nell'app, font, dimensione em in punti, colore, testo.
struct Sample {
    role: &'static str,
    face: Face,
    px: f32,
    tone: Tone,
    text: String,
}

/// Le frasi: dal catalogo italiano di oggi, con numeri scritti da `format` come nell'app. Le dimensioni e
/// i colori sono quelli che l'interfaccia usa per quel ruolo.
fn samples(l: &Lingue, f: &Formatter) -> Vec<Sample> {
    let gib = 1u64 << 30;
    let s = |role, face, px, tone, text: String| Sample { role, face, px, tone, text };
    let states = ["stato-gestibile", "stato-da-verificare", "stato-decidi-tu", "stato-non-sappiamo"].map(|k| l.t(k));
    let managed = l.ta("stato-lo-gestisce", &[("chi", "Windows".into())]);
    vec![
        s("Titolo di schermata", Face::Semibold, 20.0, Tone::Text, l.ta("scansione-titolo", &[("volume", "Disco locale (C:)".into())])),
        s("Cifre della scansione", Face::Semibold, 24.0, Tone::Text, format!("{} · {}", f.count(2_934_423), f.size(709 * gib))),
        s("Titolo di card", Face::Semibold, 14.0, Tone::Text, l.t("top-titolo")),
        s("Corpo", Face::Sans, 13.0, Tone::Text, l.t("spostamento-reversibilita")),
        s("Riga sotto il titolo", Face::Sans, 13.0, Tone::Text2, l.t("volumi-meta")),
        s("Stati", Face::Medium, 13.0, Tone::Text, format!("{} · {} · {} · {managed} · {}", states[0], states[1], states[2], states[3])),
        s("Fatti dell'ispettore", Face::Sans, 12.5, Tone::Text2, l.ta("fatto-dimensioni-su-disco", &[("dimensione", f.size(14_806_917_939).into()), ("byte", f.count(14_806_917_939).into())])),
        s("Barra di stato", Face::Sans, 12.0, Tone::Text2, format!("{} · {}", l.t("stato-metodo"), l.t("stato-senza-privilegi"))),
        s("Etichetta di sezione", Face::Medium, 12.0, Tone::Text2, l.t("record-controlli")),
        s("Nota piccola", Face::Sans, 11.5, Tone::Text3, l.t("filtro-nota-estensione")),
        s("Numeri in tabella", Face::Sans, 13.0, Tone::Text, format!("{}   {}   {}", f.size(14_806_917_939), f.count(1_734_052), f.percent(0.162))),
        s("Percorso", Face::Mono, 12.0, Tone::Text2, r"C:\Users\demo\AppData\Local\CapCut\Apps\9.4.0.4015".to_owned()),
        s("Nomi in altre scritture", Face::Sans, 13.0, Tone::Text, "写真 Документ 사진 ファイル Ελληνικά".to_owned()),
    ]
}

/// Rettangolo in pixel `[x0, y0, x1, y1)`.
type PxRect = [i64; 4];

struct Scene<'a> {
    samples: &'a [Sample],
    theme: Theme,
    palette: Palette,
    fonts: bool,
    rects: Vec<PxRect>,
}

/// Margine dell'area di egui e distanza tra le righe, in pixel.
const MARGIN: i64 = 16;
const GAP: i64 = 14;

fn draw(ui: &mut egui::Ui, sc: &mut Scene<'_>) {
    let ctx = ui.ctx().clone();
    // Tema, token e font come nell'app, al primo frame; valgono dal successivo. Il tema conta anche per il
    // testo: egui rende le coperture dei glifi in modo diverso in chiaro e in scuro
    // (`FontColorTransferFunction`). Con i font di ripiego di Windows, per la riga delle altre scritture.
    if !sc.fonts {
        let pref = match sc.theme {
            Theme::Light => egui::ThemePreference::Light,
            Theme::Dark => egui::ThemePreference::Dark,
        };
        ctx.options_mut(|o| o.theme_preference = pref);
        tokens::apply(&ctx, &Palette::of(Theme::Light), false);
        tokens::apply(&ctx, &Palette::of(Theme::Dark), false);
        fonts::install(&ctx, FontOptions { system_fallbacks: true, segoe_family: false });
        sc.fonts = true;
        ctx.request_repaint();
        return;
    }
    let ppp = ctx.pixels_per_point();
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("confronto-testo")));
    painter.rect_filled(ctx.content_rect(), 0.0, sc.palette.panel);
    sc.rects.clear();
    let mut y = MARGIN;
    for s in sc.samples {
        let color = s.tone.color(&sc.palette);
        let galley = painter.layout_no_wrap(s.text.clone(), s.face.egui(s.px), color);
        // Posizione su pixel interi, come le etichette dell'app.
        let pos = pos2(MARGIN as f32 / ppp, y as f32 / ppp);
        let r = Rect::from_min_size(pos, galley.size());
        painter.galley(pos, galley, color);
        // Arrotondato verso l'esterno, con un margine per non tagliare discendenti e accenti (come in M0).
        let px = |v: f32| f64::from(v * ppp);
        sc.rects.push([px(r.min.x).floor() as i64 - 1, px(r.min.y).floor() as i64 - 2, px(r.max.x).ceil() as i64 + 1, px(r.max.y).ceil() as i64 + 2]);
        y = px(r.max.y).ceil() as i64 + GAP;
    }
}

/// Resa di egui fuori schermo: l'immagine e il rettangolo di ogni riga.
fn render_egui(samples: &[Sample], theme: Theme, ppp: f32) -> Result<(RgbImage, Vec<PxRect>), String> {
    let scene = Scene { samples, theme, palette: Palette::of(theme), fonts: false, rects: Vec::new() };
    let mut h = Harness::builder().with_size([1400.0, 900.0]).with_pixels_per_point(ppp).wgpu().build_ui_state(draw, scene);
    h.run_steps(4);
    let img = h.render()?;
    let rects = h.state().rects.clone();
    let want = match theme {
        Theme::Light => egui::Theme::Light,
        Theme::Dark => egui::Theme::Dark,
    };
    if h.ctx.theme() != want {
        return Err(format!("tema di egui {:?}, atteso {want:?}", h.ctx.theme()));
    }
    if rects.len() != samples.len() {
        return Err(format!("righe disegnate {} su {}", rects.len(), samples.len()));
    }
    Ok((image::DynamicImage::ImageRgba8(img).to_rgb8(), rects))
}

fn rgb(c: egui::Color32) -> [u8; 3] {
    [c.r(), c.g(), c.b()]
}

fn crop(img: &RgbImage, r: PxRect) -> RgbImage {
    let x0 = r[0].clamp(0, i64::from(img.width())) as u32;
    let y0 = r[1].clamp(0, i64::from(img.height())) as u32;
    let x1 = r[2].clamp(0, i64::from(img.width())) as u32;
    let y1 = r[3].clamp(0, i64::from(img.height())) as u32;
    image::imageops::crop_imm(img, x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)).to_image()
}

/// Tra i pixel diversi dallo sfondo, la quota con i canali R, G, B diversi tra loro: le frange di ClearType
/// (stesso criterio della prova di M0). La soglia parte dal più colorato tra fondo e testo: i grigi
/// caldi dei token (`text-3` ha 12 livelli tra il canale più alto e il più basso) non sono frange.
fn chroma_fraction(img: &RgbImage, bg: [u8; 3], fg: [u8; 3]) -> f64 {
    let spread = |c: [u8; 3]| i32::from(*c.iter().max().unwrap_or(&0)) - i32::from(*c.iter().min().unwrap_or(&0));
    let bg_spread = spread(bg).max(spread(fg));
    let (mut ink, mut chroma) = (0u64, 0u64);
    for p in img.pixels() {
        let c = p.0;
        let d: i32 = (0..3).map(|k| (i32::from(c[k]) - i32::from(bg[k])).abs()).sum();
        if d < 12 {
            continue;
        }
        ink += 1;
        if spread(c) > bg_spread + 10 {
            chroma += 1;
        }
    }
    if ink == 0 { 0.0 } else { chroma as f64 / ink as f64 }
}

/// Inchiostro di una riga: la somma, su tutti i pixel, della distanza dal fondo (0 = fondo, 1 = colore
/// opposto). Confronta il peso del testo di egui con quello di Windows, a parità di frase.
fn ink(img: &RgbImage, bg: [u8; 3]) -> f64 {
    img.pixels().map(|p| (0..3).map(|k| f64::from((i16::from(p.0[k]) - i16::from(bg[k])).unsigned_abs())).sum::<f64>() / (3.0 * 255.0)).sum()
}

/// Dimensione em in pixel per GDI: il pixel intero più vicino a quella di egui.
fn gdi_px(px: f32, ppp: f32) -> (f32, i32) {
    let exact = px * ppp;
    (exact, exact.round() as i32)
}

/// Numero con la virgola decimale, senza zeri inutili: "12,5", "19,5", "13".
fn decimal(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}

/// Una frase resa da GDI in Segoe UI per le etichette dell'immagine (non fa parte del confronto).
fn caption(text: &str, ppp: f32, bg: [u8; 3], fg: [u8; 3]) -> Result<RgbImage, String> {
    gdi::render("Segoe UI", 400, (12.0 * ppp).round() as i32, text, bg, fg)
}

fn blit(dst: &mut RgbImage, src: &RgbImage, x: u32, y: u32) {
    image::imageops::replace(dst, src, i64::from(x), i64::from(y));
}

/// Un'immagine del confronto: per ogni riga egui sopra e Windows sotto, con l'etichetta a sinistra.
fn compose(samples: &[Sample], theme: Theme, ppp: f32, tsv: &mut String, label: &str) -> Result<RgbImage, String> {
    let p = Palette::of(theme);
    let (bg, cap) = (rgb(p.panel), rgb(p.text_3));
    let (shot, rects) = render_egui(samples, theme, ppp)?;
    let mut pieces: Vec<(RgbImage, RgbImage, RgbImage, RgbImage)> = Vec::new();
    for (k, (s, r)) in samples.iter().zip(&rects).enumerate() {
        let ours = crop(&shot, *r);
        let (exact, px) = gdi_px(s.px, ppp);
        let (face, weight) = s.face.gdi();
        let native = gdi::render(face, weight, px, &s.text, bg, rgb(s.tone.color(&p)))?;
        let size = if (exact - px as f32).abs() < 0.01 {
            format!("{} px", decimal(exact))
        } else {
            format!("{} px; Windows {px} px, GDI usa pixel interi", decimal(exact))
        };
        let top = caption(&format!("{} · {} {size} · {} · egui", s.role, s.face.name(), s.tone.name()), ppp, bg, cap)?;
        let bottom = caption("Windows, GDI con ClearType", ppp, bg, cap)?;
        let fg = rgb(s.tone.color(&p));
        let (ce, cn) = (chroma_fraction(&ours, bg, fg), chroma_fraction(&native, bg, fg));
        let (ie, iw) = (ink(&ours, bg), ink(&native, bg));
        let _ = writeln!(tsv, "{label}\t{k}\t{}\t{}\t{}\t{px}\t{ce:.3}\t{cn:.3}\t{ie:.0}\t{iw:.0}\t{:.2}", s.role, s.face.name(), decimal(exact), ie / iw.max(1.0));
        pieces.push((top, ours, bottom, native));
    }
    let gap = (6.0 * ppp).round() as u32;
    let margin = (16.0 * ppp).round() as u32;
    let label_w = pieces.iter().map(|(t, _, b, _)| t.width().max(b.width())).max().unwrap_or(0) + margin;
    let content_w = pieces.iter().map(|(_, o, _, n)| o.width().max(n.width())).max().unwrap_or(0);
    let header = caption(&format!("Quado M6 · tema {} · {}% · per ogni riga: sopra egui, sotto Windows (GDI, ClearType), stessi pixel", if theme == Theme::Light { "chiaro" } else { "scuro" }, (ppp * 100.0).round()), ppp, bg, rgb(p.text_2))?;
    let mut height = margin + header.height() + 2 * gap;
    for (t, o, b, n) in &pieces {
        height += t.height().max(o.height()) + gap + b.height().max(n.height()) + 3 * gap;
    }
    let width = (margin + label_w + content_w + margin).max(margin + header.width() + margin);
    let mut out = RgbImage::from_pixel(width, height + margin, Rgb(bg));
    blit(&mut out, &header, margin, margin);
    let mut y = margin + header.height() + 2 * gap;
    let line = Rgb(rgb(p.separator));
    for (t, o, b, n) in &pieces {
        let h1 = t.height().max(o.height());
        blit(&mut out, t, margin, y + (h1 - t.height()) / 2);
        blit(&mut out, o, margin + label_w, y);
        y += h1 + gap;
        let h2 = b.height().max(n.height());
        blit(&mut out, b, margin, y + (h2 - b.height()) / 2);
        blit(&mut out, n, margin + label_w, y);
        y += h2 + gap;
        for x in margin..width - margin {
            out.put_pixel(x, y, line);
        }
        y += 2 * gap;
    }
    Ok(out)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

pub fn run(args: &[String]) -> Result<(), String> {
    let mut out_dir = workspace_root().join("target").join("testi-immagini");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--uscita" => out_dir = PathBuf::from(it.next().ok_or("--uscita vuole una cartella")?),
            other => return Err(format!("argomento sconosciuto: {other}. Uso: confronto-testo [--uscita DIR]")),
        }
    }
    let fonts_dir = workspace_root().join("assets").join("fonts");
    for f in ["Inter-Regular-tnum.ttf", "Inter-Medium-tnum.ttf", "Inter-SemiBold-tnum.ttf", "IBMPlexMono-Regular.ttf"] {
        gdi::load_private_font(&fonts_dir.join(f))?;
    }
    let l = Lingue::load("it", None);
    let f = Formatter::default();
    let rows = samples(&l, &f);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let mut tsv = String::from("immagine\triga\truolo\tfont\tpx_egui\tpx_windows\tquota_colore_egui\tquota_colore_windows\tinchiostro_egui\tinchiostro_windows\trapporto\n");
    for (ppp, zoom) in [(1.0f32, "100"), (1.5, "150")] {
        for (theme, tname) in [(Theme::Light, "chiaro"), (Theme::Dark, "scuro")] {
            let name = format!("confronto-testo-{zoom}-{tname}");
            let img = compose(&rows, theme, ppp, &mut tsv, &name)?;
            let path = out_dir.join(format!("{name}.png"));
            img.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("scritto {}", path.display());
        }
    }
    let path = out_dir.join("confronto-testo.tsv");
    std::fs::write(&path, tsv).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("scritto {}", path.display());
    Ok(())
}
