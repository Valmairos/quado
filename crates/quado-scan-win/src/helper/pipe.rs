// SPDX-License-Identifier: GPL-3.0-or-later
//! Pipe con nome tra interfaccia e helper (05 D3, 06 §11).
//!
//! - [`Server`] (interfaccia, non elevata): nome casuale `\\.\pipe\quado-<128 bit esadecimali>`, DACL
//!   con il solo SID dell'utente, `PIPE_REJECT_REMOTE_CLIENTS`, una sola istanza. Accetta un solo
//!   client e verifica con `GetNamedPipeClientProcessId` che sia l'helper appena avviato.
//! - [`Client`] (helper, elevato): verifica con `GetNamedPipeServerProcessId` che dall'altra parte ci sia
//!   il processo che l'ha lanciato. Legge un solo messaggio (la richiesta), poi scrive soltanto.
//!
//! Gli handle della pipe si aprono senza `GENERIC_WRITE` (vietato fuori da `quado-actions`, test
//! d'architettura): bastano i diritti specifici della pipe.
//!
//! Tutto l'I/O è sovrapposto e si attende a fette brevi: `accept` e `recv` rispettano annullamento e
//! tempo massimo, e un'operazione annullata si completa sempre prima di tornare (la struttura
//! `OVERLAPPED` vive sullo stack della funzione che l'ha avviata).

use std::os::windows::io::{AsRawHandle, RawHandle};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, ERROR_INVALID_NAME, ERROR_INVALID_STATE, ERROR_IO_PENDING, ERROR_NO_DATA,
    ERROR_PIPE_CONNECTED, ERROR_PIPE_NOT_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::Authorization::{ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1};
use windows_sys::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};
use windows_sys::Win32::Security::{GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, FILE_READ_ATTRIBUTES, FILE_READ_DATA, FILE_WRITE_DATA,
    OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT, SYNCHRONIZE, WriteFile,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId, GetNamedPipeServerProcessId,
    PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, INFINITE, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION, ResetEvent, WaitForSingleObject,
};

use super::proto::{self, MAX_REPLY_BYTES, MAX_REQUEST_BYTES, ProtoError, Reply, Request};

/// Errore della pipe: codice di Windows o protocollo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipeError {
    Win(u32),
    Proto(ProtoError),
    /// Il processo dall'altra parte non è quello atteso: pid trovato.
    WrongPeer(u32),
    /// Nessun client entro il tempo, o annullato.
    Timeout,
    Cancelled,
    /// L'altra parte ha chiuso.
    Closed,
}

/// Prefisso del nome della pipe; seguono 32 cifre esadecimali minuscole.
pub const NAME_PREFIX: &str = r"\\.\pipe\quado-";
/// Buffer della pipe in ciascuna direzione.
const BUFFER: u32 = 64 * 1024;
/// Fetta di attesa: tempo massimo fra un annullamento e il ritorno, oltre al completamento dell'I/O.
const SLICE: Duration = Duration::from_millis(20);
/// Tempo massimo dell'helper per ricevere la richiesta: l'interfaccia la invia appena collegata.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Lettura del corpo di una risposta a pezzi: la memoria cresce con i byte arrivati, non con la
/// lunghezza dichiarata.
const READ_STEP: usize = 1 << 20;

