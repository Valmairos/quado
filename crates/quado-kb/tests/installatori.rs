// SPDX-License-Identifier: GPL-3.0-or-later
//! Installatori nella lente Download: risorsa di versione di eseguibili generati qui
//! (con risorsa, senza, con risorsa malformata), segnaposto cloud, corrispondenza con le voci di
//! disinstallazione della fixture e con voci costruite, fatti per il record, cache, testi.
//!
//! I file si creano in una cartella temporanea e si cancellano alla fine; nessun file dell'utente si
//! legge. Gli MSI si generano con le API di Windows Installer nei test di `installers_win`, perché la
//! creazione richiede `unsafe`.

#![cfg(windows)]

use std::collections::BTreeSet;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use proptest::prelude::*;
use quado_kb::extract::{self, ExtractError, FormatIssue, Uninstall, UninstallEntry};
use quado_kb::installers::{self, ContentKey, InstallerCache, InstallerInfo, InstallerKind, MatchOutcome};
use quado_kb::installers_win::{msi_info, version_info};
use quado_kb::reg::RegFile;
use quado_model::record::{Criterion, Fact, Hive, Provenance, Source, Value};

// ---------------------------------------------------------------------------------------------
// Cartella temporanea

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("quado-installatori-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("cartella temporanea");
        Self(p)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> String {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).expect("scrittura");
        p.to_string_lossy().into_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------------------------
// Generatore di eseguibili: PE32+ minimo con una risorsa di versione scritta qui, deterministico

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
}

fn pad4(v: &mut Vec<u8>) {
    while !v.len().is_multiple_of(4) {
        v.push(0);
    }
}

/// Blocco di `VS_VERSIONINFO`: lunghezza, lunghezza del valore (caratteri per i testi, byte per i dati,
/// come rc.exe), tipo, chiave, valore e figli allineati a 4 byte.
fn block(key: &str, text: bool, value: &[u8], value_len: u16, children: &[Vec<u8>]) -> Vec<u8> {
    let mut b = vec![0, 0];
    b.extend(value_len.to_le_bytes());
    b.extend(u16::from(text).to_le_bytes());
    b.extend(utf16z(key));
    pad4(&mut b);
    b.extend(value);
    for c in children {
        pad4(&mut b);
        b.extend(c);
    }
    let len = u16::try_from(b.len()).expect("blocco entro 64 KiB");
    b[0..2].copy_from_slice(&len.to_le_bytes());
    b
}

fn string(key: &str, value: &str) -> Vec<u8> {
    let v = utf16z(value);
    block(key, true, &v, (v.len() / 2) as u16, &[])
}

/// `VS_FIXEDFILEINFO` con versione del file e del prodotto.
fn fixed(file: [u16; 4], product: [u16; 4]) -> Vec<u8> {
    let ms = |v: [u16; 4]| (u32::from(v[0]) << 16) | u32::from(v[1]);
    let ls = |v: [u16; 4]| (u32::from(v[2]) << 16) | u32::from(v[3]);
    let words = [0xFEEF_04BD, 0x0001_0000, ms(file), ls(file), ms(product), ls(product), 0x3F, 0, 0x0004_0004, 1, 0, 0, 0];
    words.iter().flat_map(|w: &u32| w.to_le_bytes()).collect()
}

struct Version<'a> {
    fixed: Option<Vec<u8>>,
    table: &'a str,
    strings: Vec<(&'a str, &'a str)>,
    translation: Option<(u16, u16)>,
}

impl Version<'_> {
    fn standard() -> Self {
        Version {
            fixed: Some(fixed([2, 10, 91, 0], [2, 10, 91, 91])),
            table: "040904B0",
            strings: vec![
                ("CompanyName", "Editore di prova"),
                ("FileDescription", "Installatore di prova"),
                ("FileVersion", "2.10.91.0"),
                ("ProductName", "Programma di prova"),
                ("ProductVersion", "2.10.91.91"),
            ],
            translation: Some((0x0409, 0x04B0)),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let strings: Vec<Vec<u8>> = self.strings.iter().map(|(k, v)| string(k, v)).collect();
        let table = block(self.table, true, &[], 0, &strings);
        let mut children = vec![block("StringFileInfo", true, &[], 0, &[table])];
        if let Some((lang, cp)) = self.translation {
            let var = block("Translation", false, &[lang.to_le_bytes(), cp.to_le_bytes()].concat(), 4, &[]);
            children.push(block("VarFileInfo", true, &[], 0, &[var]));
        }
        let f = self.fixed.clone().unwrap_or_default();
        block("VS_VERSION_INFO", false, &f, f.len() as u16, &children)
    }
}

const RT_RCDATA: u32 = 10;
const RT_VERSION: u32 = 16;

fn le16(v: &mut Vec<u8>, x: u16) {
    v.extend(x.to_le_bytes());
}

fn le32(v: &mut Vec<u8>, x: u32) {
    v.extend(x.to_le_bytes());
}

fn le64(v: &mut Vec<u8>, x: u64) {
    v.extend(x.to_le_bytes());
}

fn align(x: usize, a: usize) -> usize {
    x.div_ceil(a) * a
}

/// Sezione delle risorse all'RVA `rva`: tre livelli di directory (tipo, nome 1, lingua 0x0409), una voce
/// di dati e i dati.
fn rsrc(ty: u32, data: &[u8], rva: u32) -> Vec<u8> {
    let mut s = Vec::new();
    let dir = |s: &mut Vec<u8>, id: u32, target: u32| {
        le32(s, 0);
        le32(s, 0);
        le16(s, 0);
        le16(s, 0);
        le16(s, 0);
        le16(s, 1);
        le32(s, id);
        le32(s, target);
    };
    dir(&mut s, ty, 0x8000_0000 | 24);
    dir(&mut s, 1, 0x8000_0000 | 48);
    dir(&mut s, 0x0409, 72);
    le32(&mut s, rva + 88);
    le32(&mut s, data.len() as u32);
    le32(&mut s, 0);
    le32(&mut s, 0);
    s.extend(data);
    s
}

