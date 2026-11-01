// SPDX-License-Identifier: GPL-3.0-or-later
//! Vista Programmi (03 F-AP, 05 D6): elenco dai programmi registrati, MSIX, Steam ed Epic; spazio solo
//! con una prova del percorso, ogni byte con un solo proprietario (prefisso più lungo); dichiarato
//! contro misurato; uso noto; disinstallazione tramite interfaccia ufficiale; possibili residui.
//!
//! - Elenco (F-AP-1): voci di disinstallazione con `DisplayName` delle tre chiavi (quelle con
//!   `SystemComponent=1` nascoste per default), pacchetti MSIX non framework, giochi Steam, prodotti
//!   Epic. Gli errori degli estrattori arrivano come codici, mai con dati ricostruiti.
//! - Spazio (F-AP-2): cartella da `InstallLocation`, dall'indizio di `UninstallString`/`DisplayIcon`
//!   (solo un percorso, mai eseguito), da `InstalledLocation` di MSIX o dai manifest dei launcher, con
//!   gli stessi criteri del motore (`engine.rs`). Ogni byte ha un solo proprietario: la cartella di
//!   programma più profonda che lo contiene; più programmi con la stessa cartella la condividono e i
//!   byte si contano una volta. Gli elementi che una regola dichiara tuoi non si contano e producono
//!   l'avviso "tuoi file" (F-AP-5). Fuori dalla cartella contano solo i dati delle regole che dichiarano
//!   come gestore proprio quel programma (nome uguale, senza somiglianze). Senza prova: "posizione non
//!   dichiarata" e nessun byte.
//! - Dichiarato contro misurato (F-AP-3): `EstimatedSize` e `SizeOnDisk` restano fatti da fonte, mai
//!   sommati al misurato.
//! - Uso noto (F-AP-4): `LastPlayed` di Steam; UserAssist solo con il tracciamento attivo, abbinato per
//!   cartella; `InstallDate`; per Epic e MSIX nessun dato d'uso. Lo stato resta Decidi tu.
//! - Possibili residui (F-AP-6): cartelle di primo livello in Local, Roaming, LocalLow e ProgramData che
//!   nessun programma e nessuna regola rivendicano e che nessun nome di programma installato spiega: il
//!   confronto è per parole, e una cartella è spiegata se tutte le parole del suo nome stanno in uno stesso
//!   nome dichiarato da un programma (`name_words`, `NameIndex`). Stato Non sappiamo, nessuna azione.
//!
//! Modulo puro: niente I/O, niente `unsafe`. Il costo non dipende dal numero di nodi dell'albero, salvo
//! la visita dei sottoalberi dei possibili residui (data di ultima modifica).

use std::collections::{HashMap, HashSet};

use quado_model::record::{
    Action, Check, CheckKind, CheckOutcome, Criterion, Date, Estimate, Expiry, Fact, Function, Hive, Manager, NotEstimable, Origin, Privileges,
    ProvaDiIdentita, Provenance, Record, Source, State, Step, UnknownReason, Value,
};
use quado_model::{Flags, NONE, Tree};

use crate::engine::{self, Classification};
use crate::evidence::Evidence;
use crate::extract::{self, ExtractError, TrackProgs, UserAssist, UserAssistEntry};
use crate::rules::Kb;

// ---------------------------------------------------------------------------------------------
// Tipi pubblici

/// Da quale estrattore viene un programma (F-AP-1).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProgramSource {
    /// Voce di disinstallazione: hive, nome della chiave e indice in `Uninstall::entries`.
    Uninstall { hive: Hive, key: String, entry: usize },
    /// Pacchetto MSIX non framework, con il nome di famiglia (PFN).
    Msix { family_name: String },
    /// Gioco di Steam dal suo manifest.
    Steam { appid: u32 },
    /// Prodotto di Epic dal suo manifest (`AppName`).
    Epic { app_name: String },
}

/// Prova del percorso di un programma (F-AP-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathEvidence {
    /// `InstallLocation` della voce di disinstallazione.
    InstallLocation,
    /// Cartella dell'eseguibile indicato da `DisplayIcon` o `UninstallString`: solo un indizio di percorso,
    /// mai eseguito (`UninstallEntry::path_hint`).
    UninstallHint,
    /// `InstalledLocation` del pacchetto MSIX.
    MsixInstalled,
    /// Cartella del gioco secondo `libraryfolders.vdf` e `appmanifest_*.acf`.
    SteamManifest,
    /// `InstallLocation` del manifest di Epic.
    EpicManifest,
}

/// Posizione di un programma.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Location {
    /// Nessuna prova del percorso: "posizione non dichiarata", nessun byte dalla cartella.
    NotDeclared,
    /// Percorso dichiarato che non può essere la cartella di un programma (radice del disco, cartella nota
    /// di Windows o una cartella che ne contiene una): nessun byte, come nel motore.
    Implausible { path: String, evidence: PathEvidence },
    /// Cartella del programma con la prova e la provenienza, come nel motore: da fonte con
    /// `InstallLocation` di HKLM e con i manifest; dedotto media con le voci di HKCU (RS22) e con gli
    /// indizi, alta se l'eseguibile dell'indizio di HKLM c'è o con due fonti concordi (CapCut). `node` è il
    /// nodo della scansione; `None` se la cartella non c'è nella scansione (altro volume, cartella assente).
    Folder { path: String, evidence: PathEvidence, provenance: Provenance, node: Option<u32> },
}

impl Location {
    pub fn node(&self) -> Option<u32> {
        match self {
            Self::Folder { node, .. } => *node,
            _ => None,
        }
    }
}

/// Spazio misurato di un programma (allocata, "Dimensioni su disco").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Space {
    /// Nessuna prova del percorso e nessun dato dalle regole: nessun byte attribuito (0).
    NotAttributed,
    /// Cartella dichiarata ma non nella scansione o non leggibile, e nessun dato dalle regole: non
    /// misurato, mai 0.
    NotMeasured,
    /// Byte della cartella (senza le cartelle di altri programmi al suo interno e senza i tuoi file) più
    /// i dati delle regole fuori dalla cartella. Se la cartella non è leggibile contano solo i dati.
    Measured(u64),
}

impl Space {
    /// Byte attribuiti: 0 salvo `Measured`.
    pub fn bytes(self) -> u64 {
        match self {
            Self::Measured(b) => b,
            _ => 0,
        }
    }
}

/// Elemento riconosciuto da una regola e attribuito al programma come suoi dati.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleData {
    pub node: u32,
    /// Indice in `Classification::matches`: la regola citata è `matches[matched].rule`.
    pub matched: usize,
    /// Indice della regola in `Kb::rules`.
    pub rule: usize,
    /// Byte che la regola spiega (`Classification::own_bytes`).
    pub bytes: u64,
    /// Dentro la cartella del programma (gestore Programmi); falso per i dati fuori, attribuiti perché la
    /// regola dichiara come gestore questo programma, o per la cartella dati di un pacchetto MSIX.
    pub inside: bool,
}

/// Tuoi file nella cartella di un programma (F-AP-5): elementi che una regola dichiara tuoi. Non si contano
/// nel misurato; prima della disinstallazione la UI chiede di copiarli altrove (04 §6.8).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct YourFiles {
    pub node: u32,
    /// Indice in `Classification::matches`.
    pub matched: usize,
    /// Indice della regola in `Kb::rules`.
    pub rule: usize,
    /// Allocata dell'elemento.
    pub bytes: u64,
    /// Percorso relativo alla cartella del programma (o al suo ambito, `Program::area`): la
    /// "sottocartella" del testo.
    pub relative: String,
}

/// Dimensione dichiarata (F-AP-3): `EstimatedSize` (KB convertiti in byte) o `SizeOnDisk` di Steam.
/// Sempre con la sua fonte, mai sommata al misurato.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Declared {
    pub bytes: u64,
    pub source: Source,
}

/// Avvii registrati da UserAssist per gli eseguibili nella cartella del programma.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Launches {
    /// Ultimo avvio (FILETIME), il più recente tra gli eseguibili della cartella.
    pub last: i64,
    /// Avvii contati da UserAssist, sommati tra gli eseguibili.
    pub runs: u64,
}

