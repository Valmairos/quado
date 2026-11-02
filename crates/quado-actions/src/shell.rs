// SPDX-License-Identifier: GPL-3.0-or-later
//! Azioni dell'ispettore sempre disponibili: "Apri in Esplora file" e "Proprietà".
//!
//! Nessuna delle due modifica file né crea processi da Quado: parlano con la shell di Windows già
//! in esecuzione. "Apri in Esplora file" seleziona l'elemento nella sua cartella senza aprirlo.
//! "Proprietà" carica nel processo le estensioni della shell di terze parti, che su un
//! segnaposto cloud possono scaricare il file, quindi parte solo su richiesta esplicita.
//!
//! Percorsi oltre 260 caratteri: la shell di Windows 10 non li accetta sempre. Se il percorso lungo
//! viene rifiutato si riprova con il nome breve 8.3, quando il volume lo conserva. Il core restituisce
//! un codice, la UI lo traduce.

use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::FileSystem::GetShortPathNameW;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{SHOP_FILEPATH, SHObjectProperties, SHOpenFolderAndSelectItems, SHParseDisplayName};
use windows::core::{HSTRING, PCWSTR};

/// Esito negativo: codice HRESULT o di errore Windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellError(pub i32);

/// COM sul thread corrente per la durata del valore; bilancia solo un'inizializzazione riuscita.
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

/// Toglie il prefisso `\\?\`, che la shell non interpreta.
fn plain(path: &str) -> &str {
    path.strip_prefix(r"\\?\").unwrap_or(path)
}

/// Nome breve 8.3 del percorso, se il volume lo conserva.
fn short_path(path: &str) -> Option<String> {
    let long = HSTRING::from(format!(r"\\?\{}", plain(path)));
    let mut buf = vec![0u16; 32_768];
    // SAFETY: buffer locale con la sua lunghezza; sola lettura dei nomi.
    let n = unsafe { GetShortPathNameW(&long, Some(&mut buf)) } as usize;
    if n == 0 || n >= buf.len() {
        return None;
    }
    let s = String::from_utf16_lossy(&buf[..n]);
    let s = plain(&s).to_owned();
    (s.len() < 260).then_some(s)
}

/// Candidati da provare nell'ordine: il percorso così com'è, poi il nome breve se è più corto di 260.
fn candidates(path: &str) -> Vec<String> {
    let mut v = vec![plain(path).to_owned()];
    if v[0].len() >= 260
        && let Some(s) = short_path(path)
    {
        v.push(s);
    }
    v
}

/// Apre la cartella che contiene `path` in Esplora file e seleziona l'elemento, senza aprirlo.
pub fn reveal(path: &str) -> Result<(), ShellError> {
    let _com = Com::init();
    let mut last = ShellError(0);
    for p in candidates(path) {
        let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        // SAFETY: percorso valido per la chiamata; `pidl` allocato dalla shell e liberato sotto.
        let parsed = unsafe { SHParseDisplayName(&HSTRING::from(p.as_str()), None, &mut pidl, 0, None) };
        if let Err(e) = parsed {
            last = ShellError(e.code().0);
            continue;
        }
        // SAFETY: con zero elementi, `pidl` è l'identificatore completo dell'elemento da selezionare.
        let r = unsafe { SHOpenFolderAndSelectItems(pidl, None, 0) };
        // SAFETY: `pidl` allocato da SHParseDisplayName con l'allocatore COM.
        unsafe { CoTaskMemFree(Some(pidl as _)) };
        match r {
            Ok(()) => return Ok(()),
            Err(e) => last = ShellError(e.code().0),
        }
    }
    Err(last)
}

/// Mostra la finestra Proprietà di Windows per `path`.
pub fn properties(path: &str) -> Result<(), ShellError> {
    let _com = Com::init();
    let mut last = ShellError(0);
    for p in candidates(path) {
        // SAFETY: percorso valido per la chiamata; nessuna pagina iniziale; finestra senza proprietario.
        let ok = unsafe { SHObjectProperties(Some(HWND::default()), SHOP_FILEPATH, &HSTRING::from(p.as_str()), PCWSTR::null()) };
        if ok.as_bool() {
            return Ok(());
        }
        last = ShellError(windows::core::Error::from_thread().code().0);
    }
    Err(last)
}

/// Testo con cui la shell di Windows scrive una dimensione, come in Esplora file
/// (`StrFormatByteSizeEx`, cifre troncate). Serve al confronto con le unità di Esplora file; non modifica niente.
pub fn explorer_size_text(bytes: u64) -> Option<String> {
    use windows::Win32::UI::Shell::{SFBS_FLAGS_TRUNCATE_UNDISPLAYED_DECIMAL_DIGITS, StrFormatByteSizeEx};
    let mut buf = [0u16; 64];
    // SAFETY: buffer locale con la sua lunghezza; funzione di sola formattazione.
    unsafe { StrFormatByteSizeEx(bytes, SFBS_FLAGS_TRUNCATE_UNDISPLAYED_DECIMAL_DIGITS, &mut buf) }.ok()?;
    let n = buf.iter().position(|&u| u == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]))
}

/// Percorsi che "Apri in Esplora file" e "Proprietà" provano, nell'ordine (per i test con i percorsi lunghi).
pub fn path_candidates(path: &str) -> Vec<String> {
    candidates(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_is_removed() {
        assert_eq!(plain(r"\\?\C:\a"), r"C:\a");
        assert_eq!(plain(r"C:\a"), r"C:\a");
    }

    #[test]
    fn short_paths_only_for_long_ones() {
        assert_eq!(candidates(r"C:\Windows").len(), 1);
    }
}
