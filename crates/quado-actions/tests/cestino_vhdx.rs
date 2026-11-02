// SPDX-License-Identifier: GPL-3.0-or-later
//! Spostamento nel Cestino con `ShellMover` e Cestino del volume con `WinProbe`, sul VHDX di prova NTFS
//! (`R:`) con il Cestino ridotto a 64 MiB.
//!
//! Solo su `R:\interventi-prove-b`: nessun test sposta niente fuori da lì. Il Cestino di `R:` si svuota
//! all'inizio e alla fine con `quado_testkit::bin::empty_test_volume`, che rifiuta ogni altro volume;
//! capacità e `NukeOnDelete` si ripristinano alla fine di ogni prova. Si eseguono con
//! `cargo test -p quado-actions --test cestino_vhdx -- --ignored --test-threads=1`.
//!
//! Ogni spostamento gira in un thread a parte, sorvegliato: se compare una finestra di Windows la prova
//! la registra e la chiude (chiudere vuol dire "No" o "Annulla": niente viene eliminato), e se
//! l'operazione non finisce entro 60 secondi la prova fallisce invece di bloccarsi. Una sola prova
//! risponde "Sì" alla domanda di Windows, per misurare cosa succede dopo: elimina per sempre un suo file
//! sparse di prova in `R:\interventi-prove-b`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use quado_actions::execute::MoveResult;
use quado_actions::mover::{ShellMover, Trace};
use quado_actions::probe::{Probe, SystemConfig};
use quado_actions::win_probe::WinProbe;
use quado_kb::extract::ExtractError;
use quado_model::intervention::{BinCount, ReferenceReport};
use quado_testkit::bin::{self, BinSetting};
use quado_testkit::trees;

/// Capacità del Cestino di `R:` durante le prove, in MiB.
const CAPACITY_MIB: u32 = 64;

/// Impostazioni del Cestino lette dal registro vero (HKCU, sola lettura) come le leggerà l'interfaccia.
struct HkcuConfig;

impl SystemConfig for HkcuConfig {
    fn bin_settings(&self, root: &str) -> Result<(Option<u64>, bool), u32> {
        let guid = quado_kb::win::volume_guid(root).ok_or(2u32)?;
        match quado_kb::extract::bin_settings(&quado_kb::win::LiveRegistry, &guid) {
            Ok(b) => Ok((b.max_capacity_mib.map(|m| u64::from(m) << 20), b.nuke_on_delete)),
            Err(ExtractError::NotPresent) => Ok((None, false)),
            Err(ExtractError::Io(e)) => Err(e),
            Err(_) => Err(13),
        }
    }
    fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
        Ok(None)
    }
    fn references(&self, _: &[String]) -> ReferenceReport {
        ReferenceReport::default()
    }
}

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}

/// La cartella di prova `R:\interventi-prove-b`, con il Cestino di `R:` vuoto e ridotto. Alla fine si
/// cancella la cartella, si svuota il Cestino di `R:` e si ripristina la capacità.
struct Area {
    root: PathBuf,
    dir: PathBuf,
    probe: WinProbe,
    _capacity: BinSetting,
}

impl Area {
    fn new() -> Self {
        let root = trees::mounted_vhdx()
            .into_iter()
            .find(|(n, _)| n == "ntfs")
            .map(|(_, p)| p)
            .expect("VHDX di prova NTFS montato (cargo xtask vhdx crea)");
        assert!(s(&root).eq_ignore_ascii_case(r"R:\"), "il VHDX di prova NTFS non è R:\\ ma {}", root.display());
        bin::empty_test_volume(&root);
        let dir = root.join("interventi-prove-b");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let capacity = BinSetting::capacity(&root, CAPACITY_MIB);
        Self { root, dir, probe: WinProbe::new(Box::new(HkcuConfig)), _capacity: capacity }
    }

    fn bin(&self) -> BinCount {
        self.probe.volume_bin(&s(&self.dir)).unwrap().content.expect("contenuto del Cestino di R:")
    }

    /// Voci della cartella del Cestino dell'utente su `R:`: (nome, cartella, byte).
    fn bin_entries(&self) -> Vec<(String, bool, u64)> {
        let Some(d) = bin::user_bin_dir(&self.root) else { return Vec::new() };
        let mut v: Vec<_> = fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| {
                let m = e.metadata().unwrap();
                (e.file_name().to_string_lossy().into_owned(), m.is_dir(), m.len())
            })
            .collect();
        v.sort();
        v
    }
}

impl Drop for Area {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
        bin::empty_test_volume(&self.root);
    }
}