/// Uso noto (F-AP-4): solo da fonti che lo registrano, ciascuna con la sua copertura. Mai "inutilizzato":
/// lo stato resta Decidi tu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Usage {
    /// `LastPlayed` di Steam (FILETIME) dal manifest e da `localconfig.vdf`; `None` se la fonte non lo ha.
    Steam { manifest: Option<i64>, local_config: Option<i64> },
    /// UserAssist con il tracciamento attivo: avvii da Esplora file e Start degli eseguibili nella cartella
    /// del programma. `None`: nessuna voce per quella cartella (non conta gli avvii da riga di comando,
    /// da altri launcher e da attività pianificate).
    UserAssist(Option<Launches>),
    /// Tracciamento attivo ma il programma non ha una cartella: UserAssist non si abbina.
    NoFolder,
    /// Tracciamento degli avvii non attivo (`Start_TrackProgs` diverso da 1, o non letto): nessun valore
    /// UserAssist, anche se la chiave contiene dati vecchi.
    NotTracked(TrackProgs),
    /// Tracciamento attivo ma UserAssist non letto.
    Unreadable(ExtractError),
    /// La piattaforma non registra l'uso (Epic, MSIX).
    NotRecorded,
}

/// Metodo di disinstallazione ufficiale (F-AP-5). Quado non esegue mai `UninstallString`: la UI apre
/// l'interfaccia con `quado_actions::launch` (Impostazioni ▸ App copiando `name` negli appunti, pagina
/// dell'app MSIX, `steam://uninstall/<appid>`) o rimanda alla libreria di Epic.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum UninstallVia {
    Settings { name: String },
    SettingsApp { family_name: String },
    Steam { appid: u32 },
    EpicLibrary,
}

/// Un programma installato.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    /// `DisplayName`, nome del gioco, `DisplayName` di Epic o nome di famiglia MSIX.
    pub name: String,
    /// `DisplayVersion` o `AppVersionString` di Epic.
    pub version: Option<String>,
    /// `Publisher` della voce di disinstallazione.
    pub publisher: Option<String>,
    pub source: ProgramSource,
    /// `SystemComponent=1`: nascosto per default (F-AP-1); Impostazioni ▸ App non lo elenca, quindi
    /// nessun metodo di disinstallazione.
    pub hidden: bool,
    pub location: Location,
    /// Cartella che contiene la cartella del programma e i suoi dati fuori dalla cartella, quando è sotto
    /// una cartella nota e non contiene altri programmi (CapCut: `Local\CapCut` intorno ad `Apps` e a
    /// `User Data\Cache`): lì si cercano anche i tuoi file. Non aggiunge byte.
    pub area: Option<u32>,
    pub space: Space,
    /// Indici in `Programs::items` dei programmi con la stessa cartella: lo spazio è lo stesso e nel
    /// totale si conta una volta.
    pub shared_with: Vec<usize>,
    /// Dati dalle regole: dentro la cartella (già nel misurato) e fuori (aggiunti al misurato).
    pub data: Vec<RuleData>,
    pub your_files: Vec<YourFiles>,
    pub declared: Option<Declared>,
    pub usage: Usage,
    /// `InstallDate` valida (`AAAAMMGG`, data esistente, non dopo oggi).
    pub install_date: Option<Date>,
    /// `None` per le voci nascoste e per i valori non validi (PFN, appid 0).
    pub uninstall: Option<UninstallVia>,
    /// Decidi tu, `Action::Uninstall` (o nessuna azione senza metodo), origine `Function(Program)`.
    pub record: Record,
}

/// Cartella nota sotto cui si cercano i possibili residui.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResidueBase {
    Local,
    Roaming,
    LocalLow,
    ProgramData,
}

/// Cartella che potrebbe essere un residuo (F-AP-6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Residue {
    /// Cartella di primo livello sotto la base.
    pub node: u32,
    pub base: ResidueBase,
    /// Ultima modifica più recente nel sottoalbero (FILETIME); `None` se la scansione non ha date.
    pub last_modified: Option<i64>,
    /// Non sappiamo (solo nome), nessuna azione, origine `Function(PossibleResidue)`.
    pub record: Record,
}

/// Estrattore che alimenta la vista.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Extractor {
    Uninstall,
    Msix,
    Steam,
    Epic,
    UserAssist,
}

/// Estrattore non riuscito, o un suo elemento saltato per un formato inatteso: codice, mai testo.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SourceError {
    pub extractor: Extractor,
    pub error: ExtractError,
    /// Chiave di registro o file dell'elemento saltato; `None` se non è stato letto l'estrattore intero.
    pub path: Option<String>,
}

/// La vista Programmi.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Programs {
    /// Voci di disinstallazione, poi pacchetti MSIX, giochi Steam, prodotti Epic, ciascuno nell'ordine del
    /// suo estrattore.
    pub items: Vec<Program>,
    pub residues: Vec<Residue>,
    pub errors: Vec<SourceError>,
    /// `Start_TrackProgs` letto con la scansione: con un valore diverso da `On` nessun dato UserAssist.
    pub tracking: TrackProgs,
}

impl Programs {
    /// Byte misurati di tutti i programmi: una cartella condivisa conta una volta (06 §4).
    pub fn measured_total(&self) -> u64 {
        self.items.iter().enumerate().filter(|(i, p)| p.shared_with.iter().all(|j| j > i)).map(|(_, p)| p.space.bytes()).sum()
    }

    /// Programmi da mostrare per default (senza `SystemComponent=1`).
    pub fn visible(&self) -> impl Iterator<Item = &Program> {
        self.items.iter().filter(|p| !p.hidden)
    }
}

// ---------------------------------------------------------------------------------------------
// Supporto

fn fact(key: &str, p: Provenance, s: Source, v: Vec<Value>) -> Fact {
    Fact::new(key, p, s, v)
}

fn text(s: &str) -> Value {
    Value::Text(s.to_owned())
}

/// Un percorso dichiarato non è la cartella di un programma se è la radice di un'unità, una cartella nota
/// o un suo antenato. Copia di `engine::Ctx::plausible_program_dir`, senza il controllo del volume: qui un
/// percorso su un altro volume è una cartella dichiarata ma non misurata.
fn plausible_program_dir(ev: &Evidence, path: &str) -> bool {
    let p = extract::normalize(path).to_lowercase();
    if p.len() <= 3 {
        return false;
    }
    for k in ev.folders.all() {
        let k = k.to_lowercase();
        if k == p || k.starts_with(&format!("{p}\\")) {
            return false;
        }
    }
    true
}

fn on_volume(ev: &Evidence, path: &str) -> bool {
    let root = ev.volume_root.to_lowercase();
    let p = path.to_lowercase();
    !root.is_empty() && p.starts_with(&root)
}

/// Secondi Unix in FILETIME, senza traboccare su valori malformati. Come `engine::unix_to_filetime`.
fn unix_to_filetime(secs: i64) -> Option<i64> {
    secs.checked_add(11_644_473_600)?.checked_mul(10_000_000)
}

/// Giorni dal 1/1/1970 (algoritmo civile di Howard Hinnant).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Una data del calendario come FILETIME alle 12:00 UTC: la stessa data in ogni fuso tra -12 e +11 ore,
/// quando la UI la scrive in ora locale. Il valore esatto resta in `Program::install_date`.
fn date_filetime(d: Date) -> i64 {
    let days = days_from_civil(i64::from(d.year), i64::from(d.month), i64::from(d.day));
    (days * 86_400 + 43_200 + 11_644_473_600) * 10_000_000
}

/// `InstallDate` (`AAAAMMGG`) validata: otto cifre, data esistente, dal 1980, non dopo oggi. Altrimenti
/// si ignora.
pub fn parse_install_date(s: &str, today: Option<Date>) -> Option<Date> {
    let s = s.trim();
    if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let d = Date::parse(&format!("{}-{}-{}", &s[..4], &s[4..6], &s[6..]))?;
    if d.year < 1980 || today.is_some_and(|t| d > t) {
        return None;
    }
    Some(d)
}

/// Nome senza separatori: solo lettere e cifre, in minuscolo ("Copper Kite" e `CopperKite`
/// coincidono). È anche l'unione di tutte le parole di `name_words`.
pub fn name_key(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Parole di un nome per il confronto dei residui (F-AP-6): sequenze di lettere o di cifre, in
/// minuscolo. Una parola finisce a ogni carattere che non è una lettera o una cifra, al passaggio tra
/// lettere e cifre (`7zip` e "7-Zip" danno `7`, `zip`; `Win11` dà `win`, `11`) e al CamelCase: tra una
/// minuscola e una maiuscola (`CopperKite`) e prima dell'ultima maiuscola di una sigla seguita da una
/// minuscola (`HTMLParser` dà `html`, `parser`; `NVIDIA` resta una parola). Il taglio è lo stesso per
/// la cartella e per i nomi dei programmi, quindi due scritture dello stesso nome hanno le stesse parole.
/// Le parole di una lettera contano come le altre ("A Paper Lagoon", la `x` di "x64"): toglierle dalla
/// cartella renderebbe `R` o `C` rivendicabili da qualunque programma, toglierle dai programmi
/// restringerebbe il confronto.
pub fn name_words(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        if !cur.is_empty() {
            out.push(std::mem::take(cur));
        }
    };
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            flush(&mut cur, &mut out);
            continue;
        }
        if let Some(&p) = i.checked_sub(1).and_then(|j| chars.get(j))
            && p.is_alphanumeric()
        {
            let digits = p.is_numeric() != c.is_numeric();
            let camel = p.is_lowercase() && c.is_uppercase();
            let acronym = p.is_uppercase() && c.is_uppercase() && chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if digits || camel || acronym {
                flush(&mut cur, &mut out);
            }
        }
        cur.extend(c.to_lowercase());
    }
    flush(&mut cur, &mut out);
    out
}

