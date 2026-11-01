// SPDX-License-Identifier: GPL-3.0-or-later
//! Pipe e modalità helper (05 D3, 06 §1.5 e §11), senza elevazione e senza prompt UAC: server e client
//! nello stesso processo di test (un thread), oppure il client in un processo figlio, che è questo stesso
//! eseguibile di test avviato con `child_process_entry` e una variabile d'ambiente.
//!
//! - DACL della pipe letta con `GetSecurityInfo`: un solo ACE, il SID dell'utente, DACL protetta.
//! - Nome casuale diverso a ogni creazione; nessuno aggiunge un'istanza con lo stesso nome.
//! - Un client di un processo diverso da quello atteso viene rifiutato (`WrongPeer`), e il client rifiuta
//!   un server diverso dal genitore.
//! - Giro completo client/server; annullamento di `accept` e `recv` entro 500 ms; limiti delle cornici.
//! - `run_with` in un processo figlio con una raccolta finta, `serve_connected` nello stesso processo; i
//!   profili altrui non trasmettono nomi (si cercano nei byte della pipe).

use std::ffi::c_void;
use std::io::Write;
use std::os::windows::io::AsRawHandle;
use std::process::{Command, Stdio};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use quado_model::{Flags, NodeData, Tree};
use quado_scan_win::helper::pipe::{self, Client, PipeError, Server};
use quado_scan_win::helper::proto::{
    MAX_REPLY_BYTES, MAX_REQUEST_BYTES, PathIssue, ProtoError, Reply, Request, Scope, ShadowStorage, Totals, classify, decode_reply,
    encode_request,
};
use quado_scan_win::helper::serve::{
    Collector, FolderScan, exe_location_refused, exit, final_path, forbidden_dirs, inside_any, parent_process, run_with,
    scope_of, serve_connected,
};
use quado_scan_win::{Problem, RootError, system};
use quado_testkit::trees::{self, TestDir};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, GetSecurityInfo, SE_KERNEL_OBJECT};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL, DACL_SECURITY_INFORMATION, GetAce, GetSecurityDescriptorControl, PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_READ_DATA, FILE_WRITE_DATA, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile};
use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT};

const LONG: Duration = Duration::from_secs(30);

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn me() -> u32 {
    std::process::id()
}

/// Nome valido per l'helper, diverso a ogni chiamata (per i server scritti a mano qui).
fn test_pipe_name() -> String {
    static N: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    format!("{}{:08x}{:08x}{:08x}{:08x}", pipe::NAME_PREFIX, me(), N.fetch_add(1, Relaxed), nanos, 0x5eed_u32)
}

fn frame(body: &[u8]) -> Vec<u8> {
    let mut f = (body.len() as u32).to_le_bytes().to_vec();
    f.extend_from_slice(body);
    f
}

