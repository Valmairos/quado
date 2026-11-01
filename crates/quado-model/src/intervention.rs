// SPDX-License-Identifier: GPL-3.0-or-later
//! Interventi e verifica (03 F-AC, F-VE; 05 D7, D8): tipi condivisi tra `quado-actions` (controlli,
//! piano, esecuzione, registro), `quado-kb` (elementi protetti, riferimenti) e l'interfaccia. Solo
//! dati e codici: i testi li scrive l'interfaccia dai file di lingua (05 D13). I percorsi sono dati.
//!
//! Le funzioni pure che lavorano su questi tipi stanno in `verification` (stimato contro ottenuto).

use crate::record::Estimate;

// ---------------------------------------------------------------------------------------------
// Elementi protetti (F-AC-4)

/// Perché un percorso è protetto: mai spostabile nel Cestino. Solo `RegisteredProgram` ammette
/// l'eccezione via regola (03 §6.1, caso CapCut); le altre non hanno eccezioni.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProtectedKind {
    /// Cartella di Windows (`%WINDIR%`).
    WindowsDir,
    /// `Windows\WinSxS`.
    WinSxS,
    /// `Windows\Installer`.
    WindowsInstaller,
    /// `System Volume Information` del volume.
    SystemVolumeInformation,
    /// `pagefile.sys`.
    PagingFile,
    /// `hiberfil.sys`.
    Hibernation,
    /// `swapfile.sys`.
    SwapFile,
    /// Cartella di un programma registrato (voce di disinstallazione, gioco di Steam o di Epic, pacchetto
    /// MSIX): si rimanda alla disinstallazione (F-AP-5).
    RegisteredProgram,
}

impl ProtectedKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::WindowsDir => "cartella-windows",
            Self::WinSxS => "winsxs",
            Self::WindowsInstaller => "windows-installer",
            Self::SystemVolumeInformation => "system-volume-information",
            Self::PagingFile => "file-di-paging",
            Self::Hibernation => "ibernazione",
            Self::SwapFile => "swap",
            Self::RegisteredProgram => "programma-registrato",
        }
    }
}

/// Radice protetta: il percorso (assoluto, come lo scrive Windows, senza `\` finale) e il motivo.
/// Un percorso è protetto se coincide con una radice, sta sotto una radice o contiene una radice.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProtectedRoot {
    pub path: String,
    pub kind: ProtectedKind,
}

// ---------------------------------------------------------------------------------------------
// Riferimenti (F-AC-9)

/// Dove un riferimento all'elemento è stato trovato.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ReferenceKind {
    /// `UninstallString` di una voce di disinstallazione.
    UninstallString,
    /// `DisplayIcon` di una voce di disinstallazione.
    DisplayIcon,
    /// `InstallLocation` di una voce di disinstallazione.
    InstallLocation,
    /// `App Paths` (HKCU o HKLM).
    AppPaths,
    /// Chiave `Run` di HKCU.
    RunUser,
    /// Chiave `Run` di HKLM.
    RunMachine,
    /// Collegamento `.lnk` nel menu Start o sul Desktop, dell'utente o pubblico, letto senza `Resolve`.
    Shortcut,
    /// Attività pianificata visibile all'utente.
    ScheduledTask,
}

impl ReferenceKind {
    pub fn code(self) -> &'static str {
        match self {
            Self::UninstallString => "uninstall-string",
            Self::DisplayIcon => "display-icon",
            Self::InstallLocation => "install-location",
            Self::AppPaths => "app-paths",
            Self::RunUser => "run-hkcu",
            Self::RunMachine => "run-hklm",
            Self::Shortcut => "collegamento",
            Self::ScheduledTask => "attivita-pianificata",
        }
    }
}

/// Un riferimento che punta a un elemento della selezione.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Reference {
    pub kind: ReferenceKind,
    /// Dove sta il riferimento: chiave e valore del registro, percorso del `.lnk`, percorso dell'attività.
    pub location: String,
    /// Il percorso a cui punta, come scritto nella fonte (variabili d'ambiente già espanse).
    pub target: String,
}

