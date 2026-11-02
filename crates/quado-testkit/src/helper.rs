// SPDX-License-Identifier: GPL-3.0-or-later
//! Contenuto di prova dell'helper elevato sul VHDX NTFS di prova (`R:`), creato senza
//! elevazione, e i valori attesi misurati con l'oracolo prima di chiudere le ACL.
//!
//! Contenuto, sotto la radice del VHDX:
//! - `m6-helper\negata`: cartella con un'ACL di negazione per l'utente (nega l'elenco), con file di
//!   dimensioni note, un hard link (i due nomi dentro la cartella) e sottocartelle;
//! - `Users\altro-m6`: profilo finto di un altro utente; la DACL dà tutto il controllo solo al SID
//!   inventato [`PROFILE_SID`] (all'utente i soli diritti di preparazione, sotto);
//! - `$Recycle.Bin\S-1-5-18` (esiste: SYSTEM) e `$Recycle.Bin\S-1-5-21-1-2-3-9998` (orfano), ciascuna
//!   con qualche file con i nomi del Cestino (`$I...`, `$R...`) e la DACL dello stesso tipo, per quel SID.
//!   Un utente normale può creare cartelle in `$Recycle.Bin` (`BUILTIN\Users` vi ha "aggiungi
//!   sottocartella"): così nasce anche la sua.
//!
//! # Come si chiudono le cartelle senza `WRITE_DAC`
//!
//! Su `R:` i processi dell'utente non elevati non ottengono `WRITE_DAC` né `WRITE_OWNER`, neanche sulle proprie
//! cartelle con controllo completo (errore 5 su `CreateFileW` con quei diritti; su C: sì; misurato il
//! 14/10/2026 con `fine_vhdx.rs`, e di nuovo il 16/10/2026). `icacls` e `SetFileSecurityW` quindi non
//! funzionano lì. Le ACL si danno alla creazione (`CreateDirectoryW` con un descrittore), che non chiede
//! `WRITE_DAC`:
//! 1. il contenuto si crea in una cartella di preparazione con le ACL ereditate (leggibile);
//! 2. l'oracolo la misura come utente: sono i valori attesi della prova elevata;
//! 3. la cartella finale si crea vuota con il descrittore chiuso, e i figli diretti ci si spostano dentro
//!    con un rinomino (stesso volume: identità, dimensioni e allocate non cambiano).
//!
//! Il descrittore chiuso lascia all'utente i soli diritti per riempire e svuotare la cartella senza
//! elencarla: aggiungere e togliere figli, cancellarla, leggerne gli attributi e il descrittore
//! ([`PREP_RIGHTS`]). Elencare, cioè quello che fa lo scanner, resta negato all'utente e al suo token
//! elevato senza `SeBackupPrivilege`. I figli spostati tengono le ACL della preparazione: la pulizia li
//! cancella per nome, dal layout noto, senza elencare la cartella chiusa.
//!
//! Il contenuto resta su `R:` solo durante una prova: la prova sui VHDX di `quado-scan-win`
//! (`vhdx.rs`) vuole che su `R:` le sole cartelle negate siano in `System Volume Information`.

use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser};
use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use crate::oracle;
use crate::trees::{mounted_vhdx, write_file};

/// SID inventato del profilo finto (nessun account).
pub const PROFILE_SID: &str = "S-1-5-21-1-2-3-9999";
/// Cestino di un account che esiste: SYSTEM.
pub const SYSTEM_SID: &str = "S-1-5-18";
/// Cestino di un account che non esiste più.
pub const ORPHAN_BIN_SID: &str = "S-1-5-21-1-2-3-9998";

/// Diritti dell'utente sulle cartelle chiuse: `FILE_ADD_FILE`, `FILE_ADD_SUBDIRECTORY`, `FILE_TRAVERSE`,
/// `FILE_DELETE_CHILD`, `FILE_READ_ATTRIBUTES`, `DELETE`, `READ_CONTROL`, `SYNCHRONIZE`. Senza
/// `FILE_LIST_DIRECTORY` (0x1) e `FILE_READ_EA` (0x8).
pub const PREP_RIGHTS: u32 = 0x0013_00E6;
/// Diritti negati all'utente su `negata`: `FILE_LIST_DIRECTORY` e `FILE_READ_EA`.
pub const DENIED_RIGHTS: u32 = 0x9;

