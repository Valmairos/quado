// SPDX-License-Identifier: GPL-3.0-or-later
//! Base delle regole: file TOML in `regole/`, incorporati. Ogni regola dichiara dove si applica, come
//! riconosce la struttura, i campi del record, le fonti con l'estratto, l'intervallo di versioni, la
//! data di verifica e la sua fixture. I testi visibili stanno in `regole/<lingua>.ftl` con chiavi
//! `regola-<id>-<campo>`.

use serde::Deserialize;

use quado_model::record::{Date, Manager, Privileges};

include!(concat!(env!("OUT_DIR"), "/regole.rs"));

/// Come la regola trova i suoi elementi. Ognuno ha il suo codice nel motore.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Recognizer {
    /// Percorsi sotto una cartella nota, con i file marcatore se dichiarati.
    Path,
    /// Cartelle di versione di CapCut, con la versione in uso da due fonti.
    CapCutVersions,
    /// Cestino dell'utente corrente sul volume.
    RecycleBin,
    /// Cartelle `$R` senza il loro `$I` nel Cestino dell'utente.
    RecycleOrphans,
    /// Giochi di Steam dai manifest.
    Steam,
    /// Installazioni di Epic dai manifest.
    Epic,
    /// Programmi dalle voci di disinstallazione (prefisso più lungo).
    Registered,
    /// Dati dei pacchetti MSIX (`Packages\<nome di famiglia>`).
    Msix,
    /// Cache di npm, percorso dalla configurazione.
    NpmCache,
    /// Store di pnpm, percorso dalla configurazione.
    PnpmStore,
    /// Cartelle con un nome dato e un lockfile accanto (`node_modules`, `target`, ...).
    Lockfile,
    /// Cartelle note dell'utente.
    UserFolders,
    /// Ipotesi dal nome: la applica il motore dopo le altre regole, solo dove nessuna ha riconosciuto.
    NameHypothesis,
}

impl Recognizer {
    fn from_code(c: &str) -> Option<Self> {
        Some(match c {
            "percorso" => Self::Path,
            "capcut-versioni" => Self::CapCutVersions,
            "cestino" => Self::RecycleBin,
            "cestino-orfane" => Self::RecycleOrphans,
            "steam" => Self::Steam,
            "epic" => Self::Epic,
            "programmi-registrati" => Self::Registered,
            "msix" => Self::Msix,
            "npm-cache" => Self::NpmCache,
            "pnpm-store" => Self::PnpmStore,
            "lockfile" => Self::Lockfile,
            "cartelle-utente" => Self::UserFolders,
            "ipotesi" => Self::NameHypothesis,
            _ => return None,
        })
    }
}

/// Stato dichiarato dalla regola. Gestibile e Da verificare diventano tali solo se il riconoscitore
/// produce una prova di gestibilità.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StateCode {
    Manageable,
    ToVerify,
    YouDecide,
    ManagedBy,
    ManagedByNoProcedure,
}

