// SPDX-License-Identifier: GPL-3.0-or-later
//! Record di raccomandazione (03 §6.1, 05 D6): stato, prove con la provenienza di ogni fatto, metodo,
//! controlli, stima, regola. Solo codici e valori: i testi li scrive l'interfaccia dai file di lingua
//! (05 D13). Le chiavi (`key`) sono identificatori dei file di lingua, non frasi.

/// Affidabilità di un fatto dedotto (03 §6.2). L'ordine conta: `min` dà la più bassa.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reliability {
    Low,
    Medium,
    High,
}

/// Criterio di 03 §6.2 che produce il livello. Il livello lo calcola il motore, mai la regola.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Criterion {
    /// Alta: due fonti concordi.
    TwoSources,
    /// Alta: una fonte ufficiale con la struttura descritta dalla regola.
    OfficialWithStructure,
    /// Media: una sola fonte non ufficiale (per esempio una voce in HKCU, scrivibile dall'utente).
    OneSource,
    /// Media: struttura riconosciuta senza fonte.
    StructureOnly,
    /// Bassa: solo nome o somiglianza.
    NameOnly,
    /// Bassa: versione installata fuori dall'intervallo della regola, o non nota (03 §6.1 regola 4).
    VersionOutOfRange,
    /// Bassa: regola non riverificata da oltre 12 mesi (07 §5.1).
    RuleExpired,
    /// Bassa: due regole in conflitto sullo stesso elemento (03 §6.1 regola 5).
    Conflict,
}

impl Criterion {
    pub fn level(self) -> Reliability {
        match self {
            Self::TwoSources | Self::OfficialWithStructure => Reliability::High,
            Self::OneSource | Self::StructureOnly => Reliability::Medium,
            Self::NameOnly | Self::VersionOutOfRange | Self::RuleExpired | Self::Conflict => Reliability::Low,
        }
    }
}

/// Classe di provenienza di un fatto (03 §6.1, glifi di 04 §6.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Provenance {
    /// Letto dal file system o da un'API di misura in questa sessione.
    Measured,
    /// Dichiarato da Windows o dall'app (registro, API, manifest, configurazione).
    FromSource,
    /// Ricavato da una regola; il livello viene dal criterio.
    Deduced(Criterion),
    /// Non leggibile o non misurabile in questa modalità: mai mostrato come zero.
    Unavailable,
}

impl Provenance {
    pub fn reliability(self) -> Option<Reliability> {
        match self {
            Self::Deduced(c) => Some(c.level()),
            _ => None,
        }
    }

    /// Il colore del gestore nasce solo da fatti "da fonte" o "dedotto alta" (03 §6.2).
    pub fn colors(self) -> bool {
        match self {
            Self::FromSource => true,
            Self::Deduced(c) => c.level() == Reliability::High,
            Self::Measured | Self::Unavailable => false,
        }
    }

    /// Codice per i file di lingua e per i glifi.
    pub fn code(self) -> &'static str {
        match self {
            Self::Measured => "misurato",
            Self::FromSource => "da-fonte",
            Self::Deduced(_) => "dedotto",
            Self::Unavailable => "non-disponibile",
        }
    }
}

/// Da dove viene un fatto.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// Scansione del file system di questa sessione.
    Scan,
    /// Voce di disinstallazione (`...\Uninstall`), con l'hive.
    Uninstall(Hive),
    /// Pacchetto MSIX (`PackageManager`).
    Msix,
    /// Manifest di Steam (`appmanifest_*.acf`, `libraryfolders.vdf`).
    SteamManifest,
    /// `localconfig.vdf` di Steam.
    SteamLocalConfig,
    /// Manifest di Epic (`Manifests\*.item`).
    EpicManifest,
    /// `Apps\ProductInfo.xml` di CapCut.
    CapCutProductInfo,
    /// Configurazione di CapCut (`User Data\Config\globalSetting`).
    CapCutConfig,
    /// `SHQueryRecycleBinW`.
    RecycleBinApi,
    /// API di alimentazione.
    PowerApi,
    /// Chiavi di Sensore memoria.
    StorageSense,
    /// Chiave `BitBucket` del volume.
    BitBucket,
    /// Configurazione di npm (`.npmrc`, variabile d'ambiente, valore predefinito documentato).
    NpmConfig,
    /// Cartella nota di Windows (`SHGetKnownFolderPath`).
    KnownFolder,
    /// La regola stessa (fatti dedotti).
    Rule,
    /// SHA-256 del contenuto, calcolato su richiesta (F-DL-4).
    ContentHash,
    /// Directory centrale di un archivio zip, letta su richiesta (F-DL-5).
    ZipDirectory,
    /// Risorsa di versione di un eseguibile (`GetFileVersionInfoW`, F-DL-2).
    VersionResource,
    /// Tabella Property di un pacchetto MSI aperto in sola lettura (F-DL-2).
    MsiProperties,
    /// Interrogazione di Restart Manager (F-DL-6).
    RestartManager,
    /// UserAssist, letto solo con `Start_TrackProgs` = 1 (F-AP-4).
    UserAssist,
}

