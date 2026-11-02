// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask scan <radice>`: una scansione con le misure delle prestazioni.

use quado_model::{Flags, Tree};
use quado_scan_win::{Options, Progress, scan, win};

pub fn gib(b: u64) -> String {
    format!("{:.2} GiB", b as f64 / (1u64 << 30) as f64)
}

pub fn gib_i(b: i64) -> String {
    format!("{}{:.2} GiB", if b < 0 { "-" } else { "" }, b.unsigned_abs() as f64 / (1u64 << 30) as f64)
}

pub fn run(args: &[String]) -> Result<(), String> {
    let root = args.first().ok_or("manca la radice, per esempio C:\\")?;
    let mut opts = Options::default();
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--threads" => opts.threads = it.next().and_then(|v| v.parse().ok()).ok_or("--threads N")?,
            "--no-dedup" => opts.inject_no_dedup = true,
            other => return Err(format!("argomento sconosciuto: {other}")),
        }
    }
    let progress = Progress::default();
    let r = scan(root, opts, &progress).map_err(|e| format!("radice non apribile: errore {}", e.0))?;
    let m = win::process_metrics();
    let t = &r.tree;
    let s = &r.stats;
    println!("radice            {root}");
    println!("nodi              {} ({} file, {} cartelle)", t.len(), s.files, s.dirs);
    println!("tempo             enumerazione {:.2} s, chiusura {:.2} s", s.enumeration.as_secs_f64(), s.finish.as_secs_f64());
    println!("CPU               utente {:.1} s, kernel {:.1} s", m.user.as_secs_f64(), m.kernel.as_secs_f64());
    let (units, wide) = t.name_units();
    println!("memoria           picco {} MB, attuale {} MB, albero {} MB, nomi {units} unità ({wide} UTF-16)", m.peak_working_set >> 20, m.working_set >> 20, t.heap_bytes() >> 20);
    println!("handle            {}", m.handles);
    println!("logico / allocato {} / {}", gib(t.logical(Tree::ROOT)), gib(t.alloc(Tree::ROOT)));
    println!("hard link         {} gruppi, {} nomi in più, {} contati una volta", r.links.groups.len(), r.links.extra_names, gib(r.links.extra_alloc));
    println!("WOF corretti      {}", s.alloc_fixed);
    println!("cloud             {} file, {} cartelle non elencate", s.cloud_files, s.cloud_dirs_not_listed);
    println!("non seguiti       {}", s.not_followed);
    println!("problemi          {} negate, {} altri, annullata: {}", s.denied, s.other_problems, r.cancelled);
    if let Some(b) = r.balance {
        println!("--- bilancio");
        println!("capacità          {}", gib(b.capacity));
        println!("usato inizio/fine {} / {} (deriva {})", gib(b.used_start), gib(b.used), gib_i(b.drift));
        println!("allocato unico A  {}", gib(b.allocated));
        println!("metadati M        $MFT {}, indici {}, USN {}", gib(b.metadata.mft.unwrap_or(0)), gib(b.metadata.dir_indexes), gib(b.metadata.usn_journal.unwrap_or(0)));
        println!("non attribuito N  {} (± {})", gib_i(b.unattributed), gib(b.uncertainty()));
    }
    for (node, p) in r.problems.iter().take(8) {
        println!("  {:?}: {}", p, t.path_lossy(*node));
    }
    let denied_alloc: usize = (0..t.len() as u32).filter(|&i| t.flags(i).has(Flags::DENIED)).count();
    println!("cartelle negate   {denied_alloc}");
    Ok(())
}
