// SPDX-License-Identifier: GPL-3.0-or-later
//! Motore di classificazione (05 D6): dalle regole, dall'albero della scansione e dalle prove degli
//! estrattori produce i record di raccomandazione (03 §6.1) e il gestore di ogni elemento.
//!
//! - Ogni regola ha un riconoscitore che trova i suoi elementi e raccoglie i fatti.
//! - L'affidabilità la calcola il motore con i criteri di 03 §6.2, mai la regola.
//! - Un byte ha un solo proprietario: l'elemento riconosciuto più profondo (prefisso più lungo, F-AP-2).
//! - Due regole sullo stesso elemento: se concordano su gestore e stato vale la più specifica, se no
//!   si mostrano entrambe, con affidabilità bassa e nessuna proposta (03 §6.1 regola 5).
//! - Scadenza delle regole secondo 07 §5.1.

use std::collections::HashMap;

use rayon::prelude::*;

use quado_model::record::{
    Action, Check, CheckKind, CheckOutcome, ColorGroup, Criterion, Date, Estimate, Expiry, Fact, Manager, NotEstimable, ProvaDiGestibilita,
    Origin, ProvaDiIdentita, Provenance, Record, RuleRef, Source, State, Step, UnknownReason, Value,
};
use quado_model::{Flags, NONE, Name, Tree};

use crate::evidence::Evidence;
use crate::extract::{self, ConfigOrigin, ToolPath, parse_version};
use crate::rules::{ActionCode, EstimateCode, Kb, Passo, Recognizer, Rule, StateCode};

/// Elementi riconosciuti da una regola, con il record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub rule: usize,
    pub nodes: Vec<u32>,
    pub manager: Manager,
    /// Provenienza dell'attribuzione al gestore: decide il colore (03 §6.2).
    pub attribution: Provenance,
    /// Valori per i testi della regola (`regola-<id>-riga`): nome, versione, numero di elementi.
    pub args: Vec<(&'static str, Value)>,
    pub record: Record,
    /// Altre regole che riconoscono gli stessi elementi, mostrate insieme (03 §6.1 regola 5).
    pub others: Vec<usize>,
}

impl Match {
    pub fn color(&self) -> ColorGroup {
        if self.attribution.colors() { ColorGroup::Manager(self.manager) } else { ColorGroup::Unclassified }
    }

    pub fn arg(&self, name: &str) -> Option<&Value> {
        self.args.iter().find(|(k, _)| *k == name).map(|(_, v)| v)
    }
}

/// Byte dell'allocato letto per gruppo di colore (bilancio per gestore, F-OV-1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ByManager {
    pub windows: u64,
    pub programs: u64,
    pub you: u64,
    /// Non classificato e ipotesi.
    pub unclassified: u64,
}

impl ByManager {
    pub fn get(&self, g: ColorGroup) -> u64 {
        match g {
            ColorGroup::Manager(Manager::Windows) => self.windows,
            ColorGroup::Manager(Manager::Programs) => self.programs,
            ColorGroup::Manager(Manager::You) => self.you,
            ColorGroup::Unclassified => self.unclassified,
        }
    }