impl Source {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Scan => "scansione",
            Self::Uninstall(Hive::Machine) => "disinstallazione-hklm",
            Self::Uninstall(Hive::Machine32) => "disinstallazione-wow",
            Self::Uninstall(Hive::User) => "disinstallazione-hkcu",
            Self::Msix => "msix",
            Self::SteamManifest => "steam-manifest",
            Self::SteamLocalConfig => "steam-localconfig",
            Self::EpicManifest => "epic-manifest",
            Self::CapCutProductInfo => "capcut-productinfo",
            Self::CapCutConfig => "capcut-config",
            Self::RecycleBinApi => "cestino-api",
            Self::PowerApi => "alimentazione",
            Self::StorageSense => "sensore-memoria",
            Self::BitBucket => "bitbucket",
            Self::NpmConfig => "npm-config",
            Self::KnownFolder => "cartella-nota",
            Self::Rule => "regola",
            Self::ContentHash => "hash-contenuto",
            Self::ZipDirectory => "indice-zip",
            Self::VersionResource => "risorsa-versione",
            Self::MsiProperties => "proprieta-msi",
            Self::RestartManager => "restart-manager",
            Self::UserAssist => "userassist",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hive {
    Machine,
    Machine32,
    User,
}

/// Valore di un fatto. Il testo è un dato (nome, versione, percorso), mai una frase.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Text(String),
    Bytes(u64),
    Count(u64),
    /// FILETIME.
    Time(i64),
    Days(u32),
}

/// Un fatto con la sua provenienza (F-IN-1). `key` è la chiave del testo.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Fact {
    pub key: String,
    pub provenance: Provenance,
    pub source: Source,
    pub values: Vec<Value>,
}

impl Fact {
    pub fn new(key: impl Into<String>, provenance: Provenance, source: Source, values: Vec<Value>) -> Self {
        Self { key: key.into(), provenance, source, values }
    }
}

/// Prova di identità: cos'è l'elemento e a chi appartiene.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProvaDiIdentita(pub Vec<Fact>);

/// Prova di gestibilità: l'azione proposta ha effetti noti e limitati. Distinta dalla prova di identità
/// e mai vuota: senza questo valore non si costruiscono Gestibile né Da verificare (03 §6.1, 05 D6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvaDiGestibilita(Vec<Fact>);

impl ProvaDiGestibilita {
    /// `None` se non ci sono fatti, o se uno dei fatti è solo una misura di dimensione o di data:
    /// dimensione, età, nome ed estensione non provano la gestibilità (03 §6.1 regola 2).
    pub fn new(facts: Vec<Fact>) -> Option<Self> {
        if facts.is_empty() || facts.iter().any(|f| f.provenance == Provenance::Measured && f.source == Source::Scan) {
            return None;
        }
        Some(Self(facts))
    }

    pub fn facts(&self) -> &[Fact] {
        &self.0
    }
}

/// Chi gestisce i dati: decide il colore (04 §6.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Manager {
    Windows,
    Programs,
    You,
}

impl Manager {
    pub const ALL: [Self; 3] = [Self::Windows, Self::Programs, Self::You];

    pub fn code(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Programs => "programmi",
            Self::You => "tu",
        }
    }

    pub fn from_code(c: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.code() == c)
    }
}

/// Gruppo di colore di un elemento: il gestore, oppure il grigio di "non classificato" e "ipotesi".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ColorGroup {
    Manager(Manager),
    /// Nessuna regola, oppure attribuzione non da fonte né dedotta alta.
    Unclassified,
}

