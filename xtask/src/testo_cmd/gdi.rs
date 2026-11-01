// SPDX-License-Identifier: GPL-3.0-or-later
//! Testo reso da Windows con GDI e ClearType, lo stesso motore dei controlli nativi (un'etichetta di
//! Windows disegna così), per il confronto con egui (06 §7). Fuori dal prodotto: serve solo a
//! `cargo xtask confronto-testo`.
//!
//! I font di Quado si caricano solo per questo processo (`FR_PRIVATE`): niente si installa. Se GDI
//! sostituisce il font chiesto con un altro, la resa si rifiuta invece di confrontare font diversi.

// Chiamate a GDI: ogni blocco ha il suo invariante.
#![allow(unsafe_code)]

use std::path::Path;

use image::{Rgb, RgbImage};
use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    AddFontResourceExW, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateDIBSection,
    CreateFontW, DEFAULT_CHARSET, DIB_RGB_COLORS, DT_CALCRECT, DT_NOCLIP, DT_NOPREFIX, DT_SINGLELINE, DeleteDC, DeleteObject, DrawTextW,
    FR_PRIVATE, GdiFlush, GetTextFaceW, HDC, HFONT, HGDIOBJ, OPAQUE, OUT_DEFAULT_PRECIS, SelectObject, SetBkColor, SetBkMode, SetTextColor,
};
use windows::core::HSTRING;

/// Margine attorno al testo, come in M0: due pixel per lato.
const PAD: i32 = 2;

/// Carica un file di font per questo solo processo.
pub fn load_private_font(path: &Path) -> Result<(), String> {
    let p = HSTRING::from(path.as_os_str());
    // SAFETY: `p` è una stringa terminata da zero che vive per tutta la chiamata; `FR_PRIVATE` limita il
    // font a questo processo; nessun puntatore riservato.
    let n = unsafe { AddFontResourceExW(&p, FR_PRIVATE, None) };
    if n == 0 { Err(format!("font non caricato: {}", path.display())) } else { Ok(()) }
}

fn colorref(c: [u8; 3]) -> COLORREF {
    COLORREF(u32::from(c[0]) | (u32::from(c[1]) << 8) | (u32::from(c[2]) << 16))
}

/// DC di memoria con un font selezionato; si libera da solo.
struct FontDc {
    hdc: HDC,
    font: HFONT,
    old: HGDIOBJ,
}

