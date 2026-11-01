// SPDX-License-Identifier: GPL-3.0-or-later
//! Interventi nell'interfaccia (M5, 03 F-AC e F-VE): le azioni di `quado-actions` dietro un insieme
//! iniettabile ([`Actions`]), le letture di configurazione e dei riferimenti da `quado-kb` per
//! `WinProbe` ([`KbConfig`]), l'origine di un intervento ([`Origin`]) e la riscansione dei sottoalberi
//! toccati (F-VE-4) da applicare al modello con `data::apply_rescans`.
//!
//! Nei test `Actions::fake` usa `FakeProbe` e `FakeMover`, e riscansiona l'albero simulato: niente tocca
//! il disco.

use std::sync::Arc;

use quado_actions::execute::Mover;
use quado_actions::fake::{FakeMover, FakeProbe};
use quado_actions::launch::{self, Launch};
use quado_actions::mover::ShellMover;
use quado_actions::probe::{Probe, SystemConfig};
use quado_actions::procedures;
use quado_actions::shell::ShellError;
use quado_actions::win_probe::WinProbe;
use quado_kb::extract::{self, ExtractError};
use quado_model::intervention::{ProtectedRoot, ReferenceReport};
use quado_model::{Flags, Links, NodeData, Tree};
use quado_scan_win::{Options, Problem, Progress, ScanResult, Stats};

use super::App;
use super::data::{Loaded, Task};

/// Crea il mover nel thread che esegue il piano (COM a thread singolo), con la finestra proprietaria.
pub type MoverFactory = Arc<dyn Fn(Option<isize>) -> Box<dyn Mover> + Send + Sync>;
/// "Svuota il Cestino" del volume (`C:\`), con la finestra proprietaria del dialogo di Windows.
pub type EmptyBin = Arc<dyn Fn(&str, Option<isize>) -> Result<(), ShellError> + Send + Sync>;
/// Riscansione in sola lettura di una cartella (F-VE-4): `quado_scan_win::scan` nell'app, l'albero di
/// `FakeProbe` nei test. Errore: codice di Windows (2 o 3 se la cartella non c'è più).
pub type RescanFn = Arc<dyn Fn(&str) -> Result<Box<ScanResult>, u32> + Send + Sync>;

/// Le azioni dell'interfaccia: vere nell'app, simulate nei test.
#[derive(Clone)]
pub struct Actions {
    pub probe: Arc<dyn Probe>,
    pub mover: MoverFactory,
    pub empty_bin: EmptyBin,
    /// Apre una pagina tipizzata (Cestino, Impostazioni); `launch::open` nell'app.
    pub launcher: fn(&Launch) -> Result<(), ShellError>,
    /// Riscansione dei sottoalberi toccati (`spawn_rescan_with`).
    pub rescan: RescanFn,
}

impl std::fmt::Debug for Actions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Actions")
    }
}

fn launch_nothing(_: &Launch) -> Result<(), ShellError> {
    Ok(())
}

impl Actions {
    /// Le azioni vere: `WinProbe` con le letture di `quado-kb`, `ShellMover`, il dialogo di Windows per
    /// svuotare il Cestino, il lanciatore tipizzato.
    pub fn live() -> Self {
        Self {
            probe: Arc::new(WinProbe::new(Box::new(KbConfig))),
            mover: Arc::new(|owner| Box::new(ShellMover::new(owner)) as Box<dyn Mover>),
            empty_bin: Arc::new(procedures::empty_recycle_bin),
            launcher: launch::open,
            rescan: Arc::new(live_scan),
        }
    }

    /// Azioni simulate su `probe`: lo spostamento toglie dalle letture e aggiunge al Cestino simulato,
    /// lo svuotamento lo libera, il lanciatore non apre niente, la riscansione legge l'albero simulato.
    pub fn fake(probe: &FakeProbe) -> Self {
        let p = probe.clone();
        let m = probe.clone();
        let r = probe.clone();
        Self {
            probe: Arc::new(probe.clone()),
            mover: Arc::new(move |_| Box::new(FakeMover::new(&m)) as Box<dyn Mover>),
            empty_bin: Arc::new(move |root, _| {
                p.empty_bin(root);
                Ok(())
            }),
            launcher: launch_nothing,
            rescan: Arc::new(move |path| fake_scan(&r, path)),
        }
    }

    /// Nessun volume e nessun file: per i test che non arrivano agli interventi.
    pub fn inert() -> Self {
        Self::fake(&FakeProbe::new())
    }
}

