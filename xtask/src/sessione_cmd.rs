// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask sessione-prova [--senza-build] [--exe EXE] [--escludi "a;b"]` (06 §10, righe "Sessione senza rete", "Dati
//! salvati", "Zone.Identifier"): build di release, poi `quado --sessione-prova` come processo figlio e,
//! per tutta la sessione, un campione ogni 120-200 ms di:
//!
//! - connessioni TCP e UDP del processo, IPv4 e IPv6 (`GetExtendedTcpTable`, `GetExtendedUdpTable` con il
//!   pid proprietario): attese zero;
//! - handle di tipo File del processo (`NtQuerySystemInformation` con `SystemExtendedHandleInformation`),
//!   con il nome letto da un duplicato (`DuplicateHandle`, `GetFinalPathNameByHandleW`, `NtQueryObject`)
//!   in un thread con un tempo massimo, perché su una pipe sincrona la richiesta del nome può bloccarsi.
//!   Si segnalano: handle in scrittura (`WRITE_BITS`) su file fuori da `%APPDATA%\Quado` e dal Cestino;
//!   nomi con `:Zone.Identifier`; dispositivi di rete (`\Device\Afd`, `\Device\Tcp`, `\Device\Udp`,
//!   `\Device\RawIp`); percorsi con una parte di `--escludi` (passata anche a Quado, che non li legge mai:
//!   sono ammesse solo le cartelle elencate dalla scansione, in sola lettura).
//!
//! Lo stato della rete (adattatori attivi con gateway, `GetAdaptersAddresses`) decide il resoconto:
//! `target/misure/sessione-rete-presente.md` o `target/misure/sessione-rete-assente.md`. Il comando esce con un errore
//! se un controllo fallisce.
//!
//! Limite dichiarato: il campionamento può perdere aperture brevissime (tra due campioni); la prova
//! completa con Process Monitor o una traccia ETW resta da fare a mano.

// Chiamate native per handle, connessioni e adattatori: ogni blocco ha il suo invariante.
#![allow(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::ffi::c_void;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use quado_scan_win::win::WRITE_BITS;
use windows_sys::Wdk::Foundation::NtQueryObject;
use windows_sys::Wdk::System::SystemInformation::NtQuerySystemInformation;
use windows_sys::Wdk::Storage::FileSystem::{FileFsDeviceInformation, NtQueryVolumeInformationFile};
use windows_sys::Win32::Foundation::{CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE, HANDLE_FLAG_INHERIT, SetHandleInformation, UNICODE_STRING};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses, GetExtendedTcpTable,
    GetExtendedUdpTable, IP_ADAPTER_ADDRESSES_LH, TCP_TABLE_OWNER_PID_ALL, UDP_TABLE_OWNER_PID,
};
use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_NAME_NORMALIZED, FILE_TYPE_DISK, GetFileInformationByHandle, GetFileType,
    GetFinalPathNameByHandleW, VOLUME_NAME_DOS,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE, PROCESS_QUERY_LIMITED_INFORMATION};

/// Intervallo cercato tra l'inizio di due campioni (un campione costa qualche decina di millisecondi: la
/// tabella degli handle è quella di tutto il sistema). La media misurata va nel resoconto.
const INTERVAL: Duration = Duration::from_millis(120);
/// Tempo massimo per il nome di un handle.
const NAME_TIMEOUT: Duration = Duration::from_millis(250);
/// Tempo massimo della sessione: oltre, il processo si ferma e la prova fallisce.
const SESSION_LIMIT: Duration = Duration::from_secs(45 * 60);
/// Parti di percorso mai lette (`--escludi`, in minuscolo), le stesse passate alla sessione di Quado.
static EXCLUDED: OnceLock<Vec<String>> = OnceLock::new();
/// Dispositivi dei socket e dei protocolli IP (nomi NT, minuscoli).
const NET_DEVICES: [&str; 4] = [r"\device\afd", r"\device\tcp", r"\device\udp", r"\device\rawip"];
/// Passi della sessione, nell'ordine (`quado/src/session.rs`).
const STEPS: [&str; 10] = ["scansione", "panoramica", "esplora", "ricerca", "file-grandi", "download", "programmi", "intervento", "impostazioni", "fine"];

struct Options {
    build: bool,
    exe: PathBuf,
    exclude: Vec<String>,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options { build: true, exe: PathBuf::from(crate::importazioni_cmd::DEFAULT_EXE), exclude: Vec::new() };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--senza-build" => o.build = false,
            "--exe" => {
                o.exe = it.next().map(PathBuf::from).ok_or("--exe: manca il valore")?;
                o.build = false;
            }
            "--escludi" => o.exclude = it.next().ok_or("--escludi: manca il valore")?.split(';').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect(),
            other => return Err(format!("argomento sconosciuto: {other}\nuso: cargo xtask sessione-prova [--senza-build] [--exe EXE] [--escludi \"a;b\"]")),
        }
    }
    Ok(o)
}

// ---------------------------------------------------------------------------------------------
// Handle

/// Voce di `SYSTEM_HANDLE_INFORMATION_EX` (40 byte su x64).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Entry {
    object: usize,
    pid: usize,
    value: usize,
    access: u32,
    back_trace: u16,
    object_type: u16,
    attributes: u32,
    reserved: u32,
}

const SYSTEM_EXTENDED_HANDLE_INFORMATION: i32 = 64;
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;

/// Tabella degli handle di tutto il sistema, con il buffer riusato tra i campioni.
struct Snapshot {
    buf: Vec<u64>,
}

impl Snapshot {
    fn new() -> Self {
        Self { buf: vec![0; 1 << 20] }
    }

    fn query(&mut self) -> Result<&[Entry], String> {
        loop {
            let bytes = u32::try_from(self.buf.len() * 8).map_err(|_| "tabella degli handle troppo grande")?;
            let mut needed = 0u32;
            // SAFETY: buffer allineato a 8 e grande `bytes` byte; la funzione scrive al più `bytes` byte e, se
            // non bastano, restituisce STATUS_INFO_LENGTH_MISMATCH con la dimensione necessaria.
            let status = unsafe { NtQuerySystemInformation(SYSTEM_EXTENDED_HANDLE_INFORMATION, self.buf.as_mut_ptr().cast(), bytes, &mut needed) };
            if status == STATUS_INFO_LENGTH_MISMATCH {
                let want = (needed as usize).max(self.buf.len() * 8 * 2);
                if want > 1 << 30 {
                    return Err("tabella degli handle oltre 1 GiB".into());
                }
                self.buf = vec![0; want / 8 + (1 << 17)];
                continue;
            }
            if status < 0 {
                return Err(format!("NtQuerySystemInformation: {status:#x}"));
            }
            let n = self.buf[0] as usize;
            let max = (self.buf.len() * 8 - 16) / size_of::<Entry>();
            // SAFETY: SYSTEM_HANDLE_INFORMATION_EX inizia con due ULONG_PTR (numero di voci e riservato), poi
            // `n` voci da 40 byte scritte dal sistema nel buffer, limitate a `max`; il buffer è allineato a 8.
            return Ok(unsafe { std::slice::from_raw_parts(self.buf.as_ptr().add(2).cast::<Entry>(), n.min(max)) });
        }
    }
}

/// Indice del tipo "File" in questa versione di Windows: quello di un file aperto qui.
fn file_type_index(snap: &mut Snapshot) -> Result<u16, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let probe = std::fs::File::open(&exe).map_err(|e| e.to_string())?;
    let value = probe.as_raw_handle() as usize;
    let me = std::process::id() as usize;
    let t = snap.query()?.iter().find(|e| e.pid == me && e.value == value).map(|e| e.object_type);
    drop(probe);
    t.ok_or_else(|| "handle di prova non trovato nella tabella del sistema".into())
}

