// SPDX-License-Identifier: GPL-3.0-or-later
//! Impostazioni e fatti di Windows letti in sola lettura per l'interfaccia (05 D13, 04 §6.1, 05 D2):
//! lingue preferite dell'utente, formato dell'area geografica, ora locale, "Effetti di animazione",
//! SID dell'utente corrente, scheda video che guida un monitor; i font di Windows usati come ripiego,
//! proiettati in memoria in sola lettura.
//!
//! Come il resto del crate: nessuna scrittura, nessun processo creato. Le funzioni restituiscono valori,
//! mai frasi (05 D13).

use std::mem::{MaybeUninit, size_of};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, LocalFree, SYSTEMTIME};
use windows_sys::Win32::Globalization::{
    GetLocaleInfoEx, GetUserPreferredUILanguages, LOCALE_SDECIMAL, LOCALE_SGROUPING, LOCALE_SSHORTDATE, LOCALE_STHOUSAND,
    MUI_LANGUAGE_NAME,
};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows_sys::Win32::System::SystemInformation::GetSystemTimePreciseAsFileTime;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};
use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW};

/// Lingue dell'interfaccia preferite dall'utente, in ordine (`it-IT`, `en-US`, ...).
pub fn ui_languages() -> Vec<String> {
    let mut count = 0u32;
    let mut len = 0u32;
    // SAFETY: prima chiamata con buffer nullo: restituisce solo la lunghezza necessaria in `len`.
    if unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, null_mut(), &mut len) } == 0 || len == 0 {
        return Vec::new();
    }
    let mut buf = vec![0u16; len as usize];
    // SAFETY: `buf` ha esattamente `len` unità, come chiesto dalla prima chiamata.
    if unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, buf.as_mut_ptr(), &mut len) } == 0 {
        return Vec::new();
    }
    // Elenco di stringhe terminate da zero, chiuso da uno zero in più.
    buf.split(|&u| u == 0).filter(|s| !s.is_empty()).map(String::from_utf16_lossy).collect()
}

fn locale_info(kind: u32) -> Option<String> {
    let mut buf = [0u16; 96];
    // SAFETY: località dell'utente (puntatore nullo = LOCALE_NAME_USER_DEFAULT); buffer locale con la sua lunghezza.
    let n = unsafe { GetLocaleInfoEx(null(), kind, buf.as_mut_ptr(), buf.len() as i32) };
    (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
}

/// Formato dei numeri e delle date dell'area geografica di Windows (Impostazioni ▸ Data/ora e lingua ▸ Area geografica).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionFormat {
    pub decimal: String,
    pub thousands: String,
    /// Raggruppamento delle cifre come lo scrive Windows (`3;0` per gruppi di tre, `3;2;0` per l'India).
    pub grouping: String,
    /// Data breve, per esempio `dd/MM/yyyy`.
    pub short_date: String,
}

impl Default for RegionFormat {
    /// Formato italiano: serve solo se Windows non risponde.
    fn default() -> Self {
        Self { decimal: ",".into(), thousands: ".".into(), grouping: "3;0".into(), short_date: "dd/MM/yyyy".into() }
    }
}

pub fn region_format() -> RegionFormat {
    let d = RegionFormat::default();
    RegionFormat {
        decimal: locale_info(LOCALE_SDECIMAL).unwrap_or(d.decimal),
        thousands: locale_info(LOCALE_STHOUSAND).unwrap_or(d.thousands),
        grouping: locale_info(LOCALE_SGROUPING).unwrap_or(d.grouping),
        short_date: locale_info(LOCALE_SSHORTDATE).unwrap_or(d.short_date),
    }
}

/// Ora locale di un istante.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
}

/// Converte un `FILETIME` (intervalli di 100 ns dal 1/1/1601, UTC) nell'ora locale di quel giorno,
/// con l'ora legale in vigore allora.
pub fn local_time(filetime: i64) -> Option<LocalTime> {
    if filetime <= 0 {
        return None;
    }
    let ft = FILETIME { dwLowDateTime: filetime as u32, dwHighDateTime: (filetime >> 32) as u32 };
    let mut utc = MaybeUninit::<SYSTEMTIME>::zeroed();
    let mut local = MaybeUninit::<SYSTEMTIME>::zeroed();
    // SAFETY: strutture locali inizializzate a zero; il fuso nullo è quello corrente del sistema.
    let ok = unsafe {
        FileTimeToSystemTime(&ft, utc.as_mut_ptr()) != 0 && SystemTimeToTzSpecificLocalTime(null(), utc.as_ptr(), local.as_mut_ptr()) != 0
    };
    if !ok {
        return None;
    }
    // SAFETY: scritta dalla chiamata riuscita qui sopra.
    let s = unsafe { local.assume_init() };
    Some(LocalTime { year: s.wYear, month: s.wMonth, day: s.wDay, hour: s.wHour, minute: s.wMinute })
}