/// Perché lo stato è Non sappiamo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnknownReason {
    /// Nessuna regola riconosce l'elemento.
    NoRule,
    /// La regola vale per versioni diverse da quella installata, o la versione non si conosce.
    VersionOutOfRange,
    /// Regola non riverificata da oltre 12 mesi.
    RuleExpired,
    /// Due regole in conflitto.
    Conflict,
    /// Ruolo dedotto solo dal nome (possibile residuo).
    NameOnly,
    /// La regola dichiara Gestibile o Da verificare ma non c'è una prova di gestibilità.
    NoManageabilityProof,
}

/// Uno dei cinque stati (03 §6.1). Gestibile e Da verificare portano la prova di gestibilità.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Manageable(ProvaDiGestibilita),
    ToVerify(ProvaDiGestibilita),
    YouDecide,
    /// Lo gestisce `by` (nome del programma o "Windows": un dato). `procedure` falso è la variante
    /// "Nessuna procedura: non toccare".
    ManagedBy { by: String, procedure: bool },
    Unknown(UnknownReason),
}

impl State {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Manageable(_) => "gestibile",
            Self::ToVerify(_) => "da-verificare",
            Self::YouDecide => "decidi-tu",
            Self::ManagedBy { procedure: true, .. } => "lo-gestisce",
            Self::ManagedBy { procedure: false, .. } => "lo-gestisce-nessuna-procedura",
            Self::Unknown(_) => "non-sappiamo",
        }
    }

    pub fn manageability(&self) -> Option<&ProvaDiGestibilita> {
        match self {
            Self::Manageable(p) | Self::ToVerify(p) => Some(p),
            _ => None,
        }
    }

    /// Stati che propongono qualcosa (card interventi, F-OV-4).
    pub fn proposes(&self) -> bool {
        match self {
            Self::Manageable(_) | Self::ToVerify(_) | Self::YouDecide => true,
            Self::ManagedBy { procedure, .. } => *procedure,
            Self::Unknown(_) => false,
        }
    }
}

/// Azione del record. In M3 il record la descrive; le azioni arrivano con M4 e M5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Spostamento nel Cestino dopo la simulazione (F-AC-2).
    Recycle,
    /// Procedura di Windows o dell'app (URI aperto).
    Procedure,
    /// Comando ufficiale mostrato e copiato, mai eseguito.
    Command,
    /// Disinstallazione tramite interfaccia ufficiale (F-AP-5).
    Uninstall,
    /// Nessuna azione oltre a quelle sempre disponibili (F-IN-5).
    None,
}

impl Action {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Recycle => "cestino",
            Self::Procedure => "procedura",
            Self::Command => "comando",
            Self::Uninstall => "disinstallazione",
            Self::None => "nessuna",
        }
    }
}

/// Un metodo: testo (chiave) e, se c'è, il comando o l'URI letterale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub key: String,
    pub command: Option<String>,
    pub uri: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CheckKind {
    Automatic,
    Manual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CheckOutcome {
    /// Si esegue nella simulazione (M5): Restart Manager, riferimenti, capacità del Cestino.
    AtSimulation,
    Passed,
    Failed,
    /// Da spuntare (controlli manuali).
    ToTick,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub key: String,
    pub kind: CheckKind,
    pub outcome: CheckOutcome,
}

/// Perché la stima non ha un valore.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotEstimable {
    /// Hard link verso cartelle che la regola non conta (store di pnpm).
    HardLinksNotCounted,
    /// Procedura o comando esterno senza un metodo di stima (`npm cache verify`).
    ExternalProcedure,
    /// Nessuna azione: non c'è niente da stimare.
    NoAction,
    /// Contiene cartelle non leggibili o file con altri nomi: si calcola nella simulazione.
    AtSimulation,
}

impl NotEstimable {
    pub const ALL: [Self; 4] = [Self::HardLinksNotCounted, Self::ExternalProcedure, Self::NoAction, Self::AtSimulation];

    /// Codice stabile (registro degli interventi).
    pub const fn code(self) -> &'static str {
        match self {
            Self::HardLinksNotCounted => "hard-link-non-contati",
            Self::ExternalProcedure => "procedura-esterna",
            Self::NoAction => "nessuna-azione",
            Self::AtSimulation => "in-simulazione",
        }
    }
}

