// SPDX-License-Identifier: GPL-3.0-or-later
//! Spostamento nel Cestino di un elemento con `IFileOperation`: `FOFX_RECYCLEONDELETE`
//! e `FOF_ALLOWUNDO`, mai `FOF_NOCONFIRMATION` (né `FOF_NO_UI`, che la comprende). Un'operazione per
//! elemento; il sink registra `PostDeleteItem` per sapere se l'elemento è arrivato nel Cestino.
//!
//! La protezione vera dall'eliminazione definitiva è il controllo preventivo della simulazione (capacità,
//! `NukeOnDelete`, unità con Cestino). Qui ci sono tre difese in più:
//! - un elemento per operazione, così un elemento troppo grande non trascina gli altri (prove su VHDX in
//!   `docs/m1/`: in un'operazione con più elementi Windows eliminava per sempre anche quelli piccoli);
//! - in `PreDeleteItem`, se manca `TSF_DELETE_RECYCLE_IF_POSSIBLE`, l'operazione si annulla con
//!   `E_ABORT` (nelle stesse prove, con un solo elemento questo lo salva quando Windows riporta il flag; ma
//!   Windows non lo riporta sempre, quindi non basta);
//! - senza `FOF_NOCONFIRMATION`, se Windows decide di eliminare per sempre chiede all'utente, con
//!   `FOF_WANTNUKEWARNING` anche quando l'operazione è silenziosa. Misurato su R: il 14/10/2026: un file
//!   da 100 MiB in un Cestino da 64 MiB riceve in `PreDeleteItem` il flag "nel Cestino" (0x280), poi
//!   Windows mostra "This file is too big to recycle. Do you want to permanently delete it?"; con "No"
//!   l'esito è `COPYENGINE_E_USER_CANCELLED` e il file resta. Questa è la difesa che ha agito, non il sink.
//!
//! L'esito viene da `PostDeleteItem`, che nelle prove su VHDX ha sempre detto il vero: il nuovo elemento
//! del Cestino c'è solo se l'elemento è arrivato nel Cestino.
//!
//! Percorsi di 260 caratteri o più: la shell li accetta ma non li mette nel Cestino, e chiede subito
//! "Are you sure you want to permanently delete this file?", prima di qualunque evento del sink
//! (misurato su R: con 325 caratteri; 259 va nel Cestino). Si usa il nome breve 8.3 se il volume lo
//! conserva (`shell::path_candidates`, come "Apri in Esplora file"); altrimenti l'elemento non si passa
//! alla shell e l'esito è `Failed(PATH_TOO_LONG)`. Lo stesso vale dentro le cartelle: una cartella con
//! un file interno di 260 caratteri o più non va nel Cestino e Windows chiede "Do you want to permanently
//! delete this folder?" (dopo un `PreDeleteItem` con il flag "nel Cestino"). Qui non si controlla, perché
//! servirebbe rileggere tutto il sottoalbero a ogni spostamento: lo blocca la simulazione, che lo
//! attraversa già, e in ultimo la domanda di Windows.

use std::cell::RefCell;

use windows::Win32::Foundation::{E_ABORT, HWND};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize};
use windows::Win32::UI::Shell::{
    FILEOPERATION_FLAGS, FOF_ALLOWUNDO, FOF_NOERRORUI, FOF_SILENT, FOF_WANTNUKEWARNING, FOFX_RECYCLEONDELETE, FileOperation, IFileOperation,
    IFileOperationProgressSink, IFileOperationProgressSink_Impl, IShellItem, SHCreateItemFromParsingName, SICHINT_CANONICAL, SIGDN_FILESYSPATH,
};
use windows::core::{HRESULT, HSTRING, PCWSTR, Ref, implement};
use windows_core::ComObject;

use crate::execute::{MoveResult, Mover};
use crate::recycle::TSF_DELETE_RECYCLE_IF_POSSIBLE;