/// Nomi dei programmi installati per il confronto dei residui. Un candidato è un nome che un programma
/// dichiara: nome del programma, editore, nome di famiglia MSIX e sua parte nome, `installdir` di Steam,
/// `AppName` di Epic. Ogni candidato porta le sue parole e le unioni di 2 o 3 parole consecutive
/// ("Python 3.11" spiega `Python311`, "Node.js 20" spiega `nodejs`).
struct NameIndex {
    /// Parola o unione: i candidati che la contengono, in ordine e senza ripetizioni.
    by_word: HashMap<String, Vec<u32>>,
    /// Nomi interi senza separatori (`name_key`).
    whole: HashSet<String>,
}

impl NameIndex {
    fn new<'a>(candidates: impl Iterator<Item = &'a str>) -> Self {
        let mut by_word: HashMap<String, Vec<u32>> = HashMap::new();
        let mut whole = HashSet::new();
        for (id, c) in candidates.enumerate() {
            let id = id as u32;
            let words = name_words(c);
            if words.is_empty() {
                continue;
            }
            whole.insert(words.concat());
            for i in 0..words.len() {
                for j in i + 1..=(i + 3).min(words.len()) {
                    let ids = by_word.entry(words[i..j].concat()).or_default();
                    if ids.last() != Some(&id) {
                        ids.push(id);
                    }
                }
            }
        }
        Self { by_word, whole }
    }

    /// Una cartella è rivendicata per nome se tutte le parole del suo nome compaiono tra le parole (o le
    /// unioni) di uno stesso candidato, oppure se il nome senza separatori è quello di un candidato
    /// ("Thunderbird" con "Mozilla Thunderbird", `Epic Games` con "Epic Games Launcher", `Copperkite` con
    /// "Copper Kite"). Parole sparse tra candidati diversi non bastano. È il lato prudente: meno cartelle
    /// indicate come possibili residui. Un nome senza lettere né cifre non è rivendicato.
    fn claims(&self, folder: &str) -> bool {
        let words = name_words(folder);
        if words.is_empty() {
            return false;
        }
        if self.whole.contains(&words.concat()) {
            return true;
        }
        let mut lists: Vec<&Vec<u32>> = Vec::with_capacity(words.len());
        for w in &words {
            match self.by_word.get(w) {
                Some(ids) => lists.push(ids),
                None => return false,
            }
        }
        lists.sort_by_key(|l| l.len());
        lists[0].iter().any(|id| lists[1..].iter().all(|l| l.binary_search(id).is_ok()))
    }
}

/// Forma del PFN per l'URI di Impostazioni: la stessa che `quado_actions::launch::Pfn` accetta.
fn pfn_for_uri(pfn: &str) -> bool {
    match pfn.rsplit_once('_') {
        Some((name, id)) => {
            (3..=50).contains(&name.len())
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
                && id.len() == 13
                && id.bytes().all(|b| b.is_ascii_alphanumeric())
        }
        None => false,
    }
}

/// Cartelle note identificate in UserAssist con il loro KNOWNFOLDERID (`{GUID}\resto`).
fn known_folder_by_id(id: &str, ev: &Evidence) -> Option<String> {
    let f = &ev.folders;
    let windows = f.windows.as_deref();
    let sub = |base: Option<&str>, rel: &str| base.map(|b| extract::join(b, rel));
    match id.to_ascii_uppercase().as_str() {
        "{6D809377-6AF0-444B-8957-A3773F02200E}" | "{905E63B6-C1BF-494E-B29C-65B732D3D21A}" => f.program_files.clone(),
        "{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}" => f.program_files_x86.clone(),
        "{6365D5A7-0F0D-45E5-87F6-0DA56B6A4F7D}" => sub(f.program_files.as_deref(), "Common Files"),
        "{DE974D24-D9C6-4D3E-BF91-F4455120B917}" => sub(f.program_files_x86.as_deref(), "Common Files"),
        "{F38BF404-1D43-42F2-9305-67DE0B28FC23}" => f.windows.clone(),
        "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}" => sub(windows, "System32"),
        "{D65231B0-B2F1-4857-A4CE-A8E7C6EA7D27}" => sub(windows, "SysWOW64"),
        "{F1B32785-6FBA-4FCF-9D55-7B8E7F157091}" => f.local.clone(),
        "{3EB685DB-65F9-4CF6-A03A-E3EF65729F3D}" => f.roaming.clone(),
        "{A520A1A4-1780-4FF6-BD18-167343C5AF16}" => f.local_low.clone(),
        "{62AB5D82-FDC1-4DC3-A9DD-070D1D495D97}" => f.program_data.clone(),
        "{5E6C858F-0E22-4760-9AFE-EA3317B67173}" => f.profile.clone(),
        "{B4BFCC3A-DB2C-424C-B029-7FE99A87C641}" => f.desktop.clone(),
        "{FDD39AD0-238F-46AF-ADB4-6C85480369C7}" => f.documents.clone(),
        "{374DE290-123F-4565-9164-39C4925E467B}" => f.downloads.clone(),
        _ => None,
    }
}

/// Percorso dell'eseguibile di una voce UserAssist, in minuscolo: assoluto, oppure con il KNOWNFOLDERID
/// in testa. Gli identificativi delle app (`...!App`) e i collegamenti non sono percorsi.
fn userassist_path(name: &str, ev: &Evidence) -> Option<String> {
    let p = if extract::is_absolute(name) {
        extract::normalize(name)
    } else if name.starts_with('{') {
        let (id, rest) = name.split_once('}')?;
        let base = known_folder_by_id(&format!("{id}}}"), ev)?;
        extract::join(&base, rest.trim_start_matches(['\\', '/']))
    } else {
        return None;
    };
    let p = extract::normalize(&p);
    (extract::is_absolute(&p) && !p.contains("..")).then(|| p.to_lowercase())
}