/// Esito del controllo dei riferimenti su un insieme di percorsi.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReferenceReport {
    /// Riferimenti che puntano a uno dei percorsi o a qualcosa al loro interno.
    pub found: Vec<Reference>,
    /// Fonti non lette per intero (per esempio attività pianificate non visibili all'utente, codice di
    /// errore di Windows): il controllo è "parziale" e la conferma lo dice.
    pub partial: Vec<(ReferenceKind, u32)>,
}

// ---------------------------------------------------------------------------------------------
// Esecuzione (F-AC-3, F-AC-5, F-AC-8)

/// Perché un elemento confermato non è stato spostato senza che Windows lo toccasse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkipReason {
    /// ID, dimensione o data di modifica diversi dalla simulazione (RS6).
    Changed,
    /// Non esiste più.
    Missing,
    /// Restart Manager trova un processo che lo usa, al ricontrollo subito prima.
    InUse,
    /// Il ricontrollo non è riuscito a leggerlo: codice di errore di Windows.
    Unreadable(u32),
}

impl SkipReason {
    /// Codice stabile (registro e file di lingua).
    pub const fn code(self) -> &'static str {
        match self {
            Self::Changed => "cambiato",
            Self::Missing => "mancante",
            Self::InUse => "in-uso",
            Self::Unreadable(_) => "illeggibile",
        }
    }
}

/// Esito per elemento (riepilogo e registro).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemOutcome {
    /// Nel Cestino: `PostDeleteItem` ha restituito il nuovo elemento del Cestino.
    Recycled,
    /// Saltato prima di chiamare Windows.
    Skipped(SkipReason),
    /// Windows non lo ha spostato e l'elemento è ancora al suo posto (per esempio in uso): HRESULT.
    Failed(i32),
    /// Non più al suo posto ma non nel Cestino: il piano si è fermato qui (05, "Aggiornamento dopo M1").
    NotInRecycleBin(i32),
    /// Non raggiunto, perché il piano si è fermato prima o è stato annullato.
    NotReached,
}

impl ItemOutcome {
    pub fn code(self) -> &'static str {
        match self {
            Self::Recycled => "nel-cestino",
            Self::Skipped(_) => "saltato",
            Self::Failed(_) => "fallito",
            Self::NotInRecycleBin(_) => "non-nel-cestino",
            Self::NotReached => "non-raggiunto",
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Misure e verifica (F-VE)

/// Contenuto del Cestino di un volume secondo Windows (`SHQueryRecycleBinW`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BinCount {
    pub items: u64,
    pub bytes: u64,
}

/// Quando è stata presa una misura (F-VE-1, F-VE-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeasurePoint {
    /// Prima della prima chiamata a Windows o dell'apertura della procedura.
    Start,
    /// Dopo lo spostamento nel Cestino.
    AfterAction,
    /// Dopo lo svuotamento del Cestino.
    AfterEmpty,
    /// Dopo "Ho eseguito la procedura".
    AfterProcedure,
}

impl MeasurePoint {
    pub fn code(self) -> &'static str {
        match self {
            Self::Start => "partenza",
            Self::AfterAction => "dopo-azione",
            Self::AfterEmpty => "dopo-svuotamento",
            Self::AfterProcedure => "dopo-procedura",
        }
    }
}

/// Una misura del volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Measure {
    pub point: MeasurePoint,
    /// `FILETIME` della misura.
    pub at: i64,
    /// Spazio libero del volume (`GetDiskFreeSpaceExW`, byte liberi del volume).
    pub free: u64,
    /// Cestino del volume secondo Windows; `None` se non letto.
    pub bin: Option<BinCount>,
    /// Byte allocati che gli elementi dell'intervento occupano ancora nei loro percorsi originali,
    /// secondo la riscansione dei sottoalberi toccati. `None` alla partenza o se non riscansionati.
    pub still_present: Option<u64>,
}

