// SPDX-License-Identifier: GPL-3.0-or-later
//! Ispettore, variante A: testata con nome, tipo, la cifra grande e leggera e i fatti misurati; poi "Cosa
//! puoi fare" per primo, dal record di raccomandazione e con l'azione successiva; poi le sezioni apribili
//! "Cos'è" con gestore e fatti ciascuno con la sua provenienza, "Perché è così grande" con parti, schemi
//! riconosciuti dalle regole, tipi secondo l'estensione ed età, la regola con data e "Apri la regola"; in
//! fondo le azioni sempre disponibili. Niente card dentro card: l'ispettore sta sul fondo della sua colonna e
//! l'unico riquadro è quello di "Cosa puoi fare".

use std::sync::Arc;
use std::time::Duration;

use egui::{CornerRadius, Rect, ScrollArea, Sense, Ui, pos2, vec2};
use quado_actions::shell;
use quado_model::{Family, Flags, NONE, Tree};
use quado_scan_win::Problem;

use quado_model::record::{Estimate, Record, State, UnknownReason};

use super::data::{self, Model, Task, Verified};
use super::kbtext;
use super::{App, Screen, volume_title};
use crate::i18n::Lingue;
use crate::icons::Icon;
use crate::tokens::{Gestore, Glyph, Palette, Type};
use crate::widgets::{self, ButtonKind, Evidence, mono, sans};

/// Misure di una cartella calcolate in background: si percorre tutto il sottoalbero.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Info {
    pub dirs: u64,
    /// (famiglia, allocata, file), in ordine di allocata.
    pub families: Vec<(Family, u64, u64)>,
    /// (allocata, file) per età dell'ultima modifica: 30 giorni, un anno, tre anni, oltre.
    pub ages: [(u64, u64); 4],
}

pub fn compute_info(m: &Model, node: u32, now: i64) -> Info {
    const DAY: i64 = 864_000_000_000;
    let mut ages = [(0u64, 0u64); 4];
    let (t, ord) = (&m.tree, &m.ord);
    let mut dirs = 0u64;
    let mut fam: std::collections::HashMap<Family, (u64, u64)> = std::collections::HashMap::new();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        for &k in ord.children(n) {
            if t.is_dir(k) {
                dirs += 1;
                stack.push(k);
            } else if !t.flags(k).has(Flags::SECONDARY_LINK) {
                let e = fam.entry(Family::of_name(t.name(k))).or_default();
                e.0 += t.alloc(k);
                e.1 += 1;
                let age = now - t.mtime(k);
                let b = if age <= 30 * DAY { 0 } else if age <= 365 * DAY { 1 } else if age <= 3 * 365 * DAY { 2 } else { 3 };
                ages[b].0 += t.alloc(k);
                ages[b].1 += 1;
            }
        }
    }
    let mut families: Vec<(Family, u64, u64)> = fam.into_iter().map(|(f, (a, c))| (f, a, c)).collect();
    families.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Info { dirs, families, ages }
}

/// Sezioni dell'ispettore aperte o chiuse: la scelta resta per tutte le voci finché l'app è aperta.
#[derive(Clone, Debug, Default)]
pub struct Sections(std::collections::HashMap<&'static str, bool>);

impl Sections {
    /// Sezioni chiuse finché l'utente non le apre: le misure lunghe da leggere e la scheda completa.
    const CLOSED: [&'static str; 3] = ["tipi", "eta", "scheda"];

    pub fn is_open(&self, key: &'static str) -> bool {
        self.0.get(key).copied().unwrap_or(!Self::CLOSED.contains(&key))
    }

    pub fn set(&mut self, key: &'static str, open: bool) {
        self.0.insert(key, open);
    }
}

#[derive(Default)]
pub struct InspectorState {
    info: Option<(u32, u64, Info)>,
    info_task: Option<Task<(u32, u64, Info)>>,
    pub verify: Option<Task<Verified>>,
    pub verifying: Option<u32>,
    pub sections: Sections,
}

impl InspectorState {
    /// Raccoglie i lavori finiti; restituisce una verifica da applicare al modello.
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<Verified> {
        if let Some(t) = self.info_task.as_mut() {
            ctx.request_repaint_after(Duration::from_millis(30));
            if let Some(i) = t.poll() {
                self.info = Some(i);
                self.info_task = None;
            }
        }
        if let Some(t) = self.verify.as_mut() {
            ctx.request_repaint_after(Duration::from_millis(50));
            if let Some(v) = t.poll() {
                self.verify = None;
                self.verifying = None;
                return Some(v);
            }
        }
        None
    }

    pub fn wait(&mut self) {
        if let Some(t) = self.info_task.take() {
            self.info = t.wait();
        }
    }

    fn info_for(&mut self, model: &Arc<Model>, node: u32, generation: u64) -> Option<&Info> {
        let fresh = self.info.as_ref().is_some_and(|(n, g, _)| *n == node && *g == generation);
        if !fresh && self.info_task.is_none() {
            let m = Arc::clone(model);
            let now = quado_scan_win::system::now_filetime();
            self.info_task = Some(Task::spawn(move || {
                let i = compute_info(&m, node, now);
                drop(m);
                (node, generation, i)
            }));
        }
        self.info.as_ref().filter(|(n, g, _)| *n == node && *g == generation).map(|(_, _, i)| i)
    }
}

/// Un fatto: glifo della provenienza (con l'etichetta nel suggerimento) e testo a capo.
pub fn fact(ui: &mut Ui, p: &Palette, e: Evidence, s: &str, l: &Lingue) {
    fact_in(ui, p, e, s, l, p.text_2);
}

/// Un fatto dell'ispettore (`.fact`): come [`fact`], nel testo principale e nel ruolo Callout.
pub(crate) fn fact_in(ui: &mut Ui, p: &Palette, e: Evidence, s: &str, l: &Lingue, color: egui::Color32) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, resp) = ui.allocate_exact_size(vec2(12.0, 18.0), Sense::hover());
        widgets::evidence_glyph(ui.painter(), pos2(r.center().x, r.min.y + 9.0), e, p);
        resp.on_hover_text(l.t(e.key()));
        widgets::paragraph(ui, s, Type::Callout, color);
    });
}

