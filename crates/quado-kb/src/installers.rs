// SPDX-License-Identifier: GPL-3.0-or-later
//! Installatori nella lente Download (03 F-DL-2, 05 D6): nome e versione dalla risorsa di versione PE o
//! dalla tabella Property degli MSI, letti su richiesta; corrispondenza con i programmi installati.
//! ProductCode uguale alla chiave di disinstallazione vale "da fonte"; nome e versione di un exe
//! valgono "dedotto", con l'affidabilità di 03 §6.2.
//!
//! Qui sta la parte pura: tipo dall'estensione, controllo della struttura di una risorsa di versione,
//! corrispondenza, fatti per il record e cache delle letture. Le letture su Windows sono in
//! `installers_win`. Nessuna somiglianza parziale: un nome o è uguale dopo la normalizzazione
//! descritta in `normalize_name`, o non corrisponde.
//!
//! I tipi marcati "interfaccia M4" sono condivisi tra i crate: non si cambiano senza accordo.

use std::collections::{HashMap, HashSet};

use quado_model::record::{Criterion, Fact, Hive, Provenance, Source, Value};

use crate::extract::{self, ExtractError, FormatIssue, Uninstall, UninstallEntry};

/// Interfaccia M4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InstallerKind {
    /// Eseguibile PE (`.exe`), risorsa di versione.
    Exe,
    /// Pacchetto Windows Installer (`.msi`), tabella Property.
    Msi,
}

impl InstallerKind {
    /// Codice per i file di lingua.
    pub fn code(self) -> &'static str {
        match self {
            Self::Exe => "exe",
            Self::Msi => "msi",
        }
    }
}

/// Dati di un installatore letti su richiesta. Interfaccia M4. I testi sono dati dell'installatore,
/// mai frasi di Quado.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct InstallerInfo {
    pub kind: Option<InstallerKind>,
    /// `ProductName` (risorsa di versione o tabella Property).
    pub product_name: Option<String>,
    /// `ProductVersion`.
    pub product_version: Option<String>,
    /// `CompanyName` o `Manufacturer`.
    pub company: Option<String>,
    /// Solo MSI: `ProductCode`, GUID con le graffe come nella tabella.
    pub product_code: Option<String>,
    /// Solo MSI: `UpgradeCode`.
    pub upgrade_code: Option<String>,
    /// Solo exe: `FileVersion`.
    pub file_version: Option<String>,
}

/// Corrispondenza con i programmi installati (F-DL-2). Interfaccia M4. `entry` è l'indice in
/// `Uninstall::entries`; la provenienza è `FromSource` per il ProductCode, `Deduced` per nome e versione.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MatchOutcome {
    SameVersion { entry: usize, provenance: Provenance },
    OtherVersion { entry: usize, provenance: Provenance },
    /// Nessun programma installato corrispondente.
    NotInstalled,
}

/// Tipo di installatore dall'estensione del nome (`.exe`, `.msi`, senza distinzione di maiuscole).
/// Accetta anche un percorso: conta solo l'ultimo componente.
pub fn installer_kind(name: &str) -> Option<InstallerKind> {
    let file = name.rsplit(['\\', '/']).next().unwrap_or(name);
    let (_, ext) = file.rsplit_once('.')?;
    if ext.eq_ignore_ascii_case("exe") {
        Some(InstallerKind::Exe)
    } else if ext.eq_ignore_ascii_case("msi") {
        Some(InstallerKind::Msi)
    } else {
        None
    }
}

/// Lunghezza massima, in caratteri UTF-16, di un valore letto (nome, versione, editore, codici). Oltre
/// il limite la lettura dà `FormatIssue::TooLarge`: il valore non si tronca, sarebbe un dato ricostruito.
pub const MAX_TEXT: usize = 1024;

/// Valore presente e non vuoto, senza spazi ai lati.
fn field(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------------------------
// Struttura della risorsa di versione

/// Firma di `VS_FIXEDFILEINFO`.
pub const FIXED_SIGNATURE: u32 = 0xFEEF_04BD;
/// Dimensione di `VS_FIXEDFILEINFO`.
pub const FIXED_LEN: usize = 52;

/// Blocco di una risorsa di versione a 32 bit (`VS_VERSIONINFO`, `StringFileInfo`, `StringTable`,
/// `String`, `VarFileInfo`, `Var`): intestazione di tre `WORD`, chiave UTF-16 terminata da zero, valore e
/// figli allineati a 4 byte dall'inizio della risorsa.
struct Block {
    key: Vec<u16>,
    end: usize,
    value: std::ops::Range<usize>,
    children: usize,
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let b = d.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([b[0], b[1]]))
}

/// `u32` little endian a una posizione qualunque (anche non allineata).
pub fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let b = d.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn align4(x: usize) -> usize {
    x.saturating_add(3) & !3
}