    fn slot(g: ColorGroup) -> usize {
        match g {
            ColorGroup::Manager(Manager::Windows) => 0,
            ColorGroup::Manager(Manager::Programs) => 1,
            ColorGroup::Manager(Manager::You) => 2,
            ColorGroup::Unclassified => 3,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Classification {
    pub today: Option<Date>,
    pub matches: Vec<Match>,
    /// Per nodo: la corrispondenza dell'elemento riconosciuto più vicino (o `NONE`).
    inherited: Vec<u32>,
    /// Nodi riconosciuti direttamente -> corrispondenza.
    own: HashMap<u32, u32>,
    pub by_manager: ByManager,
    /// Regole oltre i 6 mesi dalla verifica (banner "regole da riverificare").
    pub recheck: Vec<usize>,
    /// Per elemento riconosciuto: allocata degli elementi riconosciuti al suo interno.
    nested: HashMap<u32, u64>,
}

impl Classification {
    /// Corrispondenza che vale per `node`: la sua o quella dell'antenato riconosciuto più vicino.
    pub fn match_of(&self, node: u32) -> Option<usize> {
        self.inherited.get(node as usize).copied().filter(|&m| m != NONE).map(|m| m as usize)
    }

    /// Corrispondenza di cui `node` è uno degli elementi.
    pub fn own(&self, node: u32) -> Option<usize> {
        self.own.get(&node).map(|&m| m as usize)
    }

    /// Allocata di un elemento riconosciuto senza gli elementi di altre regole al suo interno: il
    /// Cestino senza le cartelle orfane, un programma senza i dati che un'altra regola spiega.
    pub fn own_bytes(&self, tree: &Tree, node: u32) -> u64 {
        tree.alloc(node).saturating_sub(self.nested.get(&node).copied().unwrap_or(0))
    }

    pub fn color(&self, node: u32) -> ColorGroup {
        self.match_of(node).map_or(ColorGroup::Unclassified, |m| self.matches[m].color())
    }

    /// Elementi riconosciuti, in ordine di nodo.
    pub fn anchors(&self) -> impl Iterator<Item = (u32, usize)> + '_ {
        let mut v: Vec<(u32, usize)> = self.own.iter().map(|(&n, &m)| (n, m as usize)).collect();
        v.sort_unstable();
        v.into_iter()
    }
}

// ---------------------------------------------------------------------------------------------
// Albero

fn name_is(name: Name<'_>, want: &str) -> bool {
    match name {
        Name::Ascii(b) => b.len() == want.len() && b.iter().zip(want.bytes()).all(|(a, w)| a.eq_ignore_ascii_case(&w)),
        Name::Wide(_) => name.to_string_lossy().to_lowercase() == want.to_lowercase(),
    }
}

/// Glob con un solo `*` (`thumbcache_*.db`), senza distinzione di maiuscole.
fn glob(name: &str, pattern: &str) -> bool {
    let n = name.to_lowercase();
    let p = pattern.to_lowercase();
    match p.split_once('*') {
        None => n == p,
        Some((a, b)) => n.len() >= a.len() + b.len() && n.starts_with(a) && n.ends_with(b),
    }
}

pub fn child(tree: &Tree, parent: u32, name: &str) -> Option<u32> {
    tree.children(parent).find(|&c| !tree.flags(c).has(Flags::DETACHED) && name_is(tree.name(c), name))
}

/// Il primo di `locks` (nel loro ordine) che è un file figlio di `parent`: come provare `child` per ogni
/// nome, ma con un passaggio solo sui figli. Per ogni nome conta il primo figlio con quel nome, come in
/// `child`.
fn first_lockfile(tree: &Tree, parent: u32, locks: &[String]) -> Option<usize> {
    let mut first: Vec<Option<u32>> = vec![None; locks.len()];
    for c in tree.children(parent) {
        if tree.flags(c).has(Flags::DETACHED) {
            continue;
        }
        let name = tree.name(c);
        for (k, l) in locks.iter().enumerate() {
            if first[k].is_none() && name_is(name, l) {
                first[k] = Some(c);
            }
        }
    }
    first.iter().position(|f| f.is_some_and(|c| !tree.is_dir(c)))
}

/// Nodo di un percorso assoluto (`C:\Users\...`), se la scansione l'ha incontrato.
pub fn find(tree: &Tree, path: &str) -> Option<u32> {
    let root = tree.name_lossy(Tree::ROOT);
    let root_trim = root.trim_end_matches('\\');
    let p = extract::normalize(path);
    let rest = if p.len() >= root_trim.len() && p[..root_trim.len()].eq_ignore_ascii_case(root_trim) { &p[root_trim.len()..] } else { return None };
    let mut n = Tree::ROOT;
    for seg in rest.split('\\').filter(|s| !s.is_empty()) {
        n = child(tree, n, seg)?;
    }
    Some(n)
}

fn has_child(tree: &Tree, n: u32, name: &str) -> bool {
    child(tree, n, name).is_some()
}

fn is_version(name: &str) -> bool {
    parse_version(name).is_some_and(|v| v.len() == 4)
}

/// Nodi che corrispondono a un percorso relativo (`CapCut/Apps/<versione>`).
fn match_pattern(tree: &Tree, base: u32, pattern: &str, sid: Option<&str>) -> Vec<u32> {
    let mut cur = vec![base];
    for seg in pattern.split('/').filter(|s| !s.is_empty() && *s != ".") {
        let mut next = Vec::new();
        for &n in &cur {
            for c in tree.children(n) {
                if tree.flags(c).has(Flags::DETACHED) {
                    continue;
                }
                let ok = match seg {
                    "*" => true,
                    "<versione>" => is_version(&tree.name_lossy(c)),
                    "<sid>" => sid.is_some_and(|s| name_is(tree.name(c), s)),
                    s if s.contains('*') => glob(&tree.name_lossy(c), s),
                    s => name_is(tree.name(c), s),
                };
                if ok {
                    next.push(c);
                }
            }
        }
        cur = next;
    }
    cur.sort_unstable();
    cur
}

fn is_ancestor(tree: &Tree, a: u32, mut b: u32) -> bool {
    while b != NONE {
        if a == b {
            return true;
        }
        b = tree.parent(b);
    }
    false
}

// ---------------------------------------------------------------------------------------------
// Riconoscitori

/// Ciò che un riconoscitore ha trovato, prima di diventare un record.
#[derive(Default)]
struct Found {
    nodes: Vec<u32>,
    attribution: Option<Provenance>,
    /// Criterio dei fatti dedotti dalla regola.
    criterion: Option<Criterion>,
    identity: Vec<Fact>,
    manageability: Vec<Fact>,
    checks: Vec<Check>,
    args: Vec<(&'static str, Value)>,
    estimate: Option<Estimate>,
    unknown: Option<UnknownReason>,
    risks: Vec<String>,
    /// Passo preferito diverso da quello della regola (URI con un identificativo).
    uri: Option<String>,
}

fn fact(key: &str, p: Provenance, s: Source, v: Vec<Value>) -> Fact {
    Fact::new(key, p, s, v)
}

fn text(s: &str) -> Value {
    Value::Text(s.to_owned())
}

struct Ctx<'a> {
    kb: &'a Kb,
    tree: &'a Tree,
    ev: &'a Evidence,
}

impl Ctx<'_> {
    fn official(&self, rule: &Rule) -> bool {
        rule.sources.iter().any(|s| s.ufficiale)
    }

    /// Criterio di una struttura riconosciuta sotto un percorso noto: alta con una fonte ufficiale e i
    /// marcatori presenti, media senza fonte ufficiale, bassa se i marcatori mancano.
    fn structure(&self, rule: &Rule, node: u32) -> Criterion {
        if !rule.place.marcatori.iter().all(|m| has_child(self.tree, node, m)) {
            return Criterion::NameOnly;
        }
        if self.official(rule) { Criterion::OfficialWithStructure } else { Criterion::StructureOnly }
    }

    fn base(&self, rule: &Rule) -> Option<u32> {
        let code = rule.place.cartella.as_deref()?;
        let p = self.ev.folders.get(code, &self.ev.volume_root)?;
        find(self.tree, p)
    }

    fn path_rule(&self, rule: &Rule) -> Vec<Found> {
        let default_base = self.base(rule);
        let mut nodes: Vec<u32> = Vec::new();
        for p in &rule.place.percorsi {
            let (base, rel) = match p.split_once(':') {
                Some((code, rel)) => (self.ev.folders.get(code, &self.ev.volume_root).and_then(|b| find(self.tree, b)), rel),
                None => (default_base, p.as_str()),
            };
            let Some(base) = base else { continue };
            if rel == "." {
                nodes.push(base);
            } else {
                nodes.extend(match_pattern(self.tree, base, rel, self.ev.user_sid.as_deref()));
            }
        }
        nodes.sort_unstable();
        nodes.dedup();
        if nodes.is_empty() {
            return Vec::new();
        }
        let criterion = nodes.iter().map(|&n| self.structure(rule, n)).min_by_key(|c| c.level()).unwrap_or(Criterion::NameOnly);
        let mut f = Found { criterion: Some(criterion), attribution: Some(Provenance::Deduced(criterion)), ..Default::default() };
        f.identity.push(fact("fatto-percorso-documentato", Provenance::Deduced(criterion), Source::Rule, vec![Value::Count(nodes.len() as u64)]));
        f.args.push(("n", Value::Count(nodes.len() as u64)));
        // Ibernazione: avviso se l'azione per batteria critica è l'ibernazione (decisioni §3).
        if rule.id == "ibernazione" {
            self.hibernation(&mut f);
        }
        // Progetti di CapCut: la cartella delle bozze dichiarata nella configurazione di CapCut.
        if rule.id == "capcut-progetti"
            && let Ok(d) = &self.ev.capcut_drafts
        {
            let dl = d.to_lowercase();
            if nodes.iter().any(|&n| {
                let p = self.tree.path_lossy(n).to_lowercase();
                dl == p || dl.starts_with(&format!("{p}\\"))
            }) {
                f.identity.push(fact("fatto-cartella-bozze", Provenance::FromSource, Source::CapCutConfig, vec![text(d)]));
                f.attribution = Some(Provenance::FromSource);
            }
        }
        f.nodes = nodes;
        vec![f]
    }

    fn hibernation(&self, f: &mut Found) {
        match &self.ev.power {
            Ok(p) => {
                f.identity.push(fact("fatto-ibernazione-attiva", Provenance::FromSource, Source::PowerApi, vec![Value::Count(u64::from(p.hibernate))]));
                if let Some(fs) = p.fast_startup {
                    f.identity.push(fact("fatto-avvio-rapido", Provenance::FromSource, Source::PowerApi, vec![Value::Count(u64::from(fs))]));
                }
                let outcome = match (p.has_battery, p.critical_battery_action) {
                    (true, Some(2)) => CheckOutcome::Failed,
                    (_, Some(_)) | (false, None) => CheckOutcome::Passed,
                    (true, None) => CheckOutcome::AtSimulation,
                };
                if let Some(a) = p.critical_battery_action {
                    f.identity.push(fact("fatto-batteria-critica", Provenance::FromSource, Source::PowerApi, vec![Value::Count(u64::from(a))]));
                }
                // Le conseguenze sulla batteria valgono solo con una batteria; lo standby moderno iberna da
                // solo il PC in sospensione per non scaricarla (Adaptive Hibernate).
                if p.has_battery {
                    f.risks.push("regola-ibernazione-rischio-batteria".into());
                    if p.modern_standby {
                        f.identity.push(fact("fatto-standby-moderno", Provenance::FromSource, Source::PowerApi, Vec::new()));
                        f.risks.push("regola-ibernazione-rischio-standby".into());
                    }
                }
                f.checks.push(Check { key: "controllo-batteria-critica".into(), kind: CheckKind::Automatic, outcome });
            }
            Err(_) => {
                f.identity.push(fact("fatto-alimentazione-non-letta", Provenance::Unavailable, Source::PowerApi, Vec::new()));
                f.checks.push(Check { key: "controllo-batteria-critica".into(), kind: CheckKind::Automatic, outcome: CheckOutcome::AtSimulation });
            }
        }
    }

    /// Voce di disinstallazione di CapCut (HKCU) e versione dichiarata.
    fn capcut_entry(&self) -> Option<&extract::UninstallEntry> {
        let u = self.ev.uninstall.as_ref().ok()?;
        u.entries.iter().find(|e| e.display_name.eq_ignore_ascii_case("CapCut"))
    }

    fn capcut_versions(&self, rule: &Rule) -> Vec<Found> {
        let Some(base) = self.base(rule) else { return Vec::new() };
        let mut versions: Vec<(Vec<u32>, u32)> = Vec::new();
        for p in &rule.place.percorsi {
            for n in match_pattern(self.tree, base, p, None) {
                if let Some(v) = parse_version(&self.tree.name_lossy(n)) {
                    versions.push((v, n));
                }
            }
        }
        if versions.is_empty() {
            return Vec::new();
        }
        versions.sort();
        let mut f = Found::default();
        f.identity.push(fact("fatto-cartelle-di-versione", Provenance::Measured, Source::Scan, vec![Value::Count(versions.len() as u64)]));
        let from_xml = self.ev.capcut.as_ref().ok();
        let from_reg = self.capcut_entry().and_then(|e| e.version.as_deref());
        if let Some(v) = from_xml {
            f.identity.push(fact("fatto-versione-in-uso", Provenance::FromSource, Source::CapCutProductInfo, vec![text(v)]));
        }
        if let (Some(v), Some(e)) = (from_reg, self.capcut_entry()) {
            f.identity.push(fact("fatto-versione-in-uso", Provenance::FromSource, Source::Uninstall(e.hive), vec![text(v)]));
        }
        let in_use = match (from_xml, from_reg) {
            (Some(a), Some(b)) if a == b => a.clone(),
            (Some(_), Some(_)) => {
                f.unknown = Some(UnknownReason::Conflict);
                f.criterion = Some(Criterion::Conflict);
                f.attribution = Some(Provenance::Deduced(Criterion::Conflict));
                f.nodes = versions.iter().map(|&(_, n)| n).collect();
                return vec![f];
            }
            _ => {
                f.unknown = Some(UnknownReason::VersionOutOfRange);
                f.criterion = Some(Criterion::VersionOutOfRange);
                f.attribution = Some(Provenance::Deduced(Criterion::VersionOutOfRange));
                f.nodes = versions.iter().map(|&(_, n)| n).collect();
                return vec![f];
            }
        };
        let in_use_v = parse_version(&in_use).unwrap_or_default();
        // Versione installata dentro l'intervallo della regola (03 §6.1 regola 4).
        let in_range = rule.versions.as_ref().is_some_and(|r| match (parse_version(&r.min), parse_version(&r.max)) {
            (Some(lo), Some(hi)) => in_use_v >= lo && in_use_v <= hi,
            _ => false,
        });
        let Some(k_in_use) = versions.iter().position(|(v, _)| *v == in_use_v) else {
            f.unknown = Some(UnknownReason::VersionOutOfRange);
            f.criterion = Some(Criterion::VersionOutOfRange);
            f.attribution = Some(Provenance::Deduced(Criterion::VersionOutOfRange));
            f.nodes = versions.iter().map(|&(_, n)| n).collect();
            return vec![f];
        };
        // Si tengono la versione in uso e la precedente; le versioni più nuove di quella in uso (un
        // aggiornamento in corso) non si toccano.
        let kept: Vec<usize> = versions.iter().enumerate().filter(|&(k, _)| k + 1 >= k_in_use).map(|(k, _)| k).collect();
        let old: Vec<u32> = versions.iter().enumerate().filter(|(k, _)| !kept.contains(k)).map(|(_, &(_, n))| n).collect();
        let previous = k_in_use.checked_sub(1).map(|k| self.tree.name_lossy(versions[k].1));
        f.args.push(("n", Value::Count(old.len() as u64)));
        f.args.push(("versione", text(&in_use)));
        if let Some(p) = &previous {
            f.args.push(("precedente", text(p)));
        }
        f.identity.push(fact("fatto-versioni-precedenti", Provenance::Deduced(Criterion::StructureOnly), Source::Rule, vec![Value::Count(old.len() as u64)]));
        // Prova di gestibilità: la versione in uso da due fonti concordi, diversa da queste cartelle,
        // e le ultime due tenute. È parziale: per questo lo stato è Da verificare.
        f.manageability.push(fact("fatto-non-versione-in-uso", Provenance::Deduced(Criterion::TwoSources), Source::Rule, vec![text(&in_use)]));
        f.manageability.push(fact("fatto-versioni-tenute", Provenance::Deduced(Criterion::StructureOnly), Source::Rule, vec![Value::Count(kept.len() as u64)]));
        if !in_range {
            f.unknown = Some(UnknownReason::VersionOutOfRange);
            f.criterion = Some(Criterion::VersionOutOfRange);
            f.attribution = Some(Provenance::Deduced(Criterion::VersionOutOfRange));
        } else {
            f.criterion = Some(Criterion::StructureOnly);
            f.attribution = Some(Provenance::Deduced(Criterion::TwoSources));
        }
        f.estimate = Some(Estimate::Exact(old.iter().map(|&n| self.tree.alloc(n)).sum()));
        f.nodes = old;
        if f.nodes.is_empty() {
            return Vec::new();
        }
        vec![f]
    }

    fn user_bin(&self) -> Option<u32> {
        let sid = self.ev.user_sid.as_deref()?;
        let bin = child(self.tree, Tree::ROOT, "$Recycle.Bin")?;
        child(self.tree, bin, sid)
    }

    /// Cartelle `$R...` senza il `$I...` corrispondente.
    fn orphans(&self, bin: u32) -> Vec<u32> {
        let names: Vec<(u32, String)> = self.tree.children(bin).map(|c| (c, self.tree.name_lossy(c))).collect();
        let mut out: Vec<u32> = names
            .iter()
            .filter(|(c, n)| self.tree.is_dir(*c) && n.starts_with("$R"))
            .filter(|(_, n)| {
                let want = format!("$I{}", &n[2..]);
                !names.iter().any(|(_, m)| m.eq_ignore_ascii_case(&want))
            })
            .map(|&(c, _)| c)
            .collect();
        out.sort_unstable();
        out
    }

    fn recycle_bin(&self, rule: &Rule) -> Vec<Found> {
        let Some(bin) = self.user_bin() else { return Vec::new() };
        let c = self.structure(rule, bin);
        let mut f = Found { nodes: vec![bin], criterion: Some(c), attribution: Some(Provenance::Deduced(c)), ..Default::default() };
        f.identity.push(fact("fatto-cestino-su-disco", Provenance::Measured, Source::Scan, vec![Value::Bytes(self.tree.alloc(bin))]));
        match &self.ev.recycle {
            Ok(q) => {
                f.identity.push(fact("fatto-cestino-secondo-windows", Provenance::FromSource, Source::RecycleBinApi, vec![Value::Bytes(q.bytes), Value::Count(q.items)]));
                f.args.push(("windows", Value::Bytes(q.bytes)));
                f.args.push(("elementi", Value::Count(q.items)));
            }
            // Senza conteggio il fatto non ha valori: una chiave sua, senza segnaposto.
            Err(_) => f.identity.push(fact("fatto-cestino-secondo-windows-non-letto", Provenance::Unavailable, Source::RecycleBinApi, Vec::new())),
        }
        f.args.push(("disco", Value::Bytes(self.tree.alloc(bin))));
        if let Ok(s) = &self.ev.storage_sense
            && s.enabled
            && let Some(d) = s.recycle_days
        {
            f.identity.push(fact("fatto-sensore-memoria-cestino", Provenance::FromSource, Source::StorageSense, vec![Value::Days(d)]));
        }
        if let Ok(b) = &self.ev.bin
            && let Some(mib) = b.max_capacity_mib
        {
            f.identity.push(fact("fatto-cestino-capacita", Provenance::FromSource, Source::BitBucket, vec![Value::Bytes(u64::from(mib) << 20)]));
        }
        // La stima dello svuotamento la fa Windows: si mostra il conteggio di Windows, mai il disco.
        f.estimate = Some(Estimate::NotEstimable(NotEstimable::ExternalProcedure));
        vec![f]
    }

    fn recycle_orphans(&self, rule: &Rule) -> Vec<Found> {
        let Some(bin) = self.user_bin() else { return Vec::new() };
        let orphans = self.orphans(bin);
        if orphans.is_empty() {
            return Vec::new();
        }
        let bytes: u64 = orphans.iter().map(|&n| self.tree.alloc(n)).sum();
        let c = self.structure(rule, bin);
        let mut f = Found { criterion: Some(c), attribution: Some(Provenance::Deduced(c)), ..Default::default() };
        f.identity.push(fact("fatto-cartelle-orfane", Provenance::Measured, Source::Scan, vec![Value::Count(orphans.len() as u64), Value::Bytes(bytes)]));
        f.args.push(("n", Value::Count(orphans.len() as u64)));
        f.args.push(("disco", Value::Bytes(bytes)));
        f.estimate = Some(Estimate::NotEstimable(NotEstimable::NoAction));
        f.nodes = orphans;
        vec![f]
    }

    fn steam(&self, _rule: &Rule) -> Vec<Found> {
        let Ok(s) = &self.ev.steam else { return Vec::new() };
        let mut out = Vec::new();
        for app in &s.apps {
            let Some(n) = find(self.tree, &app.folder()) else { continue };
            let mut f = Found { nodes: vec![n], attribution: Some(Provenance::FromSource), criterion: Some(Criterion::OfficialWithStructure), ..Default::default() };
            // L'ID è un identificativo, non un conteggio: si scrive come in Steam, senza separatore delle migliaia.
            f.identity.push(fact("fatto-gioco-steam", Provenance::FromSource, Source::SteamManifest, vec![text(&app.name), text(&app.appid.to_string())]));
            if let Some(sz) = app.size_on_disk {
                f.identity.push(fact("fatto-dimensione-dichiarata", Provenance::FromSource, Source::SteamManifest, vec![Value::Bytes(sz)]));
            }
            let local = s.local_last_played.iter().find(|(id, _)| *id == app.appid).map(|&(_, t)| t);
            match (app.last_played.and_then(unix_to_filetime), local.and_then(unix_to_filetime)) {
                (Some(t), _) => f.identity.push(fact("fatto-ultima-sessione-steam", Provenance::FromSource, Source::SteamManifest, vec![Value::Time(t)])),
                (None, Some(t)) => f.identity.push(fact("fatto-ultima-sessione-steam", Provenance::FromSource, Source::SteamLocalConfig, vec![Value::Time(t)])),
                (None, None) => f.identity.push(fact("fatto-uso-non-registrato", Provenance::Unavailable, Source::SteamManifest, Vec::new())),
            }
            f.args.push(("nome", text(&app.name)));
            f.uri = Some(format!("steam://uninstall/{}", app.appid));
            out.push(f);
        }
        out
    }

    fn epic(&self, _rule: &Rule) -> Vec<Found> {
        let Ok(e) = &self.ev.epic else { return Vec::new() };
        let mut by_node: Vec<(u32, Found)> = Vec::new();
        for it in &e.items {
            let Some(n) = find(self.tree, &it.install_location) else { continue };
            let k = match by_node.iter().position(|(m, _)| *m == n) {
                Some(k) => k,
                None => {
                    let f = Found { nodes: vec![n], attribution: Some(Provenance::FromSource), criterion: Some(Criterion::OfficialWithStructure), ..Default::default() };
                    by_node.push((n, f));
                    by_node.len() - 1
                }
            };
            let f = &mut by_node[k].1;
            f.identity.push(fact("fatto-installazione-epic", Provenance::FromSource, Source::EpicManifest, vec![text(&it.display_name)]));
            if f.args.is_empty() {
                f.args.push(("nome", text(&it.display_name)));
            }
        }
        for (_, f) in &mut by_node {
            f.args.push(("n", Value::Count(f.identity.len() as u64)));
            f.identity.push(fact("fatto-epic-nessun-dato-uso", Provenance::Unavailable, Source::EpicManifest, Vec::new()));
        }
        by_node.into_iter().map(|(_, f)| f).collect()
    }

    /// Un percorso dichiarato non è la cartella di un programma se è la radice, una cartella nota o un
    /// suo antenato (voci sbagliate o scritte dall'utente).
    fn plausible_program_dir(&self, path: &str) -> bool {
        let p = extract::normalize(path).to_lowercase();
        if p.len() <= 3 || !p.starts_with(&self.ev.volume_root.to_lowercase()) {
            return false;
        }
        for k in self.ev.folders.all() {
            let k = k.to_lowercase();
            if k == p || k.starts_with(&format!("{p}\\")) {
                return false;
            }
        }
        true
    }

    fn registered(&self, _rule: &Rule) -> Vec<Found> {
        let Ok(u) = &self.ev.uninstall else { return Vec::new() };
        let mut by_node: Vec<(u32, Found, Criterion)> = Vec::new();
        for e in &u.entries {
            let (path, declared) = match (e.location(), e.path_hint()) {
                (Some(p), _) => (p, true),
                (None, Some(p)) => (p, false),
                (None, None) => continue,
            };
            if !self.plausible_program_dir(&path) {
                continue;
            }
            let Some(n) = find(self.tree, &path) else { continue };
            // HKLM con InstallLocation: da fonte. HKCU (scrivibile dall'utente) o solo indizio dal
            // comando di disinstallazione: dedotto media (05 D6, RS22).
            let machine = e.hive != quado_model::record::Hive::User;
            let mut c = if declared && machine { None } else { Some(Criterion::OneSource) };
            // Indizio da una voce di HKLM con l'eseguibile presente nella cartella: fonte di Windows
            // con la struttura che dichiara (03 §6.2, alta).
            if !declared && machine && e.hint_exe().is_some_and(|x| find(self.tree, &x).is_some()) {
                c = Some(Criterion::OfficialWithStructure);
            }
            // CapCut: la seconda fonte è `ProductInfo.xml` nella stessa cartella, con la stessa versione.
            if c.is_some()
                && e.display_name.eq_ignore_ascii_case("CapCut")
                && let (Ok(v), Some(dv)) = (&self.ev.capcut, &e.version)
                && v == dv
                && has_child(self.tree, n, "ProductInfo.xml")
            {
                c = Some(Criterion::TwoSources);
            }
            let k = match by_node.iter().position(|(m, _, _)| *m == n) {
                Some(k) => k,
                None => {
                    by_node.push((n, Found { nodes: vec![n], ..Default::default() }, c.unwrap_or(Criterion::OfficialWithStructure)));
                    by_node.len() - 1
                }
            };
            let entry = &mut by_node[k];
            let p = if declared { Provenance::FromSource } else { Provenance::Deduced(c.unwrap_or(Criterion::OneSource)) };
            let mut vals = vec![text(&e.display_name)];
            if let Some(v) = &e.version {
                vals.push(text(v));
            }
            entry.1.identity.push(fact(if declared { "fatto-programma-installato" } else { "fatto-programma-indizio" }, p, Source::Uninstall(e.hive), vals));
            if let Some(kb) = e.estimated_kb {
                entry.1.identity.push(fact("fatto-dimensione-dichiarata", Provenance::FromSource, Source::Uninstall(e.hive), vec![Value::Bytes(u64::from(kb) * 1024)]));
            }
            if entry.1.args.is_empty() {
                entry.1.args.push(("nome", text(&e.display_name)));
                if let Some(v) = &e.version {
                    entry.1.args.push(("versione", text(v)));
                }
            }
            // La fonte migliore decide: una voce da fonte vale più di un indizio.
            let new_c = c.unwrap_or(Criterion::OfficialWithStructure);
            if new_c.level() > entry.2.level() || (c.is_none() && entry.1.attribution != Some(Provenance::FromSource)) {
                entry.2 = new_c;
            }
            if c.is_none() {
                entry.1.attribution = Some(Provenance::FromSource);
            }
        }
        by_node
            .into_iter()
            .map(|(_, mut f, c)| {
                if f.attribution.is_none() {
                    f.attribution = Some(Provenance::Deduced(c));
                }
                f.criterion = Some(c);
                f
            })
            .collect()
    }

    /// Ipotesi dal nome (03 §6.2): cartelle di primo livello in Local, Roaming e ProgramData con il nome
    /// dell'editore di un programma installato. Grigio "ipotesi", nessuna proposta.
    fn name_hypotheses(&self, claimed: &dyn Fn(u32) -> bool) -> Vec<(u32, String)> {
        let Ok(u) = &self.ev.uninstall else { return Vec::new() };
        let mut names: Vec<String> = Vec::new();
        for e in &u.entries {
            for cand in [e.publisher.as_deref().unwrap_or(""), e.display_name.split_whitespace().next().unwrap_or("")] {
                let c = cand.trim().trim_end_matches(',').to_owned();
                if c.chars().count() >= 4 && !names.iter().any(|x| x.eq_ignore_ascii_case(&c)) {
                    names.push(c);
                }
            }
        }
        let mut out = Vec::new();
        for base in [self.ev.folders.local.as_deref(), self.ev.folders.roaming.as_deref(), self.ev.folders.program_data.as_deref()].into_iter().flatten() {
            let Some(b) = find(self.tree, base) else { continue };
            for c in self.tree.children(b) {
                if !self.tree.is_dir(c) || claimed(c) {
                    continue;
                }
                let n = self.tree.name_lossy(c);
                if let Some(x) = names.iter().find(|x| x.eq_ignore_ascii_case(&n)) {
                    out.push((c, x.clone()));
                }
            }
        }
        out
    }

    fn msix(&self, rule: &Rule) -> Vec<Found> {
        let Ok(pkgs) = &self.ev.msix else { return Vec::new() };
        let Some(local) = self.ev.folders.local.as_deref() else { return Vec::new() };
        let Some(packages) = find(self.tree, &extract::join(local, "Packages")) else { return Vec::new() };
        let official = self.official(rule);
        let c = if official { Criterion::OfficialWithStructure } else { Criterion::StructureOnly };
        let mut out = Vec::new();
        for p in pkgs.iter().filter(|p| !p.framework) {
            let Some(n) = child(self.tree, packages, &p.family_name) else { continue };
            let mut f = Found { nodes: vec![n], attribution: Some(Provenance::Deduced(c)), criterion: Some(c), ..Default::default() };
            f.identity.push(fact("fatto-pacchetto-msix", Provenance::FromSource, Source::Msix, vec![text(&p.family_name)]));
            f.args.push(("nome", text(&p.family_name)));
            out.push(f);
        }
        out
    }

    fn tool_cache(&self, rule: &Rule, tp: &ToolPath, key: &str) -> Vec<Found> {
        let Some(n) = find(self.tree, &tp.path) else { return Vec::new() };
        let structure = self.structure(rule, n);
        let (attribution, criterion) = match tp.origin {
            ConfigOrigin::Default => (Provenance::Deduced(structure), structure),
            ConfigOrigin::Environment | ConfigOrigin::Npmrc if structure != Criterion::NameOnly => (Provenance::FromSource, structure),
            _ => (Provenance::Deduced(structure), structure),
        };
        let origin = match tp.origin {
            ConfigOrigin::Environment => "ambiente",
            ConfigOrigin::Npmrc => "npmrc",
            ConfigOrigin::Default => "predefinito",
        };
        let mut f = Found { nodes: vec![n], attribution: Some(attribution), criterion: Some(criterion), ..Default::default() };
        let p = if tp.origin == ConfigOrigin::Default { Provenance::Deduced(Criterion::OfficialWithStructure) } else { Provenance::FromSource };
        f.identity.push(fact(key, p, Source::NpmConfig, vec![text(&tp.path), text(origin)]));
        f.args.push(("dimensione", Value::Bytes(self.tree.alloc(n))));
        vec![f]
    }

    /// Cartelle di `escludi` della regola presenti nell'albero (codice di una cartella nota, con un percorso
    /// relativo facoltativo: `profilo:.vscode`).
    fn excluded(&self, rule: &Rule) -> Vec<u32> {
        let mut out = Vec::new();
        for e in &rule.place.escludi {
            let (code, rel) = e.split_once(':').map_or((e.as_str(), None), |(c, r)| (c, Some(r)));
            let Some(base) = self.ev.folders.get(code, &self.ev.volume_root) else { continue };
            let path = match rel {
                Some(r) => format!("{}\\{}", base.trim_end_matches('\\'), r.replace('/', "\\")),
                None => base.to_owned(),
            };
            out.extend(find(self.tree, &path));
        }
        out
    }

    fn lockfile(&self, rule: &Rule) -> Vec<Found> {
        let t = self.tree;
        let pairs = &rule.place.coppie;
        let mut matched: Vec<u32> = Vec::new();
        let mut out = Vec::new();
        // Le cache dei gestori di pacchetti contengono progetti temporanei con il loro lockfile (`npx`
        // in `npm-cache\_npx`): sono della cache, non tuoi. Sotto AppData, le cartelle dei programmi e le
        // estensioni di VS Code una `node_modules` fa parte di un programma: senza il progetto non si
        // ricostruisce.
        let mut skip: Vec<u32> = [&self.ev.npm_cache.path, &self.ev.pnpm_store.path].into_iter().filter_map(|p| find(t, p)).collect();
        skip.extend(self.excluded(rule));
        // Le cartelle con uno dei nomi e accanto un lockfile del suo tipo, cercate in parallelo su tutti i nodi
        // (era più di metà della classificazione: 40-80 ms con C:); poi, in ordine di indice (i genitori prima
        // dei figli), solo le più esterne.
        let candidates: Vec<(u32, usize, usize)> = (1..t.len() as u32)
            .into_par_iter()
            .filter_map(|i| {
                let fl = t.flags(i);
                if !fl.has(Flags::DIR) || fl.any(Flags::DETACHED | Flags::NOT_FOLLOWED) {
                    return None;
                }
                let k = pairs.iter().position(|c| name_is(t.name(i), &c.nome))?;
                let lock = first_lockfile(t, t.parent(i), &pairs[k].lockfile)?;
                Some((i, k, lock))
            })
            .collect();
        for (i, k, lock) in candidates {
            let (nm, lock) = (&pairs[k].nome, &pairs[k].lockfile[lock]);
            // Solo la cartella più esterna: una `node_modules` dentro un'altra fa parte della prima.
            if matched.iter().any(|&m| is_ancestor(t, m, i)) || skip.iter().any(|&c| c == i || is_ancestor(t, c, i)) {
                continue;
            }
            matched.push(i);
            let c = if self.official(rule) { Criterion::OfficialWithStructure } else { Criterion::StructureOnly };
            let mut f = Found { nodes: vec![i], attribution: Some(Provenance::Deduced(c)), criterion: Some(c), ..Default::default() };
            f.identity.push(fact("fatto-cartella-ricostruibile", Provenance::Deduced(c), Source::Rule, vec![text(nm), text(lock)]));
            f.manageability.push(fact("fatto-lockfile-presente", Provenance::Measured, Source::Rule, vec![text(lock)]));
            f.checks.push(Check { key: "controllo-lockfile".into(), kind: CheckKind::Automatic, outcome: CheckOutcome::Passed });
            f.args.push(("nome", text(nm)));
            f.args.push(("lockfile", text(lock)));
            f.args.push(("comando", Value::Text(rebuild_command(nm, lock))));
            out.push(f);
        }
        out
    }

    fn user_folders(&self, _rule: &Rule) -> Vec<Found> {
        let mut out = Vec::new();
        for (code, path) in self.ev.folders.user_folders() {
            let Some(p) = path else { continue };
            let Some(n) = find(self.tree, p) else { continue };
            let mut f = Found { nodes: vec![n], attribution: Some(Provenance::FromSource), criterion: Some(Criterion::OfficialWithStructure), ..Default::default() };
            f.identity.push(fact("fatto-cartella-nota", Provenance::FromSource, Source::KnownFolder, vec![text(code)]));
            f.args.push(("cartella", text(code)));
            out.push(f);
        }
        out
    }

    fn run(&self, idx: usize) -> Vec<Found> {
        let rule = &self.kb.rules[idx];
        match rule.recognizer {
            Recognizer::Path => self.path_rule(rule),
            Recognizer::CapCutVersions => self.capcut_versions(rule),
            Recognizer::RecycleBin => self.recycle_bin(rule),
            Recognizer::RecycleOrphans => self.recycle_orphans(rule),
            Recognizer::Steam => self.steam(rule),
            Recognizer::Epic => self.epic(rule),
            Recognizer::Registered => self.registered(rule),
            Recognizer::Msix => self.msix(rule),
            Recognizer::NpmCache => self.tool_cache(rule, &self.ev.npm_cache, "fatto-percorso-cache-npm"),
            Recognizer::PnpmStore => self.tool_cache(rule, &self.ev.pnpm_store, "fatto-percorso-store-pnpm"),
            Recognizer::Lockfile => self.lockfile(rule),
            Recognizer::UserFolders => self.user_folders(rule),
            Recognizer::NameHypothesis => Vec::new(),
        }
    }
}

/// Il comando che di solito ricrea la cartella (deduzione detta come tale nel testo della riga): il gestore
/// dei pacchetti viene dal lockfile, `install` per le dipendenze, `run build` per `.next`, `cargo build` per
/// `target`.
fn rebuild_command(name: &str, lock: &str) -> String {
    let tool = match lock.to_ascii_lowercase().as_str() {
        "cargo.lock" => return "cargo build".into(),
        "pnpm-lock.yaml" => "pnpm",
        "yarn.lock" => "yarn",
        "bun.lockb" => "bun",
        _ => "npm",
    };
    if name.eq_ignore_ascii_case(".next") { format!("{tool} run build") } else { format!("{tool} install") }
}

/// Secondi Unix in FILETIME; `None` per valori negativi o fuori scala (manifest malformato).
fn unix_to_filetime(secs: i64) -> Option<i64> {
    if secs < 0 {
        return None;
    }
    secs.checked_add(11_644_473_600)?.checked_mul(10_000_000)
}

// ---------------------------------------------------------------------------------------------
// Record

fn step(rule: &Rule, key: String, p: &Passo, uri: Option<&String>) -> Step {
    let _ = rule;
    Step { key, command: p.comando.clone(), uri: uri.cloned().or_else(|| p.uri.clone()) }
}

/// Abbassa ad affidabilità bassa i fatti dedotti (regola scaduta, versione fuori intervallo, conflitto).
fn lower(facts: &mut [Fact], c: Criterion) {
    for f in facts {
        if let Provenance::Deduced(_) = f.provenance {
            f.provenance = Provenance::Deduced(c);
        }
    }
}

fn build(rule: &Rule, mut f: Found, tree: &Tree, today: Date) -> (Record, Provenance, Vec<(&'static str, Value)>, Vec<u32>) {
    let expiry = Expiry::at(rule.verified, today);
    let mut criterion = f.criterion.unwrap_or(Criterion::NameOnly);
    let mut attribution = f.attribution.unwrap_or(Provenance::Deduced(criterion));
    let mut unknown = f.unknown;
    if expiry == Expiry::Expired {
        unknown = Some(UnknownReason::RuleExpired);
        criterion = Criterion::RuleExpired;
    }
    if unknown.is_none() && criterion.level() == quado_model::record::Reliability::Low {
        unknown = Some(UnknownReason::NameOnly);
    }
    if let Some(u) = unknown {
        let c = match u {
            UnknownReason::RuleExpired => Criterion::RuleExpired,
            UnknownReason::Conflict => Criterion::Conflict,
            UnknownReason::VersionOutOfRange => Criterion::VersionOutOfRange,
            _ => criterion,
        };
        criterion = c;
        lower(&mut f.identity, c);
        lower(&mut f.manageability, c);
        if let Provenance::Deduced(_) = attribution {
            attribution = Provenance::Deduced(c);
        } else if matches!(u, UnknownReason::RuleExpired | UnknownReason::Conflict) {
            // Anche un'attribuzione da fonte passa al grigio quando la regola non vale più.
            attribution = Provenance::Deduced(c);
        }
    }
    let proposing = unknown.is_none();
    let state = if let Some(u) = unknown {
        State::Unknown(u)
    } else {
        match rule.state {
            StateCode::Manageable | StateCode::ToVerify => match ProvaDiGestibilita::new(std::mem::take(&mut f.manageability)) {
                Some(p) if rule.state == StateCode::Manageable => State::Manageable(p),
                Some(p) => State::ToVerify(p),
                None => State::Unknown(UnknownReason::NoManageabilityProof),
            },
            StateCode::YouDecide => State::YouDecide,
            StateCode::ManagedBy => State::ManagedBy { by: rule.managed_by.clone().unwrap_or_default(), procedure: rule.action != ActionCode::None },
            StateCode::ManagedByNoProcedure => State::ManagedBy { by: rule.managed_by.clone().unwrap_or_default(), procedure: false },
        }
    };
    let has_action = proposing && !matches!(state, State::Unknown(_));
    let action = if !has_action {
        Action::None
    } else {
        match rule.action {
            ActionCode::Recycle => Action::Recycle,
            ActionCode::Procedure => Action::Procedure,
            ActionCode::Command => Action::Command,
            ActionCode::Uninstall => Action::Uninstall,
            ActionCode::None => Action::None,
        }
    };
    let (preferred, alternatives) = if action == Action::None {
        (None, Vec::new())
    } else {
        let preferred = rule.method.as_ref().map(|p| step(rule, rule.key("metodo"), p, f.uri.as_ref()));
        let alts = rule.alternatives.iter().enumerate().map(|(i, p)| step(rule, rule.key(&format!("alternativa-{}", i + 1)), p, None)).collect();
        (preferred, alts)
    };
    let mut checks: Vec<Check> = Vec::new();
    if action != Action::None {
        for c in &rule.checks {
            let key = format!("controllo-{c}");
            match f.checks.iter().position(|x| x.key == key) {
                Some(k) => checks.push(f.checks.remove(k)),
                None => checks.push(Check { key, kind: CheckKind::Automatic, outcome: CheckOutcome::AtSimulation }),
            }
        }
        for c in &rule.manual_checks {
            checks.push(Check { key: rule.key(&format!("controllo-{c}")), kind: CheckKind::Manual, outcome: CheckOutcome::ToTick });
        }
    }
    let estimate = if action == Action::None {
        Estimate::NotEstimable(NotEstimable::NoAction)
    } else {
        match rule.estimate {
            EstimateCode::Exact => f.estimate.unwrap_or_else(|| {
                let incomplete = f.nodes.iter().any(|&n| tree.flags(n).any(Flags::DENIED | Flags::PARTIAL | Flags::ERROR));
                if incomplete { Estimate::NotEstimable(NotEstimable::AtSimulation) } else { Estimate::Exact(f.nodes.iter().map(|&n| tree.alloc(n)).sum()) }
            }),
            EstimateCode::NotEstimableLinks => Estimate::NotEstimable(NotEstimable::HardLinksNotCounted),
            EstimateCode::NotEstimableProcedure => Estimate::NotEstimable(NotEstimable::ExternalProcedure),
            EstimateCode::None => Estimate::NotEstimable(NotEstimable::NoAction),
        }
    };
    let mut risks: Vec<String> = (1..=rule.risks).map(|i| rule.key(&format!("rischio-{i}"))).collect();
    risks.extend(f.risks);
    let record = Record {
        protected_exception: rule.protected_exception && matches!(state, State::ToVerify(_)),
        state,
        action,
        identity: ProvaDiIdentita(f.identity),
        hypotheses: (1..=rule.hypotheses).map(|i| rule.key(&format!("ipotesi-{i}"))).collect(),
        reliability: criterion,
        risks,
        preferred,
        alternatives,
        checks,
        estimate,
        reversibility: format!("reversibilita-{}", rule.reversibility),
        privileges: rule.privileges,
        on_error: format!("errore-{}", rule.on_error),
        after: format!("verifica-{}", rule.after),
        origin: Origin::Rule(RuleRef { id: rule.id.clone(), verified: rule.verified }),
        expiry,
    };
    (record, attribution, f.args, f.nodes)
}

/// Voce della card "Interventi possibili" (F-OV-4): una per regola, solo da record che propongono
/// un'azione. Mai Non sappiamo, mai la variante "Nessuna procedura", mai le cartelle orfane del
/// Cestino; mai voci da dimensione, età, nome o estensione; mai le regole con `card = false`. Le
/// disinstallazioni stanno nella vista Programmi (M4), non nella card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Intervention {
    pub rule: usize,
    pub matches: Vec<usize>,
    /// Allocata degli elementi (la misura, non la stima).
    pub bytes: u64,
    pub estimate: Estimate,
    /// Codice dello stato (uguale per tutte le corrispondenze della voce).
    pub state: &'static str,
}

pub fn interventions(kb: &Kb, cls: &Classification, tree: &Tree) -> Vec<Intervention> {
    let mut out: Vec<Intervention> = Vec::new();
    for (k, m) in cls.matches.iter().enumerate() {
        let r = &m.record;
        if !r.state.proposes() || matches!(r.action, Action::None | Action::Uninstall) {
            continue;
        }
        // Regole fuori dalla card: configurazione di Windows (ibernazione), voci che liberano poco (WinSxS,
        // cache dei browser). Restano nell'ispettore.
        if kb.rules.get(m.rule).is_some_and(|rule| !rule.in_card) {
            continue;
        }
        let bytes: u64 = m.nodes.iter().map(|&n| cls.own_bytes(tree, n)).sum();
        match out.iter_mut().find(|i| i.rule == m.rule && i.state == r.state.code()) {
            Some(i) => {
                i.matches.push(k);
                i.bytes += bytes;
                i.estimate = sum_estimates(i.estimate, r.estimate);
            }
            None => out.push(Intervention { rule: m.rule, matches: vec![k], bytes, estimate: r.estimate, state: r.state.code() }),
        }
    }
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.rule.cmp(&b.rule)));
    out
}

