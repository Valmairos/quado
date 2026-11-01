// SPDX-License-Identifier: GPL-3.0-or-later
//! Estrattori in sola lettura (05 D6): voci di disinstallazione, pacchetti MSIX, Steam, Epic, CapCut,
//! UserAssist e configurazione (F-SY-3). Leggono da sorgenti astratte (registro, file), così i test
//! usano fixture nel formato vero. Un formato inatteso produce "estrattore non riuscito" con il
//! motivo, mai un dato ricostruito (06 §2.3).

use crate::reg::{RegValue, RegistrySource, Root};
use crate::vdf::{self, Vdf};
use quado_model::record::Hive;

/// Campo coinvolto in un problema di formato.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    DisplayName,
    DisplayVersion,
    InstallLocation,
    UninstallString,
    DisplayIcon,
    EstimatedSize,
    LibraryPath,
    AppId,
    AppName,
    InstallDir,
    SizeOnDisk,
    LastPlayed,
    Version,
    Root,
    Data,
    PackageFamilyName,
    InstalledPath,
}

impl Field {
    pub fn code(self) -> &'static str {
        match self {
            Self::DisplayName => "DisplayName",
            Self::DisplayVersion => "DisplayVersion",
            Self::InstallLocation => "InstallLocation",
            Self::UninstallString => "UninstallString",
            Self::DisplayIcon => "DisplayIcon",
            Self::EstimatedSize => "EstimatedSize",
            Self::LibraryPath => "path",
            Self::AppId => "appid",
            Self::AppName => "AppName",
            Self::InstallDir => "installdir",
            Self::SizeOnDisk => "SizeOnDisk",
            Self::LastPlayed => "LastPlayed",
            Self::Version => "full_appver",
            Self::Root => "root",
            Self::Data => "data",
            Self::PackageFamilyName => "PackageFamilyName",
            Self::InstalledPath => "InstalledPath",
        }
    }
}

/// Problema di formato: codice, mai testo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormatIssue {
    /// Il testo finisce a metà.
    Truncated,
    /// Elemento inatteso (graffa in più, radice diversa).
    Unexpected,
    Empty,
    TooDeep,
    Encoding,
    /// Oltre il limite di dimensione letto dagli estrattori.
    TooLarge,
    MissingField(Field),
    /// Il campo c'è ma ha un tipo diverso da quello atteso (per esempio un numero al posto di un testo).
    WrongType(Field),
    /// Il campo c'è ma il valore non ha la forma attesa (versione, percorso, numero).
    BadValue(Field),
}

/// Esito non riuscito di un estrattore.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExtractError {
    /// Programma o file assente: nessun dato, non un errore di lettura.
    NotPresent,
    /// Errore di Windows nella lettura (5 = accesso negato).
    Io(u32),
    Format(FormatIssue),
    /// Segnaposto cloud: il contenuto non si legge, per non scaricarlo (05 D6).
    CloudPlaceholder,
}

impl From<FormatIssue> for ExtractError {
    fn from(f: FormatIssue) -> Self {
        Self::Format(f)
    }
}

/// Limite di dimensione dei file letti dagli estrattori: manifest e configurazioni, mai documenti.
pub const MAX_FILE: u64 = 32 << 20;

/// Lettura di file in sola lettura, astratta per i test.
pub trait FileSource: Sync {
    /// Testo del file, entro `MAX_FILE` byte. `NotPresent` se non esiste.
    fn read(&self, path: &str) -> Result<String, ExtractError>;
    /// Nomi dei file di una cartella.
    fn list(&self, dir: &str) -> Result<Vec<String>, ExtractError>;
}

pub fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('\\') { format!("{dir}{name}") } else { format!("{dir}\\{name}") }
}