// ---------------------------------------------------------------------------------------------
// Sorveglianza delle finestre di Windows

/// Una finestra visibile di primo livello di questo processo, con i controlli che hanno un testo.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Window {
    hwnd: usize,
    class: String,
    title: String,
    /// (handle, identificativo del controllo, testo).
    controls: Vec<(usize, i32, String)>,
}

impl Window {
    /// Il pulsante "Sì" (`&Yes` su questo PC, in inglese; `&Sì` in italiano).
    fn yes_button(&self) -> Option<usize> {
        self.controls.iter().find(|(_, _, t)| matches!(t.replace('&', "").as_str(), "Yes" | "Sì")).map(|c| c.0)
    }
}

fn window_text(hwnd: windows_sys::Win32::Foundation::HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let mut buf = [0u16; 512];
    // SAFETY: buffer locale della lunghezza dichiarata; la finestra può essere già chiusa (risponde 0).
    let n = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn class_name(hwnd: windows_sys::Win32::Foundation::HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW;
    let mut buf = [0u16; 256];
    // SAFETY: come `window_text`.
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

unsafe extern "system" fn collect_top(hwnd: windows_sys::Win32::Foundation::HWND, lparam: isize) -> windows_sys::core::BOOL {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindowVisible};
    let mut pid = 0u32;
    // SAFETY: `hwnd` arriva da EnumWindows; `lparam` è il puntatore al Vec di `own_windows`, vivo per
    // tutta l'enumerazione e usato solo da questo thread.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == std::process::id() && IsWindowVisible(hwnd) != 0 {
            (*(lparam as *mut Vec<usize>)).push(hwnd as usize);
        }
    }
    1
}

unsafe extern "system" fn collect_child(hwnd: windows_sys::Win32::Foundation::HWND, lparam: isize) -> windows_sys::core::BOOL {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetDlgCtrlID;
    let t = window_text(hwnd);
    if !t.trim().is_empty() {
        // SAFETY: `hwnd` arriva da EnumChildWindows; `lparam` è il puntatore al Vec di `own_windows`, vivo
        // per tutta l'enumerazione e usato solo da questo thread.
        unsafe { (*(lparam as *mut Vec<(usize, i32, String)>)).push((hwnd as usize, GetDlgCtrlID(hwnd), t)) };
    }
    1
}

/// Finestre visibili di primo livello di questo processo (i dialoghi di `IFileOperation`).
fn own_windows() -> Vec<Window> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{EnumChildWindows, EnumWindows};
    let mut handles: Vec<usize> = Vec::new();
    // SAFETY: il callback riceve il puntatore a `handles`, che vive fino alla fine della chiamata.
    unsafe { EnumWindows(Some(collect_top), (&raw mut handles) as isize) };
    handles
        .into_iter()
        .map(|h| {
            let hwnd = h as windows_sys::Win32::Foundation::HWND;
            let mut controls: Vec<(usize, i32, String)> = Vec::new();
            // SAFETY: come sopra, con `controls`.
            unsafe { EnumChildWindows(hwnd, Some(collect_child), (&raw mut controls) as isize) };
            Window { hwnd: h, class: class_name(hwnd), title: window_text(hwnd), controls }
        })
        .collect()
}

fn post(hwnd: usize, msg: u32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;
    // SAFETY: messaggio asincrono a una finestra di questo processo; se è già chiusa non succede niente.
    unsafe { PostMessageW(hwnd as windows_sys::Win32::Foundation::HWND, msg, 0, 0) };
}