/// Flag dell'operazione. Perché ciascuno c'è:
/// - `FOF_ALLOWUNDO`: l'eliminazione va nel Cestino ed è annullabile (con `FOFX_RECYCLEONDELETE`).
/// - `FOFX_RECYCLEONDELETE`: chiede esplicitamente il Cestino per `DeleteItem`.
/// - `FOF_SILENT`: niente finestra di avanzamento; l'avanzamento lo mostra Quado, un elemento alla volta.
/// - `FOF_NOERRORUI`: niente dialoghi d'errore (per esempio "file in uso, Riprova/Ignora"): l'errore
///   torna come HRESULT in `PostDeleteItem` e finisce nel riepilogo. Un dialogo d'errore
///   bloccherebbe l'esecuzione in attesa di una risposta che il riepilogo già dà.
/// - `FOF_WANTNUKEWARNING`: se Windows deciderebbe di eliminare per sempre (elemento troppo grande per il
///   Cestino, nomi interni troppo lunghi), chiede all'utente anche in un'operazione silenziosa (misurato:
///   la domanda compare con `FOF_SILENT` e `FOF_NOERRORUI`).
///
/// Perché altri non ci sono:
/// - `FOF_NOCONFIRMATION` (0x10) e `FOF_NO_UI` (0x614, che la contiene): risponderebbero "Sì a tutto"
///   anche alla domanda "Eliminare definitivamente?". Senza, Windows chiede all'utente. Se
///   l'utente ha acceso "Chiedi conferma prima di eliminare" nelle proprietà del Cestino, Windows chiede
///   anche per lo spostamento: è una scelta sua e si rispetta.
/// - `FOF_NOCONFIRMMKDIR`, `FOF_RENAMEONCOLLISION`: servono a copie e spostamenti, non all'eliminazione.
/// - `FOFX_SHOWELEVATIONPROMPT`, `FOFX_REQUIREELEVATION`: Quado non chiede privilegi per spostare.
pub const FLAGS: u32 = FOF_ALLOWUNDO.0 | FOFX_RECYCLEONDELETE.0 | FOF_SILENT.0 | FOF_NOERRORUI.0 | FOF_WANTNUKEWARNING.0;

/// Bit di "Sì a tutto" (`FOF_NOCONFIRMATION`), mai presente in [`FLAGS`].
const YES_TO_ALL: u32 = 0x10;
const _: () = assert!(FLAGS & YES_TO_ALL == 0);

/// `HRESULT_FROM_WIN32(ERROR_FILENAME_EXCED_RANGE)`: percorso di 260 caratteri o più senza nome breve 8.3.
/// L'elemento non si passa alla shell e resta dov'è.
pub const PATH_TOO_LONG: HRESULT = HRESULT(0x8007_00CE_u32 as i32);

/// Un evento del sink, registrato per l'esito e per le prove.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// `PreDeleteItem`: flag `TSF_*`; `target` se è l'elemento chiesto (non un suo figlio); `aborted` se il
    /// sink ha risposto `E_ABORT` perché mancava `TSF_DELETE_RECYCLE_IF_POSSIBLE`.
    Pre { path: String, flags: u32, target: bool, aborted: bool },
    /// `PostDeleteItem`: flag, HRESULT dell'elemento, nuovo elemento del Cestino non nullo.
    Post { path: String, flags: u32, hr: i32, new_item: bool, target: bool },
}

/// Tutto quello che è successo in uno spostamento: l'esito e i fatti da cui viene.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trace {
    pub result: MoveResult,
    /// Percorso passato alla shell (quello chiesto, o il nome breve 8.3).
    pub parsed: Option<String>,
    /// HRESULT di `PerformOperations` (0 se riuscita o non chiamata).
    pub perform: i32,
    /// `GetAnyOperationsAborted`.
    pub aborted: bool,
    pub events: Vec<Event>,
}

/// Lo spostamento vero. `owner` è la finestra di Quado (HWND), proprietaria dei dialoghi di Windows.
pub struct ShellMover {
    owner: isize,
}

impl ShellMover {
    pub fn new(owner: Option<isize>) -> Self {
        Self { owner: owner.unwrap_or(0) }
    }

    /// Come [`Mover::recycle`], con gli eventi del sink (per le prove e per la diagnosi).
    pub fn recycle_traced(&self, path: &str) -> Trace {
        let _com = match Com::init() {
            Ok(c) => c,
            Err(hr) => return failed(hr),
        };
        // Solo percorsi sotto i 260 caratteri: con uno più lungo la shell accetta l'elemento ma non lo
        // mette nel Cestino, e prima ancora di `PreDeleteItem` chiede all'utente se eliminarlo per sempre
        // (misurato su R: il 14/10/2026). Così quella domanda non compare mai.
        let candidates: Vec<String> = crate::shell::path_candidates(path).into_iter().filter(|c| c.encode_utf16().count() < 260).collect();
        if candidates.is_empty() {
            return failed(PATH_TOO_LONG);
        }
        let mut last = HRESULT(0);
        for candidate in candidates {
            // SAFETY: COM è inizializzato su questo thread; la stringa vive per la chiamata; nessun contesto.
            let parsed: windows::core::Result<IShellItem> = unsafe { SHCreateItemFromParsingName(&HSTRING::from(candidate.as_str()), None) };
            match parsed {
                Ok(item) => {
                    let mut t = self.perform(&item);
                    t.parsed = Some(candidate);
                    return t;
                }
                Err(e) => last = e.code(),
            }
        }
        failed(last)
    }

