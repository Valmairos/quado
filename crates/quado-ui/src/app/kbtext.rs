// SPDX-License-Identifier: GPL-3.0-or-later
//! Testi e segni della classificazione: stati con icona e parola, provenienza dei fatti con glifo ed
//! etichetta, righe delle regole. Tutto dai file di lingua: il core dà codici e valori, qui diventano frasi.

use quado_kb::{Classification, Kb, Match};
use quado_model::record::{
    Action, CheckOutcome, ColorGroup, Criterion, Estimate, Fact, Manager, NotEstimable, Privileges, Provenance, Record, Reliability, State, UnknownReason,
    Value,
};

use crate::format::Formatter;
use crate::i18n::{Arg, Lingue};
use crate::icons::Icon;
use crate::tokens::Gestore;
use crate::widgets::Evidence;

pub fn gestore(g: ColorGroup) -> Gestore {
    match g {
        ColorGroup::Manager(Manager::Programs) => Gestore::App,
        ColorGroup::Manager(Manager::You) => Gestore::Mine,
        ColorGroup::Manager(Manager::Windows) => Gestore::Windows,
        ColorGroup::Unclassified => Gestore::Unknown,
    }
}

pub fn manager_gestore(m: Manager) -> Gestore {
    gestore(ColorGroup::Manager(m))
}

/// Gestore e colore di un nodo.
pub fn node_gestore(cls: &Classification, node: u32) -> Gestore {
    gestore(cls.color(node))
}

pub fn evidence(p: Provenance) -> Evidence {
    match p {
        Provenance::Measured => Evidence::Measured,
        Provenance::FromSource => Evidence::Source,
        Provenance::Deduced(_) => Evidence::Inferred,
        Provenance::Unavailable => Evidence::Unavailable,
    }
}

/// L'attribuzione al gestore non basta per il colore: si mostra come "ipotesi".
pub fn is_hypothesis(m: &Match) -> bool {
    !m.attribution.colors()
}

pub fn state_icon(s: &State) -> Icon {
    match s {
        State::Manageable(_) => Icon::StateManageable,
        State::ToVerify(_) => Icon::StateToVerify,
        State::YouDecide => Icon::StateYouDecide,
        State::ManagedBy { .. } => Icon::StateManagedBy,
        State::Unknown(_) => Icon::StateUnknown,
    }
}

pub fn state_label(l: &Lingue, s: &State) -> String {
    match s {
        State::Manageable(_) => l.t("stato-gestibile"),
        State::ToVerify(_) => l.t("stato-da-verificare"),
        State::YouDecide => l.t("stato-decidi-tu"),
        State::ManagedBy { by, procedure: true } => l.ta("stato-lo-gestisce", &[("chi", by.clone().into())]),
        State::ManagedBy { by, procedure: false } => l.ta("stato-lo-gestisce-nessuna-procedura", &[("chi", by.clone().into())]),
        State::Unknown(_) => l.t("stato-non-sappiamo"),
    }
}

/// Perché lo stato è Non sappiamo: il cambio di stato ha sempre il suo testo.
pub fn unknown_reason(l: &Lingue, r: UnknownReason, verified: Option<String>) -> String {
    match r {
        UnknownReason::NoRule => l.t("motivo-stato-nessuna-regola"),
        UnknownReason::VersionOutOfRange => l.t("motivo-stato-versione"),
        UnknownReason::RuleExpired => l.ta("motivo-stato-regola-scaduta", &[("data", verified.unwrap_or_default().into())]),
        UnknownReason::Conflict => l.t("motivo-stato-conflitto"),
        UnknownReason::NameOnly => l.t("motivo-stato-solo-nome"),
        UnknownReason::NoManageabilityProof => l.t("motivo-stato-senza-prova"),
    }
}

/// Il record ha fatti dedotti, di identità o di gestibilità: solo allora la riga dell'affidabilità dice
/// qualcosa. Le voci della lente Download senza fatti dedotti non la mostrano.
pub fn has_deduced(r: &Record) -> bool {
    let proof = r.state.manageability().map(|p| p.facts()).unwrap_or_default();
    r.identity.0.iter().chain(proof).any(|f| matches!(f.provenance, Provenance::Deduced(_)))
}

pub fn reliability_label(l: &Lingue, c: Criterion) -> String {
    let level = match c.level() {
        Reliability::High => l.t("affidabilita-alta"),
        Reliability::Medium => l.t("affidabilita-media"),
        Reliability::Low => l.t("affidabilita-bassa"),
    };
    let crit = match c {
        Criterion::TwoSources => "criterio-due-fonti",
        Criterion::OfficialWithStructure => "criterio-fonte-ufficiale",
        Criterion::OneSource => "criterio-una-fonte",
        Criterion::StructureOnly => "criterio-struttura",
        Criterion::NameOnly => "criterio-nome",
        Criterion::VersionOutOfRange => "criterio-versione",
        Criterion::RuleExpired => "criterio-scaduta",
        Criterion::Conflict => "criterio-conflitto",
    };
    l.ta("affidabilita-riga", &[("livello", level.into()), ("criterio", l.t(crit).into())])
}

pub fn action_label(l: &Lingue, a: &Action) -> String {
    l.t(&format!("azione-tipo-{}", a.code()))
}

pub fn privileges_label(l: &Lingue, p: Privileges) -> String {
    l.t(match p {
        Privileges::None => "privilegi-nessuno",
        Privileges::Admin => "privilegi-amministratore",
    })
}