/// Percorso assoluto di Windows con lettera di unità (`C:\...`).
pub fn is_absolute(p: &str) -> bool {
    let b = p.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// Normalizza un percorso: barre rovesciate, niente barra finale (salvo la radice), niente `\\?\`.
pub fn normalize(p: &str) -> String {
    let p = p.trim().trim_matches('"');
    let p = p.strip_prefix(r"\\?\").unwrap_or(p);
    let mut s: String = p.replace('/', "\\");
    while s.len() > 3 && s.ends_with('\\') {
        s.pop();
    }
    s
}

// ---------------------------------------------------------------------------------------------
// Voci di disinstallazione

pub const UNINSTALL_KEYS: [(Root, &str, Hive); 3] = [
    (Root::LocalMachine, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall", Hive::Machine),
    (Root::LocalMachine, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall", Hive::Machine32),
    (Root::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Uninstall", Hive::User),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UninstallEntry {
    pub hive: Hive,
    pub key: String,
    pub display_name: String,
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub install_location: Option<String>,
    pub uninstall_string: Option<String>,
    pub display_icon: Option<String>,
    pub install_date: Option<String>,
    /// `EstimatedSize`, in KB, dichiarato (mai sommato al misurato, F-AP-3).
    pub estimated_kb: Option<u32>,
    pub system_component: bool,
}

impl UninstallEntry {
    /// Cartella indicata da `UninstallString` o `DisplayIcon`: solo un indizio di percorso, mai
    /// eseguito (F-AP-2). `None` per `MsiExec` e per i percorsi relativi.
    pub fn path_hint(&self) -> Option<String> {
        self.hint_exe().and_then(|p| p.rsplit_once('\\').map(|(d, _)| d.to_owned()))
    }

    /// Eseguibile indicato da `DisplayIcon` o `UninstallString` (percorso completo).
    pub fn hint_exe(&self) -> Option<String> {
        [self.display_icon.as_deref(), self.uninstall_string.as_deref()].into_iter().flatten().find_map(exe_path)
    }

    pub fn location(&self) -> Option<String> {
        self.install_location.as_deref().map(normalize).filter(|p| is_absolute(p))
    }
}

/// Eseguibile in una riga di comando o in un `DisplayIcon` (`"C:\x\a.exe",0`).
fn exe_path(s: &str) -> Option<String> {
    let s = s.trim();
    let path = if let Some(rest) = s.strip_prefix('"') {
        rest.split('"').next()?.to_owned()
    } else {
        let lower = s.to_ascii_lowercase();
        let end = lower.find(".exe").map(|k| k + 4).unwrap_or_else(|| s.find(',').unwrap_or(s.len()));
        s[..end].to_owned()
    };
    let path = normalize(&path);
    if !is_absolute(&path) || path.to_ascii_lowercase().contains("msiexec") {
        return None;
    }
    let (dir, file) = path.rsplit_once('\\')?;
    if file.is_empty() || dir.len() < 3 {
        return None;
    }
    Some(path)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Uninstall {
    pub entries: Vec<UninstallEntry>,
    /// Voci non lette perché un campo ha un formato inatteso.
    pub skipped: Vec<(Hive, String, FormatIssue)>,
}

fn opt_str(values: &[(String, RegValue)], name: &str, field: Field) -> Result<Option<String>, FormatIssue> {
    match values.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
        None => Ok(None),
        Some((_, v)) => v.as_str().map(|s| Some(s.to_owned())).ok_or(FormatIssue::WrongType(field)),
    }
}

fn read_entry(hive: Hive, key: &str, values: &[(String, RegValue)]) -> Result<Option<UninstallEntry>, FormatIssue> {
    let Some(display_name) = opt_str(values, "DisplayName", Field::DisplayName)? else { return Ok(None) };
    if display_name.trim().is_empty() {
        return Ok(None);
    }
    let dword = |name: &str| values.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone());
    let estimated_kb = match dword("EstimatedSize") {
        None => None,
        Some(RegValue::Dword(v)) => Some(v),
        Some(_) => return Err(FormatIssue::WrongType(Field::EstimatedSize)),
    };
    Ok(Some(UninstallEntry {
        hive,
        key: key.to_owned(),
        display_name,
        version: opt_str(values, "DisplayVersion", Field::DisplayVersion)?,
        publisher: opt_str(values, "Publisher", Field::DisplayName)?,
        install_location: opt_str(values, "InstallLocation", Field::InstallLocation)?,
        uninstall_string: opt_str(values, "UninstallString", Field::UninstallString)?,
        display_icon: opt_str(values, "DisplayIcon", Field::DisplayIcon)?,
        install_date: opt_str(values, "InstallDate", Field::DisplayName).ok().flatten(),
        estimated_kb,
        system_component: matches!(dword("SystemComponent"), Some(RegValue::Dword(1))),
    }))
}

/// Voci con `DisplayName` delle tre chiavi di disinstallazione (F-AP-1).
pub fn uninstall(reg: &dyn RegistrySource) -> Result<Uninstall, ExtractError> {
    let mut out = Uninstall::default();
    let mut read_any = false;
    let mut last_err = 2;
    for (root, path, hive) in UNINSTALL_KEYS {
        let keys = match reg.subkeys(root, path) {
            Ok(k) => k,
            Err(e) => {
                last_err = e;
                continue;
            }
        };
        read_any = true;
        for k in keys {
            let Ok(values) = reg.values(root, &format!(r"{path}\{k}")) else { continue };
            match read_entry(hive, &k, &values) {
                Ok(Some(e)) => out.entries.push(e),
                Ok(None) => {}
                Err(issue) => out.skipped.push((hive, k, issue)),
            }
        }
    }
    if !read_any {
        return Err(if last_err == 2 { ExtractError::NotPresent } else { ExtractError::Io(last_err) });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Pacchetti MSIX

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsixPackage {
    pub family_name: String,
    pub installed_path: Option<String>,
    pub framework: bool,
}

/// Forma di un nome di famiglia: `Nome_editore` con l'identificativo dell'editore di 13 caratteri.
pub fn valid_family_name(pfn: &str) -> bool {
    match pfn.rsplit_once('_') {
        Some((name, id)) => !name.is_empty() && id.len() == 13 && id.bytes().all(|b| b.is_ascii_alphanumeric()),
        None => false,
    }
}

/// Elenco MSIX salvato come fixture: una riga per pacchetto, `nome_famiglia|percorso|framework`.
pub fn parse_msix_list(text: &str) -> Result<Vec<MsixPackage>, FormatIssue> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let mut parts = line.split('|');
        let (Some(pfn), Some(path), Some(fw), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
            return Err(FormatIssue::Truncated);
        };
        out.push(MsixPackage { family_name: pfn.to_owned(), installed_path: (!path.is_empty()).then(|| path.to_owned()), framework: fw == "1" });
    }
    validate_msix(&out)?;
    Ok(out)
}

pub fn validate_msix(pkgs: &[MsixPackage]) -> Result<(), FormatIssue> {
    for p in pkgs {
        if !valid_family_name(&p.family_name) {
            return Err(FormatIssue::BadValue(Field::PackageFamilyName));
        }
        if p.installed_path.as_deref().is_some_and(|x| !is_absolute(x)) {
            return Err(FormatIssue::BadValue(Field::InstalledPath));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Steam

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SteamApp {
    pub appid: u32,
    pub name: String,
    pub installdir: String,
    /// Cartella della libreria (`...\Steam`, che contiene `steamapps`).
    pub library: String,
    /// `SizeOnDisk`: dimensione logica dichiarata, può essere vecchia (F-AP-3).
    pub size_on_disk: Option<u64>,
    /// `LastPlayed` del manifest, secondi Unix.
    pub last_played: Option<i64>,
}

impl SteamApp {
    pub fn folder(&self) -> String {
        join(&join(&join(&self.library, "steamapps"), "common"), &self.installdir)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Steam {
    pub root: String,
    pub libraries: Vec<String>,
    pub apps: Vec<SteamApp>,
    /// `LastPlayed` da `localconfig.vdf` (massimo tra gli utenti), per confronto con i manifest.
    pub local_last_played: Vec<(u32, i64)>,
    /// File non letti, con il motivo.
    pub skipped: Vec<(String, FormatIssue)>,
}

pub fn parse_libraryfolders(text: &str) -> Result<Vec<String>, FormatIssue> {
    let v = vdf::parse(text)?;
    let lf = v.get("libraryfolders").ok_or(FormatIssue::MissingField(Field::Root))?;
    let mut out = Vec::new();
    for (_, lib) in lf.entries() {
        // Formato vecchio: "1" "D:\\SteamLibrary"; nuovo: oggetto con "path".
        let path = match lib {
            Vdf::Str(s) => s.as_str(),
            Vdf::Obj(_) => lib.str("path").ok_or(FormatIssue::MissingField(Field::LibraryPath))?,
        };
        if !is_absolute(path) {
            // "contentstatsid" e simili nel formato vecchio sono valori, non librerie.
            if matches!(lib, Vdf::Obj(_)) {
                return Err(FormatIssue::BadValue(Field::LibraryPath));
            }
            continue;
        }
        out.push(normalize(path));
    }
    if out.is_empty() {
        return Err(FormatIssue::Empty);
    }
    Ok(out)
}

pub fn parse_appmanifest(text: &str, library: &str) -> Result<SteamApp, FormatIssue> {
    let v = vdf::parse(text)?;
    let s = v.get("AppState").ok_or(FormatIssue::MissingField(Field::Root))?;
    let appid = s.str("appid").ok_or(FormatIssue::MissingField(Field::AppId))?.parse().map_err(|_| FormatIssue::BadValue(Field::AppId))?;
    let name = s.str("name").ok_or(FormatIssue::MissingField(Field::AppName))?.to_owned();
    let installdir = s.str("installdir").ok_or(FormatIssue::MissingField(Field::InstallDir))?.to_owned();
    if installdir.is_empty() || installdir.contains(['\\', '/']) || installdir == ".." {
        return Err(FormatIssue::BadValue(Field::InstallDir));
    }
    let num = |k: &str, f: Field| -> Result<Option<i64>, FormatIssue> {
        match s.str(k) {
            None => Ok(None),
            Some(x) => x.parse::<i64>().map(Some).map_err(|_| FormatIssue::BadValue(f)),
        }
    };
    let size_on_disk = num("SizeOnDisk", Field::SizeOnDisk)?.map(|x| x.max(0) as u64);
    let last_played = num("LastPlayed", Field::LastPlayed)?.filter(|&x| x > 0);
    Ok(SteamApp { appid, name, installdir, library: normalize(library), size_on_disk, last_played })
}

pub fn parse_localconfig(text: &str) -> Result<Vec<(u32, i64)>, FormatIssue> {
    let v = vdf::parse(text)?;
    let store = v.get("UserLocalConfigStore").ok_or(FormatIssue::MissingField(Field::Root))?;
    let apps = store.path(&["Software", "Valve", "Steam", "apps"]).ok_or(FormatIssue::MissingField(Field::AppId))?;
    let mut out = Vec::new();
    for (id, app) in apps.entries() {
        let Ok(appid) = id.parse::<u32>() else { continue };
        if let Some(lp) = app.str("LastPlayed") {
            let t: i64 = lp.parse().map_err(|_| FormatIssue::BadValue(Field::LastPlayed))?;
            if t > 0 {
                out.push((appid, t));
            }
        }
    }
    Ok(out)
}

/// Steam: radice dal registro (`SteamPath`), librerie, manifest dei giochi, `localconfig.vdf` degli
/// utenti. Solo le librerie su `volume` (la scansione riguarda un volume alla volta).
pub fn steam(reg: &dyn RegistrySource, files: &dyn FileSource, volume: &str) -> Result<Steam, ExtractError> {
    let root = match reg.value(Root::CurrentUser, r"Software\Valve\Steam", "SteamPath") {
        Ok(Some(v)) => normalize(v.as_str().ok_or(FormatIssue::WrongType(Field::Root))?),
        Ok(None) | Err(2) => return Err(ExtractError::NotPresent),
        Err(e) => return Err(ExtractError::Io(e)),
    };
    let lf = files.read(&join(&join(&root, "steamapps"), "libraryfolders.vdf"))?;
    let libraries = parse_libraryfolders(&lf)?;
    let mut out = Steam { root: root.clone(), ..Default::default() };
    for lib in libraries {
        if !lib.to_ascii_lowercase().starts_with(&volume.to_ascii_lowercase()) {
            continue;
        }
        let dir = join(&lib, "steamapps");
        let names = match files.list(&dir) {
            Ok(n) => n,
            Err(ExtractError::Format(f)) => {
                out.skipped.push((dir.clone(), f));
                continue;
            }
            Err(_) => continue,
        };
        for n in names.iter().filter(|n| n.to_ascii_lowercase().starts_with("appmanifest_") && n.to_ascii_lowercase().ends_with(".acf")) {
            let path = join(&dir, n);
            match files.read(&path).and_then(|t| parse_appmanifest(&t, &lib).map_err(ExtractError::Format)) {
                Ok(app) => out.apps.push(app),
                Err(ExtractError::Format(f)) => out.skipped.push((path, f)),
                Err(_) => {}
            }
        }
        out.libraries.push(lib);
    }
    // LastPlayed degli utenti: il valore più recente per gioco.
    let userdata = join(&root, "userdata");
    if let Ok(users) = files.list(&userdata) {
        for u in users {
            let path = join(&join(&join(&userdata, &u), "config"), "localconfig.vdf");
            match files.read(&path).and_then(|t| parse_localconfig(&t).map_err(ExtractError::Format)) {
                Ok(v) => {
                    for (id, t) in v {
                        match out.local_last_played.iter_mut().find(|(a, _)| *a == id) {
                            Some(x) => x.1 = x.1.max(t),
                            None => out.local_last_played.push((id, t)),
                        }
                    }
                }
                Err(ExtractError::Format(f)) => out.skipped.push((path, f)),
                Err(_) => {}
            }
        }
    }
    out.apps.sort_by_key(|a| a.appid);
    out.local_last_played.sort_unstable();
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Epic

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpicItem {
    pub display_name: String,
    pub app_name: String,
    pub install_location: String,
    pub version: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Epic {
    pub items: Vec<EpicItem>,
    pub skipped: Vec<(String, FormatIssue)>,
}

pub fn parse_epic_item(text: &str) -> Result<EpicItem, FormatIssue> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| if e.is_eof() { FormatIssue::Truncated } else { FormatIssue::Unexpected })?;
    let obj = v.as_object().ok_or(FormatIssue::Unexpected)?;
    let s = |k: &str, f: Field| -> Result<String, FormatIssue> {
        match obj.get(k) {
            None => Err(FormatIssue::MissingField(f)),
            Some(serde_json::Value::String(x)) => Ok(x.clone()),
            Some(_) => Err(FormatIssue::WrongType(f)),
        }
    };
    let install_location = normalize(&s("InstallLocation", Field::InstallLocation)?);
    if !is_absolute(&install_location) {
        return Err(FormatIssue::BadValue(Field::InstallLocation));
    }
    Ok(EpicItem {
        display_name: s("DisplayName", Field::DisplayName)?,
        app_name: s("AppName", Field::AppName)?,
        install_location,
        version: obj.get("AppVersionString").and_then(|x| x.as_str()).map(str::to_owned),
    })
}

pub const EPIC_MANIFESTS: &str = r"Epic\EpicGamesLauncher\Data\Manifests";

/// Manifest di Epic in `ProgramData\Epic\EpicGamesLauncher\Data\Manifests`. Nessun dato d'uso.
pub fn epic(files: &dyn FileSource, program_data: &str) -> Result<Epic, ExtractError> {
    let dir = join(program_data, EPIC_MANIFESTS);
    let names = files.list(&dir)?;
    let mut out = Epic::default();
    for n in names.iter().filter(|n| n.to_ascii_lowercase().ends_with(".item")) {
        let path = join(&dir, n);
        match files.read(&path).and_then(|t| parse_epic_item(&t).map_err(ExtractError::Format)) {
            Ok(i) => out.items.push(i),
            Err(ExtractError::Format(f)) => out.skipped.push((path, f)),
            Err(_) => {}
        }
    }
    out.items.sort_by(|a, b| a.app_name.cmp(&b.app_name));
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// CapCut

/// Versione completa da `Apps\ProductInfo.xml` (`<full_appver value="9.4.0.4015" />`).
pub fn parse_product_info(text: &str) -> Result<String, FormatIssue> {
    let t = text.trim_start_matches('\u{feff}').trim();
    if t.is_empty() {
        return Err(FormatIssue::Empty);
    }
    let body = match t.strip_prefix("<?xml") {
        Some(rest) => rest.split_once("?>").ok_or(FormatIssue::Truncated)?.1.trim_start(),
        None => t,
    };
    if !body.starts_with("<packet>") {
        return Err(FormatIssue::Unexpected);
    }
    if !body.trim_end().ends_with("</packet>") {
        return Err(FormatIssue::Truncated);
    }
    let k = body.find("<full_appver").ok_or(FormatIssue::MissingField(Field::Version))?;
    let tag = &body[k..body[k..].find('>').map_or(body.len(), |e| k + e)];
    let v = tag.split_once("value=\"").and_then(|(_, r)| r.split_once('"')).map(|(v, _)| v).ok_or(FormatIssue::MissingField(Field::Version))?;
    if parse_version(v).is_none() {
        return Err(FormatIssue::BadValue(Field::Version));
    }
    Ok(v.to_owned())
}

/// Versione a numeri puntati (`9.4.0.4015`), confrontabile.
pub fn parse_version(v: &str) -> Option<Vec<u32>> {
    let parts: Option<Vec<u32>> = v.split('.').map(|p| p.parse().ok()).collect();
    parts.filter(|p| (2..=4).contains(&p.len()))
}

pub const CAPCUT_PRODUCT_INFO: &str = r"CapCut\Apps\ProductInfo.xml";

pub fn capcut(files: &dyn FileSource, local_app_data: &str) -> Result<String, ExtractError> {
    let t = files.read(&join(local_app_data, CAPCUT_PRODUCT_INFO))?;
    Ok(parse_product_info(&t)?)
}

pub const CAPCUT_SETTINGS: &str = r"CapCut\User Data\Config\globalSetting";

/// Cartella delle bozze da `globalSetting` (`currentCustomDraftPath=C:\\...`, barre raddoppiate).
pub fn parse_capcut_drafts(text: &str) -> Result<String, FormatIssue> {
    if text.trim().is_empty() {
        return Err(FormatIssue::Empty);
    }
    let line = text.lines().find_map(|l| l.trim().strip_prefix("currentCustomDraftPath=")).ok_or(FormatIssue::MissingField(Field::InstallLocation))?;
    let p = normalize(&line.replace(r"\\", r"\"));
    if !is_absolute(&p) {
        return Err(FormatIssue::BadValue(Field::InstallLocation));
    }
    Ok(p)
}

pub fn capcut_drafts(files: &dyn FileSource, local_app_data: &str) -> Result<String, ExtractError> {
    let t = files.read(&join(local_app_data, CAPCUT_SETTINGS))?;
    Ok(parse_capcut_drafts(&t)?)
}

// ---------------------------------------------------------------------------------------------
// UserAssist e tracciamento degli avvii

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrackProgs {
    On,
    Off,
    /// Valore non registrato.
    Absent,
    /// Valore con un tipo o un numero inatteso.
    Unexpected,
    Unreadable(u32),
}

pub fn track_progs(reg: &dyn RegistrySource) -> TrackProgs {
    match reg.value(Root::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced", "Start_TrackProgs") {
        Ok(Some(RegValue::Dword(1))) => TrackProgs::On,
        Ok(Some(RegValue::Dword(0))) => TrackProgs::Off,
        Ok(Some(_)) => TrackProgs::Unexpected,
        Ok(None) | Err(2) => TrackProgs::Absent,
        Err(e) => TrackProgs::Unreadable(e),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserAssistEntry {
    /// Nome decodificato da ROT13 (percorso o identificativo dell'app).
    pub name: String,
    pub runs: u32,
    /// FILETIME dell'ultimo avvio; `None` se zero.
    pub last_run: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserAssist {
    /// Tracciamento non attivo: nessun valore si usa, anche se la chiave contiene dati vecchi (F-AP-4).
    NotTracked(TrackProgs),
    Entries(Vec<UserAssistEntry>),
}

fn rot13(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a') + 13) % 26 + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A') + 13) % 26 + b'A') as char,
            _ => c,
        })
        .collect()
}

const USERASSIST: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\UserAssist";

/// UserAssist (avvii da Esplora file e Start), solo con `Start_TrackProgs` = 1.
pub fn userassist(reg: &dyn RegistrySource) -> Result<UserAssist, ExtractError> {
    let track = track_progs(reg);
    if track != TrackProgs::On {
        return Ok(UserAssist::NotTracked(track));
    }
    let guids = reg.subkeys(Root::CurrentUser, USERASSIST).map_err(|e| if e == 2 { ExtractError::NotPresent } else { ExtractError::Io(e) })?;
    let mut out = Vec::new();
    for g in guids {
        let Ok(values) = reg.values(Root::CurrentUser, &format!(r"{USERASSIST}\{g}\Count")) else { continue };
        for (name, v) in values {
            let RegValue::Binary(b) = v else { return Err(FormatIssue::WrongType(Field::Data).into()) };
            // Formato da Windows 7: 72 byte; avvii a 4, ultimo avvio (FILETIME) a 60.
            if b.len() != 72 {
                if b.len() == 16 && name.starts_with("HRZR_PGYFRFFVBA") {
                    continue; // voce di sessione, senza conteggi per programma
                }
                return Err(FormatIssue::BadValue(Field::Data).into());
            }
            let runs = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
            let ft = i64::from_le_bytes([b[60], b[61], b[62], b[63], b[64], b[65], b[66], b[67]]);
            out.push(UserAssistEntry { name: rot13(&name), runs, last_run: (ft > 0).then_some(ft) });
        }
    }
    Ok(UserAssist::Entries(out))
}

// ---------------------------------------------------------------------------------------------
// Configurazione (F-SY-3)

/// Sensore memoria (`StorageSense\Parameters\StoragePolicy`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StorageSense {
    pub configured: bool,
    pub enabled: bool,
    /// Frequenza in giorni (1, 7, 30); 0 = quando lo spazio libero è poco.
    pub frequency_days: Option<u32>,
    /// Giorni dopo cui svuota gli elementi del Cestino; `None` se la regola è spenta.
    pub recycle_days: Option<u32>,
    /// Giorni dopo cui elimina i file di Download; `None` se la regola è spenta.
    pub downloads_days: Option<u32>,
}

const STORAGE_POLICY: &str = r"Software\Microsoft\Windows\CurrentVersion\StorageSense\Parameters\StoragePolicy";

pub fn storage_sense(reg: &dyn RegistrySource) -> Result<StorageSense, ExtractError> {
    let values = match reg.values(Root::CurrentUser, STORAGE_POLICY) {
        Ok(v) => v,
        Err(2) => return Ok(StorageSense::default()),
        Err(e) => return Err(ExtractError::Io(e)),
    };
    let d = |name: &str| -> Result<Option<u32>, ExtractError> {
        match values.iter().find(|(n, _)| n == name) {
            None => Ok(None),
            Some((_, RegValue::Dword(v))) => Ok(Some(*v)),
            Some(_) => Err(FormatIssue::WrongType(Field::Data).into()),
        }
    };
    let on = |name: &str| -> Result<bool, ExtractError> { Ok(d(name)? == Some(1)) };
    let days = |flag: &str, value: &str| -> Result<Option<u32>, ExtractError> {
        // Giorni a 0: "mai" per Windows, cioè regola spenta.
        Ok(if on(flag)? { d(value)?.filter(|&x| x > 0) } else { None })
    };
    Ok(StorageSense {
        configured: true,
        enabled: on("01")?,
        frequency_days: d("2048")?,
        recycle_days: days("08", "256")?,
        downloads_days: days("32", "512")?,
    })
}

/// Impostazioni del Cestino di un volume (`BitBucket\Volume\{GUID}`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BinSettings {
    /// Capacità in MiB; `None` se non registrata (vale la formula predefinita).
    pub max_capacity_mib: Option<u32>,
    pub nuke_on_delete: bool,
}

pub fn bin_settings(reg: &dyn RegistrySource, volume_guid: &str) -> Result<BinSettings, ExtractError> {
    let path = format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume\{volume_guid}");
    let values = match reg.values(Root::CurrentUser, &path) {
        Ok(v) => v,
        Err(2) => return Err(ExtractError::NotPresent),
        Err(e) => return Err(ExtractError::Io(e)),
    };
    let get = |name: &str| -> Result<Option<u32>, ExtractError> {
        match values.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
            None => Ok(None),
            Some((_, RegValue::Dword(v))) => Ok(Some(*v)),
            Some(_) => Err(FormatIssue::WrongType(Field::Data).into()),
        }
    };
    Ok(BinSettings { max_capacity_mib: get("MaxCapacity")?, nuke_on_delete: get("NukeOnDelete")? == Some(1) })
}

/// Alimentazione (API di alimentazione e registro, mai `powercfg`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Power {
    /// Ibernazione disponibile (stato S4 supportato e abilitato).
    pub hibernate: bool,
    pub hiberfile_present: bool,
    /// Avvio rapido (`HiberbootEnabled`); `None` se non registrato.
    pub fast_startup: Option<bool>,
    pub has_battery: bool,
    /// Standby moderno (`AoAc` delle capacità di alimentazione): in sospensione Windows iberna da solo il PC
    /// per non scaricare la batteria (Adaptive Hibernate).
    pub modern_standby: bool,
    /// Azione per batteria critica a batteria: 0 niente, 1 sospensione, 2 ibernazione, 3 arresto.
    pub critical_battery_action: Option<u32>,
}

/// Da dove viene il percorso di una cache di npm o di pnpm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConfigOrigin {
    Environment,
    Npmrc,
    /// Valore predefinito documentato.
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolPath {
    pub path: String,
    pub origin: ConfigOrigin,
}

/// Valore di una chiave in un `.npmrc` (`chiave=valore`, commenti con `;` o `#`).
pub fn npmrc_value(text: &str, key: &str) -> Option<String> {
    text.lines().map(str::trim).filter(|l| !l.starts_with(';') && !l.starts_with('#')).find_map(|l| {
        let (k, v) = l.split_once('=')?;
        k.trim().eq_ignore_ascii_case(key).then(|| v.trim().trim_matches('"').to_owned())
    })
}

/// Percorso di una cache dalla configurazione, senza eseguire npm: variabile d'ambiente, `.npmrc`
/// dell'utente, valore predefinito documentato.
pub fn tool_path(env: Option<String>, npmrc: Option<&str>, key: &str, default: String) -> ToolPath {
    if let Some(p) = env.map(|p| normalize(&p)).filter(|p| is_absolute(p)) {
        return ToolPath { path: p, origin: ConfigOrigin::Environment };
    }
    if let Some(p) = npmrc.and_then(|t| npmrc_value(t, key)).map(|p| normalize(&p)).filter(|p| is_absolute(p)) {
        return ToolPath { path: p, origin: ConfigOrigin::Npmrc };
    }
    ToolPath { path: normalize(&default), origin: ConfigOrigin::Default }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_path_hints() {
        assert_eq!(exe_path(r#""C:\Users\u\AppData\Local\CapCut\Apps\uninst.exe" /x"#), Some(r"C:\Users\u\AppData\Local\CapCut\Apps\uninst.exe".into()));
        assert_eq!(exe_path(r"C:\Program Files\App\app.exe,0"), Some(r"C:\Program Files\App\app.exe".into()));
        assert_eq!(exe_path(r"MsiExec.exe /X{1234}"), None);
        assert_eq!(exe_path(r"C:\Windows\System32\msiexec.exe /x {1}"), None);
        assert_eq!(exe_path("app.exe"), None);
    }

    #[test]
    fn product_info() {
        let ok = "<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<packet>\n    <full_appver value=\"9.4.0.4015\" />\n</packet>\n";
        assert_eq!(parse_product_info(ok), Ok("9.4.0.4015".into()));
        assert_eq!(parse_product_info(&ok[..60]), Err(FormatIssue::Truncated));
        assert_eq!(parse_product_info("<root><full_appver value=\"1.0\"/></root>"), Err(FormatIssue::Unexpected));
        assert_eq!(parse_product_info("<packet><appver value=\"9.4\"/></packet>"), Err(FormatIssue::MissingField(Field::Version)));
        assert_eq!(parse_product_info("<packet><full_appver value=\"nove\"/></packet>"), Err(FormatIssue::BadValue(Field::Version)));
    }

    #[test]
    fn npmrc_and_defaults() {
        let rc = "; commento\nregistry=https://registry.npmjs.org/\ncache = \"D:\\cache npm\"\n";
        assert_eq!(npmrc_value(rc, "cache"), Some(r"D:\cache npm".into()));
        let p = tool_path(None, Some(rc), "cache", r"C:\Users\u\AppData\Local\npm-cache".into());
        assert_eq!(p, ToolPath { path: r"D:\cache npm".into(), origin: ConfigOrigin::Npmrc });
        let p = tool_path(Some("relativo".into()), None, "cache", r"C:\x\".into());
        assert_eq!(p, ToolPath { path: r"C:\x".into(), origin: ConfigOrigin::Default });
    }
}