    /// Un'operazione con il solo `item`.
    fn perform(&self, item: &IShellItem) -> Trace {
        // SAFETY: COM è inizializzato in apartment a thread singolo da `recycle_traced` e resta tale per
        // tutta la funzione; l'operazione, il sink e l'elemento si usano solo su questo thread.
        unsafe {
            let op: IFileOperation = match CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER) {
                Ok(op) => op,
                Err(e) => return failed(e.code()),
            };
            if let Err(e) = op.SetOperationFlags(FILEOPERATION_FLAGS(FLAGS)) {
                return failed(e.code());
            }
            if self.owner != 0
                && let Err(e) = op.SetOwnerWindow(HWND(self.owner as *mut _))
            {
                return failed(e.code());
            }
            let sink_obj = ComObject::new(Sink { target: item.clone(), events: RefCell::new(Vec::new()) });
            let sink: IFileOperationProgressSink = sink_obj.to_interface();
            let cookie = match op.Advise(&sink) {
                Ok(c) => c,
                Err(e) => return failed(e.code()),
            };
            if let Err(e) = op.DeleteItem(item, None) {
                let _ = op.Unadvise(cookie);
                return failed(e.code());
            }
            let perform = match op.PerformOperations() {
                Ok(()) => HRESULT(0),
                Err(e) => e.code(),
            };
            let aborted = op.GetAnyOperationsAborted().map(|b| b.as_bool()).unwrap_or(false);
            let _ = op.Unadvise(cookie);
            let events = sink_obj.events.take();
            Trace { result: outcome(perform, aborted, &events), parsed: None, perform: perform.0, aborted, events }
        }
    }
}

/// Esito senza operazione: la shell non ha ricevuto l'elemento, o l'operazione non è partita.
fn failed(hr: HRESULT) -> Trace {
    Trace { result: MoveResult::Failed(hr.0), parsed: None, perform: 0, aborted: false, events: Vec::new() }
}

impl Mover for ShellMover {
    fn recycle(&self, path: &str) -> MoveResult {
        self.recycle_traced(path).result
    }
}

/// Esito dagli eventi dell'elemento chiesto (in mancanza di un evento riconosciuto come suo, l'ultimo
/// `PostDeleteItem`, che per una cartella arriva dopo quelli dei figli):
/// - nuovo elemento del Cestino e HRESULT di successo: `Recycled`;
/// - HRESULT di successo senza nuovo elemento: Windows ha finito senza mettere l'elemento nel Cestino
///   (visto nelle prove su VHDX: eliminato per sempre), `NotRecycled`;
/// - HRESULT di errore senza nuovo elemento: Windows non lo ha eliminato (per esempio in uso, o annullato
///   da `PreDeleteItem`), `Failed`; per una cartella una parte può essere già stata spostata, quindi chi
///   esegue rilegge l'elemento;
/// - HRESULT di errore con un nuovo elemento del Cestino: stato incerto, `NotRecycled` (il piano si ferma);
/// - nessun evento `PostDeleteItem`: `Failed` con l'HRESULT di `PerformOperations` se è fallita o è stata
///   annullata (`E_ABORT` se annullata con successo apparente), altrimenti `NotRecycled(0)`: Windows dice
///   di aver finito ma non dice dov'è l'elemento, e il piano si ferma.
fn outcome(perform: HRESULT, aborted: bool, events: &[Event]) -> MoveResult {
    let posts = events.iter().filter_map(|e| match e {
        Event::Post { hr, new_item, target, .. } => Some((*hr, *new_item, *target)),
        Event::Pre { .. } => None,
    });
    let post = posts.clone().rev().find(|p| p.2).or_else(|| posts.clone().next_back());
    match post {
        Some((hr, true, _)) if hr >= 0 => MoveResult::Recycled,
        Some((hr, false, _)) if hr >= 0 => MoveResult::NotRecycled(hr),
        Some((hr, false, _)) => MoveResult::Failed(hr),
        Some((hr, true, _)) => MoveResult::NotRecycled(hr),
        None if perform.is_err() => MoveResult::Failed(perform.0),
        None if aborted => MoveResult::Failed(E_ABORT.0),
        None => MoveResult::NotRecycled(perform.0),
    }
}

/// COM in apartment a thread singolo sul thread corrente, per la durata del valore; `IFileOperation`
/// funziona solo lì. Bilancia solo un'inizializzazione riuscita.
struct Com;

