// SPDX-License-Identifier: GPL-3.0-or-later
//! Record della lente Download: ciò che si capisce di una cartella, con uno stato per voce. Nessuna
//! proposta nasce da nome, data o dimensione da soli.
//!
//! - Le voci nascono da funzioni di Quado (`Origin::Function`), non da regole: decidono solo da ciò
//!   che la scansione ha misurato e da ciò che è stato letto su richiesta in questa sessione
//!   ([`LensInput`]).
//! - Gestibile solo con una prova di gestibilità misurata: impronte uguali (copie identiche) e
//!   Restart Manager che non trova processi (download incompleti). Tutto il resto è Decidi tu,
//!   senza azione e senza cifra recuperabile: installatori, immagini disco, archivi con la cartella
//!   accanto, file della stessa dimensione non ancora confrontati o con contenuto diverso.
//! - Il suffisso " (N)" non entra mai nei record: nessun fatto porta il nome di un file.
//! - La prima voce della prova di identità è il riassunto della voce (la riga sotto il titolo): il tipo
//!   secondo l'estensione, oppure, per un installatore letto, la corrispondenza con i programmi installati
//!   o il motivo della lettura non riuscita.
//! - I fatti degli installatori sono quelli di `installers` (`info_facts`, `match_fact`): una sola
//!   sorgente per chiavi, provenienza e fonte.

use std::collections::{HashMap, HashSet};

use quado_model::downloads::{self, ArchiveCheck, FileUse, GroupComparison, Hashed, IdenticalGroup, Lens, LensFamily, LensItem};
use quado_model::record::{
    Action, Check, CheckKind, CheckOutcome, Criterion, Estimate, Expiry, Fact, Function, NotEstimable, Origin, Privileges, ProvaDiGestibilita,
    ProvaDiIdentita, Provenance, Record, Source, State, Step, Value,
};
use quado_model::{Flags, Tree};

use crate::extract::{ExtractError, FormatIssue, Uninstall};
use crate::installers::{self, InstallerInfo, InstallerKind, MatchOutcome};

/// Le funzioni che producono le voci della lente: ognuna ha il suo nome, `funzione-<codice>-nome`
/// ([`name_key`]), per l'elenco e per la card interventi.
pub const FUNCTIONS: [Function; 6] =
    [Function::SameSize, Function::IdenticalCopies, Function::Installer, Function::ArchiveWithFolder, Function::DiskImage, Function::IncompleteDownload];

/// Esito della lettura di un installatore per la voce.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstallerStatus {
    /// Dati di versione non ancora letti (voce unica per tutti).
    NotRead,
    /// Segnaposto cloud: non si legge, per non scaricarlo (voce unica per tutti).
    CloudOnly,
    /// Lettura non riuscita, con il motivo nei fatti.
    Failed,
    SameVersion,
    OtherVersion,
    NotInstalled,
    /// Letto, ma senza corrispondenza: non calcolata dal chiamante (voci di disinstallazione non lette) o
    /// non possibile (`installers::FACT_NOT_COMPARABLE`: né nome né codice di prodotto).
    NotMatched,
}

/// Uso di un download incompleto secondo Restart Manager.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UseStatus {
    /// Nessun processo: Gestibile (voce unica per tutti).
    Free,
    /// In uso: nessuna azione, con i processi.
    InUse,
    /// Restart Manager non ha risposto: Decidi tu senza proposta.
    Unknown,
    /// Non ancora controllato (voce unica per tutti).
    NotChecked,
}

/// Una voce di "Cosa riconosce Quado in questa cartella".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Finding {
    /// File della stessa dimensione: contenuto non confrontato, confronto non completo o non valido.
    SameSize,
    /// Copie con la stessa impronta: `keep` resta, le altre si possono spostare nel Cestino.
    IdenticalCopies { keep: u32 },
    /// Stessa dimensione, contenuto confrontato: nessuna copia tra questi file.
    DifferentContent,
    /// Installatori (una voce per file letto; una sola per i non letti e una per quelli nel cloud).
    Installer(InstallerStatus),
    /// Zip (`nodes[0]`) con la cartella `folder` accanto.
    ArchiveWithFolder { folder: u32 },
    /// Tutte le immagini disco della lente.
    DiskImages,
    /// Download incompleti.
    Incomplete(UseStatus),
}

