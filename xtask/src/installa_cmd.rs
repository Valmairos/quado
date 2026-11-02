// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask installa [--dest DIR] [--menu DIR] [--senza-build] [--rimuovi-rilievo [--vecchia DIR]]`:
//! build con il profilo `dist` (release con LTO "fat" e una sola unità di codegen,
//! `target\dist\quado.exe`, con i flag di `cargo xtask rustflags-dist`), controllo delle importazioni e
//! dei percorsi incorporati (se uno fallisce, niente si installa), copia di
//! `quado.exe` in `%LOCALAPPDATA%\Programs\Quado` e collegamento `Quado.lnk` nel menu Start dell'utente
//! (`%APPDATA%\Microsoft\Windows\Start Menu\Programs`), con `IShellLinkW` e `IPersistFile`. Il collegamento
//! prende l'icona dall'eseguibile (la prima delle sue risorse, `quado.ico`); si rilegge dopo il salvataggio
//! e la destinazione deve coincidere con l'eseguibile copiato.
//!
//! `--rimuovi-rilievo`, dopo l'installazione riuscita, toglie la versione che si chiamava Rilievo:
//! `%LOCALAPPDATA%\Programs\Rilievo` (o `--vecchia DIR`) e `Rilievo.lnk` nella cartella del menu. Solo
//! quelli, e solo se sono davvero di Rilievo (`check_old`): altrimenti niente si toglie. I dati in
//! `%APPDATA%\Rilievo` restano: si cancellano da Quado, in Impostazioni › Dati e privacy.
//!
//! `--dest`, `--menu` e `--vecchia` cambiano le cartelle (prove in cartelle temporanee); `--senza-build`
//! usa l'eseguibile già compilato. Nessun privilegio: tutto resta nel profilo dell'utente.

// Chiamate COM della shell di Windows (collegamento): ogni blocco ha il suo invariante.
#![allow(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, STGM_READ};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::core::{HSTRING, Interface};

use crate::{dist_cmd, importazioni_cmd};

/// Nome del collegamento nel menu Start.
pub const LINK_NAME: &str = "Quado.lnk";

/// Eseguibile del profilo `dist`, quello che si installa.
pub const DIST_EXE: &str = r"target\dist\quado.exe";

/// Descrizione del collegamento (suggerimento del menu Start): il nome e la riga che lo segue nella
/// testata del README. Niente «sola lettura»: lo è la scansione, mentre Quado sposta file nel Cestino.
pub const DESCRIPTION: &str = "Quado, disk space analyzer for Windows";

/// Versione precedente, che si chiamava Rilievo: cartella in `%LOCALAPPDATA%\Programs`, eseguibile e
/// collegamento nel menu Start.
pub const OLD_DIR_NAME: &str = "Rilievo";
pub const OLD_EXE: &str = "rilievo.exe";
pub const OLD_LINK_NAME: &str = "Rilievo.lnk";

/// Testo che c'è negli eseguibili di Rilievo e non in quelli di Quado: il prefisso della pipe dell'helper.
const OLD_MARKER: &[u8] = br"\\.\pipe\rilievo-";

const USAGE: &str = "uso: cargo xtask installa [--dest DIR] [--menu DIR] [--senza-build] [--rimuovi-rilievo [--vecchia DIR]]";

struct Options {
    dest: Option<PathBuf>,
    menu: Option<PathBuf>,
    build: bool,
    remove_old: bool,
    old: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options { dest: None, menu: None, build: true, remove_old: false, old: None };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().map(PathBuf::from).ok_or_else(|| format!("{a}: manca il valore"));
        match a.as_str() {
            "--dest" => o.dest = Some(value()?),
            "--menu" => o.menu = Some(value()?),
            "--senza-build" => o.build = false,
            "--rimuovi-rilievo" => o.remove_old = true,
            "--vecchia" => o.old = Some(value()?),
            other => return Err(format!("argomento sconosciuto: {other}\n{USAGE}")),
        }
    }
    if o.old.is_some() && !o.remove_old {
        return Err(format!("--vecchia serve solo con --rimuovi-rilievo\n{USAGE}"));
    }
    Ok(o)
}

fn env_dir(var: &str) -> Result<PathBuf, String> {
    std::env::var_os(var).map(PathBuf::from).ok_or_else(|| format!("%{var}% non definita"))
}