/// Il nome ha la forma prodotta da [`Server::create`]: l'helper non apre altro.
pub fn valid_name(name: &str) -> bool {
    name.strip_prefix(NAME_PREFIX).is_some_and(|hex| hex.len() == 32 && hex.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

fn last_error() -> u32 {
    // SAFETY: nessun prerequisito.
    unsafe { GetLastError() }
}

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Handle di un oggetto del kernel, chiuso al drop.
pub(crate) struct Owned(HANDLE);

// SAFETY: un handle del kernel si può usare e chiudere da qualunque thread; `Owned` non lo condivide
// (nessun `Clone`) e lo chiude una volta sola.
unsafe impl Send for Owned {}

impl Owned {
    /// Da un handle restituito da Windows: `None` se nullo o `INVALID_HANDLE_VALUE`.
    pub(crate) fn new(h: HANDLE) -> Option<Self> {
        (!h.is_null() && h != INVALID_HANDLE_VALUE).then_some(Self(h))
    }

    pub(crate) fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: handle valido (controllato in `new`), chiuso una volta sola, qui.
        unsafe { CloseHandle(self.0) };
    }
}

/// Fine di un'attesa non riuscita.
enum Stop {
    Win(u32),
    Timeout,
    Cancelled,
}

impl From<Stop> for PipeError {
    fn from(s: Stop) -> Self {
        match s {
            Stop::Win(ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED | ERROR_NO_DATA) => PipeError::Closed,
            Stop::Win(c) => PipeError::Win(c),
            Stop::Timeout => PipeError::Timeout,
            Stop::Cancelled => PipeError::Cancelled,
        }
    }
}

/// Handle della pipe aperto con `FILE_FLAG_OVERLAPPED` e il suo evento.
struct Io {
    pipe: Owned,
    event: Owned,
}

impl Io {
    fn new(pipe: Owned) -> Result<Self, u32> {
        // SAFETY: evento anonimo a reimpostazione manuale, non segnalato, senza attributi di sicurezza.
        let ev = unsafe { CreateEventW(null(), 1, 0, null()) };
        let event = Owned::new(ev).ok_or_else(last_error)?;
        Ok(Self { pipe, event })
    }

    /// Struttura per una nuova operazione, con l'evento riportato a non segnalato.
    fn overlapped(&self) -> OVERLAPPED {
        // SAFETY: l'evento è valido; un evento a reimpostazione manuale va riportato a non segnalato
        // prima di ogni operazione.
        unsafe { ResetEvent(self.event.raw()) };
        OVERLAPPED { hEvent: self.event.raw(), ..Default::default() }
    }

    /// Attende la fine dell'operazione avviata con `ov` (completata subito o `ERROR_IO_PENDING`). Con
    /// `deadline` o `cancel` l'attesa è a fette; allo scadere l'operazione si annulla e si attende il
    /// suo completamento, così `ov` non è più in uso al ritorno. `ov` è un puntatore: mentre l'I/O è in
    /// corso Windows ci scrive, e dal lato Rust non esistono riferimenti alla struttura.
    fn finish(&self, ov: *mut OVERLAPPED, deadline: Option<Instant>, cancel: Option<&AtomicBool>) -> Result<u32, Stop> {
        let sliced = deadline.is_some() || cancel.is_some();
        loop {
            let wait = match deadline {
                Some(d) => SLICE.min(d.saturating_duration_since(Instant::now())).as_millis() as u32,
                None if sliced => SLICE.as_millis() as u32,
                None => INFINITE,
            };
            // SAFETY: evento valido, posseduto da `self`.
            let w = unsafe { WaitForSingleObject(self.event.raw(), wait) };
            if w == WAIT_OBJECT_0 {
                break;
            }
            let stop = if w != WAIT_TIMEOUT {
                Some(Stop::Win(last_error()))
            } else if cancel.is_some_and(|c| c.load(Relaxed)) {
                Some(Stop::Cancelled)
            } else if deadline.is_some_and(|d| Instant::now() >= d) {
                Some(Stop::Timeout)
            } else {
                None
            };
            if let Some(stop) = stop {
                let mut n = 0u32;
                // SAFETY: `ov` è la struttura dell'operazione in corso su questo handle; dopo
                // l'annullamento l'attesa bloccante ne garantisce il completamento prima del ritorno.
                let done = unsafe {
                    CancelIoEx(self.pipe.raw(), ov);
                    GetOverlappedResult(self.pipe.raw(), ov, &mut n, 1)
                };
                // L'operazione può essere finita da sola un attimo prima dell'annullamento: vale il risultato.
                return if done != 0 { Ok(n) } else { Err(stop) };
            }
        }
        let mut n = 0u32;
        // SAFETY: l'evento è segnalato, quindi l'operazione di `ov` è completata; nessuna attesa.
        if unsafe { GetOverlappedResult(self.pipe.raw(), ov, &mut n, 0) } == 0 { Err(Stop::Win(last_error())) } else { Ok(n) }
    }

    /// Esito dell'avvio di un'operazione sovrapposta (`ReadFile`, `WriteFile`).
    fn started(ok: i32) -> Result<(), Stop> {
        if ok != 0 {
            return Ok(());
        }
        match last_error() {
            ERROR_IO_PENDING => Ok(()),
            e => Err(Stop::Win(e)),
        }
    }

    fn read_some(&self, buf: &mut [u8], deadline: Option<Instant>, cancel: Option<&AtomicBool>) -> Result<usize, Stop> {
        let mut ov = self.overlapped();
        let ov: *mut OVERLAPPED = &mut ov;
        let len = buf.len().min(u32::MAX as usize) as u32;
        // SAFETY: `buf` e `ov` restano validi e fermi fino al completamento, garantito da `finish`
        // (anche se annullato) prima che questa funzione torni.
        let ok = unsafe { ReadFile(self.pipe.raw(), buf.as_mut_ptr(), len, null_mut(), ov) };
        Self::started(ok)?;
        let n = self.finish(ov, deadline, cancel)? as usize;
        // Una lettura di zero byte su una pipe in modalità byte vuol dire che l'altra parte ha chiuso.
        if n == 0 { Err(Stop::Win(ERROR_BROKEN_PIPE)) } else { Ok(n) }
    }

    fn read_exact(&self, buf: &mut [u8], deadline: Option<Instant>, cancel: Option<&AtomicBool>) -> Result<(), Stop> {
        let mut done = 0;
        while done < buf.len() {
            done += self.read_some(&mut buf[done..], deadline, cancel)?;
        }
        Ok(())
    }

    fn write_all(&self, mut buf: &[u8]) -> Result<(), Stop> {
        while !buf.is_empty() {
            let mut ov = self.overlapped();
            let ov: *mut OVERLAPPED = &mut ov;
            let len = buf.len().min(u32::MAX as usize) as u32;
            // SAFETY: come in `read_some`: buffer e `ov` validi fino al completamento atteso da `finish`.
            let ok = unsafe { WriteFile(self.pipe.raw(), buf.as_ptr(), len, null_mut(), ov) };
            Self::started(ok)?;
            let n = self.finish(ov, None, None)? as usize;
            if n == 0 {
                return Err(Stop::Win(ERROR_BROKEN_PIPE));
            }
            buf = &buf[n.min(buf.len())..];
        }
        Ok(())
    }

    /// Una cornice: lunghezza `u32` e corpo (scritti uno dopo l'altro: un solo scrittore per lato, e il
    /// corpo di un albero grande non si copia).
    fn write_frame(&self, body: &[u8]) -> Result<(), Stop> {
        self.write_all(&(body.len() as u32).to_le_bytes())?;
        self.write_all(body)
    }
}

/// 16 byte casuali dal generatore di sistema, in esadecimale.
fn random_hex() -> Result<String, u32> {
    let mut b = [0u8; 16];
    // SAFETY: algoritmo nullo con BCRYPT_USE_SYSTEM_PREFERRED_RNG (il generatore preferito dal sistema);
    // buffer locale con la sua lunghezza.
    let status = unsafe { BCryptGenRandom(null_mut(), b.as_mut_ptr(), b.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    if status < 0 {
        return Err(status as u32);
    }
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// Descrittore di sicurezza da SDDL, liberato al drop.
struct Sd(PSECURITY_DESCRIPTOR);

impl Sd {
    fn from_sddl(sddl: &str) -> Result<Self, u32> {
        let z = wide_z(sddl);
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        // SAFETY: stringa terminata da zero; Windows alloca il descrittore con LocalAlloc in `sd`.
        if unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(z.as_ptr(), SDDL_REVISION_1, &mut sd, null_mut()) } == 0 {
            return Err(last_error());
        }
        Ok(Self(sd))
    }
}

impl Drop for Sd {
    fn drop(&mut self) {
        // SAFETY: allocato da ConvertStringSecurityDescriptorToSecurityDescriptorW, liberato una volta.
        unsafe { LocalFree(self.0) };
    }
}

/// SID dell'utente del token di `process` (aperto almeno con `PROCESS_QUERY_LIMITED_INFORMATION`).
fn token_user_sid(process: HANDLE) -> Option<String> {
    let mut token: HANDLE = null_mut();
    // SAFETY: handle di processo valido; il token si apre in sola lettura (TOKEN_QUERY).
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return None;
    }
    let token = Owned::new(token)?;
    // TOKEN_USER più il SID (al massimo 68 byte): 256 byte allineati a 8 bastano.
    let mut buf = [0u64; 32];
    let mut len = 0u32;
    // SAFETY: buffer locale allineato con la sua dimensione in byte; token valido.
    let ok = unsafe { GetTokenInformation(token.raw(), TokenUser, buf.as_mut_ptr().cast(), size_of_val(&buf) as u32, &mut len) } != 0;
    if !ok || (len as usize) < size_of::<TOKEN_USER>() {
        return None;
    }
    // SAFETY: il buffer, allineato a 8, contiene una TOKEN_USER scritta da Windows.
    let user: TOKEN_USER = unsafe { buf.as_ptr().cast::<TOKEN_USER>().read() };
    let mut text: *mut u16 = null_mut();
    // SAFETY: il SID punta dentro `buf`, ancora vivo; Windows alloca `text` con LocalAlloc.
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 || text.is_null() {
        return None;
    }
    // SAFETY: stringa terminata da zero allocata da Windows, letta e poi liberata con LocalFree.
    let s = unsafe {
        let mut n = 0;
        while *text.add(n) != 0 {
            n += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(text, n));
        LocalFree(text.cast());
        s
    };
    Some(s)
}

/// SID dell'utente del processo `pid`, letto dal suo token.
fn process_user_sid(pid: u32) -> Option<String> {
    // SAFETY: apertura in sola interrogazione; l'handle si chiude al drop di `Owned`.
    let h = Owned::new(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) })?;
    token_user_sid(h.raw())
}

