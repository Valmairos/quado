// SPDX-License-Identifier: GPL-3.0-or-later
//! Misure per la baseline delle prestazioni, in sola lettura.
//!
//! - `fasi [--runs N]`: N processi `fasi-riga`, ciascuno con la scansione del volume di sistema come
//!   l'app (prove degli estrattori in parallelo), le fasi interne della scansione, il caricamento del
//!   risultato (`Loaded::with_rules`) e i suoi pezzi uno per uno, il controllo dei font di ripiego che
//!   l'interfaccia fa al primo frame, memoria e CPU. Mediana, minimo e massimo per ogni valore.
//! - `d3d12-costo [--runs N]`: quanto costa creare un dispositivo Direct3D 12 su ciascuna scheda video,
//!   come fa wgpu quando elenca gli adattatori (`D3D12CreateDevice`): tempo, memoria, moduli caricati. Ogni
//!   misura in un processo a sé (`d3d12-riga`).
//! - `rm-fasi [--runs N]`: le fasi di Restart Manager sui campioni della simulazione delle versioni di
//!   CapCut (apertura dei file, sessione, registrazione, elenco), una sessione per versione come la
//!   simulazione e una sola sessione per tutte; `processes_using` su più thread (una versione per
//!   chiamata) e con campioni più piccoli; poi le stesse cartelle lette con la scansione.
//! - `attributi-fasi [--runs N]`: la lettura delle cartelle della simulazione sulle stesse versioni, con
//!   le letture una per una di prima e con `WinProbe::entries` su più thread, e il confronto dei risultati.
//! - `lente-installatori [--cartella DIR] [--escludi "a;b"] [--thread N]...`: la lettura dei dati degli
//!   installatori della lente Download con N thread, senza i percorsi con una parte di `--escludi`.
//!
//! Le righe dei processi figli sono `chiave<TAB>valore`; il padre le raccoglie.

#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use quado_kb::{Kb, classify};
use quado_model::view::Ordered;
use quado_model::{Flags, Name, Tree};
use quado_scan_win::{Options, Progress, process, scan, system, volumes, win};
use quado_ui::app::data::{Loaded, Observed, VolumeInfo, today, top_list};

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn kv(k: &str, v: impl std::fmt::Display) {
    println!("{k}\t{v}");
}

fn mib(b: u64) -> f64 {
    b as f64 / f64::from(1u32 << 20)
}

fn stats(v: &mut [f64]) -> (f64, f64, f64) {
    v.sort_by(f64::total_cmp);
    let med = if v.len() % 2 == 1 { v[v.len() / 2] } else { (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0 };
    (med, v[0], v[v.len() - 1])
}

fn runs_arg(args: &[String], default: usize) -> Result<usize, String> {
    let mut it = args.iter();
    let mut runs = default;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--runs" => runs = it.next().and_then(|v| v.parse().ok()).ok_or("--runs N")?,
            other => return Err(format!("argomento sconosciuto: {other}")),
        }
    }
    Ok(runs)
}

/// Lancia `runs` volte `xtask <child> <extra>` e raccoglie le righe numeriche `chiave<TAB>valore`; le altre
/// righe (testo) si stampano una volta.
fn collect(child: &str, extra: &[&str], runs: usize) -> Result<BTreeMap<String, Vec<f64>>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut all: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut texts: Vec<String> = Vec::new();
    for i in 0..runs {
        let out = Command::new(&exe).arg(child).args(extra).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("esecuzione {}: {}", i + 1, String::from_utf8_lossy(&out.stderr)));
        }
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let mut parts = line.splitn(2, '\t');
            let (Some(k), Some(v)) = (parts.next(), parts.next()) else { continue };
            match v.trim().parse::<f64>() {
                Ok(x) => all.entry(k.to_owned()).or_default().push(x),
                Err(_) => {
                    if !texts.contains(&line.to_owned()) {
                        texts.push(line.to_owned());
                    }
                }
            }
        }
        eprintln!("{child}: esecuzione {} di {runs} finita", i + 1);
    }
    for t in texts {
        println!("testo\t{t}");
    }
    Ok(all)
}

fn print_table(all: &mut BTreeMap<String, Vec<f64>>) {
    println!("| Valore | Mediana | Minimo | Massimo | Esecuzioni |\n|---|---|---|---|---|");
    for (k, v) in all.iter_mut() {
        let n = v.len();
        let (med, min, max) = stats(v);
        println!("| `{k}` | {med:.1} | {min:.1} | {max:.1} | {n} |");
    }
}

// ---------------------------------------------------------------------------------------------
// fasi

pub fn phases(args: &[String]) -> Result<(), String> {
    let runs = runs_arg(args, 5)?;
    let mut all = collect("fasi-riga", &[], runs)?;
    println!("## Fasi della scansione e del caricamento ({runs} esecuzioni, cache calda)\n");
    println!("| Condizione | Valore |\n|---|---|");
    for (k, v) in crate::measure_cmd::environment() {
        println!("| {k} | {v} |");
    }
    println!();
    print_table(&mut all);
    Ok(())
}