/// Ora attuale come `FILETIME`, al decimo di microsecondo (`GetSystemTimePreciseAsFileTime`): le misure degli
/// interventi devono avere un'ora strettamente precedente alla prima chiamata a Windows (F-VE-1), e
/// `GetSystemTimeAsFileTime` avanza a passi di circa 15,6 ms.
pub fn now_filetime() -> i64 {
    let mut ft = MaybeUninit::<FILETIME>::zeroed();
    // SAFETY: scrive la struttura locale.
    unsafe { GetSystemTimePreciseAsFileTime(ft.as_mut_ptr()) };
    // SAFETY: scritta qui sopra.
    let ft = unsafe { ft.assume_init() };
    (i64::from(ft.dwHighDateTime) << 32) | i64::from(ft.dwLowDateTime)
}

/// "Mostra animazioni in Windows" (Impostazioni ▸ Accessibilità ▸ Schermo). Vero se Windows non risponde.
pub fn animations_enabled() -> bool {
    let mut on: i32 = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION scrive un BOOL nel puntatore dato; nessuna modifica alle impostazioni.
    let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut on).cast(), 0) };
    ok == 0 || on != 0
}

/// SID dell'utente corrente in forma testuale (`S-1-5-21-...`): serve a riconoscere i cestini di altri account.
pub fn user_sid() -> Option<String> {
    let mut token: HANDLE = null_mut();
    // SAFETY: token del processo corrente aperto in sola lettura (TOKEN_QUERY), chiuso sotto.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return None;
    }
    let mut buf = vec![0u8; 256];
    let mut len = 0u32;
    // SAFETY: buffer locale con la sua dimensione; `token` è valido.
    let ok = unsafe { GetTokenInformation(token, TokenUser, buf.as_mut_ptr().cast(), buf.len() as u32, &mut len) } != 0;
    // SAFETY: handle aperto qui sopra.
    unsafe { CloseHandle(token) };
    if !ok || (len as usize) < size_of::<TOKEN_USER>() {
        return None;
    }
    // SAFETY: il buffer contiene una TOKEN_USER scritta da Windows; la lettura è non allineata.
    let user: TOKEN_USER = unsafe { buf.as_ptr().cast::<TOKEN_USER>().read_unaligned() };
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

/// Scheda video che guida il monitor sotto il punto (in pixel dello schermo).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayAdapter {
    pub vendor: u32,
    pub device: u32,
    pub name: String,
}

/// La scheda video collegata al monitor che contiene `(x, y)`, o al monitor principale se il punto è
/// fuori da ogni monitor. Disegnare con un'altra scheda costa una copia tra GPU a ogni frame
/// (7-9 ms su un portatile con due schede video, M0).
pub fn adapter_for_point(x: i32, y: i32) -> Option<DisplayAdapter> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
    use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint};

    // SAFETY: chiamate DXGI e GDI di sola lettura; gli oggetti COM si rilasciano al drop.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTOPRIMARY);
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut i = 0;
        while let Ok(adapter) = factory.EnumAdapters1(i) {
            i += 1;
            let Ok(desc) = adapter.GetDesc1() else { continue };
            let mut j = 0;
            while let Ok(output) = adapter.EnumOutputs(j) {
                j += 1;
                if output.GetDesc().is_ok_and(|o| o.Monitor == monitor) {
                    let n = desc.Description.iter().position(|&u| u == 0).unwrap_or(desc.Description.len());
                    return Some(DisplayAdapter {
                        vendor: desc.VendorId,
                        device: desc.DeviceId,
                        name: String::from_utf16_lossy(&desc.Description[..n]),
                    });
                }
            }
        }
        None
    }
}