impl Finding {
    /// Ordine a parità di byte.
    fn rank(self) -> u8 {
        match self {
            Self::IdenticalCopies { .. } => 0,
            Self::Incomplete(_) => 1,
            Self::SameSize => 2,
            Self::DifferentContent => 3,
            Self::Installer(_) => 4,
            Self::ArchiveWithFolder { .. } => 5,
            Self::DiskImages => 6,
        }
    }
}

/// Una voce con i suoi file, la sua cifra e il suo record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LensFinding {
    pub finding: Finding,
    /// File della voce, in ordine di nodo (le copie per data di creazione, la prima resta per default).
    pub nodes: Vec<u32>,
    /// Dimensioni su disco dei file della voce, ogni file contato una volta (la misura, non la stima:
    /// quella è in `record.estimate`).
    pub bytes: u64,
    pub record: Record,
}

/// Corrispondenza di un installatore con i programmi installati: l'esito di
/// `installers::match_installer` e il fatto di `installers::match_fact`, con la provenienza già decisa
/// (ProductCode uguale: da fonte; nome e versione: dedotto, con il suo criterio). La calcola chi chiama,
/// con le voci di disinstallazione delle prove (`Evidence::uninstall`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallerMatch {
    pub outcome: MatchOutcome,
    pub fact: Fact,
}

impl InstallerMatch {
    /// Corrispondenza di `info` con le voci di disinstallazione `programs`.
    pub fn compute(info: &InstallerInfo, programs: &Uninstall) -> Self {
        let outcome = installers::match_installer(info, programs);
        let fact = installers::match_fact(&outcome, info, programs);
        Self { outcome, fact }
    }
}

/// Lettura di un installatore su richiesta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallerRead {
    pub info: Result<InstallerInfo, ExtractError>,
    /// Solo se la lettura è riuscita; `None` se la corrispondenza non è stata calcolata.
    pub matched: Option<InstallerMatch>,
}

/// Controllo di Restart Manager su un download incompleto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UseCheck {
    pub usage: FileUse,
    /// Ora del controllo (`FILETIME`).
    pub at: i64,
}

/// Ciò che è stato letto su richiesta in questa sessione. Vuoto: la lente appena aperta, senza
/// nessuna lettura di contenuto.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LensInput {
    /// Impronte di "Confronta il contenuto", anche di un confronto annullato a metà: valgono
    /// solo i gruppi della stessa dimensione letti per intero (`downloads::compare_groups`).
    pub hashed: Vec<Hashed>,
    /// File da tenere scelti dall'utente (`downloads::with_keep`): in un gruppo di copie resta il primo,
    /// in ordine di creazione, che compare qui; senza scelta resta il più vecchio.
    pub keep: Vec<u32>,
    /// Installatori letti, per nodo.
    pub installers: HashMap<u32, InstallerRead>,
    /// Confronto tra indice dello zip e cartella (`downloads::compare_archive`), per nodo dell'archivio;
    /// l'errore è quello del lettore della directory centrale.
    pub archives: HashMap<u32, Result<ArchiveCheck, ExtractError>>,
    /// Uso dei download incompleti secondo Restart Manager, per nodo.
    pub usage: HashMap<u32, UseCheck>,
}

// ---------------------------------------------------------------------------------------------
// Fatti e record

fn fact(key: &str, p: Provenance, s: Source, v: Vec<Value>) -> Fact {
    Fact::new(key, p, s, v)
}

fn text(s: &str) -> Value {
    Value::Text(s.to_owned())
}

fn count(n: usize) -> Value {
    Value::Count(n as u64)
}

/// "N file di una famiglia secondo l'estensione": misurato dalla scansione (il nome), mai una prova di
/// gestibilità.
fn family_fact(f: LensFamily, n: usize) -> Fact {
    fact("fatto-lente-famiglia", Provenance::Measured, Source::Scan, vec![text(f.code()), count(n)])
}

/// Codice di errore di Windows come dato: le cifre senza separatori, come nell'interfaccia.
fn code(c: u32) -> Value {
    Value::Text(c.to_string())
}