/// Un blocco che comincia a `start` e deve finire entro `limit`.
fn read_block(d: &[u8], start: usize, limit: usize) -> Result<Block, FormatIssue> {
    let len = usize::from(u16_at(d, start).ok_or(FormatIssue::Truncated)?);
    let value_len = usize::from(u16_at(d, start + 2).ok_or(FormatIssue::Truncated)?);
    let value_type = u16_at(d, start + 4).ok_or(FormatIssue::Truncated)?;
    if len < 6 {
        return Err(FormatIssue::Unexpected);
    }
    let end = start + len;
    if end > limit || end > d.len() {
        return Err(FormatIssue::Truncated);
    }
    let mut key = Vec::new();
    let mut p = start + 6;
    loop {
        if p + 2 > end {
            return Err(FormatIssue::Truncated);
        }
        let c = u16_at(d, p).ok_or(FormatIssue::Truncated)?;
        p += 2;
        if c == 0 {
            break;
        }
        key.push(c);
    }
    let vstart = align4(p).min(end);
    // I valori di testo hanno la lunghezza in caratteri; alcuni compilatori di risorse la scrivono in
    // byte: si accetta anche quella, purché il valore resti dentro il blocco.
    let bytes = if value_type == 1 { value_len * 2 } else { value_len };
    let vend = if vstart + bytes <= end {
        vstart + bytes
    } else if value_type == 1 && vstart + value_len <= end {
        vstart + value_len
    } else {
        return Err(FormatIssue::Truncated);
    };
    Ok(Block { key, end, value: vstart..vend, children: align4(vend) })
}

/// Figli di un blocco. Come `VerQueryValueW`, la scansione dei figli si ferma alla fine del padre o a
/// un figlio di lunghezza zero; un figlio deve comunque finire dentro la risorsa (`root_end`), così
/// nessuna lettura esce dal buffer.
fn children(d: &[u8], b: &Block, root_end: usize) -> Result<Vec<Block>, FormatIssue> {
    let mut out = Vec::new();
    let mut p = b.children;
    while p + 6 <= b.end {
        if u16_at(d, p) == Some(0) {
            break;
        }
        let c = read_block(d, p, root_end)?;
        p = align4(c.end);
        out.push(c);
        if out.len() > 4096 {
            return Err(FormatIssue::TooLarge);
        }
    }
    Ok(out)
}

fn key_is(b: &Block, s: &str) -> bool {
    b.key.iter().copied().eq(s.encode_utf16())
}

/// Controlla la struttura di una risorsa di versione a 32 bit come la restituisce
/// `GetFileVersionInfoExW` e restituisce i nomi delle tabelle di stringhe (`040904B0`), come sono scritti.
/// Ogni blocco, chiave e valore deve stare dentro la lunghezza della radice, e la radice dentro `data`:
/// è l'invariante che rende sicura la successiva lettura con `VerQueryValueW`. Errori: `Truncated` per
/// lunghezze che escono dai limiti, `Unexpected` per una radice diversa da `VS_VERSION_INFO` o un
/// `VS_FIXEDFILEINFO` senza la sua firma.
pub fn version_resource_tables(data: &[u8]) -> Result<Vec<String>, FormatIssue> {
    if data.len() < 6 {
        return Err(FormatIssue::Truncated);
    }
    let root = read_block(data, 0, data.len())?;
    if !key_is(&root, "VS_VERSION_INFO") {
        return Err(FormatIssue::Unexpected);
    }
    match root.value.len() {
        0 => {}
        FIXED_LEN => {
            if u32_at(data, root.value.start) != Some(FIXED_SIGNATURE) {
                return Err(FormatIssue::Unexpected);
            }
        }
        _ => return Err(FormatIssue::Unexpected),
    }
    let root_end = root.end;
    let mut tables = Vec::new();
    for info in children(data, &root, root_end)? {
        let string_info = key_is(&info, "StringFileInfo");
        for table in children(data, &info, root_end)? {
            // Le stringhe e le variabili si controllano tutte, anche quelle che poi non si leggono.
            children(data, &table, root_end)?;
            if string_info && table.key.len() == 8 {
                let name = String::from_utf16(&table.key).map_err(|_| FormatIssue::Encoding)?;
                if name.bytes().all(|b| b.is_ascii_hexdigit()) {
                    tables.push(name);
                }
            }
        }
    }
    Ok(tables)
}

// ---------------------------------------------------------------------------------------------
// Intestazioni PE

