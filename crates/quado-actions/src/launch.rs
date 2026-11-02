// SPDX-License-Identifier: GPL-3.0-or-later
//! Disinstallazione tramite interfaccia ufficiale e procedure di Windows: il lanciatore accetta solo valori
//! tipizzati (URI di Impostazioni, `steam://uninstall/` con un numero, PFN validato, apertura del Cestino),
//! mai stringhe lette dal registro. `UninstallString` e `QuietUninstallString` non si eseguono mai.
//! Gli URI scritti nelle regole si riconoscono con [`Launch::for_rule_uri`], per confronto esatto con le
//! sole costanti note.
//!
//! Quado non crea processi: [`open`] chiede alla shell di aprire un URI con il verbo "open", senza
//! parametri e senza le finestre di errore della shell. Il nome del programma negli appunti lo
//! copia l'interfaccia, come "Copia percorso".
//!
//! Nessuna API pubblica accetta una stringa arbitraria da aprire: `Launch` non si costruisce da una
//! `String`, i campi di [`Pfn`] e [`SteamAppId`] sono privati e i loro costruttori validano. I test di
//! compilazione sotto riportano il codice d'errore atteso, controllato a mano il 14/10/2026: rustdoc
//! stabile non lo verifica.
//!
//! ```compile_fail,E0308
//! // Nessuna conversione da una stringa letta dal registro.
//! use quado_actions::launch::Launch;
//! let _ = Launch::from(String::new());
//! ```
//!
//! ```compile_fail,E0423
//! // Il campo del PFN è privato: si passa solo da `Pfn::new`, che valida.
//! use quado_actions::launch::Pfn;
//! let _ = Pfn(String::from("x"));
//! ```
//!
//! ```compile_fail,E0277
//! // Nessun `FromStr`.
//! use quado_actions::launch::Launch;
//! let _ = "x".parse::<Launch>();
//! ```
//!
//! ```compile_fail,E0423
//! // Anche l'appid ha il campo privato.
//! use quado_actions::launch::SteamAppId;
//! let _ = SteamAppId(0);
//! ```
//!
//! ```compile_fail,E0308
//! // `open` vuole un `Launch`, non un URI scritto a mano.
//! let _ = quado_actions::launch::open("ms-settings:appsfeatures");
//! ```
//!
//! ```compile_fail,E0308
//! // Anche l'URI di una regola diventa un `Launch` solo passando da `for_rule_uri`.
//! let _ = quado_actions::launch::open(&String::from("shell:RecycleBinFolder"));
//! ```
//!
//! ```
//! // Le sole costruzioni ammesse.
//! use quado_actions::launch::{Launch, Pfn, SteamAppId};
//! let pfn = Pfn::new("Microsoft.WindowsTerminal_8wekyb3d8bbwe").expect("PFN valido");
//! assert_eq!(Launch::AppsFeaturesApp(pfn).uri(), "ms-settings:appsfeatures-app?Microsoft.WindowsTerminal_8wekyb3d8bbwe");
//! let id = SteamAppId::new(3_999_910).expect("appid valido");
//! assert_eq!(Launch::SteamUninstall(id).uri(), "steam://uninstall/3999910");
//! assert_eq!(Launch::AppsFeatures.uri(), "ms-settings:appsfeatures");
//! assert_eq!(Launch::RecycleBin.uri(), "shell:RecycleBinFolder");
//! // Gli URI delle regole: solo le costanti note, per confronto esatto.
//! assert_eq!(Launch::for_rule_uri("ms-settings:storagesense"), Some(Launch::StorageSense));
//! assert_eq!(Launch::for_rule_uri("ms-settings:storagesense?x"), None);
//! assert_eq!(Launch::for_rule_uri("steam://uninstall/3999910"), None);
//! ```

use std::ffi::c_void;

use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::Shell::{SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, w};

use crate::shell::ShellError;

/// Lunghezza massima del nome nel nome di famiglia: lo schema del manifest (`ST_PackageName`) ammette
/// da 3 a 50 caratteri tra lettere ASCII, cifre, `.` e `-`.
const NAME_MIN: usize = 3;
const NAME_MAX: usize = 50;
/// Identificativo dell'editore nel nome di famiglia: 13 caratteri.
const PUBLISHER_ID: usize = 13;

/// Nome di famiglia di un pacchetto MSIX (`Nome_idEditore`), validato: nel nome solo lettere ASCII,
/// cifre, `.` e `-`; l'identificativo dell'editore di 13 caratteri alfanumerici. Spazi, `:`, `/`, `\`,
/// `?`, `&`, `#`, virgolette e ogni altro carattere sono rifiutati, quindi il PFN non può aggiungere
/// parametri all'URI né diventare un percorso.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Pfn(String);

