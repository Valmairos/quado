// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask icona [--anteprima DIR]`: l'icona di Quado dal segno «Metà» (`quado_ui::brand`, la stessa
//! geometria degli SVG di `docs/brand`), scritta in `crates/quado/icona/quado.ico`. Il `build.rs` di
//! `quado` la incorpora nell'eseguibile come risorsa; un test controlla che il file corrisponda alla geometria.
//!
//! `quado.ico` è l'icona dell'eseguibile (Esplora file, menu Start, barra delle applicazioni) e della finestra
//! (barra del titolo, Alt+Tab): la tessera grafite con il segno chiaro e il bordo interno chiaro a 16, 20, 24,
//! 32, 40, 48, 64 e 256 px, ogni misura col suo disegno (`brand::Tile`). Si vede su fondo chiaro e scuro: la
//! tessera sul chiaro, il segno e il bordo sullo scuro.
//!
//! Antialiasing per copertura: per ogni pixel la frazione dell'area dentro la forma, con 16 × 16 campioni sui
//! pixel di bordo (quelli lontani dal bordo più di mezza diagonale sono pieni o vuoti). I colori si mescolano
//! in sRGB, come fa il browser con gli SVG. Fino a 64 px le immagini sono bitmap a 32 bit con la maschera; a
//! 256 px PNG. Con `--anteprima` anche le immagini in PNG e la tavola `tavola.png`: l'icona a ogni misura sui
//! fondi di Windows chiari e scuri, ingrandita 4 volte senza sfumare i pixel.

use std::path::{Path, PathBuf};

use quado_ui::brand::{GRAPHITE, LIGHT, RIM, Tile};

/// Cartella delle icone, relativa alla radice del repository.
pub const DIR: &str = r"crates\quado\icona";

/// Campioni per lato di un pixel di bordo.
const SAMPLES: u32 = 16;

/// Il file dell'icona e le sue misure, dalla più piccola.
pub const FILE: &str = "quado.ico";
pub const SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

/// Fondi della tavola: chiari e scuri di Windows 10 e 11, il colore della tessera, il nero e l'accento
/// predefinito.
const BACKGROUNDS: [[u8; 3]; 8] = [
    [0xFF, 0xFF, 0xFF],
    [0xF3, 0xF3, 0xF3],
    [0x2B, 0x2B, 0x2B],
    [0x20, 0x20, 0x20],
    [0x19, 0x19, 0x19],
    [0x1D, 0x1D, 0x1F],
    [0x00, 0x00, 0x00],
    [0x00, 0x78, 0xD4],
];