/// Come rispondere alle finestre di Windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    /// Chiudere (`WM_CLOSE`): "No" o "Annulla".
    Close,
    /// Premere "Sì" (`BM_CLICK`), solo nelle prove che misurano cosa succede dopo.
    Yes,
}

/// Esito di uno spostamento sorvegliato.
#[derive(Debug)]
struct Run {
    trace: Trace,
    /// Finestre di Windows comparse durante l'operazione.
    windows: Vec<Window>,
}

/// Sposta `path` nel Cestino con `ShellMover` in un thread a parte e sorveglia le finestre di Windows.
fn recycle_watched(path: &Path, answer: Answer) -> Run {
    use windows_sys::Win32::UI::WindowsAndMessaging::{BM_CLICK, WM_CLOSE};
    let p = s(path);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(ShellMover::new(None).recycle_traced(&p));
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut windows: Vec<Window> = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(trace) => {
                eprintln!("{}: {trace:#?}", path.display());
                return Run { trace, windows };
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => panic!("il thread dello spostamento è terminato senza esito"),
        }
        for w in own_windows() {
            let new = !windows.iter().any(|x| x.hwnd == w.hwnd);
            if new {
                eprintln!("finestra di Windows durante lo spostamento: {w:?}");
                windows.push(w.clone());
            }
            match (answer, w.yes_button()) {
                (Answer::Yes, Some(button)) if new => post(button, BM_CLICK),
                (Answer::Yes, Some(_)) => {}
                _ => post(w.hwnd, WM_CLOSE),
            }
        }
        assert!(Instant::now() < deadline, "IFileOperation non ha finito entro 60 s; finestre: {windows:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Prove

#[test]
#[ignore = "richiede i VHDX di prova"]
fn file_goes_to_the_recycle_bin() {
    let area = Area::new();
    let f = area.dir.join("documento.bin");
    trees::write_file(&f, 3 << 20, 7);
    let (bin0, entries0) = (area.bin(), area.bin_entries());
    let run = recycle_watched(&f, Answer::Close);
    assert!(run.windows.is_empty(), "nessuna finestra attesa: {:?}", run.windows);
    assert_eq!(run.trace.result, MoveResult::Recycled);
    assert!(!f.exists());
    let bin1 = area.bin();
    assert_eq!(bin1.items, bin0.items + 1);
    assert_eq!(bin1.bytes, bin0.bytes + (3 << 20));
    let new: Vec<_> = area.bin_entries().into_iter().filter(|e| !entries0.contains(e)).collect();
    eprintln!("nuove voci nel Cestino dell'utente: {new:?}");
    assert!(new.iter().any(|(n, dir, len)| n.starts_with("$R") && !dir && *len == 3 << 20), "{new:?}");
    assert!(new.iter().any(|(n, _, _)| n.starts_with("$I")), "{new:?}");
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn folder_goes_to_the_recycle_bin() {
    let area = Area::new();
    let d = area.dir.join("cartella");
    trees::write_file(&d.join("a.bin"), 1 << 20, 1);
    trees::write_file(&d.join("sotto").join("b.bin"), 2 << 20, 2);
    fs::write(d.join("piccolo.txt"), b"x").unwrap();
    let (bin0, entries0) = (area.bin(), area.bin_entries());
    let run = recycle_watched(&d, Answer::Close);
    assert!(run.windows.is_empty(), "nessuna finestra attesa: {:?}", run.windows);
    assert_eq!(run.trace.result, MoveResult::Recycled);
    assert!(!d.exists());
    let bin1 = area.bin();
    assert_eq!(bin1.items, bin0.items + 1, "una cartella è un elemento del Cestino");
    assert_eq!(bin1.bytes, bin0.bytes + (3 << 20) + 1);
    let new: Vec<_> = area.bin_entries().into_iter().filter(|e| !entries0.contains(e)).collect();
    eprintln!("nuove voci nel Cestino dell'utente: {new:?}");
    assert!(new.iter().any(|(n, dir, _)| n.starts_with("$R") && *dir), "{new:?}");
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn file_held_open_stays_in_place() {
    let area = Area::new();
    let f = area.dir.join("in-uso.bin");
    trees::write_file(&f, 2 << 20, 9);
    let before = area.probe.attributes(&s(&f)).unwrap();
    let bin0 = area.bin();
    // Aperto senza condivisione: nessuno può rinominarlo o eliminarlo finché resta aperto.
    let held = trees::hold_exclusive(&f);
    let run = recycle_watched(&f, Answer::Close);
    drop(held);
    assert!(run.windows.is_empty(), "nessuna finestra attesa (FOF_NOERRORUI): {:?}", run.windows);
    assert!(matches!(run.trace.result, MoveResult::Failed(_) | MoveResult::NotRecycled(_)), "{:?}", run.trace.result);
    let after = area.probe.attributes(&s(&f)).expect("il file è ancora al suo posto");
    assert_eq!((after.file_id, after.logical, after.alloc), (before.file_id, before.logical, before.alloc));
    assert_eq!(area.bin(), bin0);
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn volume_bin_capacity_and_nuke_on_delete() {
    let area = Area::new();
    let vb = area.probe.volume_bin(&s(&area.dir)).unwrap();
    eprintln!("Cestino di R: {vb:?}");
    assert_eq!(vb.root, r"R:\");
    assert!(vb.has_bin);
    assert!(!vb.nuke_on_delete);
    assert_eq!(vb.capacity, Some(u64::from(CAPACITY_MIB) << 20));
    assert_eq!(vb.content, Some(BinCount { items: 0, bytes: 0 }), "Cestino di R: svuotato all'inizio");
    {
        let _nuke = BinSetting::nuke(&area.root);
        let vb = area.probe.volume_bin(&s(&area.dir.join("qualunque.bin"))).unwrap();
        // Con NukeOnDelete lo spostamento resta disattivato: il mover qui non si chiama mai.
        assert!(vb.nuke_on_delete && vb.has_bin);
        assert_eq!(vb.capacity, Some(u64::from(CAPACITY_MIB) << 20));
    }
    assert!(!area.probe.volume_bin(&s(&area.dir)).unwrap().nuke_on_delete, "NukeOnDelete ripristinato");
    // Spazio libero: quello del volume R:.
    assert!(area.probe.free_space(&s(&area.dir)).unwrap() > 0);
}

/// File di `len` caratteri di percorso esatti dentro `dir` (una cartella intermedia e il nome).
fn file_with_path_len(dir: &Path, len: usize) -> PathBuf {
    let sub = dir.join(format!("p{len}-{}", "x".repeat(100)));
    let base = s(&sub).encode_utf16().count() + 1;
    let name = format!("{}.bin", "n".repeat(len - base - 4));
    let f = sub.join(name);
    trees::write_file(&f, 1 << 20, 3);
    assert_eq!(s(&f).encode_utf16().count(), len);
    f
}

#[test]
#[ignore = "richiede i VHDX di prova"]
fn paths_near_and_over_260() {
    let area = Area::new();
    // Fino a 259 caratteri: il Cestino li accetta.
    for len in [255, 259] {
        let f = file_with_path_len(&area.dir, len);
        let bin0 = area.bin();
        let run = recycle_watched(&f, Answer::Close);
        assert!(run.windows.is_empty(), "{len} caratteri, nessuna finestra attesa: {:?}", run.windows);
        assert_eq!(run.trace.result, MoveResult::Recycled, "{len} caratteri");
        assert!(!f.exists());
        assert_eq!(area.bin().items, bin0.items + 1);
    }
    // Oltre: la shell accetterebbe l'elemento ma chiederebbe di eliminarlo per sempre (misurato con 325
    // caratteri, prima di ogni evento del sink). Senza nome breve 8.3 il mover non lo passa alla shell.
    for len in [260, 325] {
        let f = if len == 260 { file_with_path_len(&area.dir, len) } else { trees::long_path(&area.dir, len - 10).join("lungo.bin") };
        if !f.exists() {
            trees::write_file(&f, 1 << 20, 3);
        }
        let candidates = quado_actions::shell::path_candidates(&s(&f));
        eprintln!("percorso di {} caratteri; candidati: {candidates:?}", s(&f).encode_utf16().count());
        let bin0 = area.bin();
        let before = area.probe.attributes(&s(&f)).unwrap();
        let run = recycle_watched(&f, Answer::Close);
        assert!(run.windows.is_empty(), "nessuna finestra attesa: {:?}", run.windows);
        if candidates.len() == 2 {
            assert_eq!(run.trace.result, MoveResult::Recycled, "nome breve {:?}", run.trace.parsed);
            assert!(!f.exists());
            assert_eq!(area.bin().items, bin0.items + 1);
        } else {
            assert_eq!(run.trace.result, MoveResult::Failed(quado_actions::mover::PATH_TOO_LONG.0));
            assert!(run.trace.events.is_empty() && run.trace.parsed.is_none());
            let after = area.probe.attributes(&s(&f)).expect("il file è ancora al suo posto");
            assert_eq!((after.file_id, after.logical), (before.file_id, before.logical));
            assert_eq!(area.bin(), bin0);
        }
    }
}

/// Cartella con un percorso corto e file interni lunghi (come `node_modules`). Conta il percorso originale
/// del file interno, non quello che avrà nel Cestino: con 259 caratteri la cartella va nel Cestino (dove
/// il file avrà circa 300 caratteri); da 260 in su Windows chiede "Do you want to permanently delete this
/// folder? The folder contains items whose names are too long for the Recycle Bin", dopo un
/// `PreDeleteItem` con il flag "nel Cestino". Con "No" la cartella resta. La simulazione deve quindi
/// bloccare le cartelle con un file interno di 260 caratteri o più (misurato il 14/10/2026).
#[test]
#[ignore = "richiede i VHDX di prova"]
fn folder_with_long_paths_inside() {
    use windows::Win32::UI::Shell::COPYENGINE_E_USER_CANCELLED;
    let area = Area::new();
    let ok = area.dir.join("interno-259");
    let f = file_with_path_len(&ok, 259);
    let bin0 = area.bin();
    let run = recycle_watched(&ok, Answer::Close);
    assert!(run.windows.is_empty(), "nessuna finestra attesa: {:?}", run.windows);
    assert_eq!(run.trace.result, MoveResult::Recycled);
    assert!(!ok.exists() && !f.exists());
    assert_eq!(area.bin().items, bin0.items + 1);

    for len in [260, 337] {
        let d = area.dir.join(format!("interno-{len}"));
        let f = if len == 260 { file_with_path_len(&d, len) } else { trees::long_path(&d, len - 15).join("in-fondo.bin") };
        if !f.exists() {
            trees::write_file(&f, 1 << 20, 6);
        }
        let before = area.probe.attributes(&s(&f)).unwrap();
        let bin0 = area.bin();
        let run = recycle_watched(&d, Answer::Close);
        assert_eq!(run.windows.len(), 1, "{len} caratteri: Windows chiede di eliminare per sempre: {:?}", run.windows);
        assert_eq!(run.trace.result, MoveResult::Failed(COPYENGINE_E_USER_CANCELLED.0));
        let after = area.probe.attributes(&s(&f)).expect("con \"No\" la cartella resta al suo posto");
        assert_eq!((after.file_id, after.logical), (before.file_id, before.logical));
        assert_eq!(area.bin(), bin0);
    }
}

/// Elemento più grande del Cestino, spostato da solo: senza "Sì a tutto" e con `FOF_WANTNUKEWARNING`
/// Windows chiede prima di eliminarlo per sempre. La prova chiude la finestra (cioè "No"). Le prove in
/// `docs/m1/` hanno visto che `PreDeleteItem` non riporta sempre l'eliminazione definitiva: qui si vede
/// quale difesa agisce.
#[test]
#[ignore = "richiede i VHDX di prova"]
fn oversized_item_asks_and_no_keeps_it() {
    let area = Area::new();
    let f = area.dir.join("troppo-grande.bin");
    // 100 MiB logici in un Cestino da 64 MiB: Windows conta la dimensione logica; file sparse, quasi
    // niente scritto sul disco.
    trees::sparse_file(&f, 100 << 20);
    let before = area.probe.attributes(&s(&f)).unwrap();
    let bin0 = area.bin();
    let run = recycle_watched(&f, Answer::Close);
    assert_eq!(run.windows.len(), 1, "Windows chiede prima di eliminare per sempre: {:?}", run.windows);
    assert!(matches!(run.trace.result, MoveResult::Failed(_)), "{:?}", run.trace.result);
    let after = area.probe.attributes(&s(&f)).expect("con \"No\" il file resta al suo posto");
    assert_eq!((after.file_id, after.logical), (before.file_id, before.logical));
    assert_eq!(area.bin(), bin0, "niente nel Cestino");
}

/// Come la prova precedente, ma la prova risponde "Sì" (l'utente che conferma senza leggere): si misura
/// se il sink ferma ancora l'eliminazione, e che l'esito non dica mai "nel Cestino". Il file è dati di
/// prova su R:.
#[test]
#[ignore = "richiede i VHDX di prova"]
fn oversized_item_after_yes_is_reported_honestly() {
    let area = Area::new();
    let f = area.dir.join("troppo-grande-si.bin");
    trees::sparse_file(&f, 100 << 20);
    let bin0 = area.bin();
    let run = recycle_watched(&f, Answer::Yes);
    let exists = f.exists();
    eprintln!("dopo \"Sì\": il file {}", if exists { "è ancora al suo posto" } else { "non c'è più" });
    assert!(!run.windows.is_empty(), "nessuna domanda di Windows");
    assert_ne!(run.trace.result, MoveResult::Recycled);
    assert_eq!(area.bin(), bin0, "niente nel Cestino");
    if exists {
        assert!(matches!(run.trace.result, MoveResult::Failed(_)), "{:?}", run.trace.result);
    } else {
        // Eliminato per sempre: l'esito lo deve dire, e il piano si ferma.
        assert!(matches!(run.trace.result, MoveResult::NotRecycled(_)), "{:?}", run.trace.result);
    }
}

/// "Svuota il Cestino" del prodotto (`procedures::empty_recycle_bin`): Windows mostra il suo
/// dialogo di conferma (nessun `SHERB_NOCONFIRMATION`); la prova risponde "No" e il Cestino di R: resta
/// com'era.
#[test]
#[ignore = "richiede i VHDX di prova"]
fn empty_shows_the_windows_dialog_and_no_keeps_the_items() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{BM_CLICK, WM_CLOSE};
    let area = Area::new();
    let f = area.dir.join("da-tenere-nel-cestino.bin");
    trees::write_file(&f, 1 << 20, 5);
    assert_eq!(recycle_watched(&f, Answer::Close).trace.result, MoveResult::Recycled);
    let bin0 = area.bin();
    assert!(bin0.items >= 1);
    let root = s(&area.root);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(quado_actions::procedures::empty_recycle_bin(&root, None));
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut windows: Vec<Window> = Vec::new();
    let result = loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(r) => break r,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => panic!("il thread dello svuotamento è terminato senza esito"),
        }
        for w in own_windows() {
            if !windows.iter().any(|x| x.hwnd == w.hwnd) {
                eprintln!("finestra di Windows durante lo svuotamento: {w:?}");
                windows.push(w.clone());
            }
            let no = w.controls.iter().find(|(_, _, t)| t.replace('&', "") == "No").map(|c| c.0);
            match no {
                Some(button) => post(button, BM_CLICK),
                None => post(w.hwnd, WM_CLOSE),
            }
        }
        assert!(Instant::now() < deadline, "lo svuotamento non è finito entro 60 s; finestre: {windows:?}");
    };
    eprintln!("esito dello svuotamento annullato: {result:?}");
    assert!(!windows.is_empty(), "Windows non ha chiesto conferma prima di svuotare");
    assert_eq!(area.bin(), bin0, "con \"No\" il Cestino resta com'era");
}
