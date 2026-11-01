// SPDX-License-Identifier: GPL-3.0-or-later
//! Test che chiudono righe scoperte della tabella di copertura dei requisiti (06 §13), scritti in
//! M6 dall'agente P. Ognuno prova nell'interfaccia un requisito che prima era provato solo in parte:
//! - F-SC-4: l'avanzamento della scansione si aggiorna dal vivo, totali di primo livello compresi;
//! - F-SC-10: exFAT e FAT32 con la nota, ReFS e unità di rete "senza garanzie", nei volumi e nel banner;
//! - F-IN-2: "Perché è così grande" nell'ispettore: parti, schemi con la regola, tipi ed età dei file;
//! - F-IN-5: "Copia percorso" mette negli appunti il percorso completo e lo dice nella barra di stato;
//! - F-SY-1, F-SY-2: Windows e sistema con il Cestino in due modi e l'anomalia, e le voci che senza
//!   amministratore non si misurano dette "non disponibile", mai 0.
//!
//! Tutto sul C: sintetico di `synthetic::sample_c`: niente legge o scrive sul disco vero.

use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use quado_scan_win::Progress;
use quado_ui::app::data::ScanJob;
use quado_ui::app::{App, Env, Screen, kbtext, synthetic};

fn app() -> App {
    let mut env = Env::fixed();
    env.windows_languages = vec!["it-IT".into()];
    App::new(env, Vec::new())
}

/// Finestra alta: ispettore e viste intere senza scorrere.
fn harness(a: App) -> Harness<'static, App> {
    Harness::builder().with_size([1600.0, 2600.0]).build_ui_state(|ui, app: &mut App| app.show(ui), a)
}

fn loaded() -> Harness<'static, App> {
    let mut a = app();
    let (v, r) = synthetic::sample_c(false);
    a.load_result_with(v, r, Duration::from_secs(14), synthetic::sample_c_evidence());
    let mut h = harness(a);
    h.run_steps(4);
    h
}

/// Testi disegnati nell'ultimo frame.
fn drawn(h: &Harness<'_, App>) -> Vec<String> {
    fn walk(s: &egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut out);
    }
    out
}

fn shows(h: &Harness<'_, App>, s: &str) -> bool {
    drawn(h).iter().any(|t| t == s)
}

fn node_named(h: &Harness<'_, App>, n: &str) -> u32 {
    let t = h.state().data.as_ref().expect("dati").tree();
    (0..t.len() as u32).find(|&i| t.name_lossy(i) == n).unwrap_or_else(|| panic!("{n}"))
}

#[test]
fn l_avanzamento_della_scansione_si_aggiorna_dal_vivo() {
    // F-SC-4: file letti, cartelle, spazio misurato e totali di primo livello crescono mentre la scansione
    // procede; la barra di stato dice quanti file sono stati letti.
    let mut a = app();
    let top = [("Users", 300u64 << 30), ("Windows", 30 << 30)].iter().map(|(n, b)| (n.encode_utf16().collect(), *b)).collect();
    a.job = Some(ScanJob::preview(synthetic::volume_c(), Progress::preset(1_000, 100, 10 << 30, top), Duration::from_secs(2)));
    a.screen = Screen::Scanning;
    let mut h = harness(a);
    h.run_steps(3);
    let f = h.state().fmt.clone();
    assert!(shows(&h, &f.count(1_000)), "file letti all'inizio");
    assert!(shows(&h, &f.size(300 << 30)), "Users all'inizio");
    {
        let p = &h.state().job.as_ref().expect("scansione").progress;
        p.files.fetch_add(234_567, Relaxed);
        p.dirs.fetch_add(4_321, Relaxed);
        p.alloc.fetch_add(5 << 30, Relaxed);
        // Windows supera Users: l'elenco di primo livello si riordina.
        let top = p.top().expect("primo livello");
        top[1].alloc.store(400 << 30, Relaxed);
    }
    h.run_steps(3);
    assert!(shows(&h, &f.count(235_567)), "file letti aggiornati");
    assert!(shows(&h, &f.count(4_421)), "cartelle aggiornate");
    assert!(shows(&h, &f.size(15 << 30)), "spazio misurato aggiornato");
    assert!(shows(&h, &f.size(400 << 30)), "totale di primo livello aggiornato");
    let status = h.state().lingue.ta("stato-scansione-in-corso", &[("file", f.count(235_567).into())]);
    assert!(shows(&h, &status), "barra di stato: {status}");
    // Il primo dell'elenco ora è Windows.
    let texts = drawn(&h);
    let windows = texts.iter().position(|t| t == "Windows").expect("Windows");
    let users = texts.iter().position(|t| t == "Users").expect("Users");
    assert!(windows < users, "il più grande in cima");
}