/// Riga con il glifo di una forma dei dati (gestore) al posto della provenienza.
fn glyph_line(ui: &mut Ui, p: &Palette, shape: Glyph, color: egui::Color32, s: &str) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (r, _) = ui.allocate_exact_size(vec2(12.0, 18.0), Sense::hover());
        widgets::glyph(ui.painter(), pos2(r.center().x, r.min.y + 9.0), 9.0, shape, color, p);
        widgets::paragraph(ui, s, Type::Callout, p.text);
    });
}

/// Sezione apribile dell'ispettore (`.insp section`): titolo con la freccia, poi il contenuto se aperta.
pub(crate) fn section(ui: &mut Ui, p: &Palette, sections: &mut Sections, key: &'static str, title: &str, body: impl FnOnce(&mut Ui)) {
    ui.add_space(14.0);
    let mut open = sections.is_open(key);
    if ui.push_id(("sezione", key), |ui| widgets::disclosure(ui, p, title, Type::BodyStrong, &mut open)).inner.changed() {
        sections.set(key, open);
    }
    if open {
        ui.add_space(4.0);
        ui.push_id(("contenuto", key), body);
    }
}

/// Pulsante largo quanto la colonna (`.todo .btn`, `.insp .acts .btn`): fondo `field`, icona e testo a
/// sinistra o al centro. Un testo che non entra va a capo e il pulsante si allunga (testi tradotti più lunghi).
pub(crate) fn wide_button(ui: &mut Ui, p: &Palette, label: &str, icon: Option<Icon>, width: f32, centered: bool, enabled: bool) -> egui::Response {
    let color = if enabled { p.text } else { p.disabled };
    let iw = if icon.is_some() { 23.0 } else { 0.0 };
    let g = ui.painter().layout(label.to_owned(), widgets::medium(13.0), color, (width - 24.0 - iw).max(40.0));
    let h = crate::tokens::metrics(ui.ctx()).control.max(g.size().y + 14.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, h), if enabled { Sense::click() } else { Sense::hover() });
    let fill = if !enabled {
        p.field
    } else if resp.is_pointer_button_down_on() {
        p.select
    } else if resp.hovered() {
        p.hover
    } else {
        p.field
    };
    ui.painter().rect_filled(rect, CornerRadius::same(crate::tokens::radius::CONTROL), fill);
    let x = if centered { rect.center().x - (g.size().x + iw) / 2.0 } else { rect.min.x + 12.0 };
    let first = g.rows.first().map_or(g.size().y, |r| r.height());
    let top = rect.center().y - g.size().y / 2.0;
    if let Some(i) = icon {
        crate::icons::paint(ui.painter(), Rect::from_min_size(pos2(x, top + first / 2.0 - 8.0), vec2(16.0, 16.0)), i, color);
    }
    ui.painter().galley(pos2(x + iw, top), g, color);
    widgets::focus_ring(ui, &resp, p, crate::tokens::radius::CONTROL);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    resp
}

/// Motivo di una cartella non leggibile. L'accesso negato dipende da com'è avviato Quado (`elevated`): da
/// amministratore la stessa frase sarebbe falsa.
fn reason(l: &Lingue, pr: Problem, elevated: bool) -> String {
    match pr {
        Problem::AccessDenied => l.t(if elevated { "motivo-accesso-negato-amministratore" } else { "motivo-accesso-negato" }),
        Problem::InUse => l.t("motivo-in-uso"),
        Problem::NotFound => l.t("motivo-non-trovata"),
        Problem::Interrupted(c) => l.ta("motivo-interrotta", &[("codice", c.to_string().into())]),
        Problem::Cancelled => l.t("motivo-annullata"),
        Problem::Other(c) => l.ta("motivo-altro", &[("codice", c.to_string().into())]),
    }
}

/// Tipo dell'elemento: "Cartella", "File MP4 · Video, secondo l'estensione", "Unità".
pub fn kind_line(l: &Lingue, t: &Tree, n: u32) -> String {
    if n == Tree::ROOT {
        return l.t("tipo-unita");
    }
    let f = t.flags(n);
    if f.has(Flags::NOT_FOLLOWED) {
        return l.t("tipo-collegamento");
    }
    if t.is_dir(n) {
        return l.t("tipo-cartella");
    }
    let ext = quado_model::kind::extension(t.name(n));
    let fam = Family::of_name(t.name(n));
    let fam_name = l.t(&format!("famiglia-{}", fam.code()));
    if ext.is_empty() {
        l.ta("tipo-file-senza-estensione", &[("famiglia", fam_name.into())])
    } else {
        l.ta("tipo-file", &[("estensione", ext.to_uppercase().into()), ("famiglia", fam_name.into())])
    }
}