/// Lato interfaccia.
pub struct Server {
    name: String,
    io: Io,
    connected: bool,
    sent: bool,
    /// Dopo `Closed` o una cornice non valida il flusso non è più allineato: niente altre letture.
    dead: bool,
}

impl Server {
    /// Crea la pipe (nome casuale, DACL del solo utente corrente, client remoti rifiutati).
    pub fn create() -> Result<Server, PipeError> {
        let name = format!("{NAME_PREFIX}{}", random_hex().map_err(PipeError::Win)?);
        // DACL protetta (niente ACE ereditate) con un solo ACE: tutti i diritti al SID dell'utente.
        let sid = crate::system::user_sid().ok_or(PipeError::Win(ERROR_INVALID_STATE))?;
        let sd = Sd::from_sddl(&format!("D:P(A;;GA;;;{sid})")).map_err(PipeError::Win)?;
        let sa = SECURITY_ATTRIBUTES { nLength: size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd.0, bInheritHandle: 0 };
        let z = wide_z(&name);
        // SAFETY: nome terminato da zero e attributi di sicurezza validi per la durata della chiamata.
        // FILE_FLAG_FIRST_PIPE_INSTANCE: se il nome esiste già (qualcuno l'ha creato prima) la chiamata
        // fallisce invece di aggiungere un'istanza a una pipe altrui.
        let h = unsafe {
            CreateNamedPipeW(
                z.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                BUFFER,
                BUFFER,
                0,
                &sa,
            )
        };
        let pipe = Owned::new(h).ok_or_else(|| PipeError::Win(last_error()))?;
        let io = Io::new(pipe).map_err(PipeError::Win)?;
        Ok(Server { name, io, connected: false, sent: false, dead: false })
    }

