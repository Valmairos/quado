// SPDX-License-Identifier: GPL-3.0-or-later
//! Riferimenti (03 F-AC-9, 05 D7 controllo 6, 06 §5.5): registro simulato nel formato `.reg` (06 §2.3)
//! con collegamenti e attività di prova; un collegamento vero, creato in una cartella temporanea, letto
//! senza `Resolve`; le attività vere di questo PC in sola lettura. Il controllo manuale con HKCU e
//! un'attività dell'utente vere è ignorato: scrive, e si esegue a mano dopo ogni modifica dei riferimenti.
//!
//! `unsafe` qui serve solo a creare il `.lnk` di prova con `IShellLinkW::SetPath` e
//! `IPersistFile::Save`: il prodotto (`quado-kb`) non scrive collegamenti.

#![allow(unsafe_code)]

use std::path::{Path, PathBuf};
use std::time::Instant;

use quado_kb::references::{self, ScheduledTask, Shortcut, ShortcutList, ShortcutSource, TaskList, TaskSource};
use quado_kb::reg::{RegFile, RegValue, RegistrySource, Root};
use quado_kb::win::{LiveShortcuts, LiveTasks};
use quado_model::intervention::{Reference, ReferenceKind, ReferenceReport};

const VERSION: &str = r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1";
const OTHER_VERSION: &str = r"C:\Users\demo\AppData\Local\CapCut\Apps\9.3.0.3970";
const LOCAL: &str = r"C:\Users\demo\AppData\Local";

/// Valore `REG_EXPAND_SZ` nel formato di `reg export` (`hex(2):` UTF-16LE con lo zero finale).
fn hex2(s: &str) -> String {
    let bytes: Vec<String> = s.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).map(|b| format!("{b:02x}")).collect();
    format!("hex(2):{}", bytes.join(","))
}

/// Registro di prova: la cartella di versione `9.2.0.1` di CapCut citata da `UninstallString` con
/// argomenti (HKLM a 32 bit), `DisplayIcon` (HKCU), `InstallLocation` con la barra finale (voce senza
/// nome), App Paths (valore predefinito e `Path`), Run di HKCU, di HKLM (con una variabile, in un
/// `REG_EXPAND_SZ`) e di HKLM a 32 bit (in minuscolo). Più riferimenti che non contano: la cartella
/// `Apps` che contiene la versione, MsiExec con la versione negli argomenti, un'altra versione, valori di
/// tipo sbagliato.
fn registry_text() -> String {
    let updater = hex2(r"%LOCALAPPDATA%\CapCut\Apps\9.2.0.1\updater.exe /silent");
    format!(
        r#"Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{{11111111-2222-3333-4444-555555555555}}]
"DisplayName"="Componente MSI"
"UninstallString"="MsiExec.exe /X{{11111111-2222-3333-4444-555555555555}} INSTALLDIR=\"C:\\Users\\demo\\AppData\\Local\\CapCut\\Apps\\9.2.0.1\""
"DisplayIcon"="C:\\Windows\\Installer\\{{11111111-2222-3333-4444-555555555555}}\\icona.exe,0"

[HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\CapCutHelper]
"DisplayName"="CapCut Helper"
"UninstallString"="C:\\Users\\demo\\AppData\\Local\\CapCut\\Apps\\9.2.0.1\\uninst.exe /S /lang=it"
"DisplayIcon"=dword:00000001
"InstallLocation"=hex:01,02

[HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Uninstall\CapCut]
"DisplayName"="CapCut"
"DisplayIcon"="C:\\Users\\demo\\AppData\\Local\\CapCut\\Apps\\9.2.0.1\\CapCut.exe,0"
"UninstallString"="\"C:\\Users\\demo\\AppData\\Local\\CapCut\\Apps\\uninst.exe\""
"InstallLocation"="C:\\Users\\demo\\AppData\\Local\\CapCut\\Apps"
"DisplayVersion"="9.4.0.4015"

[HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Uninstall\CapCut 9.2]
"InstallLocation"="C:\\Users\\demo\\AppData\\Local\\CapCut\\Apps\\9.2.0.1\\"

[HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Uninstall\CapCut 9.3]
"InstallLocation"="C:\\Users\\demo\\AppData\\Local\\CapCut\\Apps\\9.3.0.3970"

[HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\App Paths\CapCut.exe]
@="\"C:\\Users\\demo\\AppData\\Local\\CapCut\\Apps\\9.2.0.1\\CapCut.exe\""
"Path"="C:\\Users\\demo\\AppData\\Local\\CapCut\\Apps;C:\\Users\\demo\\AppData\\Local\\CapCut\\Apps\\9.2.0.1\\bin"

[HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run]
"CapCut"="\"C:\\Users\\demo\\AppData\\Local\\CapCut\\Apps\\9.2.0.1\\CapCut.exe\" --autostart"
"Altro"="C:\\Program Files\\Altro\\altro.exe"
"Numero"=dword:00000001
"Lista"=hex(7):41,00,00,00,00,00

[HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Run]
"CapCutUpdater"={updater}

[HKEY_LOCAL_MACHINE\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run]
"Helper"="c:\\users\\DEMO\\appdata\\local\\capcut\\apps\\9.2.0.1\\Helper.exe"
"#
    )
}