/// Un font di Windows (`%WINDIR%\Fonts`) proiettato in memoria in sola lettura per tutta la vita del processo,
/// per i font di ripiego dell'interfaccia (05 D2): egui lo usa senza copiarlo, e Windows carica solo le
/// pagine lette (tabelle e glifi usati), condivise con gli altri processi che usano lo stesso font. Non è
/// memoria privata del processo, a differenza di un file letto.
///
/// Il file resta aperto fino alla fine del processo, condiviso solo in lettura (né scrittura né
/// cancellazione): il contenuto non può cambiare sotto la proiezione. Se un altro processo lo tiene aperto in
/// scrittura, l'apertura fallisce e chi chiama legge il file. Errore: codice di Windows.
pub fn map_font(path: &std::path::Path) -> Result<&'static [u8], u32> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{ERROR_FILE_INVALID, GENERIC_READ, GetLastError, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, GetFileSizeEx, OPEN_EXISTING};
    use windows_sys::Win32::System::Memory::{CreateFileMappingW, FILE_MAP_READ, MapViewOfFile, PAGE_READONLY};

    let w: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    // SAFETY: `w` è terminata da zero; nessun attributo di sicurezza, nessun file modello.
    let file = unsafe { CreateFileW(w.as_ptr(), GENERIC_READ, FILE_SHARE_READ, null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, null_mut()) };
    if file == INVALID_HANDLE_VALUE {
        // SAFETY: subito dopo la chiamata fallita.
        return Err(unsafe { GetLastError() });
    }
    let fail = |handles: &[HANDLE]| -> u32 {
        // SAFETY: subito dopo la chiamata fallita; gli handle sono aperti da questa funzione e non si usano più.
        let code = unsafe { GetLastError() };
        for &h in handles {
            // SAFETY: vedi sopra.
            unsafe { CloseHandle(h) };
        }
        code
    };
    let mut size = 0i64;
    // SAFETY: `file` è un handle valido, `size` una variabile locale.
    if unsafe { GetFileSizeEx(file, &mut size) } == 0 {
        return Err(fail(&[file]));
    }
    let Ok(len) = usize::try_from(size) else { return Err(fail(&[file])) };
    if len == 0 {
        // Un file vuoto non si proietta.
        fail(&[file]);
        return Err(ERROR_FILE_INVALID);
    }
    // SAFETY: `file` è aperto in lettura; proiezione di tutto il file in sola lettura, senza nome.
    let mapping = unsafe { CreateFileMappingW(file, null(), PAGE_READONLY, 0, 0, null()) };
    if mapping.is_null() {
        return Err(fail(&[file]));
    }
    // SAFETY: `mapping` è valido; vista di tutto il file in sola lettura.
    let view = unsafe { MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0) };
    if view.Value.is_null() {
        return Err(fail(&[mapping, file]));
    }
    // La vista tiene viva la proiezione: il suo handle si chiude. L'handle del file invece resta aperto (non
    // si chiude mai), così nessuno può aprirlo in scrittura finché la vista esiste.
    // SAFETY: `mapping` è aperto da questa funzione e non si usa più.
    unsafe { CloseHandle(mapping) };
    // SAFETY: la vista copre `len` byte leggibili (tutto il file), non si chiude mai e il contenuto non cambia
    // (file aperto senza condivisione in scrittura): la fetta vale per tutta la vita del processo.
    Ok(unsafe { std::slice::from_raw_parts(view.Value.cast::<u8>(), len) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_and_languages_answer() {
        let f = region_format();
        assert!(!f.decimal.is_empty());
        assert!(!f.short_date.is_empty());
        assert!(!ui_languages().is_empty());
    }

    #[test]
    fn local_time_of_a_known_instant() {
        // 13/10/2026 12:00 UTC: in Italia 14:00 (ora legale). Il test vale su qualunque fuso: controlla il giorno.
        let ft = 134_363_664_000_000_000_i64;
        let t = local_time(ft).expect("ora locale");
        assert_eq!((t.year, t.month), (2026, 10));
        assert!((12..=14).contains(&t.day));
        assert!(now_filetime() > ft - 10_000_000 * 86_400 * 365);
    }

    #[test]
    fn user_sid_is_a_sid() {
        assert!(user_sid().expect("SID").starts_with("S-1-5-"));
    }

    #[test]
    fn primary_monitor_has_an_adapter() {
        let a = adapter_for_point(0, 0).expect("scheda del monitor principale");
        assert!(a.vendor != 0);
    }

    #[test]
    fn a_mapped_font_reads_like_the_file() {
        let Some(dir) = std::env::var_os("WINDIR").map(|w| std::path::PathBuf::from(w).join("Fonts")) else { return };
        let path = dir.join("segoeui.ttf");
        if !path.is_file() {
            return;
        }
        let mapped = map_font(&path).expect("proiezione");
        assert_eq!(mapped, std::fs::read(&path).expect("lettura").as_slice());
        // Un file che non c'è: codice di Windows, nessuna proiezione.
        assert_eq!(map_font(&dir.join("non-esiste-quado.ttf")), Err(2));
    }
}
