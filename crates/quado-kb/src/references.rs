// SPDX-License-Identifier: GPL-3.0-or-later
//! Riferimenti a un elemento prima di spostarlo (03 F-AC-9, 05 D7 controllo 6): registro
//! (`UninstallString`, `DisplayIcon`, `InstallLocation`, App Paths, chiavi Run di HKCU e HKLM),
//! collegamenti `.lnk` di Start e Desktop (dell'utente e pubblici) letti con `IShellLinkW::GetPath`
//! senza `Resolve`, attività pianificate visibili all'utente (`ITaskService`). Sola lettura.
//!
//! La logica (`find`) lavora su sorgenti iniettabili: il registro simulato del §2.3 e collegamenti e
//! attività di prova. `find_live` usa quelle vere di `win`.
//!
//! Regole:
//! - Da ogni fonte si prende un percorso: l'eseguibile di una riga di comando (`UninstallString`, Run,
//!   valore predefinito di App Paths: tra virgolette, oppure fino a `.exe` seguito da spazio o dalla
//!   fine; senza `.exe` la riga intera), il file di `DisplayIcon` (`percorso,indice`), la cartella di
//!   `InstallLocation`, ciascuna cartella del valore `Path` di App Paths (separate da `;`), il bersaglio
//!   dei collegamenti, il percorso delle azioni `Exec` delle attività. Le `%VARIABILI%` si espandono
//!   prima; una variabile sconosciuta resta com'è. Gli argomenti non si guardano: `MsiExec.exe /X{GUID}`
//!   punta a msiexec, non all'elemento.
//! - Un riferimento punta a un elemento se il percorso, normalizzato (senza `\\?\`, `/` come `\`, senza
//!   `\` finale, senza distinzione di maiuscole), è uguale all'elemento o sta sotto di esso. Un
//!   riferimento alla cartella che contiene l'elemento non punta all'elemento.
//! - Un valore illeggibile o di tipo diverso da testo si salta. Una chiave che non si legge per un errore
//!   diverso da 2 (non trovata) rende il controllo parziale per i suoi tipi di riferimento; così le
//!   cartelle di collegamenti non leggibili (salvo 2 e 3, cartella assente) e le attività o cartelle di
//!   attività negate o non visibili all'utente (`win::LiveTasks` le trova anche quando la libreria le
//!   omette senza errore).
//! - Il registro si legge nel thread del chiamante; collegamenti e attività in due thread in parallelo.
//! - Non coperti (08 RS5): argomenti delle righe di comando, associazioni di file, PATH, collegamenti in
//!   altre cartelle, nomi brevi 8.3 nei percorsi delle fonti.

use std::collections::HashSet;

use quado_model::intervention::{Reference, ReferenceKind, ReferenceReport};

use crate::reg::{RegistrySource, Root};

/// Codice per un collegamento o una cartella di collegamenti che è un segnaposto cloud: non si legge,
/// per non scaricarlo (05 D6), e il controllo è parziale. È `ERROR_FILE_OFFLINE` di Windows.
pub const CLOUD_PLACEHOLDER: u32 = 4350;

/// Un collegamento `.lnk`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shortcut {
    /// Percorso del file `.lnk`.
    pub link: String,
    /// Bersaglio come scritto nel collegamento (`GetPath` con `SLGP_RAWPATH`): variabili non espanse.
    pub target: String,
}

/// Collegamenti letti da una sorgente.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShortcutList {
    /// Collegamenti con un bersaglio nel file system; quelli senza (pannello di controllo, collegamenti
    /// pubblicizzati di Windows Installer) non compaiono.
    pub links: Vec<Shortcut>,
    /// Cartelle o collegamenti non letti, con il codice di Windows. I codici 2 e 3 (cartella assente)
    /// non rendono il controllo parziale.
    pub errors: Vec<(String, u32)>,
}