/// Cosa si sa di un handle di tipo File del processo.
#[derive(Clone, Debug, Default)]
struct Info {
    /// Chiuso prima del duplicato.
    gone: bool,
    /// Il nome non è arrivato entro il tempo massimo.
    blocked: bool,
    /// Riferisce lo stesso oggetto di un handle di xtask prima dell'avvio: ereditato, non aperto da Quado.
    inherited: bool,
    disk: bool,
    directory: bool,
    /// Tipo di dispositivo (`FileFsDeviceInformation`): 0x11 pipe, 0x12 rete (AFD), 0x15 null, 0x50 console.
    device_type: Option<u32>,
    /// Percorso DOS (`GetFinalPathNameByHandleW`), solo per i file su disco.
    dos: Option<String>,
    /// Nome NT (`NtQueryObject`): `\Device\HarddiskVolume3\...`, `\Device\Afd`, `\Device\NamedPipe\...`.
    nt: Option<String>,
}

/// `FILE_DEVICE_NETWORK`: il dispositivo dei socket (AFD).
const FILE_DEVICE_NETWORK: u32 = 0x12;

impl Info {
    fn names(&self) -> impl Iterator<Item = &str> {
        self.dos.as_deref().into_iter().chain(self.nt.as_deref())
    }

    fn shown(&self) -> String {
        self.dos.as_deref().or(self.nt.as_deref()).map_or_else(|| "(senza nome)".to_owned(), tidy)
    }

    /// Socket o dispositivo di un protocollo IP.
    fn network(&self) -> bool {
        self.nt.as_deref().is_some_and(is_net_device) || self.device_type == Some(FILE_DEVICE_NETWORK)
    }
}

fn object_name(h: HANDLE) -> Option<String> {
    let mut buf: Vec<u64> = vec![0; 1024];
    for _ in 0..2 {
        let bytes = (buf.len() * 8) as u32;
        let mut ret = 0u32;
        // SAFETY: `h` è un handle valido di questo processo; buffer allineato a 8 e grande `bytes`. Per
        // ObjectNameInformation (1) il sistema scrive un UNICODE_STRING seguito dai caratteri del nome.
        let status = unsafe { NtQueryObject(h, 1, buf.as_mut_ptr().cast(), bytes, &mut ret) };
        if status < 0 {
            if ret as usize > buf.len() * 8 && ret < 1 << 20 {
                buf = vec![0; ret as usize / 8 + 8];
                continue;
            }
            return None;
        }
        // SAFETY: il buffer contiene un UNICODE_STRING all'inizio, allineato.
        let us: UNICODE_STRING = unsafe { std::ptr::read(buf.as_ptr().cast::<UNICODE_STRING>()) };
        let len = us.Length as usize / 2;
        let (start, end) = (buf.as_ptr() as usize, buf.as_ptr() as usize + buf.len() * 8);
        let p = us.Buffer as usize;
        if us.Buffer.is_null() || len == 0 || p < start || p + len * 2 > end {
            return None;
        }
        // SAFETY: `len` caratteri a `p`, dentro il buffer (controllato sopra).
        let s = unsafe { std::slice::from_raw_parts(us.Buffer, len) };
        return Some(String::from_utf16_lossy(s));
    }
    None
}

fn final_path(h: HANDLE) -> Option<String> {
    let mut buf = vec![0u16; 32_768];
    // SAFETY: `h` è un handle valido di questo processo; buffer di `buf.len()` caratteri dichiarato.
    let n = unsafe { GetFinalPathNameByHandleW(h, buf.as_mut_ptr(), buf.len() as u32, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS) } as usize;
    (n > 0 && n < buf.len()).then(|| String::from_utf16_lossy(&buf[..n]))
}

fn device_type(h: HANDLE) -> Option<u32> {
    let mut iosb = IO_STATUS_BLOCK::default();
    // FILE_FS_DEVICE_INFORMATION: DeviceType e Characteristics.
    let mut out = [0u32; 2];
    // SAFETY: `h` è un handle valido di questo processo; struttura di uscita locale di 8 byte dichiarata.
    let status = unsafe { NtQueryVolumeInformationFile(h, &mut iosb, out.as_mut_ptr().cast(), 8, FileFsDeviceInformation) };
    (status >= 0).then_some(out[0])
}

/// Accesso concesso e nome del tipo di un handle di questo processo (`ObjectBasicInformation`,
/// `ObjectTypeInformation`): domande sull'oggetto, senza I/O, che non si bloccano.
fn access_and_type(h: HANDLE) -> Option<(u32, String)> {
    // OBJECT_BASIC_INFORMATION: Attributes, GrantedAccess, HandleCount, PointerCount, Reserved[10].
    let mut basic = [0u32; 14];
    let mut ret = 0u32;
    // SAFETY: `h` è valido; struttura di uscita locale di 56 byte dichiarata.
    let status = unsafe { NtQueryObject(h, 0, basic.as_mut_ptr().cast(), 56, &mut ret) };
    if status < 0 {
        return None;
    }
    let mut buf: Vec<u64> = vec![0; 128];
    // SAFETY: `h` è valido; buffer allineato a 8 e grande quanto dichiarato. Per ObjectTypeInformation (2) il
    // sistema scrive un UNICODE_STRING con il nome del tipo, seguito dai caratteri, dentro il buffer.
    let status = unsafe { NtQueryObject(h, 2, buf.as_mut_ptr().cast(), (buf.len() * 8) as u32, &mut ret) };
    if status < 0 {
        return None;
    }
    // SAFETY: il buffer contiene un UNICODE_STRING all'inizio, allineato.
    let us: UNICODE_STRING = unsafe { std::ptr::read(buf.as_ptr().cast::<UNICODE_STRING>()) };
    let len = us.Length as usize / 2;
    let (start, end) = (buf.as_ptr() as usize, buf.as_ptr() as usize + buf.len() * 8);
    let p = us.Buffer as usize;
    if us.Buffer.is_null() || p < start || p + len * 2 > end {
        return None;
    }
    // SAFETY: `len` caratteri a `p`, dentro il buffer (controllato sopra).
    let name = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(us.Buffer, len) });
    Some((basic[1], name))
}

/// Duplica un handle del processo figlio in questo processo: subito dopo il campione, così il file resta
/// leggibile per il nome anche se il figlio lo chiude. `None` se l'handle è già chiuso, o se il suo valore
/// è già stato riusato per un altro oggetto (tipo diverso da File o accesso diverso da quello del campione).
fn duplicate(process: usize, value: usize, access: u32) -> Option<usize> {
    let mut dup: HANDLE = std::ptr::null_mut();
    // SAFETY: `process` è un handle del processo figlio aperto con PROCESS_DUP_HANDLE e valido per tutto il
    // comando; `value` è un valore di handle di quel processo (se è già chiuso la funzione fallisce). Il
    // duplicato appartiene a questo processo e lo chiude `describe`, o questa funzione se non serve.
    let ok = unsafe { DuplicateHandle(process as HANDLE, value as HANDLE, GetCurrentProcess(), &mut dup, 0, 0, DUPLICATE_SAME_ACCESS) };
    if ok == 0 {
        return None;
    }
    match access_and_type(dup) {
        Some((a, t)) if a == access && t == "File" => Some(dup as usize),
        _ => {
            // SAFETY: duplicato aperto sopra e non passato a nessuno: si chiude una volta, qui.
            unsafe { CloseHandle(dup) };
            None
        }
    }
}

/// Nome e tipo da un duplicato, poi chiuso.
fn describe(dup: usize) -> Info {
    let h = dup as HANDLE;
    // SAFETY: `h` è un duplicato valido di questo processo (da `duplicate`).
    let disk = unsafe { GetFileType(h) } == FILE_TYPE_DISK;
    let mut info = Info { disk, device_type: device_type(h), nt: object_name(h), ..Default::default() };
    if disk {
        info.dos = final_path(h);
        let mut bh = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: `h` è valido; la struttura è locale.
        if unsafe { GetFileInformationByHandle(h, &mut bh) } != 0 {
            info.directory = bh.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
        }
    }
    // SAFETY: duplicato aperto da `duplicate`, chiuso una volta sola, qui.
    unsafe { CloseHandle(h) };
    info
}