pub fn show(app: &mut App, ui: &mut Ui, p: &Palette, node: Option<u32>) {
    // Nessuna card: l'ispettore sta sul fondo della sua colonna (`.insp`), i riquadri sono dentro.
    let Some(node) = node else {
        widgets::paragraph(ui, app.lingue.t("ispettore-nessuna-selezione"), Type::Body, p.text_2);
        return;
    };
    let Some(d) = app.data.as_ref() else { return };
    if node as usize >= d.tree().len() || d.tree().flags(node).has(Flags::DETACHED) {
        return;
    }
    let model = Arc::clone(&d.model);
    let generation = d.generation;
    let info = if d.tree().is_dir(node) { app.inspector.info_for(&model, node, generation).cloned() } else { None };
    let mut action: Option<&'static str> = None;
    let mut req: Option<KbRequest> = None;
    let mut sections = app.inspector.sections.clone();
    let busy = app.inspector.verifying.is_some();
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let (l, f) = (&app.lingue, &app.fmt);
        let t = d.tree();
        let w = ui.available_width();
        // Testata: nome, tipo, la cifra grande e leggera, conteggi e quota, poi i fatti misurati.
        let title = if node == Tree::ROOT { volume_title(l, &d.volume) } else { t.name_lossy(node) };
        widgets::label_elided(ui, &title, widgets::font(Type::Title), p.text, w);
        widgets::paragraph(ui, kind_line(l, t, node), Type::Callout, p.text_2);
        ui.add_space(10.0);
        let un = super::explore::unmeasured(t, node);
        if un {
            widgets::typed(ui, l.t("ispettore-non-leggibile"), Type::Title, p.text_2);
        } else {
            let s = f.size(t.alloc(node));
            ui.add(egui::Label::new(egui::RichText::new(s).font(widgets::light(34.0)).color(p.text).extra_letter_spacing(-0.8)));
        }
        let mut meta = Vec::new();
        if t.is_dir(node) {
            let files = u64::from(t.files(node));
            meta.push(l.ta("conteggio-file", &[("n", files.into()), ("testo", f.count(files).into())]));
            if let Some(i) = &info {
                meta.push(l.ta("conteggio-cartelle", &[("n", i.dirs.into()), ("testo", f.count(i.dirs).into())]));
            }
        }
        let share = t.alloc(node) as f64 / d.used() as f64;
        meta.push(l.ta("ispettore-quota-usato", &[("quota", f.percent(share).into())]));
        widgets::paragraph(ui, meta.join(" · "), Type::Callout, p.text_2);

        // Fatti, ciascuno con la sua provenienza.
        ui.add_space(12.0);
        let fl = t.flags(node);
        let logical = t.logical(node);
        let alloc = t.alloc(node);
        let fact = |ui: &mut Ui, e: Evidence, s: &str| fact_in(ui, p, e, s, l, p.text);
        if !un {
            fact(ui, Evidence::Measured, &l.ta("fatto-dimensioni", &[("dimensione", f.size(logical).into()), ("byte", f.count(logical).into())]));
            fact(ui, Evidence::Measured, &l.ta("fatto-dimensioni-su-disco", &[("dimensione", f.size(alloc).into()), ("byte", f.count(alloc).into())]));
        }
        if node != Tree::ROOT && t.mtime(node) > 0 {
            fact(ui, Evidence::Measured, &l.ta("fatto-ultima-modifica", &[("data", f.date(t.mtime(node)).into()), ("ora", f.time(t.mtime(node)).into())]));
        }
        if let Some(pr) = d.problem(node).filter(|_| fl.any(Flags::DENIED | Flags::ERROR | Flags::PARTIAL)) {
            fact(ui, Evidence::Unavailable, &l.ta("fatto-non-leggibile", &[("motivo", reason(l, pr, app.env.elevated).into())]));
        }
        if fl.has(Flags::CLOUD_NOT_LISTED) {
            fact(ui, Evidence::Unavailable, &l.t("fatto-cloud-non-elencata"));
        } else if fl.has(Flags::CLOUD) && !t.is_dir(node) {
            fact(ui, Evidence::Measured, &l.ta("fatto-cloud", &[("nel-cloud", f.size(logical).into()), ("locale", f.size(alloc).into())]));
        }
        if fl.has(Flags::NOT_FOLLOWED) {
            fact(ui, Evidence::Measured, &l.t("fatto-collegamento"));
        }
        if fl.has(Flags::MULTI_LINK) {
            fact(ui, Evidence::Measured, &l.t(if fl.has(Flags::SECONDARY_LINK) { "fatto-hard-link-secondario" } else { "fatto-hard-link" }));
        }
        if fl.has(Flags::ALLOC_FIXED) {
            fact(ui, Evidence::Measured, &l.t("fatto-allocata-corretta"));
        }
        let attrs = t.attrs(node);
        let mut tags = Vec::new();
        for (bit, key) in [(0x800, "attributo-compresso"), (0x200, "attributo-sparse"), (0x4000, "attributo-cifrato"), (0x2, "attributo-nascosto"), (0x4, "attributo-sistema")] {
            if attrs & bit != 0 {
                tags.push(l.t(key));
            }
        }
        if !tags.is_empty() && node != Tree::ROOT {
            fact(ui, Evidence::Measured, &l.ta("fatto-attributi", &[("elenco", tags.join(", ").into())]));
        }

        // Cosa puoi fare, per primo: dal record, con l'azione successiva.
        let mut details = sections.is_open("scheda");
        section(ui, p, &mut sections, "cosa", &l.t("ispettore-cosa-puoi-fare"), |ui| what_you_can_do(ui, p, l, f, d, node, &mut req, &mut details));
        sections.set("scheda", details);

        // Cos'è: gestore e fatti dalla regola.
        section(ui, p, &mut sections, "cose", &l.t("ispettore-cos-e"), |ui| what_it_is(ui, p, l, f, d, node, &mut req));

        // Perché è così grande: parti, schemi delle regole, tipi ed età.
        if t.is_dir(node) && d.ord().has_children(node) {
            section(ui, p, &mut sections, "parti", &l.t("ispettore-parti-principali"), |ui| {
                let kids: Vec<u32> = d.ord().children(node).iter().copied().take(5).collect();
                let max = kids.first().map_or(1, |&k| t.alloc(k)).max(1);
                for k in kids {
                    let g = kbtext::node_gestore(&d.cls, k);
                    part_row(ui, p, f, &t.name_lossy(k), None, t.alloc(k), max, Some((g.shape(), p.data_color(g))), true);
                }
            });
            let pats = patterns(d, node);
            if !pats.is_empty() {
                section(ui, p, &mut sections, "schemi", &l.t("ispettore-schemi"), |ui| patterns_rows(ui, p, l, f, d, node, &pats));
            }
            section(ui, p, &mut sections, "tipi", &l.t("ispettore-tipi-di-file"), |ui| match &info {
                Some(i) if !i.families.is_empty() => {
                    for &(fam, a, c) in i.families.iter().take(6) {
                        let name = l.t(&format!("famiglia-{}", fam.code()));
                        let count = l.ta("conteggio-file", &[("n", c.into()), ("testo", f.count(c).into())]);
                        part_row(ui, p, f, &name, Some(&count), a, t.alloc(node), None, false);
                    }
                    ui.add_space(4.0);
                    widgets::paragraph(ui, l.t("ispettore-secondo-estensione"), Type::Callout, p.text_2);
                }
                Some(_) => {
                    widgets::paragraph(ui, l.t("ispettore-nessun-file"), Type::Callout, p.text_2);
                }
                None => {
                    widgets::paragraph(ui, l.t("ispettore-calcolo"), Type::Callout, p.text_2);
                }
            });
            if let Some(i) = info.as_ref().filter(|i| i.ages.iter().any(|&(a, _)| a > 0)) {
                section(ui, p, &mut sections, "eta", &l.t("ispettore-eta"), |ui| ages(ui, p, l, f, i, t.alloc(node)));
            }
        }

        // La regola, con versione e data di verifica.
        if let Some(m) = d.cls.own(node).map(|k| &d.cls.matches[k]) {
            section(ui, p, &mut sections, "regola", &l.t("ispettore-regola"), |ui| rule_block(ui, p, l, f, d, m, &mut req));
        }

        // Azioni sempre disponibili, in due colonne (`.insp .acts`). Id stabili: le misure in background che
        // arrivano sopra non devono cambiare l'identità dei pulsanti tra un frame e l'altro.
        ui.add_space(18.0);
        widgets::typed(ui, l.t("ispettore-azioni"), Type::BodyStrong, p.text);
        ui.add_space(8.0);
        ui.push_id("ispettore_azioni", |ui| {
            let gap = 6.0;
            let col = ((ui.available_width() - gap) / 2.0).floor();
            let mut buttons: Vec<(&'static str, Icon, String, Option<String>, bool)> = vec![
                ("reveal", Icon::External, l.t("azione-apri-in-esplora"), None, true),
                ("copy", Icon::Copy, l.t("azione-copia-percorso"), None, true),
                ("properties", Icon::Properties, l.t("azione-proprieta"), Some(l.t("azione-proprieta-suggerimento")), true),
                ("verify", Icon::Refresh, l.t("azione-verifica"), Some(l.t("azione-verifica-suggerimento")), !busy && !fl.has(Flags::NOT_FOLLOWED)),
            ];
            // Vista Download su qualunque cartella dell'utente.
            if in_profile(d, node) {
                buttons.push(("lens", Icon::Download, l.t("download-apri-lente"), Some(l.t("download-apri-lente-suggerimento")), true));
            }
            // Due colonne se ogni nome ci sta su una riga, se no una (testi tradotti più lunghi).
            let fits = buttons.iter().all(|b| ui.painter().layout_no_wrap(b.2.clone(), widgets::medium(13.0), p.text).size().x + 24.0 + 23.0 <= col);
            let (per_row, col) = if fits { (2, col) } else { (1, ui.available_width()) };
            for pair in buttons.chunks(per_row) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (key, icon, label, tip, enabled) in pair {
                        let r = wide_button(ui, p, label, Some(*icon), col, false, *enabled);
                        let r = match tip {
                            Some(s) => widgets::tip(r, p, s, None),
                            None => r,
                        };
                        if r.clicked() {
                            action = Some(key);
                        }
                    }
                });
            }
        });
        if app.inspector.verifying == Some(node) {
            ui.add_space(6.0);
            widgets::paragraph(ui, l.t("verifica-in-corso"), Type::Callout, p.text_2);
        }
        if let Some(v) = &app.last_verify
            && v.node == node
        {
            let s = match (v.after, v.error) {
                (_, Some(code)) => l.ta("verifica-errore", &[("codice", code.to_string().into())]),
                (Some(a), None) if a == v.before => l.ta("verifica-invariata", &[("ora", f.time(v.at).into())]),
                (Some(a), None) => l.ta("verifica-cambiata", &[("ora", f.time(v.at).into()), ("prima", f.size(v.before).into()), ("dopo", f.size(a).into())]),
                (None, None) => String::new(),
            };
            ui.add_space(6.0);
            widgets::paragraph(ui, s, Type::Callout, p.text_2);
        }
        ui.add_space(12.0);
    });
    app.inspector.sections = sections;
    match req {
        Some(KbRequest::Select(n)) => {
            super::explore::show_in_folder(app, n);
            return;
        }
        Some(KbRequest::Rule(r)) => {
            app.rules_view.selected = Some(r);
            app.rules_view.scroll_to = true;
            app.go(Screen::Rules);
            return;
        }
        Some(KbRequest::Copy(c)) => {
            ui.ctx().copy_text(c);
            app.notice.set(app.lingue.t("avviso-comando-copiato"));
            return;
        }
        Some(KbRequest::Act(a)) => {
            // L'elemento riconosciuto da una regola che possiede il nodo (lo stesso di "Cosa puoi fare").
            if let Some(m) = d.cls.own(node) {
                super::intervention_jobs::dispatch(app, a, super::intervention_jobs::Origin::Rule(m));
            }
            return;
        }
        None => {}
    }
    let Some(a) = action else { return };
    let path = d.tree().path_lossy(node);
    let is_dir = d.tree().is_dir(node);
    match a {
        "reveal" => {
            if let Err(e) = shell::reveal(&path) {
                app.notice.set(app.lingue.ta("avviso-shell-errore", &[("codice", format!("0x{:08X}", e.0 as u32).into())]));
            }
        }
        "copy" => {
            ui.ctx().copy_text(path);
            app.notice.set(app.lingue.t("avviso-percorso-copiato"));
        }
        "properties" => {
            if let Err(e) = shell::properties(&path) {
                app.notice.set(app.lingue.ta("avviso-shell-errore", &[("codice", format!("0x{:08X}", e.0 as u32).into())]));
            }
        }
        "lens" => {
            app.downloads.choose(Some(path));
            app.go(Screen::Downloads);
        }
        "verify" => {
            if node == Tree::ROOT || d.tree().parent(node) == NONE {
                // La radice si verifica con una nuova scansione completa.
                let v = d.volume.clone();
                app.start_scan(v);
            } else {
                app.inspector.verifying = Some(node);
                app.inspector.verify = Some(data::spawn_verify(path, node, is_dir));
            }
        }
        _ => {}
    }
}