/// Percorso relativo da `top` a `node` (`top` antenato), con `\`.
fn relative(tree: &Tree, top: u32, node: u32) -> String {
    let mut parts = Vec::new();
    let mut n = node;
    while n != NONE && n != top {
        parts.push(tree.name_lossy(n));
        n = tree.parent(n);
    }
    parts.reverse();
    parts.join("\\")
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

/// Antenato comune più profondo.
fn common_ancestor(tree: &Tree, a: u32, b: u32) -> u32 {
    let mut up: Vec<u32> = Vec::new();
    let mut n = a;
    while n != NONE {
        up.push(n);
        n = tree.parent(n);
    }
    let mut m = b;
    while m != NONE {
        if up.contains(&m) {
            return m;
        }
        m = tree.parent(m);
    }
    Tree::ROOT
}

// ---------------------------------------------------------------------------------------------
// Raccolta

/// Programma letto da un estrattore, prima dello spazio.
struct Raw {
    name: String,
    version: Option<String>,
    publisher: Option<String>,
    source: ProgramSource,
    /// Fonte dei fatti del programma.
    fact_source: Source,
    hidden: bool,
    location: Location,
    /// Criterio della prova di identità e di percorso (03 §6.2): l'affidabilità del record.
    criterion: Criterion,
    declared: Option<Declared>,
    install_date: Option<Date>,
    steam: Option<(Option<i64>, Option<i64>)>,
    uninstall: Option<UninstallVia>,
    /// Fatti di identità letti dalla fonte (nome, versione, editore, pacchetto, gioco).
    identity: Vec<Fact>,
    /// Altri nomi per il confronto dei residui (cartella del gioco, nome di famiglia, `AppName`).
    aliases: Vec<String>,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Win32,
    Msix,
    Steam,
    Epic,
}

/// Cartella dichiarata: plausibile e, se è su questo volume, cercata nella scansione.
fn locate(tree: &Tree, ev: &Evidence, path: &str, evidence: PathEvidence, provenance: Provenance) -> Location {
    let p = extract::normalize(path);
    if !extract::is_absolute(&p) {
        return Location::NotDeclared;
    }
    if !plausible_program_dir(ev, &p) {
        return Location::Implausible { path: p, evidence };
    }
    let node = if on_volume(ev, &p) { engine::find(tree, &p) } else { None };
    Location::Folder { path: p, evidence, provenance, node }
}

/// Voci di disinstallazione, con i criteri di `engine::Ctx::registered` (05 D6, RS22).
fn collect_uninstall(tree: &Tree, ev: &Evidence, today: Option<Date>, out: &mut Vec<Raw>) {
    let Ok(u) = &ev.uninstall else { return };
    for (i, e) in u.entries.iter().enumerate() {
        let src = Source::Uninstall(e.hive);
        let machine = e.hive != Hive::User;
        let (path, declared) = match (e.location(), e.path_hint()) {
            (Some(p), _) => (Some(p), true),
            (None, Some(p)) => (Some(p), false),
            (None, None) => (None, false),
        };
        let mut criterion = if machine { Criterion::OfficialWithStructure } else { Criterion::OneSource };
        let location = match &path {
            None => Location::NotDeclared,
            Some(p) => {
                // HKLM con InstallLocation: da fonte. HKCU o solo indizio: dedotto media (05 D6, RS22).
                let mut c = if declared && machine { None } else { Some(Criterion::OneSource) };
                let loc = locate(tree, ev, p, if declared { PathEvidence::InstallLocation } else { PathEvidence::UninstallHint }, Provenance::FromSource);
                if let Some(n) = loc.node() {
                    // Indizio da HKLM con l'eseguibile presente: fonte di Windows con la struttura che dichiara.
                    if !declared && machine && e.hint_exe().is_some_and(|x| engine::find(tree, &x).is_some()) {
                        c = Some(Criterion::OfficialWithStructure);
                    }
                    // CapCut: seconda fonte `ProductInfo.xml` nella stessa cartella, con la stessa versione.
                    if c.is_some()
                        && e.display_name.eq_ignore_ascii_case("CapCut")
                        && let (Ok(v), Some(dv)) = (&ev.capcut, &e.version)
                        && v == dv
                        && engine::child(tree, n, "ProductInfo.xml").is_some()
                    {
                        c = Some(Criterion::TwoSources);
                    }
                }
                criterion = c.unwrap_or(Criterion::OfficialWithStructure);
                match loc {
                    Location::Folder { path, evidence, node, .. } => {
                        let provenance = if c.is_none() { Provenance::FromSource } else { Provenance::Deduced(criterion) };
                        Location::Folder { path, evidence, provenance, node }
                    }
                    other => other,
                }
            }
        };
        let mut identity = Vec::new();
        let mut vals = vec![text(&e.display_name)];
        if let Some(v) = &e.version {
            vals.push(text(v));
        }
        identity.push(fact("fatto-programma-installato", Provenance::FromSource, src.clone(), vals));
        if let Some(p) = e.publisher.as_deref().filter(|p| !p.trim().is_empty()) {
            identity.push(fact("fatto-programma-editore", Provenance::FromSource, src.clone(), vec![text(p)]));
        }
        if e.system_component {
            identity.push(fact("fatto-programma-componente-di-sistema", Provenance::FromSource, src.clone(), Vec::new()));
        }
        out.push(Raw {
            name: e.display_name.clone(),
            version: e.version.clone(),
            publisher: e.publisher.clone(),
            source: ProgramSource::Uninstall { hive: e.hive, key: e.key.clone(), entry: i },
            fact_source: src.clone(),
            hidden: e.system_component,
            location,
            criterion,
            declared: e.estimated_kb.map(|kb| Declared { bytes: u64::from(kb) * 1024, source: src.clone() }),
            install_date: e.install_date.as_deref().and_then(|d| parse_install_date(d, today)),
            steam: None,
            uninstall: (!e.system_component).then(|| UninstallVia::Settings { name: e.display_name.clone() }),
            identity,
            aliases: Vec::new(),
            kind: Kind::Win32,
        });
    }
}

fn collect_msix(tree: &Tree, ev: &Evidence, out: &mut Vec<Raw>) {
    let Ok(pkgs) = &ev.msix else { return };
    for p in pkgs.iter().filter(|p| !p.framework) {
        let location = match &p.installed_path {
            Some(path) => locate(tree, ev, path, PathEvidence::MsixInstalled, Provenance::FromSource),
            None => Location::NotDeclared,
        };
        let mut aliases = vec![p.family_name.clone()];
        if let Some((name, _)) = p.family_name.rsplit_once('_') {
            aliases.push(name.to_owned());
        }
        out.push(Raw {
            name: p.family_name.clone(),
            version: None,
            publisher: None,
            source: ProgramSource::Msix { family_name: p.family_name.clone() },
            fact_source: Source::Msix,
            hidden: false,
            location,
            criterion: Criterion::OfficialWithStructure,
            declared: None,
            install_date: None,
            steam: None,
            uninstall: pfn_for_uri(&p.family_name).then(|| UninstallVia::SettingsApp { family_name: p.family_name.clone() }),
            identity: vec![fact("fatto-pacchetto-msix", Provenance::FromSource, Source::Msix, vec![text(&p.family_name)])],
            aliases,
            kind: Kind::Msix,
        });
    }
}

fn collect_steam(tree: &Tree, ev: &Evidence, out: &mut Vec<Raw>) {
    let Ok(s) = &ev.steam else { return };
    for app in &s.apps {
        let local = s.local_last_played.iter().find(|(id, _)| *id == app.appid).and_then(|&(_, t)| unix_to_filetime(t));
        let manifest = app.last_played.and_then(unix_to_filetime);
        out.push(Raw {
            name: app.name.clone(),
            version: None,
            publisher: None,
            source: ProgramSource::Steam { appid: app.appid },
            fact_source: Source::SteamManifest,
            hidden: false,
            location: locate(tree, ev, &app.folder(), PathEvidence::SteamManifest, Provenance::FromSource),
            criterion: Criterion::OfficialWithStructure,
            declared: app.size_on_disk.map(|b| Declared { bytes: b, source: Source::SteamManifest }),
            install_date: None,
            steam: Some((manifest, local)),
            uninstall: (app.appid != 0).then_some(UninstallVia::Steam { appid: app.appid }),
            identity: vec![fact("fatto-gioco-steam", Provenance::FromSource, Source::SteamManifest, vec![text(&app.name), text(&app.appid.to_string())])],
            aliases: vec![app.installdir.clone()],
            kind: Kind::Steam,
        });
    }
}

/// Appid di una voce di disinstallazione scritta da Steam (`Steam App 3999910`).
fn steam_entry_appid(key: &str) -> Option<u32> {
    let (prefix, id) = key.split_at_checked("Steam App ".len())?;
    if !prefix.eq_ignore_ascii_case("Steam App ") {
        return None;
    }
    id.trim().parse().ok().filter(|&n| n != 0)
}

/// Le voci "Steam App N" del registro sono gli stessi giochi dei manifest di Steam: una riga sola, quella
/// di Steam (uso noto, disinstallazione con `steam://uninstall/`), con il fatto della voce di
/// disinstallazione e la sua data di installazione. Prima comparivano due righe con la cartella condivisa
/// (nelle immagini di M4 alcuni giochi di Steam comparivano due volte).
fn merge_steam_entries(raws: &mut Vec<Raw>) {
    let steam: HashMap<u32, usize> = raws
        .iter()
        .enumerate()
        .filter_map(|(i, r)| match r.source {
            ProgramSource::Steam { appid } => Some((appid, i)),
            _ => None,
        })
        .collect();
    let mut merged: Vec<usize> = Vec::new();
    for i in 0..raws.len() {
        let ProgramSource::Uninstall { key, .. } = &raws[i].source else { continue };
        let Some(&j) = steam_entry_appid(key).and_then(|id| steam.get(&id)) else { continue };
        let entry: Vec<Fact> = raws[i].identity.iter().filter(|f| f.key == "fatto-programma-installato").cloned().collect();
        let date = raws[i].install_date;
        raws[j].identity.extend(entry);
        if raws[j].install_date.is_none() {
            raws[j].install_date = date;
        }
        merged.push(i);
    }
    for i in merged.into_iter().rev() {
        raws.remove(i);
    }
}

fn collect_epic(tree: &Tree, ev: &Evidence, out: &mut Vec<Raw>) {
    let Ok(e) = &ev.epic else { return };
    for it in &e.items {
        let mut identity = vec![fact("fatto-installazione-epic", Provenance::FromSource, Source::EpicManifest, vec![text(&it.display_name)])];
        if let Some(v) = &it.version {
            identity.push(fact("fatto-programma-versione", Provenance::FromSource, Source::EpicManifest, vec![text(v)]));
        }
        out.push(Raw {
            name: it.display_name.clone(),
            version: it.version.clone(),
            publisher: None,
            source: ProgramSource::Epic { app_name: it.app_name.clone() },
            fact_source: Source::EpicManifest,
            hidden: false,
            location: locate(tree, ev, &it.install_location, PathEvidence::EpicManifest, Provenance::FromSource),
            criterion: Criterion::OfficialWithStructure,
            declared: None,
            install_date: None,
            steam: None,
            uninstall: Some(UninstallVia::EpicLibrary),
            identity,
            aliases: vec![it.app_name.clone()],
            kind: Kind::Epic,
        });
    }
}

fn collect_errors(ev: &Evidence) -> Vec<SourceError> {
    let mut out = Vec::new();
    let whole = |out: &mut Vec<SourceError>, extractor, e: &ExtractError| out.push(SourceError { extractor, error: *e, path: None });
    match &ev.uninstall {
        Err(e) => whole(&mut out, Extractor::Uninstall, e),
        Ok(u) => {
            for (hive, key, issue) in &u.skipped {
                let base = extract::UNINSTALL_KEYS.iter().find(|(_, _, h)| h == hive).map(|(root, p, _)| {
                    let r = match root {
                        crate::reg::Root::LocalMachine => "HKEY_LOCAL_MACHINE",
                        crate::reg::Root::CurrentUser => "HKEY_CURRENT_USER",
                    };
                    format!(r"{r}\{p}")
                });
                let path = base.map(|b| format!(r"{b}\{key}"));
                out.push(SourceError { extractor: Extractor::Uninstall, error: ExtractError::Format(*issue), path });
            }
        }
    }
    if let Err(e) = &ev.msix {
        whole(&mut out, Extractor::Msix, e);
    }
    match &ev.steam {
        Err(e) => whole(&mut out, Extractor::Steam, e),
        Ok(s) => out.extend(s.skipped.iter().map(|(p, i)| SourceError { extractor: Extractor::Steam, error: ExtractError::Format(*i), path: Some(p.clone()) })),
    }
    match &ev.epic {
        Err(e) => whole(&mut out, Extractor::Epic, e),
        Ok(x) => out.extend(x.skipped.iter().map(|(p, i)| SourceError { extractor: Extractor::Epic, error: ExtractError::Format(*i), path: Some(p.clone()) })),
    }
    if ev.track_progs == TrackProgs::On
        && let Err(e) = &ev.userassist
    {
        whole(&mut out, Extractor::UserAssist, e);
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Spazio: un proprietario per byte

/// Nodo che delimita lo spazio: cartella di un gruppo, dati di un gruppo fuori dalla cartella, tuoi file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Folder(usize),
    Data(usize),
    You,
}

/// Programmi con la stessa cartella (o un programma da solo).
#[derive(Default)]
struct Group {
    members: Vec<usize>,
    /// Nodo della cartella, se è nella scansione.
    folder: Option<u32>,
    /// Percorso della cartella in minuscolo (UserAssist).
    folder_path: Option<String>,
    /// Almeno un membro dichiara una cartella (anche fuori dalla scansione).
    declares_folder: bool,
    data: Vec<RuleData>,
    your_files: Vec<YourFiles>,
    area: Option<u32>,
    bytes: u64,
    launches: Option<Launches>,
}

/// Una sola passata sulle corrispondenze della classificazione e sui nodi delle cartelle: niente percorsi
/// ricostruiti per nodo.
struct Owners<'a> {
    tree: &'a Tree,
    marks: HashMap<u32, Mark>,
}

impl Owners<'_> {
    /// Primo antenato (escluso `n`) che delimita lo spazio.
    fn up(&self, n: u32) -> Option<(u32, Mark)> {
        let mut a = self.tree.parent(n);
        while a != NONE {
            if let Some(&m) = self.marks.get(&a) {
                return Some((a, m));
            }
            a = self.tree.parent(a);
        }
        None
    }
}