/// Stima dello spazio recuperabile (03 §6.3). In M3 viene dal modello della scansione; la simulazione
/// di M5 la ricalcola rileggendo gli attributi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Estimate {
    Exact(u64),
    Range { min: u64, max: u64 },
    NotEstimable(NotEstimable),
}

impl Estimate {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Exact(_) => "esatta",
            Self::Range { .. } => "intervallo",
            Self::NotEstimable(_) => "non-stimabile",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Privileges {
    None,
    Admin,
}

/// Data del calendario (data di verifica delle regole).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Date {
    pub fn new(year: u16, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    /// Da `AAAA-MM-GG`.
    pub fn parse(s: &str) -> Option<Self> {
        let mut it = s.split('-');
        let y: u16 = it.next()?.parse().ok()?;
        let m: u8 = it.next()?.parse().ok()?;
        let d: u8 = it.next()?.parse().ok()?;
        if it.next().is_some() || !(1..=12).contains(&m) || d == 0 || d > days_in_month(y, m) {
            return None;
        }
        Some(Self { year: y, month: m, day: d })
    }

    /// La stessa data `n` mesi dopo (giorno ridotto all'ultimo del mese se serve).
    pub fn add_months(self, n: u32) -> Self {
        let total = u32::from(self.month) - 1 + n;
        let year = self.year + (total / 12) as u16;
        let month = (total % 12) as u8 + 1;
        Self { year, month, day: self.day.min(days_in_month(year, month)) }
    }
}

fn days_in_month(y: u16, m: u8) -> u8 {
    match m {
        4 | 6 | 9 | 11 => 30,
        2 if (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400) => 29,
        2 => 28,
        _ => 31,
    }
}

/// Regola che ha prodotto il record: identificativo e data di verifica. Fonti e testi stanno nella
/// base delle regole (`quado-kb`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleRef {
    pub id: String,
    pub verified: Date,
}

/// Funzione di Quado che produce un record senza una regola della base: decide solo da prove
/// misurate o lette in questa sessione (lente Download e Programmi, 07 §7 ultima riga).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Function {
    /// File della stessa dimensione, contenuto non ancora confrontato (F-DL-4).
    SameSize,
    /// Copie con lo stesso SHA-256 (F-DL-4).
    IdenticalCopies,
    /// Installatore, con la corrispondenza con i programmi installati (F-DL-2).
    Installer,
    /// Archivio zip con una cartella omonima accanto (F-DL-5).
    ArchiveWithFolder,
    /// Immagine disco (F-DL-1).
    DiskImage,
    /// Download incompleto (F-DL-6).
    IncompleteDownload,
    /// Programma installato (F-AP).
    Program,
    /// Cartella che potrebbe essere un residuo (F-AP-6).
    PossibleResidue,
}

impl Function {
    pub const ALL: [Self; 8] = [
        Self::SameSize,
        Self::IdenticalCopies,
        Self::Installer,
        Self::ArchiveWithFolder,
        Self::DiskImage,
        Self::IncompleteDownload,
        Self::Program,
        Self::PossibleResidue,
    ];

    /// Codice stabile per i file di lingua (`funzione-<codice>`).
    pub fn code(self) -> &'static str {
        match self {
            Self::SameSize => "stessa-dimensione",
            Self::IdenticalCopies => "copie-identiche",
            Self::Installer => "installatore",
            Self::ArchiveWithFolder => "archivio-con-cartella",
            Self::DiskImage => "immagine-disco",
            Self::IncompleteDownload => "download-incompleto",
            Self::Program => "programma",
            Self::PossibleResidue => "possibile-residuo",
        }
    }
}

/// Da dove nasce un record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Regola della base (`quado-kb`), con la data di verifica.
    Rule(RuleRef),
    /// Funzione di Quado (M4): nessuna regola, solo prove misurate o lette.
    Function(Function),
    /// Nessuna regola riconosce l'elemento (Non sappiamo).
    NoRule,
}

/// Annuncio della scadenza di una regola (07 §5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Expiry {
    /// Fino a 6 mesi.
    Fresh,
    /// Oltre 6 mesi: conta nel banner "regole da riverificare".
    Recheck,
    /// Oltre 11 mesi: ispettore e conferma annunciano la scadenza.
    Announce,
    /// Oltre 12 mesi: niente azioni né procedure, stato Non sappiamo.
    Expired,
}