/// Cartella dentro il profilo dell'utente (non il profilo stesso): la lente Download vale su qualunque
/// cartella dell'utente.
pub fn in_profile(d: &data::Loaded, node: u32) -> bool {
    let t = d.tree();
    let Some(profile) = d.evidence.folders.profile.as_deref().and_then(|p| quado_kb::engine::find(t, p)) else { return false };
    node != profile && t.is_dir(node) && !t.flags(node).has(Flags::NOT_FOLLOWED) && quado_model::view::is_ancestor(t, profile, node)
}

/// Riga di una ripartizione (`.kvr`): forma del gestore se c'è, nome (a capo se è un testo tradotto,
/// troncato se è un nome di file), eventuale seconda riga, barretta della quota rispetto a `total` e
/// dimensione. Senza forma la barretta è neutra.
#[allow(clippy::too_many_arguments)]
fn part_row(ui: &mut Ui, p: &Palette, f: &crate::format::Formatter, name: &str, sub: Option<&str>, size: u64, total: u64, glyph: Option<(Glyph, egui::Color32)>, data: bool) {
    let w = ui.available_width();
    let painter = ui.painter().clone();
    let sg = painter.layout_no_wrap(f.size(size), widgets::num(12.0), p.text);
    let sw = sg.size().x;
    let x0 = if glyph.is_some() { 20.0 } else { 0.0 };
    let bar_w = 56.0;
    let name_w = (w - x0 - sw - bar_w - 20.0).max(60.0);
    let font = widgets::font(Type::Callout);
    let g = if data { widgets::elided(&painter, name.to_owned(), font.clone(), p.text, name_w) } else { painter.layout(name.to_owned(), font.clone(), p.text, name_w) };
    let sub_g = sub.map(|s| painter.layout(s.to_owned(), font.clone(), p.text_2, name_w));
    let h = 10.0 + g.size().y + sub_g.as_ref().map_or(0.0, |s| s.size().y + 1.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h.max(26.0)), Sense::hover());
    let top = rect.min.y + 5.0;
    let line_h = g.rows.first().map_or(g.size().y, |r| r.height());
    let cy = top + line_h / 2.0;
    if let Some((shape, color)) = glyph {
        widgets::glyph(&painter, pos2(rect.min.x + 5.0, cy), 9.0, shape, color, p);
    }
    painter.galley(pos2(rect.max.x - sw, cy - sg.size().y / 2.0), sg, p.text);
    let elided = g.elided;
    let gh = g.size().y;
    painter.galley(pos2(rect.min.x + x0, top), g, p.text);
    if let Some(s) = sub_g {
        painter.galley(pos2(rect.min.x + x0, top + gh + 1.0), s, p.text_2);
    }
    let bar = Rect::from_min_size(pos2(rect.max.x - sw - 12.0 - bar_w, cy - 2.0), vec2(bar_w, 4.0));
    let fill = glyph.map_or(p.text_2, |(_, c)| c);
    widgets::share_bar(&painter, bar, size as f64 / total.max(1) as f64, fill, p.track);
    if elided {
        resp.on_hover_text(name);
    }
}

