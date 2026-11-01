// SPDX-License-Identifier: GPL-3.0-or-later
//! Procedure di Windows (03 F-AC-1, 05 D7): "Svuota il Cestino" con il dialogo di conferma di Windows
//! (`SHEmptyRecycleBinW` con i flag a zero: dialogo di conferma, avanzamento e suono di Windows; mai
//! `SHERB_NOCONFIRMATION`). Le pagine da aprire ("Apri il Cestino", Impostazioni › Archiviazione e File
//! temporanei) passano dal lanciatore tipizzato di `launch`.
//!
//! Lo svuotamento accetta solo la radice di un'unità (`C:\`): mai la radice nulla, che per Windows vuol
//! dire i Cestini di tutti i volumi, mai una sottocartella o un percorso di rete. Prima del dialogo
//! l'interfaccia dice quanti altri elementi e quanti GB contiene il Cestino secondo Windows (F-AC-1).

use windows::Win32::Foundation::{E_INVALIDARG, HWND};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Shell::SHEmptyRecycleBinW;
use windows::core::HSTRING;

use crate::shell::ShellError;

/// Codice della procedura "Svuota il Cestino" nel registro (`InterventionKind::Procedure`).
pub const EMPTY_RECYCLE_BIN: &str = "cestino-svuota";

/// Flag di `SHEmptyRecycleBinW`: nessuno. Windows mostra il dialogo di conferma, l'avanzamento e il
/// suono; se l'utente annulla non elimina niente.
const EMPTY_FLAGS: u32 = 0;

/// `root` è la radice di un'unità con lettera: lettera ASCII, due punti, barra rovesciata, e nient'altro.
/// Vuota (la radice nulla), `C:` senza barra, `C:/`, sottocartelle, `\\?\C:\` e percorsi UNC no.
pub fn is_drive_root(root: &str) -> bool {
    matches!(root.as_bytes(), [letter, b':', b'\\'] if letter.is_ascii_alphabetic())
}

/// COM sul thread corrente per la durata del valore, come in `shell.rs`: il dialogo della shell può
/// caricare estensioni che lo richiedono. Bilancia solo un'inizializzazione riuscita.
struct Com(bool);

impl Com {
    fn init() -> Self {
        // SAFETY: inizializzazione COM del thread corrente. S_OK e S_FALSE vanno bilanciati; con
        // RPC_E_CHANGED_MODE il thread è già in un altro modello e non si tocca.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Com(hr.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: bilancia la CoInitializeEx riuscita di `init` sullo stesso thread.
            unsafe { CoUninitialize() };
        }
    }
}

/// Svuota il Cestino del volume `root` (`C:\`) mostrando il dialogo di conferma di Windows, con `owner`
/// (HWND) come finestra proprietaria. Se l'utente annulla, Windows non elimina niente.
///
/// Una radice che non è quella di un'unità (vedi [`is_drive_root`]) si rifiuta con `E_INVALIDARG`
/// senza chiamare Windows. Gli altri errori sono l'HRESULT di Windows; l'esito vero si legge dalle misure
/// dopo lo svuotamento (F-VE-2), non dal codice: quale codice dia l'annullamento del dialogo non è
/// verificato.
pub fn empty_recycle_bin(root: &str, owner: Option<isize>) -> Result<(), ShellError> {
    if !is_drive_root(root) {
        return Err(ShellError(E_INVALIDARG.0));
    }
    let _com = Com::init();
    let root = HSTRING::from(root);
    let owner = owner.map(|h| HWND(h as *mut core::ffi::c_void));
    // SAFETY: `root` è una stringa terminata da zero che vive fino al ritorno e non è vuota (radice di
    // un'unità controllata sopra: mai NULL, che svuoterebbe tutti i Cestini). `owner` è l'HWND della
    // finestra dell'interfaccia o nessuno. Flag a zero: il dialogo di conferma di Windows decide.
    unsafe { SHEmptyRecycleBinW(owner, &root, EMPTY_FLAGS) }.map_err(|e| ShellError(e.code().0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_drive_roots() {
        for ok in [r"C:\", r"c:\", r"R:\", r"Z:\"] {
            assert!(is_drive_root(ok), "{ok}");
        }
        for bad in ["", "C:", "C:/", r"C:\Windows", r"\\server\share\", r"\\?\C:\", r"1:\", r"é:\"] {
            assert!(!is_drive_root(bad), "{bad:?}");
        }
    }
}