/// Controllo delle intestazioni di un eseguibile prima di passarlo a `GetFileVersionInfoExW`, che su un
/// file qualunque risponde "risorsa non trovata" invece di "non è un eseguibile". Si leggono solo le
/// intestazioni (meno di 4 KiB in un file normale):
/// - firma `MZ` e firma `PE\0\0` all'offset `e_lfanew`, altrimenti `Format(Unexpected)`;
/// - intestazioni, sezioni e tabella dei certificati (Authenticode, che negli installatori firmati sta in
///   fondo al file, dopo i dati aggiunti) dentro il file, altrimenti `Format(Truncated)`.
///
/// Un installatore senza firma troncato nei dati aggiunti dopo le sezioni non si riconosce: le
/// intestazioni non ne dichiarano la lunghezza.
pub fn pe_check<R: std::io::Read + std::io::Seek>(r: &mut R, size: u64) -> Result<(), ExtractError> {
    if size == 0 {
        return Err(FormatIssue::Empty.into());
    }
    let mut dos = [0u8; 64];
    let n = size.min(64) as usize;
    read_at(r, 0, &mut dos[..n])?;
    if dos[0] != b'M' || (n > 1 && dos[1] != b'Z') {
        return Err(FormatIssue::Unexpected.into());
    }
    if n < 64 {
        return Err(FormatIssue::Truncated.into());
    }
    let lfanew = u64::from(u32_at(&dos, 0x3C).unwrap_or(0));
    if !(64..=1 << 20).contains(&lfanew) {
        return Err(FormatIssue::Unexpected.into());
    }
    let mut coff = [0u8; 24];
    if lfanew + 24 > size {
        return Err(FormatIssue::Truncated.into());
    }
    read_at(r, lfanew, &mut coff)?;
    if coff[..4] != *b"PE\0\0" {
        return Err(FormatIssue::Unexpected.into());
    }
    let sections = u64::from(u16::from_le_bytes([coff[6], coff[7]]));
    let optional_len = u64::from(u16::from_le_bytes([coff[20], coff[21]]));
    if sections > 96 {
        return Err(FormatIssue::Unexpected.into());
    }
    let headers_end = lfanew + 24 + optional_len + 40 * sections;
    if headers_end > size {
        return Err(FormatIssue::Truncated.into());
    }
    let mut rest = vec![0u8; (optional_len + 40 * sections) as usize];
    read_at(r, lfanew + 24, &mut rest)?;
    let word = |at: usize| rest.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    // Directory dei dati: da 96 (PE32) o 112 (PE32+) nell'intestazione facoltativa; la quinta è la
    // tabella dei certificati, con un offset nel file invece di un RVA.
    let dirs = match word(0) {
        Some(0x10B) => Some(96),
        Some(0x20B) => Some(112),
        _ => None,
    };
    let mut ranges: Vec<(u64, u64)> = Vec::new();
    if let Some(d) = dirs {
        let count = u32_at(&rest, d - 4).unwrap_or(0);
        let at = d + 4 * 8;
        if count > 4 && (at + 8) as u64 <= optional_len {
            ranges.push((u64::from(u32_at(&rest, at).unwrap_or(0)), u64::from(u32_at(&rest, at + 4).unwrap_or(0))));
        }
    }
    for s in 0..sections as usize {
        let h = optional_len as usize + 40 * s;
        ranges.push((u64::from(u32_at(&rest, h + 20).unwrap_or(0)), u64::from(u32_at(&rest, h + 16).unwrap_or(0))));
    }
    if ranges.iter().any(|&(start, len)| len > 0 && start + len > size) {
        return Err(FormatIssue::Truncated.into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// File composto (contenitore degli MSI)

/// Firma di un file composto OLE (MS-CFB), il contenitore dei pacchetti MSI.
pub const CFB_SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
/// Voce libera della FAT.
const FREESECT: u32 = 0xFFFF_FFFF;
/// Ultimo numero di settore normale; i valori sopra sono marcatori (fine catena, settore della FAT...).
const MAXREGSECT: u32 = 0xFFFF_FFFA;

fn read_at<R: std::io::Read + std::io::Seek>(r: &mut R, at: u64, buf: &mut [u8]) -> Result<(), ExtractError> {
    let io = |e: std::io::Error| match e.kind() {
        std::io::ErrorKind::UnexpectedEof => ExtractError::Format(FormatIssue::Truncated),
        _ => ExtractError::Io(e.raw_os_error().unwrap_or(0) as u32),
    };
    r.seek(std::io::SeekFrom::Start(at)).map_err(io)?;
    r.read_exact(buf).map_err(io)
}

/// Controllo di un file composto prima di passarlo a Windows Installer: firma, intestazione coerente
/// (versione 3 con settori da 512 byte o 4 con settori da 4096) e nessun settore in uso oltre la fine del
/// file. Così un MSI troncato dà `Format(Truncated)` anche quando la parte rimasta contiene ancora la
/// tabella Property. Si leggono l'intestazione, gli eventuali settori DIFAT e solo i settori della FAT che
/// descrivono la fine del file (di solito uno). `size` è la dimensione del file.
pub fn compound_file_check<R: std::io::Read + std::io::Seek>(r: &mut R, size: u64) -> Result<(), ExtractError> {
    if size == 0 {
        return Err(FormatIssue::Empty.into());
    }
    let mut head = [0u8; 512];
    let n = size.min(512) as usize;
    read_at(r, 0, &mut head[..n])?;
    let k = n.min(CFB_SIGNATURE.len());
    if head[..k] != CFB_SIGNATURE[..k] {
        return Err(FormatIssue::Unexpected.into());
    }
    if n < 512 {
        return Err(FormatIssue::Truncated.into());
    }
    let word = |at: usize| u16::from_le_bytes([head[at], head[at + 1]]);
    let dword = |at: usize| u32_at(&head, at).unwrap_or(0);
    let ss: u64 = match (word(0x1A), word(0x1E)) {
        (3, 9) => 512,
        (4, 12) => 4096,
        _ => return Err(FormatIssue::Unexpected.into()),
    };
    if word(0x1C) != 0xFFFE {
        return Err(FormatIssue::Unexpected.into());
    }
    if size < 2 * ss {
        return Err(FormatIssue::Truncated.into());
    }
    // Settori interi presenti dopo l'intestazione (il settore k sta a (k + 1) * ss).
    let avail = size / ss - 1;
    let fat_sectors = u64::from(dword(0x2C));
    let (first_difat, difat_sectors) = (dword(0x44), u64::from(dword(0x48)));
    if fat_sectors == 0 {
        return Err(FormatIssue::Unexpected.into());
    }
    let per = ss / 4;
    let mut sector = vec![0u8; ss as usize];
    // Settore che contiene la k-esima parte della FAT: le prime 109 nell'intestazione, le altre nella
    // catena DIFAT (ogni settore DIFAT ne elenca per - 1, l'ultima voce porta al settore seguente).
    let mut fat_location = |r: &mut R, k: u64| -> Result<u64, ExtractError> {
        if k < 109 {
            return Ok(u64::from(dword(0x4C + 4 * k as usize)));
        }
        let idx = k - 109;
        let hops = idx / (per - 1);
        if hops >= difat_sectors {
            return Err(FormatIssue::Unexpected.into());
        }
        let mut d = u64::from(first_difat);
        for hop in 0..=hops {
            if d >= avail {
                return Err(FormatIssue::Truncated.into());
            }
            read_at(r, (d + 1) * ss, &mut sector)?;
            if hop < hops {
                d = u64::from(u32_at(&sector, (ss - 4) as usize).unwrap_or(FREESECT));
            }
        }
        Ok(u64::from(u32_at(&sector, (4 * (idx % (per - 1))) as usize).unwrap_or(FREESECT)))
    };
    // Solo le parti della FAT che descrivono settori dalla fine del file in poi: in un file completo una o
    // due. Molte di più vogliono dire una FAT per un file più lungo di questo.
    let first = avail / per;
    if fat_sectors > first + 64 {
        return Err(FormatIssue::Truncated.into());
    }
    for k in first..fat_sectors {
        let s = fat_location(r, k)?;
        if s >= avail {
            return Err(if s > u64::from(MAXREGSECT) { FormatIssue::Unexpected } else { FormatIssue::Truncated }.into());
        }
        let mut fat = vec![0u8; ss as usize];
        read_at(r, (s + 1) * ss, &mut fat)?;
        for j in 0..per {
            let v = u32_at(&fat, (4 * j) as usize).unwrap_or(FREESECT);
            let index = k * per + j;
            if (v != FREESECT && index >= avail) || (v <= MAXREGSECT && u64::from(v) >= avail) {
                return Err(FormatIssue::Truncated.into());
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Versioni e nomi

/// Versione come testo confrontabile: spazi ai lati tolti; la forma vecchia con le virgole
/// (`1, 2, 3, 4`) diventa puntata.
fn version_text(v: &str) -> String {
    let t = v.trim();
    let parts: Vec<&str> = t.split(',').map(str::trim).collect();
    if parts.len() > 1 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())) {
        parts.join(".")
    } else {
        t.to_owned()
    }
}

/// Uguaglianza numerica con gli zeri finali sottintesi: `1.2` = `1.2.0` = `1.2.0.0`.
fn padded_eq(a: &[u32], b: &[u32]) -> bool {
    let n = a.len().max(b.len());
    (0..n).all(|k| a.get(k).copied().unwrap_or(0) == b.get(k).copied().unwrap_or(0))
}

/// Stessa versione. Se entrambe sono versioni puntate (`extract::parse_version`, da 2 a 4 numeri) il
/// confronto è numerico, con gli zeri finali sottintesi: `1.2` = `1.2.0` (le risorse di versione hanno
/// quattro numeri, `ProductVersion` degli MSI tre, `DisplayVersion` quanti ne scrive il programma) e
/// `26.03` = `26.3`. Altrimenti il confronto è testuale, senza distinzione di maiuscole: `1.2 beta` e
/// `1.2` sono versioni diverse.
pub fn same_version(a: &str, b: &str) -> bool {
    let (a, b) = (version_text(a), version_text(b));
    match (extract::parse_version(&a), extract::parse_version(&b)) {
        (Some(x), Some(y)) => padded_eq(&x, &y),
        _ => a.eq_ignore_ascii_case(&b),
    }
}

/// Suffissi di architettura tolti dalla fine di un nome (già in minuscolo). Le forme senza parentesi
/// valgono solo come parola intera, dopo uno spazio.
const ARCH_SUFFIXES: [&str; 32] = [
    "(x64 edition)",
    "(x86 edition)",
    "(64-bit)",
    "(32-bit)",
    "(64 bit)",
    "(32 bit)",
    "(64bit)",
    "(32bit)",
    "(x64)",
    "(x86)",
    "(x86_64)",
    "(amd64)",
    "(arm64)",
    "(win64)",
    "(win32)",
    "(64-bit edition)",
    "x64 edition",
    "x86 edition",
    "64-bit edition",
    "64-bit",
    "32-bit",
    "64 bit",
    "32 bit",
    "64bit",
    "32bit",
    "x64",
    "x86",
    "x86_64",
    "amd64",
    "arm64",
    "win64",
    "win32",
];

/// Separatori rimasti in fondo dopo aver tolto un suffisso ("Nome - x64" diventa "Nome").
fn trim_separators(s: &str) -> &str {
    s.trim_end_matches([' ', '-', ',', '–'])
}

fn strip_arch(s: &str) -> Option<&str> {
    for suf in ARCH_SUFFIXES {
        let Some(rest) = s.strip_suffix(suf) else { continue };
        let ok = if suf.starts_with('(') { true } else { rest.ends_with(' ') };
        let rest = trim_separators(rest);
        if ok && !rest.is_empty() {
            return Some(rest);
        }
    }
    None
}

/// Toglie l'ultima parola se è una versione puntata (con o senza `v` davanti) coerente con la versione
/// dichiarata dalla stessa fonte: uguale, oppure uguale nei primi numeri (`26.03` con `26.03.00.0`).
fn strip_version<'a>(s: &'a str, declared: Option<&[u32]>) -> Option<&'a str> {
    let declared = declared?;
    let (rest, last) = s.rsplit_once(' ')?;
    let token = last.strip_prefix('v').unwrap_or(last);
    let v = extract::parse_version(token)?;
    let coherent = padded_eq(&v, declared) || (v.len() <= declared.len() && v[..] == declared[..v.len()]);
    let rest = trim_separators(rest);
    (coherent && !rest.is_empty()).then_some(rest)
}

/// Nome normalizzato per il confronto tra il nome del prodotto di un installatore e il `DisplayName` di
/// una voce di disinstallazione. Regole, in quest'ordine:
/// 1. minuscole (Unicode) e spazi: quelli ai lati tolti, quelli ripetuti (anche non separabili) ridotti a uno;
/// 2. dalla fine, finché si applicano: un suffisso di architettura (`(x64)`, `x64`, `(64-bit)`, `64-bit`,
///    `(x64 edition)` e simili in `ARCH_SUFFIXES`), oppure l'ultima parola se è una versione puntata
///    coerente con la versione dichiarata dalla stessa fonte (`declared_version`: `ProductVersion` per
///    l'installatore, `DisplayVersion` per la voce). Restano i separatori `-` e `,` rimasti in fondo.
///
/// Niente altro: nessun suffisso di lingua, nessun marchio, nessuna somiglianza. `None` per un nome vuoto.
/// Esempio: "7-Zip 26.03 (x64 edition)" con versione 26.03.00.0 diventa "7-zip".
pub fn normalize_name(name: &str, declared_version: Option<&str>) -> Option<String> {
    let lower = name.to_lowercase();
    let mut s: String = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    let declared = declared_version.map(version_text).and_then(|v| extract::parse_version(&v));
    for _ in 0..8 {
        let next = strip_arch(&s).or_else(|| strip_version(&s, declared.as_deref()));
        match next {
            Some(n) => s = n.to_owned(),
            None => break,
        }
    }
    (!s.is_empty()).then_some(s)
}

/// `ProductCode` in forma di GUID con le graffe, come lo richiede Windows Installer: solo così vale
/// come nome di una chiave di disinstallazione.
pub fn is_product_code(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 38
        && b[0] == b'{'
        && b[37] == b'}'
        && b[1..37].iter().enumerate().all(|(k, c)| if matches!(k, 8 | 13 | 18 | 23) { *c == b'-' } else { c.is_ascii_hexdigit() })
}

fn hive_rank(h: Hive) -> u8 {
    match h {
        Hive::Machine => 0,
        Hive::Machine32 => 1,
        Hive::User => 2,
    }
}

/// Le due versioni sono note e uguali.
fn versions_match(installer: Option<&str>, entry: &UninstallEntry) -> bool {
    match (installer, entry.version.as_deref().map(str::trim).filter(|v| !v.is_empty())) {
        (Some(a), Some(b)) => same_version(a, b),
        _ => false,
    }
}

/// Tra più voci candidate: prima quella con la stessa versione, poi HKLM (64 bit, poi 32 bit) prima di
/// HKCU, poi le voci visibili prima dei componenti di sistema, infine l'ordine di lettura.
fn best<'a>(cands: impl Iterator<Item = (usize, &'a UninstallEntry)>, version: Option<&str>) -> Option<(usize, bool)> {
    cands
        .map(|(i, e)| (i, versions_match(version, e), e))
        .min_by_key(|(i, same, e)| (!*same, hive_rank(e.hive), e.system_component, *i))
        .map(|(i, same, _)| (i, same))
}

/// Corrispondenza con i programmi installati (F-DL-2).
/// - `ProductCode` valido (`is_product_code`) uguale al nome di una chiave di disinstallazione, senza
///   distinzione di maiuscole: provenienza `FromSource`.
/// - Altrimenti (exe, o MSI senza voce con il suo codice): nome del prodotto uguale al `DisplayName` dopo
///   `normalize_name`. Con la stessa versione la provenienza è `Deduced(OneSource)`, affidabilità media
///   (03 §6.2: nome e versione uguali); con una versione diversa o non confrontabile è
///   `Deduced(NameOnly)`, affidabilità bassa, perché coincide solo il nome.
/// - Versioni con `same_version`. Una versione mancante da una delle due parti dà `OtherVersion`: non si
///   può dire che sia la stessa (il fatto lo dice con la sua chiave, vedi `match_fact`).
/// - Più voci candidate: vedi `best`.
pub fn match_installer(info: &InstallerInfo, programs: &Uninstall) -> MatchOutcome {
    let version = field(&info.product_version);
    if let Some(code) = field(&info.product_code).filter(|c| is_product_code(c)) {
        let cands = programs.entries.iter().enumerate().filter(|(_, e)| e.key.trim().eq_ignore_ascii_case(code));
        if let Some((entry, same)) = best(cands, version) {
            let provenance = Provenance::FromSource;
            return if same { MatchOutcome::SameVersion { entry, provenance } } else { MatchOutcome::OtherVersion { entry, provenance } };
        }
    }
    let Some(name) = field(&info.product_name).and_then(|n| normalize_name(n, version)) else {
        return MatchOutcome::NotInstalled;
    };
    let same_name = |e: &UninstallEntry| normalize_name(&e.display_name, e.version.as_deref()).as_deref() == Some(name.as_str());
    match best(programs.entries.iter().enumerate().filter(|(_, e)| same_name(e)), version) {
        Some((entry, true)) => MatchOutcome::SameVersion { entry, provenance: Provenance::Deduced(Criterion::OneSource) },
        Some((entry, false)) => MatchOutcome::OtherVersion { entry, provenance: Provenance::Deduced(Criterion::NameOnly) },
        None => MatchOutcome::NotInstalled,
    }
}

// ---------------------------------------------------------------------------------------------
// Fatti per il record (03 §6.1): chiavi dei testi in `regole/<lingua>.ftl`, sezione "M4: installatori"

/// Nome e versione del prodotto (`$a`, `$b`).
pub const FACT_PRODUCT: &str = "fatto-installatore-prodotto";
/// Nome del prodotto senza versione (`$a`).
pub const FACT_PRODUCT_NO_VERSION: &str = "fatto-installatore-prodotto-senza-versione";
/// Versione senza nome del prodotto (`$a`).
pub const FACT_VERSION_NO_NAME: &str = "fatto-installatore-versione-senza-nome";
/// Editore (`$a`).
pub const FACT_COMPANY: &str = "fatto-installatore-editore";
/// Codice di prodotto MSI (`$a`).
pub const FACT_PRODUCT_CODE: &str = "fatto-installatore-codice-prodotto";
/// Versione del file, se diversa da quella del prodotto (`$a`).
pub const FACT_FILE_VERSION: &str = "fatto-installatore-versione-file";
/// Nessun dato nel file (`$a`: `exe` o `msi`).
pub const FACT_NO_DATA: &str = "fatto-installatore-senza-dati";
/// Stesso codice di prodotto e stessa versione (`$a` programma, `$b` versione).
pub const FACT_SAME_CODE: &str = "fatto-installatore-stesso-codice";
/// Stesso nome e stessa versione (`$a` programma, `$b` versione).
pub const FACT_SAME_NAME: &str = "fatto-installatore-stesso-nome";
/// Stesso codice, versione diversa (`$a` programma, `$b` versione installata, `$c` dell'installatore).
pub const FACT_CODE_OTHER_VERSION: &str = "fatto-installatore-codice-versione-diversa";
/// Stesso nome, versione diversa (`$a`, `$b`, `$c` come sopra).
pub const FACT_NAME_OTHER_VERSION: &str = "fatto-installatore-nome-versione-diversa";
/// Stesso codice, versione non indicata da una delle due parti (`$a` programma).
pub const FACT_CODE_VERSION_UNKNOWN: &str = "fatto-installatore-codice-versione-non-nota";
/// Stesso nome, versione non indicata da una delle due parti (`$a` programma).
pub const FACT_NAME_VERSION_UNKNOWN: &str = "fatto-installatore-nome-versione-non-nota";
/// Nessun programma corrispondente (`$a`: `codice` se si è confrontato anche il codice, altrimenti `nome`).
pub const FACT_NOT_INSTALLED: &str = "fatto-installatore-non-installato";
/// Confronto non possibile (`$a`: `senza-dati` se mancano nome e codice, altrimenti `voce-mancante`).
pub const FACT_NOT_COMPARABLE: &str = "fatto-installatore-non-confrontabile";

/// Tutte le chiavi dei fatti prodotti da `info_facts` e `match_fact`.
pub const FACT_KEYS: [&str; 15] = [
    FACT_PRODUCT,
    FACT_PRODUCT_NO_VERSION,
    FACT_VERSION_NO_NAME,
    FACT_COMPANY,
    FACT_PRODUCT_CODE,
    FACT_FILE_VERSION,
    FACT_NO_DATA,
    FACT_SAME_CODE,
    FACT_SAME_NAME,
    FACT_CODE_OTHER_VERSION,
    FACT_NAME_OTHER_VERSION,
    FACT_CODE_VERSION_UNKNOWN,
    FACT_NAME_VERSION_UNKNOWN,
    FACT_NOT_INSTALLED,
    FACT_NOT_COMPARABLE,
];

/// Fonte dei dati dell'installatore: tabella Property per gli MSI, risorsa di versione per il resto.
pub fn installer_source(info: &InstallerInfo) -> Source {
    match info.kind {
        Some(InstallerKind::Msi) => Source::MsiProperties,
        Some(InstallerKind::Exe) | None => Source::VersionResource,
    }
}

fn text(s: &str) -> Value {
    Value::Text(s.to_owned())
}

/// Fatti letti dall'installatore, tutti "da fonte" con la fonte `installer_source`: nome e versione
/// del prodotto, editore, codice di prodotto (MSI), versione del file se diversa da quella del prodotto
/// (exe). Senza nessuno di questi dati: un solo fatto `fatto-installatore-senza-dati`, perché un file
/// senza risorsa di versione è un esito normale, non un errore. Gli errori di lettura non sono fatti:
/// l'interfaccia li scrive dall'`ExtractError`.
pub fn info_facts(info: &InstallerInfo) -> Vec<Fact> {
    let source = installer_source(info);
    let fact = |key: &str, values: Vec<Value>| Fact::new(key, Provenance::FromSource, source.clone(), values);
    let version = field(&info.product_version);
    let mut out = Vec::new();
    match (field(&info.product_name), version) {
        (Some(n), Some(v)) => out.push(fact(FACT_PRODUCT, vec![text(n), text(v)])),
        (Some(n), None) => out.push(fact(FACT_PRODUCT_NO_VERSION, vec![text(n)])),
        (None, Some(v)) => out.push(fact(FACT_VERSION_NO_NAME, vec![text(v)])),
        (None, None) => {}
    }
    if let Some(c) = field(&info.company) {
        out.push(fact(FACT_COMPANY, vec![text(c)]));
    }
    if let Some(c) = field(&info.product_code) {
        out.push(fact(FACT_PRODUCT_CODE, vec![text(c)]));
    }
    if let Some(fv) = field(&info.file_version)
        && version.is_none_or(|pv| !same_version(pv, fv))
    {
        out.push(fact(FACT_FILE_VERSION, vec![text(fv)]));
    }
    if out.is_empty() {
        out.push(fact(FACT_NO_DATA, vec![text(info.kind.unwrap_or(InstallerKind::Exe).code())]));
    }
    out
}

/// Fatto della corrispondenza, con provenienza e fonte:
/// - `SameVersion`/`OtherVersion`: provenienza dell'esito, fonte `Source::Uninstall(hive)` della voce;
///   valori: `DisplayName`, `DisplayVersion` e, per la versione diversa, `ProductVersion` dell'installatore.
/// - `NotInstalled` con un `ProductCode` valido: `Deduced(OneSource)`, il registro non ha né il codice né il
///   nome; senza codice: `Deduced(NameOnly)`, perché un programma può essere registrato con un altro nome.
///   La fonte è quella dell'installatore: non c'è una voce da citare.
/// - Senza nome né codice, o con un indice di voce che non esiste: `fatto-installatore-non-confrontabile`,
///   provenienza `Unavailable`.
pub fn match_fact(outcome: &MatchOutcome, info: &InstallerInfo, programs: &Uninstall) -> Fact {
    let source = installer_source(info);
    let code = field(&info.product_code).filter(|c| is_product_code(c));
    let not_comparable = |why: &str| Fact::new(FACT_NOT_COMPARABLE, Provenance::Unavailable, source.clone(), vec![text(why)]);
    let (entry, provenance, same) = match *outcome {
        MatchOutcome::NotInstalled => {
            if field(&info.product_name).is_none() && code.is_none() {
                return not_comparable("senza-dati");
            }
            return match code {
                Some(_) => Fact::new(FACT_NOT_INSTALLED, Provenance::Deduced(Criterion::OneSource), source, vec![text("codice")]),
                None => Fact::new(FACT_NOT_INSTALLED, Provenance::Deduced(Criterion::NameOnly), source, vec![text("nome")]),
            };
        }
        MatchOutcome::SameVersion { entry, provenance } => (entry, provenance, true),
        MatchOutcome::OtherVersion { entry, provenance } => (entry, provenance, false),
    };
    let Some(e) = programs.entries.get(entry) else { return not_comparable("voce-mancante") };
    let by_code = provenance == Provenance::FromSource;
    let installed = e.version.as_deref().map(str::trim).filter(|v| !v.is_empty());
    let installer = field(&info.product_version);
    let pick = |code_key: &'static str, name_key: &'static str| if by_code { code_key } else { name_key };
    let program = text(&e.display_name);
    let (key, values) = match (same, installed, installer) {
        (true, Some(v), _) => (pick(FACT_SAME_CODE, FACT_SAME_NAME), vec![program, text(v)]),
        (false, Some(v), Some(w)) => (pick(FACT_CODE_OTHER_VERSION, FACT_NAME_OTHER_VERSION), vec![program, text(v), text(w)]),
        _ => (pick(FACT_CODE_VERSION_UNKNOWN, FACT_NAME_VERSION_UNKNOWN), vec![program]),
    };
    Fact::new(key, provenance, Source::Uninstall(e.hive), values)
}

// ---------------------------------------------------------------------------------------------
// Cache delle letture (05 D6, D8): in memoria, per la sessione

/// Chiave di contenuto di un file (05 D6): percorso, dimensione, data di modifica (`FILETIME`), ID file.
/// Se uno dei tre valori cambia, il file si rilegge.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentKey {
    pub path: String,
    pub size: u64,
    pub modified: i64,
    pub file_id: u128,
}