/// Valori di un sottoalbero come li conta lo scanner: logica e allocata uniche (ogni file con più nomi
/// contato una volta), file unici, cartelle esclusa la radice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Measured {
    pub logical: u64,
    pub alloc: u64,
    pub files: u64,
    pub dirs: u64,
}

/// Un nome di file, relativo alla cartella, con i valori dell'oracolo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedFile {
    pub rel: String,
    pub logical: u64,
    pub alloc: u64,
}

#[derive(Clone, Debug)]
pub struct ExpectedFolder {
    /// Percorso assoluto della cartella chiusa (`R:\m6-helper\negata`).
    pub path: String,
    pub totals: Measured,
    /// Tutti i nomi di file, hard link compresi, ordinati.
    pub files: Vec<ExpectedFile>,
}

#[derive(Clone, Debug)]
pub struct ExpectedBin {
    pub sid: &'static str,
    /// Il SID non corrisponde a nessun account.
    pub orphan: bool,
    pub folder: ExpectedFolder,
}

/// Valori attesi della prova elevata, misurati dall'oracolo come utente prima della chiusura.
#[derive(Clone, Debug)]
pub struct Expected {
    /// Radice del volume (`R:\`).
    pub volume: String,
    pub denied: ExpectedFolder,
    pub profile: ExpectedFolder,
    pub bins: Vec<ExpectedBin>,
}

impl Expected {
    /// Le quattro cartelle chiuse.
    pub fn folders(&self) -> Vec<&ExpectedFolder> {
        let mut v = vec![&self.denied, &self.profile];
        v.extend(self.bins.iter().map(|b| &b.folder));
        v
    }

    /// Tabella in Markdown per il resoconto.
    pub fn to_markdown(&self) -> String {
        let mut s = String::from("| Cartella | Tipo | Logica | Allocata | File | Cartelle |\n|---|---|---:|---:|---:|---:|\n");
        let mut row = |f: &ExpectedFolder, kind: &str| {
            let t = f.totals;
            s.push_str(&format!("| `{}` | {kind} | {} | {} | {} | {} |\n", f.path, t.logical, t.alloc, t.files, t.dirs));
        };
        row(&self.denied, "negata");
        row(&self.profile, "profilo altrui");
        for b in &self.bins {
            row(&b.folder, if b.orphan { "Cestino, SID orfano" } else { "Cestino, SID esistente" });
        }
        s
    }
}

/// Una cartella del contenuto: file (percorso relativo, byte, riempimento) e hard link (nuovo nome,
/// nome esistente). Le sottocartelle nascono dai percorsi.
struct Spec {
    files: &'static [(&'static str, usize, u8)],
    links: &'static [(&'static str, &'static str)],
}

impl Spec {
    /// Figli diretti della cartella: si spostano e si cancellano per nome.
    fn top_children(&self) -> Vec<&'static str> {
        let mut v: Vec<&'static str> = Vec::new();
        for p in self.files.iter().map(|f| f.0).chain(self.links.iter().map(|l| l.0)) {
            let first = p.split('\\').next().unwrap_or(p);
            if !v.contains(&first) {
                v.push(first);
            }
        }
        v
    }
}

const DENIED: Spec = Spec {
    files: &[
        ("a.bin", 100_000, 1),
        ("piccolo.txt", 700, 2),
        (r"sotto\c.bin", 1_234_567, 3),
        (r"sotto\profonda\d.bin", 4096, 4),
    ],
    links: &[(r"sotto\a-link.bin", "a.bin")],
};