/// Gruppi per cartella, dati dalle regole, tuoi file e byte di ogni gruppo.
fn groups_and_space(tree: &Tree, ev: &Evidence, kb: &Kb, cls: &Classification, raws: &[Raw]) -> (Vec<Group>, Vec<usize>) {
    // Gruppi: stessa cartella nella scansione, oppure stesso percorso dichiarato fuori dalla scansione.
    let mut groups: Vec<Group> = Vec::new();
    let mut of: Vec<usize> = Vec::with_capacity(raws.len());
    let mut by_key: HashMap<String, usize> = HashMap::new();
    for (i, r) in raws.iter().enumerate() {
        let key = match &r.location {
            Location::Folder { node: Some(n), .. } => Some(format!("#{n}")),
            Location::Folder { path, node: None, .. } => Some(path.to_lowercase()),
            _ => None,
        };
        let g = match key.as_ref().and_then(|k| by_key.get(k)) {
            Some(&g) => g,
            None => {
                groups.push(Group::default());
                let g = groups.len() - 1;
                if let Some(k) = key {
                    by_key.insert(k, g);
                }
                g
            }
        };
        let grp = &mut groups[g];
        grp.members.push(i);
        if let Location::Folder { path, node, .. } = &r.location {
            grp.declares_folder = true;
            grp.folder_path = Some(path.to_lowercase());
            grp.folder = node.filter(|&n| !tree.flags(n).has(Flags::DETACHED));
        }
        of.push(g);
    }

    let anchors: Vec<(u32, usize)> = cls.anchors().filter(|&(n, m)| m < cls.matches.len() && !tree.flags(n).has(Flags::DETACHED)).collect();
    let mut own = Owners { tree, marks: HashMap::new() };
    for (g, grp) in groups.iter().enumerate() {
        if let Some(f) = grp.folder {
            own.marks.insert(f, Mark::Folder(g));
        }
    }
    for &(n, m) in &anchors {
        if cls.matches[m].manager == Manager::You {
            own.marks.entry(n).or_insert(Mark::You);
        }
    }

    // Dati fuori dalla cartella: regole che dichiarano come gestore un programma con quel nome esatto, e
    // la cartella dati di un pacchetto MSIX (`Local\Packages\<PFN>`, regola dei pacchetti MSIX). Solo
    // attribuzioni che danno il colore (da fonte o dedotto alta, 03 §6.2) e un solo gruppo candidato.
    let mut by_name: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, r) in raws.iter().enumerate() {
        let v = by_name.entry(r.name.trim()).or_default();
        if !v.contains(&of[i]) {
            v.push(of[i]);
        }
    }
    let mut candidates: Vec<(u32, usize, usize)> = Vec::new();
    for &(n, m) in &anchors {
        let mt = &cls.matches[m];
        if mt.manager != Manager::Programs || !mt.attribution.colors() {
            continue;
        }
        let Some(by) = kb.rules.get(mt.rule).and_then(|r| r.managed_by.as_deref()) else { continue };
        if let Some([g]) = by_name.get(by.trim()).map(Vec::as_slice) {
            candidates.push((n, m, *g));
        }
    }
    let packages = ev.folders.local.as_deref().and_then(|l| engine::find(tree, &extract::join(l, "Packages")));
    if let Some(pk) = packages {
        for (i, r) in raws.iter().enumerate() {
            let ProgramSource::Msix { family_name } = &r.source else { continue };
            let Some(n) = engine::child(tree, pk, family_name) else { continue };
            if let Some(m) = cls.own(n).filter(|&m| m < cls.matches.len() && cls.matches[m].manager == Manager::Programs && cls.matches[m].attribution.colors()) {
                candidates.push((n, m, of[i]));
            }
        }
    }
    // In ordine di nodo: i genitori hanno indici minori, quindi un elemento esterno viene prima dei suoi.
    candidates.sort_unstable();
    candidates.dedup_by_key(|c| c.0);
    let mut outside: Vec<(u32, usize, usize)> = Vec::new();
    for (n, m, g) in candidates {
        if own.marks.contains_key(&n) || own.up(n).is_some() {
            continue;
        }
        own.marks.insert(n, Mark::Data(g));
        outside.push((n, m, g));
    }

    // Byte di ogni nodo delimitato: allocata meno i nodi delimitati al suo interno (il più vicino).
    let mut inner: HashMap<u32, u64> = HashMap::new();
    let marked: Vec<(u32, Mark)> = own.marks.iter().map(|(&n, &m)| (n, m)).collect();
    for &(n, _) in &marked {
        if let Some((p, _)) = own.up(n) {
            *inner.entry(p).or_default() += tree.alloc(n);
        }
    }
    for &(n, mark) in &marked {
        let bytes = tree.alloc(n).saturating_sub(inner.get(&n).copied().unwrap_or(0));
        match mark {
            Mark::Folder(g) if !tree.flags(n).any(Flags::DENIED | Flags::ERROR) => groups[g].bytes += bytes,
            Mark::Data(g) => groups[g].bytes += bytes,
            _ => {}
        }
    }
    for &(n, m, g) in &outside {
        groups[g].data.push(RuleData { node: n, matched: m, rule: cls.matches[m].rule, bytes: cls.own_bytes(tree, n), inside: false });
    }
    // Dati dentro la cartella: elementi con gestore Programmi il cui primo delimitatore è la cartella.
    for &(n, m) in &anchors {
        if own.marks.contains_key(&n) || cls.matches[m].manager != Manager::Programs {
            continue;
        }
        if let Some((_, Mark::Folder(g))) = own.up(n) {
            groups[g].data.push(RuleData { node: n, matched: m, rule: cls.matches[m].rule, bytes: cls.own_bytes(tree, n), inside: true });
        }
    }
    for grp in &mut groups {
        grp.data.sort_by_key(|d| (!d.inside, d.node));
    }

    // Ambito: la cartella comune alla cartella del programma e ai suoi dati fuori, se non è una cartella
    // nota e non contiene altri programmi.
    let folders: Vec<(u32, usize)> = groups.iter().enumerate().filter_map(|(g, x)| x.folder.map(|f| (f, g))).collect();
    let mut areas: HashMap<u32, usize> = HashMap::new();
    for (g, grp) in groups.iter_mut().enumerate() {
        let mut nodes: Vec<u32> = grp.folder.into_iter().collect();
        nodes.extend(outside.iter().filter(|o| o.2 == g).map(|o| o.0));
        if nodes.len() < 2 {
            continue;
        }
        let lca = nodes[1..].iter().fold(nodes[0], |a, &b| common_ancestor(tree, a, b));
        if Some(lca) == grp.folder || !plausible_program_dir(ev, &tree.path_lossy(lca)) {
            continue;
        }
        if folders.iter().any(|&(f, h)| h != g && is_ancestor(tree, lca, f)) || areas.contains_key(&lca) {
            continue;
        }
        grp.area = Some(lca);
        areas.insert(lca, g);
    }

    // Tuoi file: elementi con gestore "tu" il cui primo delimitatore è una cartella o un dato del gruppo,
    // oppure dentro il suo ambito. Quelli dentro altri tuoi file sono già compresi.
    for &(n, m) in &anchors {
        if own.marks.get(&n) != Some(&Mark::You) {
            continue;
        }
        let mut a = tree.parent(n);
        let mut owner: Option<(usize, u32)> = None;
        while a != NONE {
            match own.marks.get(&a) {
                Some(Mark::Folder(g)) | Some(Mark::Data(g)) => {
                    owner = Some((*g, groups[*g].area.unwrap_or(a)));
                    break;
                }
                Some(Mark::You) => break,
                None => {}
            }
            if let Some(&g) = areas.get(&a) {
                owner = Some((g, a));
                break;
            }
            a = tree.parent(a);
        }
        if let Some((g, top)) = owner {
            groups[g].your_files.push(YourFiles { node: n, matched: m, rule: cls.matches[m].rule, bytes: tree.alloc(n), relative: relative(tree, top, n) });
        }
    }
    (groups, of)
}