/// Lettura non riuscita: `$a` codice del motivo, `$b` codice di errore di Windows o nome del campo.
fn error_fact(key: &str, source: Source, e: &ExtractError) -> Fact {
    let values = match e {
        ExtractError::NotPresent => vec![text("non-presente")],
        ExtractError::Io(5) => vec![text("accesso-negato")],
        ExtractError::Io(c) => vec![text("errore"), code(*c)],
        ExtractError::CloudPlaceholder => vec![text("solo-cloud")],
        ExtractError::Format(f) => match f {
            FormatIssue::Truncated => vec![text("troncato")],
            FormatIssue::Unexpected => vec![text("inatteso")],
            FormatIssue::Empty => vec![text("vuoto")],
            FormatIssue::TooDeep => vec![text("troppo-annidato")],
            FormatIssue::Encoding => vec![text("codifica")],
            FormatIssue::TooLarge => vec![text("troppo-grande")],
            FormatIssue::MissingField(x) => vec![text("campo-mancante"), text(x.code())],
            FormatIssue::WrongType(x) => vec![text("tipo-diverso"), text(x.code())],
            FormatIssue::BadValue(x) => vec![text("valore"), text(x.code())],
        },
    };
    fact(key, Provenance::Unavailable, source, values)
}

/// Livello dei fatti dedotti: il criterio più basso. Senza fatti dedotti (solo misure e fonti) il record
/// dice "struttura riconosciuta senza fonte": la lente riconosce estensioni, dimensioni e nomi accanto,
/// senza una regola con una fonte.
fn reliability(identity: &[Fact], proof: &[Fact]) -> Criterion {
    identity
        .iter()
        .chain(proof)
        .filter_map(|f| match f.provenance {
            Provenance::Deduced(c) => Some(c),
            _ => None,
        })
        .min_by_key(|c| c.level())
        .unwrap_or(Criterion::StructureOnly)
}

/// Decidi tu senza azione: nessuna cifra recuperabile, nessuna voce nella card interventi.
fn decide(function: Function, identity: Vec<Fact>, checks: Vec<Check>) -> Record {
    Record {
        state: State::YouDecide,
        action: Action::None,
        reliability: reliability(&identity, &[]),
        identity: ProvaDiIdentita(identity),
        hypotheses: Vec::new(),
        risks: Vec::new(),
        preferred: None,
        alternatives: Vec::new(),
        checks,
        estimate: Estimate::NotEstimable(NotEstimable::NoAction),
        reversibility: "reversibilita-nessuna-azione".into(),
        privileges: Privileges::None,
        on_error: "errore-nessuna-azione".into(),
        after: "verifica-nessuna-azione".into(),
        origin: Origin::Function(function),
        expiry: Expiry::Fresh,
        protected_exception: false,
    }
}

/// Gestibile con spostamento nel Cestino dopo l'anteprima. Senza una prova valida (non succede: le prove
/// della lente non sono mai misure della scansione) resta Decidi tu senza azione.
fn manageable(function: Function, identity: Vec<Fact>, proof: Vec<Fact>, estimate: Estimate, restart_manager: CheckOutcome) -> Record {
    let reliability = reliability(&identity, &proof);
    let Some(p) = ProvaDiGestibilita::new(proof) else { return decide(function, identity, Vec::new()) };
    let code = function.code();
    Record {
        state: State::Manageable(p),
        action: Action::Recycle,
        reliability,
        identity: ProvaDiIdentita(identity),
        hypotheses: Vec::new(),
        risks: vec![format!("funzione-{code}-rischio-1")],
        preferred: Some(Step { key: format!("funzione-{code}-metodo"), command: None, uri: None }),
        alternatives: Vec::new(),
        checks: vec![
            Check { key: "controllo-restart-manager".into(), kind: CheckKind::Automatic, outcome: restart_manager },
            Check { key: "controllo-capacita-cestino".into(), kind: CheckKind::Automatic, outcome: CheckOutcome::AtSimulation },
        ],
        estimate,
        reversibility: "reversibilita-cestino".into(),
        privileges: Privileges::None,
        on_error: "errore-per-elemento".into(),
        after: "verifica-riscansione".into(),
        origin: Origin::Function(function),
        expiry: Expiry::Fresh,
        protected_exception: false,
    }
}