/// Cartella dell'eseguibile installato.
pub fn default_dest() -> Result<PathBuf, String> {
    Ok(env_dir("LOCALAPPDATA")?.join(r"Programs\Quado"))
}

/// Cartella dell'eseguibile della versione precedente.
pub fn default_old_dest() -> Result<PathBuf, String> {
    Ok(env_dir("LOCALAPPDATA")?.join("Programs").join(OLD_DIR_NAME))
}

/// Programmi del menu Start dell'utente.
pub fn default_menu() -> Result<PathBuf, String> {
    Ok(env_dir("APPDATA")?.join(r"Microsoft\Windows\Start Menu\Programs"))
}

/// Cartelle da cui l'helper rifiuta di partire: Download e cartelle temporanee.
fn helper_refuses(dir: &Path) -> bool {
    let d = dir.to_string_lossy().to_lowercase();
    let temp = std::env::temp_dir().to_string_lossy().to_lowercase();
    d.contains(r"\downloads") || d.starts_with(temp.trim_end_matches('\\'))
}

pub fn run(args: &[String]) -> Result<(), String> {
    let o = parse(args)?;
    if o.build {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        // Flag di `.cargo/config.toml` più il remap delle cartelle di cargo, del repository e di rustup
        // (`cargo xtask rustflags-dist`): nell'eseguibile nessun percorso di questa macchina.
        let flags = dist_cmd::rustflags()?;
        let s = Command::new(&cargo)
            .args(["build", "--profile", "dist", "--quiet", "-p", "quado"])
            .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\x1f"))
            .status()
            .map_err(|e| e.to_string())?;
        if !s.success() {
            return Err("build di quado fallita: niente installato".into());
        }
    }
    let built = PathBuf::from(DIST_EXE);
    if !built.is_file() {
        return Err(format!("{} assente: compila prima (senza --senza-build)", built.display()));
    }
    importazioni_cmd::verify(&built).map_err(|e| format!("{e}\nniente installato"))?;
    let bytes = std::fs::read(&built).map_err(|e| format!("{}: {e}", built.display()))?;
    let paths = dist_cmd::embedded_user_paths(&bytes);
    if !paths.is_empty() {
        return Err(format!("percorsi di un profilo utente nell'eseguibile (manca il remap?): {paths:#?}\nniente installato"));
    }
    println!("percorsi di un profilo utente nell'eseguibile: nessuno");
    // L'eseguibile installato non ha le opzioni di misura e di prova (feature `misure` spenta).
    let dev = dist_cmd::embedded_dev_options(&bytes);
    if !dev.is_empty() {
        return Err(format!("opzioni di misura nell'eseguibile (compilato con la feature misure?): {dev:?}\nniente installato"));
    }
    println!("opzioni di misura e di prova nell'eseguibile: nessuna");
    println!();

    let dest_dir = match o.dest {
        Some(d) => d,
        None => default_dest()?,
    };
    let menu_dir = match o.menu {
        Some(d) => d,
        None => default_menu()?,
    };
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("{}: {e}", dest_dir.display()))?;
    let dest_dir = std::fs::canonicalize(&dest_dir).map(|p| strip_verbatim(&p)).map_err(|e| e.to_string())?;
    let exe = dest_dir.join("quado.exe");
    std::fs::copy(&built, &exe).map_err(|e| format!("copia in {}: {e} (Quado è aperto? chiudilo e riprova)", exe.display()))?;
    let same = std::fs::read(&built).ok() == std::fs::read(&exe).ok();
    if !same {
        return Err(format!("la copia in {} non coincide con l'eseguibile compilato", exe.display()));
    }
    println!("eseguibile: {}", exe.display());

    std::fs::create_dir_all(&menu_dir).map_err(|e| format!("{}: {e}", menu_dir.display()))?;
    let menu_dir = std::fs::canonicalize(&menu_dir).map(|p| strip_verbatim(&p)).map_err(|e| e.to_string())?;
    let lnk = menu_dir.join(LINK_NAME);
    let want = Link { target: exe.clone(), workdir: dest_dir.clone(), description: DESCRIPTION.to_owned(), icon: (exe.clone(), 0) };
    create_link(&lnk, &want).map_err(|e| format!("collegamento {}: {e}", lnk.display()))?;
    let got = read_link(&lnk).map_err(|e| format!("rilettura di {}: {e}", lnk.display()))?;
    if !same_path(&got.target, &want.target) || !same_path(&got.workdir, &want.workdir) || got.description != want.description {
        return Err(format!("il collegamento riletto non coincide: {got:?}"));
    }
    println!("collegamento: {} (riletto: destinazione {}, cartella di lavoro {})", lnk.display(), got.target.display(), got.workdir.display());
    if helper_refuses(&dest_dir) {
        println!("attenzione: da questa cartella l'helper elevato non parte (Download o cartella temporanea)");
    }
    println!("\nQuado installato. Per toglierlo:");
    println!("1. in Quado, Impostazioni › Dati e privacy: cancella i dati (%APPDATA%\\Quado);");
    println!("2. cancella la cartella {};", dest_dir.display());
    println!("3. cancella il collegamento {}.", lnk.display());
    println!("Aggiornare: ricompila dal sorgente e ripeti `cargo xtask installa`.");

    // La versione precedente, solo dopo che la nuova è installata: chi la toglie ha sempre un'app che parte.
    let old_dir = match o.old {
        Some(d) => d,
        None if o.remove_old => default_old_dest()?,
        None => default_old_dest().unwrap_or_default(),
    };
    if o.remove_old {
        println!();
        let old = check_old(&old_dir, &menu_dir).map_err(|e| format!("Rilievo: {e}\nQuado è installato; niente di Rilievo è stato tolto"))?;
        let removed = remove_old(&old).map_err(|e| format!("Rilievo: {e}\nchiudi Rilievo, se è aperto, e riprova con --senza-build --rimuovi-rilievo"))?;
        if removed.is_empty() {
            println!("Rilievo: niente da togliere ({} e {} non ci sono)", old_dir.display(), menu_dir.join(OLD_LINK_NAME).display());
        }
        for r in removed {
            println!("Rilievo: tolto {}", r.display());
        }
        println!("I dati di Rilievo (%APPDATA%\\Rilievo) restano: si cancellano in Quado, Impostazioni › Dati e privacy.");
    } else if old_dir.join(OLD_EXE).is_file() || menu_dir.join(OLD_LINK_NAME).is_file() {
        println!("\nC'è ancora Rilievo, la versione precedente: per toglierlo ripeti con --senza-build --rimuovi-rilievo.");
    }
    Ok(())
}