/// Cornici di un flusso di byte, decodificate.
fn replies_in(bytes: &[u8]) -> Vec<Reply> {
    let mut out = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let len = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
        out.push(decode_reply(&rest[4..4 + len]).expect("risposta valida"));
        rest = &rest[4 + len..];
    }
    out
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn utf16le(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// Server di pipe scritto a mano, sincrono, senza i controlli di `Server`: per inviare byte qualunque
/// all'helper e catturare tutto quello che scrive.
struct RawServer(HANDLE);

impl RawServer {
    fn create(name: &str) -> Self {
        let z = wide_z(name);
        // SAFETY: nome terminato da zero; sicurezza predefinita; una istanza.
        let h = unsafe { CreateNamedPipeW(z.as_ptr(), PIPE_ACCESS_DUPLEX, PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT, 1, 65536, 65536, 0, null()) };
        assert_ne!(h, INVALID_HANDLE_VALUE, "CreateNamedPipeW");
        Self(h)
    }

    fn accept(&self) {
        // SAFETY: pipe valida, I/O sincrono.
        if unsafe { ConnectNamedPipe(self.0, null_mut()) } == 0 {
            // SAFETY: nessun prerequisito.
            assert_eq!(unsafe { GetLastError() }, ERROR_PIPE_CONNECTED);
        }
    }

    fn write(&self, mut b: &[u8]) {
        while !b.is_empty() {
            let mut n = 0u32;
            // SAFETY: buffer valido con la sua lunghezza; I/O sincrono.
            assert_ne!(unsafe { WriteFile(self.0, b.as_ptr(), b.len() as u32, &mut n, null_mut()) }, 0, "WriteFile");
            b = &b[n as usize..];
        }
    }

    /// Tutto ciò che il client scrive, fino alla sua chiusura.
    fn read_to_end(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut buf = vec![0u8; 65536];
        loop {
            let mut n = 0u32;
            // SAFETY: buffer locale con la sua lunghezza; I/O sincrono.
            let ok = unsafe { ReadFile(self.0, buf.as_mut_ptr(), buf.len() as u32, &mut n, null_mut()) };
            if ok == 0 {
                // SAFETY: nessun prerequisito.
                assert_eq!(unsafe { GetLastError() }, ERROR_BROKEN_PIPE);
                return out;
            }
            out.extend_from_slice(&buf[..n as usize]);
        }
    }
}

impl Drop for RawServer {
    fn drop(&mut self) {
        // SAFETY: handle creato in `create`, chiuso una volta.
        unsafe { CloseHandle(self.0) };
    }
}

/// Client scritto a mano (diritti di lettura e scrittura generici, nessun controllo del server).
fn raw_client(name: &str) -> std::fs::File {
    std::fs::OpenOptions::new().read(true).write(true).open(name).expect("apertura della pipe")
}

// ---------------------------------------------------------------------------------------------------
// Raccolta finta

const OWN_PROFILE: &str = r"Q:\Users\io";
/// Nomi che esistono "dentro" i profili e i Cestini altrui della raccolta finta: non devono mai uscire.
const SECRETS: [&str; 4] = ["SegretoAltrui", "diario-privato.txt", "$RSEGRETO.docx", "altro"];

/// Raccolta finta su un volume `Q:` che non esiste: `folder` elenca sempre anche i nomi segreti se il
/// percorso è di un profilo o di un Cestino altrui, così un errore di ambito li farebbe trasmettere.
struct Fake {
    own_sid: String,
    privilege: Result<(), u32>,
    calls: Mutex<Vec<String>>,
}

impl Fake {
    fn new(own_sid: &str) -> Self {
        Self { own_sid: own_sid.to_owned(), privilege: Ok(()), calls: Mutex::new(Vec::new()) }
    }

    fn log(&self, s: String) {
        self.calls.lock().unwrap().push(s);
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Collector for Fake {
    fn enable_backup_privilege(&self) -> Result<(), u32> {
        self.privilege
    }

    fn folder(&self, path: &str) -> Result<FolderScan, RootError> {
        self.log(format!("folder {path}"));
        if path.ends_with("Rotta") {
            return Err(RootError(5));
        }
        let w = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
        let mut tree = Tree::with_root(&w(path), 0);
        let d = tree.push(Tree::ROOT, &w("sotto"), NodeData { flags: Flags::DIR, ..Default::default() });
        tree.push(d, &w("visibile.txt"), NodeData { logical: 10, alloc: 4096, ..Default::default() });
        if path.contains(r"\Users\altro") || path.contains("1002") {
            for s in SECRETS {
                tree.push(Tree::ROOT, &w(s), NodeData { logical: 1, alloc: 1, ..Default::default() });
            }
        }
        tree.seal();
        tree.aggregate();
        Ok(FolderScan { tree, problems: vec![(d, Problem::AccessDenied)] })
    }

    fn totals(&self, path: &str) -> Result<Totals, RootError> {
        self.log(format!("totals {path}"));
        if path.ends_with("Rotta2") {
            return Err(RootError(3));
        }
        Ok(Totals { logical: 5000, alloc: 8192, files: SECRETS.len() as u64, dirs: 1, unreadable: 0 })
    }

    fn sid_is_orphan(&self, sid: &str) -> Result<bool, u32> {
        self.log(format!("orphan {sid}"));
        if sid.ends_with("1002") { Ok(true) } else { Err(1332) }
    }

    fn shadow_storage(&self, volume: &str) -> Result<Option<ShadowStorage>, u32> {
        self.log(format!("shadow {volume}"));
        Ok(Some(ShadowStorage { used: 1, allocated: 2, max: 3 }))
    }

    fn profile_of(&self, sid: &str) -> Option<String> {
        (sid == self.own_sid).then(|| OWN_PROFILE.to_owned())
    }
}

fn own_sid() -> String {
    system::user_sid().expect("SID dell'utente")
}

/// Richiesta della prova end-to-end: una cartella per ogni ambito e per ogni errore.
fn e2e_request() -> Request {
    let sid = own_sid();
    Request {
        volume: r"Q:\".into(),
        folders: vec![
            r"Q:\Users\io\AppData\Negata".into(),
            r"Q:\Users\altro".into(),
            r"Q:\$Recycle.Bin\S-1-5-21-1-2-3-1002".into(),
            r"Q:\$Recycle.Bin\S-1-5-21-1-2-3-1003".into(),
            format!(r"Q:\$Recycle.Bin\{sid}"),
            r"Q:\Sistema\Rotta".into(),
            r"Q:\Users\altro2\Rotta2".into(),
        ],
    }
}

/// Sequenza e contenuto delle risposte alla richiesta di [`e2e_request`].
fn check_e2e_replies(replies: &[Reply]) {
    let sid = own_sid();
    assert_eq!(replies.len(), 10, "{replies:#?}");
    assert!(matches!(replies[0], Reply::Started { backup_privilege: true }));
    let Reply::Folder { index: 0, tree, problems } = &replies[1] else { panic!("{:?}", replies[1]) };
    assert_eq!(tree.path_lossy(2), r"Q:\Users\io\AppData\Negata\sotto\visibile.txt");
    assert_eq!((tree.alloc(Tree::ROOT), tree.files(Tree::ROOT)), (4096, 1));
    assert_eq!(problems, &vec![(1, Problem::AccessDenied)]);
    let totals = Totals { logical: 5000, alloc: 8192, files: 4, dirs: 1, unreadable: 0 };
    assert!(matches!(&replies[2], Reply::Totals { index: 1, totals: t } if *t == totals), "{:?}", replies[2]);
    assert!(matches!(&replies[3], Reply::Bin { index: 2, sid, totals: t, orphan: true } if sid == "S-1-5-21-1-2-3-1002" && *t == totals), "{:?}", replies[3]);
    // Ricerca dell'account non riuscita: non orfano.
    assert!(matches!(&replies[4], Reply::Bin { index: 3, sid, orphan: false, .. } if sid == "S-1-5-21-1-2-3-1003"), "{:?}", replies[4]);
    // Il Cestino dell'utente stesso è un albero con i nomi.
    let Reply::Folder { index: 4, tree, .. } = &replies[5] else { panic!("{:?}", replies[5]) };
    assert_eq!(tree.path_lossy(Tree::ROOT), format!(r"Q:\$Recycle.Bin\{sid}"));
    assert!(matches!(replies[6], Reply::Failed { index: 5, code: 5 }), "{:?}", replies[6]);
    assert!(matches!(replies[7], Reply::Failed { index: 6, code: 3 }), "{:?}", replies[7]);
    assert!(matches!(replies[8], Reply::Shadow(Ok(Some(ShadowStorage { used: 1, allocated: 2, max: 3 })))), "{:?}", replies[8]);
    assert!(matches!(replies[9], Reply::Done { code: 0 }), "{:?}", replies[9]);
}

// ---------------------------------------------------------------------------------------------------
// Processo figlio

const ENV_MODE: &str = "QUADO_PROVA_HELPER";
const ENV_PIPE: &str = "QUADO_PROVA_PIPE";
const ENV_PARENT: &str = "QUADO_PROVA_PADRE";

/// Punto d'ingresso del processo figlio: senza la variabile d'ambiente non fa niente.
#[test]
fn child_process_entry() {
    let Ok(mode) = std::env::var(ENV_MODE) else { return };
    let name = std::env::var(ENV_PIPE).unwrap();
    let parent: u32 = std::env::var(ENV_PARENT).unwrap().parse().unwrap();
    match mode.as_str() {
        // `run_with` per intero: posizione, genitore, connessione, richiesta, risposte.
        "run_with" => {
            let fake = Fake::new(&own_sid());
            assert_eq!(run_with(&[name, parent.to_string()], &fake), exit::OK);
        }
        // Solo un client: si collega (il server è il genitore) e viene scollegato, già durante la
        // connessione o alla lettura della richiesta.
        "client" => {
            let r = Client::connect(&name, parent).and_then(|mut c| c.read_request());
            assert_eq!(r, Err(PipeError::Closed), "il server rifiuta questo processo");
        }
        m => panic!("modo sconosciuto {m}"),
    }
}

fn spawn_child(mode: &str, pipe: &str) -> std::process::Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["child_process_entry", "--exact", "--nocapture", "--test-threads=1"])
        .env(ENV_MODE, mode)
        .env(ENV_PIPE, pipe)
        .env(ENV_PARENT, me().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("processo figlio")
}

fn child_ok(child: std::process::Child) {
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "figlio: {}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

// ---------------------------------------------------------------------------------------------------
// Pipe

#[test]
fn the_pipe_dacl_has_only_the_user() {
    let s = Server::create().unwrap();
    let mut dacl: *mut ACL = null_mut();
    let mut sd: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: handle del server valido; uscite locali; `sd` va liberato con LocalFree.
    let r = unsafe { GetSecurityInfo(s.as_raw_handle(), SE_KERNEL_OBJECT, DACL_SECURITY_INFORMATION, null_mut(), null_mut(), &mut dacl, null_mut(), &mut sd) };
    assert_eq!(r, 0, "GetSecurityInfo");
    assert!(!dacl.is_null());
    // SAFETY: `dacl` punta dentro `sd`, valido fino a LocalFree.
    let count = unsafe { (*dacl).AceCount };
    assert_eq!(count, 1, "un solo ACE");
    let mut ace: *mut c_void = null_mut();
    // SAFETY: indice 0 di una ACL con un ACE.
    assert_ne!(unsafe { GetAce(dacl, 0, &mut ace) }, 0);
    // SAFETY: ACE dentro la ACL; il tipo si controlla prima di leggere il resto.
    let (kind, mask) = unsafe {
        let a = ace.cast::<ACCESS_ALLOWED_ACE>();
        ((*a).Header.AceType, (*a).Mask)
    };
    assert_eq!(kind, 0, "ACCESS_ALLOWED_ACE_TYPE");
    assert_eq!(mask & (FILE_READ_DATA | FILE_WRITE_DATA), FILE_READ_DATA | FILE_WRITE_DATA, "{mask:#x}");
    let mut text: *mut u16 = null_mut();
    // SAFETY: il SID segue l'intestazione e la maschera (`SidStart`); Windows alloca `text`.
    let sid = unsafe {
        let a = ace.cast::<ACCESS_ALLOWED_ACE>();
        assert_ne!(ConvertSidToStringSidW((&raw mut (*a).SidStart).cast(), &mut text), 0);
        let mut n = 0;
        while *text.add(n) != 0 {
            n += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(text, n));
        LocalFree(text.cast());
        s
    };
    assert_eq!(sid, own_sid());
    let (mut control, mut rev) = (0u16, 0u32);
    // SAFETY: descrittore valido restituito da GetSecurityInfo.
    assert_ne!(unsafe { GetSecurityDescriptorControl(sd, &mut control, &mut rev) }, 0);
    assert_ne!(control & SE_DACL_PROTECTED, 0, "DACL protetta: niente ACE ereditate");
    // SAFETY: allocato da GetSecurityInfo.
    unsafe { LocalFree(sd) };
}

#[test]
fn names_are_random_and_cannot_be_taken_over() {
    // Server e client si spostano fra thread (l'interfaccia lavora su un thread di lavoro).
    fn send<T: Send>() {}
    send::<Server>();
    send::<Client>();
    let a = Server::create().unwrap();
    let b = Server::create().unwrap();
    assert_ne!(a.name(), b.name());
    for s in [&a, &b] {
        assert!(pipe::valid_name(s.name()), "{}", s.name());
    }
    // Una seconda istanza con lo stesso nome (un processo che volesse farsi trovare dall'helper) non si
    // crea: una istanza sola, e FILE_FLAG_FIRST_PIPE_INSTANCE.
    let z = wide_z(a.name());
    // SAFETY: nome terminato da zero; sicurezza predefinita.
    let h = unsafe { CreateNamedPipeW(z.as_ptr(), PIPE_ACCESS_DUPLEX, PIPE_TYPE_BYTE, 1, 0, 0, 0, null()) };
    assert_eq!(h, INVALID_HANDLE_VALUE);
    for bad in [r"\\.\pipe\quado-0123", r"\\.\pipe\altro-0123456789abcdef0123456789abcdef", r"C:\Windows\win.ini", r"\\.\pipe\quado-0123456789ABCDEF0123456789ABCDEF"] {
        assert!(!pipe::valid_name(bad), "{bad}");
        assert!(matches!(Client::connect(bad, me()), Err(PipeError::Win(123))), "{bad}");
    }
}

#[test]
fn full_round_trip_in_one_process() {
    let mut s = Server::create().unwrap();
    let name = s.name().to_owned();
    let request = Request { volume: r"C:\".into(), folders: vec![r"C:\Users\altro".into(), r"C:\Windows\x".into()] };
    let expected = request.clone();
    let helper = thread::spawn(move || {
        let mut c = Client::connect(&name, me()).unwrap();
        let sid = c.server_user_sid();
        let r = c.read_request().unwrap();
        assert_eq!(c.read_request(), Err(PipeError::Win(5023)), "una richiesta sola");
        c.send(&Reply::Started { backup_privilege: true }).unwrap();
        c.send(&Reply::Totals { index: 0, totals: Totals { logical: 1, alloc: 2, files: 3, dirs: 4, unreadable: 5 } }).unwrap();
        c.send(&Reply::Done { code: 0 }).unwrap();
        (r, sid)
    });
    let cancel = AtomicBool::new(false);
    s.accept(me(), LONG, &cancel).unwrap();
    s.send_request(&request).unwrap();
    assert_eq!(s.send_request(&request), Err(PipeError::Win(5023)), "una richiesta sola");
    assert!(matches!(s.recv(&cancel), Ok(Reply::Started { backup_privilege: true })));
    assert!(matches!(s.recv(&cancel), Ok(Reply::Totals { index: 0, totals }) if totals.unreadable == 5));
    assert!(matches!(s.recv(&cancel), Ok(Reply::Done { code: 0 })));
    let (got, sid) = helper.join().unwrap();
    assert_eq!(got, expected);
    assert_eq!(sid, Some(own_sid()), "SID dal token del processo server");
    // Il client ha chiuso: fine del flusso.
    assert_eq!(s.recv(&cancel).map(|_| ()), Err(PipeError::Closed));
    assert_eq!(s.recv(&cancel).map(|_| ()), Err(PipeError::Closed));
}

#[test]
fn the_server_validates_the_request_before_sending() {
    let mut s = Server::create().unwrap();
    let name = s.name().to_owned();
    let helper = thread::spawn(move || Client::connect(&name, me()).map(|_| ()));
    let cancel = AtomicBool::new(false);
    assert_eq!(s.send_request(&Request { volume: r"C:\".into(), folders: vec![] }), Err(PipeError::Win(233)), "prima di accept");
    s.accept(me(), LONG, &cancel).unwrap();
    helper.join().unwrap().unwrap();
    let bad = Request { volume: r"C:\".into(), folders: vec![r"C:\a\..\b".into()] };
    assert_eq!(s.send_request(&bad), Err(PipeError::Proto(ProtoError::BadPath(0, PathIssue::NotNormalized))));
}

#[test]
fn a_client_from_an_unexpected_process_is_rejected_in_process() {
    let mut s = Server::create().unwrap();
    let name = s.name().to_owned();
    // Il client è questo processo, il server aspetta un altro pid.
    let helper = thread::spawn(move || Client::connect(&name, me()).and_then(|mut c| c.read_request()));
    let cancel = AtomicBool::new(false);
    let other = me() ^ 0x4;
    assert_eq!(s.accept(other, LONG, &cancel), Err(PipeError::WrongPeer(me())));
    // Il client rifiutato è stato scollegato: già durante la connessione o alla lettura.
    assert_eq!(helper.join().unwrap(), Err(PipeError::Closed));
    // Il server tiene la pipe e può accettare il processo giusto.
    let name = s.name().to_owned();
    let helper = thread::spawn(move || Client::connect(&name, me()).map(|_| ()));
    s.accept(me(), LONG, &cancel).unwrap();
    helper.join().unwrap().unwrap();
}

#[test]
fn a_client_from_another_process_is_rejected() {
    let mut s = Server::create().unwrap();
    let child = spawn_child("client", s.name());
    let cancel = AtomicBool::new(false);
    // Il server aspetta questo processo (come se l'helper fosse lui): il figlio è un altro.
    assert_eq!(s.accept(me(), LONG, &cancel), Err(PipeError::WrongPeer(child.id())));
    child_ok(child);
}

#[test]
fn the_client_rejects_a_server_that_is_not_its_parent() {
    let s = Server::create().unwrap();
    assert_eq!(Client::connect(s.name(), me() ^ 0x4).map(|_| ()), Err(PipeError::WrongPeer(me())));
}

#[test]
fn accept_and_recv_are_cancelled_quickly() {
    let mut s = Server::create().unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let c2 = cancel.clone();
    let setter = thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        c2.store(true, Relaxed);
        Instant::now()
    });
    let r = s.accept(me(), LONG, &cancel);
    let done = Instant::now();
    let set_at = setter.join().unwrap();
    assert_eq!(r, Err(PipeError::Cancelled));
    assert!(done.duration_since(set_at) < Duration::from_millis(500), "{:?}", done.duration_since(set_at));

    // Senza client: tempo scaduto.
    let t0 = Instant::now();
    assert_eq!(s.accept(me(), Duration::from_millis(200), &AtomicBool::new(false)), Err(PipeError::Timeout));
    assert!(t0.elapsed() < Duration::from_millis(700), "{:?}", t0.elapsed());

    // `recv` senza dati, annullato.
    let _client = raw_client(s.name());
    s.accept(me(), LONG, &AtomicBool::new(false)).unwrap();
    cancel.store(false, Relaxed);
    let c2 = cancel.clone();
    let setter = thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        c2.store(true, Relaxed);
        Instant::now()
    });
    let r = s.recv(&cancel).map(|_| ());
    let done = Instant::now();
    let set_at = setter.join().unwrap();
    assert_eq!(r, Err(PipeError::Cancelled));
    assert!(done.duration_since(set_at) < Duration::from_millis(500), "{:?}", done.duration_since(set_at));
}

#[test]
fn recv_checks_the_frames() {
    let cancel = AtomicBool::new(false);
    // Lunghezza oltre il limite: rifiutata senza leggere né allocare il corpo; poi il flusso è chiuso.
    let mut s = Server::create().unwrap();
    let mut c = raw_client(s.name());
    s.accept(me(), LONG, &cancel).unwrap();
    c.write_all(&((MAX_REPLY_BYTES as u32) + 1).to_le_bytes()).unwrap();
    assert_eq!(s.recv(&cancel).map(|_| ()), Err(PipeError::Proto(ProtoError::TooLarge)));
    assert_eq!(s.recv(&cancel).map(|_| ()), Err(PipeError::Closed));

    // Corpo non valido.
    let mut s = Server::create().unwrap();
    let mut c = raw_client(s.name());
    s.accept(me(), LONG, &cancel).unwrap();
    c.write_all(&frame(&[0x10, 7])).unwrap();
    assert_eq!(s.recv(&cancel).map(|_| ()), Err(PipeError::Proto(ProtoError::Malformed)));

    // Cornice troncata dalla chiusura del client (lunghezza dichiarata grande, pochi byte): la memoria
    // cresce con i byte arrivati.
    let mut s = Server::create().unwrap();
    let mut c = raw_client(s.name());
    s.accept(me(), LONG, &cancel).unwrap();
    c.write_all(&(256u32 << 20).to_le_bytes()).unwrap();
    c.write_all(&[0x10; 100]).unwrap();
    drop(c);
    assert_eq!(s.recv(&cancel).map(|_| ()), Err(PipeError::Closed));

    // Una risposta grande passa (più del buffer della pipe e del passo di lettura).
    let mut s = Server::create().unwrap();
    let name = s.name().to_owned();
    let mut tree = Tree::with_root(&"R:\\grande".encode_utf16().collect::<Vec<_>>(), 0);
    for i in 0..60_000 {
        tree.push(Tree::ROOT, &format!("file-{i:06}.bin").encode_utf16().collect::<Vec<_>>(), NodeData { logical: i, alloc: 4096, ..Default::default() });
    }
    tree.aggregate();
    let sent = tree.alloc(Tree::ROOT);
    let helper = thread::spawn(move || {
        let mut c = Client::connect(&name, me()).unwrap();
        c.send(&Reply::Folder { index: 0, tree: Box::new(tree), problems: vec![] })
    });
    s.accept(me(), LONG, &cancel).unwrap();
    let Ok(Reply::Folder { tree, .. }) = s.recv(&cancel) else { panic!("albero grande") };
    assert_eq!((tree.len(), tree.alloc(Tree::ROOT)), (60_001, sent));
    helper.join().unwrap().unwrap();
}

#[test]
fn the_helper_reads_one_bounded_request() {
    // Lunghezza oltre il limite: rifiutata prima di leggere il corpo.
    let name = test_pipe_name();
    let raw = RawServer::create(&name);
    let n2 = name.clone();
    let helper = thread::spawn(move || {
        let mut c = Client::connect(&n2, me()).unwrap();
        let first = c.read_request();
        (first, c.read_request())
    });
    raw.accept();
    raw.write(&((MAX_REQUEST_BYTES as u32) + 1).to_le_bytes());
    let (first, second) = helper.join().unwrap();
    assert_eq!(first, Err(PipeError::Proto(ProtoError::TooLarge)));
    assert_eq!(second, Err(PipeError::Win(5023)), "una lettura sola");
    drop(raw);

    // Richiesta non valida: errore di protocollo con il motivo.
    let name = test_pipe_name();
    let raw = RawServer::create(&name);
    let helper = thread::spawn(move || Client::connect(&name, me()).unwrap().read_request());
    raw.accept();
    let bad = encode_request(&Request { volume: r"C:\".into(), folders: vec![r"D:\x".into()] });
    raw.write(&frame(&bad));
    assert_eq!(helper.join().unwrap(), Err(PipeError::Proto(ProtoError::BadPath(0, PathIssue::OtherVolume))));
}

/// Via SMB sul computer stesso un client conta come remoto. Controllo: una pipe senza
/// PIPE_REJECT_REMOTE_CLIENTS raggiunta per la stessa via; se non si raggiunge (servizio Server spento,
/// come su questo PC il 16/10/2026: "percorso di rete non trovato") la prova non dice niente e lo
/// dichiara. Esclusa dai test automatici: i provider di rete tentano la connessione a 127.0.0.1 e con
/// le altre prove in parallelo l'attesa arriva a 20 s.
#[test]
#[ignore = "richiede il servizio Server (SMB) attivo; lenta"]
fn remote_clients_are_rejected() {
    let control_name = test_pipe_name();
    let _control = RawServer::create(&control_name);
    let via_smb = |n: &str| n.replacen(r"\\.\", r"\\127.0.0.1\", 1);
    let reachable = std::fs::OpenOptions::new().read(true).open(via_smb(&control_name));
    let s = Server::create().unwrap();
    let ours = std::fs::OpenOptions::new().read(true).write(true).open(via_smb(s.name()));
    match reachable {
        Ok(_) => assert!(ours.is_err(), "client remoto accettato"),
        Err(e) => eprintln!("SMB locale non disponibile ({e}): prova dei client remoti non eseguita"),
    }
}

// ---------------------------------------------------------------------------------------------------
// Modalità helper

#[test]
fn run_with_checks_arguments_and_parent() {
    let fake = Fake::new(&own_sid());
    let name = Server::create().unwrap().name().to_owned();
    let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<String>>();
    assert_eq!(run_with(&[], &fake), exit::ARGS);
    assert_eq!(run_with(&s(&[&name]), &fake), exit::ARGS);
    assert_eq!(run_with(&s(&[&name, "12", "x"]), &fake), exit::ARGS);
    assert_eq!(run_with(&s(&[&name, "dodici"]), &fake), exit::ARGS);
    assert_eq!(run_with(&s(&[r"C:\Windows\win.ini", "12"]), &fake), exit::ARGS);
    // Questo processo non è il genitore di se stesso.
    assert_eq!(run_with(&s(&[&name, &me().to_string()]), &fake), exit::PEER);
    assert!(fake.calls().is_empty(), "nessuna raccolta prima della connessione");
}

#[test]
fn run_with_end_to_end_in_a_child_process() {
    let mut s = Server::create().unwrap();
    let child = spawn_child("run_with", s.name());
    let cancel = AtomicBool::new(false);
    s.accept(child.id(), LONG, &cancel).unwrap();
    s.send_request(&e2e_request()).unwrap();
    let mut replies = Vec::new();
    loop {
        match s.recv(&cancel) {
            Ok(r) => replies.push(r),
            Err(PipeError::Closed) => break,
            Err(e) => panic!("{e:?}"),
        }
    }
    check_e2e_replies(&replies);
    child_ok(child);
}

#[test]
fn serve_connected_answers_in_order() {
    let mut s = Server::create().unwrap();
    let name = s.name().to_owned();
    let helper = thread::spawn(move || {
        let fake = Fake::new(&own_sid());
        let code = serve_connected(Client::connect(&name, me()).unwrap(), &fake);
        (code, fake.calls())
    });
    let cancel = AtomicBool::new(false);
    s.accept(me(), LONG, &cancel).unwrap();
    let request = e2e_request();
    s.send_request(&request).unwrap();
    let mut replies = Vec::new();
    while let Ok(r) = s.recv(&cancel) {
        replies.push(r);
    }
    check_e2e_replies(&replies);
    let (code, calls) = helper.join().unwrap();
    assert_eq!(code, exit::OK);
    // Profili e Cestini altrui: solo totali, mai l'elenco.
    let f = &request.folders;
    let expected = vec![
        format!("folder {}", f[0]),
        format!("totals {}", f[1]),
        format!("totals {}", f[2]),
        "orphan S-1-5-21-1-2-3-1002".to_owned(),
        format!("totals {}", f[3]),
        "orphan S-1-5-21-1-2-3-1003".to_owned(),
        format!("folder {}", f[4]),
        format!("folder {}", f[5]),
        format!("totals {}", f[6]),
        r"shadow Q:\".to_owned(),
    ];
    assert_eq!(calls, expected);
}

#[test]
fn other_profiles_transmit_no_names() {
    // Server scritto a mano: si cattura ogni byte scritto dall'helper.
    let name = test_pipe_name();
    let raw = RawServer::create(&name);
    let n2 = name.clone();
    let helper = thread::spawn(move || serve_connected(Client::connect(&n2, me()).unwrap(), &Fake::new(&own_sid())));
    raw.accept();
    raw.write(&frame(&encode_request(&e2e_request())));
    let bytes = raw.read_to_end();
    assert_eq!(helper.join().unwrap(), exit::OK);
    check_e2e_replies(&replies_in(&bytes));
    // Controllo della ricerca: il nome dell'albero trasmesso c'è.
    assert!(contains(&bytes, &utf16le("visibile.txt")));
    for secret in SECRETS {
        assert!(!contains(&bytes, &utf16le(secret)), "{secret} in UTF-16");
        assert!(!contains(&bytes, secret.as_bytes()), "{secret} in ASCII");
    }
}

#[test]
fn an_invalid_request_gets_no_reply() {
    let name = test_pipe_name();
    let raw = RawServer::create(&name);
    let n2 = name.clone();
    let helper = thread::spawn(move || {
        let fake = Fake::new(&own_sid());
        let code = serve_connected(Client::connect(&n2, me()).unwrap(), &fake);
        (code, fake.calls())
    });
    raw.accept();
    let bad = encode_request(&Request { volume: r"Q:\".into(), folders: vec![r"Q:\a".into(), r"Q:\A\b".into()] });
    raw.write(&frame(&bad));
    assert!(raw.read_to_end().is_empty(), "nessuna risposta");
    let (code, calls) = helper.join().unwrap();
    assert_eq!(code, exit::REQUEST);
    assert!(calls.is_empty(), "{calls:?}");

    // Pipe chiusa prima della richiesta: errore della pipe.
    let name = test_pipe_name();
    let raw = RawServer::create(&name);
    let n2 = name.clone();
    let helper = thread::spawn(move || serve_connected(Client::connect(&n2, me()).unwrap(), &Fake::new(&own_sid())));
    raw.accept();
    drop(raw);
    assert_eq!(helper.join().unwrap(), exit::PIPE);
}

#[test]
fn done_reports_a_missing_privilege() {
    let name = test_pipe_name();
    let raw = RawServer::create(&name);
    let n2 = name.clone();
    let helper = thread::spawn(move || {
        let fake = Fake { privilege: Err(1300), ..Fake::new(&own_sid()) };
        serve_connected(Client::connect(&n2, me()).unwrap(), &fake)
    });
    raw.accept();
    raw.write(&frame(&encode_request(&Request { volume: r"Q:\".into(), folders: vec![r"Q:\Users\altro".into()] })));
    let replies = replies_in(&raw.read_to_end());
    assert_eq!(helper.join().unwrap(), exit::OK);
    assert!(matches!(replies.first(), Some(Reply::Started { backup_privilege: false })));
    assert!(matches!(replies.last(), Some(Reply::Done { code: 1300 })));
}

#[test]
fn parent_and_location_checks() {
    // Sé stessi e il pid 0 non sono il genitore (il caso positivo è la prova di `run_with` nel figlio).
    assert!(parent_process(me()).is_none());
    assert!(parent_process(0).is_none());

    // L'eseguibile di prova sta nel worktree, non in Download o in una cartella temporanea.
    assert!(!exe_location_refused(), "{:?}", std::env::current_exe());
    let dirs = forbidden_dirs();
    let temp = std::env::temp_dir();
    assert!(inside_any(&temp.join("quado.exe").to_string_lossy(), &dirs), "{dirs:?}");
    assert!(inside_any(&temp.join(r"sotto\quado.exe").to_string_lossy().to_uppercase(), &dirs), "senza maiuscole");
    if let Ok(profile) = std::env::var("USERPROFILE") {
        assert!(inside_any(&format!(r"{profile}\Downloads\quado.exe"), &dirs) || !std::path::Path::new(&format!(r"{profile}\Downloads")).exists(), "{dirs:?}");
        assert!(inside_any(&format!(r"{profile}\AppData\Local\Temp\x\quado.exe"), &dirs), "{dirs:?}");
        assert!(!inside_any(&format!(r"{profile}\AppData\Local\Programs\Quado\quado.exe"), &dirs), "{dirs:?}");
    }
    let windows = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    assert!(inside_any(&format!(r"{windows}\Temp\quado.exe"), &dirs), "{dirs:?}");
    assert!(!inside_any(&format!(r"{windows}\System32\quado.exe"), &dirs), "{dirs:?}");
    // Nomi brevi e junction: la cartella temporanea compare anche come percorso finale.
    let fin = final_path(&temp.to_string_lossy(), true).expect("percorso finale della cartella temporanea");
    assert!(dirs.iter().any(|d| d.eq_ignore_ascii_case(&fin)), "{fin} in {dirs:?}");
}

#[test]
fn a_junction_does_not_turn_another_profile_into_a_tree() {
    // Junction nella cartella temporanea (dentro il profilo dell'utente) verso `C:\Users\Public`: il
    // percorso chiesto sembra del profilo proprio, quello finale è di un altro profilo. Solo attributi
    // letti fuori dalla cartella di prova.
    let profile = std::env::var("USERPROFILE").expect("USERPROFILE");
    let public = std::path::Path::new(r"C:\Users\Public\Documents");
    if !public.exists() {
        eprintln!("C:\\Users\\Public\\Documents assente: prova non eseguita");
        return;
    }
    let d = TestDir::new("helper-junction");
    let link = d.join("collegamento");
    assert!(trees::junction(&link, std::path::Path::new(r"C:\Users\Public")), "mklink /J");
    let asked = link.join("Documents").to_string_lossy().into_owned();
    assert!(asked.to_ascii_lowercase().starts_with(&profile.to_ascii_lowercase()), "{asked}");
    let sid = own_sid();
    assert_eq!(classify(r"C:\", &asked, Some(&profile), Some(&sid)), Scope::Tree, "il solo percorso chiesto non basta");
    assert_eq!(final_path(&asked, false).as_deref(), Some(r"C:\Users\Public\Documents"));
    assert_eq!(scope_of(r"C:\", &asked, Some(&profile), Some(&sid)), Scope::Profile);
    // Un percorso senza alias resta com'è.
    let plain_dir = d.join("normale");
    std::fs::create_dir(&plain_dir).unwrap();
    assert_eq!(scope_of(r"C:\", &plain_dir.to_string_lossy(), Some(&profile), Some(&sid)), Scope::Tree);
    // La junction si toglie da sola, senza toccare la destinazione.
    std::fs::remove_dir(&link).unwrap();
    assert!(public.exists());
}
