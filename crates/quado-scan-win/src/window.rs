// SPDX-License-Identifier: GPL-3.0-or-later
//! Icone della finestra di Quado dalle risorse dell'eseguibile (`crates/quado/build.rs`, file
//! `crates/quado/icona/quado.ico`): la grande (barra delle applicazioni, Alt+Tab) e la piccola (barra del
//! titolo, pulsanti piccoli della barra delle applicazioni) sono la tessera dell'eseguibile, che si vede sui
//! fondi chiari e scuri. Le misure sono quelle di Windows per i DPI del monitor della finestra.
//!
//! Nessuna scrittura su disco: solo messaggi alla finestra del processo.

use std::ptr::null;

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON,
    SendMessageW, WM_SETICON,
};

/// Gruppo dell'icona nelle risorse dell'eseguibile (`crates/quado/build.rs`).
pub const GROUP_APP: u16 = 1;

/// L'immagine del gruppo `group` più adatta a `cx` × `cy` pixel, dalle risorse dell'eseguibile.
fn load(group: u16, cx: i32, cy: i32) -> Option<HICON> {
    // SAFETY: `GetModuleHandleW(null)` restituisce il modulo dell'eseguibile, valido per tutta la vita del
    // processo; il nome della risorsa è un intero (MAKEINTRESOURCE), che `LoadImageW` accetta al posto
    // del puntatore a una stringa.
    let h = unsafe { LoadImageW(GetModuleHandleW(null()), group as usize as *const u16, IMAGE_ICON, cx, cy, LR_DEFAULTCOLOR) };
    (!h.is_null()).then_some(h)
}

/// Mette le icone alla finestra `hwnd` del processo, grande e piccola, dal gruppo dell'eseguibile alle misure
/// dei DPI della finestra. Le icone messe prima da qui si distruggono. Restituisce falso se una delle due non
/// si carica (eseguibile senza risorse, come nei test).
pub fn set_icons(hwnd: isize) -> bool {
    let hwnd = hwnd as HWND;
    // SAFETY: `hwnd` è una finestra di questo processo (da eframe); con una finestra non valida le funzioni
    // restituiscono 0 e si usano le misure del sistema.
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    // SAFETY: indici validi di GetSystemMetricsForDpi, nessun puntatore.
    let metric = |i| unsafe { GetSystemMetricsForDpi(i, dpi) };
    let icons = [(ICON_BIG, load(GROUP_APP, metric(SM_CXICON), metric(SM_CYICON))), (ICON_SMALL, load(GROUP_APP, metric(SM_CXSMICON), metric(SM_CYSMICON)))];
    let mut all = true;
    for (kind, icon) in icons {
        let Some(icon) = icon else {
            all = false;
            continue;
        };
        // SAFETY: WM_SETICON con un'icona valida; il valore restituito è l'icona di prima di quel tipo, che
        // in questa finestra mette solo questa funzione (eframe non ne riceve): si distrugge.
        let old = unsafe { SendMessageW(hwnd, WM_SETICON, kind as usize, icon as isize) };
        if old != 0 {
            // SAFETY: un'icona caricata da `LoadImageW` senza LR_SHARED in una chiamata precedente.
            unsafe { DestroyIcon(old as HICON) };
        }
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_the_resources_nothing_loads() {
        // L'eseguibile dei test non ha le risorse dell'icona: nessun gruppo si carica e nessuna finestra cambia.
        assert!(load(GROUP_APP, 32, 32).is_none());
        assert!(!set_icons(0));
    }
}