/// Somma di due stime della stessa voce: esatte danno un'esatta, con un intervallo si sommano minimi e
/// massimi; una non stimabile rende non stimabile la somma.
pub fn sum_estimates(a: Estimate, b: Estimate) -> Estimate {
    let bounds = |e: Estimate| match e {
        Estimate::Exact(x) => Some((x, x)),
        Estimate::Range { min, max } => Some((min, max)),
        Estimate::NotEstimable(_) => None,
    };
    match (a, b) {
        (Estimate::Exact(x), Estimate::Exact(y)) => Estimate::Exact(x.saturating_add(y)),
        (Estimate::NotEstimable(x), _) | (_, Estimate::NotEstimable(x)) => Estimate::NotEstimable(x),
        _ => match (bounds(a), bounds(b)) {
            (Some((a0, a1)), Some((b0, b1))) => Estimate::Range { min: a0.saturating_add(b0), max: a1.saturating_add(b1) },
            _ => a,
        },
    }
}

/// Record per un elemento senza regola: Non sappiamo, solo misure e "Apri in Esplora file" (F-IN-4).
pub fn unknown_record() -> Record {
    Record {
        state: State::Unknown(UnknownReason::NoRule),
        action: Action::None,
        identity: ProvaDiIdentita::default(),
        hypotheses: Vec::new(),
        reliability: Criterion::NameOnly,
        risks: Vec::new(),
        preferred: None,
        alternatives: Vec::new(),
        checks: Vec::new(),
        estimate: Estimate::NotEstimable(NotEstimable::NoAction),
        reversibility: "reversibilita-nessuna-azione".into(),
        privileges: quado_model::record::Privileges::None,
        on_error: "errore-nessuna-azione".into(),
        after: "verifica-nessuna-azione".into(),
        origin: Origin::NoRule,
        expiry: Expiry::Fresh,
        protected_exception: false,
    }
}

