// SPDX-License-Identifier: GPL-3.0-or-later
//! Helper elevato su richiesta (05 D3, 03 F-SC-3, 06 §1.5 e §11).
//!
//! Lo stesso eseguibile, avviato con `--helper <pipe> <pid>` e il verbo `runas` solo quando l'utente lo
//! chiede dalla card dei conti, abilita `SeBackupPrivilege` ed elenca con lo stesso scanner le sole
//! cartelle che la scansione non elevata ha trovato negate. I profili e i Cestini degli altri account
//! arrivano solo come totali, senza nomi. Raccoglie anche lo spazio delle copie shadow. Termina a fine
//! lavoro.
//!
//! Pipe: la crea l'interfaccia ([`pipe::Server`]) con un nome casuale, DACL del solo utente e
//! `PIPE_REJECT_REMOTE_CLIENTS`; ciascuna parte verifica il processo dall'altra parte. L'helper legge
//! un solo messaggio, la [`proto::Request`], la valida e poi scrive soltanto ([`proto::Reply`]).
//!
//! L'avvio con UAC sta in `quado-actions` (`elevate`), l'unico crate che crea processi.
//!
//! L'archiviazione riservata non passa dall'helper: si legge senza privilegi con
//! `GetDiskSpaceInformationW` (M6, `win::reserved_space`).

pub mod collect;
pub mod pipe;
pub mod proto;
pub mod serve;