/// Letture di `quado-kb` per `WinProbe`, fatte al momento della simulazione: impostazioni del Cestino
/// (`BitBucket`) e di Sensore memoria dal registro in sola lettura, riferimenti (F-AC-9).
pub struct KbConfig;

fn extract_code(e: ExtractError) -> u32 {
    match e {
        ExtractError::Io(c) => c,
        ExtractError::NotPresent => 2,
        // Formato inatteso: ERROR_INVALID_DATA.
        _ => 13,
    }
}

impl SystemConfig for KbConfig {
    fn bin_settings(&self, root: &str) -> Result<(Option<u64>, bool), u32> {
        let guid = quado_kb::win::volume_guid(root).ok_or(2u32)?;
        match extract::bin_settings(&quado_kb::win::LiveRegistry, &guid) {
            Ok(b) => Ok((b.max_capacity_mib.map(|m| u64::from(m) << 20), b.nuke_on_delete)),
            Err(ExtractError::NotPresent) => Ok((None, false)),
            Err(e) => Err(extract_code(e)),
        }
    }

    fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
        let s = extract::storage_sense(&quado_kb::win::LiveRegistry).map_err(extract_code)?;
        Ok(if s.enabled { s.recycle_days } else { None })
    }

    fn references(&self, paths: &[String]) -> ReferenceReport {
        quado_kb::references::find_live(paths)
    }

    /// Dropbox (`info.json`) e Google Drive per desktop (`DefaultMountPoint`), in sola lettura.
    fn sync_roots(&self) -> Vec<quado_actions::sync::SyncRoot> {
        use quado_actions::sync::{SyncKind, SyncRoot};
        let kf = quado_kb::win::known_folders();
        let mut out: Vec<SyncRoot> = quado_kb::sync::dropbox_roots(&quado_kb::win::LiveFiles, kf.local.as_deref(), kf.roaming.as_deref())
            .into_iter()
            .map(|path| SyncRoot { path, provider: "Dropbox".into(), kind: SyncKind::Cloud })
            .collect();
        if let Some(path) = quado_kb::sync::google_drive_mount(&quado_kb::win::LiveRegistry, &|v| std::env::var(v).ok()) {
            out.push(SyncRoot { path, provider: "Google Drive".into(), kind: SyncKind::Cloud });
        }
        out
    }
}

/// Da dove parte un intervento.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Elemento riconosciuto da una regola: indice in `Classification::matches`.
    Rule(usize),
    /// Voci della lente Download: indici in `LensView::findings` della vista corrente.
    Lens(Vec<usize>),
    /// Programma della vista Programmi: indice in `Programs::items` (disinstallazione, F-AP-5).
    Program(usize),
}

/// Cosa chiede un pulsante del blocco "Cosa puoi fare" (ispettore, lente Download).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActRequest {
    /// "Sposta nel Cestino…": simulazione e dialogo di conferma (F-AC-2).
    PrepareRecycle,
    /// Apre la pagina della procedura (URI tipizzato), dopo la misura di partenza (F-VE-1).
    OpenProcedure,
    /// "Ho eseguito la procedura": nuove misure e riscansione (F-VE-2).
    ProcedureDone,
    /// Misura di partenza senza aprire niente, per le procedure dentro un'app che Quado non apre (cache
    /// dei browser, di CapCut, dispositivi virtuali di Android Studio): F-VE-1.
    MeasureStart,
    /// "Copia" di un comando ufficiale (azione comando): copia negli appunti e misura di partenza. Quado
    /// non lo esegue (F-AC-1). `alternative`: `None` per il metodo, `Some(i)` per l'alternativa `i`.
    CopyCommand { alternative: Option<usize> },
    /// "Svuota il Cestino…" dal record del Cestino: misura di partenza, dialogo di Windows, nuove misure.
    EmptyBin,
}

/// Esegue la richiesta di un pulsante: spostamento (`intervention_view`) o procedura (`procedure_flow`).
pub fn dispatch(app: &mut App, req: ActRequest, origin: Origin) {
    match req {
        ActRequest::PrepareRecycle => super::intervention_view::open(app, origin),
        ActRequest::OpenProcedure => super::procedure_flow::open(app, origin),
        ActRequest::ProcedureDone => super::procedure_flow::done(app, origin),
        ActRequest::MeasureStart => super::procedure_flow::measure_start(app, origin),
        ActRequest::CopyCommand { alternative } => super::procedure_flow::copy_command(app, origin, alternative),
        ActRequest::EmptyBin => super::procedure_flow::empty_bin(app, origin),
    }
}