/// Cosa togliere della versione precedente, dopo i controlli di [`check_old`]: `None` dove non c'è niente.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OldInstall {
    pub link: Option<PathBuf>,
    pub exe: Option<PathBuf>,
    pub dir: Option<PathBuf>,
}

/// Controlla che cartella e collegamento siano davvero di Rilievo: la cartella si chiama Rilievo e contiene
/// solo `rilievo.exe`, un eseguibile (firma MZ) con il prefisso della pipe dell'helper di Rilievo; il
/// collegamento `Rilievo.lnk` in `menu` punta a quell'eseguibile (o, se la cartella non c'è più, a un
/// `rilievo.exe` in una cartella Rilievo). Se qualcosa non torna, l'errore lo dice e niente si toglie.
pub fn check_old(dir: &Path, menu: &Path) -> Result<OldInstall, String> {
    let mut out = OldInstall { link: None, exe: None, dir: None };
    // Percorso completo, come quello che il collegamento riletto restituisce.
    let dir = if dir.exists() { std::fs::canonicalize(dir).map(|p| strip_verbatim(&p)).map_err(|e| format!("{}: {e}", dir.display()))? } else { dir.to_owned() };
    let dir = dir.as_path();
    let exe = dir.join(OLD_EXE);
    if dir.exists() {
        if dir.file_name().is_none_or(|n| !n.eq_ignore_ascii_case(OLD_DIR_NAME)) {
            return Err(format!("{} non si chiama {OLD_DIR_NAME}", dir.display()));
        }
        let names: Vec<String> = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        let others: Vec<&String> = names.iter().filter(|n| !n.eq_ignore_ascii_case(OLD_EXE)).collect();
        if !others.is_empty() {
            return Err(format!("{} contiene anche {others:?}: non è solo l'installazione di Rilievo", dir.display()));
        }
        if names.is_empty() {
            out.dir = Some(dir.to_owned());
        } else {
            let bytes = std::fs::read(&exe).map_err(|e| format!("{}: {e}", exe.display()))?;
            if !bytes.starts_with(b"MZ") || !bytes.windows(OLD_MARKER.len()).any(|w| w == OLD_MARKER) {
                return Err(format!("{} non è un eseguibile di Rilievo", exe.display()));
            }
            out.exe = Some(exe.clone());
            out.dir = Some(dir.to_owned());
        }
    }
    let lnk = menu.join(OLD_LINK_NAME);
    if lnk.is_file() {
        let got = read_link(&lnk).map_err(|e| format!("rilettura di {}: {e}", lnk.display()))?;
        let ours = if dir.exists() {
            same_path(&got.target, &exe)
        } else {
            got.target.file_name().is_some_and(|n| n.eq_ignore_ascii_case(OLD_EXE))
                && got.target.parent().and_then(Path::file_name).is_some_and(|n| n.eq_ignore_ascii_case(OLD_DIR_NAME))
        };
        if !ours {
            return Err(format!("{} punta a {}, non all'eseguibile di Rilievo", lnk.display(), got.target.display()));
        }
        out.link = Some(lnk);
    }
    Ok(out)
}