/// PE32+ per x64: intestazione DOS, intestazioni PE, una sezione. Con `resource` la sezione è `.rsrc` con
/// la directory delle risorse; senza, `.rdata` di soli zeri e nessuna directory delle risorse. Nessun
/// codice e nessun punto d'ingresso: il file si legge come dati, non si esegue.
fn pe(resource: Option<(u32, &[u8])>) -> Vec<u8> {
    const RVA: u32 = 0x1000;
    let section = match resource {
        Some((ty, data)) => rsrc(ty, data, RVA),
        None => vec![0u8; 16],
    };
    let virt = section.len();
    let raw = align(virt, 0x200);
    let mut f = vec![0u8; 0x40];
    f[0..2].copy_from_slice(b"MZ");
    f[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    f.extend(b"PE\0\0");
    // Intestazione COFF: x64, una sezione, intestazione facoltativa PE32+ da 240 byte, eseguibile.
    le16(&mut f, 0x8664);
    le16(&mut f, 1);
    le32(&mut f, 0);
    le32(&mut f, 0);
    le32(&mut f, 0);
    le16(&mut f, 240);
    le16(&mut f, 0x0022);
    // Intestazione facoltativa PE32+.
    le16(&mut f, 0x20B);
    f.extend([14, 0]);
    le32(&mut f, 0);
    le32(&mut f, raw as u32);
    le32(&mut f, 0);
    le32(&mut f, 0);
    le32(&mut f, RVA);
    le64(&mut f, 0x1_4000_0000);
    le32(&mut f, 0x1000);
    le32(&mut f, 0x200);
    for v in [6, 0, 0, 0, 6, 0] {
        le16(&mut f, v);
    }
    le32(&mut f, 0);
    le32(&mut f, RVA + align(virt, 0x1000) as u32);
    le32(&mut f, 0x200);
    le32(&mut f, 0);
    le16(&mut f, 2);
    le16(&mut f, 0x8160);
    for v in [0x10_0000, 0x1000, 0x10_0000, 0x1000] {
        le64(&mut f, v);
    }
    le32(&mut f, 0);
    le32(&mut f, 16);
    for k in 0..16 {
        let (rva, size) = if k == 2 && resource.is_some() { (RVA, virt as u32) } else { (0, 0) };
        le32(&mut f, rva);
        le32(&mut f, size);
    }
    // Intestazione della sezione.
    f.extend(if resource.is_some() { *b".rsrc\0\0\0" } else { *b".rdata\0\0" });
    le32(&mut f, virt as u32);
    le32(&mut f, RVA);
    le32(&mut f, raw as u32);
    le32(&mut f, 0x200);
    le32(&mut f, 0);
    le32(&mut f, 0);
    le16(&mut f, 0);
    le16(&mut f, 0);
    le32(&mut f, 0x4000_0040);
    f.resize(0x200, 0);
    f.extend(&section);
    f.resize(0x200 + raw, 0);
    f
}

fn exe_with(v: &Version<'_>) -> Vec<u8> {
    pe(Some((RT_VERSION, &v.bytes())))
}

/// Posizione nella risorsa del blocco `String` con questa chiave.
fn string_block(res: &[u8], key: &str) -> usize {
    let k = utf16z(key);
    let at = res.windows(k.len()).position(|w| w == k.as_slice()).expect("chiave nella risorsa");
    at - 6
}

// ---------------------------------------------------------------------------------------------
// Risorsa di versione

fn modified(path: &str) -> u64 {
    std::fs::metadata(path).expect("metadati").last_write_time()
}

#[test]
fn version_resource_is_read() {
    let dir = TempDir::new("pe");
    let path = dir.write("setup.exe", &exe_with(&Version::standard()));
    let before = std::fs::read(&path).expect("contenuto");
    let mtime = modified(&path);
    let info = version_info(&path).expect("lettura");
    assert_eq!(
        info,
        InstallerInfo {
            kind: Some(InstallerKind::Exe),
            product_name: Some("Programma di prova".into()),
            product_version: Some("2.10.91.91".into()),
            company: Some("Editore di prova".into()),
            product_code: None,
            upgrade_code: None,
            file_version: Some("2.10.91.0".into()),
        }
    );
    // Contenuto e data di modifica invariati.
    assert_eq!(std::fs::read(&path).expect("contenuto"), before);
    assert_eq!(modified(&path), mtime);
    // Il percorso con le barre normali e quello con `\\?\` danno lo stesso esito.
    assert_eq!(version_info(&path.replace('\\', "/")).as_ref(), Ok(&info));
    assert_eq!(version_info(&format!(r"\\?\{path}")).as_ref(), Ok(&info));
}

#[test]
fn version_resource_fallbacks() {
    let dir = TempDir::new("pe-ripieghi");
    // Tabella italiana indicata dalla traduzione.
    let mut v = Version::standard();
    v.table = "041004B0";
    v.translation = Some((0x0410, 0x04B0));
    let info = version_info(&dir.write("it.exe", &exe_with(&v))).expect("lettura");
    assert_eq!(info.product_name.as_deref(), Some("Programma di prova"));
    // Code page comune senza traduzione.
    let mut v = Version::standard();
    v.table = "040904E4";
    v.translation = None;
    assert_eq!(version_info(&dir.write("e4.exe", &exe_with(&v))).expect("lettura").product_name.as_deref(), Some("Programma di prova"));
    // Tabella che non è né nella traduzione né tra le comuni: si trova tra quelle presenti.
    let mut v = Version::standard();
    v.table = "041104B0";
    v.translation = Some((0x0407, 0x04E4));
    assert_eq!(version_info(&dir.write("ja.exe", &exe_with(&v))).expect("lettura").company.as_deref(), Some("Editore di prova"));
    // Senza le stringhe di versione: si usa VS_FIXEDFILEINFO.
    let mut v = Version::standard();
    v.strings = vec![("ProductName", "Programma di prova"), ("ProductVersion", "   ")];
    let info = version_info(&dir.write("fisso.exe", &exe_with(&v))).expect("lettura");
    assert_eq!(info.product_version.as_deref(), Some("2.10.91.91"));
    assert_eq!(info.file_version.as_deref(), Some("2.10.91.0"));
    assert_eq!(info.company, None);
    // VS_FIXEDFILEINFO con i numeri a zero e senza versioni scritte: nessuna versione inventata.
    let mut v = Version::standard();
    v.fixed = Some(fixed([0; 4], [0; 4]));
    v.strings = vec![("ProductName", "Solo nome")];
    let info = version_info(&dir.write("nome.exe", &exe_with(&v))).expect("lettura");
    assert_eq!((info.product_name.as_deref(), info.product_version, info.file_version), (Some("Solo nome"), None, None));
    // Senza VS_FIXEDFILEINFO Windows rifiuta la risorsa (ERROR_INVALID_DATA): formato inatteso.
    let mut v = Version::standard();
    v.fixed = None;
    assert_eq!(version_info(&dir.write("senza-fisso.exe", &exe_with(&v))), Err(FormatIssue::Unexpected.into()));
}

#[test]
fn without_version_resource_is_a_normal_outcome() {
    let dir = TempDir::new("pe-senza");
    let empty = InstallerInfo { kind: Some(InstallerKind::Exe), ..Default::default() };
    // Risorse senza RT_VERSION, e nessuna directory delle risorse.
    assert_eq!(version_info(&dir.write("rcdata.exe", &pe(Some((RT_RCDATA, b"dati qualunque"))))), Ok(empty.clone()));
    assert_eq!(version_info(&dir.write("nessuna.exe", &pe(None))), Ok(empty.clone()));
    let facts = installers::info_facts(&empty);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].key, installers::FACT_NO_DATA);
    assert_eq!(facts[0].values, vec![Value::Text("exe".into())]);
}