impl Com {
    fn init() -> Result<Self, HRESULT> {
        // SAFETY: inizializzazione COM del thread corrente. S_OK e S_FALSE (già inizializzato a thread
        // singolo) vanno bilanciati nel drop; con RPC_E_CHANGED_MODE il thread è multithread e
        // `IFileOperation` non si può usare: non si bilancia niente e l'elemento non si sposta.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr.is_ok() { Ok(Com) } else { Err(hr) }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: bilancia la CoInitializeEx riuscita di `init` sullo stesso thread.
        unsafe { CoUninitialize() };
    }
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
    /// L'elemento chiesto, per riconoscere i suoi eventi da quelli dei figli.
    target: IShellItem,
    events: RefCell<Vec<Event>>,
}

impl Sink {
    fn is_target(&self, item: &IShellItem) -> bool {
        // SAFETY: entrambi gli elementi sono validi e usati sul thread dell'operazione.
        unsafe { self.target.Compare(item, SICHINT_CANONICAL.0 as u32) }.is_ok_and(|order| order == 0)
    }
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
    /// Senza `TSF_DELETE_RECYCLE_IF_POSSIBLE` Windows eliminerebbe per sempre: `E_ABORT` ferma
    /// l'operazione (provato su VHDX: con un solo elemento, l'elemento resta al suo posto).
    fn PreDeleteItem(&self, dwflags: u32, psiitem: Ref<IShellItem>) -> windows::core::Result<()> {
        let item = psiitem.as_ref();
        let permanent = dwflags & TSF_DELETE_RECYCLE_IF_POSSIBLE == 0;
        self.events.borrow_mut().push(Event::Pre {
            path: item.map(item_path).unwrap_or_default(),
            flags: dwflags,
            target: item.is_some_and(|i| self.is_target(i)),
            aborted: permanent,
        });
        if permanent { Err(E_ABORT.into()) } else { Ok(()) }
    }
    fn PostDeleteItem(&self, dwflags: u32, psiitem: Ref<IShellItem>, hrdelete: HRESULT, psinewlycreated: Ref<IShellItem>) -> windows::core::Result<()> {
        let item = psiitem.as_ref();
        self.events.borrow_mut().push(Event::Post {
            path: item.map(item_path).unwrap_or_default(),
            flags: dwflags,
            hr: hrdelete.0,
            new_item: psinewlycreated.as_ref().is_some(),
            target: item.is_some_and(|i| self.is_target(i)),
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn post(hr: i32, new_item: bool, target: bool) -> Event {
        Event::Post { path: String::new(), flags: 0x282, hr, new_item, target }
    }

    #[test]
    fn flags_never_answer_yes_to_all() {
        assert_eq!(FLAGS & YES_TO_ALL, 0);
        // `FOF_NO_UI` contiene "Sì a tutto": FLAGS non la contiene.
        assert_ne!(FLAGS & 0x614, 0x614);
        assert_eq!(FLAGS, 0x0008_4444);
    }

    #[test]
    fn outcome_from_events() {
        const DONT_PROCESS_CHILDREN: i32 = 0x0027_0008;
        const SHARING: i32 = 0x8007_0020_u32 as i32;
        let ok = HRESULT(0);
        assert_eq!(outcome(ok, false, &[post(DONT_PROCESS_CHILDREN, true, true)]), MoveResult::Recycled);
        // Prove su VHDX: eliminato per sempre con un esito di successo e nessun elemento nel Cestino.
        assert_eq!(outcome(ok, false, &[post(DONT_PROCESS_CHILDREN, false, true)]), MoveResult::NotRecycled(DONT_PROCESS_CHILDREN));
        // Prove su VHDX: annullato da PreDeleteItem, PostDeleteItem con E_ABORT e PerformOperations fallita.
        assert_eq!(outcome(E_ABORT, false, &[post(E_ABORT.0, false, true)]), MoveResult::Failed(E_ABORT.0));
        assert_eq!(outcome(ok, true, &[post(SHARING, false, true)]), MoveResult::Failed(SHARING));
        assert_eq!(outcome(ok, false, &[post(SHARING, true, true)]), MoveResult::NotRecycled(SHARING));
        // L'evento dell'elemento chiesto conta più di quelli dei figli, anche se arriva prima.
        assert_eq!(outcome(ok, false, &[post(0, true, true), post(0, false, false)]), MoveResult::Recycled);
        // Senza un evento riconosciuto vale l'ultimo.
        assert_eq!(outcome(ok, false, &[post(0, true, false), post(0, false, false)]), MoveResult::NotRecycled(0));
        // Nessun evento.
        assert_eq!(outcome(E_ABORT, true, &[]), MoveResult::Failed(E_ABORT.0));
        assert_eq!(outcome(ok, true, &[]), MoveResult::Failed(E_ABORT.0));
        assert_eq!(outcome(ok, false, &[]), MoveResult::NotRecycled(0));
    }
}