/// Copertura di ogni pixel di un'immagine di `size` pixel: `dist` è la distanza con segno dalla forma in
/// unità della griglia (negativa dentro, mai più grande della distanza vera), `scale` i pixel per unità,
/// `origin` la coordinata della griglia nell'angolo in alto a sinistra.
fn coverage(size: u32, scale: f32, origin: f32, dist: impl Fn(f32, f32) -> f32) -> Vec<f32> {
    // Mezza diagonale del pixel, con un po' di margine, in unità della griglia.
    let half = 0.75 / scale;
    let mut out = Vec::with_capacity((size * size) as usize);
    for py in 0..size {
        for px in 0..size {
            let at = |x: f32, y: f32| (origin + x / scale, origin + y / scale);
            let (cx, cy) = at(px as f32 + 0.5, py as f32 + 0.5);
            let d = dist(cx, cy);
            let c = if d > half {
                0.0
            } else if d < -half {
                1.0
            } else {
                let mut inside = 0u32;
                for sy in 0..SAMPLES {
                    for sx in 0..SAMPLES {
                        let (x, y) = at(px as f32 + (sx as f32 + 0.5) / SAMPLES as f32, py as f32 + (sy as f32 + 0.5) / SAMPLES as f32);
                        if dist(x, y) < 0.0 {
                            inside += 1;
                        }
                    }
                }
                inside as f32 / (SAMPLES * SAMPLES) as f32
            };
            out.push(c);
        }
    }
    out
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// L'immagine RGBA (righe dall'alto, alfa non premoltiplicata) di `size` pixel: la tessera, il bordo interno
/// e il segno sopra.
pub fn render(size: u32) -> Vec<u8> {
    let t = Tile::for_pixels(size);
    let tile = coverage(size, 1.0, 0.0, |x, y| t.distance(x, y, 0.0));
    let inner = coverage(size, 1.0, 0.0, |x, y| t.distance(x, y, t.rim));
    let mark = coverage(size, 1.0, 0.0, |x, y| t.mark.distance(x, y));
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for ((o, i), m) in tile.into_iter().zip(inner).zip(mark) {
        // Parte della tessera nel pixel che è bordo.
        let rim = if o > 0.0 { ((o - i) / o).clamp(0.0, 1.0) } else { 0.0 };
        for k in 0..3 {
            let base = f32::from(RIM[k]) * rim + f32::from(GRAPHITE[k]) * (1.0 - rim);
            rgba.push(byte((base * (1.0 - m) + f32::from(LIGHT[k]) * m) / 255.0));
        }
        rgba.push(byte(o));
    }
    rgba
}

/// La tavola: una riga per fondo, una colonna per misura fino a 64 px, ingrandita `zoom` volte.
pub fn sheet(images: &[(u32, Vec<u8>)], zoom: u32) -> image::RgbaImage {
    let small: Vec<&(u32, Vec<u8>)> = images.iter().filter(|(s, _)| *s <= 64).collect();
    let pad = 12;
    let width = pad + small.iter().map(|(s, _)| s + pad).sum::<u32>();
    let row = 64 + 2 * pad;
    let mut out = image::RgbaImage::new(width * zoom, row * BACKGROUNDS.len() as u32 * zoom);
    for (r, bg) in BACKGROUNDS.iter().enumerate() {
        let y0 = r as u32 * row;
        for y in y0..y0 + row {
            for x in 0..width {
                put(&mut out, x, y, zoom, *bg);
            }
        }
        let mut x0 = pad;
        for (s, rgba) in &small {
            let top = y0 + pad + (64 - s) / 2;
            for y in 0..*s {
                for x in 0..*s {
                    let i = ((y * s + x) * 4) as usize;
                    let a = f32::from(rgba[i + 3]) / 255.0;
                    let px = [0, 1, 2].map(|k| byte((f32::from(rgba[i + k]) * a + f32::from(bg[k]) * (1.0 - a)) / 255.0));
                    put(&mut out, x0 + x, top + y, zoom, px);
                }
            }
            x0 += s + pad;
        }
    }
    out
}

/// Un pixel della tavola, ingrandito in un quadrato di `zoom` pixel.
fn put(img: &mut image::RgbaImage, x: u32, y: u32, zoom: u32, rgb: [u8; 3]) {
    for dy in 0..zoom {
        for dx in 0..zoom {
            img.put_pixel(x * zoom + dx, y * zoom + dy, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
        }
    }
}

fn u16le(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn u32le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Un'immagine come la vuole il formato `.ico`: bitmap a 32 bit (BITMAPINFOHEADER, pixel BGRA dal basso,
/// maschera a 1 bit con 1 dove l'alfa è zero) fino a 64 px, PNG a 256.
fn encode_image(size: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    if size >= 256 {
        let img = image::RgbaImage::from_raw(size, size, rgba.to_vec()).ok_or("immagine")?;
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        return Ok(out.into_inner());
    }
    let row_mask = size.div_ceil(32) * 4;
    let mut v = Vec::new();
    u32le(&mut v, 40);
    u32le(&mut v, size);
    u32le(&mut v, size * 2);
    u16le(&mut v, 1);
    u16le(&mut v, 32);
    u32le(&mut v, 0);
    u32le(&mut v, size * size * 4 + row_mask * size);
    v.extend_from_slice(&[0u8; 16]);
    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            v.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    for y in (0..size).rev() {
        let mut row = vec![0u8; row_mask as usize];
        for x in 0..size {
            if rgba[((y * size + x) * 4 + 3) as usize] == 0 {
                row[(x / 8) as usize] |= 0x80 >> (x % 8);
            }
        }
        v.extend_from_slice(&row);
    }
    Ok(v)
}

/// Il file `.ico` con le immagini date (lato, RGBA).
pub fn encode_ico(images: &[(u32, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let encoded: Vec<Vec<u8>> = images.iter().map(|(s, rgba)| encode_image(*s, rgba)).collect::<Result<_, _>>()?;
    let mut v = Vec::new();
    u16le(&mut v, 0);
    u16le(&mut v, 1);
    u16le(&mut v, images.len() as u16);
    let mut offset = 6 + 16 * images.len() as u32;
    for ((size, _), data) in images.iter().zip(&encoded) {
        let side = if *size >= 256 { 0 } else { *size as u8 };
        v.extend_from_slice(&[side, side, 0, 0]);
        u16le(&mut v, 1);
        u16le(&mut v, 32);
        u32le(&mut v, data.len() as u32);
        u32le(&mut v, offset);
        offset += data.len() as u32;
    }
    for data in encoded {
        v.extend_from_slice(&data);
    }
    Ok(v)
}

fn read_u16(b: &[u8], at: usize) -> Result<u16, String> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| "file troncato".to_owned())
}

fn read_u32(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| "file troncato".to_owned())
}

/// Le immagini di un file `.ico` scritto da [`encode_ico`]: lato e RGBA.
pub fn decode_ico(b: &[u8]) -> Result<Vec<(u32, Vec<u8>)>, String> {
    if read_u16(b, 0)? != 0 || read_u16(b, 2)? != 1 {
        return Err("non è un file .ico".into());
    }
    let n = read_u16(b, 4)? as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let e = 6 + 16 * i;
        let side = b.get(e).copied().ok_or("file troncato")?;
        let size = if side == 0 { 256 } else { u32::from(side) };
        let (len, offset) = (read_u32(b, e + 8)? as usize, read_u32(b, e + 12)? as usize);
        let data = b.get(offset..offset + len).ok_or("immagine fuori dal file")?;
        if data.starts_with(b"\x89PNG") {
            let img = image::load_from_memory_with_format(data, image::ImageFormat::Png).map_err(|e| e.to_string())?.to_rgba8();
            out.push((size, img.into_raw()));
            continue;
        }
        if read_u32(data, 0)? != 40 || read_u32(data, 4)? != size || read_u32(data, 8)? != 2 * size || read_u16(data, 14)? != 32 {
            return Err(format!("bitmap di {size} px inattesa"));
        }
        let mut rgba = vec![0u8; (size * size * 4) as usize];
        for y in 0..size {
            for x in 0..size {
                let src = 40 + (((size - 1 - y) * size + x) * 4) as usize;
                let px = data.get(src..src + 4).ok_or("bitmap troncata")?;
                let dst = ((y * size + x) * 4) as usize;
                rgba[dst..dst + 4].copy_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
        }
        out.push((size, rgba));
    }
    Ok(out)
}

/// Le immagini dell'icona, disegnate.
pub fn rendered() -> Vec<(u32, Vec<u8>)> {
    SIZES.iter().map(|&s| (s, render(s))).collect()
}

fn repo_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

pub fn run(args: &[String]) -> Result<(), String> {
    let preview = match args {
        [] => None,
        [flag, dir] if flag == "--anteprima" => Some(PathBuf::from(dir)),
        _ => return Err("uso: cargo xtask icona [--anteprima DIR]".into()),
    };
    let dir = repo_dir().join(DIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let images = rendered();
    let bytes = encode_ico(&images)?;
    let path = dir.join(FILE);
    std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    // Il file si rilegge: le immagini devono essere quelle disegnate.
    let back = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if decode_ico(&back)? != images {
        return Err(format!("{}: riletto diverso da quello scritto", path.display()));
    }
    let sizes: Vec<String> = images.iter().map(|(s, _)| s.to_string()).collect();
    println!("{DIR}\\{FILE}: {} px, {} byte, riletto", sizes.join(", "), bytes.len());
    if let Some(out) = preview {
        std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        let p = out.join("tavola.png");
        sheet(&images, 4).save(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        for (s, rgba) in images {
            let p = out.join(format!("quado-{s}.png"));
            image::RgbaImage::from_raw(s, s, rgba).ok_or("immagine")?.save(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        println!("anteprime PNG e tavola in {}", out.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_committed_icon_matches_the_geometry_of_the_mark() {
        // Chi cambia il segno o la tessera in `quado_ui::brand` rigenera l'icona con `cargo xtask icona`.
        let path = repo_dir().join(DIR).join(FILE);
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let got = decode_ico(&bytes).expect("ico");
        let want = rendered();
        assert_eq!(got.iter().map(|(s, _)| *s).collect::<Vec<_>>(), SIZES);
        for ((s, g), (_, w)) in got.iter().zip(&want) {
            assert!(g == w, "{FILE} a {s} px: rigenera con `cargo xtask icona`");
        }
    }

    #[test]
    fn small_sizes_have_sharp_cuts_and_a_visible_rim() {
        // 16 px: tessera di 15, segno da 3 a 12, taglio orizzontale sulla riga 7 e verticale sulla colonna 7
        // sotto il taglio. Colonna e riga 15 vuote.
        let img = render(16);
        let p = |x: u32, y: u32| -> [u8; 4] { img[((y * 16 + x) * 4) as usize..((y * 16 + x) * 4 + 4) as usize].try_into().unwrap() };
        let light = [LIGHT[0], LIGHT[1], LIGHT[2], 255];
        let graphite = [GRAPHITE[0], GRAPHITE[1], GRAPHITE[2], 255];
        assert!((0..16).all(|i| p(15, i)[3] == 0 && p(i, 15)[3] == 0), "ultima riga e colonna libere");
        assert_eq!(p(0, 0)[3], 0, "angolo arrotondato");
        assert_eq!((p(7, 4), p(4, 10), p(10, 10)), (light, light, light));
        assert_eq!((p(7, 7), p(7, 10), p(4, 2), p(2, 7)), (graphite, graphite, graphite, graphite));
        // Bordo: la prima colonna a metà altezza è del colore del bordo, piena.
        assert_eq!(p(0, 7), [RIM[0], RIM[1], RIM[2], 255]);
        // Ogni misura: il centro del taglio orizzontale è grafite, il centro della metà alta è chiaro.
        for s in SIZES {
            let t = Tile::for_pixels(s);
            let img = render(s);
            let at = |x: f32, y: f32| -> [u8; 4] {
                let i = ((y as u32 * s + x as u32) * 4) as usize;
                img[i..i + 4].try_into().unwrap()
            };
            let [top, left, _] = t.mark.parts;
            let cx = t.side / 2.0;
            assert_eq!(at(cx, (top.y1 + left.y0) / 2.0), graphite, "{s} px: taglio");
            assert_eq!(at(cx, (top.y0 + top.y1) / 2.0), light, "{s} px: segno");
        }
    }

    #[test]
    fn the_sheet_shows_every_small_size_on_every_background() {
        let img = sheet(&rendered(), 1);
        assert_eq!(img.height(), 8 * 88);
        assert_eq!(img.width(), 12 + [16, 20, 24, 32, 40, 48, 64].iter().map(|s| s + 12).sum::<u32>());
        // Sul fondo nero, a 16 px, il centro della metà alta del segno è chiaro e l'angolo è nero.
        let (x0, y0) = (12, 6 * 88 + 12 + 24);
        assert_eq!(img.get_pixel(x0 + 7, y0 + 4).0, [LIGHT[0], LIGHT[1], LIGHT[2], 255]);
        assert_eq!(img.get_pixel(x0, y0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn ico_round_trip() {
        let images: Vec<(u32, Vec<u8>)> = [16u32, 20, 256].iter().map(|&s| (s, render(s))).collect();
        let bytes = encode_ico(&images).unwrap();
        assert_eq!(decode_ico(&bytes).unwrap(), images);
        assert!(decode_ico(&bytes[..40]).is_err());
        assert!(decode_ico(b"MZ\0\0").is_err());
    }
}
