// SPDX-License-Identifier: GPL-3.0-or-later
//! Interventi di Quado: l'unico crate con API di scrittura e di avvio di procedure.
//! Contiene le azioni sempre disponibili dell'ispettore, che parlano con la shell di Windows; Restart
//! Manager e il lanciatore della disinstallazione; simulazione, piano confermato, spostamento nel Cestino,
//! procedure, misure e registro degli interventi; la copia e la cancellazione dei dati di Rilievo, il nome
//! precedente di Quado (`legacy`).

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