fn registry() -> RegFile {
    RegFile::parse(&registry_text()).expect("registro di prova")
}

fn env(name: &str) -> Option<String> {
    name.eq_ignore_ascii_case("LOCALAPPDATA").then(|| LOCAL.to_owned())
}

struct FakeShortcuts(ShortcutList);

impl ShortcutSource for FakeShortcuts {
    fn shortcuts(&self) -> ShortcutList {
        self.0.clone()
    }
}

struct FakeTasks(TaskList);

impl TaskSource for FakeTasks {
    fn tasks(&self) -> TaskList {
        self.0.clone()
    }
}

const START_LINK: &str = r"C:\Users\demo\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\CapCut.lnk";

/// Collegamenti di prova: uno nel menu Start alla versione (con una variabile), uno sul Desktop al
/// lanciatore in `Apps`; il Desktop pubblico non c'è (codice 2, non parziale).
fn shortcuts() -> FakeShortcuts {
    FakeShortcuts(ShortcutList {
        links: vec![
            Shortcut { link: START_LINK.into(), target: r"%LOCALAPPDATA%\CapCut\Apps\9.2.0.1\CapCut.exe".into() },
            Shortcut { link: r"C:\Users\demo\Desktop\CapCut.lnk".into(), target: r"C:\Users\demo\AppData\Local\CapCut\Apps\CapCut.exe".into() },
        ],
        errors: vec![(r"C:\Users\Public\Desktop".into(), 2)],
    })
}