pub fn phases_line(_args: &[String]) -> Result<(), String> {
    // Pezzi dell'avvio fuori dalla scheda video, come `App::new`: lingue (due volte, come l'app: una per
    // sapere quali ci sono, una per quella scelta), elenco dei volumi.
    let lang_dir = quado_ui::app::settings::data_dir().map(|d| d.join(quado_ui::app::settings::LANG_DIR));
    let t = Instant::now();
    let probe = quado_ui::i18n::Lingue::load("it", lang_dir.as_deref());
    kv("avvio-lingue-prima-ms", format!("{:.1}", ms(t.elapsed())));
    let t = Instant::now();
    let tag = quado_ui::i18n::first_run_language(&system::ui_languages(), &probe.available);
    let _l = quado_ui::i18n::Lingue::load(&tag, lang_dir.as_deref());
    kv("avvio-lingue-seconda-ms", format!("{:.1}", ms(t.elapsed())));
    let t = Instant::now();
    let vols = volumes::list();
    kv("avvio-volumi-ms", format!("{:.1}", ms(t.elapsed())));
    let vol = vols.iter().find(|v| v.is_system).and_then(VolumeInfo::from_volume).ok_or("volume di sistema")?;
    let sid = system::user_sid();
    let t = Instant::now();
    let kb = Arc::new(Kb::embedded().map_err(|e| format!("regole: {e:?}"))?);
    kv("regole-ms", format!("{:.1}", ms(t.elapsed())));

    // Come `ScanJob::start`: le prove degli estrattori in un thread, insieme alla scansione.
    let t0 = Instant::now();
    let (root, s2) = (vol.root.clone(), sid.clone());
    let ev_thread = std::thread::spawn(move || {
        let t = Instant::now();
        let e = quado_kb::win::gather(&root, s2);
        (e, t.elapsed())
    });
    let r = scan(&vol.root, Options::default(), &Progress::default()).map_err(|e| format!("scansione: errore {}", e.0))?;
    let scan_d = t0.elapsed();
    let m = win::process_metrics();
    let (ev, ev_d) = ev_thread.join().map_err(|_| "prove")?;
    let waited = t0.elapsed() - scan_d;
    let s = r.stats;
    kv("scansione-ms", format!("{:.0}", ms(scan_d)));
    kv("scansione-preparazione-ms", format!("{:.1}", ms(s.setup)));
    kv("scansione-enumerazione-ms", format!("{:.0}", ms(s.enumeration)));
    kv("scansione-costruttore-occupato-ms", format!("{:.0}", ms(s.builder_busy)));
    kv("scansione-chiusura-ms", format!("{:.0}", ms(s.finish)));
    kv("scansione-chiusura-seal-ms", format!("{:.1}", ms(s.seal)));
    kv("scansione-chiusura-hard-link-ms", format!("{:.0}", ms(s.links)));
    kv("scansione-chiusura-aggregate-ms", format!("{:.1}", ms(s.aggregate)));
    kv("scansione-coda-volume-ms", format!("{:.1}", ms(s.tail)));
    kv("thread-apertura-cartelle-ms", format!("{:.0}", ms(s.thread_open)));
    kv("thread-indice-cartelle-ms", format!("{:.0}", ms(s.thread_index)));
    kv("thread-elenco-ms", format!("{:.0}", ms(s.thread_enumerate)));
    kv("thread-chiusura-ms", format!("{:.0}", ms(s.thread_close)));
    kv("thread-wof-sparse-ms", format!("{:.0}", ms(s.thread_wof)));
    kv("scansione-cpu-utente-ms", format!("{:.0}", ms(m.user)));
    kv("scansione-cpu-kernel-ms", format!("{:.0}", ms(m.kernel)));
    kv("scansione-picco-working-set-mib", format!("{:.1}", mib(m.peak_working_set)));
    kv("scansione-working-set-mib", format!("{:.1}", mib(m.working_set)));
    kv("scansione-privata-mib", format!("{:.1}", mib(m.private)));
    kv("scansione-handle", m.handles);
    kv("nodi", r.tree.len());
    kv("file", s.files);
    kv("cartelle", s.dirs);
    kv("id-per-hard-link", s.id_entries);
    kv("allocata-corretta-wof", s.alloc_fixed);
    kv("albero-mib", format!("{:.1}", mib(r.tree.heap_bytes() as u64)));
    kv("prove-ms", format!("{:.0}", ms(ev_d)));
    kv("prove-attesa-dopo-la-scansione-ms", format!("{:.1}", ms(waited)));

    // Come il thread della scansione: indici delle viste, classificazione, Programmi, top list.
    let t = Instant::now();
    let loaded = Loaded::with_rules(vol, r, scan_d, sid.as_deref(), Arc::clone(&kb), ev);
    kv("caricamento-with-rules-ms", format!("{:.0}", ms(t.elapsed())));
    let m = win::process_metrics();
    kv("caricamento-picco-working-set-mib", format!("{:.1}", mib(m.peak_working_set)));
    kv("caricamento-working-set-mib", format!("{:.1}", mib(m.working_set)));
    kv("caricamento-privata-mib", format!("{:.1}", mib(m.private)));
    kv("indici-viste-mib", format!("{:.1}", mib(loaded.ord().heap_bytes() as u64)));

    // I pezzi uno per uno, sullo stesso modello.
    let tree = loaded.tree();
    let t = Instant::now();
    let ord = Ordered::build(tree);
    kv("pezzo-ordered-build-ms", format!("{:.0}", ms(t.elapsed())));
    let t = Instant::now();
    let cls = classify(&kb, tree, &loaded.evidence, today());
    kv("pezzo-classificazione-ms", format!("{:.0}", ms(t.elapsed())));
    let t = Instant::now();
    let programs = quado_kb::programs::build(tree, &loaded.evidence, &kb, &cls);
    kv("pezzo-programmi-ms", format!("{:.0}", ms(t.elapsed())));
    let t = Instant::now();
    let top = top_list(tree, &ord, &kb, &cls);
    kv("pezzo-top-list-ms", format!("{:.1}", ms(t.elapsed())));
    let t = Instant::now();
    let obs = Observed::compute(tree, &loaded.problems, sid.as_deref());
    kv("pezzo-cartelle-non-leggibili-ms", format!("{:.1}", ms(t.elapsed())));
    kv("programmi", programs.items.len());
    kv("top-list", top.len());
    kv("non-leggibili", obs.unreadable.len());
    let t = Instant::now();
    let outline = quado_model::view::Outline::new(&ord, tree, Tree::ROOT);
    kv("pezzo-outline-radice-ms", format!("{:.2}", ms(t.elapsed())));
    drop(outline);

    fonts_check(tree);
    Ok(())
}