/// Toglie ciò che [`check_old`] ha controllato: collegamento, eseguibile, poi la cartella (vuota).
/// Restituisce i percorsi tolti.
pub fn remove_old(old: &OldInstall) -> Result<Vec<PathBuf>, String> {
    let mut removed = Vec::new();
    for p in [&old.link, &old.exe].into_iter().flatten() {
        std::fs::remove_file(p).map_err(|e| format!("{}: {e}", p.display()))?;
        removed.push(p.clone());
    }
    if let Some(d) = &old.dir {
        std::fs::remove_dir(d).map_err(|e| format!("{}: {e}", d.display()))?;
        removed.push(d.clone());
    }
    Ok(removed)
}

/// Toglie il prefisso `\\?\` di `canonicalize`, che la shell e il menu Start non usano.
fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s).to_owned())
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().trim_end_matches('\\').eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches('\\'))
}

/// Campi del collegamento che si scrivono e si rileggono.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub target: PathBuf,
    pub workdir: PathBuf,
    pub description: String,
    pub icon: (PathBuf, i32),
}

/// COM sul thread corrente, finché il valore vive.
struct Com(bool);

impl Com {
    fn init() -> Self {
        // SAFETY: inizializzazione COM del thread corrente; il drop la bilancia solo se è riuscita.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Com(hr.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: bilancia il CoInitializeEx riuscito di `init`, sullo stesso thread.
            unsafe { CoUninitialize() };
        }
    }
}

fn hs(p: &Path) -> HSTRING {
    HSTRING::from(p.as_os_str())
}

/// Scrive il collegamento `lnk` (lo sovrascrive se c'è).
pub fn create_link(lnk: &Path, l: &Link) -> Result<(), String> {
    let _com = Com::init();
    // SAFETY: COM è inizializzato su questo thread e gli oggetti si usano solo qui; le stringhe HSTRING
    // vivono per tutta la durata delle chiamate che le ricevono.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| format!("ShellLink: {e}"))?;
        link.SetPath(&hs(&l.target)).map_err(|e| format!("SetPath: {e}"))?;
        link.SetWorkingDirectory(&hs(&l.workdir)).map_err(|e| format!("SetWorkingDirectory: {e}"))?;
        link.SetDescription(&HSTRING::from(l.description.as_str())).map_err(|e| format!("SetDescription: {e}"))?;
        link.SetIconLocation(&hs(&l.icon.0), l.icon.1).map_err(|e| format!("SetIconLocation: {e}"))?;
        let file: IPersistFile = link.cast().map_err(|e| format!("IPersistFile: {e}"))?;
        file.Save(&hs(lnk), true).map_err(|e| format!("Save: {e}"))?;
    }
    Ok(())
}