impl Pfn {
    pub fn new(s: &str) -> Option<Pfn> {
        let (name, id) = s.rsplit_once('_')?;
        let name_ok = (NAME_MIN..=NAME_MAX).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
        let id_ok = id.len() == PUBLISHER_ID && id.bytes().all(|b| b.is_ascii_alphanumeric());
        (name_ok && id_ok).then(|| Pfn(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Identificativo di un'app di Steam, diverso da zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SteamAppId(u32);

impl SteamAppId {
    pub fn new(id: u32) -> Option<Self> {
        (id != 0).then_some(Self(id))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

/// Cosa apre il lanciatore: interfacce di disinstallazione e procedure di Windows.
/// Epic non ha un URI: la UI rimanda alla libreria del launcher.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Launch {
    /// Impostazioni ▸ App (`ms-settings:appsfeatures`), per i programmi Win32: il nome lo copia la UI.
    AppsFeatures,
    /// Pagina dell'app MSIX in Impostazioni (`ms-settings:appsfeatures-app?<PFN>`).
    AppsFeaturesApp(Pfn),
    /// Disinstallazione di un gioco da Steam (`steam://uninstall/<appid>`), che chiede conferma.
    SteamUninstall(SteamAppId),
    /// "Apri il Cestino" (`shell:RecycleBinFolder`): la cartella del Cestino in Esplora file, dove
    /// l'utente elimina solo gli elementi che sceglie.
    RecycleBin,
    /// Impostazioni › Sistema › Archiviazione (`ms-settings:storagesense`, pagina "Storage" della tabella
    /// di Microsoft citata in [`TEMP_FILES_URI`]).
    StorageSense,
    /// Impostazioni › Archiviazione › File temporanei: URI in [`TEMP_FILES_URI`], con la fonte.
    TempFiles,
}

/// URI della procedura File temporanei.
///
/// Fonte: Microsoft Learn, "Launch Windows Settings" (sezione "ms-settings: URI scheme reference",
/// System), `https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings`, pagina con
/// `ms.date` 26/09/2026, letta il 14/10/2026. La tabella non ha una riga per File temporanei; per
/// l'archiviazione elenca solo Storage (`ms-settings:storagesense`), Storage Sense
/// (`ms-settings:storagepolicies`), Storage recommendations (`ms-settings:storagerecommendations`, che in
/// `SystemSettings.dll` di Windows 10 22H2 build 19045.6466 non c'è) e Disks & volumes. Nella tabella
/// degli URI di `SystemSettings.dll` di questo PC le sole voci di archiviazione sono `storagesense` e
/// `storagepolicies`; `ms-settings:storagecleanup`, citato da siti non ufficiali, non compare.
///
/// Si apre quindi la pagina Archiviazione, dove File temporanei è una categoria a un clic, con l'elenco
/// delle voci e le caselle da scegliere. `ms-settings:storagepolicies` (Sensore memoria) non si usa: la sua
/// azione "Pulisci ora" elimina secondo le impostazioni di Sensore memoria (file temporanei, e Cestino e
/// Download oltre un'età) senza elencare cosa, cioè uno "svuota tutto" implicito; dedotto dalla
/// pagina di Windows 10, da confermare con la verifica. La verifica è un controllo manuale su questo PC
/// (`tests/procedure.rs`): se trova un URI diretto, cambia solo questa costante.
pub const TEMP_FILES_URI: &str = "ms-settings:storagesense";

/// URI costanti che una regola può scrivere nel suo metodo, con il `Launch` che li apre. Se due
/// varianti condividono l'URI vale la prima: Archiviazione prima di File temporanei.
const RULE_URIS: [Launch; 4] = [Launch::AppsFeatures, Launch::RecycleBin, Launch::StorageSense, Launch::TempFiles];

impl Launch {
    /// URI che il lanciatore apre, costruito solo da valori validati.
    pub fn uri(&self) -> String {
        match self {
            Self::AppsFeatures => "ms-settings:appsfeatures".to_owned(),
            Self::AppsFeaturesApp(p) => format!("ms-settings:appsfeatures-app?{}", p.as_str()),
            Self::SteamUninstall(id) => format!("steam://uninstall/{}", id.get()),
            Self::RecycleBin => "shell:RecycleBinFolder".to_owned(),
            Self::StorageSense => "ms-settings:storagesense".to_owned(),
            Self::TempFiles => TEMP_FILES_URI.to_owned(),
        }
    }

    /// Il `Launch` per l'URI scritto nel metodo di una regola (`metodo = { uri = "…" }`): solo le
    /// costanti note, per confronto esatto (maiuscole comprese). Qualunque altra stringa, compresi
    /// parametri aggiunti, `steam://` e `appsfeatures-app?` (che si costruiscono da valori validati), dà
    /// `None`: la regola non può far aprire un URI arbitrario.
    pub fn for_rule_uri(uri: &str) -> Option<Launch> {
        RULE_URIS.into_iter().find(|l| l.uri() == uri)
    }

    /// Codice della procedura nel registro (`InterventionKind::Procedure`); `None` per le interfacce di
    /// disinstallazione.
    pub fn procedure_code(&self) -> Option<&'static str> {
        match self {
            Self::RecycleBin => Some("cestino-apri"),
            Self::StorageSense => Some("archiviazione"),
            Self::TempFiles => Some("file-temporanei"),
            Self::AppsFeatures | Self::AppsFeaturesApp(_) | Self::SteamUninstall(_) => None,
        }
    }
}

/// `SHELLEXECUTEINFOW` di `shellapi.h`. La struttura del crate `windows` richiede la feature
/// `Win32_System_Registry` (per il campo `hkeyClass`), che il workspace non attiva: si dichiara qui con
/// la stessa disposizione, verificata dalle asserzioni sotto. I campi non usati restano nulli.
#[repr(C)]
struct ShellExecuteInfoW {
    cb_size: u32,
    f_mask: u32,
    hwnd: *mut c_void,
    lp_verb: *const u16,
    lp_file: *const u16,
    lp_parameters: *const u16,
    lp_directory: *const u16,
    n_show: i32,
    h_inst_app: *mut c_void,
    lp_id_list: *mut c_void,
    lp_class: *const u16,
    hkey_class: *mut c_void,
    dw_hot_key: u32,
    /// Unione `hIcon` / `hMonitor`.
    h_icon: *mut c_void,
    h_process: *mut c_void,
}

// Dimensioni documentate di `SHELLEXECUTEINFOW`: 112 byte a 64 bit, 60 a 32 bit.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(std::mem::size_of::<ShellExecuteInfoW>() == 112);
#[cfg(target_pointer_width = "32")]
const _: () = assert!(std::mem::size_of::<ShellExecuteInfoW>() == 60);

/// Importazione privata: fuori da questo modulo non si raggiunge.
mod ffi {
    use super::ShellExecuteInfoW;
    windows_core::link!("shell32.dll" "system" fn ShellExecuteExW(pexecinfo: *mut ShellExecuteInfoW) -> windows_core::BOOL);
}

/// COM sul thread corrente per la durata del valore, come in `shell.rs`: `ShellExecuteExW` può
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

/// Apre l'interfaccia ufficiale con `ShellExecuteExW`: verbo "open", nessun parametro, nessuna
/// finestra di errore della shell (`SEE_MASK_FLAG_NO_UI`). Non crea processi da Quado: è la shell ad
/// aprire Impostazioni o Steam. In caso di errore restituisce l'HRESULT (per esempio Steam assente:
/// nessun programma associato all'URI).
pub fn open(l: &Launch) -> Result<(), ShellError> {
    let _com = Com::init();
    let file = HSTRING::from(l.uri());
    let verb = w!("open");
    let mut info = ShellExecuteInfoW {
        cb_size: std::mem::size_of::<ShellExecuteInfoW>() as u32,
        f_mask: SEE_MASK_FLAG_NO_UI | SEE_MASK_NOASYNC,
        hwnd: std::ptr::null_mut(),
        lp_verb: verb.as_ptr(),
        lp_file: file.as_ptr(),
        lp_parameters: std::ptr::null(),
        lp_directory: std::ptr::null(),
        n_show: SW_SHOWNORMAL.0,
        h_inst_app: std::ptr::null_mut(),
        lp_id_list: std::ptr::null_mut(),
        lp_class: std::ptr::null(),
        hkey_class: std::ptr::null_mut(),
        dw_hot_key: 0,
        h_icon: std::ptr::null_mut(),
        h_process: std::ptr::null_mut(),
    };
    // SAFETY: `info` ha `cb_size` uguale alla sua dimensione e la disposizione di SHELLEXECUTEINFOW;
    // verbo e URI sono stringhe terminate da zero che vivono fino al ritorno (`verb` statica, `file`
    // locale). Senza SEE_MASK_NOCLOSEPROCESS la shell non restituisce handle da chiudere.
    let ok = unsafe { ffi::ShellExecuteExW(&mut info) };
    if ok.as_bool() { Ok(()) } else { Err(ShellError(windows::core::Error::from_thread().code().0)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_the_documented_size() {
        // Offset dei campi letti per primi dalla shell, a 64 bit (shellapi.h).
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(std::mem::offset_of!(ShellExecuteInfoW, lp_file), 24);
            assert_eq!(std::mem::offset_of!(ShellExecuteInfoW, n_show), 48);
            assert_eq!(std::mem::offset_of!(ShellExecuteInfoW, h_process), 104);
        }
    }
}
