// SPDX-License-Identifier: GPL-3.0-or-later
//! Esecuzione di un piano confermato (03 F-AC-3, F-AC-5, F-AC-8; 05 D7 e "Aggiornamento dopo M1"):
//! un elemento alla volta, ricontrollato subito prima (ID, dimensione, data di modifica; per le cartelle
//! anche Restart Manager), un'operazione di Windows per elemento, verifica che sia arrivato nel Cestino,
//! arresto del piano al primo elemento sparito senza arrivare nel Cestino.
//!
//! Al mover arrivano solo i percorsi degli elementi del piano, uno per chiamata e nell'ordine del piano.
//! Se Windows non sposta un elemento, una nuova lettura degli attributi dice dove sta:
//! - `MoveResult::Failed`: ancora al suo posto → fallito, gli altri proseguono (06 §5.4, file aperto
//!   dopo il controllo); sparito → non nel Cestino, il piano si ferma.
//! - `MoveResult::NotRecycled`: Windows ha finito senza un elemento nel Cestino (per esempio l'utente ha
//!   detto sì a "eliminare per sempre?"). Il piano si ferma sempre: sparito → non nel Cestino; ancora al
//!   suo posto → fallito, ma lo stato è inatteso.
//! - Se quella lettura non riesce, non si sa: l'elemento è fallito e il piano si ferma per prudenza.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use quado_model::intervention::{ItemOutcome, SkipReason};

use crate::plan::{PianoConfermato, PlanItem, SelectionEstimate};
use crate::probe::Probe;
use crate::simulate::{is_missing, read_item};

/// Esito di un singolo spostamento chiesto a Windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MoveResult {
    /// `PostDeleteItem` ha restituito il nuovo elemento del Cestino.
    Recycled,
    /// Windows ha finito l'operazione senza un elemento nel Cestino (HRESULT dell'elemento).
    NotRecycled(i32),
    /// L'operazione non è riuscita (HRESULT).
    Failed(i32),
}

/// Chi sposta un elemento nel Cestino: `mover::ShellMover` con `IFileOperation`, o un sostituto nei test.
pub trait Mover {
    fn recycle(&self, path: &str) -> MoveResult;
}

/// Avanzamento dell'esecuzione; `cancel` ferma prima dell'elemento successivo.
#[derive(Debug, Default)]
pub struct ExecProgress {
    /// Elementi conclusi.
    pub done: AtomicU64,
    pub cancel: AtomicBool,
}

/// Un elemento nel riepilogo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemResult {
    pub path: String,
    pub directory: bool,
    /// Byte allocati stimati dell'elemento.
    pub alloc: u64,
    pub outcome: ItemOutcome,
}

/// Riepilogo dell'esecuzione (F-AC-3): riusciti, saltati e falliti con il motivo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Riepilogo {
    /// Radice del volume.
    pub volume: String,
    /// Regola o funzione del record.
    pub origin: String,
    pub estimate: SelectionEstimate,
    pub items: Vec<ItemResult>,
    /// `FILETIME` di inizio e fine.
    pub started: i64,
    pub finished: i64,
    /// Il piano si è fermato prima della fine (elemento non arrivato nel Cestino, o annullamento).
    pub stopped: bool,
}

/// Esito del ricontrollo di un elemento.
enum Recheck {
    /// Uguale alla simulazione: si chiede a Windows di spostarlo.
    Same,
    Skip(SkipReason),
    /// Annullato durante la visita della cartella: l'elemento non è raggiunto.
    Cancelled,
}

/// Ricontrollo subito prima dello spostamento (RS6): identità, dimensione e data come in simulazione;
/// per le cartelle una nuova visita (numero di file e logica totale) e Restart Manager con lo stesso
/// campione.
fn recheck(item: &PlanItem, probe: &dyn Probe, progress: &ExecProgress) -> Recheck {
    let a = match probe.attributes(&item.path) {
        Ok(a) => a,
        Err(e) if is_missing(e) => return Recheck::Skip(SkipReason::Missing),
        Err(e) => return Recheck::Skip(SkipReason::Unreadable(e)),
    };
    let s = &item.snapshot;
    let changed = a.directory != item.directory
        || (s.file_id != 0 && a.file_id != s.file_id)
        || (s.volume_serial != 0 && a.volume_serial != s.volume_serial)
        || a.modified != s.modified;
    if changed {
        return Recheck::Skip(SkipReason::Changed);
    }
    if !(a.directory && !a.link) {
        return if a.logical == s.logical { Recheck::Same } else { Recheck::Skip(SkipReason::Changed) };
    }
    let step = || !progress.cancel.load(Ordering::Relaxed);
    let Some(v) = read_item(probe, &item.path, &a, &step) else {
        return Recheck::Cancelled;
    };
    // Un percorso diventato lungo (per esempio una sottocartella rinominata) non cambia numero di file
    // e logica, ma la shell eliminerebbe per sempre: è un cambiamento.
    if v.files != s.files || v.logical != s.logical || v.long {
        return Recheck::Skip(SkipReason::Changed);
    }
    match probe.processes_using(&item.sample) {
        Ok(p) if p.is_empty() => Recheck::Same,
        Ok(_) => Recheck::Skip(SkipReason::InUse),
        Err(e) => Recheck::Skip(SkipReason::Unreadable(e.code())),
    }
}

/// Sposta un elemento già ricontrollato; il secondo valore dice se il piano si ferma.
fn move_one(path: &str, probe: &dyn Probe, mover: &dyn Mover) -> (ItemOutcome, bool) {
    let (hr, finished) = match mover.recycle(path) {
        MoveResult::Recycled => return (ItemOutcome::Recycled, false),
        MoveResult::Failed(hr) => (hr, false),
        MoveResult::NotRecycled(hr) => (hr, true),
    };
    match probe.attributes(path) {
        Ok(_) => (ItemOutcome::Failed(hr), finished),
        Err(e) if is_missing(e) => (ItemOutcome::NotInRecycleBin(hr), true),
        Err(_) => (ItemOutcome::Failed(hr), true),
    }
}

/// Esegue il piano e lo consuma: un piano vale una volta.
pub fn esegui(piano: PianoConfermato, probe: &dyn Probe, mover: &dyn Mover, progress: &ExecProgress) -> Riepilogo {
    let PianoConfermato { items, volume, estimate, origin } = piano;
    let started = probe.now();
    let mut results = Vec::with_capacity(items.len());
    let mut stopped = false;
    for item in items {
        if !stopped && progress.cancel.load(Ordering::Relaxed) {
            stopped = true;
        }
        let outcome = if stopped {
            ItemOutcome::NotReached
        } else {
            match recheck(&item, probe, progress) {
                Recheck::Cancelled => {
                    stopped = true;
                    ItemOutcome::NotReached
                }
                Recheck::Skip(r) => ItemOutcome::Skipped(r),
                Recheck::Same => {
                    let (o, stop) = move_one(&item.path, probe, mover);
                    stopped = stop;
                    o
                }
            }
        };
        if outcome != ItemOutcome::NotReached {
            progress.done.fetch_add(1, Ordering::Relaxed);
        }
        results.push(ItemResult { path: item.path, directory: item.directory, alloc: item.alloc, outcome });
    }
    let finished = probe.now();
    Riepilogo { volume, origin, estimate, items: results, started, finished, stopped }
}
