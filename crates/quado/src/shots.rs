// SPDX-License-Identifier: GPL-3.0-or-later
//! `quado --immagini DIR`: scansiona il volume di sistema e salva le schermate con i dati veri, in
//! tema chiaro e scuro (revisioni di M2, M3, M4 e M5). La lente Download si apre senza letture di
//! contenuto. L'ultima immagine è il dialogo di conferma dello spostamento delle versioni precedenti di
//! CapCut: la simulazione legge solo attributi, Restart Manager e riferimenti, e niente si conferma.
//! Le immagini restano sul PC: contengono nomi di file.

use std::path::PathBuf;

use eframe::egui;
use quado_ui::app::data::VolumeInfo;
use quado_ui::app::{App, Screen};
use quado_ui::tokens::ThemeChoice;

#[derive(Clone, Copy)]
enum Step {
    Scan,
    Shot { k: usize, wait: u32, asked: bool },
    Done,
}

pub struct Shots {
    out: PathBuf,
    step: Step,
    pub finished: bool,
}

/// (nome del file, schermata, tema, preparazione). Le immagini sono in italiano, tranne l'ultima.
const PLAN: &[(&str, Screen, ThemeChoice, u8)] = &[
    ("panoramica-chiaro", Screen::Overview, ThemeChoice::Light, 0),
    ("panoramica-scuro", Screen::Overview, ThemeChoice::Dark, 0),
    ("esplora-radice-chiaro", Screen::Explore, ThemeChoice::Light, 0),
    ("esplora-local-chiaro", Screen::Explore, ThemeChoice::Light, 1),
    ("esplora-local-scuro", Screen::Explore, ThemeChoice::Dark, 1),
    ("file-grandi-chiaro", Screen::BigFiles, ThemeChoice::Light, 0),
    ("ricerca-chiaro", Screen::Explore, ThemeChoice::Light, 2),
    ("impostazioni-scuro", Screen::Settings, ThemeChoice::Dark, 0),
    ("esplora-capcut-chiaro", Screen::Explore, ThemeChoice::Light, 4),
    ("sistema-chiaro", Screen::System, ThemeChoice::Light, 0),
    ("regole-chiaro", Screen::Rules, ThemeChoice::Light, 5),
    ("download-chiaro", Screen::Downloads, ThemeChoice::Light, 0),
    ("programmi-chiaro", Screen::Programs, ThemeChoice::Light, 6),
    ("programmi-residui-chiaro", Screen::Programs, ThemeChoice::Light, 7),
    ("overview-en-chiaro", Screen::Overview, ThemeChoice::Light, 3),
    ("intervento-capcut-chiaro", Screen::Explore, ThemeChoice::Light, 8),
];

impl Shots {
    pub fn new(out: PathBuf, app: &mut App) -> Self {
        app.set_language("it");
        if let Some(v) = app.volumes.iter().find(|v| v.is_system).and_then(VolumeInfo::from_volume) {
            app.start_scan(v);
        }
        Self { out, step: Step::Scan, finished: false }
    }