/// Coda tra il campionamento e i thread dei nomi.
#[derive(Default)]
struct Shared {
    queue: Mutex<VecDeque<(Key, usize)>>,
    ready: Condvar,
    results: Mutex<Vec<(Key, Info)>>,
    /// Lavoro in corso per thread: chiave e inizio.
    busy: Mutex<HashMap<usize, (Key, Instant)>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Nomi degli handle fuori dal ciclo del campionamento, che così mantiene il suo intervallo. Un thread fermo
/// da più di `NAME_TIMEOUT` su un handle (per esempio una pipe sincrona) si abbandona con il suo duplicato
/// e ne parte un altro, che continua la coda.
struct Namer {
    shared: Arc<Shared>,
    next_id: usize,
    abandoned: HashSet<usize>,
    stuck: usize,
}

impl Namer {
    fn new() -> Self {
        let mut n = Self { shared: Arc::default(), next_id: 0, abandoned: HashSet::new(), stuck: 0 };
        n.spawn();
        n
    }

    fn spawn(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        let sh = Arc::clone(&self.shared);
        let _ = std::thread::Builder::new().name("sessione-nomi".into()).spawn(move || {
            loop {
                let (key, dup) = {
                    let mut q = lock(&sh.queue);
                    loop {
                        if let Some(item) = q.pop_front() {
                            break item;
                        }
                        q = sh.ready.wait(q).unwrap_or_else(PoisonError::into_inner);
                    }
                };
                lock(&sh.busy).insert(id, (key, Instant::now()));
                let info = describe(dup);
                lock(&sh.busy).remove(&id);
                lock(&sh.results).push((key, info));
            }
        });
    }

    fn submit(&self, key: Key, dup: usize) {
        lock(&self.shared.queue).push_back((key, dup));
        self.shared.ready.notify_one();
    }

    /// Nomi arrivati (anche in ritardo, dopo un abbandono) e, per i thread fermi, il nome mancante.
    fn collect(&mut self) -> Vec<(Key, Info)> {
        let mut out = Vec::new();
        let stalled: Vec<(usize, Key)> =
            lock(&self.shared.busy).iter().filter(|(id, (_, t))| !self.abandoned.contains(*id) && t.elapsed() > NAME_TIMEOUT).map(|(id, (k, _))| (*id, *k)).collect();
        for (id, key) in stalled {
            self.abandoned.insert(id);
            self.stuck += 1;
            out.push((key, Info { blocked: true, ..Default::default() }));
            self.spawn();
        }
        out.append(&mut lock(&self.shared.results));
        out
    }

    /// Nomi ancora da leggere, esclusi quelli dei thread abbandonati.
    fn pending(&self) -> bool {
        !lock(&self.shared.queue).is_empty() || lock(&self.shared.busy).keys().any(|id| !self.abandoned.contains(id))
    }
}

/// Un handle osservato: valore, oggetto e accesso (un valore riusato per un altro file è un altro handle).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Key {
    value: usize,
    object: usize,
    access: u32,
}

struct Seen {
    info: Info,
    /// Il nome non è ancora arrivato.
    waiting: bool,
    /// Passo della sessione al primo campione che l'ha visto.
    step: String,
    samples: usize,
}

/// Toglie l'ereditarietà a tutti gli handle di xtask prima di avviare Quado: il processo figlio riceve
/// solo le sue pipe (create dopo, durante l'avvio), non i file aperti da chi ha lanciato xtask (per esempio
/// un file di uscita rediretto), che altrimenti comparirebbero tra i suoi handle in scrittura.
fn stop_inheritance() -> usize {
    let mut n = 0;
    for h in quado_scan_win::process::own_handles() {
        // SAFETY: `h.value` è un handle di questo processo (dalla sua tabella); cambia solo il flag di
        // ereditarietà, che vale per i processi creati dopo.
        n += usize::from(unsafe { SetHandleInformation(h.value as HANDLE, HANDLE_FLAG_INHERIT, 0) } != 0);
    }
    n
}

