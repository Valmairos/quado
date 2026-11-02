// SPDX-License-Identifier: GPL-3.0-or-later
//! Scansione di Windows: l'unico crate con le API di lettura del file system.
//! Solo lettura: nessun handle in scrittura. La scansione non legge il contenuto dei file;
//! `content` e `zip` lo leggono solo su richiesta dell'utente (lente Download). Qui anche le
//! chiamate di Windows per la finestra dell'app (`window`: le icone dalle risorse dell'eseguibile).

pub mod attrs;
pub mod content;
pub mod helper;
pub mod process;
pub mod scan;
pub mod system;
pub mod volumes;
pub mod win;
pub mod window;
pub mod zip;

pub use scan::{Options, Problem, Progress, RootError, ScanResult, Stats, TopEntry, VolumeFacts, scan};
