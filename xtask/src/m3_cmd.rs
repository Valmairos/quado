// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask m3-misure`: classificazione di C: con le regole (M3). Tempo degli estrattori e del
//! motore, bilancio per gestore, card interventi, elementi riconosciuti per regola. Scrive
//! `target/misure/m3-misure-C.md`. Solo letture.

use std::fmt::Write as _;
use std::time::Instant;

use quado_kb::engine::interventions;
use quado_kb::{Kb, classify};
use quado_model::record::{Date, Estimate};
use quado_scan_win::{Options, Progress, scan, system};

fn gib(b: u64) -> String {
    format!("{:.2}", b as f64 / f64::from(1u32 << 30))
}

pub fn run() -> Result<(), String> {
    let kb = Kb::embedded().map_err(|e| format!("regole: {e:?}"))?;
    let t0 = Instant::now();
    let r = scan("C:\\", Options::default(), &Progress::default()).map_err(|e| format!("scansione: {e:?}"))?;
    let scan_s = t0.elapsed().as_secs_f64();
    let sid = system::user_sid();
    let mut ev_ms = Vec::new();
    let mut ev = None;
    for _ in 0..3 {
        let t = Instant::now();
        ev = Some(quado_kb::win::gather("C:\\", sid.clone()));
        ev_ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let ev = ev.ok_or("prove")?;
    let lt = system::local_time(system::now_filetime()).ok_or("data")?;
    let today = Date::new(lt.year, lt.month as u8, lt.day as u8);
    let mut cls_ms = Vec::new();
    let mut cls = None;
    for _ in 0..5 {
        let t = Instant::now();
        cls = Some(classify(&kb, &r.tree, &ev, today));
        cls_ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let cls = cls.ok_or("classificazione")?;
    let peak = quado_scan_win::win::process_metrics().peak_working_set;
    cls_ms.sort_by(f64::total_cmp);
    ev_ms.sort_by(f64::total_cmp);
    let t = &r.tree;
    let mut out = String::new();
    let _ = writeln!(out, "# M3 · Misure su C: ({:02}/{:02}/{})\n", lt.day, lt.month, lt.year);
    let _ = writeln!(out, "Generato da `cargo xtask m3-misure`. Unità: GiB (1024³ byte).\n");
    let _ = writeln!(out, "| Misura | Valore |\n|---|---|");
    let _ = writeln!(out, "| Scansione (cache calda) | {scan_s:.1} s, {} voci |", t.len());
    let _ = writeln!(out, "| Estrattori (3 volte) | {} ms |", ev_ms.iter().map(|x| format!("{x:.0}")).collect::<Vec<_>>().join(", "));
    let _ = writeln!(out, "| Motore (5 volte) | {} ms |", cls_ms.iter().map(|x| format!("{x:.0}")).collect::<Vec<_>>().join(", "));
    let _ = writeln!(out, "| Corrispondenze | {} |", cls.matches.len());
    let _ = writeln!(out, "| Picco di memoria del processo (scansione, prove, classificazione) | {} MB |", peak >> 20);
    let b = cls.by_manager;
    let _ = writeln!(out, "\n## Bilancio per gestore (allocato letto)\n\n| Gruppo | GiB |\n|---|---|");
    for (name, v) in [("Windows", b.windows), ("Programmi e dati delle app", b.programs), ("Tuoi file", b.you), ("Non classificato o ipotesi", b.unclassified)] {
        let _ = writeln!(out, "| {name} | {} |", gib(v));
    }
    let _ = writeln!(out, "| Totale | {} |", gib(t.alloc(quado_model::Tree::ROOT)));
    let _ = writeln!(out, "\n## Elementi per regola\n\n| Regola | Corrispondenze | Elementi | GiB | Stato del primo |\n|---|---|---|---|---|");
    for (i, rule) in kb.rules.iter().enumerate() {
        let ms: Vec<_> = cls.matches.iter().filter(|m| m.rule == i).collect();
        if ms.is_empty() {
            let _ = writeln!(out, "| {} | 0 | 0 | 0 | - |", rule.id);
            continue;
        }
        let nodes: usize = ms.iter().map(|m| m.nodes.len()).sum();
        let bytes: u64 = ms.iter().flat_map(|m| m.nodes.iter()).map(|&n| t.alloc(n)).sum();
        let _ = writeln!(out, "| {} | {} | {nodes} | {} | {} |", rule.id, ms.len(), gib(bytes), ms[0].record.state.code());
    }
    let _ = writeln!(out, "\n## Card interventi\n\n| Regola | Stato | Elementi | GiB | Stima |\n|---|---|---|---|---|");
    for i in interventions(&kb, &cls, t) {
        let est = match i.estimate {
            Estimate::Exact(x) => format!("esatta, {} GiB", gib(x)),
            Estimate::Range { min, max } => format!("{}-{} GiB", gib(min), gib(max)),
            Estimate::NotEstimable(n) => format!("non stimabile ({n:?})"),
        };
        let _ = writeln!(out, "| {} | {} | {} | {} | {est} |", kb.rules[i.rule].id, i.state, i.matches.len(), gib(i.bytes));
    }
    // Annullamento fino al risultato pronto per l'interfaccia: scansione parziale, indici delle viste,
    // prove e classificazione (F-SC-5, obiettivo 500 ms).
    let _ = writeln!(out, "\n## Annullamento fino al risultato pronto\n\n| Prova | Dopo | Dalla richiesta al risultato |\n|---|---|---|");
    let kb = std::sync::Arc::new(kb);
    for (k, secs) in [1u64, 2, 4, 6, 8].into_iter().enumerate() {
        let v = quado_ui::app::data::VolumeInfo {
            root: "C:\\".into(),
            label: String::new(),
            file_system: "NTFS".into(),
            kind: quado_scan_win::volumes::DriveKind::Fixed,
            support: quado_scan_win::volumes::Support::Full,
            is_system: true,
        };
        let mut job = quado_ui::app::data::ScanJob::start(v, sid.clone(), std::sync::Arc::clone(&kb));
        std::thread::sleep(std::time::Duration::from_secs(secs));
        let t = Instant::now();
        job.cancel();
        let res = loop {
            if let Some(r) = job.poll() {
                break r;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let done = res.as_ref().map(|l| l.cancelled).unwrap_or(false);
        let _ = writeln!(out, "| {} | {secs} s | {ms:.0} ms{} |", k + 1, if done { "" } else { " (finita prima)" });
        eprintln!("annullamento {}: {ms:.0} ms", k + 1);
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("target").join("misure");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("m3-misure-C.md"), &out).map_err(|e| e.to_string())?;
    println!("{out}");
    Ok(())
}