/// Attività di prova: una che avvia l'aggiornamento della versione (percorso tra virgolette), una di
/// sistema, una cartella non leggibile dall'utente.
fn tasks() -> FakeTasks {
    FakeTasks(TaskList {
        tasks: vec![
            ScheduledTask { path: r"\CapCut\Updater".into(), executables: vec![r#""C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1\updater.exe""#.into()] },
            ScheduledTask { path: r"\Altro".into(), executables: vec![r"%windir%\system32\cmd.exe".into(), String::new()] },
        ],
        unreadable: vec![(r"\Microsoft\Windows\Segreta".into(), 5)],
    })
}

fn find_in(reg: &dyn RegistrySource, paths: &[&str]) -> ReferenceReport {
    let paths: Vec<String> = paths.iter().map(|p| (*p).to_owned()).collect();
    references::find(reg, &shortcuts(), &tasks(), &env, &paths)
}

fn r(kind: ReferenceKind, location: &str, target: &str) -> Reference {
    Reference { kind, location: location.into(), target: target.into() }
}

fn expected() -> Vec<Reference> {
    use ReferenceKind::*;
    vec![
        r(UninstallString, r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\CapCutHelper\UninstallString", r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1\uninst.exe"),
        r(DisplayIcon, r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\CapCut\DisplayIcon", r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1\CapCut.exe"),
        r(InstallLocation, r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\CapCut 9.2\InstallLocation", r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1\"),
        r(AppPaths, r"HKCU\Software\Microsoft\Windows\CurrentVersion\App Paths\CapCut.exe\@", r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1\CapCut.exe"),
        r(AppPaths, r"HKCU\Software\Microsoft\Windows\CurrentVersion\App Paths\CapCut.exe\Path", r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1\bin"),
        r(RunUser, r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run\CapCut", r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1\CapCut.exe"),
        r(RunMachine, r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run\CapCutUpdater", r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1\updater.exe"),
        r(RunMachine, r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run\Helper", r"c:\users\DEMO\appdata\local\capcut\apps\9.2.0.1\Helper.exe"),
        r(Shortcut, START_LINK, r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1\CapCut.exe"),
        r(ScheduledTask, r"\CapCut\Updater", r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1\updater.exe"),
    ]
}

#[test]
fn every_source_finds_the_version_folder() {
    let report = find_in(&registry(), &[VERSION]);
    assert_eq!(report.found, expected());
    // Solo la cartella di attività negata rende il controllo parziale; il Desktop pubblico assente no.
    assert_eq!(report.partial, [(ReferenceKind::ScheduledTask, 5)]);
    // Il fixture usa davvero un REG_EXPAND_SZ per la variabile.
    let run = registry().values(Root::LocalMachine, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run").expect("Run");
    assert!(matches!(&run[0].1, RegValue::ExpandSz(s) if s.starts_with("%LOCALAPPDATA%")));
}

#[test]
fn the_containing_folder_and_msiexec_do_not_count() {
    let report = find_in(&registry(), &[VERSION]);
    let msi = r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{11111111-2222-3333-4444-555555555555}";
    assert!(report.found.iter().all(|x| !x.location.starts_with(msi)), "MsiExec punta a msiexec, non alla versione");
    // `InstallLocation`, `UninstallString` di CapCut, la prima parte di `Path` e il collegamento sul Desktop
    // indicano `Apps` o un file in `Apps`: contengono la versione, non puntano alla versione.
    assert!(report.found.iter().all(|x| !x.location.ends_with(r"Uninstall\CapCut\InstallLocation")));
    assert!(report.found.iter().all(|x| !x.location.ends_with(r"Uninstall\CapCut\UninstallString")));
    assert!(report.found.iter().all(|x| x.location != r"C:\Users\demo\Desktop\CapCut.lnk"));
    assert_eq!(report.found.iter().filter(|x| x.kind == ReferenceKind::AppPaths).count(), 2);
}

#[test]
fn the_containing_folder_itself_is_referenced() {
    // Se l'elemento è `Apps`, i riferimenti a `Apps` e a tutto ciò che contiene contano.
    let report = find_in(&registry(), &[r"C:\Users\demo\AppData\Local\CapCut\Apps"]);
    let at = |loc: &str| report.found.iter().any(|x| x.location.ends_with(loc));
    assert!(at(r"Uninstall\CapCut\InstallLocation"));
    assert!(at(r"Uninstall\CapCut\UninstallString"));
    assert!(at(r"Uninstall\CapCut 9.3\InstallLocation"));
    assert!(at(r"Desktop\CapCut.lnk"));
    assert_eq!(report.found.len(), expected().len() + 4 + 1, "{:#?}", report.found);
}

#[test]
fn another_path_is_not_found() {
    let report = find_in(&registry(), &[r"C:\Users\demo\AppData\Local\CapCut\Apps\9.4.0.4015", r"D:\Altro"]);
    assert!(report.found.is_empty(), "{:#?}", report.found);
    assert_eq!(report.partial, [(ReferenceKind::ScheduledTask, 5)]);
    // Una versione con un prefisso uguale (`9.2.0.10`) non è sotto `9.2.0.1`.
    let report = find_in(&registry(), &[r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.10"]);
    assert!(report.found.is_empty(), "{:#?}", report.found);
}

#[test]
fn case_and_form_of_the_paths_do_not_matter() {
    let upper = VERSION.to_uppercase();
    let report = find_in(&registry(), &[&format!(r"{upper}\")]);
    assert_eq!(report.found, expected());
    let report = find_in(&registry(), &[&format!(r"\\?\{}", VERSION.replace('\\', "/"))]);
    assert_eq!(report.found, expected());
    // Più elementi: ogni riferimento una volta sola.
    let report = find_in(&registry(), &[VERSION, OTHER_VERSION, &format!(r"{VERSION}\bin")]);
    assert_eq!(report.found.len(), expected().len() + 1, "{:#?}", report.found);
    assert!(report.found.iter().any(|x| x.location.ends_with(r"Uninstall\CapCut 9.3\InstallLocation")));
}

#[test]
fn environment_variables_are_expanded() {
    let report = find_in(&registry(), &[VERSION]);
    let link = report.found.iter().find(|x| x.kind == ReferenceKind::Shortcut).expect("collegamento");
    assert_eq!(link.target, r"C:\Users\demo\AppData\Local\CapCut\Apps\9.2.0.1\CapCut.exe");
    // Senza la variabile il riferimento non si riconosce e il valore resta com'è.
    let paths = [VERSION.to_owned()];
    let none = |_: &str| None;
    let report = references::find(&registry(), &shortcuts(), &tasks(), &none, &paths);
    assert!(report.found.iter().all(|x| x.kind != ReferenceKind::Shortcut && x.location != r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run\CapCutUpdater"));
    assert_eq!(report.found.len(), expected().len() - 2);
}

/// Registro con alcune chiavi negate (codice 5) o rotte (codice 1018).
struct Denied<'a> {
    inner: &'a RegFile,
    keys: Vec<(Root, &'static str, u32)>,
}

impl Denied<'_> {
    fn denied(&self, root: Root, path: &str) -> Option<u32> {
        self.keys.iter().find(|(r, p, _)| *r == root && p.eq_ignore_ascii_case(path)).map(|(_, _, c)| *c)
    }
}

impl RegistrySource for Denied<'_> {
    fn subkeys(&self, root: Root, path: &str) -> Result<Vec<String>, u32> {
        match self.denied(root, path) {
            Some(c) => Err(c),
            None => self.inner.subkeys(root, path),
        }
    }

    fn values(&self, root: Root, path: &str) -> Result<Vec<(String, RegValue)>, u32> {
        match self.denied(root, path) {
            Some(c) => Err(c),
            None => self.inner.values(root, path),
        }
    }
}

#[test]
fn unreadable_root_keys_make_the_check_partial() {
    let reg = registry();
    let denied = Denied {
        inner: &reg,
        keys: vec![
            (Root::LocalMachine, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall", 5),
            (Root::LocalMachine, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", 5),
            (Root::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\App Paths\CapCut.exe", 1018),
        ],
    };
    let report = find_in(&denied, &[VERSION]);
    use ReferenceKind::*;
    assert_eq!(
        report.partial,
        [(UninstallString, 5), (DisplayIcon, 5), (InstallLocation, 5), (AppPaths, 1018), (RunMachine, 5), (ScheduledTask, 5)]
    );
    // Le altre fonti si leggono lo stesso.
    let kinds: Vec<ReferenceKind> = report.found.iter().map(|x| x.kind).collect();
    assert_eq!(kinds, [DisplayIcon, InstallLocation, RunUser, RunMachine, Shortcut, ScheduledTask]);
    // Una chiave assente (codice 2) non è una lettura parziale.
    let missing = Denied { inner: &reg, keys: vec![(Root::CurrentUser, r"Software\Microsoft\Windows\CurrentVersion\Run", 2)] };
    let report = find_in(&missing, &[VERSION]);
    assert_eq!(report.partial, [(ScheduledTask, 5)]);
    assert_eq!(report.found.len(), expected().len() - 1);
}

#[test]
fn unreadable_shortcuts_and_tasks_make_the_check_partial() {
    let paths = [VERSION.to_owned()];
    let links = FakeShortcuts(ShortcutList { links: Vec::new(), errors: vec![(r"C:\ProgramData\Microsoft\Windows\Start Menu\Privata".into(), 5), (r"C:\x.lnk".into(), 3)] });
    let tasks = FakeTasks(TaskList { tasks: Vec::new(), unreadable: vec![(r"\".into(), 5), (r"\Microsoft".into(), 5), (r"\Rotta".into(), 0x8004_1309)] });
    let empty = RegFile::parse("Windows Registry Editor Version 5.00\r\n").expect("vuoto");
    let report = references::find(&empty, &links, &tasks, &env, &paths);
    assert!(report.found.is_empty());
    assert_eq!(report.partial, [(ReferenceKind::Shortcut, 5), (ReferenceKind::ScheduledTask, 5), (ReferenceKind::ScheduledTask, 0x8004_1309)]);
}

#[test]
fn no_paths_no_work() {
    let report = references::find(&registry(), &shortcuts(), &tasks(), &env, &[]);
    assert_eq!(report, ReferenceReport::default());
    let report = references::find(&registry(), &shortcuts(), &tasks(), &env, &[String::new(), "  ".into()]);
    assert_eq!(report, ReferenceReport::default());
}

// ---------------------------------------------------------------------------------------------
// Fonti vere

/// Cartella temporanea dei test, cancellata alla fine anche se il test fallisce.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let p = std::env::temp_dir().join(format!("quado-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&p).expect("cartella temporanea");
        Self(p)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Scrive un `.lnk` che punta a `target` (che non deve esistere) con `IShellLinkW::SetPath` e
/// `IPersistFile::Save`, solo nella cartella temporanea del test.
fn write_link(link: &Path, target: &Path) {
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{HSTRING, Interface};
    assert!(link.starts_with(std::env::temp_dir()), "solo nella cartella temporanea");
    // SAFETY: inizializzazione COM del thread del test, bilanciata sotto se riuscita.
    let init = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    {
        // SAFETY: CLSID del collegamento della shell e la sua interfaccia; COM è inizializzato.
        let sl: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.expect("ShellLink");
        // SAFETY: percorso terminato da zero valido per la chiamata.
        unsafe { sl.SetPath(&HSTRING::from(target.as_os_str())) }.expect("SetPath");
        let pf: IPersistFile = sl.cast().expect("IPersistFile");
        // SAFETY: percorso del `.lnk` nella cartella temporanea, terminato da zero.
        unsafe { pf.Save(&HSTRING::from(link.as_os_str()), true) }.expect("Save");
    }
    if init {
        // SAFETY: bilancia la CoInitializeEx riuscita; gli oggetti COM sono già rilasciati.
        unsafe { CoUninitialize() };
    }
}

#[test]
fn real_shortcut_is_read_without_resolve() {
    let dir = TempDir::new("collegamenti");
    let missing = dir.0.join("inesistente");
    let version = missing.join("9.2.0.1");
    let target = version.join("CapCut.exe");
    let other = missing.join("altro.exe");
    let link = dir.0.join("CapCut.lnk");
    let nested = dir.0.join("Programs").join("Sub");
    std::fs::create_dir_all(&nested).expect("sottocartella");
    let nested_link = nested.join("Altro.lnk");
    std::fs::write(dir.0.join("nota.txt"), b"non un collegamento").expect("file");
    write_link(&link, &target);
    write_link(&nested_link, &other);
    assert!(!missing.exists(), "la creazione del collegamento non crea il bersaglio");

    let source = LiveShortcuts::new(vec![dir.0.to_string_lossy().into_owned()]);
    let list = source.shortcuts();
    assert!(list.errors.is_empty(), "{:?}", list.errors);
    let mut got: Vec<(String, String)> = list.links.iter().map(|l| (l.link.clone(), l.target.clone())).collect();
    got.sort();
    let mut want = vec![
        (link.to_string_lossy().into_owned(), target.to_string_lossy().into_owned()),
        (nested_link.to_string_lossy().into_owned(), other.to_string_lossy().into_owned()),
    ];
    want.sort();
    assert_eq!(got, want);
    // Leggere i collegamenti non crea né tocca il bersaglio.
    assert!(!missing.exists());

    let empty = RegFile::parse("Windows Registry Editor Version 5.00\r\n").expect("vuoto");
    let no_tasks = FakeTasks(TaskList::default());
    let report = references::find(&empty, &source, &no_tasks, &env, &[version.to_string_lossy().into_owned()]);
    assert_eq!(report.found, [Reference { kind: ReferenceKind::Shortcut, location: link.to_string_lossy().into_owned(), target: target.to_string_lossy().into_owned() }]);
    assert!(report.partial.is_empty());
    assert!(!missing.exists());
}

#[test]
fn missing_shortcut_folder_is_not_partial() {
    let dir = TempDir::new("collegamenti-assenti");
    let source = LiveShortcuts::new(vec![dir.0.join("non-esiste").to_string_lossy().into_owned()]);
    let list = source.shortcuts();
    assert!(list.links.is_empty());
    assert!(list.errors.iter().all(|(_, c)| matches!(c, 2 | 3)), "{:?}", list.errors);
    let empty = RegFile::parse("Windows Registry Editor Version 5.00\r\n").expect("vuoto");
    let report = references::find(&empty, &source, &FakeTasks(TaskList::default()), &env, &[VERSION.to_owned()]);
    assert_eq!(report, ReferenceReport::default());
}

#[test]
fn live_tasks_are_read_only_and_complete_or_partial() {
    // Sola lettura: enumera le attività visibili all'utente, non ne crea.
    let list = LiveTasks.tasks();
    assert!(!list.tasks.is_empty() || !list.unreadable.is_empty());
    assert!(list.tasks.iter().all(|t| t.path.starts_with('\\')), "{:?}", list.tasks.iter().map(|t| &t.path).collect::<Vec<_>>());
    assert!(list.unreadable.iter().all(|(_, c)| *c != 0));
    // Da un altro thread, con COM già inizializzato in un altro modello: funziona lo stesso.
    let other = std::thread::spawn(|| {
        use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
        // SAFETY: COM in modalità multithread su questo thread, bilanciato sotto.
        let init = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        let n = LiveTasks.tasks().tasks.len();
        if init {
            // SAFETY: bilancia la CoInitializeEx riuscita.
            unsafe { CoUninitialize() };
        }
        n
    })
    .join()
    .expect("thread");
    assert_eq!(other, list.tasks.len());
}

#[test]
#[ignore = "legge registro, collegamenti e attività di questo PC: misura manuale"]
fn measure_find_live_on_this_pc() {
    let t = Instant::now();
    let source = LiveShortcuts::system();
    let links = source.shortcuts();
    let t_links = t.elapsed();
    let t = Instant::now();
    let tasks = LiveTasks.tasks();
    let t_tasks = t.elapsed();
    println!("cartelle dei collegamenti: {:?}", source.folders());
    println!("collegamenti: {} in {:?}; errori: {:?}", links.links.len(), t_links, links.errors);
    let actions: usize = tasks.tasks.iter().map(|t| t.executables.len()).sum();
    println!("attività: {} ({} azioni Exec) in {:?}; non leggibili: {}", tasks.tasks.len(), actions, t_tasks, tasks.unreadable.len());
    for (p, c) in tasks.unreadable.iter().take(10) {
        println!("  non leggibile: {p} ({c})");
    }
    // Le altre fonti dell'elenco completo delle attività non si leggono senza amministratore (14/10/2026:
    // codice 5 per entrambe), per questo `LiveTasks` confronta le sottocartelle di `System32\Tasks`.
    let cache = quado_kb::win::LiveRegistry.subkeys(Root::LocalMachine, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\TaskCache\Tree").map(|k| k.len());
    let store = std::fs::read_dir(r"C:\Windows\System32\Tasks").map(|_| ()).map_err(|e| e.raw_os_error());
    println!("TaskCache\\Tree: {cache:?}; System32\\Tasks: {store:?}");
    let paths = [r"C:\Users\Public\Quado\non-esiste\9.9.9".to_owned()];
    let mut times = Vec::new();
    let mut report = ReferenceReport::default();
    for _ in 0..5 {
        let t = Instant::now();
        report = references::find_live(&paths);
        times.push(t.elapsed().as_millis());
    }
    println!("find_live: {times:?} ms; trovati {}; parziale {:?}", report.found.len(), report.partial);
    let capcut = std::env::var("LOCALAPPDATA").map(|l| format!(r"{l}\CapCut\Apps")).unwrap_or_default();
    let t = Instant::now();
    let report = references::find_live(&[capcut]);
    println!("find_live su CapCut\\Apps in {:?}: {:#?}", t.elapsed(), report);
}

/// Controllo manuale di 06 §5.5 con HKCU e un'attività dell'utente veri: un valore Run e un'attività
/// che puntano a un file inesistente in una cartella di versione sotto la cartella temporanea, poi
/// `find_live`; valori e attività si rimuovono alla fine anche se il test fallisce.
/// `cargo test -p quado-kb --test riferimenti -- --ignored manual --nocapture`
#[test]
#[ignore = "controllo manuale: scrive in HKCU"]
fn manual_hkcu_run_and_user_task() {
    use std::process::Command;
    const RUN: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "QuadoProvaRiferimenti";

    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = Command::new("reg").args(["delete", RUN, "/v", NAME, "/f"]).status();
            let _ = Command::new("schtasks").args(["/delete", "/tn", NAME, "/f"]).status();
        }
    }

    let dir = TempDir::new("riferimenti-manuale");
    let version = dir.0.join("CapCut").join("Apps").join("9.2.0.1");
    let exe = version.join("CapCut.exe");
    assert!(!exe.exists());
    let _cleanup = Cleanup;
    let quoted = format!("\"{}\"", exe.display());
    let ok = Command::new("reg").args(["add", RUN, "/v", NAME, "/t", "REG_SZ", "/d", &format!("{quoted} --prova"), "/f"]).status().expect("reg");
    assert!(ok.success(), "reg add");
    // Una volta sola, a un'ora già passata: l'attività non parte.
    let ok = Command::new("schtasks").args(["/create", "/tn", NAME, "/tr", &quoted, "/sc", "ONCE", "/st", "00:00", "/f"]).status().expect("schtasks");
    assert!(ok.success(), "schtasks /create");

    let report = references::find_live(&[version.to_string_lossy().into_owned()]);
    println!("{report:#?}");
    let exe = exe.to_string_lossy().into_owned();
    assert!(report.found.contains(&Reference { kind: ReferenceKind::RunUser, location: format!(r"{RUN}\{NAME}"), target: exe.clone() }));
    assert!(report.found.contains(&Reference { kind: ReferenceKind::ScheduledTask, location: format!(r"\{NAME}"), target: exe }));
    assert!(!version.exists());
}