#[test]
fn malformed_version_resource_is_an_error() {
    let dir = TempDir::new("pe-malformato");
    let good = Version::standard().bytes();
    // Lunghezze incoerenti: un blocco String che dice di finire oltre la radice.
    let mut res = good.clone();
    let at = string_block(&res, "ProductName");
    res[at..at + 2].copy_from_slice(&0x7000u16.to_le_bytes());
    assert_eq!(version_info(&dir.write("lunghezza.exe", &pe(Some((RT_VERSION, &res))))), Err(FormatIssue::Truncated.into()));
    // Una tabella più lunga della risorsa intera.
    let mut res = good.clone();
    let k = utf16z("040904B0");
    let at = res.windows(k.len()).position(|w| w == k.as_slice()).expect("tabella") - 6;
    res[at..at + 2].copy_from_slice(&(good.len() as u16 + 64).to_le_bytes());
    assert_eq!(version_info(&dir.write("tabella.exe", &pe(Some((RT_VERSION, &res))))), Err(FormatIssue::Truncated.into()));
    // Radice con un'altra chiave.
    let mut res = good.clone();
    let k = utf16z("VS_VERSION_INFO");
    res[6..6 + k.len()].copy_from_slice(&utf16z("VS_VERSION_INFX"));
    assert_eq!(version_info(&dir.write("radice.exe", &pe(Some((RT_VERSION, &res))))), Err(FormatIssue::Unexpected.into()));
    // VS_FIXEDFILEINFO senza la sua firma.
    let mut res = good.clone();
    res[40] ^= 0xFF;
    assert_eq!(version_info(&dir.write("firma.exe", &pe(Some((RT_VERSION, &res))))), Err(FormatIssue::Unexpected.into()));
    // Testo non UTF-16 valido: un surrogato isolato nel nome del prodotto. Mai un nome ricostruito.
    let mut res = good.clone();
    let at = string_block(&res, "ProductName") + 6 + utf16z("ProductName").len();
    let value = at + (4 - at % 4) % 4;
    res[value..value + 2].copy_from_slice(&0xD800u16.to_le_bytes());
    assert_eq!(version_info(&dir.write("codifica.exe", &pe(Some((RT_VERSION, &res))))), Err(FormatIssue::Encoding.into()));
    // Risorsa troppo corta per la sua intestazione.
    assert!(version_info(&dir.write("corta.exe", &pe(Some((RT_VERSION, &good[..4]))))).is_err());
}

#[test]
fn files_that_are_not_executables() {
    let dir = TempDir::new("pe-altro");
    assert_eq!(version_info(&dir.write("vuoto.exe", b"")), Err(FormatIssue::Empty.into()));
    assert_eq!(version_info(&dir.write("testo.exe", "non sono un eseguibile\r\n".repeat(50).as_bytes())), Err(FormatIssue::Unexpected.into()));
    assert_eq!(version_info(&dir.0.join("assente.exe").to_string_lossy()), Err(ExtractError::NotPresent));
    let sub = dir.0.join("cartella.exe");
    std::fs::create_dir(&sub).expect("cartella");
    assert_eq!(version_info(&sub.to_string_lossy()), Err(FormatIssue::Unexpected.into()));
    assert_eq!(msi_info(&sub.to_string_lossy()), Err(FormatIssue::Unexpected.into()));
    // Un eseguibile non è un MSI, e viceversa un testo non è un MSI.
    let exe = dir.write("setup.exe", &exe_with(&Version::standard()));
    assert_eq!(msi_info(&exe), Err(FormatIssue::Unexpected.into()));
    assert_eq!(msi_info(&dir.write("vuoto.msi", b"")), Err(FormatIssue::Empty.into()));
    assert_eq!(msi_info(&dir.write("corto.msi", &installers::CFB_SIGNATURE[..5])), Err(FormatIssue::Truncated.into()));
}

#[test]
fn truncated_executables_are_an_error() {
    let dir = TempDir::new("pe-troncato");
    let full = exe_with(&Version::standard());
    for cut in [1, 2, 0x40, 0x80, 0x150, 0x200, 0x200 + 40, 0x200 + 100, full.len() - 200, full.len() - 1] {
        let r = version_info(&dir.write(&format!("troncato-{cut}.exe"), &full[..cut]));
        assert_eq!(r, Err(FormatIssue::Truncated.into()), "troncato a {cut} di {}", full.len());
    }
    // Tabella dei certificati (Authenticode) oltre la fine: un installatore firmato troncato nei dati
    // aggiunti dopo le sezioni.
    let mut signed = full.clone();
    let security = 0x40 + 24 + 112 + 4 * 8;
    signed[security..security + 4].copy_from_slice(&(full.len() as u32).to_le_bytes());
    signed[security + 4..security + 8].copy_from_slice(&0x400u32.to_le_bytes());
    assert_eq!(version_info(&dir.write("firmato-troncato.exe", &signed)), Err(FormatIssue::Truncated.into()));
    signed.resize(full.len() + 0x400, 0);
    assert!(version_info(&dir.write("firmato-intero.exe", &signed)).is_ok());
}

#[test]
fn long_paths() {
    let dir = TempDir::new("pe-lungo");
    let mut deep = dir.0.clone();
    while deep.as_os_str().len() < 300 {
        deep = deep.join("cartella-con-un-nome-abbastanza-lungo");
    }
    std::fs::create_dir_all(&deep).expect("cartelle");
    let p = deep.join("setup.exe");
    std::fs::write(&p, exe_with(&Version::standard())).expect("scrittura");
    let p = p.to_string_lossy().into_owned();
    assert!(p.len() > 260);
    assert_eq!(version_info(&p).map(|i| i.product_name), Ok(Some("Programma di prova".into())));
}