/// Stima dello spostamento nel Cestino di `nodes`: allocata esatta; con file che hanno più
/// nomi (`Flags::MULTI_LINK`) un intervallo, perché lo spazio torna solo se tutti i nomi lasciano il
/// volume: minimo senza quei file, massimo con loro. La simulazione la ricalcola rileggendo gli attributi.
fn estimate_of(tree: &Tree, nodes: &[u32]) -> Estimate {
    let max: u64 = nodes.iter().map(|&n| tree.alloc(n)).sum();
    let min: u64 = nodes.iter().filter(|&&n| !tree.flags(n).has(Flags::MULTI_LINK)).map(|&n| tree.alloc(n)).sum();
    if min == max { Estimate::Exact(max) } else { Estimate::Range { min, max } }
}

/// Dimensioni su disco della voce, ogni file contato una volta (i nomi secondari degli hard link no).
fn bytes_of(tree: &Tree, nodes: &[u32]) -> u64 {
    nodes.iter().filter(|&&n| !tree.flags(n).has(Flags::SECONDARY_LINK)).map(|&n| tree.alloc(n)).sum()
}

fn voice(tree: &Tree, finding: Finding, nodes: Vec<u32>, record: Record) -> LensFinding {
    LensFinding { finding, bytes: bytes_of(tree, &nodes), nodes, record }
}

// ---------------------------------------------------------------------------------------------
// Voci

/// Voci della lente, per byte decrescenti; a parità per tipo di voce e per primo nodo. Funzione pura:
/// nessuna lettura, tutto da `tree`, `lens` e `input`.
pub fn findings(tree: &Tree, lens: &Lens, input: &LensInput) -> Vec<LensFinding> {
    // Una lente costruita su un altro albero non deve far cadere niente: si tengono i nodi validi.
    let filtered;
    let lens = if lens.items.iter().all(|i| (i.node as usize) < tree.len()) {
        lens
    } else {
        filtered = Lens { items: lens.items.iter().copied().filter(|i| (i.node as usize) < tree.len()).collect(), ..lens.clone() };
        &filtered
    };
    let mut out = Vec::new();
    copies(tree, lens, input, &mut out);
    installers(tree, lens, input, &mut out);
    archives(tree, lens, input, &mut out);
    disk_images(tree, lens, &mut out);
    incomplete(tree, lens, input, &mut out);
    out.sort_by(|a, b| {
        b.bytes.cmp(&a.bytes).then(a.finding.rank().cmp(&b.finding.rank())).then_with(|| a.nodes.cmp(&b.nodes))
    });
    out
}

/// File della stessa dimensione e copie identiche.
fn copies(tree: &Tree, lens: &Lens, input: &LensInput, out: &mut Vec<LensFinding>) {
    let groups = downloads::same_size_groups(lens);
    let comparisons = downloads::compare_groups(&groups, &input.hashed);
    let keep: HashSet<u32> = input.keep.iter().copied().collect();
    let mut created: HashMap<u32, i64> = HashMap::new();
    for h in &input.hashed {
        created.insert(h.node, h.created);
    }
    for (g, c) in groups.iter().zip(comparisons) {
        let size = |n: usize| fact("fatto-lente-stessa-dimensione", Provenance::Measured, Source::Scan, vec![count(n), Value::Bytes(g.logical)]);
        match c {
            GroupComparison::NotRead { read } => {
                let second = if read == 0 {
                    fact("fatto-lente-contenuto-non-confrontato", Provenance::Unavailable, Source::ContentHash, Vec::new())
                } else {
                    fact("fatto-lente-letti-in-parte", Provenance::Unavailable, Source::ContentHash, vec![count(read), count(g.nodes.len())])
                };
                out.push(voice(tree, Finding::SameSize, g.nodes.clone(), decide(Function::SameSize, vec![size(g.nodes.len()), second], Vec::new())));
            }
            GroupComparison::Changed { changed } => {
                let changed = fact("fatto-lente-dimensione-cambiata", Provenance::Measured, Source::ContentHash, vec![count(changed)]);
                out.push(voice(tree, Finding::SameSize, g.nodes.clone(), decide(Function::SameSize, vec![size(g.nodes.len()), changed], Vec::new())));
            }
            GroupComparison::Compared { identical, unique } => {
                for ig in identical {
                    let ig = ig.nodes.iter().find(|n| keep.contains(n)).and_then(|&k| downloads::with_keep(&ig, k)).unwrap_or(ig);
                    out.push(identical_voice(tree, &ig, created.get(&ig.keep).copied()));
                }
                if !unique.is_empty() {
                    let different = fact("fatto-lente-contenuto-diverso", Provenance::Measured, Source::ContentHash, vec![count(unique.len())]);
                    let record = decide(Function::SameSize, vec![size(g.nodes.len()), different], Vec::new());
                    out.push(voice(tree, Finding::DifferentContent, unique, record));
                }
            }
        }
    }
}