/// Una voce che spiega la differenza tra stimato e ottenuto (03 §6.3): ognuna corrisponde a un fatto
/// misurato.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Explanation {
    /// Elementi saltati o falliti (riepilogo dell'esecuzione): i loro byte stimati.
    SkippedOrFailed { items: u64, bytes: u64 },
    /// File esclusi dalla stima perché hanno altri nomi fuori dalla selezione (rilettura in simulazione).
    ExcludedNames { files: u64, bytes: u64 },
    /// Contenuto ancora nel Cestino (dimensione del Cestino e riscansione).
    StillInBin { bytes: u64 },
    /// Altri elementi eliminati svuotando il Cestino (dimensione del Cestino alla partenza e dopo).
    OtherBinItems { items: u64, bytes: u64 },
    /// File ancora aperti al momento dello svuotamento (Restart Manager sui file della selezione).
    StillOpen { files: u64 },
    /// Attività di altri processi: la riscansione dice che la selezione non occupa più spazio, ma la
    /// variazione del volume è diversa di questi byte (con segno).
    OtherActivity { bytes: i64 },
}

impl Explanation {
    /// Codice stabile (registro e file di lingua).
    pub const fn code(self) -> &'static str {
        match self {
            Self::SkippedOrFailed { .. } => "saltati-o-falliti",
            Self::ExcludedNames { .. } => "altri-nomi",
            Self::StillInBin { .. } => "ancora-nel-cestino",
            Self::OtherBinItems { .. } => "altri-elementi-del-cestino",
            Self::StillOpen { .. } => "file-aperti",
            Self::OtherActivity { .. } => "altri-processi",
        }
    }
}

/// Stimato contro ottenuto a una misura (F-VE-3, 03 §6.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    pub estimated: Estimate,
    /// Variazione del volume: spazio libero alla misura meno spazio libero alla partenza.
    pub obtained: i64,
    /// Differenza secondo la tabella di 03 §6.3; `None` se la stima non ha un valore o vale 0.
    pub difference: Option<i64>,
    /// La stessa differenza in centesimi di punto percentuale (500 = 5%); `None` come sopra.
    pub basis_points: Option<u64>,
    /// Voci di spiegazione, solo oltre il 5%.
    pub explanations: Vec<Explanation>,
    /// Parte della differenza che le voci non spiegano (byte con segno), solo oltre il 5%.
    pub unexplained: Option<i64>,
}

/// Soglia oltre la quale la differenza si spiega voce per voce (F-VE-3): 5%.
pub const EXPLAIN_THRESHOLD_BP: u64 = 500;

// ---------------------------------------------------------------------------------------------
// Registro degli interventi (F-AC-6, 05 D8)

/// Tipo di intervento registrato.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InterventionKind {
    /// Spostamento nel Cestino da un piano confermato.
    Recycle,
    /// Procedura aperta (codice della procedura, per esempio `cestino-apri`, `file-temporanei`).
    Procedure(String),
    /// Comando ufficiale copiato (codice della regola che lo propone).
    Command(String),
}

/// Un elemento dell'intervento.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct JournalItem {
    pub path: String,
    /// Byte allocati stimati per l'elemento (0 se escluso dalla stima).
    pub bytes: u64,
    pub outcome: ItemOutcome,
}

/// Un intervento nel registro `%APPDATA%\Quado\interventi.jsonl`. Il registro è solo in aggiunta: ogni
/// riga è l'intervento completo, e per uno stesso `id` vale l'ultima riga (le misure arrivano dopo).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalEntry {
    /// Identificativo: `FILETIME` della misura di partenza (unico nel registro).
    pub id: i64,
    /// Radice del volume (`C:\`).
    pub volume: String,
    pub kind: InterventionKind,
    /// Regola o funzione che ha prodotto il record (`Record::origin_id`).
    pub origin: String,
    /// Metodo: codice dell'azione (`Action::code`) o comando e URI della procedura.
    pub method: String,
    /// Stima con il suo stato (03 §6.3) al momento della conferma.
    pub estimate: Estimate,
    pub items: Vec<JournalItem>,
    /// Misure nell'ordine in cui sono state prese; la prima è la partenza.
    pub measures: Vec<Measure>,
    /// Confronto all'ultima misura, se c'è.
    pub comparison: Option<Comparison>,
}