    /// Nome completo, da passare all'helper.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Attende la connessione fino a `timeout` o all'annullamento; rifiuta un client diverso da
    /// `expected_pid`.
    ///
    /// Un client rifiutato viene scollegato (`DisconnectNamedPipe`); l'istanza resta di questo processo,
    /// quindi nessun altro può creare una pipe con lo stesso nome finché `Server` vive.
    pub fn accept(&mut self, expected_pid: u32, timeout: Duration, cancel: &AtomicBool) -> Result<(), PipeError> {
        if self.connected {
            return Ok(());
        }
        let deadline = Instant::now() + timeout;
        let mut ov = self.io.overlapped();
        let ov: *mut OVERLAPPED = &mut ov;
        // SAFETY: handle della pipe valido; `ov` resta fermo fino al completamento (vedi `Io::finish`).
        let ok = unsafe { ConnectNamedPipe(self.io.pipe.raw(), ov) };
        if ok == 0 {
            match last_error() {
                // Il client si è collegato prima della chiamata: nessuna operazione in corso.
                ERROR_PIPE_CONNECTED => {}
                ERROR_IO_PENDING => {
                    self.io.finish(ov, Some(deadline), Some(cancel)).map_err(PipeError::from)?;
                }
                e => return Err(PipeError::Win(e)),
            }
        }
        let mut pid = 0u32;
        // SAFETY: pipe collegata; uscita locale.
        if unsafe { GetNamedPipeClientProcessId(self.io.pipe.raw(), &mut pid) } == 0 {
            let e = last_error();
            self.disconnect();
            return Err(PipeError::Win(e));
        }
        if pid != expected_pid {
            self.disconnect();
            return Err(PipeError::WrongPeer(pid));
        }
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) {
        // SAFETY: handle della pipe valido; chiude il lato del client, se c'è.
        unsafe { DisconnectNamedPipe(self.io.pipe.raw()) };
        self.connected = false;
    }