impl ContentKey {
    /// Percorso normalizzato (`extract::normalize`) e in minuscolo: NTFS non distingue le maiuscole.
    fn path_key(&self) -> String {
        extract::normalize(&self.path).to_lowercase()
    }

    fn same_content(&self, other: &Self) -> bool {
        self.size == other.size && self.modified == other.modified && self.file_id == other.file_id
    }
}

/// Esito che dipende solo dal contenuto del file: non si rilegge finché la chiave non cambia. Gli altri
/// (`Io`, `NotPresent`, `CloudPlaceholder`) dipendono dallo stato del momento (file in uso o aperto da un
/// altro processo, segnaposto scaricato nel frattempo) e alla richiesta successiva si riprovano.
fn settled(r: &Result<InstallerInfo, ExtractError>) -> bool {
    matches!(r, Ok(_) | Err(ExtractError::Format(_)))
}

/// Cache dei dati degli installatori (F-DL-2): una voce per percorso, valida finché dimensione, data di
/// modifica e ID file restano uguali. Resta in memoria (05 D8).
#[derive(Clone, Debug, Default)]
pub struct InstallerCache {
    entries: HashMap<String, (ContentKey, Result<InstallerInfo, ExtractError>)>,
}

impl InstallerCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Esito salvato per questo file, solo se la chiave coincide (percorso senza distinzione di
    /// maiuscole, dimensione, data, ID).
    pub fn get(&self, key: &ContentKey) -> Option<&Result<InstallerInfo, ExtractError>> {
        let (k, r) = self.entries.get(&key.path_key())?;
        k.same_content(key).then_some(r)
    }

    /// Salva l'esito di una lettura; sostituisce quello di prima per lo stesso percorso.
    pub fn insert(&mut self, key: ContentKey, result: Result<InstallerInfo, ExtractError>) {
        self.entries.insert(key.path_key(), (key, result));
    }

    /// File da leggere: nuovi, con dimensione, data o ID cambiati, oppure con un esito che dipende dal
    /// momento (vedi `settled`). Ogni percorso una volta sola, nell'ordine della richiesta.
    pub fn to_read<'a>(&self, keys: &'a [ContentKey]) -> Vec<&'a ContentKey> {
        let mut seen = HashSet::new();
        keys.iter().filter(|k| seen.insert(k.path_key()) && !self.get(k).is_some_and(settled)).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_from_extension() {
        assert_eq!(installer_kind("Setup.EXE"), Some(InstallerKind::Exe));
        assert_eq!(installer_kind(r"C:\Users\u\Downloads\7z2603-x64.msi"), Some(InstallerKind::Msi));
        assert_eq!(installer_kind("archivio.zip"), None);
        assert_eq!(installer_kind("senza estensione"), None);
        assert_eq!(installer_kind(r"C:\cartella.exe\file"), None);
        assert_eq!(installer_kind("setup.exe.crdownload"), None);
    }

    #[test]
    fn versions() {
        assert!(same_version("1.2", "1.2.0"));
        assert!(same_version("1.2", "1.2.0.0"));
        assert!(same_version("26.03", "26.03.00.0"));
        assert!(same_version("26.03", "26.3"));
        assert!(same_version("1, 2, 3, 4", "1.2.3.4"));
        assert!(!same_version("1.2.3", "1.2.3.4"));
        assert!(!same_version("1.2 beta", "1.2"));
        assert!(same_version("Build A", "build a"));
        assert!(!same_version("1", "1.0"), "un numero solo non è una versione puntata: confronto testuale");
    }

    #[test]
    fn names() {
        assert_eq!(normalize_name("7-Zip 26.03 (x64 edition)", Some("26.03.00.0")).as_deref(), Some("7-zip"));
        assert_eq!(normalize_name("  Mozilla\u{a0}  Firefox  ", None).as_deref(), Some("mozilla firefox"));
        assert_eq!(normalize_name("Foo (x64)", None).as_deref(), Some("foo"));
        assert_eq!(normalize_name("Foo x64", None).as_deref(), Some("foo"));
        assert_eq!(normalize_name("Foo - 64-bit", None).as_deref(), Some("foo"));
        assert_eq!(normalize_name("Foox64", None).as_deref(), Some("foox64"), "solo parole intere");
        assert_eq!(normalize_name("x64", None).as_deref(), Some("x64"), "il nome non resta vuoto");
        // La versione nel nome si toglie solo se coerente con quella dichiarata.
        assert_eq!(normalize_name("Foo 2.1", Some("2.1.0.123")).as_deref(), Some("foo"));
        assert_eq!(normalize_name("Foo 2.1", Some("3.0")).as_deref(), Some("foo 2.1"));
        assert_eq!(normalize_name("Foo 2.1", None).as_deref(), Some("foo 2.1"));
        assert_eq!(normalize_name("Python 3", Some("3.0")).as_deref(), Some("python 3"), "un numero solo resta");
        assert_eq!(normalize_name("Foo v1.2 (x86)", Some("1.2")).as_deref(), Some("foo"));
        assert_eq!(normalize_name("   ", None), None);
    }

    #[test]
    fn product_codes() {
        assert!(is_product_code("{23170F69-40C1-2702-2603-000001000000}"));
        assert!(is_product_code("{23170f69-40c1-2702-2603-000001000000}"));
        assert!(!is_product_code("23170F69-40C1-2702-2603-000001000000"));
        assert!(!is_product_code("{B2FE1952-0186-46C3-BAEC-A80AA35AC5B8}_Display.Driver"));
        assert!(!is_product_code("Steam"));
        assert!(!is_product_code("{23170F69-40C1-2702-2603-00000100000G}"));
    }
}