/// Come `App::check_fonts`, sul thread dell'interfaccia al primo frame dopo il caricamento: un nome che
/// Plex non copre fa caricare i font di Windows.
fn fonts_check(tree: &Tree) {
    use quado_ui::fonts::{self, FontOptions};
    let ctx = egui::Context::default();
    let t = Instant::now();
    fonts::install(&ctx, FontOptions { system_fallbacks: false, segoe_family: false });
    let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
    kv("font-plex-primo-frame-ms", format!("{:.1}", ms(t.elapsed())));
    let font = quado_ui::widgets::sans(13.0);
    let mut first = None;
    let t = Instant::now();
    let needs = ctx.fonts_mut(|f| {
        (0..tree.len() as u32).any(|i| match tree.name(i) {
            Name::Ascii(_) => false,
            n => {
                let miss = n.to_string_lossy().chars().any(|c| !c.is_control() && !f.has_glyph(&font, c));
                if miss {
                    first = Some(i);
                }
                miss
            }
        })
    });
    kv("font-controllo-ms", format!("{:.1}", ms(t.elapsed())));
    kv("font-servono-ripieghi", u8::from(needs));
    if let Some(i) = first {
        kv("font-primo-nome-senza-glifi-indice", i);
        println!("font-primo-nome-senza-glifi\t{}", tree.path_lossy(i).replace('\t', " "));
    }
    // Quanti nomi non ASCII e quanti senza glifi in Plex, su tutto l'albero (solo informazione).
    let t = Instant::now();
    let (mut wide, mut missing) = (0u64, 0u64);
    ctx.fonts_mut(|f| {
        for i in 0..tree.len() as u32 {
            if let n @ Name::Wide(_) = tree.name(i) {
                wide += 1;
                if n.to_string_lossy().chars().any(|c| !c.is_control() && !f.has_glyph(&font, c)) {
                    missing += 1;
                }
            }
        }
    });
    kv("font-scansione-completa-ms", format!("{:.0}", ms(t.elapsed())));
    kv("nomi-non-ascii", wide);
    kv("nomi-senza-glifi-plex", missing);
    // Come l'app dopo le misure dell'avvio: caratteri distinti dei nomi (thread della scansione), controllo dei soli caratteri
    // (thread dell'interfaccia), scelta e apertura dei font che servono (altro thread), installazione e un
    // frame che disegna i caratteri.
    let t = Instant::now();
    let chars = quado_ui::app::data::name_chars(tree, 0, &[]);
    kv("font-caratteri-dei-nomi-ms", format!("{:.1}", ms(t.elapsed())));
    kv("font-caratteri-non-ascii", chars.len());
    let t = Instant::now();
    let missing = fonts::missing_glyphs(&ctx, chars.iter().copied());
    kv("font-controllo-caratteri-ms", format!("{:.2}", ms(t.elapsed())));
    kv("font-caratteri-senza-glifi", missing.len());
    println!("font-caratteri-senza-glifi-elenco\t{}", missing.iter().collect::<String>());
    let m0 = win::process_metrics();
    let t = Instant::now();
    let chosen = fonts::choose_fallbacks(&missing, &[]);
    kv("font-scelta-ms", format!("{:.1}", ms(t.elapsed())));
    let files: Vec<&str> = chosen.fonts.iter().map(|f| f.file).collect();
    println!("font-scelti\t{}", files.join(" "));
    kv("font-scelti-byte", chosen.fonts.iter().map(|f| f.bytes.len()).sum::<usize>());
    kv("font-scoperti", chosen.uncovered.len());
    let t = Instant::now();
    fonts::install_fallbacks(&ctx, &chosen.fonts);
    let text: String = missing.iter().collect();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.label(&text);
    });
    kv("font-scelti-installazione-e-frame-ms", format!("{:.1}", ms(t.elapsed())));
    let m1 = win::process_metrics();
    kv("font-scelti-privata-mib", format!("{:.1}", mib(m1.private.saturating_sub(m0.private))));
    kv("font-scelti-working-set-mib", format!("{:.1}", mib(m1.working_set.saturating_sub(m0.working_set))));
    // Per confronto, tutti e sette (come prima delle misure dell'avvio, ma proiettati in memoria e senza copie).
    let t = Instant::now();
    let report = fonts::install(&ctx, FontOptions { system_fallbacks: true, segoe_family: false });
    let read = t.elapsed();
    let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.label("漢字 かな 한글 עברית");
    });
    kv("font-ripieghi-lettura-ms", format!("{:.1}", ms(read)));
    kv("font-ripieghi-totale-ms", format!("{:.1}", ms(t.elapsed())));
    kv("font-ripieghi-byte", report.system_bytes());
    let m2 = win::process_metrics();
    kv("font-ripieghi-privata-mib", format!("{:.1}", mib(m2.private.saturating_sub(m0.private))));
    kv("font-ripieghi-working-set-mib", format!("{:.1}", mib(m2.working_set.saturating_sub(m0.working_set))));
}

// ---------------------------------------------------------------------------------------------
// d3d12-costo

pub fn d3d12(args: &[String]) -> Result<(), String> {
    let runs = runs_arg(args, 5)?;
    // Quante schede: dall'elenco del primo processo.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let list = Command::new(&exe).arg("d3d12-riga").arg("elenco").output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&list.stdout).into_owned();
    print!("{text}");
    let n = text.lines().filter(|l| l.starts_with("scheda\t")).count();
    println!("## Costo di D3D12CreateDevice per scheda ({runs} esecuzioni, ogni misura in un processo nuovo)\n");
    // Da sola, ogni scheda; poi tutte in fila come wgpu (0, 1, ..., n-1).
    let mut sets: Vec<Vec<String>> = (0..n).map(|i| vec![i.to_string()]).collect();
    sets.push((0..n).map(|i| i.to_string()).collect());
    for set in sets {
        let extra: Vec<&str> = set.iter().map(String::as_str).collect();
        println!("### Schede {}\n", set.join(", "));
        let mut all = collect("d3d12-riga", &extra, runs)?;
        print_table(&mut all);
        println!();
    }
    Ok(())
}

pub fn d3d12_line(args: &[String]) -> Result<(), String> {
    use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
    use windows::Win32::Graphics::Direct3D12::{D3D12CreateDevice, ID3D12Device};
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
    let mods = || process::loaded_modules();
    let before = mods();
    let m0 = win::process_metrics();
    let t = Instant::now();
    // SAFETY: nessun prerequisito; restituisce un'interfaccia COM con il conteggio dei riferimenti.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(|e| e.to_string())?;
    kv("fabbrica-dxgi-ms", format!("{:.2}", ms(t.elapsed())));
    if args.first().map(String::as_str) == Some("elenco") {
        for i in 0.. {
            // SAFETY: indice qualunque; oltre l'ultima scheda la chiamata restituisce un errore.
            let Ok(a) = (unsafe { factory.EnumAdapters1(i) }) else { break };
            // SAFETY: interfaccia valida appena ottenuta.
            let d = unsafe { a.GetDesc1() }.map_err(|e| e.to_string())?;
            let name = String::from_utf16_lossy(&d.Description[..d.Description.iter().position(|&c| c == 0).unwrap_or(d.Description.len())]);
            println!("scheda\t{i}\t{name}\tflag {:#x}\tmemoria video {} MiB", d.Flags, d.DedicatedVideoMemory >> 20);
        }
        return Ok(());
    }
    let mut devices: Vec<ID3D12Device> = Vec::new();
    for a in args {
        let i: u32 = a.parse().map_err(|_| "indice della scheda")?;
        // SAFETY: indice controllato dall'esito.
        let adapter = unsafe { factory.EnumAdapters1(i) }.map_err(|e| e.to_string())?;
        let t = Instant::now();
        let mut dev: Option<ID3D12Device> = None;
        // SAFETY: scheda valida; livello minimo 11_0 come wgpu; `dev` riceve il dispositivo.
        unsafe { D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_11_0, &mut dev) }.map_err(|e| e.to_string())?;
        kv(&format!("dispositivo-scheda-{i}-ms"), format!("{:.1}", ms(t.elapsed())));
        if let Some(d) = dev {
            devices.push(d);
        }
    }
    let m1 = win::process_metrics();
    let after = mods();
    kv("working-set-in-piu-mib", format!("{:.1}", mib(m1.working_set.saturating_sub(m0.working_set))));
    kv("privata-in-piu-mib", format!("{:.1}", mib(m1.private.saturating_sub(m0.private))));
    kv("handle-in-piu", m1.handles.saturating_sub(m0.handles));
    let new: Vec<&String> = after.iter().filter(|m| !before.contains(m)).collect();
    kv("moduli-in-piu", new.len());
    for m in new {
        let size = std::fs::metadata(m).map(|x| x.len()).unwrap_or(0);
        println!("modulo\t{m} ({} KiB)", size >> 10);
    }
    drop(devices);
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// rm-fasi