/// Collegamenti di Start e Desktop.
pub trait ShortcutSource: Sync {
    fn shortcuts(&self) -> ShortcutList;
}

/// Un'attività pianificata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledTask {
    /// Percorso dell'attività nella libreria (`\Cartella\Nome`).
    pub path: String,
    /// `IExecAction::Path` delle azioni che avviano un programma, come scritto (variabili non espanse).
    pub executables: Vec<String>,
}

/// Attività lette da una sorgente.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskList {
    pub tasks: Vec<ScheduledTask>,
    /// Cartelle o attività non leggibili dall'utente, con il codice di Windows (5 = accesso negato):
    /// rendono il controllo parziale.
    pub unreadable: Vec<(String, u32)>,
}

/// Attività pianificate visibili all'utente.
pub trait TaskSource: Sync {
    fn tasks(&self) -> TaskList;
}

// ---------------------------------------------------------------------------------------------
// Estrazione dei percorsi

/// Espande `%NOME%` con `env`. Un nome sconosciuto e un `%` senza chiusura restano come sono, come fa
/// `ExpandEnvironmentStringsW`.
pub fn expand(s: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(a) = rest.find('%') {
        out.push_str(&rest[..a]);
        let after = &rest[a + 1..];
        match after.find('%') {
            Some(b) if b > 0 => {
                let name = &after[..b];
                match env(name) {
                    Some(v) => out.push_str(&v),
                    None => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[b + 1..];
            }
            Some(_) => {
                // `%%`: il primo resta, il secondo può aprire una variabile.
                out.push('%');
                rest = after;
            }
            None => {
                out.push_str(&rest[a..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Eseguibile di una riga di comando: tra virgolette, oppure fino al primo `.exe` seguito da uno spazio
/// o dalla fine; senza `.exe`, la riga intera (un percorso che sta sotto l'elemento si riconosce lo
/// stesso). `None` per una riga vuota.
pub fn command_path(cmd: &str) -> Option<&str> {
    let s = cmd.trim();
    let p = if let Some(rest) = s.strip_prefix('"') {
        rest.split('"').next().unwrap_or(rest)
    } else {
        let lower = s.to_ascii_lowercase();
        let mut from = 0;
        let mut found = None;
        while let Some(k) = lower[from..].find(".exe") {
            let end = from + k + 4;
            if s[end..].chars().next().is_none_or(char::is_whitespace) {
                found = Some(&s[..end]);
                break;
            }
            from = end;
        }
        found.unwrap_or(s)
    };
    let p = p.trim();
    (!p.is_empty()).then_some(p)
}

/// File di un `DisplayIcon`: `"percorso",indice`, `percorso,indice` o solo il percorso.
pub fn icon_path(s: &str) -> Option<&str> {
    let s = s.trim();
    let p = if let Some(rest) = s.strip_prefix('"') {
        rest.split('"').next().unwrap_or(rest)
    } else {
        match s.rsplit_once(',') {
            Some((p, i)) if i.trim().parse::<i32>().is_ok() => p,
            _ => s,
        }
    };
    let p = p.trim();
    (!p.is_empty()).then_some(p)
}

/// Percorso di una cartella (`InstallLocation`, una parte di `Path`): senza spazi e virgolette intorno.
fn folder_path(s: &str) -> Option<&str> {
    let p = s.trim().trim_matches('"').trim();
    (!p.is_empty()).then_some(p)
}

/// Chiave di confronto: senza `\\?\`, `/` come `\`, senza `\` finale (anche alla radice: `C:\` è `c:`),
/// in minuscolo.
fn key(p: &str) -> String {
    let p = p.trim();
    let p = if let Some(unc) = p.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        p.strip_prefix(r"\\?\").unwrap_or(p).to_owned()
    };
    let mut s = p.replace('/', "\\");
    while s.ends_with('\\') {
        s.pop();
    }
    s.to_lowercase()
}

/// Gli elementi da controllare, per chiave.
struct Items(HashSet<String>);

impl Items {
    /// Il percorso è uno degli elementi o sta sotto uno di essi.
    fn contains(&self, target: &str) -> bool {
        let k = key(target);
        let mut s = k.as_str();
        while !s.is_empty() {
            if self.0.contains(s) {
                return true;
            }
            match s.rfind('\\') {
                Some(i) => s = &s[..i],
                None => break,
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------------------------
// Fonti del registro

const UNINSTALL: [(Root, &str); 3] = [
    (Root::LocalMachine, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    (Root::LocalMachine, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
    (Root::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
];

const APP_PATHS: [(Root, &str); 3] = [
    (Root::LocalMachine, r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths"),
    (Root::LocalMachine, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\App Paths"),
    (Root::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\App Paths"),
];

const RUN: [(Root, &str, ReferenceKind); 3] = [
    (Root::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Run", ReferenceKind::RunUser),
    (Root::LocalMachine, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", ReferenceKind::RunMachine),
    (Root::LocalMachine, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run", ReferenceKind::RunMachine),
];

const UNINSTALL_KINDS: [ReferenceKind; 3] = [ReferenceKind::UninstallString, ReferenceKind::DisplayIcon, ReferenceKind::InstallLocation];

fn hive(root: Root) -> &'static str {
    match root {
        Root::LocalMachine => "HKLM",
        Root::CurrentUser => "HKCU",
    }
}

/// `HKLM\chiave\valore`; il valore predefinito si scrive `@`, come nei file `.reg`.
fn value_location(root: Root, key: &str, value: &str) -> String {
    let v = if value.is_empty() { "@" } else { value };
    format!(r"{}\{key}\{v}", hive(root))
}

/// Il controllo in corso: elementi, ambiente, esito.
struct Search<'a> {
    items: Items,
    env: &'a dyn Fn(&str) -> Option<String>,
    report: ReferenceReport,
}

impl Search<'_> {
    fn partial(&mut self, kind: ReferenceKind, code: u32) {
        if !self.report.partial.contains(&(kind, code)) {
            self.report.partial.push((kind, code));
        }
    }

    /// Espande `raw`, ne prende il percorso con `extract` e lo registra se punta a un elemento.
    fn check(&mut self, kind: ReferenceKind, location: impl FnOnce() -> String, raw: &str, extract: fn(&str) -> Option<&str>) {
        let expanded = expand(raw, self.env);
        let Some(target) = extract(&expanded) else { return };
        if self.items.contains(target) {
            let r = Reference { kind, location: location(), target: target.to_owned() };
            if !self.report.found.contains(&r) {
                self.report.found.push(r);
            }
        }
    }

    fn uninstall(&mut self, reg: &dyn RegistrySource) {
        for (root, path) in UNINSTALL {
            let keys = match reg.subkeys(root, path) {
                Ok(k) => k,
                Err(2) => continue,
                Err(e) => {
                    UNINSTALL_KINDS.iter().for_each(|&k| self.partial(k, e));
                    continue;
                }
            };
            for k in keys {
                let key = format!(r"{path}\{k}");
                let values = match reg.values(root, &key) {
                    Ok(v) => v,
                    Err(2) => continue,
                    Err(e) => {
                        UNINSTALL_KINDS.iter().for_each(|&k| self.partial(k, e));
                        continue;
                    }
                };
                for (name, v) in &values {
                    let (kind, extract): (ReferenceKind, fn(&str) -> Option<&str>) = if name.eq_ignore_ascii_case("UninstallString") {
                        (ReferenceKind::UninstallString, command_path)
                    } else if name.eq_ignore_ascii_case("DisplayIcon") {
                        (ReferenceKind::DisplayIcon, icon_path)
                    } else if name.eq_ignore_ascii_case("InstallLocation") {
                        (ReferenceKind::InstallLocation, folder_path)
                    } else {
                        continue;
                    };
                    let Some(raw) = v.as_str() else { continue };
                    self.check(kind, || value_location(root, &key, name), raw, extract);
                }
            }
        }
    }

    fn app_paths(&mut self, reg: &dyn RegistrySource) {
        for (root, path) in APP_PATHS {
            let keys = match reg.subkeys(root, path) {
                Ok(k) => k,
                Err(2) => continue,
                Err(e) => {
                    self.partial(ReferenceKind::AppPaths, e);
                    continue;
                }
            };
            for k in keys {
                let key = format!(r"{path}\{k}");
                let values = match reg.values(root, &key) {
                    Ok(v) => v,
                    Err(2) => continue,
                    Err(e) => {
                        self.partial(ReferenceKind::AppPaths, e);
                        continue;
                    }
                };
                for (name, v) in &values {
                    let Some(raw) = v.as_str() else { continue };
                    if name.is_empty() {
                        self.check(ReferenceKind::AppPaths, || value_location(root, &key, name), raw, command_path);
                    } else if name.eq_ignore_ascii_case("Path") {
                        for part in raw.split(';') {
                            self.check(ReferenceKind::AppPaths, || value_location(root, &key, name), part, folder_path);
                        }
                    }
                }
            }
        }
    }

    fn run(&mut self, reg: &dyn RegistrySource) {
        for (root, path, kind) in RUN {
            let values = match reg.values(root, path) {
                Ok(v) => v,
                Err(2) => continue,
                Err(e) => {
                    self.partial(kind, e);
                    continue;
                }
            };
            for (name, v) in &values {
                // Solo testi: un REG_MULTI_SZ o un numero in Run non è una riga di comando.
                let Some(raw) = v.as_str() else { continue };
                self.check(kind, || value_location(root, path, name), raw, command_path);
            }
        }
    }

    fn shortcuts(&mut self, list: &ShortcutList) {
        for (_, code) in &list.errors {
            if !matches!(code, 2 | 3) {
                self.partial(ReferenceKind::Shortcut, *code);
            }
        }
        for l in &list.links {
            self.check(ReferenceKind::Shortcut, || l.link.clone(), &l.target, folder_path);
        }
    }

    fn tasks(&mut self, list: &TaskList) {
        for (_, code) in &list.unreadable {
            self.partial(ReferenceKind::ScheduledTask, *code);
        }
        for t in &list.tasks {
            for exe in &t.executables {
                // Il percorso dell'azione è un solo file, a volte tra virgolette.
                self.check(ReferenceKind::ScheduledTask, || t.path.clone(), exe, folder_path);
            }
        }
    }
}

/// Riferimenti a uno dei `paths` (o a qualcosa al loro interno) nelle fonti date. Ordine dei
/// riferimenti trovati: voci di disinstallazione (HKLM, HKLM a 32 bit, HKCU), App Paths, Run, collegamenti,
/// attività; senza doppioni. `partial` senza doppioni di (tipo, codice).
pub fn find(reg: &dyn RegistrySource, shortcuts: &dyn ShortcutSource, tasks: &dyn TaskSource, env: &dyn Fn(&str) -> Option<String>, paths: &[String]) -> ReferenceReport {
    let items = Items(paths.iter().map(|p| key(p)).filter(|k| !k.is_empty()).collect());
    if items.0.is_empty() {
        return ReferenceReport::default();
    }
    // Collegamenti e attività passano da COM e dal servizio Utilità di pianificazione: si leggono in
    // parallelo al registro.
    let (links, task_list, mut s) = std::thread::scope(|scope| {
        let links = scope.spawn(|| shortcuts.shortcuts());
        let task_list = scope.spawn(|| tasks.tasks());
        let mut s = Search { items, env, report: ReferenceReport::default() };
        s.uninstall(reg);
        s.app_paths(reg);
        s.run(reg);
        (links.join(), task_list.join(), s)
    });
    match links {
        Ok(l) => s.shortcuts(&l),
        Err(_) => s.partial(ReferenceKind::Shortcut, 0),
    }
    match task_list {
        Ok(t) => s.tasks(&t),
        Err(_) => s.partial(ReferenceKind::ScheduledTask, 0),
    }
    s.report
}

/// Riferimenti a uno dei `paths` (o a qualcosa al loro interno) nelle fonti vere di questo PC: registro
/// in sola lettura, collegamenti di Start e Desktop dell'utente e pubblici, attività pianificate
/// visibili all'utente, variabili d'ambiente del processo. Si può chiamare da qualunque thread.
#[cfg(windows)]
pub fn find_live(paths: &[String]) -> ReferenceReport {
    let env = |name: &str| std::env::var(name).ok();
    find(&crate::win::LiveRegistry, &crate::win::LiveShortcuts::system(), &crate::win::LiveTasks, &env, paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(name: &str) -> Option<String> {
        match name.to_ascii_uppercase().as_str() {
            "LOCALAPPDATA" => Some(r"C:\Users\u\AppData\Local".into()),
            "EMPTY" => Some(String::new()),
            _ => None,
        }
    }

    #[test]
    fn expands_variables() {
        assert_eq!(expand(r"%LocalAppData%\CapCut", &env), r"C:\Users\u\AppData\Local\CapCut");
        assert_eq!(expand(r"%NONE%\x", &env), r"%NONE%\x");
        assert_eq!(expand(r"%EMPTY%x%LOCALAPPDATA%", &env), r"xC:\Users\u\AppData\Local");
        assert_eq!(expand("100% sicuro", &env), "100% sicuro");
        assert_eq!(expand("%", &env), "%");
    }

    #[test]
    fn command_lines() {
        assert_eq!(command_path(r#""C:\a b\x.exe" /s"#), Some(r"C:\a b\x.exe"));
        assert_eq!(command_path(r"C:\a b\x.exe /s"), Some(r"C:\a b\x.exe"));
        assert_eq!(command_path(r"C:\a\x.exe"), Some(r"C:\a\x.exe"));
        assert_eq!(command_path(r"C:\a.executor\x.EXE -y"), Some(r"C:\a.executor\x.EXE"));
        assert_eq!(command_path("MsiExec.exe /X{1234}"), Some("MsiExec.exe"));
        assert_eq!(command_path(r"C:\a\run.bat"), Some(r"C:\a\run.bat"));
        assert_eq!(command_path(r#""C:\a\x.exe"#), Some(r"C:\a\x.exe"));
        assert_eq!(command_path("  "), None);
        assert_eq!(icon_path(r#""C:\a\x.exe",0"#), Some(r"C:\a\x.exe"));
        assert_eq!(icon_path(r"C:\a\x.exe,-101"), Some(r"C:\a\x.exe"));
        assert_eq!(icon_path(r"C:\a,b\x.ico"), Some(r"C:\a,b\x.ico"));
    }

    #[test]
    fn matching_is_equal_or_below() {
        let items = Items([key(r"C:\Users\u\AppData\Local\CapCut\Apps\9.2.0.1")].into_iter().collect());
        assert!(items.contains(r"c:\users\U\appdata\local\capcut\apps\9.2.0.1"));
        assert!(items.contains(r"\\?\C:\Users\u\AppData\Local\CapCut\Apps\9.2.0.1\CapCut.exe"));
        assert!(items.contains(r"C:/Users/u/AppData/Local/CapCut/Apps/9.2.0.1/"));
        assert!(!items.contains(r"C:\Users\u\AppData\Local\CapCut\Apps"));
        assert!(!items.contains(r"C:\Users\u\AppData\Local\CapCut\Apps\9.2.0.10\CapCut.exe"));
        assert!(!items.contains(""));
        let root = Items([key(r"\\?\UNC\srv\share\x")].into_iter().collect());
        assert!(root.contains(r"\\srv\share\x\y.exe"));
    }
}