/// Indice della regola delle ipotesi dal nome (riconoscimento senza fonte, mai una proposta).
pub const HYPOTHESIS_RULE: &str = "ipotesi-dal-nome";

/// Classifica l'albero. `today` è la data locale (scadenza delle regole).
pub fn classify(kb: &Kb, tree: &Tree, ev: &Evidence, today: Date) -> Classification {
    let ctx = Ctx { kb, tree, ev };
    let mut matches: Vec<Match> = Vec::new();
    for (idx, rule) in kb.rules.iter().enumerate() {
        if rule.id == HYPOTHESIS_RULE {
            continue;
        }
        for f in ctx.run(idx) {
            if f.nodes.is_empty() {
                continue;
            }
            let (record, attribution, args, nodes) = build(rule, f, tree, today);
            matches.push(Match { rule: idx, nodes, manager: rule.manager, attribution, args, record, others: Vec::new() });
        }
    }

    // Un elemento, una corrispondenza: concordi -> vale la regola più specifica (non contenitore),
    // discordi -> conflitto.
    let mut own: HashMap<u32, u32> = HashMap::new();
    for k in 0..matches.len() {
        let nodes = matches[k].nodes.clone();
        for n in nodes {
            match own.get(&n).copied() {
                None => {
                    own.insert(n, k as u32);
                }
                Some(j) => {
                    let j = j as usize;
                    let (a, b) = (&matches[j], &matches[k]);
                    let agree = a.manager == b.manager && a.record.state.code() == b.record.state.code();
                    let (keep, drop) = if agree {
                        if kb.rules[a.rule].container && !kb.rules[b.rule].container { (k, j) } else { (j, k) }
                    } else {
                        (j, k)
                    };
                    let dropped_rule = matches[drop].rule;
                    if !matches[keep].others.contains(&dropped_rule) {
                        matches[keep].others.push(dropped_rule);
                    }
                    if !agree {
                        for m in [keep, drop] {
                            let r = &mut matches[m].record;
                            r.state = State::Unknown(UnknownReason::Conflict);
                            r.action = Action::None;
                            r.preferred = None;
                            r.alternatives.clear();
                            r.checks.clear();
                            r.estimate = Estimate::NotEstimable(NotEstimable::NoAction);
                            r.reliability = Criterion::Conflict;
                            lower(&mut r.identity.0, Criterion::Conflict);
                            matches[m].attribution = Provenance::Deduced(Criterion::Conflict);
                        }
                    }
                    own.insert(n, keep as u32);
                    matches[drop].nodes.retain(|&x| x != n);
                }
            }
        }
    }

    // Ipotesi dal nome, solo dove nessuna regola ha riconosciuto l'elemento, un suo antenato o un suo
    // discendente (una cartella con parti riconosciute ha già le sue spiegazioni).
    if let Some(hidx) = kb.index(HYPOTHESIS_RULE) {
        let mut around: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for &n in own.keys() {
            let mut a = n;
            while a != NONE && around.insert(a) {
                a = tree.parent(a);
            }
        }
        let claimed_ancestor = |c: u32| -> bool {
            if around.contains(&c) {
                return true;
            }
            let mut n = c;
            while n != NONE {
                if own.contains_key(&n) {
                    return true;
                }
                n = tree.parent(n);
            }
            false
        };
        for (n, name) in ctx.name_hypotheses(&claimed_ancestor) {
            let rule = &kb.rules[hidx];
            let mut f = Found { nodes: vec![n], criterion: Some(Criterion::NameOnly), attribution: Some(Provenance::Deduced(Criterion::NameOnly)), ..Default::default() };
            f.identity.push(fact("fatto-ipotesi-dal-nome", Provenance::Deduced(Criterion::NameOnly), Source::Uninstall(quado_model::record::Hive::Machine), vec![text(&name)]));
            f.args.push(("nome", text(&name)));
            let (record, attribution, args, nodes) = build(rule, f, tree, today);
            own.insert(n, matches.len() as u32);
            matches.push(Match { rule: hidx, nodes, manager: rule.manager, attribution, args, record, others: Vec::new() });
        }
    }

    // Le corrispondenze rimaste senza elementi (unite a una regola più specifica) si tolgono.
    let mut remap = vec![NONE; matches.len()];
    let mut kept: Vec<Match> = Vec::with_capacity(matches.len());
    for (k, m) in matches.into_iter().enumerate() {
        if !m.nodes.is_empty() {
            remap[k] = kept.len() as u32;
            kept.push(m);
        }
    }
    let matches = kept;
    for v in own.values_mut() {
        *v = remap[*v as usize];
    }

    // Proprietario di ogni nodo: il riconosciuto più vicino risalendo (i figli hanno indici maggiori).
    // Prima gli elementi riconosciuti, poi un passaggio in avanti: niente ricerche per nodo.
    let mut inherited = vec![NONE; tree.len()];
    for (&n, &m) in &own {
        inherited[n as usize] = m;
    }
    for i in 1..tree.len() {
        if inherited[i] == NONE {
            let p = tree.parent(i as u32);
            if p != NONE {
                inherited[i] = inherited[p as usize];
            }
        }
    }

    // Bilancio per gestore: ogni elemento riconosciuto porta i suoi byte meno quelli degli elementi
    // riconosciuti al suo interno.
    // Somme con segno: l'ordine degli elementi non conta, si chiude a fine giro.
    let mut acc = [0i128; 4];
    let mut total_claimed: i128 = 0;
    let mut nested: HashMap<u32, u64> = HashMap::new();
    for (&n, &m) in &own {
        if tree.flags(n).has(Flags::DETACHED) {
            continue;
        }
        let a = i128::from(tree.alloc(n));
        acc[ByManager::slot(matches[m as usize].color())] += a;
        let p = tree.parent(n);
        let outer = if p == NONE { NONE } else { inherited[p as usize] };
        if outer != NONE {
            acc[ByManager::slot(matches[outer as usize].color())] -= a;
            let mut up = p;
            while up != NONE && !own.contains_key(&up) {
                up = tree.parent(up);
            }
            if up != NONE {
                *nested.entry(up).or_default() += tree.alloc(n);
            }
        } else {
            total_claimed += a;
        }
    }
    acc[3] += i128::from(tree.alloc(Tree::ROOT)) - total_claimed;
    let v = |i: usize| acc[i].max(0) as u64;
    let by = ByManager { windows: v(0), programs: v(1), you: v(2), unclassified: v(3) };

    let recheck = kb.rules.iter().enumerate().filter(|(_, r)| Expiry::at(r.verified, today) != Expiry::Fresh).map(|(i, _)| i).collect();
    Classification { today: Some(today), matches, inherited, own, nested, by_manager: by, recheck }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quado_model::NodeData;

    fn w(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn find_and_patterns() {
        let mut t = Tree::with_root(&w("C:\\"), 0);
        let d = NodeData { flags: Flags::DIR, ..Default::default() };
        let u = t.push(Tree::ROOT, &w("Users"), d);
        let a = t.push(u, &w("Ospite"), d);
        let apps = t.push(a, &w("Apps"), d);
        let v1 = t.push(apps, &w("9.4.0.4015"), d);
        t.push(apps, &w("altro"), d);
        let th = t.push(a, &w("thumbcache_256.db"), NodeData::default());
        assert_eq!(find(&t, r"c:\users\ospite\APPS"), Some(apps));
        assert_eq!(find(&t, r"C:\Users\ospite\Apps\"), Some(apps));
        assert_eq!(find(&t, r"D:\Users"), None);
        assert_eq!(match_pattern(&t, a, "Apps/<versione>", None), vec![v1]);
        assert_eq!(match_pattern(&t, a, "thumbcache_*.db", None), vec![th]);
        assert!(glob("thumbcache_idx.db", "thumbcache_*.db"));
        assert!(!glob("thumbcache.db2", "thumbcache_*.db"));
    }

    #[test]
    fn estimates_sum_with_ranges() {
        let r = |min, max| Estimate::Range { min, max };
        assert_eq!(sum_estimates(Estimate::Exact(5), Estimate::Exact(7)), Estimate::Exact(12));
        assert_eq!(sum_estimates(Estimate::Exact(5), r(1, 3)), r(6, 8));
        assert_eq!(sum_estimates(r(1, 3), Estimate::Exact(5)), r(6, 8));
        assert_eq!(sum_estimates(r(1, 3), r(10, 20)), r(11, 23));
        let ne = Estimate::NotEstimable(NotEstimable::AtSimulation);
        assert_eq!(sum_estimates(r(1, 3), ne), ne);
        assert_eq!(sum_estimates(ne, Estimate::Exact(1)), ne);
    }
}