    /// Invia la richiesta (una volta sola). Si valida qui, prima di scriverla: l'helper la rifiuterebbe.
    pub fn send_request(&mut self, r: &Request) -> Result<(), PipeError> {
        if !self.connected {
            return Err(PipeError::Win(ERROR_PIPE_NOT_CONNECTED));
        }
        if self.sent {
            return Err(PipeError::Win(ERROR_INVALID_STATE));
        }
        r.validate().map_err(PipeError::Proto)?;
        let body = proto::encode_request(r);
        if body.len() > MAX_REQUEST_BYTES {
            return Err(PipeError::Proto(ProtoError::TooLarge));
        }
        self.sent = true;
        self.io.write_frame(&body).map_err(PipeError::from)
    }

    /// Prossima risposta; `Closed` quando l'helper ha chiuso dopo `Done` o è terminato.
    pub fn recv(&mut self, cancel: &AtomicBool) -> Result<Reply, PipeError> {
        if self.dead {
            return Err(PipeError::Closed);
        }
        if !self.connected {
            return Err(PipeError::Win(ERROR_PIPE_NOT_CONNECTED));
        }
        // Dopo un errore, anche un annullamento a metà cornice, il flusso non è più allineato.
        let r = self.read_reply(cancel);
        if r.is_err() {
            self.dead = true;
        }
        r
    }

    fn read_reply(&self, cancel: &AtomicBool) -> Result<Reply, PipeError> {
        let mut head = [0u8; 4];
        self.io.read_exact(&mut head, None, Some(cancel)).map_err(PipeError::from)?;
        let len = u32::from_le_bytes(head) as usize;
        if len > MAX_REPLY_BYTES {
            return Err(PipeError::Proto(ProtoError::TooLarge));
        }
        // La memoria cresce con i byte davvero arrivati, non con la lunghezza dichiarata.
        let mut body: Vec<u8> = Vec::new();
        while body.len() < len {
            let have = body.len();
            let want = (have + READ_STEP.max(have)).min(len);
            body.resize(want, 0);
            self.io.read_exact(&mut body[have..want], None, Some(cancel)).map_err(PipeError::from)?;
        }
        proto::decode_reply(&body).map_err(PipeError::Proto)
    }
}

