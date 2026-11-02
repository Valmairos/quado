// SPDX-License-Identifier: GPL-3.0-or-later
//! Processo: ricerca delle DLL, moduli caricati, metriche, elevazione.

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, HMODULE};
use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows_sys::Win32::System::LibraryLoader::{LOAD_LIBRARY_SEARCH_SYSTEM32, SetDefaultDllDirectories};
use windows_sys::Win32::System::ProcessStatus::{EnumProcessModules, GetModuleFileNameExW};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Prima chiamata di `main`: le DLL caricate a runtime si cercano solo in System32, così una DLL esca
/// accanto all'eseguibile non si carica.
/// Le dipendenze statiche sono coperte dal flag del linker `/DEPENDENTLOADFLAG:0x800`.
pub fn harden_dll_search() -> bool {
    // SAFETY: nessun prerequisito; cambia solo l'ordine di ricerca delle DLL del processo corrente.
    unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32) != 0 }
}

/// Percorsi dei moduli caricati nel processo corrente.
pub fn loaded_modules() -> Vec<String> {
    let mut mods: Vec<HMODULE> = vec![std::ptr::null_mut(); 512];
    let mut needed = 0u32;
    // SAFETY: pseudo-handle del processo corrente; buffer della dimensione dichiarata in byte.
    let ok = unsafe {
        EnumProcessModules(GetCurrentProcess(), mods.as_mut_ptr(), (mods.len() * size_of::<HMODULE>()) as u32, &mut needed)
    };
    if ok == 0 {
        return Vec::new();
    }
    let n = (needed as usize / size_of::<HMODULE>()).min(mods.len());
    mods[..n]
        .iter()
        .map(|&m| {
            let mut buf = [0u16; 1024];
            // SAFETY: modulo restituito da EnumProcessModules; buffer di 1024 caratteri dichiarato.
            let len = unsafe { GetModuleFileNameExW(GetCurrentProcess(), m, buf.as_mut_ptr(), buf.len() as u32) } as usize;
            String::from_utf16_lossy(&buf[..len])
        })
        .collect()
}

/// Vero se il processo corrente è elevato (`TokenElevation`), cioè avviato con «Esegui come amministratore».
/// È un fatto del processo, non dell'account: con il Controllo dell'account utente (UAC) anche un
/// amministratore avvia i programmi senza elevazione. Errore: codice di Windows.
pub fn elevated() -> Result<bool, u32> {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: token del processo corrente aperto in sola lettura (TOKEN_QUERY), chiuso sotto.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        // SAFETY: subito dopo la chiamata fallita.
        return Err(unsafe { GetLastError() });
    }
    let mut info = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut len = 0u32;
    // SAFETY: `token` è valido; l'uscita è la TOKEN_ELEVATION locale, con la sua dimensione.
    let ok = unsafe { GetTokenInformation(token, TokenElevation, (&raw mut info).cast(), size_of::<TOKEN_ELEVATION>() as u32, &mut len) } != 0;
    // SAFETY: subito dopo la chiamata, prima di CloseHandle.
    let code = if ok { 0 } else { unsafe { GetLastError() } };
    // SAFETY: handle aperto qui sopra, chiuso una volta sola.
    unsafe { CloseHandle(token) };
    if ok { Ok(info.TokenIsElevated != 0) } else { Err(code) }
}

/// Usa `version.dll` (letture delle versioni): serve anche alla prova della DLL esca di
/// `cargo xtask verifica-dll`, che mette una copia di `version.dll` accanto all'eseguibile.
pub fn file_version_info_size(path: &str) -> u32 {
    let z: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let mut handle = 0u32;
    // SAFETY: `z` terminata da zero; `handle` è un'uscita locale. La funzione legge solo la risorsa
    // di versione del file indicato.
    unsafe { windows_sys::Win32::Storage::FileSystem::GetFileVersionInfoSizeW(z.as_ptr(), &mut handle) }
}

/// Un handle del processo corrente con l'accesso concesso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnHandle {
    pub value: usize,
    pub granted_access: u32,
    pub object_type: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct HandleEntry {
    handle_value: usize,
    handle_count: usize,
    pointer_count: usize,
    granted_access: u32,
    object_type_index: u32,
    handle_attributes: u32,
    reserved: u32,
}

/// Handle aperti dal processo corrente (`ProcessHandleInformation`), senza privilegi.
pub fn own_handles() -> Vec<OwnHandle> {
    use windows_sys::Wdk::System::Threading::{NtQueryInformationProcess, ProcessHandleInformation};
    let mut buf: Vec<u64> = vec![0; 1 << 14];
    loop {
        let mut needed = 0u32;
        let bytes = (buf.len() * 8) as u32;
        // SAFETY: pseudo-handle del processo corrente; buffer allineato a 8 e grande `bytes` byte;
        // se non basta la funzione restituisce STATUS_INFO_LENGTH_MISMATCH e la dimensione necessaria.
        let status = unsafe { NtQueryInformationProcess(GetCurrentProcess(), ProcessHandleInformation, buf.as_mut_ptr().cast(), bytes, &mut needed) };
        if status == 0xC000_0004_u32 as i32 {
            buf = vec![0; (needed as usize).div_ceil(8) + 1024];
            continue;
        }
        if status < 0 {
            return Vec::new();
        }
        let n = buf[0] as usize;
        let max = (buf.len() * 8 - 16) / size_of::<HandleEntry>();
        // SAFETY: la struttura PROCESS_HANDLE_SNAPSHOT_INFORMATION inizia con due ULONG_PTR seguiti
        // da `NumberOfHandles` voci da 40 byte, scritte dal sistema nel buffer (limitate a `max`).
        let entries = unsafe { std::slice::from_raw_parts(buf.as_ptr().add(2).cast::<HandleEntry>(), n.min(max)) };
        return entries
            .iter()
            .map(|e| OwnHandle { value: e.handle_value, granted_access: e.granted_access, object_type: e.object_type_index })
            .collect();
    }
}

/// Millisecondi dalla creazione del processo: comprende il caricamento dell'eseguibile dal disco,
/// quindi misura l'avvio come lo vede l'utente, fino al primo frame interattivo.
pub fn ms_since_start() -> f64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetProcessTimes;
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: strutture locali; pseudo-handle del processo corrente.
    let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user) };
    if ok == 0 {
        return f64::NAN;
    }
    let c = (i64::from(created.dwHighDateTime) << 32) | i64::from(created.dwLowDateTime);
    (crate::system::now_filetime() - c) as f64 / 10_000.0
}

#[cfg(test)]
mod tests {
    /// Windows risponde alla domanda senza errori. La risposta dipende da dove girano le prove: no su un PC
    /// di sviluppo, sì sui runner Windows di GitHub Actions, che girano come amministratore.
    #[test]
    fn windows_answers_whether_the_process_is_elevated() {
        assert!(super::elevated().is_ok(), "{:?}", super::elevated());
    }
}