const PROFILE: Spec = Spec {
    files: &[
        (r"Documents\lettera-m6.txt", 3_000, 5),
        (r"Desktop\foto-m6.jpg", 250_000, 6),
        (r"AppData\Local\cache-m6\x.bin", 1_000_000, 7),
        ("NTUSER-m6.DAT", 262_144, 8),
    ],
    links: &[(r"Desktop\foto-m6-copia.jpg", r"Desktop\foto-m6.jpg")],
};

const BIN_SYSTEM: Spec = Spec {
    files: &[("$IM6S001.txt", 98, 9), ("$RM6S001.txt", 300_000, 10), (r"$RM6S002\dentro.bin", 40_000, 11)],
    links: &[],
};

const BIN_ORPHAN: Spec = Spec { files: &[("$IM6O001.bin", 98, 12), ("$RM6O001.bin", 2_000_000, 13)], links: &[] };

/// Radice del VHDX NTFS di prova montato (`R:\`); senza, la prova si ferma.
pub fn volume() -> PathBuf {
    mounted_vhdx()
        .into_iter()
        .find(|(n, _)| n == "ntfs")
        .map(|(_, p)| p)
        .expect("VHDX di prova NTFS non montato: cargo xtask vhdx crea")
}

/// La cartella del Cestino sulla radice, con il nome che ha sul disco (`$RECYCLE.BIN` su `R:`).
fn bin_dir(vol: &Path) -> Option<PathBuf> {
    fs::read_dir(vol).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("$Recycle.Bin"))
    })
}

/// Una delle quattro cartelle chiuse.
struct Place {
    /// Nome della cartella di preparazione.
    key: &'static str,
    /// Cartella finale, chiusa.
    path: PathBuf,
    spec: &'static Spec,
    /// Descrittore della cartella finale.
    sddl: String,
}

/// Le quattro cartelle, nell'ordine di [`Expected`]: negata, profilo, Cestino di SYSTEM, Cestino orfano.
/// Le ultime due solo se il volume ha già la cartella del Cestino.
fn layout(vol: &Path, user: &str) -> Vec<Place> {
    let closed = |sid: &str| format!("D:P(A;OICI;FA;;;{sid})(A;;{PREP_RIGHTS:#x};;;{user})");
    let place = |key, path, spec, sddl| Place { key, path, spec, sddl };
    let mut v = vec![
        place(
            "negata",
            vol.join(r"m6-helper\negata"),
            &DENIED,
            format!("D:P(D;;{DENIED_RIGHTS:#x};;;{user})(A;;FA;;;{user})(A;;FA;;;SY)(A;;FA;;;BA)"),
        ),
        place("altro-m6", vol.join(r"Users\altro-m6"), &PROFILE, closed(PROFILE_SID)),
    ];
    if let Some(bin) = bin_dir(vol) {
        v.push(place("S-1-5-18", bin.join(SYSTEM_SID), &BIN_SYSTEM, closed(SYSTEM_SID)));
        v.push(place("S-1-5-21-1-2-3-9998", bin.join(ORPHAN_BIN_SID), &BIN_ORPHAN, closed(ORPHAN_BIN_SID)));
    }
    v
}

