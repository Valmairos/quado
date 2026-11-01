// SPDX-License-Identifier: GPL-3.0-or-later
//! Avvio dell'helper elevato (05 D3, 03 F-SC-3): questo stesso eseguibile con `--helper <pipe> <pid>` e
//! il verbo `runas` di `ShellExecuteExW`, solo su richiesta dell'utente ("Ripeti come amministratore").
//! Windows mostra il prompt UAC (per un eseguibile senza firma digitale l'autore risulta sconosciuto,
//! 05 D9): se l'utente risponde No, l'esito è `Err(1223)` (`ERROR_CANCELLED`).
//!
//! È l'unico processo che Quado crea (F-AP-5). Il prompt è sincrono: chiamare da un thread di lavoro.

use std::ptr::{null, null_mut};
use std::time::Duration;

use quado_scan_win::helper::pipe::valid_name;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INVALID_HANDLE, ERROR_INVALID_PARAMETER, GetLastError, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::System::Threading::{GetCurrentProcessId, GetExitCodeProcess, GetProcessId, INFINITE, TerminateProcess, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

/// Codice di uscita dato a un helper terminato da [`HelperProcess::terminate`] (`ERROR_CANCELLED`).
pub const TERMINATED: u32 = 1223;

/// Processo dell'helper avviato.
pub struct HelperProcess {
    pid: u32,
    /// Handle del processo restituito da `ShellExecuteExW`, chiuso al drop. Tenuto come intero: un
    /// handle del kernel si usa da qualunque thread, e così `HelperProcess` resta `Send`.
    handle: isize,
}

impl HelperProcess {
    pub fn pid(&self) -> u32 {
        self.pid
    }

    fn raw(&self) -> HANDLE {
        self.handle as HANDLE
    }

    /// Attende l'uscita fino a `timeout`: codice di uscita, `None` se è ancora in esecuzione.
    pub fn wait(&self, timeout: Duration) -> Option<u32> {
        // INFINITE escluso: un'attesa lunghissima resta comunque finita.
        let ms = u32::try_from(timeout.as_millis()).unwrap_or(INFINITE - 1).min(INFINITE - 1);
        // SAFETY: handle di processo valido, posseduto da `self`.
        if unsafe { WaitForSingleObject(self.raw(), ms) } != WAIT_OBJECT_0 {
            return None;
        }
        let mut code = 0u32;
        // SAFETY: handle valido; uscita locale. Il processo è terminato, quindi il codice è definitivo.
        (unsafe { GetExitCodeProcess(self.raw(), &mut code) } != 0).then_some(code)
    }

    /// Termina l'helper (annullamento o errore della pipe), con il codice [`TERMINATED`]. Se l'handle
    /// non ha il diritto di terminare, l'helper esce comunque appena la pipe si chiude (scrittura
    /// fallita, `exit::PIPE`).
    pub fn terminate(&self) {
        // SAFETY: handle di processo valido; un processo già uscito restituisce solo un errore.
        unsafe { TerminateProcess(self.raw(), TERMINATED) };
    }
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        // SAFETY: handle restituito da ShellExecuteExW con SEE_MASK_NOCLOSEPROCESS, chiuso una volta sola.
        unsafe { CloseHandle(self.raw()) };
    }
}

/// COM sul thread corrente per la durata del valore, come in `launch.rs`: `ShellExecuteExW` può
/// delegare a estensioni della shell che lo richiedono. Bilancia solo un'inizializzazione riuscita.
struct Com(bool);

impl Com {
    fn init() -> Self {
        // SAFETY: inizializzazione COM del thread corrente. S_OK e S_FALSE vanno bilanciati; con
        // RPC_E_CHANGED_MODE il thread è già in un altro modello e non si tocca.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Com(hr.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: bilancia la CoInitializeEx riuscita di `init` sullo stesso thread.
            unsafe { CoUninitialize() };
        }
    }
}

fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Percorso di un modulo o di una cartella di sistema letto con un buffer che cresce.
fn read_path(f: impl Fn(*mut u16, u32) -> u32) -> Option<String> {
    let mut buf = vec![0u16; 1024];
    loop {
        let n = f(buf.as_mut_ptr(), buf.len() as u32) as usize;
        if n == 0 {
            return None;
        }
        if n >= buf.len() {
            if buf.len() >= 1 << 16 {
                return None;
            }
            buf.resize(buf.len() * 2, 0);
            continue;
        }
        return Some(String::from_utf16_lossy(&buf[..n]));
    }
}

/// Parametri della riga di comando dell'helper: `--helper "<pipe>" <pid>`. Solo un nome di pipe della
/// forma di `Server::create` (lettere, cifre, `\`, `.`, `-`: niente virgolette né spazi che possano
/// aggiungere argomenti); le barre rovesciate non precedono mai una virgoletta, quindi restano letterali.
pub fn helper_parameters(pipe: &str, pid: u32) -> Option<String> {
    valid_name(pipe).then(|| format!("--helper \"{pipe}\" {pid}"))
}