/// Cosa chiede l'ispettore all'app dopo il frame.
pub enum KbRequest {
    /// Seleziona l'elemento riconosciuto che contiene il nodo.
    Select(u32),
    /// Apre l'elenco delle regole su questa regola.
    Rule(usize),
    /// Copia un comando o un URI negli appunti.
    Copy(String),
    /// Un pulsante degli interventi: spostamento nel Cestino o procedura.
    Act(super::intervention_jobs::ActRequest),
}

/// "Cos'è": gestore, programma e versione quando noti, fatti con la loro provenienza.
fn what_it_is(ui: &mut Ui, p: &Palette, l: &Lingue, f: &crate::format::Formatter, d: &data::Loaded, node: u32, req: &mut Option<KbRequest>) {
    let cls = &d.cls;
    let Some(mk) = cls.match_of(node) else {
        gestore_line(ui, p, l, Gestore::Unknown, false);
        fact_in(ui, p, Evidence::Unavailable, &l.t("ispettore-nessuna-regola"), l, p.text);
        return;
    };
    let m = &cls.matches[mk];
    gestore_line(ui, p, l, kbtext::manager_gestore(m.manager), kbtext::is_hypothesis(m));
    let own = cls.own(node) == Some(mk);
    if !own {
        // Il nodo sta dentro un elemento riconosciuto: la spiegazione è quella dell'elemento.
        let anchor = m.nodes.iter().copied().find(|&a| quado_model::view::is_ancestor(d.tree(), a, node)).unwrap_or(m.nodes[0]);
        // Due righe: la riga della regola ha spesso già i suoi due punti ("Cache di CapCut: la svuoti…").
        let s = l.ta("ispettore-fa-parte-di", &[("nome", d.tree().name_lossy(anchor).into())]);
        widgets::paragraph(ui, s, Type::Callout, p.text_2);
        widgets::paragraph(ui, kbtext::rule_line(l, f, &d.kb, m), Type::Callout, p.text_2);
        ui.add_space(4.0);
        if widgets::button(ui, p, &l.t("ispettore-vai-elemento"), Some(Icon::ArrowUp), ButtonKind::Secondary, true).clicked() {
            *req = Some(KbRequest::Select(anchor));
        }
        return;
    }
    for fa in &m.record.identity.0 {
        fact_in(ui, p, kbtext::evidence(fa.provenance), &kbtext::fact_with_source(l, f, fa), l, p.text);
    }
    if let Some(proof) = m.record.state.manageability() {
        ui.add_space(6.0);
        widgets::typed(ui, l.t("ispettore-prova-gestibilita"), Type::Caption, p.text_2);
        for fa in proof.facts() {
            fact_in(ui, p, kbtext::evidence(fa.provenance), &kbtext::fact_with_source(l, f, fa), l, p.text);
        }
    }
    ui.add_space(2.0);
    widgets::paragraph(ui, kbtext::reliability_label(l, m.record.reliability), Type::Callout, p.text_2);
}

/// "Gestito da" con la forma del gestore; un'ipotesi ha la forma del non classificato.
fn gestore_line(ui: &mut Ui, p: &Palette, l: &Lingue, g: Gestore, hypothesis: bool) {
    let shown = if hypothesis { Gestore::Unknown } else { g };
    let mut s = l.ta("ispettore-gestito-da", &[("gestore", l.t(g.key()).into())]);
    if hypothesis {
        s.push_str(" · ");
        s.push_str(&l.t("etichetta-ipotesi"));
    }
    glyph_line(ui, p, shown.shape(), p.data_color(shown), &s);
}