/// Copie identiche: Gestibile, con la prova misurata dell'impronta; si spostano tutte tranne `keep`.
fn identical_voice(tree: &Tree, g: &IdenticalGroup, keep_created: Option<i64>) -> LensFinding {
    let n = g.nodes.len();
    let identity = vec![fact("fatto-lente-stessa-dimensione", Provenance::Measured, Source::Scan, vec![count(n), Value::Bytes(g.logical)])];
    let mut proof = vec![fact("fatto-lente-contenuto-identico", Provenance::Measured, Source::ContentHash, vec![count(n), Value::Bytes(g.logical)])];
    if let Some(t) = keep_created {
        proof.push(fact("fatto-lente-copia-tenuta", Provenance::Measured, Source::ContentHash, vec![Value::Time(t)]));
    }
    let moved: Vec<u32> = g.nodes.iter().copied().filter(|&x| x != g.keep).collect();
    let record = manageable(Function::IdenticalCopies, identity, proof, estimate_of(tree, &moved), CheckOutcome::AtSimulation);
    // `nodes` in ordine di creazione, come il gruppo; quello che resta è `keep`.
    LensFinding { finding: Finding::IdenticalCopies { keep: g.keep }, bytes: bytes_of(tree, &g.nodes), nodes: g.nodes.clone(), record }
}

/// Dati letti con il tipo dall'estensione quando il lettore non lo dice: decide la fonte dei fatti
/// (`installers::installer_source`) e il testo del file senza dati.
fn with_kind(tree: &Tree, node: u32, info: &InstallerInfo) -> InstallerInfo {
    InstallerInfo { kind: info.kind.or_else(|| installers::installer_kind(&tree.name_lossy(node))), ..info.clone() }
}

/// Fonte dei dati di versione di un file non letto: dall'estensione.
fn source_by_name(tree: &Tree, node: u32) -> Source {
    match installers::installer_kind(&tree.name_lossy(node)) {
        Some(InstallerKind::Msi) => Source::MsiProperties,
        _ => Source::VersionResource,
    }
}

/// Fatti di un installatore letto e stato della voce. Prima la corrispondenza, che è il riassunto
/// della voce, poi i dati letti (`installers::info_facts`). Senza corrispondenza, o con una corrispondenza
/// non confrontabile, lo stato è `NotMatched`.
pub fn installer_facts(info: &InstallerInfo, matched: Option<&InstallerMatch>) -> (Vec<Fact>, InstallerStatus) {
    let mut facts = Vec::new();
    let status = match matched {
        None => InstallerStatus::NotMatched,
        Some(m) => {
            facts.push(m.fact.clone());
            match m.outcome {
                _ if m.fact.key == installers::FACT_NOT_COMPARABLE => InstallerStatus::NotMatched,
                MatchOutcome::SameVersion { .. } => InstallerStatus::SameVersion,
                MatchOutcome::OtherVersion { .. } => InstallerStatus::OtherVersion,
                MatchOutcome::NotInstalled => InstallerStatus::NotInstalled,
            }
        }
    };
    facts.extend(installers::info_facts(info));
    (facts, status)
}