/// Scritture attese dal sottosistema grafico caricato nel processo (non dal codice di Quado): cache degli
/// shader di Direct3D e dei driver, overlay di cattura del driver NVIDIA. Si riportano a parte.
const GRAPHICS: [(&str, &str); 7] = [
    (r"\d3dscache\", "cache degli shader di Direct3D (Windows)"),
    (r"\nvidia\dxcache\", "cache degli shader del driver NVIDIA"),
    (r"\nvidia\glcache\", "cache degli shader del driver NVIDIA"),
    (r"\nvidia corporation\shadowplay\", "registro dell'overlay di cattura NVIDIA (ShadowPlay)"),
    (r"\intel\shadercache\", "cache degli shader del driver Intel"),
    (r"\amd\dxcache\", "cache degli shader del driver AMD"),
    (r"\amd\glcache\", "cache degli shader del driver AMD"),
];

fn graphics(name: &str) -> Option<&'static str> {
    let n = name.to_lowercase();
    GRAPHICS.iter().find(|(p, _)| n.contains(p)).map(|(_, why)| *why)
}

// ---------------------------------------------------------------------------------------------
// Connessioni e rete

#[derive(Clone, Copy)]
enum Table {
    Tcp(u16),
    Udp(u16),
}

fn ip_table(t: Table) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u64> = vec![0; 4096];
    loop {
        let mut size = (buf.len() * 8) as u32;
        // SAFETY: buffer allineato a 8 e grande `size` byte; se non basta la funzione restituisce
        // ERROR_INSUFFICIENT_BUFFER (122) con la dimensione necessaria in `size`.
        let r = unsafe {
            match t {
                Table::Tcp(af) => GetExtendedTcpTable(buf.as_mut_ptr().cast::<c_void>(), &mut size, 0, u32::from(af), TCP_TABLE_OWNER_PID_ALL, 0),
                Table::Udp(af) => GetExtendedUdpTable(buf.as_mut_ptr().cast::<c_void>(), &mut size, 0, u32::from(af), UDP_TABLE_OWNER_PID, 0),
            }
        };
        match r {
            0 => return Ok(buf.iter().flat_map(|w| w.to_le_bytes()).collect()),
            122 if (size as usize) < 64 << 20 => buf = vec![0; size as usize / 8 + 1024],
            e => return Err(format!("tabella delle connessioni: errore {e}")),
        }
    }
}

fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn port(b: &[u8], o: usize) -> u16 {
    // DWORD con la porta nell'ordine di rete nei due byte bassi.
    u16::from_be_bytes([b[o], b[o + 1]])
}

fn v4(b: &[u8], o: usize) -> String {
    std::net::Ipv4Addr::new(b[o], b[o + 1], b[o + 2], b[o + 3]).to_string()
}

fn v6(b: &[u8], o: usize) -> String {
    let mut a = [0u8; 16];
    a.copy_from_slice(&b[o..o + 16]);
    format!("[{}]", std::net::Ipv6Addr::from(a))
}

/// Righe di una tabella `*_OWNER_PID` (numero di voci, poi righe da `row` byte) del processo `pid`.
fn rows(b: &[u8], row: usize, pid_at: usize, pid: u32) -> Vec<usize> {
    if b.len() < 4 {
        return Vec::new();
    }
    let n = (le32(b, 0) as usize).min((b.len() - 4) / row);
    (0..n).map(|i| 4 + i * row).filter(|&o| le32(b, o + pid_at) == pid).collect()
}

/// Connessioni ed endpoint TCP e UDP, IPv4 e IPv6, del processo `pid`.
fn connections(pid: u32) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let b = ip_table(Table::Tcp(AF_INET))?;
    for o in rows(&b, 24, 20, pid) {
        out.push(format!("TCP {}:{} verso {}:{} (stato {})", v4(&b, o + 4), port(&b, o + 8), v4(&b, o + 12), port(&b, o + 16), le32(&b, o)));
    }
    let b = ip_table(Table::Tcp(AF_INET6))?;
    for o in rows(&b, 56, 52, pid) {
        out.push(format!("TCP {}:{} verso {}:{} (stato {})", v6(&b, o), port(&b, o + 20), v6(&b, o + 24), port(&b, o + 44), le32(&b, o + 48)));
    }
    let b = ip_table(Table::Udp(AF_INET))?;
    for o in rows(&b, 12, 8, pid) {
        out.push(format!("UDP {}:{}", v4(&b, o), port(&b, o + 4)));
    }
    let b = ip_table(Table::Udp(AF_INET6))?;
    for o in rows(&b, 28, 24, pid) {
        out.push(format!("UDP {}:{}", v6(&b, o), port(&b, o + 20)));
    }
    Ok(out)
}

/// Un adattatore di rete: nome, tipo, attivo, con un gateway.
#[derive(Clone, Debug)]
struct Adapter {
    name: String,
    kind: u32,
    up: bool,
    gateway: bool,
}

fn wide(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    // SAFETY: stringa terminata da zero scritta da GetAdaptersAddresses nel suo buffer; al più 512 caratteri.
    while n < 512 && unsafe { *p.add(n) } != 0 {
        n += 1;
    }
    // SAFETY: `n` caratteri letti sopra.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, n) })
}

fn adapters() -> Result<Vec<Adapter>, String> {
    let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut buf: Vec<u64> = vec![0; 8192];
    loop {
        let mut size = (buf.len() * 8) as u32;
        // SAFETY: buffer allineato a 8 e grande `size` byte; se non basta la funzione restituisce
        // ERROR_BUFFER_OVERFLOW (111) con la dimensione necessaria.
        let r = unsafe { GetAdaptersAddresses(u32::from(AF_UNSPEC), flags, std::ptr::null(), buf.as_mut_ptr().cast(), &mut size) };
        match r {
            0 => break,
            111 if (size as usize) < 16 << 20 => buf = vec![0; size as usize / 8 + 1024],
            232 => return Ok(Vec::new()),
            e => return Err(format!("GetAdaptersAddresses: errore {e}")),
        }
    }
    let mut out = Vec::new();
    let mut p = buf.as_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
    while !p.is_null() && out.len() < 256 {
        // SAFETY: elenco collegato scritto da GetAdaptersAddresses dentro `buf`, che vive fino alla fine.
        let a = unsafe { &*p };
        out.push(Adapter { name: wide(a.FriendlyName), kind: a.IfType, up: a.OperStatus == IfOperStatusUp, gateway: !a.FirstGatewayAddress.is_null() });
        p = a.Next;
    }
    Ok(out)
}

fn kind_name(t: u32) -> &'static str {
    match t {
        6 => "Ethernet",
        71 => "Wi-Fi",
        24 => "loopback",
        23 => "PPP",
        131 => "tunnel",
        243 | 244 => "rete mobile",
        _ => "altro",
    }
}

/// Adattatori attivi con un gateway (loopback esclusa): la rete c'è.
fn online(a: &[Adapter]) -> Vec<&Adapter> {
    a.iter().filter(|a| a.up && a.gateway && a.kind != 24).collect()
}

// ---------------------------------------------------------------------------------------------
// Percorsi

fn env_lower(var: &str) -> Option<String> {
    std::env::var(var).ok().map(|v| v.trim_end_matches('\\').to_lowercase())
}

/// Senza `\\?\`, con `%APPDATA%`, `%LOCALAPPDATA%` e `%USERPROFILE%` al posto dei prefissi.
fn tidy(p: &str) -> String {
    let p = p.strip_prefix(r"\\?\").unwrap_or(p);
    let lower = p.to_lowercase();
    for var in ["APPDATA", "LOCALAPPDATA", "USERPROFILE"] {
        if let Some(v) = env_lower(var)
            && lower.starts_with(&v)
            && lower[v.len()..].is_empty() | lower[v.len()..].starts_with('\\')
        {
            return format!("%{var}%{}", &p[v.len()..]);
        }
    }
    p.to_owned()
}

/// Scrittura ammessa: dati di Quado o un Cestino.
fn write_allowed(name: &str) -> bool {
    let p = name.strip_prefix(r"\\?\").unwrap_or(name).to_lowercase();
    let data = env_lower("APPDATA").map(|a| format!(r"{a}\quado"));
    data.is_some_and(|d| p == d || p.starts_with(&format!(r"{d}\"))) || p.contains(r"\$recycle.bin\") || p.ends_with(r"\$recycle.bin")
}

fn is_zone(name: &str) -> bool {
    name.to_lowercase().contains(":zone.identifier")
}

fn is_net_device(nt: &str) -> bool {
    let n = nt.to_lowercase();
    NET_DEVICES.iter().any(|d| n == *d || n.starts_with(&format!(r"{d}\")) || n.starts_with(&format!("{d}6")))
}

fn is_excluded(name: &str) -> bool {
    let n = name.to_lowercase();
    EXCLUDED.get().is_some_and(|parts| parts.iter().any(|x| n.contains(x.as_str())))
}

/// `\Device\NamedPipe\...` → `\Device\NamedPipe`.
fn device(nt: &str) -> String {
    nt.split('\\').take(3).collect::<Vec<_>>().join("\\")
}

// ---------------------------------------------------------------------------------------------
// Sessione

/// Righe della sessione lette da standard output.
#[derive(Default)]
struct Lines {
    steps: Vec<(String, u64, String)>,
    details: Vec<(String, String)>,
    errors: Vec<(String, String)>,
    summary: BTreeMap<String, String>,
    other: usize,
}

impl Lines {
    fn add(&mut self, line: &str) {
        let f: Vec<&str> = line.split('\t').collect();
        match f.as_slice() {
            ["passo", name, ms, esito] => self.steps.push((name.to_string(), ms.parse().unwrap_or(0), esito.to_string())),
            ["dettaglio", name, text] => self.details.push((name.to_string(), text.to_string())),
            ["errore", name, text] => self.errors.push((name.to_string(), text.to_string())),
            ["riepilogo", k, v] => {
                self.summary.insert(k.to_string(), v.to_string());
            }
            ["inizio", _] | ["sessione", ..] | ["esclusioni", ..] => {}
            _ => self.other += 1,
        }
    }
}

enum Event {
    Out(String),
    Err(String),
}

fn reader<R: std::io::Read + Send + 'static>(r: R, tx: Sender<Event>, err: bool) {
    let _ = std::thread::Builder::new().name("sessione-uscita".into()).spawn(move || {
        for line in BufReader::new(r).lines().map_while(Result::ok) {
            let _ = tx.send(if err { Event::Err(line) } else { Event::Out(line) });
        }
    });
}

#[derive(Default)]
struct Observed {
    samples: usize,
    conn_samples: usize,
    conn_errors: BTreeSet<String>,
    handle_errors: BTreeSet<String>,
    connections: BTreeMap<String, String>,
    handles: HashMap<Key, Seen>,
    max_file_handles: usize,
}

pub fn run(args: &[String]) -> Result<(), String> {
    let o = parse(args)?;
    let _ = EXCLUDED.set(o.exclude.clone());
    if o.build {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let s = Command::new(&cargo).args(["build", "--release", "--quiet", "-p", "quado", "--features", "misure"]).status().map_err(|e| e.to_string())?;
        if !s.success() {
            return Err("build di quado fallita".into());
        }
    }
    if !o.exe.is_file() {
        return Err(format!("{} assente", o.exe.display()));
    }
    let env = crate::measure_cmd::environment();
    let net_before = adapters()?;
    let mut snap = Snapshot::new();
    let file_type = file_type_index(&mut snap)?;
    let cleared = stop_inheritance();
    let me = std::process::id() as usize;
    let inherited: HashSet<usize> = snap.query()?.iter().filter(|e| e.pid == me && e.object != 0).map(|e| e.object).collect();
    eprintln!("tipo File degli oggetti: {file_type}; handle di xtask non più ereditabili: {cleared}; rete: {} adattatori attivi con gateway", online(&net_before).len());

    let started = Instant::now();
    let mut cmd = Command::new(&o.exe);
    cmd.arg("--sessione-prova");
    if !o.exclude.is_empty() {
        cmd.args(["--escludi", &o.exclude.join(";")]);
    }
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let pid = child.id();
    let (tx, rx): (Sender<Event>, Receiver<Event>) = mpsc::channel();
    if let Some(out) = child.stdout.take() {
        reader(out, tx.clone(), false);
    }
    if let Some(err) = child.stderr.take() {
        reader(err, tx, true);
    }
    // SAFETY: nessun prerequisito; il valore restituito si controlla e resta aperto fino alla fine del
    // comando (lo usano i thread dei nomi, anche uno bloccato).
    let process = unsafe { OpenProcess(PROCESS_DUP_HANDLE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) } as usize;
    if process == 0 {
        let _ = child.kill();
        return Err("OpenProcess sul processo della sessione non riuscito".into());
    }
    let mut sampler = Sampler { snap, namer: Namer::new(), process, file_type, pid, inherited };
    let mut lines = Lines::default();
    let mut stderr_lines: Vec<String> = Vec::new();
    let mut step = "avvio".to_owned();
    let mut seen = Observed::default();
    let mut timed_out = false;
    let status = loop {
        let t0 = Instant::now();
        while let Ok(ev) = rx.try_recv() {
            match ev {
                Event::Out(l) => {
                    if let Some(s) = l.strip_prefix("inizio\t") {
                        s.clone_into(&mut step);
                    }
                    println!("{l}");
                    lines.add(&l);
                }
                Event::Err(l) => stderr_lines.push(l),
            }
        }
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        if started.elapsed() > SESSION_LIMIT {
            timed_out = true;
            let _ = child.kill();
            break child.wait().map_err(|e| e.to_string())?;
        }
        sampler.sample(&mut seen, &step);
        if let Some(rest) = INTERVAL.checked_sub(t0.elapsed()) {
            std::thread::sleep(rest);
        }
    };
    let duration = started.elapsed();
    sampler.finish(&mut seen);
    // Le ultime righe arrivano dopo la fine del processo.
    let deadline = Instant::now() + Duration::from_secs(2);
    while let Ok(ev) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        match ev {
            Event::Out(l) => {
                println!("{l}");
                lines.add(&l);
            }
            Event::Err(l) => stderr_lines.push(l),
        }
    }
    let net_after = adapters().unwrap_or_default();
    let report = Report { env, exe: o.exe.clone(), duration, net_before, net_after, lines, stderr: stderr_lines, seen, stuck: sampler.namer.stuck, exit: status.code(), timed_out };
    let (text, failures) = report.render();
    let present = !online(&report.net_before).is_empty();
    let path = PathBuf::from(if present { "target/misure/sessione-rete-presente.md" } else { "target/misure/sessione-rete-assente.md" });
    std::fs::create_dir_all("target/misure").map_err(|e| e.to_string())?;
    std::fs::write(&path, &text).map_err(|e| e.to_string())?;
    eprintln!("scritto {}", path.display());
    if failures.is_empty() {
        println!("sessione superata: {}", path.display());
        Ok(())
    } else {
        Err(format!("sessione NON superata ({}):\n- {}", path.display(), failures.join("\n- ")))
    }
}

/// Stato del campionamento: tabella del sistema, nomi, processo figlio.
struct Sampler {
    snap: Snapshot,
    namer: Namer,
    /// Handle del processo figlio con PROCESS_DUP_HANDLE.
    process: usize,
    file_type: u16,
    pid: u32,
    /// Oggetti degli handle di xtask prima dell'avvio: un handle del figlio sullo stesso oggetto è ereditato.
    inherited: HashSet<usize>,
}

impl Sampler {
    /// Un campione: connessioni e handle di tipo File del processo. Gli handle nuovi si duplicano subito,
    /// prima che il processo li chiuda, poi se ne leggono nome e tipo.
    fn sample(&mut self, seen: &mut Observed, step: &str) {
        seen.samples += 1;
        match connections(self.pid) {
            Ok(c) => {
                seen.conn_samples += 1;
                for x in c {
                    seen.connections.entry(x).or_insert_with(|| step.to_owned());
                }
            }
            Err(e) => {
                seen.conn_errors.insert(e);
            }
        }
        let (pid, file_type) = (self.pid as usize, self.file_type);
        let entries: Vec<Entry> = match self.snap.query() {
            Ok(all) => all.iter().filter(|e| e.pid == pid && e.object_type == file_type).copied().collect(),
            Err(e) => {
                seen.handle_errors.insert(e);
                return;
            }
        };
        seen.max_file_handles = seen.max_file_handles.max(entries.len());
        for e in entries {
            let key = Key { value: e.value, object: e.object, access: e.access };
            if let Some(s) = seen.handles.get_mut(&key) {
                s.samples += 1;
                continue;
            }
            let dup = duplicate(self.process, e.value, e.access);
            if let Some(d) = dup {
                self.namer.submit(key, d);
            }
            seen.handles.insert(key, Seen { info: Info { gone: dup.is_none(), ..Default::default() }, waiting: dup.is_some(), step: step.to_owned(), samples: 1 });
        }
        self.named(seen);
    }

    /// Applica i nomi arrivati.
    fn named(&mut self, seen: &mut Observed) {
        for (key, mut info) in self.namer.collect() {
            if let Some(s) = seen.handles.get_mut(&key) {
                info.inherited = key.object != 0 && self.inherited.contains(&key.object);
                s.info = info;
                s.waiting = false;
            }
        }
    }

    /// Dopo la fine del processo: i nomi ancora in coda, per al più due secondi; i mancanti restano senza nome.
    fn finish(&mut self, seen: &mut Observed) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.namer.pending() && Instant::now() < deadline {
            self.named(seen);
            std::thread::sleep(Duration::from_millis(20));
        }
        self.named(seen);
        for s in seen.handles.values_mut().filter(|s| s.waiting) {
            s.info.blocked = true;
            s.waiting = false;
        }
    }
}

struct Report {
    env: Vec<(&'static str, String)>,
    exe: PathBuf,
    duration: Duration,
    net_before: Vec<Adapter>,
    net_after: Vec<Adapter>,
    lines: Lines,
    stderr: Vec<String>,
    seen: Observed,
    stuck: usize,
    exit: Option<i32>,
    timed_out: bool,
}

fn exe_line(exe: &Path) -> String {
    let meta = std::fs::metadata(exe).ok();
    let size = meta.as_ref().map_or(0, std::fs::Metadata::len);
    format!("`{}`, {} ({size} byte)", exe.display(), crate::scan_cmd::gib(size))
}

fn net_line(a: &[Adapter]) -> String {
    let on = online(a);
    if on.is_empty() {
        return "assente: nessun adattatore attivo con un gateway".into();
    }
    let list: Vec<String> = on.iter().map(|a| format!("{} ({})", a.name, kind_name(a.kind))).collect();
    format!("presente: {}", list.join(", "))
}

impl Report {
    /// Il resoconto e l'elenco dei controlli non superati.
    fn render(&self) -> (String, Vec<String>) {
        let mut out = String::new();
        let mut failures: Vec<String> = Vec::new();
        let present = !online(&self.net_before).is_empty();
        let _ = writeln!(out, "# M6 · Sessione guidata con la rete {}\n", if present { "presente" } else { "assente" });
        let _ = writeln!(
            out,
            "Generato da `cargo xtask sessione-prova` (06 §10, righe \"Sessione senza rete\", \"Dati salvati\", \"Zone.Identifier\"). Quado guida sé stesso (`quado --sessione-prova`, `crates/quado/src/session.rs`); xtask ne campiona connessioni e handle, in media ogni {:.0} ms.\n",
            self.duration.as_secs_f64() * 1000.0 / self.seen.samples.max(1) as f64
        );
        let _ = writeln!(out, "| Condizione | Valore |\n|---|---|");
        for (k, v) in &self.env {
            if matches!(*k, "Data" | "Windows" | "CPU" | "Defender in tempo reale" | "Versione di Quado") {
                let _ = writeln!(out, "| {k} | {v} |");
            }
        }
        let _ = writeln!(out, "| Eseguibile | {} |", exe_line(&self.exe));
        let _ = writeln!(out, "| Rete all'inizio | {} |", net_line(&self.net_before));
        let _ = writeln!(out, "| Rete alla fine | {} |", net_line(&self.net_after));
        let _ = writeln!(out, "| Durata | {:.1} s |", self.duration.as_secs_f64());
        let _ = writeln!(out, "| Uscita del processo | {} |\n", self.exit.map_or_else(|| "nessun codice".to_owned(), |c| c.to_string()));

        // Passi.
        let _ = writeln!(out, "## Passi\n\n| Passo | Tempo | Esito | Dettagli |\n|---|---|---|---|");
        for name in STEPS {
            let details: Vec<&str> = self.lines.details.iter().filter(|(n, _)| n == name).map(|(_, t)| t.as_str()).collect();
            match self.lines.steps.iter().find(|(n, _, _)| n == name) {
                Some((_, ms, esito)) => {
                    let _ = writeln!(out, "| {name} | {} | {esito} | {} |", ms_text(*ms), cell(&details.join("; ")));
                    if esito != "ok" {
                        failures.push(format!("passo {name}: {esito}"));
                    }
                }
                None => {
                    let _ = writeln!(out, "| {name} | | non eseguito | {} |", cell(&details.join("; ")));
                    failures.push(format!("passo {name} non eseguito"));
                }
            }
        }
        if self.timed_out {
            failures.push(format!("sessione oltre {} minuti, processo fermato", SESSION_LIMIT.as_secs() / 60));
        }
        if self.exit != Some(0) {
            failures.push(format!("uscita del processo: {:?}", self.exit));
        }

        // Handle per categoria.
        let h = &self.seen.handles;
        let disk: Vec<&Seen> = h.values().filter(|s| s.info.disk).collect();
        let dirs = disk.iter().filter(|s| s.info.directory).count();
        let gone = h.values().filter(|s| s.info.gone).count();
        let blocked = h.values().filter(|s| s.info.blocked).count();
        let mut devices: BTreeMap<String, usize> = BTreeMap::new();
        for s in h.values().filter(|s| !s.info.disk && !s.info.gone && !s.info.blocked) {
            let name = s.info.nt.as_deref().map_or_else(|| "(senza nome)".to_owned(), device);
            let kind = s.info.device_type.map_or_else(String::new, |d| format!(" (tipo {d:#x})"));
            *devices.entry(format!("{name}{kind}")).or_default() += 1;
        }
        let writes: Vec<(&Key, &Seen)> = h.iter().filter(|(k, s)| k.access & WRITE_BITS != 0 && !s.info.inherited).collect();
        let write_ok: BTreeSet<String> = writes.iter().filter(|(_, s)| s.info.disk && s.info.names().any(write_allowed)).map(|(_, s)| s.info.shown()).collect();
        let write_gfx: Vec<(&Key, &Seen)> = writes.iter().filter(|(_, s)| s.info.disk && !s.info.names().any(write_allowed) && s.info.names().any(|n| graphics(n).is_some())).copied().collect();
        let write_bad: Vec<(&Key, &Seen)> =
            writes.iter().filter(|(_, s)| s.info.disk && !s.info.names().any(write_allowed) && !s.info.names().any(|n| graphics(n).is_some())).copied().collect();
        // Handle in scrittura chiusi prima del duplicato, o senza nome. Nel passo Impostazioni sono il file
        // delle impostazioni (la sessione lo scrive e lo rimette). All'avvio, prima del primo passo, il codice
        // di Quado non apre niente in scrittura (impostazioni e lingue si leggono, eframe è senza
        // "persistence"): li apre l'inizializzazione grafica, e contano con il sottosistema grafico. Altrove
        // non si sa dove scrivono: contano come fuori posto.
        let unnamed = |s: &Seen| s.info.gone || s.info.blocked;
        let unnamed_settings = writes.iter().filter(|(_, s)| unnamed(s) && s.step == "impostazioni").count();
        let unnamed_start: Vec<(&Key, &Seen)> = writes.iter().filter(|(_, s)| unnamed(s) && s.step == "avvio").copied().collect();
        let unnamed_bad: Vec<(&Key, &Seen)> = writes.iter().filter(|(_, s)| unnamed(s) && s.step != "impostazioni" && s.step != "avvio").copied().collect();
        let inherited: Vec<(&Key, &Seen)> = h.iter().filter(|(_, s)| s.info.inherited).collect();
        // Pipe e console in scrittura (standard output e standard error verso xtask): non sono file.
        let write_other: Vec<(&Key, &Seen)> = writes.iter().filter(|(_, s)| !s.info.disk && !unnamed(s)).copied().collect();
        let mut access: BTreeMap<u32, (usize, usize, usize)> = BTreeMap::new();
        for (k, s) in h {
            let e = access.entry(k.access).or_default();
            e.0 += 1;
            e.1 += usize::from(s.info.disk && s.info.directory);
            e.2 += usize::from(unnamed(s));
        }
        let zone: Vec<&Seen> = h.values().filter(|s| s.info.names().any(is_zone)).collect();
        let net: Vec<&Seen> = h.values().filter(|s| s.info.network()).collect();
        let excl: Vec<(&Key, &Seen)> = h.iter().filter(|(_, s)| s.info.names().any(is_excluded)).collect();
        let (excl_listing, excl_bad): (Vec<_>, Vec<_>) = excl.into_iter().partition(|(k, s)| s.step == "scansione" && s.info.directory && k.access & WRITE_BITS == 0);
        let downloads = quado_kb::win::known_folders().downloads.map(|d| d.to_lowercase());
        let in_downloads: BTreeSet<String> = disk
            .iter()
            .filter(|s| !s.info.directory)
            .filter_map(|s| s.info.dos.as_deref())
            .map(|d| d.strip_prefix(r"\\?\").unwrap_or(d).to_lowercase())
            .filter(|d| downloads.as_ref().is_some_and(|dl| d.starts_with(&format!(r"{dl}\"))))
            .collect();
        let session_errors = self.lines.errors.len();
        let stderr_errors: Vec<&String> = self.stderr.iter().filter(|l| l.starts_with("[ERROR]")).collect();
        let stderr_warn: Vec<&String> = self.stderr.iter().filter(|l| l.starts_with("[WARN]")).collect();

        let _ = writeln!(out, "\n## Controlli\n\n| Controllo | Atteso | Osservato | Esito |\n|---|---|---|---|");
        let mut check = |what: &str, want: &str, got: String, ok: bool| {
            let _ = writeln!(out, "| {what} | {want} | {got} | {} |", if ok { "superato" } else { "**non superato**" });
            if !ok {
                failures.push(format!("{what}: {got}"));
            }
        };
        let conns = self.seen.connections.len();
        check("Connessioni TCP e UDP del processo (IPv4 e IPv6)", "0", format!("{conns} in {} campioni", self.seen.conn_samples), conns == 0 && self.seen.conn_samples > 0);
        let bad = write_bad.len() + unnamed_bad.len();
        check(
            "Handle in scrittura fuori da `%APPDATA%\\Quado` e dal Cestino, esclusi quelli del sottosistema grafico",
            "0",
            format!("{} con il nome, {} senza nome fuori dall'avvio e dal passo Impostazioni", write_bad.len(), unnamed_bad.len()),
            bad == 0,
        );
        let gfx_why: BTreeSet<&str> = write_gfx.iter().filter_map(|(_, s)| s.info.names().find_map(graphics)).collect();
        // Limite dichiarato (06 §10, deciso in M6): le cache dei driver grafici e l'overlay che il driver
        // carica nel processo scrivono fuori da `%APPDATA%\Quado` come in ogni programma Direct3D. Si
        // riportano tutte, con il motivo; non fanno fallire la prova.
        check(
            "Handle in scrittura del sottosistema grafico caricato nel processo (cache dei driver, overlay: fuori dal codice di Quado, limite dichiarato in 06 §10)",
            "solo cache e registri dei driver grafici",
            format!(
                "{} con il nome ({}); {} senza nome all'avvio, chiusi prima del duplicato",
                write_gfx.len(),
                gfx_why.into_iter().collect::<Vec<_>>().join("; "),
                unnamed_start.len()
            ),
            true,
        );
        check("Nomi con `:Zone.Identifier`", "0", zone.len().to_string(), zone.is_empty());
        check("Dispositivi di rete (`\\Device\\Afd`, `\\Device\\Tcp`, `\\Device\\Udp`, `\\Device\\RawIp`)", "0", net.len().to_string(), net.is_empty());
        check(
            "Percorsi con una parte di `--escludi` fuori dall'elenco della scansione",
            "0",
            format!("{} (cartelle elencate dalla scansione, in sola lettura: {})", excl_bad.len(), excl_listing.len()),
            excl_bad.is_empty(),
        );
        check("Errori della sessione", "0", session_errors.to_string(), session_errors == 0);
        check("Errori nel registro di Quado (standard error)", "0", stderr_errors.len().to_string(), stderr_errors.is_empty());
        let refused = self.lines.summary.get("letture-rifiutate").cloned().unwrap_or_else(|| "non riportato".into());
        let confirmations = self.lines.summary.get("conferme").cloned().unwrap_or_else(|| "non riportato".into());
        check("Piani di spostamento confermati", "0", confirmations.clone(), confirmations == "0");
        let sampling_ok = self.seen.samples > 10 && self.seen.handle_errors.is_empty() && !h.is_empty();
        check("Campionamento degli handle", "attivo", format!("{} campioni, {} handle distinti", self.seen.samples, h.len()), sampling_ok);

        let _ = writeln!(out, "\n## Campionamento\n\n| Misura | Valore |\n|---|---|");
        let _ = writeln!(out, "| Campioni | {} (connessioni lette in {}) |", self.seen.samples, self.seen.conn_samples);
        let _ = writeln!(out, "| Intervallo medio tra due campioni | {:.0} ms |", self.duration.as_secs_f64() * 1000.0 / self.seen.samples.max(1) as f64);
        let _ = writeln!(out, "| Handle di tipo File distinti osservati | {} (al più {} insieme) |", h.len(), self.seen.max_file_handles);
        let _ = writeln!(out, "| File su disco | {} ({} cartelle) |", disk.len(), dirs);
        let _ = writeln!(out, "| Altri dispositivi | {} |", devices.values().sum::<usize>());
        let _ = writeln!(out, "| Chiusi prima del duplicato (nome non letto) | {gone} |");
        let _ = writeln!(out, "| Ereditati da xtask (non aperti da Quado) | {} |", inherited.len());
        let _ = writeln!(out, "| Handle in scrittura senza nome nel passo Impostazioni (file delle impostazioni) | {unnamed_settings} |");
        let _ = writeln!(out, "| Nomi non arrivati entro {} ms | thread abbandonati {}; handle rimasti senza nome {blocked} |", NAME_TIMEOUT.as_millis(), self.stuck);
        let _ = writeln!(out, "| File aperti nella cartella Download | {} (nessuno nelle copie escluse) |", in_downloads.len());
        let _ = writeln!(out, "| Letture della lente rifiutate dalla sessione (copie escluse) | {refused} |");
        if !self.seen.conn_errors.is_empty() || !self.seen.handle_errors.is_empty() {
            let errs: Vec<&String> = self.seen.conn_errors.iter().chain(&self.seen.handle_errors).collect();
            let _ = writeln!(out, "| Errori del campionamento | {} |", cell(&errs.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("; ")));
        }

        let _ = writeln!(
            out,
            "\n### Accessi concessi\n\n`0x100081` e `0x100080` sono gli accessi della scansione (elenco e attributi, `READ_ONLY_ACCESS`), `0x120089` la lettura del contenuto (`GENERIC_READ`, impronte della lente).\n\n| Accesso | Handle | di cui cartelle | di cui senza nome | Scrittura |\n|---|---|---|---|---|"
        );
        for (a, (n, d, u)) in &access {
            let _ = writeln!(out, "| `{a:#x}` | {n} | {d} | {u} | {} |", if a & WRITE_BITS != 0 { "sì" } else { "no" });
        }
        let _ = writeln!(out, "\n### Dispositivi non su disco\n\n| Dispositivo | Handle |\n|---|---|");
        for (d, n) in &devices {
            let _ = writeln!(out, "| {} | {n} |", cell(d));
        }
        let _ = writeln!(out, "\n### Scritture nella cartella dei dati\n");
        if write_ok.is_empty() {
            let _ = writeln!(out, "Nessun handle in scrittura osservato in `%APPDATA%\\Quado` (le impostazioni si scrivono e si chiudono in pochi millisecondi: il campionamento può non vederle).");
        } else {
            for p in &write_ok {
                let _ = writeln!(out, "- `{p}`");
            }
        }
        let list = |out: &mut String, title: &str, items: &[String]| {
            if !items.is_empty() {
                let _ = writeln!(out, "\n### {title}\n");
                for i in items {
                    let _ = writeln!(out, "- {i}");
                }
            }
        };
        let fmt_seen = |k: &Key, s: &Seen| format!("`{}` (accesso {:#x}, passo {}, campioni {})", s.info.shown(), k.access, s.step, s.samples);
        list(&mut out, "Handle in scrittura fuori posto", &write_bad.iter().chain(&unnamed_bad).map(|(k, s)| fmt_seen(k, s)).collect::<Vec<_>>());
        list(
            &mut out,
            "Handle in scrittura del sottosistema grafico",
            &write_gfx
                .iter()
                .map(|(k, s)| format!("{}: {}", fmt_seen(k, s), s.info.names().find_map(graphics).unwrap_or_default()))
                .chain(unnamed_start.iter().map(|(k, s)| format!("{}: aperto all'avvio, durante l'inizializzazione grafica", fmt_seen(k, s))))
                .collect::<Vec<_>>(),
        );
        list(&mut out, "Handle ereditati da xtask", &inherited.iter().map(|(k, s)| fmt_seen(k, s)).collect::<Vec<_>>());
        let device_of = |s: &Seen| s.info.device_type.map_or_else(String::new, |d| format!(", tipo di dispositivo {d:#x}"));
        list(
            &mut out,
            "Handle in scrittura non su disco (pipe 0x11 di standard output e standard error verso xtask, console 0x50, generatore casuale di Windows KsecDD 0x39: non sono file)",
            &write_other.iter().map(|(k, s)| format!("{}{}", fmt_seen(k, s), device_of(s))).collect::<Vec<_>>(),
        );
        list(&mut out, "Zone.Identifier", &zone.iter().map(|s| format!("`{}` (passo {})", s.info.shown(), s.step)).collect::<Vec<_>>());
        list(&mut out, "Dispositivi di rete", &net.iter().map(|s| format!("`{}` (passo {})", s.info.shown(), s.step)).collect::<Vec<_>>());
        list(&mut out, "Copie escluse aperte fuori dall'elenco della scansione", &excl_bad.iter().map(|(k, s)| fmt_seen(k, s)).collect::<Vec<_>>());
        list(&mut out, "Connessioni", &self.seen.connections.iter().map(|(c, step)| format!("{c} (passo {step})")).collect::<Vec<_>>());
        list(&mut out, "Errori della sessione", &self.lines.errors.iter().map(|(n, t)| format!("{n}: {t}")).collect::<Vec<_>>());
        list(&mut out, "Errori del registro", &stderr_errors.iter().map(|l| format!("`{}`", tidy_line(l))).collect::<Vec<_>>());
        list(&mut out, "Avvisi del registro", &stderr_warn.iter().map(|l| format!("`{}`", tidy_line(l))).collect::<Vec<_>>());

        let _ = writeln!(out, "\n## Limiti\n");
        let _ = writeln!(out, "- Il campionamento può perdere aperture brevissime (un file aperto e chiuso tra due campioni) e connessioni che durano meno di un intervallo. La prova completa con Process Monitor o con una traccia ETW (`FileIo`, `TcpIp`) resta da fare a mano.");
        let _ = writeln!(out, "- Gli handle chiusi prima del duplicato (le cartelle che la scansione apre per pochi microsecondi) si contano con il loro accesso ma senza nome: per loro i controlli sul nome (`Zone.Identifier`, copie escluse, cartella dei dati) non valgono; quelli in scrittura si riportano a parte.");
        let _ = writeln!(out, "- Il sottosistema grafico gira dentro il processo di Quado e scrive le sue cache fuori da `%APPDATA%\\Quado`: `D3DSCache.dll` (cache degli shader di Direct3D 12, di Windows), i driver delle schede video caricati da wgpu quando elenca gli adattatori (su un PC con due schede video anche quello della scheda che non disegna) e gli overlay che alcuni driver caricano nei processi Direct3D. Il controllo le riporta a parte, perché non dipendono dal codice di Quado; con il renderer OpenGL resterebbero le cache OpenGL dei driver.");
        let _ = writeln!(out, "- Si osservano solo il processo di Quado e i suoi handle: un servizio di Windows chiamato da Quado (per esempio Restart Manager o l'Utilità di pianificazione per i riferimenti) lavora nel suo processo.");
        let _ = writeln!(out, "- La sessione non usa \"Proprietà\" né altri dialoghi della shell, che caricano estensioni di terzi (06 §10, limite dichiarato).");
        let _ = writeln!(out, "- La scansione di C: elenca nomi e attributi anche delle cartelle di `--escludi` (come ogni scansione del volume); nessuna lettura di contenuto le raggiunge: la sessione rifiuta quei percorsi prima di aprirli.");
        if !failures.is_empty() {
            let _ = writeln!(out, "\n## Esito\n\nNon superato:\n");
            for f in &failures {
                let _ = writeln!(out, "- {f}");
            }
        } else {
            let _ = writeln!(out, "\n## Esito\n\nSuperato: tutti i passi riusciti, nessuna connessione, nessuna scrittura fuori posto, nessun `Zone.Identifier`, nessuna lettura delle copie escluse.");
        }
        (out.replace(['\u{2014}', '\u{2013}'], "-"), failures)
    }
}

fn ms_text(ms: u64) -> String {
    if ms >= 10_000 { format!("{:.1} s", ms as f64 / 1000.0) } else { format!("{ms} ms") }
}

/// Testo per una cella di tabella: niente `|` né a capo.
fn cell(s: &str) -> String {
    s.replace('|', "/").replace(['\r', '\n'], " ")
}

/// Una riga del registro, con i prefissi dei percorsi resi generici.
fn tidy_line(l: &str) -> String {
    let mut s = l.to_owned();
    for var in ["APPDATA", "LOCALAPPDATA", "USERPROFILE"] {
        if let Ok(v) = std::env::var(var) {
            s = s.replace(&v, &format!("%{var}%"));
        }
    }
    cell(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_lines_are_parsed() {
        let mut l = Lines::default();
        for line in ["inizio\tscansione", "dettaglio\tscansione\tfile 3", "passo\tscansione\t1200\tok", "errore\tdownload\ttempo scaduto", "riepilogo\tconferme\t0", "boh"] {
            l.add(line);
        }
        assert_eq!(l.steps, [("scansione".to_owned(), 1200, "ok".to_owned())]);
        assert_eq!(l.details, [("scansione".to_owned(), "file 3".to_owned())]);
        assert_eq!(l.errors.len(), 1);
        assert_eq!(l.summary.get("conferme").map(String::as_str), Some("0"));
        assert_eq!(l.other, 1);
    }

    #[test]
    fn paths_are_classified() {
        let appdata = std::env::var("APPDATA").unwrap();
        assert!(write_allowed(&format!(r"\\?\{appdata}\Quado\impostazioni.txt")));
        assert!(write_allowed(&format!(r"{appdata}\QUADO")));
        assert!(!write_allowed(&format!(r"{appdata}\QuadoAltro\x")));
        assert!(write_allowed(r"\\?\C:\$Recycle.Bin\S-1-5-21-1\$R1.txt"));
        assert!(!write_allowed(r"\\?\C:\Users\u\Documents\a.txt"));
        assert!(is_zone(r"\\?\C:\Users\u\Downloads\a.exe:Zone.Identifier"));
        assert!(is_net_device(r"\Device\Afd"));
        assert!(is_net_device(r"\Device\Afd\Endpoint"));
        assert!(is_net_device(r"\Device\Tcp6"));
        assert!(!is_net_device(r"\Device\NamedPipe\x"));
        assert!(!is_net_device(r"\Device\Nsi"));
        let _ = EXCLUDED.set(vec!["copia-a-".into()]);
        assert!(is_excluded(r"\\?\C:\Users\u\Downloads\Copia-A-0.4.0-windows-x64-portable"));
        assert!(!is_excluded(r"\\?\C:\Users\u\Downloads\copia-b-0.4.0"));
        assert_eq!(device(r"\Device\NamedPipe\abc\def"), r"\Device\NamedPipe");
        assert!(graphics(r"\\?\C:\Users\u\AppData\Local\D3DSCache\d83\F4EB.idx").is_some());
        assert!(graphics(r"\\?\C:\ProgramData\NVIDIA Corporation\ShadowPlay\CaptureCore.log").is_some());
        assert!(graphics(r"\\?\C:\Users\u\AppData\Local\Quado\x.log").is_none());
        assert_eq!(tidy(&format!(r"\\?\{appdata}\Quado\x")), r"%APPDATA%\Quado\x");
    }

    #[test]
    fn the_handle_table_and_the_connection_tables_are_readable() {
        // Questo processo: il tipo File si trova, e la tabella contiene un file aperto qui.
        let mut snap = Snapshot::new();
        let t = file_type_index(&mut snap).unwrap();
        assert!(t > 0);
        let me = std::process::id();
        let n = snap.query().unwrap().iter().filter(|e| e.pid == me as usize && e.object_type == t).count();
        assert!(n > 0);
        // Le tabelle delle connessioni si leggono (questo processo non ne ha).
        assert_eq!(connections(me).unwrap(), Vec::<String>::new());
        assert!(!adapters().unwrap().is_empty());
    }

    #[test]
    fn a_file_opened_here_is_named_through_a_duplicate() {
        let exe = std::env::current_exe().unwrap();
        let f = std::fs::File::open(&exe).unwrap();
        // SAFETY: pseudo-handle del processo corrente, valido senza chiusura.
        let me = unsafe { GetCurrentProcess() } as usize;
        let (access, kind) = access_and_type(f.as_raw_handle() as HANDLE).expect("accesso e tipo");
        assert_eq!((access, kind.as_str()), (0x120089, "File"));
        // Un accesso diverso da quello del campione: il valore è stato riusato, niente duplicato.
        assert_eq!(duplicate(me, f.as_raw_handle() as usize, access | 2), None);
        let dup = duplicate(me, f.as_raw_handle() as usize, access).expect("duplicato");
        let info = describe(dup);
        assert!(info.disk && !info.directory, "{info:?}");
        let name = exe.file_name().unwrap().to_string_lossy().to_lowercase();
        assert!(info.dos.as_deref().unwrap().to_lowercase().ends_with(&name), "{info:?}");
        assert!(info.nt.as_deref().unwrap().to_lowercase().starts_with(r"\device\"), "{info:?}");
    }
}