/// Abbina le voci UserAssist alle cartelle dei programmi: la cartella più lunga che contiene l'eseguibile.
fn match_userassist(entries: &[UserAssistEntry], ev: &Evidence, groups: &mut [Group]) {
    let folders: Vec<(String, usize)> = groups.iter().enumerate().filter_map(|(g, x)| x.folder_path.as_ref().map(|p| (format!("{p}\\"), g))).collect();
    for e in entries {
        let Some(last) = e.last_run else { continue };
        let Some(p) = userassist_path(&e.name, ev) else { continue };
        let Some(&(_, g)) = folders.iter().filter(|(f, _)| p.starts_with(f.as_str())).max_by_key(|(f, _)| f.len()) else { continue };
        let l = groups[g].launches.get_or_insert(Launches { last, runs: 0 });
        l.last = l.last.max(last);
        l.runs += u64::from(e.runs);
    }
}

// ---------------------------------------------------------------------------------------------
// Record

fn steps(via: &UninstallVia) -> (Step, Vec<Step>) {
    match via {
        UninstallVia::Settings { .. } => (
            Step { key: "funzione-programma-metodo-impostazioni".into(), command: None, uri: Some("ms-settings:appsfeatures".into()) },
            vec![Step { key: "funzione-programma-alternativa-pannello".into(), command: Some("appwiz.cpl".into()), uri: None }],
        ),
        UninstallVia::SettingsApp { family_name } => {
            (Step { key: "funzione-programma-metodo-msix".into(), command: None, uri: Some(format!("ms-settings:appsfeatures-app?{family_name}")) }, Vec::new())
        }
        UninstallVia::Steam { appid } => {
            (Step { key: "funzione-programma-metodo-steam".into(), command: None, uri: Some(format!("steam://uninstall/{appid}")) }, Vec::new())
        }
        UninstallVia::EpicLibrary => (Step { key: "funzione-programma-metodo-epic".into(), command: None, uri: None }, Vec::new()),
    }
}

struct RecordIn<'a> {
    identity: Vec<Fact>,
    criterion: Criterion,
    via: Option<&'a UninstallVia>,
    hypotheses: Vec<String>,
    your_files: bool,
}

/// Record di un programma: Decidi tu; disinstallazione dall'interfaccia ufficiale se c'è un metodo. La
/// stima non è stimabile: cosa toglie lo decide il programma di disinstallazione (può lasciare dati o
/// togliere cartelle che la vista non attribuisce); il misurato resta un fatto, non una stima.
fn program_record(x: RecordIn<'_>) -> Record {
    let action = if x.via.is_some() { Action::Uninstall } else { Action::None };
    let (preferred, alternatives) = match x.via {
        Some(v) => {
            let (p, a) = steps(v);
            (Some(p), a)
        }
        None => (None, Vec::new()),
    };
    let uninstall = action == Action::Uninstall;
    let checks = if uninstall && x.your_files {
        vec![Check { key: "funzione-programma-controllo-tuoi-file".into(), kind: CheckKind::Manual, outcome: CheckOutcome::ToTick }]
    } else {
        Vec::new()
    };
    Record {
        state: State::YouDecide,
        action,
        identity: ProvaDiIdentita(x.identity),
        hypotheses: x.hypotheses,
        reliability: x.criterion,
        risks: if uninstall { vec!["funzione-programma-rischio-1".into()] } else { Vec::new() },
        preferred,
        alternatives,
        checks,
        estimate: Estimate::NotEstimable(if uninstall { NotEstimable::ExternalProcedure } else { NotEstimable::NoAction }),
        reversibility: if uninstall { "reversibilita-disinstallazione" } else { "reversibilita-nessuna-azione" }.into(),
        privileges: Privileges::None,
        on_error: if uninstall { "errore-procedura-esterna" } else { "errore-nessuna-azione" }.into(),
        after: if uninstall { "verifica-dopo-procedura" } else { "verifica-nessuna-azione" }.into(),
        origin: Origin::Function(Function::Program),
        expiry: Expiry::Fresh,
        protected_exception: false,
    }
}

fn residue_record(last: Option<i64>) -> Record {
    let f = match last {
        Some(t) => fact("fatto-residuo-possibile", Provenance::Measured, Source::Scan, vec![Value::Time(t)]),
        None => fact("fatto-residuo-possibile-senza-data", Provenance::Unavailable, Source::Scan, Vec::new()),
    };
    Record {
        state: State::Unknown(UnknownReason::NameOnly),
        action: Action::None,
        identity: ProvaDiIdentita(vec![f]),
        hypotheses: Vec::new(),
        reliability: Criterion::NameOnly,
        risks: Vec::new(),
        preferred: None,
        alternatives: Vec::new(),
        checks: Vec::new(),
        estimate: Estimate::NotEstimable(NotEstimable::NoAction),
        reversibility: "reversibilita-nessuna-azione".into(),
        privileges: Privileges::None,
        on_error: "errore-nessuna-azione".into(),
        after: "verifica-nessuna-azione".into(),
        origin: Origin::Function(Function::PossibleResidue),
        expiry: Expiry::Fresh,
        protected_exception: false,
    }
}

// ---------------------------------------------------------------------------------------------
// Possibili residui

