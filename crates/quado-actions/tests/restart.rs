// SPDX-License-Identifier: GPL-3.0-or-later
//! Quali processi usano un file (`restart::processes_using`, con `FileProcessIdsUsingFileInformation` al
//! posto di Restart Manager). File chiuso: nessuno. File tenuto aperto da un processo figlio di prova
//! (questo stesso eseguibile, rilanciato su un test che apre il file e aspetta): compare il PID del figlio.
//! File aperto da questo processo: compare questo processo.
//!
//! I test creano processi (solo qui: Quado no). Il figlio termina sempre, anche se il test fallisce.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use quado_actions::restart::{ProcessUse, processes_using};

const HOLD_ENV: &str = "QUADO_PROVA_RM_TIENI";
const READY: &str = "QUADO-PROVA-RM-PRONTO";

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Cartella di prova in `%TEMP%`, cancellata al drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("quado-prova-rm-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }

    fn file(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, b"download incompleto").unwrap();
        p
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn s(p: &Path) -> String {
    p.to_str().unwrap().to_owned()
}

/// Processo figlio terminato al drop.
struct Holder(Child);

impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Rilancia questo eseguibile sul test `child_holds_a_file`, che apre `path` e aspetta la chiusura del
/// suo stdin. Ritorna quando il figlio ha aperto il file.
fn hold_in_child(path: &Path) -> Holder {
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["child_holds_a_file", "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env(HOLD_ENV, path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("processo figlio");
    let mut holder = Holder(child);
    let stdout = holder.0.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if line.contains(READY) {
                let _ = tx.send(());
            }
        }
    });
    rx.recv_timeout(Duration::from_secs(60)).expect("il figlio non ha aperto il file");
    holder
}

/// Il figlio: apre il file (lettura, condiviso come un browser che scarica) e aspetta.
#[test]
#[ignore = "processo figlio dei test di Restart Manager: lo avvia hold_in_child"]
fn child_holds_a_file() {
    let Ok(path) = std::env::var(HOLD_ENV) else { return };
    let _f = File::open(&path).expect("file da tenere aperto");
    println!("{READY}");
    std::io::stdout().flush().unwrap();
    let mut rest = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut rest);
}

fn pids(v: &[ProcessUse]) -> Vec<u32> {
    v.iter().map(|p| p.pid).collect()
}

/// Un file appena scritto può essere aperto per un attimo dall'antivirus: si aspetta che l'elenco si
/// svuoti, fino a 5 secondi.
fn eventually_free(paths: &[String]) -> Vec<ProcessUse> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let users = processes_using(paths).unwrap();
        if users.is_empty() || Instant::now() > deadline {
            return users;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn no_paths() {
    assert_eq!(processes_using(&[]), Ok(Vec::new()));
}

#[test]
fn closed_file_has_no_users() {
    let d = TestDir::new("chiuso");
    let p = d.file("chiuso.crdownload");
    assert_eq!(eventually_free(&[s(&p)]), Vec::new());
    // Costo per file: un'apertura con i soli attributi e una domanda a NTFS.
    let t = Instant::now();
    for _ in 0..20 {
        processes_using(&[s(&p)]).unwrap();
    }
    eprintln!("un file chiuso: {:?}", t.elapsed() / 20);
}

#[test]
fn file_held_by_this_process() {
    let d = TestDir::new("questo");
    let p = d.file("aperto.part");
    let f = File::open(&p).unwrap();
    let users = processes_using(&[s(&p)]).unwrap();
    eprintln!("aperto da questo processo: {users:?}");
    let me = users.iter().find(|u| u.pid == std::process::id()).expect("questo processo nell'elenco");
    assert!(!me.app_name.is_empty());
    assert_eq!(me.service, None);
    drop(f);
    assert_eq!(eventually_free(&[s(&p)]), Vec::new());
}

#[test]
fn file_held_by_a_child_process() {
    let d = TestDir::new("figlio");
    let held = d.file("tenuto.crdownload");
    let closed = d.file("chiuso.part");
    let child = hold_in_child(&held);
    let child_pid = child.0.id();
    let users = processes_using(&[s(&held), s(&closed)]).unwrap();
    eprintln!("figlio {child_pid}: {users:?}");
    let entry = users.iter().find(|u| u.pid == child_pid).expect("il figlio nell'elenco");
    assert!(!entry.app_name.is_empty());
    assert!(!pids(&users).contains(&std::process::id()), "questo processo non tiene aperto il file");
    // Solo il file chiuso: il figlio non c'è.
    assert!(!pids(&processes_using(&[s(&closed)]).unwrap()).contains(&child_pid));
    // Figlio terminato: il file è libero.
    drop(child);
    assert_eq!(eventually_free(&[s(&held)]), Vec::new());
}

/// Un eseguibile in esecuzione è in uso anche senza un handle aperto sul file: la sua immagine è mappata.
/// Si copia `cmd.exe` di Windows nella cartella di prova e lo si avvia in attesa sullo stdin (il
/// campione di Restart Manager comprende tutti gli eseguibili e le DLL).
#[test]
fn running_executable_is_in_use() {
    let d = TestDir::new("eseguibile");
    let exe = d.0.join("prova-cmd.exe");
    let system = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_owned());
    std::fs::copy(Path::new(&system).join(r"System32\cmd.exe"), &exe).unwrap();
    let child = Command::new(&exe).args(["/q", "/k"]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("cmd di prova");
    let holder = Holder(child);
    let pid = holder.0.id();
    let deadline = Instant::now() + Duration::from_secs(10);
    let users = loop {
        let u = processes_using(&[s(&exe)]).unwrap();
        if u.iter().any(|x| x.pid == pid) || Instant::now() > deadline {
            break u;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    eprintln!("eseguibile in esecuzione ({pid}): {users:?}");
    let entry = users.iter().find(|u| u.pid == pid).expect("il processo che esegue la copia di cmd.exe");
    // Restart Manager dà la descrizione del programma ("Windows Command Processor").
    assert!(!entry.app_name.is_empty(), "{entry:?}");
    drop(holder);
}

/// Percorsi particolari: file inesistente, prefisso `\\?\`, percorso oltre 260 caratteri.
#[test]
fn unusual_paths() {
    let d = TestDir::new("percorsi");
    let missing = d.0.join("manca.part");
    eprintln!("file inesistente: {:?}", processes_using(&[s(&missing)]));
    assert_eq!(processes_using(&[s(&missing)]), Ok(Vec::new()));

    let p = d.file("prefisso.part");
    let _f = File::open(&p).unwrap();
    let verbatim = format!(r"\\?\{}", s(&p));
    let r = processes_using(&[verbatim]);
    eprintln!("con il prefisso \\\\?\\: {r:?}");
    assert!(pids(&r.unwrap()).contains(&std::process::id()));

    let mut long = d.0.clone();
    while long.as_os_str().len() < 280 {
        long.push("cartella-con-un-nome-piuttosto-lungo");
    }
    std::fs::create_dir_all(&long).unwrap();
    let lp = long.join("lungo.part");
    std::fs::write(&lp, b"x").unwrap();
    let _g = File::open(&lp).unwrap();
    // Percorsi oltre i 260 caratteri, con e senza `\\?\`: si aprono con il prefisso, senza nome breve.
    for path in [s(&lp), format!(r"\\?\{}", s(&lp))] {
        let r = processes_using(std::slice::from_ref(&path));
        eprintln!("percorso di {} caratteri: {r:?}", path.len());
        assert!(pids(&r.unwrap()).contains(&std::process::id()));
    }
}