impl FontDc {
    fn new(face: &str, weight: i32, px: i32) -> Result<Self, String> {
        let name = HSTRING::from(face);
        // SAFETY: un DC di memoria compatibile con lo schermo, senza argomenti; si libera in `Drop`.
        let hdc = unsafe { CreateCompatibleDC(None) };
        if hdc.is_invalid() {
            return Err("CreateCompatibleDC non riuscita".into());
        }
        // SAFETY: argomenti per valore e un nome terminato da zero che vive per tutta la chiamata. Altezza
        // negativa: dimensione em in pixel, come `CreateFontW(-px, ...)` in M0. ClearType chiesto in modo
        // esplicito, indipendente dall'impostazione di sistema.
        let font = unsafe {
            CreateFontW(-px, 0, 0, 0, weight, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 0, &name)
        };
        if font.is_invalid() {
            // SAFETY: `hdc` creato sopra, non più usato.
            unsafe {
                let _ = DeleteDC(hdc);
            }
            return Err(format!("CreateFontW non riuscita per {face}"));
        }
        // SAFETY: `hdc` e `font` sono validi; il vecchio oggetto si rimette in `Drop` prima di liberarli.
        let old = unsafe { SelectObject(hdc, font.into()) };
        let dc = Self { hdc, font, old };
        let mut buf = [0u16; 64];
        // SAFETY: `buf` è scrivibile per tutta la sua lunghezza, che la funzione riceve insieme al puntatore.
        unsafe {
            GetTextFaceW(dc.hdc, Some(&mut buf));
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let got = String::from_utf16_lossy(&buf[..end]);
        if !got.eq_ignore_ascii_case(face) {
            return Err(format!("GDI ha sostituito il font: chiesto {face}, ottenuto {got}"));
        }
        Ok(dc)
    }

    /// Larghezza e altezza del testo su una riga, in pixel.
    fn measure(&self, text: &mut [u16]) -> (i32, i32) {
        let mut rc = RECT::default();
        // SAFETY: `text` e `rc` sono validi per tutta la chiamata; con `DT_CALCRECT` DrawText non disegna e
        // scrive solo `rc`.
        unsafe {
            DrawTextW(self.hdc, text, &mut rc, DT_SINGLELINE | DT_NOPREFIX | DT_CALCRECT);
        }
        (rc.right - rc.left, rc.bottom - rc.top)
    }
}

impl Drop for FontDc {
    fn drop(&mut self) {
        // SAFETY: si rimette l'oggetto selezionato all'inizio, poi si liberano font e DC, creati da `new`.
        unsafe {
            SelectObject(self.hdc, self.old);
            let _ = DeleteObject(self.font.into());
            let _ = DeleteDC(self.hdc);
        }
    }
}

/// Una riga di testo resa da GDI con ClearType su un fondo opaco `bg`, con il colore `fg`, alla dimensione
/// em `px` in pixel. Nessun ridimensionamento: l'immagine ha la misura del testo più il margine.
pub fn render(face: &str, weight: i32, px: i32, text: &str, bg: [u8; 3], fg: [u8; 3]) -> Result<RgbImage, String> {
    let dc = FontDc::new(face, weight, px)?;
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let (tw, th) = dc.measure(&mut wide);
    let (w, h) = (tw + 2 * PAD, th + 2 * PAD);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            // Negativa: righe dall'alto in basso, come l'immagine di uscita.
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY: `info` descrive una DIB a 32 bit di `w` × `h` pixel; `bits` riceve il puntatore alla memoria
    // della DIB, che resta valida finché la bitmap esiste (fino al `DeleteObject` sotto).
    let bmp = unsafe { CreateDIBSection(Some(dc.hdc), &info, DIB_RGB_COLORS, &mut bits, None, 0) }.map_err(|e| format!("CreateDIBSection: {e}"))?;
    if bits.is_null() {
        // SAFETY: bitmap creata sopra e non selezionata.
        unsafe {
            let _ = DeleteObject(bmp.into());
        }
        return Err("CreateDIBSection senza memoria".into());
    }
    let len = (w * h) as usize;
    // Fondo opaco, in BGRX.
    let fill = u32::from(bg[2]) | (u32::from(bg[1]) << 8) | (u32::from(bg[0]) << 16);
    {
        // SAFETY: la DIB ha `w * h` pixel da 4 byte, allineati a 4; il riferimento finisce con questo
        // blocco, prima che GDI ci disegni.
        let pixels = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u32>(), len) };
        pixels.fill(fill);
    }
    let mut rc = RECT { left: PAD, top: PAD, right: w, bottom: h };
    // SAFETY: `bmp` e il DC sono validi; il testo e `rc` vivono per tutta la chiamata; dopo il disegno si
    // rimette la bitmap di prima e si svuota la coda di GDI prima di leggere la memoria della DIB.
    unsafe {
        let prev = SelectObject(dc.hdc, bmp.into());
        SetBkMode(dc.hdc, OPAQUE);
        SetBkColor(dc.hdc, colorref(bg));
        SetTextColor(dc.hdc, colorref(fg));
        DrawTextW(dc.hdc, &mut wide, &mut rc, DT_SINGLELINE | DT_NOPREFIX | DT_NOCLIP);
        let _ = GdiFlush();
        SelectObject(dc.hdc, prev);
    }
    // SAFETY: stessa memoria di sopra; GDI ha finito di scriverci (`GdiFlush`) e la bitmap esiste ancora.
    let pixels = unsafe { std::slice::from_raw_parts(bits.cast::<u32>(), len) };
    let mut img = RgbImage::new(w as u32, h as u32);
    for (k, px) in pixels.iter().enumerate() {
        let v = *px;
        img.put_pixel((k as i32 % w) as u32, (k as i32 / w) as u32, Rgb([(v >> 16) as u8, (v >> 8) as u8, v as u8]));
    }
    // SAFETY: la bitmap non è più selezionata in nessun DC e la sua memoria non si usa più.
    unsafe {
        let _ = DeleteObject(bmp.into());
    }
    Ok(img)
}
