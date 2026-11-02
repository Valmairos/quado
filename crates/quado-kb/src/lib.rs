// SPDX-License-Identifier: GPL-3.0-or-later
//! Base delle regole di Quado: regole TOML incorporate, motore di classificazione, estrattori in sola
//! lettura. Niente AI durante l'esecuzione: ogni spiegazione viene da una regola con una fonte, un
//! estratto e una data di verifica. Il crate restituisce codici e valori; i testi stanno in
//! `regole/<lingua>.ftl`.

pub mod engine;
pub mod evidence;
pub mod extract;
pub mod installers;
#[cfg(windows)]
#[allow(unsafe_code)]
pub mod installers_win;
pub mod lens;
pub mod programs;
pub mod protection;
pub mod references;
pub mod reg;
pub mod rules;
pub mod sync;
pub mod vdf;
#[cfg(windows)]
#[allow(unsafe_code)]
pub mod win;

pub use engine::{ByManager, Classification, Match, classify};
pub use evidence::{Evidence, KnownFolders, RecycleQuery};
pub use rules::{Kb, Rule};

/// Testi delle regole incorporati: (lingua, testo Fluent).
pub const TEXTS: [(&str, &str); 2] = [("it", include_str!("../regole/it.ftl")), ("en", include_str!("../regole/en.ftl"))];