/// Schemi riconosciuti dalle regole dentro la cartella, per regola: (regola, corrispondenze, byte), i più
/// grandi per primi.
fn patterns(d: &data::Loaded, node: u32) -> Vec<(usize, Vec<usize>, u64)> {
    let t = d.tree();
    let cls = &d.cls;
    let mut by_rule: Vec<(usize, Vec<usize>, u64)> = Vec::new();
    for (n, mk) in cls.anchors() {
        if n == node || !quado_model::view::is_ancestor(t, node, n) {
            continue;
        }
        let bytes = cls.own_bytes(t, n);
        let rule = cls.matches[mk].rule;
        match by_rule.iter_mut().find(|(r, _, _)| *r == rule) {
            Some(e) => {
                if !e.1.contains(&mk) {
                    e.1.push(mk);
                }
                e.2 += bytes;
            }
            None => by_rule.push((rule, vec![mk], bytes)),
        }
    }
    by_rule.sort_by_key(|e| std::cmp::Reverse(e.2));
    by_rule
}

/// Righe degli schemi: ognuno cita la regola.
fn patterns_rows(ui: &mut Ui, p: &Palette, l: &Lingue, f: &crate::format::Formatter, d: &data::Loaded, node: u32, by_rule: &[(usize, Vec<usize>, u64)]) {
    let cls = &d.cls;
    for (rule, ms, bytes) in by_rule.iter().take(6) {
        let title = if ms.len() == 1 { kbtext::rule_line(l, f, &d.kb, &cls.matches[ms[0]]) } else { l.ta("interventi-gruppo", &[("nome", kbtext::rule_name(l, &d.kb, *rule).into()), ("n", ms.len().into()), ("testo", f.count(ms.len() as u64).into())]) };
        let sub = l.ta("ispettore-schema-regola", &[("regola", kbtext::rule_name(l, &d.kb, *rule).into())]);
        let g = if kbtext::is_hypothesis(&cls.matches[ms[0]]) { Gestore::Unknown } else { kbtext::manager_gestore(cls.matches[ms[0]].manager) };
        part_row(ui, p, f, &title, Some(&sub), *bytes, d.tree().alloc(node), Some((g.shape(), p.data_color(g))), false);
    }
}

/// "Cosa puoi fare", dal record di raccomandazione, in un riquadro (`.todo`): stato, regola,
/// spiegazione, stima, ipotesi e rischi, l'azione successiva e i passi; il resto del record nella scheda
/// apribile. Senza regola: Non sappiamo, solo misure e "Mostra in Esplora file".
#[allow(clippy::too_many_arguments)]
fn what_you_can_do(ui: &mut Ui, p: &Palette, l: &Lingue, f: &crate::format::Formatter, d: &data::Loaded, node: u32, req: &mut Option<KbRequest>, details: &mut bool) {
    let cls = &d.cls;
    let own = cls.own(node).map(|k| &cls.matches[k]);
    egui::Frame::new().fill(p.panel).corner_radius(CornerRadius::same(crate::tokens::radius::GROUP)).shadow(p.shadow(crate::tokens::Elevation::Card)).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 3.0;
        let Some(m) = own else {
            let state = State::Unknown(UnknownReason::NoRule);
            state_line(ui, p, l, &state);
            ui.add_space(4.0);
            let s = if cls.match_of(node).is_some() { l.t("ispettore-azioni-elemento") } else { kbtext::unknown_reason(l, UnknownReason::NoRule, None) };
            widgets::paragraph(ui, s, Type::Callout, p.text_2);
            return;
        };
        let r = &m.record;
        state_line(ui, p, l, &r.state);
        ui.add_space(4.0);
        widgets::paragraph(ui, kbtext::rule_name(l, &d.kb, m.rule), Type::BodyStrong, p.text);
        widgets::paragraph(ui, kbtext::rule_line(l, f, &d.kb, m), Type::Callout, p.text_2);
        if let State::Unknown(reason) = r.state {
            widgets::paragraph(ui, kbtext::unknown_reason(l, reason, r.rule().map(|x| f.day(x.verified))), Type::Callout, p.text_2);
        }
        // La stima comincia già con la parola "Stima" (stima-*): niente etichetta davanti.
        widgets::paragraph(ui, kbtext::estimate_text(l, f, &r.estimate), Type::Callout, p.text_2);
        if !r.hypotheses.is_empty() || !r.risks.is_empty() {
            ui.add_space(6.0);
        }
        for h in &r.hypotheses {
            fact_in(ui, p, Evidence::Inferred, &l.t(h), l, p.text);
        }
        for k in &r.risks {
            widgets::paragraph(ui, l.t(k), Type::Callout, p.warn);
        }
        let recycle = r.action == quado_model::record::Action::Recycle;
        if recycle {
            // Spostamento: il pulsante per primo, poi come lo fa Quado.
            if r.state.proposes() {
                ui.add_space(10.0);
                let w = ui.available_width();
                if wide_button(ui, p, &l.t("azione-prepara-cestino"), None, w, true, true).clicked() {
                    *req = Some(KbRequest::Act(super::intervention_jobs::ActRequest::PrepareRecycle));
                }
            }
            ui.add_space(8.0);
            steps(ui, p, l, r, req);
        } else {
            // Procedura o comando: prima i passi, poi i pulsanti e l'esito della misura.
            ui.add_space(8.0);
            steps(ui, p, l, r, req);
            action_buttons(ui, p, l, f, r, req);
        }
        ui.add_space(6.0);
        ui.push_id("scheda_record", |ui| widgets::disclosure(ui, p, &l.t("spostamento-scheda"), Type::Callout, details));
        if *details {
            let line = |ui: &mut Ui, label: &str, value: &str| widgets::paragraph(ui, format!("{label}: {value}"), Type::Callout, p.text_2);
            line(ui, &l.t("record-azione"), &kbtext::action_label(l, &r.action));
            if !r.checks.is_empty() {
                widgets::typed(ui, l.t("record-controlli"), Type::Caption, p.text_2);
                for c in &r.checks {
                    let s = format!("{} · {}", l.t(&c.key), kbtext::check_outcome(l, c.outcome));
                    let color = if c.outcome == quado_model::record::CheckOutcome::Failed { p.warn } else { p.text_2 };
                    widgets::paragraph(ui, s, Type::Callout, color);
                }
            }
            line(ui, &l.t("record-privilegi"), &kbtext::privileges_label(l, r.privileges));
            line(ui, &l.t("record-reversibilita"), &l.t(&r.reversibility));
            line(ui, &l.t("record-errore"), &l.t(&r.on_error));
            line(ui, &l.t("record-verifica"), &l.t(&r.after));
        }
    });
}