    fn prepare(app: &mut App, k: usize) {
        let (_, screen, theme, prep) = PLAN[k];
        app.settings.theme = theme;
        app.search.clear();
        // Regola delle versioni precedenti di CapCut, per il dialogo di conferma (preparazione 8).
        let mut capcut: Option<usize> = None;
        if let Some(d) = app.data.as_ref() {
            match prep {
                1 => {
                    let t = d.tree();
                    // La cartella Local più grande (quella del profilo in uso, non di Default).
                    let local = (0..t.len() as u32)
                        .filter(|&i| t.name_lossy(i).eq_ignore_ascii_case("Local") && t.name_lossy(t.parent(i)).eq_ignore_ascii_case("AppData"))
                        .max_by_key(|&i| t.alloc(i));
                    if let Some(l) = local {
                        let first = d.ord().children(l).first().copied();
                        app.explore.open(d, l, first);
                    }
                }
                2 => app.search.set_query("capcut"),
                3 => app.set_language("en"),
                4 => {
                    // CapCut\Apps con la prima versione precedente selezionata: record Da verificare.
                    let first = d.cls.matches.iter().find(|m| d.kb.rules[m.rule].id == "capcut-versioni").and_then(|m| m.nodes.first().copied());
                    if let Some(v) = first {
                        let apps = d.tree().parent(v);
                        app.explore.open(d, apps, Some(v));
                    }
                }
                5 => {
                    app.rules_view.selected = app.kb.index("capcut-versioni");
                    app.rules_view.scroll_to = true;
                }
                6 => {
                    // CapCut: programma con tuoi file nella cartella (F-AP-5) e dati dalle regole.
                    app.programs.section = quado_ui::app::programs_view::Section::Programs;
                    app.programs.selected = d.programs.items.iter().position(|p| p.name.eq_ignore_ascii_case("CapCut"));
                }
                7 => {
                    app.programs.section = quado_ui::app::programs_view::Section::Residues;
                    app.programs.selected_residue = (!d.programs.residues.is_empty()).then_some(0);
                }
                8 => {
                    capcut = d.cls.matches.iter().position(|m| d.kb.rules[m.rule].id == "capcut-versioni");
                    if let Some(v) = capcut.and_then(|k| d.cls.matches[k].nodes.first().copied()) {
                        let apps = d.tree().parent(v);
                        app.explore.open(d, apps, Some(v));
                    }
                }
                _ => {}
            }
        }
        if prep == 8 {
            // Dialogo di conferma per le versioni precedenti di CapCut: solo la simulazione, in sola
            // lettura; il dialogo resta aperto e l'app si chiude dopo l'immagine.
            app.set_language("it");
            if let Some(k) = capcut {
                quado_ui::app::intervention_view::open(app, quado_ui::app::intervention_jobs::Origin::Rule(k));
            }
        }
        app.go(screen);
    }

    /// Chiamata a ogni frame, prima di `App::show`.
    pub fn frame(&mut self, app: &mut App, ctx: &egui::Context) {
        ctx.request_repaint();
        self.save_received(ctx);
        self.step = match self.step {
            Step::Scan if app.job.is_none() && app.data.is_some() => {
                Self::prepare(app, 0);
                Step::Shot { k: 0, wait: 0, asked: false }
            }
            Step::Scan => Step::Scan,
            Step::Shot { k, wait, asked: false } => {
                let busy = app.search.pending() || app.big.pending() || app.intervention.simulating();
                // I 30 frame si contano da quando niente è più in corso: il dialogo di conferma compare
                // con una dissolvenza alla fine della simulazione.
                if busy {
                    Step::Shot { k, wait: 0, asked: false }
                } else if wait < 30 {
                    Step::Shot { k, wait: wait + 1, asked: false }
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(PLAN[k].0.to_owned())));
                    Step::Shot { k, wait: 0, asked: true }
                }
            }
            Step::Shot { k, wait, asked: true } => {
                // La schermata arriva qualche frame dopo; poi si passa alla successiva.
                if wait < 6 {
                    Step::Shot { k, wait: wait + 1, asked: true }
                } else if k + 1 < PLAN.len() {
                    Self::prepare(app, k + 1);
                    Step::Shot { k: k + 1, wait: 0, asked: false }
                } else {
                    Step::Done
                }
            }
            Step::Done => {
                self.finished = true;
                Step::Done
            }
        };
    }

    fn save_received(&self, ctx: &egui::Context) {
        ctx.input(|i| {
            for e in &i.raw.events {
                if let egui::Event::Screenshot { user_data, image, .. } = e
                    && let Some(name) = user_data.data.as_ref().and_then(|d| d.downcast_ref::<String>())
                {
                    let _ = std::fs::create_dir_all(&self.out);
                    let path = self.out.join(format!("{name}.png"));
                    let [w, h] = image.size;
                    match image::save_buffer(&path, image.as_raw(), w as u32, h as u32, image::ColorType::Rgba8) {
                        Ok(()) => eprintln!("immagine: {}", path.display()),
                        Err(e) => eprintln!("immagine {name} non salvata: {e}"),
                    }
                }
            }
        });
    }
}
