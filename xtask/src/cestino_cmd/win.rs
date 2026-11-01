// SPDX-License-Identifier: GPL-3.0-or-later
//! Prove di M1 sul Cestino (07 §3 righe 1-3), fuori dal prodotto: spostamento con `IFileOperation`
//! senza finestre di Windows (`FOF_NO_UI`) e svuotamento senza conferme (`SHERB_NOCONFIRMATION`), solo
//! sul VHDX di prova. Il prodotto non contiene questi flag (F-AC-1, F-AC-5, test d'architettura di
//! `quado-actions`): fino a M4 stavano in `quado_actions::recycle`, da M5 stanno qui.
//!
//! Servono a vedere quali flag riceve `PreDeleteItem` quando un elemento finirebbe eliminato per sempre
//! (troppo grande per il Cestino), e quale valore di ritorno salva quel solo elemento senza fermare gli
//! altri.

// Chiamate COM e della shell di Windows: ogni blocco ha il suo invariante.
#![allow(unsafe_code)]

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use quado_actions::recycle::TSF_DELETE_RECYCLE_IF_POSSIBLE;
use quado_testkit::trees;
use windows::Win32::Foundation::{E_ABORT, E_INVALIDARG, HWND};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize};
use windows::Win32::UI::Shell::{
    FILEOPERATION_FLAGS, FOF_ALLOWUNDO, FOF_NO_UI, FOFX_RECYCLEONDELETE, FileOperation, IFileOperation, IFileOperationProgressSink,
    IFileOperationProgressSink_Impl, IShellItem, SHCreateItemFromParsingName, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
    SHEmptyRecycleBinW, SIGDN_FILESYSPATH,
};
use windows::core::{HRESULT, HSTRING, PCWSTR, Ref, implement};
use windows_core::ComObject;

/// Lettura del Cestino secondo Windows: quella del prodotto, in sola lettura.
pub use quado_actions::recycle::query;

/// Cosa rispondere a `PreDeleteItem` quando l'elemento non andrebbe nel Cestino.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guard {
    /// Nessuna protezione (solo per la prova di controllo).
    None,
    /// Restituisce questo codice per l'elemento che verrebbe eliminato per sempre.
    Reject(HRESULT),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Pre { path: String, flags: u32, rejected: bool },
    Post { path: String, flags: u32, hr: HRESULT, recycled: bool },
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub perform: HRESULT,
    pub aborted: bool,
    pub events: Vec<Event>,
}

fn item_path(item: &IShellItem) -> String {
    // SAFETY: `GetDisplayName` restituisce una stringa allocata con CoTaskMemAlloc, letta e liberata qui.
    unsafe {
        match item.GetDisplayName(SIGDN_FILESYSPATH) {
            Ok(p) => {
                let s = p.to_string().unwrap_or_default();
                CoTaskMemFree(Some(p.0 as _));
                s
            }
            Err(_) => String::new(),
        }
    }
}

#[implement(IFileOperationProgressSink)]
struct Sink {
    guard: Guard,
    events: RefCell<Vec<Event>>,
}