/// Metodo e alternative del record: il testo del passo, il comando o l'indirizzo con «Copia».
fn steps(ui: &mut Ui, p: &Palette, l: &Lingue, r: &Record, req: &mut Option<KbRequest>) {
    // `which`: `None` per il metodo, `Some(i)` per l'alternativa `i`.
    let step = |ui: &mut Ui, s: &quado_model::record::Step, which: Option<usize>, req: &mut Option<KbRequest>| {
        widgets::paragraph(ui, l.t(&s.key), Type::Callout, p.text_2);
        if let Some(c) = s.command.as_ref().or(s.uri.as_ref()) {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::Label::new(egui::RichText::new(c).font(mono(12.0)).color(p.text)).wrap());
                if widgets::button(ui, p, &l.t("record-copia"), Some(Icon::Copy), ButtonKind::Quiet, true).clicked() {
                    // Il comando di un'azione comando si copia con la misura di partenza:
                    // Quado lo copia e non lo esegue.
                    *req = Some(if r.action == quado_model::record::Action::Command && s.command.is_some() {
                        KbRequest::Act(super::intervention_jobs::ActRequest::CopyCommand { alternative: which })
                    } else {
                        KbRequest::Copy(c.clone())
                    });
                }
            });
        }
    };
    if let Some(s) = &r.preferred {
        step(ui, s, None, req);
    }
    if !r.alternatives.is_empty() {
        ui.add_space(4.0);
        widgets::typed(ui, l.t("record-alternative"), Type::Caption, p.text_2);
        for (i, s) in r.alternatives.iter().enumerate() {
            step(ui, s, Some(i), req);
        }
    }
}

/// Campi del record dallo stato in giù: stato, azione, metodo e alternative, controlli, stima,
/// privilegi, ipotesi, rischi, reversibilità, errore, verifica, poi i pulsanti degli interventi.
/// `estimate` è la stima da mostrare (la somma, per una riga della lente Download con più voci).
pub fn record_block(ui: &mut Ui, p: &Palette, l: &Lingue, f: &crate::format::Formatter, r: &Record, estimate: &Estimate, req: &mut Option<KbRequest>) {
    record_fields(ui, p, l, f, r, estimate, req);
    action_buttons(ui, p, l, f, r, req);
}

/// I campi di `record_block` senza i pulsanti degli interventi (per il dialogo di conferma).
pub fn record_fields(ui: &mut Ui, p: &Palette, l: &Lingue, f: &crate::format::Formatter, r: &Record, estimate: &Estimate, req: &mut Option<KbRequest>) {
    state_line(ui, p, l, &r.state);
    if let State::Unknown(reason) = r.state {
        let s = kbtext::unknown_reason(l, reason, r.rule().map(|x| f.day(x.verified)));
        ui.add(egui::Label::new(egui::RichText::new(s).font(sans(12.5)).color(p.text_2)).wrap());
    }
    let line = |ui: &mut Ui, label: &str, value: &str| {
        ui.add(egui::Label::new(egui::RichText::new(format!("{label}: {value}")).font(sans(12.5)).color(p.text_2)).wrap());
    };
    line(ui, &l.t("record-azione"), &kbtext::action_label(l, &r.action));
    // `which`: `None` per il metodo, `Some(i)` per l'alternativa `i`.
    let step = |ui: &mut Ui, s: &quado_model::record::Step, which: Option<usize>, req: &mut Option<KbRequest>| {
        ui.add(egui::Label::new(egui::RichText::new(l.t(&s.key)).font(sans(12.5)).color(p.text)).wrap());
        if let Some(c) = s.command.as_ref().or(s.uri.as_ref()) {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::Label::new(egui::RichText::new(c).font(mono(12.0)).color(p.text)).wrap());
                if widgets::button(ui, p, &l.t("record-copia"), Some(Icon::Copy), ButtonKind::Quiet, true).clicked() {
                    // Il comando di un'azione comando si copia con la misura di partenza:
                    // Quado lo copia e non lo esegue.
                    *req = Some(if r.action == quado_model::record::Action::Command && s.command.is_some() {
                        KbRequest::Act(super::intervention_jobs::ActRequest::CopyCommand { alternative: which })
                    } else {
                        KbRequest::Copy(c.clone())
                    });
                }
            });
        }
    };
    if let Some(s) = &r.preferred {
        widgets::text(ui, l.t("record-metodo"), widgets::medium(12.0), p.text_2);
        step(ui, s, None, req);
    }
    if !r.alternatives.is_empty() {
        widgets::text(ui, l.t("record-alternative"), widgets::medium(12.0), p.text_2);
        for (i, s) in r.alternatives.iter().enumerate() {
            step(ui, s, Some(i), req);
        }
    }
    if !r.checks.is_empty() {
        widgets::text(ui, l.t("record-controlli"), widgets::medium(12.0), p.text_2);
        for c in &r.checks {
            let s = format!("{} · {}", l.t(&c.key), kbtext::check_outcome(l, c.outcome));
            let color = if c.outcome == quado_model::record::CheckOutcome::Failed { p.warn } else { p.text_2 };
            ui.add(egui::Label::new(egui::RichText::new(s).font(sans(12.5)).color(color)).wrap());
        }
    }
    // La stima comincia già con la parola "Stima" (stima-*): niente etichetta davanti.
    ui.add(egui::Label::new(egui::RichText::new(kbtext::estimate_text(l, f, estimate)).font(sans(12.5)).color(p.text_2)).wrap());
    line(ui, &l.t("record-privilegi"), &kbtext::privileges_label(l, r.privileges));
    for h in &r.hypotheses {
        ui.add(egui::Label::new(egui::RichText::new(l.t(h)).font(sans(12.5)).color(p.text_2)).wrap());
    }
    for k in &r.risks {
        ui.add(egui::Label::new(egui::RichText::new(l.t(k)).font(sans(12.5)).color(p.warn)).wrap());
    }
    line(ui, &l.t("record-reversibilita"), &l.t(&r.reversibility));
    line(ui, &l.t("record-errore"), &l.t(&r.on_error));
    line(ui, &l.t("record-verifica"), &l.t(&r.after));
}

