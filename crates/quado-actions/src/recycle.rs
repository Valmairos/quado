// SPDX-License-Identifier: GPL-3.0-or-later
//! Cestino (05 D7, 03 F-AC-5, F-AC-7): il flag che dice se un elemento andrà nel Cestino e la lettura
//! del contenuto secondo Windows, in sola lettura. Lo spostamento sta in `mover` (`IFileOperation`), lo
//! svuotamento con il dialogo di Windows in `procedures`.
//!
//! Le prove di M1 (07 §3 righe 1-3) che spostavano senza finestre e svuotavano senza conferme sono in
//! `xtask` (`cargo xtask cestino`), fuori dal prodotto: il prodotto non contiene `SHERB_NOCONFIRMATION`,
//! `FOF_NOCONFIRMATION` né `FOF_NO_UI` (test d'architettura). Il codice restituisce fatti e codici, non
//! frasi (05 D13).

use std::path::Path;

use windows::Win32::UI::Shell::{SHQUERYRBINFO, SHQueryRecycleBinW};
use windows::core::HSTRING;

/// `TSF_DELETE_RECYCLE_IF_POSSIBLE`: in `PreDeleteItem` e `PostDeleteItem` l'elemento andrà (o è andato)
/// nel Cestino. Senza, verrebbe eliminato per sempre.
pub const TSF_DELETE_RECYCLE_IF_POSSIBLE: u32 = 0x80;

/// Elementi e byte del Cestino di un volume secondo Windows (`SHQueryRecycleBinW`), in sola lettura.
pub fn query(root: &Path) -> windows::core::Result<(i64, i64)> {
    let mut info = SHQUERYRBINFO { cbSize: size_of::<SHQUERYRBINFO>() as u32, ..Default::default() };
    // SAFETY: struttura inizializzata con la sua dimensione; percorso valido per la chiamata.
    unsafe { SHQueryRecycleBinW(&HSTRING::from(root.as_os_str()), &mut info)? };
    Ok((info.i64NumItems, info.i64Size))
}