/// Installatori: Decidi tu, con i fatti della lettura e della corrispondenza.
fn installers(tree: &Tree, lens: &Lens, input: &LensInput, out: &mut Vec<LensFinding>) {
    let mut not_read = Vec::new();
    let mut cloud = Vec::new();
    for it in lens.of_family(LensFamily::Installers) {
        let read = input.installers.get(&it.node);
        if it.placeholder || matches!(read.map(|r| &r.info), Some(Err(ExtractError::CloudPlaceholder))) {
            cloud.push(it.node);
            continue;
        }
        let Some(read) = read else {
            not_read.push(it.node);
            continue;
        };
        // Il riassunto prima, il tipo secondo l'estensione in fondo.
        let (mut identity, status) = match &read.info {
            Err(e) => (vec![error_fact("fatto-lente-estrattore-non-riuscito", source_by_name(tree, it.node), e)], InstallerStatus::Failed),
            Ok(info) => installer_facts(&with_kind(tree, it.node, info), read.matched.as_ref()),
        };
        identity.push(family_fact(LensFamily::Installers, 1));
        out.push(voice(tree, Finding::Installer(status), vec![it.node], decide(Function::Installer, identity, Vec::new())));
    }
    if !not_read.is_empty() {
        let identity = vec![
            family_fact(LensFamily::Installers, not_read.len()),
            fact("fatto-lente-installatori-non-letti", Provenance::Unavailable, Source::VersionResource, Vec::new()),
        ];
        out.push(voice(tree, Finding::Installer(InstallerStatus::NotRead), not_read, decide(Function::Installer, identity, Vec::new())));
    }
    if !cloud.is_empty() {
        let identity = vec![
            family_fact(LensFamily::Installers, cloud.len()),
            fact("fatto-lente-solo-cloud", Provenance::Unavailable, Source::VersionResource, Vec::new()),
        ];
        out.push(voice(tree, Finding::Installer(InstallerStatus::CloudOnly), cloud, decide(Function::Installer, identity, Vec::new())));
    }
}

/// Zip con la cartella estratta accanto: Decidi tu, con la prova misurata "N voci su M presenti".
fn archives(tree: &Tree, lens: &Lens, input: &LensInput, out: &mut Vec<LensFinding>) {
    for pair in downloads::archive_pairs(tree, lens) {
        let mut identity = vec![fact("fatto-lente-cartella-accanto", Provenance::Measured, Source::Scan, Vec::new())];
        let read = input.archives.get(&pair.archive);
        let cloud = tree.flags(pair.archive).has(Flags::CLOUD) || matches!(read, Some(Err(ExtractError::CloudPlaceholder)));
        match read {
            _ if cloud => identity.push(fact("fatto-lente-solo-cloud", Provenance::Unavailable, Source::ZipDirectory, Vec::new())),
            None => identity.push(fact("fatto-lente-indice-non-letto", Provenance::Unavailable, Source::ZipDirectory, Vec::new())),
            Some(Err(e)) => identity.push(error_fact("fatto-lente-archivio-non-leggibile", Source::ZipDirectory, e)),
            Some(Ok(c)) => {
                let n = Value::Count;
                identity.push(fact("fatto-lente-voci-presenti", Provenance::Measured, Source::ZipDirectory, vec![n(c.present), n(c.total)]));
                if c.different_size > 0 {
                    identity.push(fact("fatto-lente-voci-dimensione-diversa", Provenance::Measured, Source::ZipDirectory, vec![n(c.different_size)]));
                }
                if c.missing > 0 {
                    identity.push(fact("fatto-lente-voci-mancanti", Provenance::Measured, Source::ZipDirectory, vec![n(c.missing)]));
                }
                if c.unreadable > 0 {
                    identity.push(fact("fatto-lente-voci-non-lette", Provenance::Unavailable, Source::Scan, vec![n(c.unreadable)]));
                }
            }
        }
        let record = decide(Function::ArchiveWithFolder, identity, Vec::new());
        out.push(voice(tree, Finding::ArchiveWithFolder { folder: pair.folder }, vec![pair.archive], record));
    }
}

/// Segnaposto cloud tra i file di una voce, se ce ne sono.
fn placeholders_fact(items: &[&LensItem]) -> Option<Fact> {
    let k = items.iter().filter(|i| i.placeholder).count();
    (k > 0).then(|| fact("fatto-lente-segnaposto", Provenance::Measured, Source::Scan, vec![count(k)]))
}