impl StateCode {
    fn from_code(c: &str) -> Option<Self> {
        Some(match c {
            "gestibile" => Self::Manageable,
            "da-verificare" => Self::ToVerify,
            "decidi-tu" => Self::YouDecide,
            "lo-gestisce" => Self::ManagedBy,
            "lo-gestisce-nessuna-procedura" => Self::ManagedByNoProcedure,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionCode {
    Recycle,
    Procedure,
    Command,
    Uninstall,
    None,
}

impl ActionCode {
    fn from_code(c: &str) -> Option<Self> {
        Some(match c {
            "cestino" => Self::Recycle,
            "procedura" => Self::Procedure,
            "comando" => Self::Command,
            "disinstallazione" => Self::Uninstall,
            "nessuna" => Self::None,
            _ => return None,
        })
    }
}

/// Metodo della stima dichiarato dalla regola.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EstimateCode {
    /// Allocata degli elementi, dal modello (ricalcolata nella simulazione).
    Exact,
    /// Hard link verso cartelle che la regola non conta.
    NotEstimableLinks,
    /// Procedura o comando esterno senza un metodo.
    NotEstimableProcedure,
    /// Nessuna azione.
    None,
}

impl EstimateCode {
    fn from_code(c: &str) -> Option<Self> {
        Some(match c {
            "esatta" => Self::Exact,
            "non-stimabile-link" => Self::NotEstimableLinks,
            "non-stimabile-procedura" => Self::NotEstimableProcedure,
            "nessuna" => Self::None,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Passo {
    #[serde(default)]
    pub comando: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Dove {
    /// Cartella nota (codici in `evidence::FOLDER_CODES`).
    #[serde(default)]
    pub cartella: Option<String>,
    /// Percorsi relativi alla cartella: segmenti separati da `/`; `*` qualunque nome, `<versione>`
    /// numeri puntati, `<sid>` il SID dell'utente corrente, `.` la cartella stessa. Un prefisso
    /// `codice:` sceglie un'altra cartella nota (`windows:Temp`).
    #[serde(default)]
    pub percorsi: Vec<String>,
    /// File che devono esistere dentro l'elemento perché la struttura sia conforme.
    #[serde(default)]
    pub marcatori: Vec<String>,
    /// Riconoscitore `lockfile`: ogni nome di cartella con i lockfile che lo ricostruiscono (`target` solo
    /// con `Cargo.lock`, `node_modules` solo con un lockfile JavaScript).
    #[serde(default)]
    pub coppie: Vec<Coppia>,
    /// Cartelle sotto cui il riconoscitore non propone niente: codice di una cartella nota, con un percorso
    /// relativo facoltativo dopo i due punti (`profilo:.vscode`). Le `node_modules` delle estensioni e dei
    /// programmi non si ricostruiscono dal progetto.
    #[serde(default)]
    pub escludi: Vec<String>,
}

/// Nome di una cartella ricostruibile e i lockfile che ne dicono il progetto.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Coppia {
    pub nome: String,
    pub lockfile: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Versioni {
    /// Programma di cui si legge la versione installata (`capcut`).
    pub programma: String,
    pub min: String,
    pub max: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Fonte {
    pub url: String,
    pub titolo: String,
    /// Frase copiata dalla fonte che giustifica la gestione: chi rilegge la regola la confronta con la
    /// pagina.
    pub estratto: String,
    /// Documentazione di Microsoft o del programma.
    pub ufficiale: bool,
}

/// Indirizzo di una fonte: una pagina `https://`, un file del PC (`file:///`, per i file che un programma
/// scrive nel profilo) o un documento del repository di Quado (`docs/...`, per le verifiche fatte con Quado).
pub fn source_url_ok(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("file:///") || (url.starts_with("docs/") && !url.contains("..") && !url.contains('\\'))
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleFile {
    id: String,
    priorita: u8,
    riconoscitore: String,
    gestore: String,
    #[serde(default)]
    gestito_da: Option<String>,
    stato: String,
    azione: String,
    #[serde(default)]
    contenitore: bool,
    /// Falso: la regola non compare nella card «Interventi possibili» (configurazione di Windows o voci
    /// che liberano poco). Resta nell'ispettore e nella vista Windows e sistema.
    #[serde(default = "yes")]
    card: bool,
    /// Prima di «Svuota il Cestino…» il risultato ripete cosa provare (`regola-<id>-prova`).
    #[serde(default)]
    prova_prima_di_svuotare: bool,
    /// «Svuota il Cestino…» resta disattivato finché non si spunta `regola-<id>-prova-fatta`.
    #[serde(default)]
    spunta_prova: bool,
    #[serde(default)]
    protetto: bool,
    #[serde(default)]
    eccezione_protetti: bool,
    #[serde(default)]
    amministratore: bool,
    stima: String,
    #[serde(default)]
    metodo: Option<Passo>,
    #[serde(default)]
    alternative: Vec<Passo>,
    #[serde(default)]
    controlli: Vec<String>,
    #[serde(default)]
    controlli_manuali: Vec<String>,
    #[serde(default)]
    ipotesi: u8,
    #[serde(default)]
    rischi: u8,
    reversibilita: String,
    errore: String,
    verifica: String,
    #[serde(default)]
    dove: Dove,
    #[serde(default)]
    versioni: Option<Versioni>,
    #[serde(default)]
    fonti: Vec<Fonte>,
    verificata: String,
    fixture: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    pub priority: u8,
    pub recognizer: Recognizer,
    pub manager: Manager,
    /// Chi gestisce, per lo stato "Lo gestisce X" (un nome, non tradotto).
    pub managed_by: Option<String>,
    pub state: StateCode,
    pub action: ActionCode,
    /// La top list può aprirlo e mostrarne il contenuto (programmi, cartelle dell'utente).
    pub container: bool,
    /// Compare nella card «Interventi possibili».
    pub in_card: bool,
    /// Il risultato di uno spostamento ripete cosa provare prima di svuotare il Cestino.
    pub try_before_empty: bool,
    /// «Svuota il Cestino…» chiede prima la spunta della prova fatta.
    pub tick_tried: bool,
    /// Elemento protetto.
    pub protected: bool,
    pub protected_exception: bool,
    pub privileges: Privileges,
    pub estimate: EstimateCode,
    pub method: Option<Passo>,
    pub alternatives: Vec<Passo>,
    pub checks: Vec<String>,
    pub manual_checks: Vec<String>,
    pub hypotheses: u8,
    pub risks: u8,
    pub reversibility: String,
    pub on_error: String,
    pub after: String,
    pub place: Dove,
    pub versions: Option<Versioni>,
    pub sources: Vec<Fonte>,
    pub verified: Date,
    pub fixture: String,
}

/// Problema in un file di regola: codici e posizione, mai testo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleIssue {
    /// TOML non valido o campo sconosciuto; byte di inizio.
    Toml(usize),
    /// L'identificativo non coincide con il nome del file.
    Id,
    UnknownCode,
    /// Nessuna fonte, o una fonte senza URL https, titolo o estratto.
    Source,
    Date,
    /// Eccezione degli elementi protetti senza stato Da verificare.
    Exception,
    /// "Lo gestisce X" senza X.
    ManagedBy,
    /// Azione senza metodo, o metodo senza comando né URI quando l'azione li richiede.
    Method,
    /// Cartella nota sconosciuta o percorsi mancanti.
    Place,
    Fixture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleError {
    /// Indice del file in `RULE_FILES`.
    pub file: usize,
    pub issue: RuleIssue,
}

impl Rule {
    pub fn parse(file_name: &str, text: &str) -> Result<Self, RuleIssue> {
        let f: RuleFile = toml::from_str(text).map_err(|e| RuleIssue::Toml(e.span().map_or(0, |s| s.start)))?;
        if file_name.strip_suffix(".toml") != Some(f.id.as_str()) {
            return Err(RuleIssue::Id);
        }
        let recognizer = Recognizer::from_code(&f.riconoscitore).ok_or(RuleIssue::UnknownCode)?;
        let manager = Manager::from_code(&f.gestore).ok_or(RuleIssue::UnknownCode)?;
        let state = StateCode::from_code(&f.stato).ok_or(RuleIssue::UnknownCode)?;
        let action = ActionCode::from_code(&f.azione).ok_or(RuleIssue::UnknownCode)?;
        let estimate = EstimateCode::from_code(&f.stima).ok_or(RuleIssue::UnknownCode)?;
        let verified = Date::parse(&f.verificata).ok_or(RuleIssue::Date)?;
        if f.fonti.is_empty() || f.fonti.iter().any(|s| !source_url_ok(&s.url) || s.estratto.trim().is_empty() || s.titolo.trim().is_empty()) {
            return Err(RuleIssue::Source);
        }
        if f.eccezione_protetti && state != StateCode::ToVerify {
            return Err(RuleIssue::Exception);
        }
        if matches!(state, StateCode::ManagedBy | StateCode::ManagedByNoProcedure) && f.gestito_da.as_deref().is_none_or(|s| s.trim().is_empty()) {
            return Err(RuleIssue::ManagedBy);
        }
        // Un comando si mostra e si copia: il metodo preferito deve averlo. Ogni azione ha un metodo.
        if action == ActionCode::Command && !f.metodo.as_ref().is_some_and(|p| p.comando.is_some()) {
            return Err(RuleIssue::Method);
        }
        if action != ActionCode::None && f.metodo.is_none() {
            return Err(RuleIssue::Method);
        }
        if action == ActionCode::None && (f.metodo.is_some() || !f.alternative.is_empty()) {
            return Err(RuleIssue::Method);
        }
        if state == StateCode::ManagedByNoProcedure && action != ActionCode::None {
            return Err(RuleIssue::Method);
        }
        if let Some(c) = &f.dove.cartella
            && !crate::evidence::FOLDER_CODES.contains(&c.as_str())
        {
            return Err(RuleIssue::Place);
        }
        if recognizer == Recognizer::Path && (f.dove.cartella.is_none() || f.dove.percorsi.is_empty()) {
            return Err(RuleIssue::Place);
        }
        if recognizer == Recognizer::Lockfile && (f.dove.coppie.is_empty() || f.dove.coppie.iter().any(|c| c.nome.trim().is_empty() || c.lockfile.is_empty())) {
            return Err(RuleIssue::Place);
        }
        if f.dove.escludi.iter().any(|e| !crate::evidence::FOLDER_CODES.contains(&e.split_once(':').map_or(e.as_str(), |(c, _)| c))) {
            return Err(RuleIssue::Place);
        }
        // La spunta della prova vale solo con il testo che dice cosa provare, e solo per uno spostamento.
        if (f.spunta_prova && !f.prova_prima_di_svuotare) || (f.prova_prima_di_svuotare && action != ActionCode::Recycle) {
            return Err(RuleIssue::Method);
        }
        if f.fixture.trim().is_empty() || f.fixture.contains(['/', '\\']) {
            return Err(RuleIssue::Fixture);
        }
        Ok(Self {
            id: f.id,
            priority: f.priorita,
            recognizer,
            manager,
            managed_by: f.gestito_da,
            state,
            action,
            container: f.contenitore,
            in_card: f.card,
            try_before_empty: f.prova_prima_di_svuotare,
            tick_tried: f.spunta_prova,
            protected: f.protetto,
            protected_exception: f.eccezione_protetti,
            privileges: if f.amministratore { Privileges::Admin } else { Privileges::None },
            estimate,
            method: f.metodo,
            alternatives: f.alternative,
            checks: f.controlli,
            manual_checks: f.controlli_manuali,
            hypotheses: f.ipotesi,
            risks: f.rischi,
            reversibility: f.reversibilita,
            on_error: f.errore,
            after: f.verifica,
            place: f.dove,
            versions: f.versioni,
            sources: f.fonti,
            verified,
            fixture: f.fixture,
        })
    }

    pub fn key(&self, field: &str) -> String {
        format!("regola-{}-{}", self.id, field)
    }

    /// Testo che il risultato di uno spostamento mostra prima di «Svuota il Cestino…»: cosa provare.
    pub fn try_key(&self) -> Option<String> {
        self.try_before_empty.then(|| self.key("prova"))
    }

    /// Casella che attiva «Svuota il Cestino…» nel risultato («Ho aperto CapCut e funziona»).
    pub fn tried_key(&self) -> Option<String> {
        self.tick_tried.then(|| self.key("prova-fatta"))
    }

    /// Tutte le chiavi di testo che la regola usa: devono esistere in italiano e in inglese.
    pub fn text_keys(&self) -> Vec<String> {
        let mut k = vec![self.key("nome"), self.key("riga")];
        if self.method.is_some() {
            k.push(self.key("metodo"));
        }
        for i in 1..=self.alternatives.len() {
            k.push(self.key(&format!("alternativa-{i}")));
        }
        for i in 1..=self.hypotheses {
            k.push(self.key(&format!("ipotesi-{i}")));
        }
        for i in 1..=self.risks {
            k.push(self.key(&format!("rischio-{i}")));
        }
        for c in &self.manual_checks {
            k.push(self.key(&format!("controllo-{c}")));
        }
        k.extend(self.try_key());
        k.extend(self.tried_key());
        for c in &self.checks {
            k.push(format!("controllo-{c}"));
        }
        k.push(format!("reversibilita-{}", self.reversibility));
        k.push(format!("errore-{}", self.on_error));
        k.push(format!("verifica-{}", self.after));
        k
    }
}

/// La base delle regole incorporata.
#[derive(Clone, Debug)]
pub struct Kb {
    pub rules: Vec<Rule>,
}

impl Kb {
    pub fn embedded() -> Result<Self, RuleError> {
        Self::from_files(RULE_FILES.iter().copied())
    }

    pub fn from_files<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> Result<Self, RuleError> {
        let mut rules = Vec::new();
        for (i, (name, text)) in files.into_iter().enumerate() {
            rules.push(Rule::parse(name, text).map_err(|issue| RuleError { file: i, issue })?);
        }
        rules.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.id.cmp(&b.id)));
        Ok(Self { rules })
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.rules.iter().position(|r| r.id == id)
    }
}