/// Campione della simulazione (`simulate::rm_sample`): tutti gli eseguibili e le DLL, più i 1.000
/// altri file modificati più di recente, dal modello della scansione.
fn rm_sample(tree: &Tree, root: u32) -> Vec<String> {
    rm_sample_with(tree, root, 1000)
}

/// Come [`rm_sample`], con al più `others_max` file che non sono eseguibili né DLL.
fn rm_sample_with(tree: &Tree, root: u32, others_max: usize) -> Vec<String> {
    let mut exe = Vec::new();
    let mut others: Vec<(i64, u32)> = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        for c in tree.children(n) {
            let f = tree.flags(c);
            if f.has(Flags::DIR) {
                if !f.has(Flags::NOT_FOLLOWED) {
                    stack.push(c);
                }
                continue;
            }
            if f.has(Flags::NOT_FOLLOWED) {
                continue;
            }
            let name = tree.name_lossy(c).to_ascii_lowercase();
            if name.ends_with(".exe") || name.ends_with(".dll") {
                exe.push(c);
            } else {
                others.push((tree.mtime(c), c));
            }
        }
    }
    others.sort_by_key(|&(m, _)| std::cmp::Reverse(m));
    others.truncate(others_max);
    exe.into_iter().chain(others.into_iter().map(|(_, n)| n)).map(|n| tree.path_lossy(n)).collect()
}

struct RmTimes {
    open: Duration,
    start: Duration,
    register: Duration,
    list: Duration,
    end: Duration,
    processes: usize,
}

/// Le fasi di Restart Manager, come `restart::restart_manager`, con i tempi; più l'apertura dei soli
/// attributi di ogni file, che `processes_using` fa prima per scegliere la strada.
fn rm_timed(paths: &[String]) -> Result<RmTimes, String> {
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::RestartManager::{CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources, RmStartSession};
    use windows::core::{PCWSTR, PWSTR};
    let t = Instant::now();
    for p in paths {
        let w: Vec<u16> = p.encode_utf16().collect();
        let _ = win::open_attributes(&w);
    }
    let open = t.elapsed();
    let t = Instant::now();
    let mut handle = 0u32;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    // SAFETY: variabili locali; chiave di CCH_RM_SESSION_KEY + 1 caratteri.
    let e = unsafe { RmStartSession(&mut handle, None, PWSTR(key.as_mut_ptr())) };
    if e != ERROR_SUCCESS {
        return Err(format!("RmStartSession {}", e.0));
    }
    let start = t.elapsed();
    let wide: Vec<Vec<u16>> = paths.iter().map(|p| p.encode_utf16().chain(std::iter::once(0)).collect()).collect();
    let t = Instant::now();
    for chunk in wide.chunks(1024) {
        let names: Vec<PCWSTR> = chunk.iter().map(|w| PCWSTR(w.as_ptr())).collect();
        // SAFETY: sessione aperta; nomi terminati da zero che vivono in `wide`.
        let e = unsafe { RmRegisterResources(handle, Some(&names), None, None) };
        if e != ERROR_SUCCESS {
            // SAFETY: sessione aperta sopra.
            let _ = unsafe { RmEndSession(handle) };
            return Err(format!("RmRegisterResources {}", e.0));
        }
    }
    let register = t.elapsed();
    let t = Instant::now();
    let mut infos: Vec<RM_PROCESS_INFO> = Vec::new();
    let mut processes = 0;
    for _ in 0..8 {
        let (mut needed, mut count, mut reasons) = (0u32, infos.len() as u32, 0u32);
        let array = if infos.is_empty() { None } else { Some(infos.as_mut_ptr()) };
        // SAFETY: `array` nullo con `count` 0 o con `count` elementi scrivibili.
        let e = unsafe { RmGetList(handle, &mut needed, &mut count, array, &mut reasons) };
        if e == ERROR_SUCCESS {
            processes = count as usize;
            break;
        }
        if e != ERROR_MORE_DATA {
            break;
        }
        infos = vec![RM_PROCESS_INFO::default(); needed as usize + 8];
    }
    let list = t.elapsed();
    let t = Instant::now();
    // SAFETY: sessione aperta sopra, chiusa una volta.
    let _ = unsafe { RmEndSession(handle) };
    let end = t.elapsed();
    Ok(RmTimes { open, start, register, list, end, processes })
}