/// Radici protette del volume caricato (F-AC-4), dalle prove e dalla vista Programmi.
pub fn protected_roots(d: &Loaded) -> Vec<ProtectedRoot> {
    quado_kb::protection::protected_roots(&d.evidence, &d.programs)
}

// ---------------------------------------------------------------------------------------------
// Percorsi e riscansione (F-VE-2, F-VE-4)

/// Chiave di confronto: `/` come `\`, senza `\` finale, minuscole.
fn key(p: &str) -> String {
    let p = p.strip_prefix(r"\\?\").unwrap_or(p).replace('/', "\\");
    p.trim_end_matches('\\').to_lowercase()
}

/// Nodo del modello con questo percorso assoluto, senza distinzione di maiuscole; `None` se non c'è.
pub fn find_node(tree: &Tree, path: &str) -> Option<u32> {
    let root = key(&tree.path_lossy(Tree::ROOT));
    let p = key(path);
    if p == root {
        return Some(Tree::ROOT);
    }
    let rest = p.strip_prefix(&root)?.strip_prefix('\\')?;
    let mut n = Tree::ROOT;
    for comp in rest.split('\\').filter(|c| !c.is_empty()) {
        n = tree.children(n).find(|&c| !tree.flags(c).has(Flags::DETACHED) && tree.name_lossy(c).to_lowercase() == comp)?;
    }
    Some(n)
}

/// Cartella del Cestino dell'utente sul volume (`C:\$Recycle.Bin\<SID>`).
pub fn user_bin_dir(volume_root: &str, user_sid: &str) -> String {
    format!(r"{}\$Recycle.Bin\{user_sid}", volume_root.trim_end_matches('\\'))
}

/// Cartelle da riscansionare dopo un intervento: la cartella che contiene ciascuno di `items` (che può
/// non esistere più) e le cartelle `extra` (per esempio il Cestino dell'utente), senza doppioni e senza
/// quelle che stanno dentro un'altra della lista.
pub fn rescan_dirs(items: &[String], extra: &[String]) -> Vec<String> {
    let mut dirs: Vec<String> = items
        .iter()
        .filter_map(|p| {
            let t = p.trim_end_matches('\\');
            t.rfind('\\').map(|k| if k <= 2 { t[..=k].to_owned() } else { t[..k].to_owned() })
        })
        .chain(extra.iter().cloned())
        .collect();
    dirs.sort_by_key(|d| key(d));
    dirs.dedup_by_key(|d| key(d));
    let keys: Vec<String> = dirs.iter().map(|d| key(d)).collect();
    dirs.into_iter()
        .zip(&keys)
        .filter(|(_, k)| !keys.iter().any(|o| o != *k && k.starts_with(o.as_str()) && k.as_bytes().get(o.len()) == Some(&b'\\')))
        .map(|(d, _)| d)
        .collect()
}

/// Una cartella riscansionata, o il codice di Windows se non si è potuta leggere.
pub struct Rescanned {
    pub path: String,
    pub result: Result<Box<ScanResult>, u32>,
}

/// Riscansiona le cartelle in un thread, in sola lettura e sul disco vero. Il modello si aggiorna poi con
/// `data::apply_rescans`, che ritrova i nodi per percorso. Con le azioni iniettate (test) si usa
/// [`spawn_rescan_with`].
pub fn spawn_rescan(dirs: Vec<String>) -> Task<Vec<Rescanned>> {
    let live: RescanFn = Arc::new(live_scan);
    spawn_rescan_with(&live, dirs)
}

/// Come [`spawn_rescan`], con la riscansione delle azioni (`Actions::rescan`): quella vera nell'app,
/// l'albero simulato nei test.
pub fn spawn_rescan_with(rescan: &RescanFn, dirs: Vec<String>) -> Task<Vec<Rescanned>> {
    let rescan = Arc::clone(rescan);
    Task::spawn(move || dirs.into_iter().map(|path| Rescanned { result: rescan(&path), path }).collect())
}

/// Scansione vera di una cartella, in sola lettura.
fn live_scan(path: &str) -> Result<Box<ScanResult>, u32> {
    quado_scan_win::scan(path, Options::default(), &Progress::default()).map(Box::new).map_err(|e| e.0)
}

/// Riscansione simulata (test): l'albero di `probe` sotto `path`, con gli attributi simulati, nella forma
/// di `quado_scan_win::scan`. Una cartella con l'elenco negato resta vuota, con il suo problema; i
/// collegamenti contano da soli e non si attraversano. Nessun file su disco.
fn fake_scan(probe: &FakeProbe, path: &str) -> Result<Box<ScanResult>, u32> {
    let a = probe.attributes(path)?;
    if !a.directory {
        // ERROR_DIRECTORY: si riscansionano solo cartelle.
        return Err(267);
    }
    let wide = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    let mut tree = Tree::with_root(&wide(path), 0);
    let mut problems = Vec::new();
    let mut stats = Stats::default();
    let mut stack = vec![(Tree::ROOT, path.trim_end_matches('\\').to_owned())];
    while let Some((node, dir)) = stack.pop() {
        let names = match probe.children(&dir) {
            Ok(n) => n,
            Err(code) => {
                problems.push((node, if code == 5 { Problem::AccessDenied } else { Problem::Other(code) }));
                stats.denied += u64::from(code == 5);
                continue;
            }
        };
        for name in names {
            let p = format!("{dir}\\{name}");
            let Ok(a) = probe.attributes(&p) else { continue };
            let mut d = if a.directory {
                NodeData { flags: Flags::DIR, mtime: a.modified, attrs: a.attributes, ..NodeData::default() }
            } else {
                NodeData { logical: a.logical, alloc: a.alloc, mtime: a.modified, attrs: a.attributes, flags: Flags::default() }
            };
            if a.link {
                d.flags |= Flags::REPARSE | Flags::NOT_FOLLOWED;
                stats.not_followed += 1;
            }
            let n = tree.push(node, &wide(&name), d);
            if a.directory && !a.link {
                stats.dirs += 1;
                stack.push((n, p));
            } else if !a.directory {
                stats.files += 1;
            }
        }
    }
    tree.aggregate();
    tree.seal();
    Ok(Box::new(ScanResult { tree, links: Links::default(), problems, stats, volume: None, balance: None, cancelled: false }))
}

/// Byte allocati che `paths` occupano ancora nel modello (dopo la riscansione): 0 per quelli che non ci
/// sono più. Ogni file contato una volta, come nei totali delle cartelle.
pub fn still_present(tree: &Tree, paths: &[String]) -> u64 {
    paths.iter().filter_map(|p| find_node(tree, p)).map(|n| tree.alloc(n)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescan_dirs_are_parents_without_nesting() {
        let items = vec![r"C:\Users\u2\Downloads\a.zip".to_owned(), r"C:\Users\u2\Downloads\sub\b.zip".to_owned(), r"C:\x.bin".to_owned()];
        let bin = vec![r"C:\$Recycle.Bin\S-1-5-21".to_owned()];
        assert_eq!(rescan_dirs(&items, &bin), vec![r"C:\".to_owned()]);
        let items = vec![r"C:\Users\u2\Downloads\a.zip".to_owned(), r"C:\Users\u2\downloads\sub\b.zip".to_owned()];
        assert_eq!(rescan_dirs(&items, &bin), vec![r"C:\$Recycle.Bin\S-1-5-21".to_owned(), r"C:\Users\u2\Downloads".to_owned()]);
    }

    #[test]
    fn fake_rescan_reads_the_simulated_tree() {
        let p = FakeProbe::new().volume("R:\\").file(r"R:\a\b.bin", 10, 4096).file(r"R:\a\c\d.bin", 20, 8192).denied(r"R:\a\negata");
        let a = Actions::fake(&p);
        let r = (a.rescan)(r"R:\a").expect("cartella");
        assert_eq!((r.tree.alloc(Tree::ROOT), r.tree.logical(Tree::ROOT)), (12_288, 30));
        assert_eq!((r.stats.files, r.stats.dirs), (2, 2));
        assert_eq!(r.problems.len(), 1, "la cartella negata");
        assert_eq!((a.rescan)(r"R:\manca").err(), Some(2));
        assert_eq!((a.rescan)(r"R:\a\b.bin").err(), Some(267));
    }

    #[test]
    fn bin_dir_of_the_user() {
        assert_eq!(user_bin_dir(r"R:\", "S-1-5-21-1"), r"R:\$Recycle.Bin\S-1-5-21-1");
    }

    #[test]
    fn nodes_by_path() {
        let (_, r) = super::super::synthetic::sample_c(false);
        let t = &r.tree;
        let any = (1..t.len() as u32).find(|&n| t.depth(n) >= 3 && !t.flags(n).has(Flags::DETACHED)).expect("un nodo profondo");
        let path = t.path_lossy(any);
        assert_eq!(find_node(t, &path), Some(any));
        assert_eq!(find_node(t, &path.to_uppercase()), Some(any));
        assert_eq!(find_node(t, &format!(r"{path}\non-esiste")), None);
        assert_eq!(find_node(t, &t.path_lossy(Tree::ROOT)), Some(Tree::ROOT));
    }
}