/// Avvia l'helper elevato. `pipe`: nome restituito da `helper::pipe::Server::name`; `owner`: finestra
/// proprietaria del prompt UAC.
///
/// `ShellExecuteExW` con il verbo `runas`, `SEE_MASK_NOCLOSEPROCESS` (serve l'handle per pid, attesa e
/// terminazione), `SEE_MASK_NOASYNC` (la chiamata finisce prima del ritorno), `SEE_MASK_FLAG_NO_UI`
/// (nessuna finestra di errore della shell), finestra nascosta, cartella di lavoro `System32`.
pub fn launch_helper(pipe: &str, owner: Option<isize>) -> Result<HelperProcess, u32> {
    // SAFETY: nessun prerequisito.
    let me = unsafe { GetCurrentProcessId() };
    let params = helper_parameters(pipe, me).ok_or(ERROR_INVALID_PARAMETER)?;
    // SAFETY: modulo nullo = l'eseguibile del processo; buffer fornito da `read_path` con la sua lunghezza.
    let exe = read_path(|p, n| unsafe { GetModuleFileNameW(null_mut(), p, n) }).ok_or_else(|| {
        // SAFETY: nessun prerequisito.
        unsafe { GetLastError() }
    })?;
    // SAFETY: buffer fornito da `read_path` con la sua lunghezza.
    let dir = read_path(|p, n| unsafe { GetSystemDirectoryW(p, n) });
    let (exe, params, verb) = (wide_z(&exe), wide_z(&params), wide_z("runas"));
    let dir = dir.map(|d| wide_z(&d));
    let _com = Com::init();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        hwnd: owner.unwrap_or(0) as _,
        lpVerb: verb.as_ptr(),
        lpFile: exe.as_ptr(),
        lpParameters: params.as_ptr(),
        lpDirectory: dir.as_ref().map_or(null(), |d| d.as_ptr()),
        nShow: SW_HIDE,
        ..Default::default()
    };
    // SAFETY: `info` ha `cbSize` uguale alla sua dimensione; verbo, file, parametri e cartella sono
    // stringhe terminate da zero che vivono fino al ritorno. Con SEE_MASK_NOCLOSEPROCESS l'handle del
    // processo, se c'è, passa a `HelperProcess`, che lo chiude.
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        // SAFETY: nessun prerequisito. Prompt UAC rifiutato: ERROR_CANCELLED (1223).
        return Err(unsafe { GetLastError() });
    }
    if info.hProcess.is_null() {
        return Err(ERROR_INVALID_HANDLE);
    }
    let mut process = HelperProcess { pid: 0, handle: info.hProcess as isize };
    // SAFETY: handle di processo valido appena restituito.
    process.pid = unsafe { GetProcessId(process.raw()) };
    if process.pid == 0 {
        // SAFETY: nessun prerequisito. L'handle si chiude al drop di `process`.
        return Err(unsafe { GetLastError() });
    }
    Ok(process)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::io::IntoRawHandle;
    use std::process::{Command, Stdio};

    const PIPE: &str = r"\\.\pipe\quado-0123456789abcdef0123456789abcdef";

    /// Un `HelperProcess` su un processo qualunque, per provare attesa e terminazione senza UAC.
    fn adopt(child: std::process::Child) -> HelperProcess {
        let pid = child.id();
        HelperProcess { pid, handle: child.into_raw_handle() as isize }
    }

    #[test]
    fn parameters_only_for_well_formed_pipe_names() {
        assert_eq!(helper_parameters(PIPE, 42).as_deref(), Some(r#"--helper "\\.\pipe\quado-0123456789abcdef0123456789abcdef" 42"#));
        for bad in [r#"\\.\pipe\quado-x" --altro"#, r"C:\Windows\x", r"\\.\pipe\quado-0123", r"\\.\pipe\altro-0123456789abcdef0123456789abcdef", ""] {
            assert_eq!(helper_parameters(bad, 42), None, "{bad}");
        }
    }

    #[test]
    fn wait_returns_the_exit_code() {
        let child = Command::new("cmd.exe").args(["/d", "/c", "exit 7"]).stdin(Stdio::null()).stdout(Stdio::null()).spawn().unwrap();
        let p = adopt(child);
        assert_eq!(p.wait(Duration::from_secs(20)), Some(7));
    }

    #[test]
    fn terminate_ends_a_running_process() {
        // `set /p` attende una riga su stdin, che resta aperto: il processo non esce da solo.
        let mut child = Command::new("cmd.exe").args(["/d", "/c", "set /p x="]).stdin(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
        let stdin = child.stdin.take();
        let p = adopt(child);
        assert_eq!(p.wait(Duration::from_millis(300)), None, "il processo è ancora in esecuzione");
        p.terminate();
        assert_eq!(p.wait(Duration::from_secs(20)), Some(TERMINATED));
        drop(stdin);
    }

    /// Fa comparire il prompt UAC: non si esegue nei test automatici. Prova manuale, con qualcuno davanti
    /// al PC che risponde al prompt: rispondendo No l'esito è `Err(1223)`; rispondendo Sì l'helper parte, non trova un server
    /// con quel nome ed esce con `exit::PIPE` (5).
    #[test]
    #[ignore = "prompt UAC: solo a mano, con qualcuno che risponde al prompt"]
    fn launch_shows_the_uac_prompt() {
        match launch_helper(PIPE, None) {
            Err(code) => assert_eq!(code, 1223),
            Ok(p) => assert_eq!(p.wait(Duration::from_secs(60)), Some(5)),
        }
    }
}