pub fn rm_phases(args: &[String]) -> Result<(), String> {
    let runs = runs_arg(args, 3)?;
    let kb = Kb::embedded().map_err(|e| format!("regole: {e:?}"))?;
    let sid = system::user_sid();
    eprintln!("scansione di C:…");
    let r = scan("C:\\", Options::default(), &Progress::default()).map_err(|e| format!("scansione: errore {}", e.0))?;
    let tree = r.tree;
    let ev = quado_kb::win::gather("C:\\", sid);
    let cls = classify(&kb, &tree, &ev, today());
    let m = cls.matches.iter().find(|m| kb.rules[m.rule].id == "capcut-versioni").ok_or("nessuna corrispondenza di capcut-versioni")?;
    let samples: Vec<(String, Vec<String>)> = m.nodes.iter().map(|&n| (tree.name_lossy(n), rm_sample(&tree, n))).collect();
    let total: usize = samples.iter().map(|(_, s)| s.len()).sum();
    println!("## Restart Manager sui campioni delle versioni di CapCut\n");
    println!("Versioni {}, file nei campioni {total}.\n", samples.len());
    println!("| Giro | Modo | Apertura dei file | RmStartSession | RmRegisterResources | RmGetList | RmEndSession | Totale | `processes_using` | Processi |\n|---|---|---|---|---|---|---|---|---|---|");
    for run in 0..runs {
        // Come la simulazione: una sessione per versione.
        let mut sum = [Duration::ZERO; 5];
        let mut procs = 0;
        let mut whole = Duration::ZERO;
        for (_, s) in &samples {
            let x = rm_timed(s)?;
            sum[0] += x.open;
            sum[1] += x.start;
            sum[2] += x.register;
            sum[3] += x.list;
            sum[4] += x.end;
            procs += x.processes;
            let t = Instant::now();
            let _ = quado_actions::restart::processes_using(s);
            whole += t.elapsed();
        }
        let tot: Duration = sum.iter().sum();
        println!(
            "| {} | una sessione per versione ({}) | {:.0} ms | {:.0} ms | {:.0} ms | {:.0} ms | {:.0} ms | {:.0} ms | {:.0} ms | {procs} |",
            run + 1,
            samples.len(),
            ms(sum[0]),
            ms(sum[1]),
            ms(sum[2]),
            ms(sum[3]),
            ms(sum[4]),
            ms(tot),
            ms(whole)
        );
        // Una sessione sola per tutti i file.
        let all: Vec<String> = samples.iter().flat_map(|(_, s)| s.iter().cloned()).collect();
        let x = rm_timed(&all)?;
        let tot = x.open + x.start + x.register + x.list + x.end;
        let t = Instant::now();
        let _ = quado_actions::restart::processes_using(&all);
        let whole = t.elapsed();
        println!(
            "| {} | una sessione sola | {:.0} ms | {:.0} ms | {:.0} ms | {:.0} ms | {:.0} ms | {:.0} ms | {:.0} ms | {} |",
            run + 1,
            ms(x.open),
            ms(x.start),
            ms(x.register),
            ms(x.list),
            ms(x.end),
            ms(tot),
            ms(whole),
            x.processes
        );
    }
    // `processes_using` come la simulazione (una chiamata per versione), su più thread, e con campioni più
    // piccoli: tempo totale delle 26 chiamate.
    println!("\n| Giro | Thread | Altri file per versione | File | Tempo delle chiamate | Processi |\n|---|---|---|---|---|---|");
    let variants: Vec<(usize, usize)> = vec![(1, 1000), (4, 1000), (8, 1000), (1, 100), (8, 100), (1, 0)];
    for run in 0..runs {
        for &(threads, others) in &variants {
            let sets: Vec<Vec<String>> = if others == 1000 { samples.iter().map(|(_, s)| s.clone()).collect() } else { m.nodes.iter().map(|&n| rm_sample_with(&tree, n, others)).collect() };
            let files: usize = sets.iter().map(Vec::len).sum();
            let next = std::sync::atomic::AtomicUsize::new(0);
            let found = std::sync::atomic::AtomicUsize::new(0);
            let t = Instant::now();
            std::thread::scope(|sc| {
                for _ in 0..threads {
                    sc.spawn(|| {
                        loop {
                            let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(set) = sets.get(k) else { break };
                            let n = quado_actions::restart::processes_using(set).map_or(0, |v| v.len());
                            found.fetch_add(n, std::sync::atomic::Ordering::Relaxed);
                        }
                    });
                }
            });
            println!("| {} | {threads} | {others} | {files} | {:.0} ms | {} |", run + 1, ms(t.elapsed()), found.load(std::sync::atomic::Ordering::Relaxed));
        }
    }
    // RmGetList secondo il numero di file registrati.
    println!("\n| File registrati | RmRegisterResources | RmGetList |\n|---|---|---|");
    let all: Vec<String> = samples.iter().flat_map(|(_, s)| s.iter().cloned()).collect();
    for n in [1usize, 10, 100, 1000, 5000] {
        let part: Vec<String> = all.iter().take(n).cloned().collect();
        let x = rm_timed(&part)?;
        println!("| {} | {:.1} ms | {:.1} ms |", part.len(), ms(x.register), ms(x.list));
    }
    // Le stesse cartelle con la scansione (enumerazione delle cartelle, 12 thread), per confronto con la
    // visita della simulazione (un'apertura per file, un thread).
    let t = Instant::now();
    let mut entries = 0usize;
    for &n in &m.nodes {
        if let Ok(x) = scan(&tree.path_lossy(n), Options::default(), &Progress::default()) {
            entries += x.tree.len();
        }
    }
    println!("\nScansione delle stesse {} cartelle: {:.0} ms, {entries} voci.", m.nodes.len(), ms(t.elapsed()));
    let t = Instant::now();
    let mut opened = 0usize;
    for &n in &m.nodes {
        let mut stack = vec![n];
        while let Some(x) = stack.pop() {
            for c in tree.children(x) {
                let w = tree.path(c);
                let _ = win::open_attributes(&w).and_then(|h| win::standard_info(&h));
                opened += 1;
                if tree.flags(c).has(Flags::DIR) && !tree.flags(c).has(Flags::NOT_FOLLOWED) {
                    stack.push(c);
                }
            }
        }
    }
    println!("Apertura dei soli attributi di ogni voce, un thread: {:.0} ms, {opened} voci.", ms(t.elapsed()));
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// lente-installatori

/// `lente-installatori [--cartella DIR] [--escludi "a;b"] [--thread N]...`: la lettura dei dati degli
/// installatori della lente Download come la fa l'app (`lens_jobs::spawn_installers_with`, letture vere),
/// una volta per ogni `--thread` nell'ordine dato (predefinito: `lens_jobs::INSTALLER_THREADS`). Cartella
/// predefinita: Download dell'utente. I percorsi con una parte di `--escludi` (come nelle misure dell'app,
/// `misure.rs`) restano fuori dall'elenco e, in più, sono rifiutati prima di aprirli
/// (`Readers::excluding`). La prima lettura dopo un po' misura Defender, le successive no.
pub fn lens_installers(args: &[String]) -> Result<(), String> {
    use quado_model::downloads::LensFamily;
    use quado_ui::app::lens_jobs::{self, Readers, Target};
    let mut folder = std::env::var("USERPROFILE").map(|u| format!(r"{u}\Downloads")).map_err(|e| e.to_string())?;
    let mut threads: Vec<usize> = Vec::new();
    let mut exclude: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--cartella" => folder = it.next().ok_or("--cartella DIR")?.clone(),
            "--escludi" => exclude = it.next().ok_or("--escludi \"a;b\"")?.split(';').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect(),
            "--thread" => threads.push(it.next().and_then(|v| v.parse().ok()).ok_or("--thread N")?),
            other => return Err(format!("argomento sconosciuto: {other}")),
        }
    }
    if threads.is_empty() {
        threads.push(lens_jobs::INSTALLER_THREADS);
    }
    let excluded = |p: &str| {
        let p = p.to_lowercase();
        exclude.iter().any(|c| p.contains(c.as_str()))
    };
    let t = Instant::now();
    let r = scan(&folder, Options::default(), &Progress::default()).map_err(|e| format!("scansione: errore {}", e.0))?;
    let scan_ms = ms(t.elapsed());
    let tree = r.tree;
    let lens = quado_model::downloads::build(&tree, Tree::ROOT);
    let all: Vec<(Target, quado_kb::installers::InstallerKind)> = lens
        .of_family(LensFamily::Installers)
        .filter(|i| !i.placeholder)
        .filter_map(|i| {
            let kind = quado_kb::installers::installer_kind(&tree.name_lossy(i.node))?;
            Some((Target { node: i.node, path: tree.path_lossy(i.node), size: i.logical }, kind))
        })
        .collect();
    let targets: Vec<_> = all.iter().filter(|(t, _)| !excluded(&t.path)).cloned().collect();
    let bytes: u64 = targets.iter().map(|(t, _)| t.size).sum();
    let readers = Readers::live().excluding(&exclude);
    println!("## Dati degli installatori della lente Download\n");
    println!(
        "Cartella `{folder}` (scansione {scan_ms:.0} ms), installatori {} ({} esclusi con --escludi), da leggere {} file, {:.2} GB.\n",
        all.len(),
        all.len() - targets.len(),
        targets.len(),
        bytes as f64 / 1e9
    );
    println!("| Lettura | Thread | Tempo | File | Riusciti | Risultati |\n|---|---|---|---|---|---|");
    let mut reference: Option<String> = None;
    for (k, &n) in threads.iter().enumerate() {
        let t = Instant::now();
        let got = lens_jobs::spawn_installers_with(&readers, targets.clone(), n).wait();
        let took = t.elapsed();
        let order = got.iter().enumerate().all(|(i, x)| x.0 == i);
        let signature = format!("{got:?}");
        let same = match &reference {
            None => {
                reference = Some(signature);
                "riferimento"
            }
            Some(r) if *r == signature => "uguali",
            Some(_) => "**diversi**",
        };
        println!(
            "| {} | {n} | {:.0} ms | {} | {} | {same}{} |",
            k + 1,
            ms(took),
            got.len(),
            got.iter().filter(|x| x.1.is_ok()).count(),
            if order { ", in ordine" } else { ", **fuori ordine**" }
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// attributi-fasi

/// `WinProbe` per misurare la lettura delle cartelle della simulazione: con le letture una per una (come
/// prima delle letture in parallelo: `children`, poi `attributes` di ogni voce, un thread) oppure con `entries` e i thread
/// scelti. L'uso dei file non si chiede: la risposta è vuota e i campioni si registrano per il confronto.
/// Cestino e cartelle di sincronizzazione hanno risposte fisse: niente di C: si legge oltre alle cartelle.
struct DirProbe<'a> {
    inner: &'a quado_actions::win_probe::WinProbe,
    fast: bool,
    threads: usize,
    samples: std::sync::Mutex<Vec<Vec<String>>>,
}

impl quado_actions::probe::Probe for DirProbe<'_> {
    fn attributes(&self, path: &str) -> Result<quado_actions::probe::Attrs, u32> {
        self.inner.attributes(path)
    }
    fn children(&self, dir: &str) -> Result<Vec<String>, u32> {
        self.inner.children(dir)
    }
    fn entries(&self, dir: &str, step: &dyn Fn() -> bool) -> Result<Vec<quado_actions::probe::Entry>, quado_actions::probe::ListError> {
        if self.fast { self.inner.entries(dir, step) } else { quado_actions::probe::entries_one_by_one(self, dir, step) }
    }
    fn threads(&self) -> usize {
        self.threads
    }
    fn processes_using(&self, paths: &[String]) -> Result<Vec<quado_actions::restart::ProcessUse>, quado_actions::restart::RmError> {
        let mut p = paths.to_vec();
        p.sort();
        if let Ok(mut s) = self.samples.lock() {
            s.push(p);
        }
        Ok(Vec::new())
    }
    fn references(&self, _: &[String]) -> quado_model::intervention::ReferenceReport {
        quado_model::intervention::ReferenceReport::default()
    }
    fn volume_bin(&self, _: &str) -> Result<quado_actions::probe::VolumeBin, u32> {
        Ok(quado_actions::probe::VolumeBin { root: r"C:\".into(), has_bin: true, capacity: Some(u64::MAX / 4), ..Default::default() })
    }
    fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
        Ok(None)
    }
    fn sync_root(&self, _: &str) -> Option<String> {
        None
    }
    fn free_space(&self, _: &str) -> Result<u64, u32> {
        Ok(0)
    }
    fn now(&self) -> i64 {
        0
    }
}

/// Ciò che la simulazione ha letto, per il confronto tra i modi: elementi, stima della selezione proposta,
/// campioni di Restart Manager (in ordine: con più thread le chiamate arrivano in un altro ordine).
fn sim_signature(sim: &quado_actions::plan::Simulazione, mut samples: Vec<Vec<String>>) -> String {
    let mut s = String::new();
    for it in sim.items() {
        s.push_str(&format!("{} {} {} {} {} {} {:?}\n", it.path, it.files, it.dirs, it.logical, it.alloc, it.denied_dirs, it.blocks));
    }
    s.push_str(&format!("{:?}\n", sim.estimate(&sim.default_selection())));
    samples.sort();
    s.push_str(&format!("{samples:?}"));
    s
}

/// Costo di un'apertura per leggere gli attributi, strada per strada, su un thread: tutte le voci (file e
/// cartelle) sotto `roots`, prese dal modello. Ogni strada fa le stesse domande (`FileBasicInfo` e
/// `FileStandardInfo`) tranne la domanda per nome, che dà tutto in una chiamata senza handle. Solo misura:
/// la domanda per nome non permette `FILE_OPEN_NO_RECALL`, quindi non entra nel prodotto.
fn open_costs(tree: &Tree, roots: &[u32]) {
    use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
    use windows_sys::Wdk::Storage::FileSystem::{
        FILE_DIRECTORY_FILE, FILE_OPEN_FOR_BACKUP_INTENT, FILE_OPEN_NO_RECALL, FILE_OPEN_REPARSE_POINT, FILE_STAT_INFORMATION, FILE_SYNCHRONOUS_IO_NONALERT,
        FileStatInformation, NtOpenFile, NtQueryInformationByName,
    };
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, OBJ_CASE_INSENSITIVE, UNICODE_STRING};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_BASIC_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_NO_RECALL, FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY,
        FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO, FileBasicInfo, FileStandardInfo,
        GetFileInformationByHandleEx, OPEN_EXISTING, SYNCHRONIZE,
    };
    use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
    const SHARE: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
    const OPTIONS: u32 = FILE_OPEN_FOR_BACKUP_INTENT | FILE_OPEN_REPARSE_POINT | FILE_OPEN_NO_RECALL | FILE_SYNCHRONOUS_IO_NONALERT;
    let w = |s: &str| -> Vec<u16> { s.encode_utf16().collect() };
    // Due domande su un handle aperto, poi la chiusura.
    let ask = |h: HANDLE| -> u64 {
        // SAFETY: strutture di soli valori; lo zero è valido.
        let (mut b, mut s): (FILE_BASIC_INFO, FILE_STANDARD_INFO) = unsafe { (std::mem::zeroed(), std::mem::zeroed()) };
        // SAFETY: handle valido aperto dal chiamante; buffer locali della dimensione dichiarata; poi lo si chiude
        // una volta.
        unsafe {
            GetFileInformationByHandleEx(h, FileBasicInfo, (&raw mut b).cast(), size_of::<FILE_BASIC_INFO>() as u32);
            GetFileInformationByHandleEx(h, FileStandardInfo, (&raw mut s).cast(), size_of::<FILE_STANDARD_INFO>() as u32);
            CloseHandle(h);
        }
        u64::from(s.NumberOfLinks)
    };
    let create_with = |path: &[u16], access: u32| -> Option<HANDLE> {
        let z = win::wide_z(&win::long_path(path));
        // SAFETY: stringa terminata da zero; accesso dei soli attributi o dell'elenco (cartelle del modello),
        // nessun collegamento seguito, nessun richiamo.
        let h = unsafe {
            CreateFileW(
                z.as_ptr(),
                access,
                SHARE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_OPEN_NO_RECALL,
                std::ptr::null_mut(),
            )
        };
        (h != INVALID_HANDLE_VALUE).then_some(h)
    };
    let create = |path: &[u16]| create_with(path, FILE_READ_ATTRIBUTES | SYNCHRONIZE);
    // Stato di `NtOpenFile` con un percorso intero; l'handle, se c'è, si chiude subito.
    let nt_status = |name: &[u16], access: u32, options: u32| -> i32 {
        let bytes = (name.len() * 2) as u16;
        let us = UNICODE_STRING { Length: bytes, MaximumLength: bytes, Buffer: name.as_ptr().cast_mut() };
        let oa = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: std::ptr::null_mut(),
            ObjectName: &us,
            Attributes: OBJ_CASE_INSENSITIVE,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };
        // SAFETY: struttura di soli valori.
        let mut iosb: IO_STATUS_BLOCK = unsafe { std::mem::zeroed() };
        let mut h: HANDLE = std::ptr::null_mut();
        // SAFETY: `us` descrive `name`, che vive per la chiamata; uscite locali; l'handle si chiude subito.
        let st = unsafe { NtOpenFile(&mut h, access, &oa, &mut iosb, SHARE, options) };
        if st >= 0 {
            // SAFETY: handle appena aperto, chiuso una volta.
            unsafe { CloseHandle(h) };
        }
        st
    };
    let nt = |root: HANDLE, name: &[u16], access: u32, options: u32| -> Option<HANDLE> {
        let bytes = (name.len() * 2) as u16;
        let us = UNICODE_STRING { Length: bytes, MaximumLength: bytes, Buffer: name.as_ptr().cast_mut() };
        let oa = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: root,
            ObjectName: &us,
            Attributes: OBJ_CASE_INSENSITIVE,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };
        // SAFETY: struttura di soli valori.
        let mut iosb: IO_STATUS_BLOCK = unsafe { std::mem::zeroed() };
        let mut h: HANDLE = std::ptr::null_mut();
        // SAFETY: `us` descrive `name`, che vive per la chiamata; `root` nullo o aperto qui; uscite locali.
        let st = unsafe { NtOpenFile(&mut h, access, &oa, &mut iosb, SHARE, options) };
        (st >= 0).then_some(h)
    };
    let by_name = |root: HANDLE, name: &[u16]| -> u64 {
        let bytes = (name.len() * 2) as u16;
        let us = UNICODE_STRING { Length: bytes, MaximumLength: bytes, Buffer: name.as_ptr().cast_mut() };
        let oa = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: root,
            ObjectName: &us,
            Attributes: OBJ_CASE_INSENSITIVE,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };
        // SAFETY: strutture di soli valori.
        let (mut iosb, mut info): (IO_STATUS_BLOCK, FILE_STAT_INFORMATION) = unsafe { (std::mem::zeroed(), std::mem::zeroed()) };
        // SAFETY: come sopra; `info` è un buffer locale della dimensione dichiarata.
        let st = unsafe { NtQueryInformationByName(&oa, &mut iosb, (&raw mut info).cast(), size_of::<FILE_STAT_INFORMATION>() as u32, FileStatInformation) };
        if st >= 0 { u64::from(info.NumberOfLinks) } else { 0 }
    };
    // Voci per cartella: (percorso della cartella, nomi).
    let mut dirs: Vec<(String, Vec<String>)> = Vec::new();
    for &root in roots {
        let mut stack = vec![root];
        while let Some(d) = stack.pop() {
            let names: Vec<String> = tree.children(d).map(|c| tree.name_lossy(c)).collect();
            dirs.push((tree.path_lossy(d), names));
            stack.extend(tree.children(d).filter(|&c| tree.flags(c).has(Flags::DIR) && !tree.flags(c).has(Flags::NOT_FOLLOWED)));
        }
    }
    let entries: usize = dirs.iter().map(|(_, n)| n.len()).sum();
    println!("\n### Costo di un'apertura per gli attributi, un thread\n\n{} cartelle, {entries} voci.\n", dirs.len());
    println!("| Giro | Strada | Tempo | Per voce | Nomi (somma di NumberOfLinks) |\n|---|---|---|---|---|");
    for round in 1..=2 {
        let t = Instant::now();
        let mut links = 0u64;
        for (d, names) in &dirs {
            for n in names {
                links += create(&w(&format!(r"{d}\{n}"))).map_or(0, ask);
            }
        }
        let a = t.elapsed();
        println!("| {round} | `CreateFileW` con il percorso intero (come prima) | {:.0} ms | {:.1} µs | {links} |", ms(a), ms(a) * 1000.0 / entries as f64);
        let t = Instant::now();
        let mut links = 0u64;
        for (d, names) in &dirs {
            for n in names {
                links += nt(std::ptr::null_mut(), &w(&format!(r"\??\{d}\{n}")), FILE_READ_ATTRIBUTES | SYNCHRONIZE, OPTIONS).map_or(0, ask);
            }
        }
        let a = t.elapsed();
        println!("| {round} | `NtOpenFile` con il percorso intero | {:.0} ms | {:.1} µs | {links} |", ms(a), ms(a) * 1000.0 / entries as f64);
        let t = Instant::now();
        let mut links = 0u64;
        let mut refused = 0usize;
        for (d, names) in &dirs {
            // La cartella come la apre la scansione (`win::open_dir`).
            let Some(dh) = create_with(&w(d), FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE) else { continue };
            for n in names {
                match nt(dh, &w(n), FILE_READ_ATTRIBUTES | SYNCHRONIZE, OPTIONS) {
                    Some(h) => links += ask(h),
                    None => refused += 1,
                }
            }
            // SAFETY: handle aperto qui sopra, chiuso una volta.
            unsafe { CloseHandle(dh) };
        }
        let a = t.elapsed();
        println!(
            "| {round} | `NtOpenFile` rispetto alla cartella (più l'apertura della cartella) | {:.0} ms | {:.1} µs | {links} ({refused} aperture rifiutate) |",
            ms(a),
            ms(a) * 1000.0 / entries as f64
        );
        // Prova delle opzioni: l'elenco con "solo cartelle" e senza richiamo.
        if round == 1 {
            let d = &dirs[0].0;
            let st = nt_status(&w(&format!(r"\??\{d}")), FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE, OPTIONS | FILE_DIRECTORY_FILE);
            let st2 = nt_status(&w(&format!(r"\??\{d}")), FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE, (OPTIONS & !FILE_OPEN_NO_RECALL) | FILE_DIRECTORY_FILE);
            println!("| | `FILE_DIRECTORY_FILE` con `FILE_OPEN_NO_RECALL`: stato {st:#x}; senza `FILE_OPEN_NO_RECALL`: {st2:#x} | | | |");
        }
        // Le stesse aperture con il percorso intero su 8 thread.
        let t = Instant::now();
        let all: Vec<String> = dirs.iter().flat_map(|(d, names)| names.iter().map(move |n| format!(r"{d}\{n}"))).collect();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let total = std::sync::atomic::AtomicU64::new(0);
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    loop {
                        let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(p) = all.get(k) else { break };
                        total.fetch_add(create(&w(p)).map_or(0, ask), std::sync::atomic::Ordering::Relaxed);
                    }
                });
            }
        });
        let a = t.elapsed();
        println!(
            "| {round} | `CreateFileW` con il percorso intero, 8 thread | {:.0} ms | {:.1} µs | {} |",
            ms(a),
            ms(a) * 1000.0 / entries as f64,
            total.load(std::sync::atomic::Ordering::Relaxed)
        );
        let t = Instant::now();
        let mut links = 0u64;
        for (d, names) in &dirs {
            for n in names {
                links += by_name(std::ptr::null_mut(), &w(&format!(r"\??\{d}\{n}")));
            }
        }
        let a = t.elapsed();
        println!("| {round} | domanda per nome (`NtQueryInformationByName`, `FileStatInformation`) | {:.0} ms | {:.1} µs | {links} |", ms(a), ms(a) * 1000.0 / entries as f64);
    }
}