fn wide_z(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

fn last_error() -> u32 {
    // SAFETY: nessun prerequisito.
    unsafe { GetLastError() }
}

/// SID dell'utente del processo, in forma `S-1-...`.
pub fn current_user_sid() -> Result<String, u32> {
    let mut h: HANDLE = null_mut();
    // SAFETY: pseudo-handle del processo corrente; `h` riceve un handle del token chiuso sotto.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut h) } == 0 {
        return Err(last_error());
    }
    // `TOKEN_USER` più il SID che lo segue: 256 byte bastano (un SID è al massimo 68 byte); `u64` per
    // l'allineamento della struttura.
    let mut buf = [0u64; 32];
    let mut len = 0u32;
    // SAFETY: buffer di 256 byte dichiarato come tale, allineato a 8; handle con TOKEN_QUERY.
    let ok = unsafe { GetTokenInformation(h, TokenUser, buf.as_mut_ptr().cast(), 256, &mut len) };
    let e = last_error();
    // SAFETY: handle aperto sopra, chiuso una volta sola.
    unsafe { CloseHandle(h) };
    if ok == 0 {
        return Err(e);
    }
    // SAFETY: la chiamata è riuscita: il buffer inizia con una TOKEN_USER il cui SID punta dentro il buffer.
    let sid = unsafe { (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut s: *mut u16 = null_mut();
    // SAFETY: SID valido (sopra); `s` riceve una stringa allocata con LocalAlloc, liberata sotto.
    if unsafe { ConvertSidToStringSidW(sid, &mut s) } == 0 {
        return Err(last_error());
    }
    // SAFETY: `s` è terminata da zero (contratto della funzione); la si legge e poi la si libera.
    let out = unsafe {
        let n = (0..).take_while(|&i| *s.add(i) != 0).count();
        let v = String::from_utf16_lossy(std::slice::from_raw_parts(s, n));
        LocalFree(s.cast());
        v
    };
    Ok(out)
}

/// Crea la cartella `path` con il descrittore `sddl` (DACL protetta): l'ACL si dà alla creazione, senza
/// `WRITE_DAC`.
fn create_dir_with(path: &Path, sddl: &str) -> Result<(), String> {
    let z: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
    let mut sd: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: `z` terminata da zero; `sd` riceve un descrittore allocato con LocalAlloc, liberato sotto.
    if unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(z.as_ptr(), SDDL_REVISION_1, &mut sd, null_mut()) } == 0 {
        return Err(format!("SDDL non valido ({}): {sddl}", last_error()));
    }
    let sa = SECURITY_ATTRIBUTES { nLength: size_of::<SECURITY_ATTRIBUTES>() as u32, lpSecurityDescriptor: sd, bInheritHandle: 0 };
    let p = wide_z(path);
    // SAFETY: percorso terminato da zero; `sa` punta a un descrittore valido per tutta la chiamata.
    let ok = unsafe { CreateDirectoryW(p.as_ptr(), &sa) };
    let e = last_error();
    // SAFETY: descrittore allocato da ConvertStringSecurityDescriptorToSecurityDescriptorW, liberato una volta.
    unsafe { LocalFree(sd) };
    if ok == 0 { Err(format!("{}: CreateDirectoryW {e}", path.display())) } else { Ok(()) }
}

fn rel_string(p: &Path, base: &Path) -> String {
    p.strip_prefix(base).map(|r| r.to_string_lossy().into_owned()).unwrap_or_else(|_| p.to_string_lossy().into_owned())
}

/// Crea il contenuto di `spec` in `dir` e lo misura con l'oracolo.
fn build_and_measure(dir: &Path, spec: &Spec, final_path: &Path) -> Result<ExpectedFolder, String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for &(rel, bytes, fill) in spec.files {
        write_file(&dir.join(rel), bytes, fill);
    }
    // I link dopo i file: la voce di cartella del nuovo nome ha già le dimensioni finali.
    for &(new, existing) in spec.links {
        let n = dir.join(new);
        if let Some(p) = n.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        fs::hard_link(dir.join(existing), &n).map_err(|e| format!("{}: {e}", n.display()))?;
    }
    let o = oracle::walk(dir);
    if !o.unreadable.is_empty() {
        return Err(format!("l'oracolo non legge la preparazione: {:?}", o.unreadable));
    }
    let ids: HashSet<u128> = o.files.values().map(|f| f.id).collect();
    let mut files: Vec<ExpectedFile> = o
        .files
        .iter()
        .map(|(p, f)| ExpectedFile { rel: rel_string(&oracle::path_from_utf16(p), dir), logical: f.logical, alloc: f.alloc })
        .collect();
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(ExpectedFolder {
        path: final_path.to_string_lossy().into_owned(),
        totals: Measured { logical: o.unique_logical, alloc: o.unique_alloc, files: ids.len() as u64, dirs: o.dirs - 1 },
        files,
    })
}