/// Rilegge un collegamento: destinazione (`IShellLinkW::GetPath`), cartella di lavoro, descrizione, icona.
pub fn read_link(lnk: &Path) -> Result<Link, String> {
    let _com = Com::init();
    let text = |buf: &[u16]| String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(buf.len())]);
    // SAFETY: COM è inizializzato su questo thread; i buffer sono locali e passati con la loro lunghezza;
    // `GetPath` accetta un WIN32_FIND_DATAW nullo; `GetIconLocation` scrive l'indice in una variabile locale.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
        let file: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        file.Load(&hs(lnk), STGM_READ).map_err(|e| e.to_string())?;
        let mut target = [0u16; 1024];
        link.GetPath(&mut target, std::ptr::null_mut(), 0).map_err(|e| e.to_string())?;
        let mut workdir = [0u16; 1024];
        link.GetWorkingDirectory(&mut workdir).map_err(|e| e.to_string())?;
        let mut description = [0u16; 1024];
        link.GetDescription(&mut description).map_err(|e| e.to_string())?;
        let mut icon = [0u16; 1024];
        let mut index = 0i32;
        link.GetIconLocation(&mut icon, &mut index).map_err(|e| e.to_string())?;
        Ok(Link { target: PathBuf::from(text(&target)), workdir: PathBuf::from(text(&workdir)), description: text(&description), icon: (PathBuf::from(text(&icon)), index) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_written_in_a_temporary_folder_reads_back_the_same() {
        // Solo una cartella temporanea: niente menu Start, niente copie. La destinazione è un eseguibile
        // di sistema, che non si apre e non si modifica.
        let dir = std::env::temp_dir().join(format!("quado-collegamento-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let system = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32");
        let target = system.join("notepad.exe");
        let want = Link { target: target.clone(), workdir: system.clone(), description: DESCRIPTION.into(), icon: (target.clone(), 0) };
        let lnk = dir.join(LINK_NAME);
        create_link(&lnk, &want).unwrap();
        let got = read_link(&lnk).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(same_path(&got.target, &want.target), "{got:?}");
        assert!(same_path(&got.workdir, &want.workdir), "{got:?}");
        assert_eq!(got.description, want.description);
        assert!(same_path(&got.icon.0, &want.icon.0) && got.icon.1 == 0, "{got:?}");
    }

    #[test]
    fn the_description_does_not_call_quado_read_only() {
        let d = DESCRIPTION.to_lowercase();
        for word in ["read-only", "read only", "sola lettura"] {
            assert!(!d.contains(word), "{DESCRIPTION}");
        }
    }

    #[test]
    fn the_options_and_the_helper_folders() {
        let o = parse(&["--dest".into(), r"C:\x".into(), "--senza-build".into()]).unwrap();
        assert_eq!((o.dest, o.menu, o.build, o.remove_old), (Some(PathBuf::from(r"C:\x")), None, false, false));
        let o = parse(&["--rimuovi-rilievo".into(), "--vecchia".into(), r"C:\y\Rilievo".into()]).unwrap();
        assert_eq!((o.remove_old, o.old), (true, Some(PathBuf::from(r"C:\y\Rilievo"))));
        assert!(parse(&["--vecchia".into(), r"C:\y\Rilievo".into()]).is_err(), "--vecchia senza --rimuovi-rilievo");
        assert!(parse(&["--dest".into()]).is_err());
        assert!(parse(&["--boh".into()]).is_err());
        assert!(helper_refuses(&std::env::temp_dir().join("a")));
        assert!(helper_refuses(Path::new(r"C:\Users\u\Downloads\Quado")));
        assert!(!helper_refuses(Path::new(r"C:\Users\u\AppData\Local\Programs\Quado")));
        assert!(default_dest().unwrap().ends_with(r"Programs\Quado"));
        assert!(default_old_dest().unwrap().ends_with(r"Programs\Rilievo"));
    }

    /// Una cartella sotto `target\tmp` del repository, tolta alla fine: le prove dell'installazione non
    /// toccano `%LOCALAPPDATA%` né il menu Start veri.
    struct Prova(PathBuf);

    impl Prova {
        fn new(name: &str) -> Self {
            let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(r"..\target\tmp").join(format!("quado-installa-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(p.join("menu")).unwrap();
            Prova(std::fs::canonicalize(&p).map(|c| strip_verbatim(&c)).unwrap())
        }

        fn menu(&self) -> PathBuf {
            self.0.join("menu")
        }

        /// Un'installazione di Rilievo: la cartella con `rilievo.exe` (`contenuto`) e `Rilievo.lnk` nel menu, che
        /// punta all'eseguibile. Restituisce la cartella.
        fn rilievo(&self, contenuto: &[u8]) -> PathBuf {
            let dir = self.0.join(OLD_DIR_NAME);
            std::fs::create_dir_all(&dir).unwrap();
            let exe = dir.join(OLD_EXE);
            std::fs::write(&exe, contenuto).unwrap();
            let link = Link { target: exe.clone(), workdir: dir.clone(), description: "Rilievo".into(), icon: (exe, 0) };
            create_link(&self.menu().join(OLD_LINK_NAME), &link).unwrap();
            dir
        }
    }

    impl Drop for Prova {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// I byte di un eseguibile di Rilievo, quanto basta per i controlli: firma MZ e prefisso della pipe. Più
    /// corto di un'intestazione DOS: con un'intestazione senza PE valido `IPersistFile::Save` del
    /// collegamento che lo punta fallisce (E_FAIL).
    fn rilievo_exe() -> Vec<u8> {
        [b"MZ ... ".as_slice(), OLD_MARKER, b"0123 ..."].concat()
    }

    #[test]
    fn rilievo_is_removed_only_when_folder_exe_and_link_are_its_own() {
        let p = Prova::new("vera");
        let dir = p.rilievo(&rilievo_exe());
        // Un altro collegamento del menu resta.
        std::fs::write(p.menu().join("Altro.lnk"), b"x").unwrap();
        let old = check_old(&dir, &p.menu()).unwrap();
        assert_eq!(old, OldInstall { link: Some(p.menu().join(OLD_LINK_NAME)), exe: Some(dir.join(OLD_EXE)), dir: Some(dir.clone()) });
        assert_eq!(remove_old(&old).unwrap().len(), 3);
        assert!(!dir.exists() && !p.menu().join(OLD_LINK_NAME).exists());
        assert!(p.menu().join("Altro.lnk").exists());
        // Già tolta: niente da fare.
        assert_eq!(check_old(&dir, &p.menu()).unwrap(), OldInstall { link: None, exe: None, dir: None });
    }

    #[test]
    fn nothing_is_removed_when_something_is_not_rilievo() {
        // Un file in più nella cartella.
        let p = Prova::new("in-piu");
        let dir = p.rilievo(&rilievo_exe());
        std::fs::write(dir.join("appunti.txt"), b"miei").unwrap();
        assert!(check_old(&dir, &p.menu()).unwrap_err().contains("appunti.txt"));
        // Un eseguibile che non è di Rilievo (per esempio quello di Quado, con la pipe nuova).
        let p = Prova::new("altro-exe");
        let dir = p.rilievo(&[b"MZ".as_slice(), br"\\.\pipe\quado-"].concat());
        assert!(check_old(&dir, &p.menu()).unwrap_err().contains("non è un eseguibile di Rilievo"));
        // Una cartella con un altro nome.
        let p = Prova::new("nome");
        let other = p.0.join("Programmi");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join(OLD_EXE), rilievo_exe()).unwrap();
        assert!(check_old(&other, &p.menu()).is_err());
        // Un collegamento Rilievo.lnk che punta altrove.
        let p = Prova::new("collegamento");
        let dir = p.rilievo(&rilievo_exe());
        let elsewhere = p.0.join("altro.exe");
        std::fs::write(&elsewhere, b"MZ").unwrap();
        create_link(&p.menu().join(OLD_LINK_NAME), &Link { target: elsewhere.clone(), workdir: p.0.clone(), description: String::new(), icon: (elsewhere, 0) }).unwrap();
        assert!(check_old(&dir, &p.menu()).unwrap_err().contains("non all'eseguibile di Rilievo"));
        // In tutti i casi la cartella e l'eseguibile restano.
        assert!(dir.join(OLD_EXE).exists());
    }

    #[test]
    fn a_link_left_without_the_folder_is_removed() {
        let p = Prova::new("solo-collegamento");
        let dir = p.rilievo(&rilievo_exe());
        std::fs::remove_file(dir.join(OLD_EXE)).unwrap();
        std::fs::remove_dir(&dir).unwrap();
        let old = check_old(&dir, &p.menu()).unwrap();
        assert_eq!(old, OldInstall { link: Some(p.menu().join(OLD_LINK_NAME)), exe: None, dir: None });
        assert_eq!(remove_old(&old).unwrap(), vec![p.menu().join(OLD_LINK_NAME)]);
    }
}