/// `attributi-fasi [--runs N]`: la lettura delle cartelle della simulazione sulle versioni di CapCut, in
/// sola lettura, con le letture una per una (prima del parallelo) e con `WinProbe::entries` su 1, 4, 8 e 12
/// thread, più il confronto dei risultati (uguali al byte) e, una volta, quello voce per voce degli
/// attributi delle due strade.
pub fn attr_phases(args: &[String]) -> Result<(), String> {
    use quado_actions::probe::Probe;
    use quado_actions::simulate::{Request, RequestItem, SimProgress, simulate};
    // `--aperture`: solo il costo di un'apertura per strada (`open_costs`).
    let only_opens = args.iter().any(|a| a == "--aperture");
    let rest: Vec<String> = args.iter().filter(|a| *a != "--aperture").cloned().collect();
    let runs = runs_arg(&rest, 3)?;
    let kb = Kb::embedded().map_err(|e| format!("regole: {e:?}"))?;
    let sid = system::user_sid();
    eprintln!("scansione di C:…");
    let r = scan("C:\\", Options::default(), &Progress::default()).map_err(|e| format!("scansione: errore {}", e.0))?;
    let tree = r.tree;
    let ev = quado_kb::win::gather("C:\\", sid);
    let cls = classify(&kb, &tree, &ev, today());
    let m = cls.matches.iter().find(|m| kb.rules[m.rule].id == "capcut-versioni").ok_or("nessuna corrispondenza di capcut-versioni")?;
    let paths: Vec<String> = m.nodes.iter().map(|&n| tree.path_lossy(n)).collect();
    if only_opens {
        open_costs(&tree, &m.nodes);
        return Ok(());
    }
    let request = || Request { items: paths.iter().map(|p| RequestItem { path: p.clone(), fixed: None }).collect(), record: m.record.clone(), protected: Vec::new() };
    let win = quado_actions::win_probe::WinProbe::new(Box::new(quado_ui::app::intervention_jobs::KbConfig));
    println!("## Lettura delle cartelle della simulazione sulle versioni di CapCut\n");
    println!("Versioni {}. Restart Manager, Cestino e riferimenti esclusi (risposte fisse).\n", paths.len());
    println!("| Giro | Modo | Tempo | Voci lette | Risultato |\n|---|---|---|---|---|");
    let modes: [(&str, bool, usize); 5] =
        [("una per una, 1 thread (prima)", false, 1), ("elenco, 1 thread", true, 1), ("elenco, 4 thread", true, 4), ("elenco, 8 thread", true, 8), ("elenco, 12 thread", true, 12)];
    let mut reference: Option<String> = None;
    for run in 0..runs {
        for &(name, fast, threads) in &modes {
            let p = DirProbe { inner: &win, fast, threads, samples: std::sync::Mutex::new(Vec::new()) };
            let progress = SimProgress::default();
            let t = Instant::now();
            let sim = simulate(request(), &p, &progress).map_err(|_| "simulazione annullata")?;
            let took = t.elapsed();
            let signature = sim_signature(&sim, p.samples.into_inner().unwrap_or_default());
            let same = match &reference {
                None => {
                    reference = Some(signature);
                    "riferimento"
                }
                Some(r) if *r == signature => "uguale",
                Some(_) => "**diverso**",
            };
            println!("| {} | {name} | {:.0} ms | {} | {same} |", run + 1, ms(took), progress.done.load(std::sync::atomic::Ordering::Relaxed));
        }
    }
    // Voce per voce: attributi delle due strade per ogni cartella delle versioni.
    let (mut dirs, mut entries, mut differ) = (0usize, 0usize, Vec::new());
    let go = || true;
    for root in &paths {
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            dirs += 1;
            let old = quado_actions::probe::entries_one_by_one(&win, &dir, &go);
            let new = win.entries(&dir, &go);
            match (&old, &new) {
                (Ok(o), Ok(n)) if o.len() == n.len() => {
                    for (a, b) in o.iter().zip(n) {
                        if a != b {
                            differ.push(format!("{dir}\\{}: {:?} contro {b:?}", a.0, a.1));
                        }
                    }
                }
                _ if old != new => differ.push(format!("{dir}: {:?} contro {:?}", old.as_ref().map(Vec::len), new.as_ref().map(Vec::len))),
                _ => {}
            }
            if let Ok(list) = &old {
                entries += list.len();
                stack.extend(list.iter().filter(|(_, a)| matches!(a, Ok(a) if a.directory && !a.link)).map(|(name, _)| format!("{dir}\\{name}")));
            }
        }
    }
    println!("\nVoce per voce: {dirs} cartelle, {entries} voci, differenze {}.", differ.len());
    for d in differ.iter().take(10) {
        println!("- {d}");
    }
    Ok(())
}
