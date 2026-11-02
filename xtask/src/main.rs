// SPDX-License-Identifier: GPL-3.0-or-later
//! Strumenti di sviluppo. Uso: `cargo xtask <comando>` (alias in `.cargo/config.toml`).
//!
//! - `scan <radice> [--threads N]`: una scansione con tempi, memoria, bilancio e problemi.
//! - `verifica-dll`: flag del linker e prova della DLL esca.
//! - `lingue [nuova <tag>]`: copertura delle lingue, scheletro di una lingua nuova.
//! - `m2-misure`: ricerca, File grandi e annullamento sulla C: reale.
//! - `m4-misure --cartella DIR [--escludi "a;b"]`: lente Download, costo delle risorse di versione e frame
//!   durante le letture; `m4-misure --genera DIR` crea una cartella di prova.
//! - `m5-misure`: costo della simulazione su C:, in sola lettura.
//! - `importazioni [EXE]`, `installa`: controllo dell'eseguibile e installazione locale.
//! - `rustflags-dist`: la stringa RUSTFLAGS dell'eseguibile pubblicato, con il remap dei percorsi.
//! - `icona [--anteprima DIR]`: le icone di Quado dal segno del marchio, in `crates/quado/icona`.
//! - `sessione-prova`: sessione guidata con campionamento di connessioni e handle.
//! - `helper-prova`: helper elevato su R: con la richiesta UAC.
//! - `confronto-testo`: testo affiancato a ClearType.
//! - `fasi`, `d3d12-costo`, `rm-fasi`: misure della baseline delle prestazioni.
//! - `attributi-fasi`, `lente-installatori`: misure delle letture in parallelo (simulazione e vista Download).

use std::process::ExitCode;

mod cestino_cmd;
mod dist_cmd;
mod dll_cmd;
mod fixture_cmd;
mod helper_cmd;
mod icona_cmd;
mod importazioni_cmd;
mod installa_cmd;
mod lingue_cmd;
mod m2_cmd;
mod m3_cmd;
mod m4_cmd;
mod m5_cmd;
mod measure_cmd;
mod prestazioni_cmd;
mod scan_cmd;
mod sessione_cmd;
mod testo_cmd;
mod vhdx_cmd;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("scan") => scan_cmd::run(&args[1..]),
        Some("verifica-dll") => dll_cmd::run(),
        Some("prestazioni") => measure_cmd::perf(&args[1..]),
        Some("riferimento") => measure_cmd::reference(&args[1..]),
        Some("scan-riga") => measure_cmd::scan_line(&args[1..]),
        Some("freddo-c") => measure_cmd::cold_c(),
        Some("vhdx") => vhdx_cmd::run(&args[1..]),
        Some("cestino") => cestino_cmd::run(),
        Some("cestino-bis") => cestino_cmd::run_followup(),
        Some("m2-misure") => m2_cmd::measures(&args[1..]),
        Some("lingue") => lingue_cmd::run(&args[1..]),
        Some("fixture-regole") => fixture_cmd::run(&args[1..]),
        Some("m3-misure") => m3_cmd::run(),
        Some("m4-misure") => m4_cmd::run(&args[1..]),
        Some("m5-misure") => m5_cmd::run(&args[1..]),
        Some("importazioni") => importazioni_cmd::run(&args[1..]),
        Some("installa") => installa_cmd::run(&args[1..]),
        Some("rustflags-dist") => dist_cmd::run(&args[1..]),
        Some("icona") => icona_cmd::run(&args[1..]),
        Some("sessione-prova") => sessione_cmd::run(&args[1..]),
        Some("helper-prova") => helper_cmd::run(&args[1..]),
        Some("confronto-testo") => testo_cmd::run(&args[1..]),
        Some("fasi") => prestazioni_cmd::phases(&args[1..]),
        Some("fasi-riga") => prestazioni_cmd::phases_line(&args[1..]),
        Some("d3d12-costo") => prestazioni_cmd::d3d12(&args[1..]),
        Some("d3d12-riga") => prestazioni_cmd::d3d12_line(&args[1..]),
        Some("rm-fasi") => prestazioni_cmd::rm_phases(&args[1..]),
        Some("attributi-fasi") => prestazioni_cmd::attr_phases(&args[1..]),
        Some("lente-installatori") => prestazioni_cmd::lens_installers(&args[1..]),
        _ => Err("comandi: scan <radice> | prestazioni [radice] [--runs N] | riferimento [radice] | verifica-dll | vhdx <crea|freddo|elimina|stato> | cestino".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