pub fn check_outcome(l: &Lingue, o: CheckOutcome) -> String {
    l.t(match o {
        CheckOutcome::AtSimulation => "controllo-esito-simulazione",
        CheckOutcome::Passed => "controllo-esito-superato",
        CheckOutcome::Failed => "controllo-esito-non-superato",
        CheckOutcome::ToTick => "controllo-esito-da-spuntare",
    })
}

pub fn estimate_text(l: &Lingue, f: &Formatter, e: &Estimate) -> String {
    match e {
        Estimate::Exact(b) => l.ta("stima-esatta", &[("valore", f.size(*b).into())]),
        Estimate::Range { min, max } => l.ta("stima-intervallo", &[("minimo", f.size(*min).into()), ("massimo", f.size(*max).into())]),
        Estimate::NotEstimable(n) => l.t(match n {
            NotEstimable::HardLinksNotCounted => "stima-non-stimabile-link",
            NotEstimable::ExternalProcedure => "stima-non-stimabile-procedura",
            NotEstimable::NoAction => "stima-nessuna-azione",
            NotEstimable::AtSimulation => "stima-nella-simulazione",
        }),
    }
}

/// Segnaposto di un valore: i numeri due volte, `$x` per il plurale e `$x_testo` già scritto; le date
/// con l'ora a parte, `$x_ora` (ora del controllo di Restart Manager).
fn push_value(out: &mut Vec<(String, Arg<'static>)>, f: &Formatter, name: &str, v: &Value) {
    match v {
        Value::Text(s) => out.push((name.to_owned(), Arg::from(s.clone()))),
        Value::Bytes(b) => out.push((name.to_owned(), Arg::from(f.size(*b)))),
        Value::Count(n) => {
            out.push((name.to_owned(), Arg::Num(*n as f64)));
            out.push((format!("{name}_testo"), Arg::from(f.count(*n))));
        }
        Value::Days(n) => {
            out.push((name.to_owned(), Arg::Num(f64::from(*n))));
            out.push((format!("{name}_testo"), Arg::from(f.count(u64::from(*n)))));
        }
        Value::Time(t) => {
            out.push((name.to_owned(), Arg::from(f.date(*t))));
            out.push((format!("{name}_ora"), Arg::from(f.time(*t))));
        }
    }
}

pub fn ta_owned(l: &Lingue, key: &str, args: &[(String, Arg<'static>)]) -> String {
    let refs: Vec<(&str, Arg<'_>)> = args.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    l.ta(key, &refs)
}

/// Testo di un fatto: i valori sono `$a`, `$b`, `$c` nell'ordine; quelli mancanti valgono "".
pub fn fact_text(l: &Lingue, f: &Formatter, fact: &Fact) -> String {
    let mut args = Vec::new();
    for (k, name) in ["a", "b", "c"].iter().enumerate() {
        match fact.values.get(k) {
            Some(v) => push_value(&mut args, f, name, v),
            None => args.push(((*name).to_owned(), Arg::from(String::new()))),
        }
    }
    ta_owned(l, &fact.key, &args)
}

pub fn match_args(f: &Formatter, m: &Match) -> Vec<(String, Arg<'static>)> {
    let mut args = Vec::new();
    for (k, v) in &m.args {
        push_value(&mut args, f, k, v);
    }
    args
}

pub fn rule_name(l: &Lingue, kb: &Kb, rule: usize) -> String {
    l.t(&kb.rules[rule].key("nome"))
}

/// Riga di spiegazione di un elemento riconosciuto (top list, ispettore, card).
pub fn rule_line(l: &Lingue, f: &Formatter, kb: &Kb, m: &Match) -> String {
    ta_owned(l, &kb.rules[m.rule].key("riga"), &match_args(f, m))
}

/// Fatto con la sua fonte quando la dichiara Windows o un programma: due fatti uguali da due fonti
/// (la versione in uso di CapCut) si distinguono.
pub fn fact_with_source(l: &Lingue, f: &Formatter, fact: &Fact) -> String {
    let s = fact_text(l, f, fact);
    if fact.provenance == Provenance::FromSource {
        format!("{s} · {}", l.t(&format!("origine-{}", fact.source.code())))
    } else {
        s
    }
}

/// Come mostrare l'indirizzo di una fonte. Una pagina: l'indirizzo, che si copia. Un file che un programma
/// scrive sul PC (`file:///`): «File del programma sul tuo PC» e il percorso dalla cartella nota
/// (`%LOCALAPPDATA%\CapCut\...`), senza il nome di un utente. Un documento del repository (`docs/`): «Prova
/// fatta con Quado» e il documento nel codice sorgente. Restituisce l'etichetta, se serve, il testo da
/// mostrare e se ha senso copiarlo.
pub fn source_place(l: &Lingue, url: &str) -> (Option<String>, String, bool) {
    if let Some(rest) = url.strip_prefix("file:///") {
        return (Some(l.t("regole-fonte-file")), rest.replace("%20", " ").replace('/', "\\"), false);
    }
    if url.starts_with("docs/") {
        return (Some(l.t("regole-fonte-prova")), url.to_owned(), false);
    }
    (None, url.to_owned(), true)
}

/// Nome breve del gestore per la colonna "Gestito da".
pub fn short_key(g: Gestore) -> &'static str {
    match g {
        Gestore::App => "gestore-app-breve",
        Gestore::Mine => "gestore-tuoi-breve",
        Gestore::Windows => "gestore-windows-breve",
        Gestore::Unknown => "gestore-non-classificato-breve",
    }
}