/// Immagini disco: una voce, Decidi tu.
fn disk_images(tree: &Tree, lens: &Lens, out: &mut Vec<LensFinding>) {
    let items: Vec<&LensItem> = lens.of_family(LensFamily::DiskImages).collect();
    if items.is_empty() {
        return;
    }
    let mut identity = vec![family_fact(LensFamily::DiskImages, items.len())];
    identity.extend(placeholders_fact(&items));
    let nodes = items.iter().map(|i| i.node).collect();
    out.push(voice(tree, Finding::DiskImages, nodes, decide(Function::DiskImage, identity, Vec::new())));
}

/// Download incompleti: Gestibile se Restart Manager non trova processi, bloccati se un processo
/// li usa, Decidi tu senza proposta se l'uso non è noto.
fn incomplete(tree: &Tree, lens: &Lens, input: &LensInput, out: &mut Vec<LensFinding>) {
    let mut free: Vec<&LensItem> = Vec::new();
    let mut free_at = i64::MAX;
    let mut unchecked: Vec<&LensItem> = Vec::new();
    for it in lens.of_family(LensFamily::Incomplete) {
        let Some(check) = input.usage.get(&it.node) else {
            unchecked.push(it);
            continue;
        };
        let rm = |key: &str, p: Provenance, values: Vec<Value>| fact(key, p, Source::RestartManager, values);
        let (status, identity, checks) = match &check.usage {
            FileUse::Free => {
                free.push(it);
                // Vale il controllo più vecchio.
                free_at = free_at.min(check.at);
                continue;
            }
            FileUse::Used(names) => {
                let mut identity = vec![family_fact(LensFamily::Incomplete, 1), rm("fatto-lente-in-uso", Provenance::Measured, vec![Value::Time(check.at)])];
                let mut seen = HashSet::new();
                for name in names.iter().filter(|n| seen.insert(n.as_str())) {
                    identity.push(rm("fatto-lente-processo", Provenance::Measured, vec![text(name)]));
                }
                let failed = Check { key: "controllo-restart-manager".into(), kind: CheckKind::Automatic, outcome: CheckOutcome::Failed };
                (UseStatus::InUse, identity, vec![failed])
            }
            FileUse::Unknown(c) => {
                let error = rm("fatto-lente-uso-non-letto", Provenance::Unavailable, vec![code(*c), Value::Time(check.at)]);
                (UseStatus::Unknown, vec![family_fact(LensFamily::Incomplete, 1), error], Vec::new())
            }
        };
        out.push(voice(tree, Finding::Incomplete(status), vec![it.node], decide(Function::IncompleteDownload, identity, checks)));
    }
    if !free.is_empty() {
        let nodes: Vec<u32> = free.iter().map(|i| i.node).collect();
        let mut identity = vec![family_fact(LensFamily::Incomplete, free.len())];
        identity.extend(placeholders_fact(&free));
        let none = vec![count(free.len()), Value::Time(free_at)];
        let proof = vec![fact("fatto-lente-nessun-processo", Provenance::Measured, Source::RestartManager, none)];
        let record = manageable(Function::IncompleteDownload, identity, proof, estimate_of(tree, &nodes), CheckOutcome::Passed);
        out.push(voice(tree, Finding::Incomplete(UseStatus::Free), nodes, record));
    }
    if !unchecked.is_empty() {
        let mut identity = vec![family_fact(LensFamily::Incomplete, unchecked.len())];
        identity.extend(placeholders_fact(&unchecked));
        identity.push(fact("fatto-lente-uso-non-controllato", Provenance::Unavailable, Source::RestartManager, Vec::new()));
        let nodes = unchecked.iter().map(|i| i.node).collect();
        out.push(voice(tree, Finding::Incomplete(UseStatus::NotChecked), nodes, decide(Function::IncompleteDownload, identity, Vec::new())));
    }
}

/// Chiave del titolo di una funzione della lente (`funzione-<codice>-nome`), per la card interventi e per
/// l'elenco. Quella dell'installatore (`funzione-installatore-nome`) sta nella sezione degli installatori
/// dei testi delle regole.
pub fn name_key(f: Function) -> String {
    format!("funzione-{}-nome", f.code())
}