impl AsRawHandle for Server {
    /// Handle del lato server della pipe (per leggerne la DACL nei test, 06 §11). Resta di `Server`.
    fn as_raw_handle(&self) -> RawHandle {
        self.io.pipe.raw()
    }
}

/// Lato helper.
pub struct Client {
    io: Io,
    server_sid: Option<String>,
    read: bool,
}

impl Client {
    /// Apre la pipe `name` e verifica che il server sia `parent_pid`.
    ///
    /// Solo nomi della forma di [`Server::create`]. Diritti specifici della pipe (lettura e scrittura
    /// dei dati, attributi), mai `GENERIC_WRITE`; `SECURITY_IDENTIFICATION`: il server può sapere chi è
    /// l'helper ma non agire con il suo token elevato.
    pub fn connect(name: &str, parent_pid: u32) -> Result<Client, PipeError> {
        if !valid_name(name) {
            return Err(PipeError::Win(ERROR_INVALID_NAME));
        }
        let z = wide_z(name);
        // SAFETY: nome terminato da zero; nessun attributo di sicurezza; nessun modello.
        let h = unsafe {
            CreateFileW(
                z.as_ptr(),
                FILE_READ_DATA | FILE_WRITE_DATA | FILE_READ_ATTRIBUTES | SYNCHRONIZE,
                0,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                null_mut(),
            )
        };
        let pipe = Owned::new(h).ok_or_else(|| PipeError::Win(last_error()))?;
        let mut pid = 0u32;
        // SAFETY: handle valido; uscita locale. Fallisce se l'oggetto aperto non è una pipe, o se il
        // server ha già scollegato questo client (`Closed`).
        if unsafe { GetNamedPipeServerProcessId(pipe.raw(), &mut pid) } == 0 {
            return Err(Stop::Win(last_error()).into());
        }
        if pid != parent_pid {
            return Err(PipeError::WrongPeer(pid));
        }
        let server_sid = process_user_sid(pid);
        let io = Io::new(pipe).map_err(PipeError::Win)?;
        Ok(Client { io, server_sid, read: false })
    }

    /// SID dell'utente del processo server (dal suo token), in forma `S-1-...`.
    pub fn server_user_sid(&self) -> Option<String> {
        self.server_sid.clone()
    }

    /// Legge e valida l'unica richiesta: una sola cornice, al massimo `MAX_REQUEST_BYTES`, entro un
    /// minuto. Una seconda chiamata è un errore.
    pub fn read_request(&mut self) -> Result<Request, PipeError> {
        if self.read {
            return Err(PipeError::Win(ERROR_INVALID_STATE));
        }
        self.read = true;
        let deadline = Some(Instant::now() + REQUEST_TIMEOUT);
        let mut head = [0u8; 4];
        self.io.read_exact(&mut head, deadline, None).map_err(PipeError::from)?;
        let len = u32::from_le_bytes(head) as usize;
        if len > MAX_REQUEST_BYTES {
            return Err(PipeError::Proto(ProtoError::TooLarge));
        }
        let mut body = vec![0u8; len];
        self.io.read_exact(&mut body, deadline, None).map_err(PipeError::from)?;
        proto::decode_request(&body).map_err(PipeError::Proto)
    }

    /// Invia una risposta. Un corpo oltre `MAX_REPLY_BYTES` non si scrive (`Proto(TooLarge)`): il
    /// server lo rifiuterebbe perdendo l'allineamento delle cornici.
    pub fn send(&mut self, r: &Reply) -> Result<(), PipeError> {
        let body = proto::encode_reply(r);
        if body.len() > MAX_REPLY_BYTES {
            return Err(PipeError::Proto(ProtoError::TooLarge));
        }
        self.io.write_frame(&body).map_err(PipeError::from)
    }
}
