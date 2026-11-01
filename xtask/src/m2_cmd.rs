// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask m2-misure`: misure di M2 sulla C: reale, senza interfaccia (06 §9, 07 §1.3 criterio 2).
//!
//! - figli ordinati e top list ("Dove finisce lo spazio") sui dati veri;
//! - ricerca su tutti i nomi (obiettivo ≤ 300 ms su 3 milioni di nomi), 5 esecuzioni per testo;
//! - File grandi (i 10.000 più grandi);
//! - annullamento della scansione (obiettivo ≤ 500 ms), 5 prove a istanti diversi.
//!
//! Il risultato va in `target/misure/m2-misure-C.md`.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use quado_model::view::{self, Filter, Needle, Ordered};
use quado_model::{Family, Tree};
use quado_scan_win::{Options, Progress, scan};

use crate::scan_cmd::gib;

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

pub fn measures(args: &[String]) -> Result<(), String> {
    let root = args.first().cloned().unwrap_or_else(|| "C:\\".to_owned());
    let mut out = String::new();
    let _ = writeln!(out, "# M2 · Misure senza interfaccia su {root}\n");
    let _ = writeln!(out, "Generato da `cargo xtask m2-misure`. Cache calda.\n");
    let _ = writeln!(out, "| Condizione | Valore |\n|---|---|");
    for (k, v) in crate::measure_cmd::environment() {
        let _ = writeln!(out, "| {k} | {v} |");
    }
    out.push('\n');

    eprintln!("scansione di {root}…");
    let t0 = Instant::now();
    let r = scan(&root, Options::default(), &Progress::default()).map_err(|e| format!("scansione: errore {}", e.0))?;
    let scan_s = t0.elapsed().as_secs_f64();
    let tree = r.tree;
    let names = tree.len();
    let t1 = Instant::now();
    let ord = Ordered::build(&tree);
    let ord_ms = t1.elapsed().as_secs_f64() * 1000.0;
    let t2 = Instant::now();
    let top = view::top_elements(&tree, &ord, Tree::ROOT, 10);
    let top_ms = t2.elapsed().as_secs_f64() * 1000.0;
    let _ = writeln!(out, "| Grandezza | Valore |\n|---|---|");
    let _ = writeln!(out, "| Nodi (nomi) | {names} |");
    let _ = writeln!(out, "| Scansione | {scan_s:.1} s |");
    let _ = writeln!(out, "| Figli ordinati (`Ordered::build`) | {ord_ms:.0} ms, {:.1} MB |", ord.heap_bytes() as f64 / 1e6);
    let _ = writeln!(out, "| Top list | {top_ms:.1} ms |\n");

    let _ = writeln!(out, "## Dove finisce lo spazio (algoritmo di M2, senza regole)\n");
    let _ = writeln!(out, "| # | Elemento | Dimensioni su disco |\n|---|---|---|");
    for (k, &n) in top.iter().enumerate() {
        let _ = writeln!(out, "| {} | `{}` | {} |", k + 1, tree.path_lossy(n), gib(tree.alloc(n)));
    }

    let _ = writeln!(out, "\n## Ricerca (obiettivo ≤ 300 ms, 5 esecuzioni)\n");
    let _ = writeln!(out, "| Testo | Filtro | Risultati | Mediana | Minimo-massimo |\n|---|---|---|---|---|");
    let big = Filter { min_alloc: Some(100 << 20), ..Default::default() };
    let video = Filter { family: Some(Family::Video), ..Default::default() };
    let cases: [(&str, &Filter, &str); 7] = [
        ("a", &Filter::default(), "nessuno"),
        ("node_modules", &Filter::default(), "nessuno"),
        ("capcut", &Filter::default(), "nessuno"),
        (".dll", &Filter::default(), "nessuno"),
        ("zzzqqq", &Filter::default(), "nessuno"),
        ("", &big, "≥ 100 MB"),
        ("e", &video, "Video"),
    ];
    let mut worst: f64 = 0.0;
    for (text, filter, fname) in cases {
        let mut times = Vec::new();
        let mut hits = 0;
        for _ in 0..5 {
            let t = Instant::now();
            hits = view::search(&tree, Tree::ROOT, &Needle::new(text), filter, None).len();
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let (mn, mx) = (times.iter().copied().fold(f64::INFINITY, f64::min), times.iter().copied().fold(0.0, f64::max));
        let med = median(&mut times);
        worst = worst.max(med);
        let _ = writeln!(out, "| `{text}` | {fname} | {hits} | {med:.0} ms | {mn:.0}-{mx:.0} ms |");
    }
    let _ = writeln!(out, "\nMediana peggiore: {worst:.0} ms. Criterio ≤ 300 ms: {}.", if worst <= 300.0 { "superato" } else { "**non superato**" });

    let mut times = Vec::new();
    let mut files = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        files = view::largest_files(&tree, &Filter::default(), 10_000, None);
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let _ = writeln!(out, "\n## File grandi\n\nI 10.000 file più grandi: mediana {:.0} ms su 5. Il primo: `{}`, {}.", median(&mut times), files.first().map_or(String::new(), |&n| tree.path_lossy(n)), files.first().map_or(String::new(), |&n| gib(tree.alloc(n))));
    drop(ord);
    drop(tree);

    let _ = writeln!(out, "\n## Annullamento della scansione (obiettivo ≤ 500 ms)\n");
    let _ = writeln!(out, "Tempo dalla richiesta di annullamento al ritorno di `scan`, con il risultato parziale.\n");
    let _ = writeln!(out, "| Prova | Annullata dopo | File letti | Tempo di arresto |\n|---|---|---|---|");
    let mut stops = Vec::new();
    for (k, after) in [1.0f64, 2.0, 4.0, 6.0, 8.0].into_iter().enumerate() {
        let p = Arc::new(Progress::default());
        let p2 = Arc::clone(&p);
        let root2 = root.clone();
        let h = std::thread::spawn(move || scan(&root2, Options::default(), &p2).map(|r| (r.cancelled, r.stats.finish)));
        std::thread::sleep(Duration::from_secs_f64(after));
        let read = p.files.load(std::sync::atomic::Ordering::Relaxed);
        let t = Instant::now();
        p.cancel();
        let res = h.join().map_err(|_| "thread della scansione".to_owned())?;
        let stop_ms = t.elapsed().as_secs_f64() * 1000.0;
        let (cancelled, finish) = res.unwrap_or((false, Duration::ZERO));
        let finish_ms = finish.as_secs_f64() * 1000.0;
        stops.push(stop_ms);
        let _ = writeln!(out, "| {} | {after:.0} s | {read} | {stop_ms:.0} ms (lettura ferma dopo {:.0} ms, poi {finish_ms:.0} ms per hard link e totali){} |", k + 1, stop_ms - finish_ms, if cancelled { "" } else { " (finita prima)" });
        eprintln!("annullamento {}: {stop_ms:.0} ms", k + 1);
    }
    let worst_stop = stops.iter().copied().fold(0.0, f64::max);
    let _ = writeln!(out, "\nMassimo {worst_stop:.0} ms, mediana {:.0} ms. Criterio ≤ 500 ms: {}.", median(&mut stops), if worst_stop <= 500.0 { "superato" } else { "**non superato**" });

    std::fs::create_dir_all("target/misure").map_err(|e| e.to_string())?;
    std::fs::write("target/misure/m2-misure-C.md", &out).map_err(|e| e.to_string())?;
    println!("{out}");
    Ok(())
}