fn build(vol: &Path) -> Result<Expected, String> {
    let user = current_user_sid().map_err(|e| format!("SID dell'utente: {e}"))?;
    let prep = vol.join(r"m6-helper\preparazione");
    let places = layout(vol, &user);
    if places.len() < 4 {
        return Err(format!("{} non ha il Cestino: spostare una volta un file nel Cestino del volume", vol.display()));
    }
    let mut measured: Vec<ExpectedFolder> = Vec::new();
    for Place { key, path: final_path, spec, sddl } in places {
        let staging = prep.join(key);
        let e = build_and_measure(&staging, spec, &final_path)?;
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        create_dir_with(&final_path, &sddl)?;
        for child in spec.top_children() {
            fs::rename(staging.join(child), final_path.join(child)).map_err(|e| format!("{} in {}: {e}", child, final_path.display()))?;
        }
        fs::remove_dir(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
        // Chiusa davvero: l'utente non la elenca.
        match fs::read_dir(&final_path) {
            Err(e) if e.raw_os_error() == Some(5) => {}
            other => return Err(format!("{} non è negata all'utente: {:?}", final_path.display(), other.map(|_| ()))),
        }
        measured.push(e);
    }
    fs::remove_dir(&prep).map_err(|e| format!("{}: {e}", prep.display()))?;
    let mut it = measured.into_iter();
    let (denied, profile, system, orphan) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
    Ok(Expected {
        volume: vol.to_string_lossy().into_owned(),
        denied,
        profile,
        bins: vec![
            ExpectedBin { sid: SYSTEM_SID, orphan: false, folder: system },
            ExpectedBin { sid: ORPHAN_BIN_SID, orphan: true, folder: orphan },
        ],
    })
}

/// Crea il contenuto di prova su `R:` e restituisce i valori attesi. Toglie prima un contenuto rimasto
/// (idempotente); se un passo fallisce, ripulisce e si ferma.
pub fn prepare() -> Expected {
    let vol = volume();
    if let Err(e) = cleanup() {
        panic!("contenuto di prova precedente non rimosso: {e}");
    }
    match build(&vol) {
        Ok(e) => e,
        Err(m) => {
            let c = cleanup();
            panic!("contenuto di prova dell'helper non creato: {m} (pulizia: {c:?})");
        }
    }
}

fn not_found(e: &std::io::Error) -> bool {
    e.kind() == ErrorKind::NotFound
}

/// Toglie il contenuto di prova da `R:` (idempotente). Le cartelle chiuse non si elencano: si cancellano
/// i figli noti dal layout, poi la cartella. `Err` con i percorsi rimasti, se ce ne sono.
pub fn cleanup() -> Result<(), String> {
    let vol = volume();
    let mut left: Vec<String> = Vec::new();
    // Il SID serve solo a scrivere i descrittori: per cancellare basta il layout.
    let user = current_user_sid().unwrap_or_default();
    for Place { path: top, spec, .. } in &layout(&vol, &user) {
        if fs::symlink_metadata(top).is_err() {
            continue;
        }
        for child in spec.top_children() {
            let p = top.join(child);
            let r = match fs::symlink_metadata(&p) {
                Ok(m) if m.is_dir() => fs::remove_dir_all(&p),
                Ok(_) => fs::remove_file(&p),
                Err(e) => Err(e),
            };
            if let Err(e) = r
                && !not_found(&e)
            {
                left.push(format!("{}: {e}", p.display()));
            }
        }
        if let Err(e) = fs::remove_dir(top)
            && !not_found(&e)
        {
            left.push(format!("{}: {e}", top.display()));
        }
    }
    let helper_dir = vol.join("m6-helper");
    if let Err(e) = fs::remove_dir_all(&helper_dir)
        && !not_found(&e)
    {
        left.push(format!("{}: {e}", helper_dir.display()));
    }
    // `Users` la crea `prepare`; si toglie solo se è vuota.
    let _ = fs::remove_dir(vol.join("Users"));
    if left.is_empty() { Ok(()) } else { Err(left.join("; ")) }
}