/// Pulsanti degli interventi sotto il record, solo quelli che hanno senso per il record e solo per gli stati
/// che propongono qualcosa: spostamento nel Cestino dopo la simulazione; per una procedura la pagina che si
/// apre (o la sola misura di partenza, se la procedura sta dentro un'app), "Svuota il Cestino…" per il
/// Cestino e "Ho eseguito la procedura"; per un comando "Ho eseguito la procedura" (il comando si copia dai
/// passi sopra, con la misura di partenza). Sotto, la misura di partenza e l'esito: li disegna
/// `procedure_flow`.
fn action_buttons(ui: &mut Ui, p: &Palette, l: &Lingue, f: &crate::format::Formatter, r: &Record, req: &mut Option<KbRequest>) {
    use super::intervention_jobs::ActRequest;
    use quado_model::record::Action;
    if !r.state.proposes() {
        return;
    }
    match r.action {
        Action::Recycle => {
            ui.add_space(6.0);
            if widgets::button(ui, p, &l.t("azione-prepara-cestino"), None, ButtonKind::Secondary, true).clicked() {
                *req = Some(KbRequest::Act(ActRequest::PrepareRecycle));
            }
        }
        Action::Procedure | Action::Command => super::procedure_flow::record_controls(ui, p, l, f, r, req),
        Action::Uninstall | Action::None => {}
    }
}

/// La regola del record, con versione, data di verifica, scadenza, altre regole e "Apri la regola".
fn rule_block(ui: &mut Ui, p: &Palette, l: &Lingue, f: &crate::format::Formatter, d: &data::Loaded, m: &quado_kb::Match, req: &mut Option<KbRequest>) {
    let r = &m.record;
    let rule = &d.kb.rules[m.rule];
    ui.spacing_mut().item_spacing.y = 2.0;
    widgets::paragraph(ui, format!("{} · {}", l.t(&rule.key("nome")), rule.id), Type::Callout, p.text);
    widgets::paragraph(ui, l.ta("regole-verificata", &[("data", f.day(rule.verified).into())]), Type::Callout, p.text_2);
    match r.expiry {
        quado_model::record::Expiry::Announce => {
            let s = l.ta("regole-scadenza-annuncio", &[("data", f.day(rule.verified).into()), ("limite", f.day(rule.verified.add_months(12)).into())]);
            widgets::paragraph(ui, s, Type::Callout, p.warn);
        }
        quado_model::record::Expiry::Expired => {
            let s = l.ta("regole-scadenza-scaduta", &[("data", f.day(rule.verified).into())]);
            widgets::paragraph(ui, s, Type::Callout, p.warn);
        }
        _ => {}
    }
    if !m.others.is_empty() {
        let names: Vec<String> = m.others.iter().map(|&o| l.t(&d.kb.rules[o].key("nome"))).collect();
        widgets::paragraph(ui, l.ta("ispettore-anche", &[("regole", names.join(", ").into())]), Type::Callout, p.text_2);
    }
    ui.add_space(4.0);
    // «Apri la regola ›»: un collegamento nel colore dell'accento come testo, senza fondo.
    if ui.push_id("ispettore_vedi_regola", |ui| link_button(ui, p, &l.t("ispettore-vedi-regola"))).inner.clicked() {
        *req = Some(KbRequest::Rule(m.rule));
    }
}

/// Collegamento con la freccia a destra (`.btn.quiet` nel colore `accent_text`), area alta 32.
pub(crate) fn link_button(ui: &mut Ui, p: &Palette, label: &str) -> egui::Response {
    let g = ui.painter().layout_no_wrap(label.to_owned(), widgets::medium(13.0), p.accent_text);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 30.0, crate::tokens::focus::MIN_TARGET), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(crate::tokens::radius::CONTROL), p.hover);
    }
    let gw = g.size().x;
    ui.painter().galley(pos2(rect.min.x + 6.0, rect.center().y - g.size().y / 2.0), g, p.accent_text);
    crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(rect.min.x + 6.0 + gw + 10.0, rect.center().y), vec2(14.0, 14.0)), Icon::ChevronRight, p.accent_text);
    widgets::focus_ring(ui, &resp, p, crate::tokens::radius::CONTROL);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

/// Stato con icona e parola; Da verificare con il tono di avviso.
pub fn state_line(ui: &mut Ui, p: &Palette, l: &Lingue, s: &State) {
    let tone = if matches!(s, State::ToVerify(_)) { p.warn } else { p.text };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (r, _) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::hover());
        crate::icons::paint(ui.painter(), r, kbtext::state_icon(s), tone);
        widgets::text(ui, kbtext::state_label(l, s), widgets::medium(13.0), tone);
    });
}

/// Età dei file secondo la data di ultima modifica.
fn ages(ui: &mut Ui, p: &Palette, l: &Lingue, f: &crate::format::Formatter, info: &Info, total: u64) {
    for (k, key) in ["eta-30-giorni", "eta-1-anno", "eta-3-anni", "eta-oltre"].iter().enumerate() {
        let (a, c) = info.ages[k];
        if c == 0 {
            continue;
        }
        let count = l.ta("conteggio-file", &[("n", c.into()), ("testo", f.count(c).into())]);
        part_row(ui, p, f, &l.t(key), Some(&count), a, total, None, false);
    }
    ui.add_space(4.0);
    widgets::paragraph(ui, l.t("ispettore-eta-nota"), Type::Callout, p.text_2);
}