impl Expiry {
    pub fn at(verified: Date, today: Date) -> Self {
        if today > verified.add_months(12) {
            Self::Expired
        } else if today > verified.add_months(11) {
            Self::Announce
        } else if today > verified.add_months(6) {
            Self::Recheck
        } else {
            Self::Fresh
        }
    }
}

/// Record di raccomandazione con tutti i campi obbligatori di 03 §6.1. Un campo senza valore dice
/// perché: le liste vuote si mostrano con il loro testo "nessuno", le chiavi non sono mai vuote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub state: State,
    pub action: Action,
    pub identity: ProvaDiIdentita,
    /// Ciò che si assume senza prova (chiavi).
    pub hypotheses: Vec<String>,
    /// Livello dei fatti dedotti e criterio che lo produce.
    pub reliability: Criterion,
    /// Rischi ed effetti (chiavi).
    pub risks: Vec<String>,
    pub preferred: Option<Step>,
    pub alternatives: Vec<Step>,
    pub checks: Vec<Check>,
    pub estimate: Estimate,
    pub reversibility: String,
    pub privileges: Privileges,
    pub on_error: String,
    pub after: String,
    pub origin: Origin,
    /// Scadenza della regola; `Fresh` per le funzioni e senza regola.
    pub expiry: Expiry,
    /// L'elemento è protetto (F-AC-4) e la regola usa l'eccezione documentata in 03 §6.1.
    pub protected_exception: bool,
}

impl Record {
    /// Regola che ha prodotto il record, se c'è.
    pub fn rule(&self) -> Option<&RuleRef> {
        match &self.origin {
            Origin::Rule(r) => Some(r),
            _ => None,
        }
    }

    /// Identificativo per i messaggi dei test: id della regola, codice della funzione o `nessuna-regola`.
    pub fn origin_id(&self) -> &str {
        match &self.origin {
            Origin::Rule(r) => &r.id,
            Origin::Function(f) => f.code(),
            Origin::NoRule => "nessuna-regola",
        }
    }

    /// Chiavi di testo del record, per i controlli di completezza.
    pub fn text_keys(&self) -> impl Iterator<Item = &str> {
        self.hypotheses
            .iter()
            .chain(&self.risks)
            .map(String::as_str)
            .chain(self.preferred.iter().map(|s| s.key.as_str()))
            .chain(self.alternatives.iter().map(|s| s.key.as_str()))
            .chain(self.checks.iter().map(|c| c.key.as_str()))
            .chain([self.reversibility.as_str(), self.on_error.as_str(), self.after.as_str()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manageability_needs_facts_that_are_not_measures() {
        assert!(ProvaDiGestibilita::new(Vec::new()).is_none());
        let size = Fact::new("dimensione", Provenance::Measured, Source::Scan, vec![Value::Bytes(1)]);
        assert!(ProvaDiGestibilita::new(vec![size]).is_none(), "una dimensione non prova la gestibilità");
        let f = Fact::new("versione-in-uso", Provenance::FromSource, Source::CapCutProductInfo, vec![Value::Text("9.4.0.4015".into())]);
        assert!(ProvaDiGestibilita::new(vec![f]).is_some());
    }

    #[test]
    fn color_only_from_source_or_high() {
        assert!(Provenance::FromSource.colors());
        assert!(Provenance::Deduced(Criterion::TwoSources).colors());
        assert!(!Provenance::Deduced(Criterion::OneSource).colors());
        assert!(!Provenance::Deduced(Criterion::NameOnly).colors());
        assert!(!Provenance::Measured.colors());
    }

    #[test]
    fn expiry_after_6_11_12_months() {
        let v = Date::new(2026, 10, 13);
        assert_eq!(Expiry::at(v, Date::new(2027, 4, 13)), Expiry::Fresh);
        assert_eq!(Expiry::at(v, Date::new(2027, 4, 14)), Expiry::Recheck);
        assert_eq!(Expiry::at(v, Date::new(2027, 9, 14)), Expiry::Announce);
        assert_eq!(Expiry::at(v, Date::new(2027, 10, 13)), Expiry::Announce);
        assert_eq!(Expiry::at(v, Date::new(2027, 10, 14)), Expiry::Expired);
        assert_eq!(Date::new(2026, 1, 31).add_months(1), Date::new(2026, 2, 28));
        assert_eq!(Date::parse("2026-02-30"), None);
        assert_eq!(Date::parse("2026-10-13"), Some(v));
    }
}