impl IFileOperationProgressSink_Impl for Sink_Impl {
    fn StartOperations(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn FinishOperations(&self, _: HRESULT) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreRenameItem(&self, _: u32, _: Ref<IShellItem>, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostRenameItem(&self, _: u32, _: Ref<IShellItem>, _: &PCWSTR, _: HRESULT, _: Ref<IShellItem>) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreMoveItem(&self, _: u32, _: Ref<IShellItem>, _: Ref<IShellItem>, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostMoveItem(&self, _: u32, _: Ref<IShellItem>, _: Ref<IShellItem>, _: &PCWSTR, _: HRESULT, _: Ref<IShellItem>) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreCopyItem(&self, _: u32, _: Ref<IShellItem>, _: Ref<IShellItem>, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostCopyItem(&self, _: u32, _: Ref<IShellItem>, _: Ref<IShellItem>, _: &PCWSTR, _: HRESULT, _: Ref<IShellItem>) -> windows::core::Result<()> {
        Ok(())
    }
    fn PreDeleteItem(&self, dwflags: u32, psiitem: Ref<IShellItem>) -> windows::core::Result<()> {
        let path = psiitem.as_ref().map(item_path).unwrap_or_default();
        let permanent = dwflags & TSF_DELETE_RECYCLE_IF_POSSIBLE == 0;
        let reject = match self.guard {
            Guard::Reject(hr) if permanent => Some(hr),
            _ => None,
        };
        self.events.borrow_mut().push(Event::Pre { path, flags: dwflags, rejected: reject.is_some() });
        match reject {
            Some(hr) => Err(hr.into()),
            None => Ok(()),
        }
    }
    fn PostDeleteItem(&self, dwflags: u32, psiitem: Ref<IShellItem>, hrdelete: HRESULT, psinewlycreated: Ref<IShellItem>) -> windows::core::Result<()> {
        let path = psiitem.as_ref().map(item_path).unwrap_or_default();
        self.events.borrow_mut().push(Event::Post { path, flags: dwflags, hr: hrdelete, recycled: psinewlycreated.as_ref().is_some() });
        Ok(())
    }
    fn PreNewItem(&self, _: u32, _: Ref<IShellItem>, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }
    fn PostNewItem(&self, _: u32, _: Ref<IShellItem>, _: &PCWSTR, _: &PCWSTR, _: u32, _: HRESULT, _: Ref<IShellItem>) -> windows::core::Result<()> {
        Ok(())
    }
    fn UpdateProgress(&self, _: u32, _: u32) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResetTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> windows::core::Result<()> {
        Ok(())
    }
}

/// COM inizializzato sul thread corrente per la durata del valore.
struct Com;

impl Com {
    fn init() -> Self {
        // SAFETY: inizializzazione COM del thread corrente, bilanciata da CoUninitialize nel drop.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Com
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: bilancia CoInitializeEx di `init` sullo stesso thread.
        unsafe { CoUninitialize() };
    }
}

/// `root` è la radice di un VHDX di prova montato da `cargo xtask vhdx crea`.
fn is_test_volume(root: &Path) -> bool {
    let r = root.to_string_lossy().to_ascii_uppercase();
    trees::mounted_vhdx().into_iter().any(|(_, p)| p.to_string_lossy().to_ascii_uppercase() == r)
}

/// Sposta `paths` nel Cestino in un'unica operazione, senza finestre di Windows, registrando ogni evento.
pub fn recycle(paths: &[PathBuf], guard: Guard) -> windows::core::Result<Outcome> {
    let _com = Com::init();
    // SAFETY: COM è inizializzato; gli oggetti si usano solo su questo thread.
    unsafe {
        let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?;
        op.SetOperationFlags(FILEOPERATION_FLAGS(FOF_NO_UI.0 | FOF_ALLOWUNDO.0 | FOFX_RECYCLEONDELETE.0))?;
        let sink_obj = ComObject::new(Sink { guard, events: RefCell::new(Vec::new()) });
        let sink: IFileOperationProgressSink = sink_obj.to_interface();
        let cookie = op.Advise(&sink)?;
        for p in paths {
            let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(p.as_os_str()), None)?;
            op.DeleteItem(&item, None)?;
        }
        let perform = match op.PerformOperations() {
            Ok(()) => HRESULT(0),
            Err(e) => e.code(),
        };
        let aborted = op.GetAnyOperationsAborted().map(|b| b.as_bool()).unwrap_or(false);
        let _ = op.Unadvise(cookie);
        let events = sink_obj.events.borrow().clone();
        Ok(Outcome { perform, aborted, events })
    }
}

/// Svuota senza conferme né suoni il Cestino del VHDX di prova `root`. Qualunque altra radice (C:, gli
/// altri dischi) si rifiuta con `E_INVALIDARG` senza chiamare Windows.
pub fn empty(root: &Path) -> windows::core::Result<()> {
    if !is_test_volume(root) {
        return Err(E_INVALIDARG.into());
    }
    // SAFETY: nessuna finestra proprietaria; percorso valido per la chiamata, radice di un VHDX di prova
    // e mai vuoto (la radice nulla svuoterebbe i Cestini di tutti i volumi).
    unsafe { SHEmptyRecycleBinW(Some(HWND::default()), &HSTRING::from(root.as_os_str()), SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND) }
}

/// Codici candidati per rifiutare un singolo elemento (07 §3 riga 2).
pub fn candidate_codes() -> Vec<(&'static str, HRESULT)> {
    use windows::Win32::UI::Shell::{COPYENGINE_E_USER_CANCELLED, COPYENGINE_S_DONT_PROCESS_CHILDREN, COPYENGINE_S_USER_IGNORED};
    vec![
        ("E_ABORT", E_ABORT),
        ("HRESULT_FROM_WIN32(ERROR_CANCELLED)", HRESULT(0x8007_04C7_u32 as i32)),
        ("COPYENGINE_E_USER_CANCELLED", COPYENGINE_E_USER_CANCELLED),
        ("COPYENGINE_S_USER_IGNORED", COPYENGINE_S_USER_IGNORED),
        ("COPYENGINE_S_DONT_PROCESS_CHILDREN", COPYENGINE_S_DONT_PROCESS_CHILDREN),
    ]
}