/// `FILE_ATTRIBUTE_OFFLINE`: i dati non sono in locale. Lo si imposta alla creazione del file di prova,
/// come lo vedrebbe Quado su un segnaposto.
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;

#[test]
fn cloud_placeholders_are_not_read() {
    let dir = TempDir::new("cloud");
    for (name, bytes) in [("nel-cloud.exe", exe_with(&Version::standard())), ("nel-cloud.msi", installers::CFB_SIGNATURE.repeat(128))] {
        let p = dir.0.join(name);
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).attributes(FILE_ATTRIBUTE_OFFLINE).open(&p).expect("creazione");
        std::io::Write::write_all(&mut f, &bytes).expect("scrittura");
        drop(f);
        let attrs = std::fs::metadata(&p).expect("metadati").file_attributes();
        assert_ne!(attrs & FILE_ATTRIBUTE_OFFLINE, 0, "attributo non impostato: {attrs:#x}");
        let p = p.to_string_lossy().into_owned();
        assert_eq!(version_info(&p), Err(ExtractError::CloudPlaceholder));
        assert_eq!(msi_info(&p), Err(ExtractError::CloudPlaceholder));
    }
}

/// Le letture degli installatori non aprono sessioni dell'installer, non eseguono e non caricano codice.
/// Controllo sul codice (commenti esclusi) dei due moduli; le importazioni del binario le controlla
/// `cargo xtask importazioni`.
#[test]
fn installer_readers_never_open_sessions_or_run_code() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden =
        ["MsiOpenPackage", "MsiInstall", "MsiConfigure", "MsiSetInternalUI", "CreateProcess", "ShellExecute", "WinExec", "LoadLibrary", "GENERIC_WRITE"];
    for file in ["installers.rs", "installers_win.rs"] {
        let text = std::fs::read_to_string(src.join(file)).expect("sorgente");
        for (n, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            for f in forbidden {
                assert!(!code.contains(f), "{file}:{}: {f}", n + 1);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Corrispondenza con i programmi installati

fn fixture_programs() -> Uninstall {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture").join("registro.reg");
    let reg = RegFile::parse(&std::fs::read_to_string(path).expect("registro")).expect("formato del registro");
    extract::uninstall(&reg).expect("voci")
}

fn index(u: &Uninstall, name: &str) -> usize {
    u.entries.iter().position(|e| e.display_name == name).unwrap_or_else(|| panic!("manca {name}"))
}

fn exe(name: &str, version: &str) -> InstallerInfo {
    InstallerInfo { kind: Some(InstallerKind::Exe), product_name: Some(name.into()), product_version: Some(version.into()), ..Default::default() }
}

fn msi(code: &str, name: &str, version: &str) -> InstallerInfo {
    InstallerInfo {
        kind: Some(InstallerKind::Msi),
        product_name: Some(name.into()),
        product_version: Some(version.into()),
        product_code: Some(code.into()),
        ..Default::default()
    }
}

const SEVEN_ZIP: &str = "{23170F69-40C1-2702-2603-000001000000}";

#[test]
fn three_outcomes_on_the_fixture() {
    let u = fixture_programs();
    let seven = index(&u, "7-Zip 26.03 (x64 edition)");
    let capcut = index(&u, "CapCut");
    // MSI: ProductCode uguale al nome della chiave, anche in minuscolo. Da fonte.
    let same = msi(&SEVEN_ZIP.to_lowercase(), "7-Zip 26.03 (x64 edition)", "26.03.00.0");
    assert_eq!(installers::match_installer(&same, &u), MatchOutcome::SameVersion { entry: seven, provenance: Provenance::FromSource });
    let other = msi(SEVEN_ZIP, "7-Zip 26.02 (x64 edition)", "26.02.00.0");
    assert_eq!(installers::match_installer(&other, &u), MatchOutcome::OtherVersion { entry: seven, provenance: Provenance::FromSource });
    // Exe: nome uguale dopo la normalizzazione, stessa versione. Dedotto, affidabilità media.
    let deduced = Provenance::Deduced(Criterion::OneSource);
    assert_eq!(installers::match_installer(&exe("7-Zip", "26.03"), &u), MatchOutcome::SameVersion { entry: seven, provenance: deduced });
    assert_eq!(installers::match_installer(&exe("CapCut", "9.4.0.4015"), &u), MatchOutcome::SameVersion { entry: capcut, provenance: deduced });
    // Exe: stesso nome, versione diversa. Coincide solo il nome: affidabilità bassa.
    let name_only = Provenance::Deduced(Criterion::NameOnly);
    assert_eq!(installers::match_installer(&exe("CapCut", "9.3.0.3970"), &u), MatchOutcome::OtherVersion { entry: capcut, provenance: name_only });
    // Non installato: nome diverso, o solo in parte uguale.
    assert_eq!(installers::match_installer(&exe("Programma di prova", "1.0"), &u), MatchOutcome::NotInstalled);
    assert_eq!(installers::match_installer(&exe("CapCut Desktop", "9.4.0.4015"), &u), MatchOutcome::NotInstalled);
    assert_eq!(installers::match_installer(&exe("Cap", "9.4.0.4015"), &u), MatchOutcome::NotInstalled);
    // MSI con un codice che non c'è: si prova il nome.
    let by_name = msi("{00000000-1111-2222-3333-444444444444}", "Steam", "2.10.91.91");
    let steam = index(&u, "Steam");
    assert_eq!(installers::match_installer(&by_name, &u), MatchOutcome::SameVersion { entry: steam, provenance: deduced });
    // Un "codice" che non è un GUID non vale come chiave, anche se una chiave ha quel nome.
    let fake = msi("Steam", "Altro", "1.0");
    assert_eq!(installers::match_installer(&fake, &u), MatchOutcome::NotInstalled);
    // Senza nome né codice.
    assert_eq!(installers::match_installer(&InstallerInfo::default(), &u), MatchOutcome::NotInstalled);
}

fn entry(hive: Hive, key: &str, name: &str, version: Option<&str>) -> UninstallEntry {
    UninstallEntry {
        hive,
        key: key.into(),
        display_name: name.into(),
        version: version.map(str::to_owned),
        publisher: None,
        install_location: None,
        uninstall_string: None,
        display_icon: None,
        install_date: None,
        estimated_kb: None,
        system_component: false,
    }
}

#[test]
fn several_candidates() {
    let u = Uninstall {
        entries: vec![
            entry(Hive::User, "Foo-utente", "Foo", Some("2.0")),
            entry(Hive::Machine32, "Foo-32", "Foo (x86)", Some("1.0")),
            entry(Hive::Machine, "Foo-64", "Foo x64", Some("1.0")),
            entry(Hive::Machine, "Bar", "Bar", None),
        ],
        skipped: Vec::new(),
    };
    // Prima la stessa versione, anche se è in HKCU.
    assert!(matches!(installers::match_installer(&exe("Foo", "2.0.0"), &u), MatchOutcome::SameVersion { entry: 0, .. }));
    // A parità di versione, HKLM a 64 bit prima di WOW6432Node.
    assert!(matches!(installers::match_installer(&exe("Foo (64-bit)", "1.0"), &u), MatchOutcome::SameVersion { entry: 2, .. }));
    // Nessuna con la stessa versione: HKLM prima di HKCU.
    assert!(matches!(installers::match_installer(&exe("Foo", "3.0"), &u), MatchOutcome::OtherVersion { entry: 2, .. }));
    // Versione non indicata nella voce: versione diversa, mai uguale.
    let r = installers::match_installer(&exe("Bar", "1.0"), &u);
    assert_eq!(r, MatchOutcome::OtherVersion { entry: 3, provenance: Provenance::Deduced(Criterion::NameOnly) });
    assert_eq!(installers::match_fact(&r, &exe("Bar", "1.0"), &u).key, installers::FACT_NAME_VERSION_UNKNOWN);
    // Nello stesso hive, le voci visibili prima dei componenti di sistema (anche se lette dopo).
    let mut sys = u.clone();
    sys.entries[1].hive = Hive::Machine;
    sys.entries[1].system_component = true;
    assert!(matches!(installers::match_installer(&exe("Foo", "1.0"), &sys), MatchOutcome::SameVersion { entry: 2, .. }));
    // Stesso ProductCode in due hive: HKLM prima di HKCU.
    let code = "{11111111-2222-3333-4444-555555555555}";
    let two = Uninstall {
        entries: vec![entry(Hive::User, code, "Pacchetto", Some("1.0")), entry(Hive::Machine, &code.to_lowercase(), "Pacchetto", Some("1.0"))],
        skipped: Vec::new(),
    };
    assert_eq!(installers::match_installer(&msi(code, "Pacchetto", "1.0"), &two), MatchOutcome::SameVersion { entry: 1, provenance: Provenance::FromSource });
}

// ---------------------------------------------------------------------------------------------
// Fatti per il record

fn texts(facts: &[Fact]) -> Vec<(&str, Vec<String>)> {
    facts
        .iter()
        .map(|f| {
            let v = f.values.iter().map(|v| if let Value::Text(t) = v { t.clone() } else { format!("{v:?}") }).collect();
            (f.key.as_str(), v)
        })
        .collect()
}

#[test]
fn facts_with_provenance_and_source() {
    let u = fixture_programs();
    // Dati letti: tutti da fonte, con la fonte dell'installatore.
    let mut info = msi(SEVEN_ZIP, "7-Zip 26.03 (x64 edition)", "26.03.00.0");
    info.company = Some("Igor Pavlov".into());
    let facts = installers::info_facts(&info);
    assert!(facts.iter().all(|f| f.provenance == Provenance::FromSource && f.source == Source::MsiProperties));
    assert_eq!(
        texts(&facts),
        vec![
            (installers::FACT_PRODUCT, vec!["7-Zip 26.03 (x64 edition)".into(), "26.03.00.0".into()]),
            (installers::FACT_COMPANY, vec!["Igor Pavlov".into()]),
            (installers::FACT_PRODUCT_CODE, vec![SEVEN_ZIP.into()]),
        ]
    );
    let mut e = exe("Programma di prova", "2.10.91.91");
    e.file_version = Some("2.10.91.0".into());
    let facts = installers::info_facts(&e);
    assert!(facts.iter().all(|f| f.source == Source::VersionResource));
    assert_eq!(facts.last().map(|f| f.key.as_str()), Some(installers::FACT_FILE_VERSION));
    // Versione del file uguale a quella del prodotto: non si ripete.
    e.file_version = Some("2.10.91.91".into());
    assert!(installers::info_facts(&e).iter().all(|f| f.key != installers::FACT_FILE_VERSION));
    let nameless = InstallerInfo { kind: Some(InstallerKind::Exe), product_version: Some("1.0".into()), ..Default::default() };
    assert_eq!(installers::info_facts(&nameless)[0].key, installers::FACT_VERSION_NO_NAME);

    // Corrispondenza: fonte = voce di disinstallazione con il suo hive.
    let f = installers::match_fact(&installers::match_installer(&info, &u), &info, &u);
    assert_eq!((f.key.as_str(), f.provenance, &f.source), (installers::FACT_SAME_CODE, Provenance::FromSource, &Source::Uninstall(Hive::Machine)));
    assert_eq!(f.values, vec![Value::Text("7-Zip 26.03 (x64 edition)".into()), Value::Text("26.03.00.0".into())]);
    let old = msi(SEVEN_ZIP, "7-Zip", "26.02");
    let f = installers::match_fact(&installers::match_installer(&old, &u), &old, &u);
    assert_eq!(f.key, installers::FACT_CODE_OTHER_VERSION);
    assert_eq!(f.values.len(), 3);
    let cap = exe("CapCut", "9.4.0.4015");
    let f = installers::match_fact(&installers::match_installer(&cap, &u), &cap, &u);
    let user = Source::Uninstall(Hive::User);
    assert_eq!((f.key.as_str(), f.provenance, &f.source), (installers::FACT_SAME_NAME, Provenance::Deduced(Criterion::OneSource), &user));
    let cap_old = exe("CapCut", "9.3.0.3970");
    let f = installers::match_fact(&installers::match_installer(&cap_old, &u), &cap_old, &u);
    assert_eq!((f.key.as_str(), f.provenance), (installers::FACT_NAME_OTHER_VERSION, Provenance::Deduced(Criterion::NameOnly)));
    assert_eq!(f.values[1..], [Value::Text("9.4.0.4015".into()), Value::Text("9.3.0.3970".into())]);
    // Il gioco di prova non dichiara la versione.
    let mine = exe("Gioco di Prova", "1.0");
    let f = installers::match_fact(&installers::match_installer(&mine, &u), &mine, &u);
    assert_eq!(f.key, installers::FACT_NAME_VERSION_UNKNOWN);

    // Non installato: con il codice affidabilità media, solo con il nome bassa; fonte dell'installatore.
    let unknown = msi("{00000000-1111-2222-3333-444444444444}", "Nessuno", "1.0");
    let f = installers::match_fact(&MatchOutcome::NotInstalled, &unknown, &u);
    let expected = (installers::FACT_NOT_INSTALLED, Provenance::Deduced(Criterion::OneSource), &Source::MsiProperties);
    assert_eq!((f.key.as_str(), f.provenance, &f.source), expected);
    assert_eq!(f.values, vec![Value::Text("codice".into())]);
    let f = installers::match_fact(&MatchOutcome::NotInstalled, &exe("Nessuno", "1.0"), &u);
    assert_eq!((f.provenance, &f.values), (Provenance::Deduced(Criterion::NameOnly), &vec![Value::Text("nome".into())]));
    // Senza dati, o con un indice che non esiste: confronto non disponibile, mai un "non installato".
    let f = installers::match_fact(&MatchOutcome::NotInstalled, &InstallerInfo::default(), &u);
    assert_eq!((f.key.as_str(), f.provenance), (installers::FACT_NOT_COMPARABLE, Provenance::Unavailable));
    let f = installers::match_fact(&MatchOutcome::SameVersion { entry: 999, provenance: Provenance::FromSource }, &info, &u);
    assert_eq!(f.key, installers::FACT_NOT_COMPARABLE);
}

/// La lente Download usa i fatti di questo modulo, con la loro provenienza: la corrispondenza (riassunto
/// della voce) e i dati letti, poi il tipo secondo l'estensione. Nessun fatto proprio della lente.
#[test]
fn lens_voices_carry_the_installer_facts() {
    use quado_kb::lens::{self, Finding, InstallerMatch, InstallerRead, InstallerStatus, LensInput};
    use quado_model::tree::w;
    use quado_model::{NodeData, Tree};
    let u = fixture_programs();
    let mut t = Tree::with_root(&w("C:\\"), 0);
    let dl = t.push(Tree::ROOT, &w("Downloads"), NodeData { flags: quado_model::Flags::DIR, ..Default::default() });
    let file = |bytes: u64| NodeData { logical: bytes, alloc: bytes.div_ceil(4096) * 4096, ..Default::default() };
    let a = t.push(dl, &w("7z2603-x64.msi"), file(1_900_000));
    let b = t.push(dl, &w("CapCutSetup.exe"), file(2_000_000));
    let c = t.push(dl, &w("prova.exe"), file(3_000_000));
    t.aggregate();
    t.seal();
    let infos = [
        (a, msi(SEVEN_ZIP, "7-Zip 26.03 (x64 edition)", "26.03.00.0"), InstallerStatus::SameVersion, Provenance::FromSource),
        (b, exe("CapCut", "9.3.0.3970"), InstallerStatus::OtherVersion, Provenance::Deduced(Criterion::NameOnly)),
        (c, exe("Programma di prova", "1.0"), InstallerStatus::NotInstalled, Provenance::Deduced(Criterion::NameOnly)),
    ];
    let mut input = LensInput::default();
    for (n, info, _, _) in &infos {
        input.installers.insert(*n, InstallerRead { info: Ok(info.clone()), matched: Some(InstallerMatch::compute(info, &u)) });
    }
    let out = lens::findings(&t, &quado_model::downloads::build(&t, dl), &input);
    for (n, info, status, provenance) in &infos {
        let v = out.iter().find(|v| v.nodes == [*n]).expect("voce");
        assert_eq!(v.finding, Finding::Installer(*status));
        let mut want = vec![installers::match_fact(&installers::match_installer(info, &u), info, &u)];
        want.extend(installers::info_facts(info));
        let facts = &v.record.identity.0;
        assert_eq!(facts[..want.len()], want[..], "{}", t.name_lossy(*n));
        assert_eq!(facts[0].provenance, *provenance);
        assert_eq!(facts.last().map(|f| f.key.as_str()), Some("fatto-lente-famiglia"));
    }
}

// ---------------------------------------------------------------------------------------------
// Cache

fn key(path: &str, size: u64, modified: i64) -> ContentKey {
    ContentKey { path: path.into(), size, modified, file_id: 7 }
}

#[test]
fn cache_reads_only_new_or_changed_files() {
    let a = key(r"C:\Users\u\Downloads\a.exe", 100, 1);
    let b = key(r"C:\Users\u\Downloads\b.msi", 200, 2);
    let c = key(r"C:\Users\u\Downloads\c.exe", 300, 3);
    let mut cache = InstallerCache::new();
    let all = vec![a.clone(), b.clone(), c.clone()];
    assert_eq!(cache.to_read(&all).len(), 3);
    for k in &all {
        cache.insert(k.clone(), Ok(exe("x", "1.0")));
    }
    assert_eq!(cache.len(), 3);
    // Seconda richiesta: nulla da leggere. Il percorso non distingue le maiuscole.
    assert!(cache.to_read(&all).is_empty());
    let upper = key(r"C:\USERS\U\DOWNLOADS\A.EXE", 100, 1);
    assert!(cache.to_read(std::slice::from_ref(&upper)).is_empty());
    assert!(cache.get(&upper).is_some());
    // Cambia la dimensione di b, la data di c, l'ID di a: si rilegge solo quello.
    let b2 = key(&b.path, 201, 2);
    assert_eq!(cache.to_read(&[a.clone(), b2.clone(), c.clone()]), vec![&b2]);
    assert_eq!(cache.get(&b2), None);
    let c2 = key(&c.path, 300, 4);
    assert_eq!(cache.to_read(&[a.clone(), b.clone(), c2.clone()]), vec![&c2]);
    let a2 = ContentKey { file_id: 8, ..a.clone() };
    assert_eq!(cache.to_read(&[a2.clone(), b.clone(), c.clone()]), vec![&a2]);
    // Lo stesso file chiesto due volte si legge una volta.
    let fresh = key(r"C:\x\nuovo.exe", 1, 1);
    assert_eq!(cache.to_read(&[fresh.clone(), fresh.clone()]).len(), 1);
    // Un errore di formato dipende dal contenuto: non si rilegge. Un errore del momento sì.
    cache.insert(a.clone(), Err(FormatIssue::Truncated.into()));
    cache.insert(b.clone(), Err(ExtractError::Io(32)));
    cache.insert(c.clone(), Err(ExtractError::CloudPlaceholder));
    assert_eq!(cache.to_read(&all), vec![&b, &c]);
    assert_eq!(cache.get(&a), Some(&Err(FormatIssue::Truncated.into())));
}

/// Il flusso dell'interfaccia: chiedere alla cache quali file leggere, leggerli, salvare l'esito.
fn request(cache: &mut InstallerCache, paths: &[&str]) -> usize {
    let keys: Vec<ContentKey> = paths
        .iter()
        .map(|p| {
            let m = std::fs::metadata(p).expect("metadati");
            ContentKey { path: (*p).to_owned(), size: m.len(), modified: m.last_write_time() as i64, file_id: 0 }
        })
        .collect();
    let todo: Vec<ContentKey> = cache.to_read(&keys).into_iter().cloned().collect();
    for k in &todo {
        let r = match installers::installer_kind(&k.path) {
            Some(InstallerKind::Msi) => msi_info(&k.path),
            _ => version_info(&k.path),
        };
        cache.insert(k.clone(), r);
    }
    todo.len()
}

#[test]
fn cache_with_real_files() {
    let dir = TempDir::new("cache");
    let one = dir.write("uno.exe", &exe_with(&Version::standard()));
    let mut v = Version::standard();
    v.strings[3].1 = "Secondo programma";
    let two = dir.write("due.exe", &exe_with(&v));
    let mut cache = InstallerCache::new();
    assert_eq!(request(&mut cache, &[&one, &two]), 2);
    assert_eq!(request(&mut cache, &[&one, &two]), 0, "seconda richiesta: nessuna lettura");
    // Il secondo file cambia (più lungo, data nuova): si rilegge solo lui, con i dati nuovi.
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut v = Version::standard();
    v.strings[3].1 = "Secondo programma, aggiornato";
    std::fs::write(&two, exe_with(&v)).expect("scrittura");
    assert_eq!(request(&mut cache, &[&one, &two]), 1);
    let m = std::fs::metadata(&two).expect("metadati");
    let k = ContentKey { path: two.to_uppercase(), size: m.len(), modified: m.last_write_time() as i64, file_id: 0 };
    let name = cache.get(&k).and_then(|r| r.as_ref().ok()).and_then(|i| i.product_name.clone());
    assert_eq!(name.as_deref(), Some("Secondo programma, aggiornato"));
}

// ---------------------------------------------------------------------------------------------
// Testi

/// Messaggio di una chiave: la riga della chiave e le righe rientrate che seguono.
fn message<'a>(ftl: &'a str, key: &str) -> Option<(Option<&'a str>, String)> {
    let lines: Vec<&str> = ftl.lines().collect();
    let k = lines.iter().position(|l| l.split_once(" =").is_some_and(|(n, _)| n.trim() == key))?;
    let comment = k.checked_sub(1).map(|p| lines[p]).filter(|l| l.starts_with("# "));
    let mut body = lines[k].split_once(" =").map(|(_, v)| v.to_owned()).unwrap_or_default();
    for l in &lines[k + 1..] {
        if l.starts_with([' ', '\t']) || l.starts_with('}') {
            body.push('\n');
            body.push_str(l);
        } else {
            break;
        }
    }
    Some((comment, body))
}

fn variables(body: &str) -> BTreeSet<String> {
    body.split('$').skip(1).map(|r| r.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect()).collect()
}

#[test]
fn every_fact_key_has_texts_in_both_languages() {
    let (it, en) = (quado_kb::TEXTS[0].1, quado_kb::TEXTS[1].1);
    let forbidden =
        ["sicuro da", "inutilizzat", "mai usato", "non più in uso", "spazzatura", "ottimizza", "unused", "never used", "safe to", "junk", "optimiz"];
    let mut problems = Vec::new();
    // I fatti e il nome delle voci degli installatori nella lente Download.
    let name = quado_kb::lens::name_key(quado_model::record::Function::Installer);
    for k in installers::FACT_KEYS.iter().copied().chain([name.as_str()]) {
        let Some((comment, it_body)) = message(it, k) else {
            problems.push(format!("{k}: manca in it.ftl"));
            continue;
        };
        let Some((_, en_body)) = message(en, k) else {
            problems.push(format!("{k}: manca in en.ftl"));
            continue;
        };
        if comment.is_none() {
            problems.push(format!("{k}: manca il commento di contesto in it.ftl"));
        }
        if variables(&it_body) != variables(&en_body) {
            problems.push(format!("{k}: segnaposto diversi ({:?} e {:?})", variables(&it_body), variables(&en_body)));
        }
        for body in [&it_body, &en_body] {
            let lower = body.to_lowercase();
            if body.contains(['!', '\u{2014}', '\u{2013}']) || forbidden.iter().any(|w| lower.contains(w)) {
                problems.push(format!("{k}: testo non ammesso «{body}»"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Ogni fatto prodotto ha una chiave nota e almeno i valori che il testo usa.
fn check_fact(f: &Fact) {
    assert!(installers::FACT_KEYS.contains(&f.key.as_str()), "chiave sconosciuta {}", f.key);
    let (_, body) = message(quado_kb::TEXTS[0].1, &f.key).expect("testo");
    for v in variables(&body) {
        let k = match v.trim_end_matches("_testo") {
            "a" => 0,
            "b" => 1,
            "c" => 2,
            other => panic!("{}: segnaposto {other} fuori dalla convenzione dei fatti", f.key),
        };
        assert!(k < f.values.len(), "{}: manca il valore per ${v}", f.key);
    }
}

fn arb_text() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some(String::new())),
        Just(Some("7-Zip".to_owned())),
        Just(Some("CapCut".to_owned())),
        Just(Some("26.03".to_owned())),
        Just(Some("9.4.0.4015".to_owned())),
        Just(Some(SEVEN_ZIP.to_owned())),
        "\\PC{0,24}".prop_map(Some),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// Corrispondenza e fatti su dati qualunque: nessun panic, chiavi note, valori sufficienti, e una
    /// provenienza coerente con l'esito.
    #[test]
    fn matching_and_facts_on_any_data(name in arb_text(), version in arb_text(), code in arb_text(), company in arb_text(), msi in any::<bool>()) {
        let u = fixture_programs();
        let info = InstallerInfo {
            kind: Some(if msi { InstallerKind::Msi } else { InstallerKind::Exe }),
            product_name: name,
            product_version: version,
            company,
            product_code: code,
            ..Default::default()
        };
        for f in installers::info_facts(&info) {
            check_fact(&f);
        }
        let outcome = installers::match_installer(&info, &u);
        let f = installers::match_fact(&outcome, &info, &u);
        check_fact(&f);
        match outcome {
            MatchOutcome::SameVersion { entry, provenance } | MatchOutcome::OtherVersion { entry, provenance } => {
                prop_assert!(entry < u.entries.len());
                prop_assert_eq!(f.provenance, provenance);
                prop_assert!(matches!(f.source, Source::Uninstall(_)));
                prop_assert!(matches!(provenance, Provenance::FromSource | Provenance::Deduced(Criterion::OneSource | Criterion::NameOnly)));
            }
            MatchOutcome::NotInstalled => prop_assert!(!matches!(f.source, Source::Uninstall(_))),
        }
    }

    /// Il controllo della risorsa di versione non va in panic su byte qualunque né su risorse modificate.
    #[test]
    fn version_resource_check_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..512), at in 0usize..1000, byte in any::<u8>()) {
        let _ = installers::version_resource_tables(&bytes);
        let mut good = Version::standard().bytes();
        let k = at % good.len();
        good[k] = byte;
        let _ = installers::version_resource_tables(&good);
    }

    #[test]
    fn names_and_versions_never_panic(a in "\\PC{0,40}", b in "\\PC{0,20}") {
        let _ = installers::normalize_name(&a, Some(&b));
        let _ = installers::same_version(&a, &b);
        let _ = installers::installer_kind(&a);
    }

    /// Il controllo del file composto su byte qualunque dopo la firma: errore o esito, mai un panic.
    #[test]
    fn compound_file_check_never_panics(tail in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let mut bytes = installers::CFB_SIGNATURE.to_vec();
        bytes.extend(tail);
        let size = bytes.len() as u64;
        let _ = installers::compound_file_check(&mut std::io::Cursor::new(bytes), size);
    }
}

// ---------------------------------------------------------------------------------------------
// Misura dei tempi di lettura: solo a mano, sui file scelti da chi misura

/// Tempo di `version_info` e `msi_info` sugli installatori di una cartella, 5 giri.
/// - `QUADO_MISURA_CARTELLA`: la cartella (primo livello; con `QUADO_MISURA_SOTTOCARTELLE` anche un
///   livello di sottocartelle);
/// - `QUADO_MISURA_ESCLUDI`: parti di percorso da saltare, separate da `;`, senza distinzione di maiuscole;
/// - `QUADO_MISURA_DETTAGLIO`: il primo giro scrive i valori letti, da confrontare con
///   `(Get-Item f).VersionInfo` di PowerShell.
///
/// Il primo giro dopo un riavvio è quello a cache fredda; gli altri misurano la cache del sistema e di
/// Defender. Uso: `cargo test -p quado-kb --release --test installatori -- --ignored --nocapture misura`
#[test]
#[ignore = "legge i file di una cartella scelta da chi misura: controllo manuale"]
fn misura_letture_installatori() {
    let Ok(dir) = std::env::var("QUADO_MISURA_CARTELLA") else {
        println!("QUADO_MISURA_CARTELLA non impostata: nessuna misura");
        return;
    };
    let skip: Vec<String> =
        std::env::var("QUADO_MISURA_ESCLUDI").unwrap_or_default().split(';').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
    let detail = std::env::var_os("QUADO_MISURA_DETTAGLIO").is_some();
    let list = |d: &Path| -> Vec<PathBuf> { std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default() };
    let mut paths = list(Path::new(&dir));
    if std::env::var_os("QUADO_MISURA_SOTTOCARTELLE").is_some() {
        let subs: Vec<PathBuf> = paths.iter().filter(|p| p.is_dir()).cloned().collect();
        paths.extend(subs.iter().flat_map(|s| list(s)));
    }
    let mut files: Vec<String> = paths
        .iter()
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|p| installers::installer_kind(p).is_some())
        .filter(|p| !skip.iter().any(|s| p.to_lowercase().contains(s.as_str())))
        .collect();
    files.sort();
    println!("file: {} ({} msi)", files.len(), files.iter().filter(|p| installers::installer_kind(p) == Some(InstallerKind::Msi)).count());
    let mut per_file: Vec<Vec<f64>> = vec![Vec::new(); files.len()];
    let mut outcomes = std::collections::BTreeMap::<String, usize>::new();
    for round in 1..=5 {
        let start = std::time::Instant::now();
        for (k, p) in files.iter().enumerate() {
            let t = std::time::Instant::now();
            let r = match installers::installer_kind(p) {
                Some(InstallerKind::Msi) => msi_info(p),
                _ => version_info(p),
            };
            per_file[k].push(t.elapsed().as_secs_f64() * 1000.0);
            if round == 1 && detail {
                let name = Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                match &r {
                    Ok(i) => {
                        let fields = [&i.product_name, &i.product_version, &i.company, &i.file_version, &i.product_code];
                        let values: Vec<&str> = fields.iter().map(|x| x.as_deref().unwrap_or_default()).collect();
                        println!("{name}\t{}", values.join("\t"));
                    }
                    Err(e) => println!("{name}\t{e:?}"),
                }
            }
            if round == 1 {
                let what = match &r {
                    Ok(i) if i.product_name.is_some() => "con ProductName".to_owned(),
                    Ok(_) => "senza ProductName".to_owned(),
                    Err(e) => format!("{e:?}"),
                };
                *outcomes.entry(what).or_default() += 1;
            }
        }
        println!("giro {round}: {:.0} ms in tutto", start.elapsed().as_secs_f64() * 1000.0);
    }
    println!("esiti del primo giro: {outcomes:?}");
    let mut first: Vec<f64> = per_file.iter().map(|v| v[0]).collect();
    first.sort_by(f64::total_cmp);
    if let (Some(max), false) = (first.last(), first.is_empty()) {
        let at = |q: f64| first[((first.len() - 1) as f64 * q).round() as usize];
        println!("primo giro per file: mediana {:.1} ms, p95 {:.1} ms, massimo {max:.1} ms", at(0.5), at(0.95));
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// Eseguibili con un byte cambiato nella risorsa di versione: `version_info` dà un esito o un errore,
    /// mai un panic né un crash nelle API di Windows.
    #[test]
    fn mutated_executables_are_handled(at in 0usize..4096, byte in any::<u8>()) {
        let dir = TempDir::new("pe-mutato");
        let mut file = exe_with(&Version::standard());
        let k = 0x200 + 88 + at % (file.len() - 0x200 - 88);
        file[k] = byte;
        let _ = version_info(&dir.write("mutato.exe", &file));
    }
}