#[test]
fn perche_e_cosi_grande_nell_ispettore() {
    // F-IN-2: la cartella di CapCut nell'ispettore: le parti principali, gli schemi riconosciuti dalle
    // regole con la regola citata, i tipi secondo l'estensione e l'età dei file.
    let mut h = loaded();
    {
        let a = h.state_mut();
        a.go(Screen::Explore);
        let d = a.data.as_ref().expect("dati");
        let t = d.tree();
        let capcut = (0..t.len() as u32).find(|&i| t.name_lossy(i) == "CapCut").expect("CapCut");
        a.explore.open(d, t.parent(capcut), Some(capcut));
    }
    h.run_steps(4);
    // Tipi ed età arrivano in background.
    h.state_mut().wait_background();
    h.run_steps(4);
    // Tipi ed età stanno in sezioni chiuse finché non le apri (variante A).
    for key in ["ispettore-tipi-di-file", "ispettore-eta"] {
        let title = h.state().lingue.t(key);
        h.get_by_label(&title).click();
        h.run_steps(2);
    }
    let a = h.state();
    let l = &a.lingue;
    for key in ["ispettore-parti-principali", "ispettore-schemi", "ispettore-tipi-di-file", "ispettore-secondo-estensione", "ispettore-eta", "ispettore-eta-nota"] {
        assert!(shows(&h, &l.t(key)) || shows(&h, &l.t(key).to_uppercase()), "manca «{}»", l.t(key));
    }
    // Le parti: le due sottocartelle di CapCut, con la loro dimensione.
    for part in ["Apps", "User Data"] {
        assert!(shows(&h, part), "parte {part}");
    }
    // Lo schema delle versioni precedenti cita la sua regola.
    let d = a.data.as_ref().expect("dati");
    let rule = d.kb.index("capcut-versioni").expect("regola");
    let sub = l.ta("ispettore-schema-regola", &[("regola", kbtext::rule_name(l, &d.kb, rule).into())]);
    assert!(shows(&h, &sub), "schema con la regola: {sub}");
    // Tipi secondo l'estensione: le dll delle versioni e i video dei progetti.
    for fam in ["famiglia-componenti", "famiglia-video"] {
        assert!(shows(&h, &l.t(fam)), "tipo {}", l.t(fam));
    }
}

#[test]
fn copia_percorso_mette_il_percorso_negli_appunti() {
    // F-IN-5: "Copia percorso" copia il percorso completo e lo dice nella barra di stato.
    let mut h = loaded();
    let node = node_named(&h, "Cargo.lock");
    {
        let a = h.state_mut();
        a.go(Screen::Explore);
        let d = a.data.as_ref().expect("dati");
        a.explore.reveal(d, node);
    }
    h.run_steps(4);
    h.state_mut().wait_background();
    h.run_steps(3);
    let path = h.state().data.as_ref().expect("dati").tree().path_lossy(node);
    let label = h.state().lingue.t("azione-copia-percorso");
    h.get_by_label(&label).click();
    h.step();
    let copied: Vec<String> = h
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(s) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(copied, vec![path.clone()], "negli appunti il percorso completo");
    assert_eq!(path, r"C:\Users\demo\Documents\progetto-rust\Cargo.lock");
    h.run_steps(2);
    let notice = h.state().lingue.t("avviso-percorso-copiato");
    assert!(shows(&h, &notice), "avviso nella barra di stato");
}

