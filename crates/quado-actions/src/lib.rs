// SPDX-License-Identifier: GPL-3.0-or-later
//! Interventi di Quado (05 D7, D10): l'unico crate con API di scrittura e di avvio di procedure.
//! Da M2 le azioni sempre disponibili dell'ispettore (F-IN-5), che parlano con la shell di Windows; da M4
//! Restart Manager e il lanciatore della disinstallazione; da M5 simulazione, piano confermato,
//! spostamento nel Cestino, procedure, misure e registro degli interventi. Dal cambio di nome, la copia e la
//! cancellazione dei dati di Rilievo (`legacy`).

mod estimate;
pub mod elevate;
pub mod execute;
pub mod fake;
pub mod journal;
pub mod launch;
pub mod legacy;
pub mod measure;
pub mod mover;
pub mod plan;
pub mod probe;
pub mod procedures;
pub mod recycle;
pub mod restart;
pub mod shell;
pub mod simulate;
pub mod sync;
pub mod win_probe;