/// Ultima modifica più recente nel sottoalbero (visita iterativa, solo di questo sottoalbero).
fn last_modified(tree: &Tree, node: u32) -> Option<i64> {
    let mut best = 0i64;
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if tree.flags(n).has(Flags::DETACHED) {
            continue;
        }
        best = best.max(tree.mtime(n));
        stack.extend(tree.children(n));
    }
    (best > 0).then_some(best)
}

fn residues(tree: &Tree, ev: &Evidence, cls: &Classification, raws: &[Raw], groups: &[Group]) -> Vec<Residue> {
    let f = &ev.folders;
    let mut bases: Vec<(u32, ResidueBase)> = Vec::new();
    let candidates = [(&f.local, ResidueBase::Local), (&f.roaming, ResidueBase::Roaming), (&f.local_low, ResidueBase::LocalLow), (&f.program_data, ResidueBase::ProgramData)];
    for (path, b) in candidates {
        if let Some(n) = path.as_deref().and_then(|p| engine::find(tree, p))
            && !bases.iter().any(|&(m, _)| m == n)
        {
            bases.push((n, b));
        }
    }
    if bases.is_empty() {
        return Vec::new();
    }
    let base_nodes: HashSet<u32> = bases.iter().map(|&(n, _)| n).collect();
    // Cartelle di primo livello rivendicate: con una corrispondenza della classificazione al loro interno
    // (o su di loro), con la cartella o i dati di un programma.
    let top = |n: u32| -> Option<u32> {
        let mut a = n;
        loop {
            let p = tree.parent(a);
            if p == NONE {
                return None;
            }
            if base_nodes.contains(&p) {
                return Some(a);
            }
            a = p;
        }
    };
    let mut claimed: HashSet<u32> = HashSet::new();
    for (n, _) in cls.anchors() {
        claimed.extend(top(n));
    }
    for g in groups {
        for n in g.folder.into_iter().chain(g.area).chain(g.data.iter().map(|d| d.node)) {
            claimed.extend(top(n));
        }
    }
    // Anche una cartella dichiarata che la scansione non ha trovato rivendica la sua cartella di primo
    // livello, se c'è (confronto sul percorso, una volta per programma).
    let base_paths: Vec<(u32, String)> = bases.iter().map(|&(n, _)| (n, format!("{}\\", tree.path_lossy(n).to_lowercase()))).collect();
    for r in raws {
        let Location::Folder { path, node: None, .. } = &r.location else { continue };
        let p = path.to_lowercase();
        for (b, bp) in &base_paths {
            if let Some(seg) = p.strip_prefix(bp.as_str()).and_then(|rest| rest.split('\\').next()).filter(|s| !s.is_empty()) {
                claimed.extend(engine::child(tree, *b, seg));
            }
        }
    }
    // Cartelle note e loro antenati non sono mai residui (una base dentro un'altra).
    let mut known: HashSet<u32> = HashSet::new();
    for k in f.all() {
        let mut n = engine::find(tree, k).unwrap_or(NONE);
        while n != NONE && known.insert(n) {
            n = tree.parent(n);
        }
    }
    // Nomi dei programmi installati, confrontati per parole: nome, editore, nomi di famiglia MSIX e loro
    // parte nome, cartelle dei giochi di Steam, `AppName` di Epic.
    let names = NameIndex::new(raws.iter().flat_map(|r| std::iter::once(r.name.as_str()).chain(r.publisher.as_deref()).chain(r.aliases.iter().map(String::as_str))));
    let mut out = Vec::new();
    for &(b, base) in &bases {
        let mut kids: Vec<u32> = tree.children(b).collect();
        kids.sort_unstable();
        for c in kids {
            let fl = tree.flags(c);
            if !fl.has(Flags::DIR) || fl.any(Flags::DETACHED | Flags::NOT_FOLLOWED) || base_nodes.contains(&c) || known.contains(&c) || claimed.contains(&c) {
                continue;
            }
            if names.claims(&tree.name_lossy(c)) {
                continue;
            }
            let last = last_modified(tree, c);
            out.push(Residue { node: c, base, last_modified: last, record: residue_record(last) });
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Vista

/// Costruisce la vista Programmi dall'albero, dalle prove degli estrattori e dalla classificazione
/// (che deve venire dalla stessa base `kb` e dallo stesso albero).
pub fn build(tree: &Tree, ev: &Evidence, kb: &Kb, cls: &Classification) -> Programs {
    let mut raws: Vec<Raw> = Vec::new();
    collect_uninstall(tree, ev, cls.today, &mut raws);
    collect_msix(tree, ev, &mut raws);
    collect_steam(tree, ev, &mut raws);
    merge_steam_entries(&mut raws);
    collect_epic(tree, ev, &mut raws);

    let (mut groups, of) = groups_and_space(tree, ev, kb, cls, &raws);

    // UserAssist solo con il tracciamento attivo: con un altro valore nessun dato, anche se la chiave ne ha.
    let ua: Result<&[UserAssistEntry], Usage> = match (ev.track_progs, &ev.userassist) {
        (TrackProgs::On, Ok(UserAssist::Entries(v))) => Ok(v.as_slice()),
        (TrackProgs::On, Ok(UserAssist::NotTracked(t))) => Err(Usage::NotTracked(*t)),
        (TrackProgs::On, Err(e)) => Err(Usage::Unreadable(*e)),
        (t, _) => Err(Usage::NotTracked(t)),
    };
    if let Ok(entries) = ua {
        match_userassist(entries, ev, &mut groups);
    }

    let mut items: Vec<Program> = Vec::with_capacity(raws.len());
    for (i, r) in raws.iter().enumerate() {
        let g = &groups[of[i]];
        let src = r.fact_source.clone();
        let mut identity = r.identity.clone();
        let mut hypotheses: Vec<String> = Vec::new();
        // Posizione.
        match &r.location {
            Location::NotDeclared => identity.push(fact("fatto-programma-posizione-non-dichiarata", Provenance::Unavailable, src.clone(), Vec::new())),
            Location::Implausible { path, .. } => identity.push(fact("fatto-programma-cartella-non-plausibile", Provenance::FromSource, src.clone(), vec![text(path)])),
            Location::Folder { path, evidence, provenance, .. } => {
                let key = if *evidence == PathEvidence::UninstallHint { "fatto-programma-cartella-indizio" } else { "fatto-programma-cartella" };
                identity.push(fact(key, *provenance, src.clone(), vec![text(path)]));
                if *evidence == PathEvidence::UninstallHint {
                    hypotheses.push("funzione-programma-ipotesi-indizio".into());
                }
            }
        }
        // Spazio misurato.
        let has_data = g.data.iter().any(|d| !d.inside);
        let folder_readable = g.folder.is_some_and(|n| !tree.flags(n).any(Flags::DENIED | Flags::ERROR));
        let space = if folder_readable || has_data {
            Space::Measured(g.bytes)
        } else if g.declares_folder {
            Space::NotMeasured
        } else {
            Space::NotAttributed
        };
        match space {
            Space::Measured(b) => {
                identity.push(fact("fatto-programma-su-disco", Provenance::Measured, Source::Scan, vec![Value::Bytes(b)]));
                // Cartella dichiarata ma non letta (WindowsApps dei pacchetti MSIX): contano solo i dati.
                if g.declares_folder && !folder_readable {
                    identity.push(fact("fatto-programma-solo-dati", Provenance::Unavailable, Source::Scan, Vec::new()));
                }
            }
            Space::NotMeasured => identity.push(fact("fatto-programma-non-misurato", Provenance::Unavailable, Source::Scan, Vec::new())),
            Space::NotAttributed => identity.push(fact("fatto-programma-nessuno-spazio", Provenance::Unavailable, Source::Scan, Vec::new())),
        }
        let shared_with: Vec<usize> = g.members.iter().copied().filter(|&j| j != i).collect();
        if !shared_with.is_empty()
            && let Location::Folder { provenance, .. } = &r.location
        {
            identity.push(fact("fatto-programma-cartella-condivisa", *provenance, src.clone(), vec![Value::Count(shared_with.len() as u64)]));
        }
        if !g.data.is_empty() {
            let total: u64 = g.data.iter().map(|d| d.bytes).sum();
            let c = g.data.iter().map(|d| cls.matches[d.matched].record.reliability).min_by_key(|c| c.level()).unwrap_or(Criterion::StructureOnly);
            identity.push(fact("fatto-programma-dati-regole", Provenance::Deduced(c), Source::Rule, vec![Value::Bytes(total), Value::Count(g.data.len() as u64)]));
        }
        for y in &g.your_files {
            let vals = vec![text(&r.name), Value::Bytes(y.bytes), text(&y.relative)];
            identity.push(fact("fatto-programma-tuoi-file", cls.matches[y.matched].attribution, Source::Rule, vals));
        }
        if g.area.is_some() && g.your_files.iter().any(|y| g.folder.is_none_or(|f| !is_ancestor(tree, f, y.node))) {
            hypotheses.push("funzione-programma-ipotesi-ambito".into());
        }
        // Dichiarato (F-AP-3).
        if let Some(d) = &r.declared {
            identity.push(fact("fatto-dimensione-dichiarata", Provenance::FromSource, d.source.clone(), vec![Value::Bytes(d.bytes)]));
        }
        // Uso noto (F-AP-4).
        let usage = match r.kind {
            Kind::Steam => {
                let (manifest, local_config) = r.steam.unwrap_or((None, None));
                match (manifest, local_config) {
                    (Some(t), _) => identity.push(fact("fatto-ultima-sessione-steam", Provenance::FromSource, Source::SteamManifest, vec![Value::Time(t)])),
                    (None, Some(t)) => identity.push(fact("fatto-ultima-sessione-steam", Provenance::FromSource, Source::SteamLocalConfig, vec![Value::Time(t)])),
                    (None, None) => identity.push(fact("fatto-uso-non-registrato", Provenance::Unavailable, Source::SteamManifest, Vec::new())),
                }
                Usage::Steam { manifest, local_config }
            }
            Kind::Epic => {
                identity.push(fact("fatto-epic-nessun-dato-uso", Provenance::Unavailable, Source::EpicManifest, Vec::new()));
                Usage::NotRecorded
            }
            Kind::Msix => {
                identity.push(fact("fatto-uso-msix-nessun-dato", Provenance::Unavailable, Source::Msix, Vec::new()));
                Usage::NotRecorded
            }
            Kind::Win32 => {
                let u = match &ua {
                    Err(u) => *u,
                    Ok(_) if g.folder_path.is_none() => Usage::NoFolder,
                    Ok(_) => Usage::UserAssist(g.launches),
                };
                let none = |key: &str| fact(key, Provenance::Unavailable, Source::UserAssist, Vec::new());
                identity.push(match u {
                    Usage::UserAssist(Some(l)) => fact("fatto-uso-userassist", Provenance::FromSource, Source::UserAssist, vec![Value::Time(l.last)]),
                    Usage::UserAssist(None) => none("fatto-uso-userassist-nessun-avvio"),
                    Usage::NoFolder => none("fatto-uso-senza-cartella"),
                    Usage::NotTracked(TrackProgs::Off) => none("fatto-uso-tracciamento-disattivato"),
                    Usage::NotTracked(_) => none("fatto-uso-tracciamento-non-letto"),
                    _ => none("fatto-uso-userassist-non-letto"),
                });
                u
            }
        };
        if let Some(d) = r.install_date {
            identity.push(fact("fatto-programma-data-installazione", Provenance::FromSource, src.clone(), vec![Value::Time(date_filetime(d))]));
        }
        let record = program_record(RecordIn { identity, criterion: r.criterion, via: r.uninstall.as_ref(), hypotheses, your_files: !g.your_files.is_empty() });
        items.push(Program {
            name: r.name.clone(),
            version: r.version.clone(),
            publisher: r.publisher.clone(),
            source: r.source.clone(),
            hidden: r.hidden,
            location: r.location.clone(),
            area: g.area,
            space,
            shared_with,
            data: g.data.clone(),
            your_files: g.your_files.clone(),
            declared: r.declared.clone(),
            usage,
            install_date: r.install_date,
            uninstall: r.uninstall.clone(),
            record,
        });
    }

    Programs { residues: residues(tree, ev, cls, &raws, &groups), items, errors: collect_errors(ev), tracking: ev.track_progs }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_dates_are_validated() {
        let today = Some(Date::new(2026, 10, 13));
        assert_eq!(parse_install_date("20261008", today), Some(Date::new(2026, 10, 8)));
        assert_eq!(parse_install_date(" 20240229 ", today), Some(Date::new(2024, 2, 29)));
        for bad in ["", "2026108", "202610081", "20261399", "20250229", "2026-10-08", "abcdefgh", "19791231", "20261014", "２０２６１００８"] {
            assert_eq!(parse_install_date(bad, today), None, "{bad}");
        }
        assert_eq!(parse_install_date("20991231", None), Some(Date::new(2099, 12, 31)));
    }

    #[test]
    fn calendar_dates_become_noon_utc() {
        // 13/10/2026 12:00 UTC = 1791892800 secondi Unix.
        assert_eq!(date_filetime(Date::new(2026, 10, 13)), (1_791_892_800 + 11_644_473_600) * 10_000_000);
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(unix_to_filetime(i64::MAX), None);
    }

    #[test]
    fn names_compare_on_letters_and_digits() {
        assert_eq!(name_key("Copper Kite"), name_key("CopperKite"));
        assert_eq!(name_key("Bytedance Pte. Ltd."), "bytedancepteltd");
        assert!(pfn_for_uri("Microsoft.WindowsTerminal_8wekyb3d8bbwe"));
        assert!(!pfn_for_uri("a b_8wekyb3d8bbwe"));
    }

    #[test]
    fn words_split_on_separators_digits_and_camel_case() {
        let w = |s: &str| name_words(s);
        assert_eq!(w("Copper Kite"), w("CopperKite"));
        assert_eq!(w("CopperKite"), ["copper", "kite"]);
        assert_eq!(w("7-Zip 24.09 (x64 edition)"), ["7", "zip", "24", "09", "x", "64", "edition"]);
        assert_eq!(w("7zip"), ["7", "zip"]);
        assert_eq!(w("Win11"), ["win", "11"]);
        assert_eq!(w("HTMLParser"), ["html", "parser"]);
        assert_eq!(w("NVIDIA"), ["nvidia"]);
        assert_eq!(w("WinRAR"), ["win", "rar"]);
        assert_eq!(w("Microsoft.WindowsTerminal_8wekyb3d8bbwe")[..3], ["microsoft", "windows", "terminal"]);
        assert_eq!(w("Bytedance Pte. Ltd."), ["bytedance", "pte", "ltd"]);
        assert_eq!(w("Città Ünïcode"), ["città", "ünïcode"]);
        assert!(w(" .-_ ").is_empty());
        // L'unione delle parole è il nome senza separatori.
        for s in ["Copper Kite", "7-Zip 24.09 (x64 edition)", "Città Ünïcode", "HTMLParser"] {
            assert_eq!(w(s).concat(), name_key(s), "{s}");
        }
    }

    #[test]
    fn a_folder_is_claimed_when_all_its_words_belong_to_one_candidate() {
        let ix = NameIndex::new(
            ["Epic Games Launcher", "Mozilla Thunderbird (x64 it)", "Microsoft Visual Studio Code", "Python 3.11.4 (64-bit)", "Node.js 20", "Copper Kite", "Epic Pen", "Mozilla Firefox"]
                .into_iter(),
        );
        // I tre esempi del PC vero: nomi un po' diversi dello stesso programma.
        assert!(ix.claims("Epic Games"));
        assert!(ix.claims("Thunderbird"));
        assert!(ix.claims("Code"));
        // Ordine e scrittura non contano; unioni di parole consecutive e nome intero.
        assert!(ix.claims("CopperKite") && ix.claims("Copperkite") && ix.claims("kite copper"));
        assert!(ix.claims("Python311") && ix.claims("nodejs"));
        // Non corrispondono: una parola che nessun candidato ha, parole di candidati diversi, un nome vuoto.
        assert!(!ix.claims("VecchioEditor"));
        assert!(!ix.claims("Epic Games Store Data"));
        assert!(!ix.claims("Mozilla Pen"), "«mozilla» e «pen» vengono da due programmi diversi");
        assert!(!ix.claims("Python 2"));
        assert!(!ix.claims("{}"));
        let none = NameIndex::new(std::iter::empty());
        assert!(!none.claims("Code"));
    }

    #[test]
    fn userassist_paths_resolve_known_folders() {
        let mut ev = Evidence::empty("C:\\");
        ev.folders = Evidence::default_folders("C:\\", "demo");
        assert_eq!(userassist_path(r"{6D809377-6AF0-444B-8957-A3773F02200E}\Esempio\a.exe", &ev).as_deref(), Some(r"c:\program files\esempio\a.exe"));
        assert_eq!(userassist_path(r"C:\Tools\b.exe", &ev).as_deref(), Some(r"c:\tools\b.exe"));
        assert_eq!(userassist_path("Microsoft.WindowsFeedbackHub_8wekyb3d8bbwe!App", &ev), None);
        assert_eq!(userassist_path(r"{00000000-0000-0000-0000-000000000000}\x.exe", &ev), None);
        assert_eq!(userassist_path(r"C:\a\..\b.exe", &ev), None);
    }
}