#[test]
fn file_system_fat_e_senza_garanzie_lo_dicono() {
    // F-SC-10: exFAT e FAT32 con la nota sui cluster; ReFS e le unità di rete "senza garanzie", nell'elenco
    // dei volumi e nel banner delle viste con i dati.
    use quado_model::Space;
    use quado_scan_win::volumes::{DriveKind, Support, Volume};
    let gib = 1u64 << 30;
    let vol = |letter: &str, fs: &str, kind: DriveKind| Volume {
        guid_path: String::new(),
        paths: vec![format!("{letter}\\")],
        label: String::new(),
        file_system: fs.into(),
        kind,
        space: Some(Space { capacity: 100 * gib, free: 40 * gib, free_to_caller: 40 * gib }),
        is_system: false,
    };
    let vols = vec![vol("C:", "NTFS", DriveKind::Fixed), vol("E:", "exFAT", DriveKind::Removable), vol("Z:", "ReFS", DriveKind::Fixed), vol("N:", "NTFS", DriveKind::Network)];
    assert_eq!(vols.iter().map(Volume::support).collect::<Vec<_>>(), vec![Support::Full, Support::Fat, Support::NoGuarantees, Support::NoGuarantees]);
    let mut env = Env::fixed();
    env.windows_languages = vec!["it-IT".into()];
    let mut h = harness(App::new(env, vols));
    h.run_steps(3);
    let fat = h.state().lingue.ta("volumi-nota-fat", &[("fs", "exFAT".into())]);
    assert!(shows(&h, &fat), "nota exFAT");
    let none = h.state().lingue.t("volumi-senza-garanzie");
    assert_eq!(drawn(&h).iter().filter(|t| **t == none).count(), 2, "ReFS e unità di rete");
    // Il banner sulle viste con i dati.
    for (fs, support, key) in [("ReFS", Support::NoGuarantees, "banner-senza-garanzie"), ("FAT32", Support::Fat, "banner-fat")] {
        let mut a = app();
        let (mut v, r) = synthetic::sample_c(false);
        v.file_system = fs.into();
        v.support = support;
        a.load_result(v, r, Duration::from_secs(1));
        let mut h = harness(a);
        h.run_steps(2);
        h.state_mut().go(Screen::Explore);
        h.run_steps(3);
        let banner = h.state().lingue.ta(key, &[("fs", fs.into())]);
        assert!(shows(&h, &banner), "banner {key}");
    }
}

#[test]
fn windows_e_sistema_cestino_in_due_modi_e_mai_zero_senza_amministratore() {
    // F-SY-2: il Cestino con le due misure (secondo Windows, su disco) e le cartelle orfane come anomalia.
    // F-SY-1: copie shadow e archiviazione riservata "non disponibile" con il motivo, mai 0.
    let mut h = loaded();
    h.state_mut().go(Screen::System);
    h.run_steps(4);
    let a = h.state();
    let (l, f) = (&a.lingue, &a.fmt);
    let d = a.data.as_ref().expect("dati");
    let bin = node_named(&h, "S-1-5-21-1-2-3-1001");
    let windows = l.ta("sistema-cestino-windows", &[("valore", f.size(1_318_890).into()), ("n", 1u64.into()), ("testo", f.count(1).into())]);
    for s in [l.t("sistema-cestino-secondo-windows"), windows, l.t("sistema-cestino-su-disco"), f.size(d.tree().alloc(bin)), l.t("sistema-cestino-orfane")] {
        assert!(shows(&h, &s), "Cestino: manca «{s}»");
    }
    let orphans = d.cls.matches.iter().find(|m| d.kb.rules[m.rule].id == "cestino-orfane").expect("orfane");
    let line = kbtext::rule_line(l, f, &d.kb, orphans);
    assert!(shows(&h, &line), "anomalia: {line}");
    // Le due misure sono diverse: 9,6 GB su disco, 1,25 MB secondo Windows.
    assert!(d.tree().alloc(bin) > 1000 * 1_318_890);
    let unavailable = l.t("sistema-non-disponibile-admin");
    assert_eq!(drawn(&h).iter().filter(|t| **t == unavailable).count(), 2, "copie shadow e archiviazione riservata");
    for key in ["sistema-voce-copie-shadow", "sistema-voce-archiviazione-riservata"] {
        assert!(shows(&h, &l.t(key)), "voce {key}");
    }
    assert!(!shows(&h, &f.size(0)), "nessuna voce scritta come 0");
}
