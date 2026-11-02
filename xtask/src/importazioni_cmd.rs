// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask importazioni [EXE]`: tabella delle importazioni dell'eseguibile (predefinito
//! `target\release\quado.exe`), normali e ritardate, letta con un parser scritto qui e controllato sui
//! confini di ogni struttura (un file troncato o malformato dà un errore, mai un accesso fuori dal file).
//!
//! Vietati:
//! - qualunque funzione di `winhttp.dll`, `wininet.dll`, `urlmon.dll` (e anche `dnsapi.dll` e
//!   `websocket.dll`, che risolvono nomi o aprono connessioni);
//! - da `ws2_32.dll`: `connect`, `WSAConnect`, `WSAConnectByName*`, `getaddrinfo`, `GetAddrInfo*`,
//!   `gethostbyname`; un ordinale di `ws2_32` che non si risolve con la tabella delle esportazioni di
//!   System32 conta come vietato;
//! - `MsiOpenPackage*`, che apre l'MSI con lo stato della macchina: Quado legge gli MSI in sola lettura;
//! - `CreateProcess*` e `WinExec`: Quado non crea processi, l'helper parte con `ShellExecuteExW`.
//!
//! Il comando controlla anche `DependentLoadFlags` = 0x800 nel load config (come `dll_cmd`). Stampa una
//! tabella per DLL, le funzioni di `ws2_32` importate e l'esito; esce con un errore se c'è una violazione.
//!
//! Limite: la tabella non vede le funzioni caricate a mano con `LoadLibrary` e `GetProcAddress` (la
//! libreria standard di Rust lo fa per alcune funzioni facoltative di Windows); il comando dice se il
//! binario le importa, e la prova della sessione (`sessione-prova`) copre il comportamento.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Eseguibile predefinito.
pub const DEFAULT_EXE: &str = r"target\release\quado.exe";

/// Librerie di rete vietate per intero.
const NET_DLLS: [&str; 5] = ["winhttp.dll", "wininet.dll", "urlmon.dll", "dnsapi.dll", "websocket.dll"];

/// Funzioni di `ws2_32.dll` vietate: nome esatto o prefisso (`*`).
const WS2_FORBIDDEN: [&str; 6] = ["connect", "WSAConnect", "WSAConnectByName*", "getaddrinfo", "GetAddrInfo*", "gethostbyname"];

/// Funzione importata: per nome o per ordinale.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Symbol {
    Name(String),
    Ordinal(u16),
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(n) => f.write_str(n),
            Self::Ordinal(o) => write!(f, "#{o}"),
        }
    }
}

/// Una voce della tabella delle importazioni. `resolved` è il nome di un ordinale, se la DLL di System32
/// lo esporta con un nome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub dll: String,
    pub symbol: Symbol,
    pub delayed: bool,
    pub resolved: Option<String>,
}

impl Import {
    /// Nome della funzione, se si conosce (dal nome o dall'ordinale risolto).
    pub fn name(&self) -> Option<&str> {
        match &self.symbol {
            Symbol::Name(n) => Some(n),
            Symbol::Ordinal(_) => self.resolved.as_deref(),
        }
    }

    fn label(&self) -> String {
        match (&self.symbol, &self.resolved) {
            (Symbol::Ordinal(o), Some(n)) => format!("{n} (#{o})"),
            (s, _) => s.to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Section {
    va: u32,
    vsize: u32,
    raw: u32,
    rsize: u32,
}

/// Intestazioni di un PE, lette con i controlli dei confini.
pub struct Pe<'a> {
    b: &'a [u8],
    pub pe64: bool,
    image_base: u64,
    size_of_headers: u32,
    sections: Vec<Section>,
    dirs: Vec<(u32, u32)>,
}

/// Massimi per i cicli su dati letti dal file: un PE malformato non tiene il comando in un ciclo lungo.
const MAX_DESCRIPTORS: usize = 4096;
const MAX_THUNKS: usize = 1 << 16;
const MAX_NAME: usize = 1024;

impl<'a> Pe<'a> {
    pub fn parse(b: &'a [u8]) -> Result<Self, String> {
        if b.get(0..2) != Some(b"MZ") {
            return Err("manca la firma MZ".into());
        }
        let pe = u32_at(b, 0x3c)? as usize;
        if b.get(pe..pe.checked_add(4).ok_or("offset PE")?) != Some(b"PE\0\0") {
            return Err("manca la firma PE".into());
        }
        let coff = pe + 4;
        let n_sections = u16_at(b, coff + 2)? as usize;
        let opt_size = u16_at(b, coff + 16)? as usize;
        let opt = coff + 20;
        let magic = u16_at(b, opt)?;
        let (pe64, image_base, n_dirs_at, dirs_at) = match magic {
            0x10b => (false, u64::from(u32_at(b, opt + 28)?), opt + 92, opt + 96),
            0x20b => (true, u64_at(b, opt + 24)?, opt + 108, opt + 112),
            m => return Err(format!("intestazione opzionale sconosciuta ({m:#x})")),
        };
        let size_of_headers = u32_at(b, opt + 60)?;
        let n_dirs = (u32_at(b, n_dirs_at)? as usize).min(16);
        if dirs_at + n_dirs * 8 > opt + opt_size {
            return Err("directory dei dati fuori dall'intestazione opzionale".into());
        }
        let mut dirs = Vec::with_capacity(n_dirs);
        for i in 0..n_dirs {
            dirs.push((u32_at(b, dirs_at + i * 8)?, u32_at(b, dirs_at + i * 8 + 4)?));
        }
        if n_sections > 96 {
            return Err(format!("troppe sezioni ({n_sections})"));
        }
        let table = opt + opt_size;
        let mut sections = Vec::with_capacity(n_sections);
        for i in 0..n_sections {
            let s = table + i * 40;
            sections.push(Section { vsize: u32_at(b, s + 8)?, va: u32_at(b, s + 12)?, rsize: u32_at(b, s + 16)?, raw: u32_at(b, s + 20)? });
        }
        Ok(Self { b, pe64, image_base, size_of_headers, sections, dirs })
    }

    /// Directory dei dati `i` (RVA, dimensione); `None` se assente o vuota.
    fn dir(&self, i: usize) -> Option<(u32, u32)> {
        self.dirs.get(i).copied().filter(|&(rva, _)| rva != 0)
    }

    /// Offset nel file di un RVA, solo se i byte stanno nei dati della sezione presenti nel file.
    fn offset(&self, rva: u32) -> Result<usize, String> {
        for s in &self.sections {
            let span = s.vsize.max(s.rsize);
            if rva >= s.va && u64::from(rva) < u64::from(s.va) + u64::from(span) {
                let delta = rva - s.va;
                if delta >= s.rsize {
                    return Err(format!("RVA {rva:#x} senza dati nel file"));
                }
                let off = s.raw as usize + delta as usize;
                return if off < self.b.len() { Ok(off) } else { Err(format!("RVA {rva:#x} oltre la fine del file")) };
            }
        }
        if rva < self.size_of_headers && (rva as usize) < self.b.len() {
            return Ok(rva as usize);
        }
        Err(format!("RVA {rva:#x} fuori dalle sezioni"))
    }

    fn u16_rva(&self, rva: u32) -> Result<u16, String> {
        u16_at(self.b, self.offset(rva)?)
    }

    fn u32_rva(&self, rva: u32) -> Result<u32, String> {
        u32_at(self.b, self.offset(rva)?)
    }

    fn u64_rva(&self, rva: u32) -> Result<u64, String> {
        u64_at(self.b, self.offset(rva)?)
    }

    /// Stringa ASCII terminata da zero, al più `MAX_NAME` byte.
    fn cstr(&self, rva: u32) -> Result<String, String> {
        let off = self.offset(rva)?;
        let rest = &self.b[off..];
        let end = rest.iter().take(MAX_NAME).position(|&c| c == 0).ok_or_else(|| format!("nome senza terminatore a {rva:#x}"))?;
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    }

    /// Voci di una tabella di thunk (nomi o ordinali) a partire da `rva`, fino allo zero.
    fn thunks(&self, rva: u32) -> Result<Vec<Symbol>, String> {
        let step = if self.pe64 { 8 } else { 4 };
        let mut out = Vec::new();
        for k in 0..MAX_THUNKS {
            let at = rva.checked_add((k * step) as u32).ok_or("RVA dei thunk")?;
            let v = if self.pe64 { self.u64_rva(at)? } else { u64::from(self.u32_rva(at)?) };
            if v == 0 {
                return Ok(out);
            }
            let ordinal_flag = if self.pe64 { 1u64 << 63 } else { 1u64 << 31 };
            if v & ordinal_flag != 0 {
                out.push(Symbol::Ordinal((v & 0xffff) as u16));
            } else {
                if v > u64::from(u32::MAX >> 1) {
                    return Err(format!("thunk non valido {v:#x}"));
                }
                // IMAGE_IMPORT_BY_NAME: suggerimento (2 byte) e nome.
                let hint_name = v as u32;
                out.push(Symbol::Name(self.cstr(hint_name.checked_add(2).ok_or("RVA del nome")?)?));
            }
        }
        Err("tabella dei thunk senza fine".into())
    }

    /// Importazioni normali (directory 1) e ritardate (directory 13), nell'ordine del file.
    pub fn imports(&self) -> Result<Vec<Import>, String> {
        let mut out = Vec::new();
        if let Some((rva, _)) = self.dir(1) {
            for k in 0..MAX_DESCRIPTORS {
                // IMAGE_IMPORT_DESCRIPTOR: OriginalFirstThunk, TimeDateStamp, ForwarderChain, Name, FirstThunk.
                let d = rva.checked_add((k * 20) as u32).ok_or("RVA dei descrittori")?;
                let (oft, name, ft) = (self.u32_rva(d)?, self.u32_rva(d.wrapping_add(12))?, self.u32_rva(d.wrapping_add(16))?);
                if oft == 0 && name == 0 && ft == 0 {
                    break;
                }
                if k + 1 == MAX_DESCRIPTORS {
                    return Err("troppi descrittori di importazione".into());
                }
                let dll = self.cstr(name)?;
                let table = if oft != 0 { oft } else { ft };
                for symbol in self.thunks(table)? {
                    out.push(Import { dll: dll.clone(), symbol, delayed: false, resolved: None });
                }
            }
        }
        if let Some((rva, _)) = self.dir(13) {
            for k in 0..MAX_DESCRIPTORS {
                // IMAGE_DELAYLOAD_DESCRIPTOR: Attributes, DllNameRVA, ModuleHandleRVA, ImportAddressTableRVA,
                // ImportNameTableRVA, BoundImportAddressTableRVA, UnloadInformationTableRVA, TimeDateStamp.
                let d = rva.checked_add((k * 32) as u32).ok_or("RVA dei descrittori ritardati")?;
                let (attrs, name, int) = (self.u32_rva(d)?, self.u32_rva(d.wrapping_add(4))?, self.u32_rva(d.wrapping_add(16))?);
                if name == 0 {
                    break;
                }
                if k + 1 == MAX_DESCRIPTORS {
                    return Err("troppi descrittori ritardati".into());
                }
                // Senza il bit 0 degli attributi (formato di Visual C++ 6) i campi sono indirizzi virtuali.
                let fix = |v: u32| -> Result<u32, String> {
                    if attrs & 1 != 0 {
                        Ok(v)
                    } else {
                        u64::from(v).checked_sub(self.image_base).and_then(|r| u32::try_from(r).ok()).ok_or_else(|| format!("VA {v:#x} sotto l'ImageBase"))
                    }
                };
                let dll = self.cstr(fix(name)?)?;
                for symbol in self.thunks(fix(int)?)? {
                    out.push(Import { dll: dll.clone(), symbol, delayed: true, resolved: None });
                }
            }
        }
        Ok(out)
    }

    /// Esportazioni con un nome, per ordinale (directory 0).
    pub fn named_exports(&self) -> Result<BTreeMap<u16, String>, String> {
        let mut out = BTreeMap::new();
        let Some((rva, _)) = self.dir(0) else { return Ok(out) };
        let base = self.u32_rva(rva.wrapping_add(16))?;
        let n_names = (self.u32_rva(rva.wrapping_add(24))? as usize).min(MAX_THUNKS);
        let (names, ordinals) = (self.u32_rva(rva.wrapping_add(32))?, self.u32_rva(rva.wrapping_add(36))?);
        for i in 0..n_names {
            let name = self.cstr(self.u32_rva(names.wrapping_add((i * 4) as u32))?)?;
            let index = self.u16_rva(ordinals.wrapping_add((i * 2) as u32))?;
            if let Some(ordinal) = base.checked_add(u32::from(index)).and_then(|o| u16::try_from(o).ok()) {
                out.insert(ordinal, name);
            }
        }
        Ok(out)
    }
}

fn u16_at(b: &[u8], o: usize) -> Result<u16, String> {
    b.get(o..o.wrapping_add(2)).filter(|s| s.len() == 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| format!("lettura oltre la fine a {o:#x}"))
}

fn u32_at(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o.wrapping_add(4)).filter(|s| s.len() == 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| format!("lettura oltre la fine a {o:#x}"))
}

fn u64_at(b: &[u8], o: usize) -> Result<u64, String> {
    let s = b.get(o..o.wrapping_add(8)).filter(|s| s.len() == 8).ok_or_else(|| format!("lettura oltre la fine a {o:#x}"))?;
    let mut a = [0u8; 8];
    a.copy_from_slice(s);
    Ok(u64::from_le_bytes(a))
}

/// Importazioni dell'eseguibile, con gli ordinali risolti dove la DLL di System32 li esporta con un nome.
pub fn read_imports(exe: &Path) -> Result<Vec<Import>, String> {
    let bytes = std::fs::read(exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    let mut imports = Pe::parse(&bytes)?.imports()?;
    let system32 = PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32");
    let mut cache: BTreeMap<String, BTreeMap<u16, String>> = BTreeMap::new();
    for i in &mut imports {
        let Symbol::Ordinal(o) = i.symbol else { continue };
        let dll = i.dll.to_ascii_lowercase();
        if dll.contains(['\\', '/']) {
            continue;
        }
        let names = cache.entry(dll.clone()).or_insert_with(|| {
            std::fs::read(system32.join(&dll)).ok().and_then(|b| Pe::parse(&b).ok().and_then(|p| p.named_exports().ok())).unwrap_or_default()
        });
        i.resolved = names.get(&o).cloned();
    }
    Ok(imports)
}

fn matches(pattern: &str, name: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == pattern,
    }
}

/// Motivo per cui un'importazione è vietata, se lo è.
pub fn verdict(i: &Import) -> Option<&'static str> {
    let dll = i.dll.to_ascii_lowercase();
    if NET_DLLS.contains(&dll.as_str()) {
        return Some("libreria di rete");
    }
    let name = i.name();
    if dll == "ws2_32.dll" {
        return match name {
            None => Some("ordinale di ws2_32 non risolto"),
            Some(n) if WS2_FORBIDDEN.iter().any(|p| matches(p, n)) => Some("connessione o risoluzione dei nomi"),
            Some(_) => None,
        };
    }
    match name {
        Some(n) if n.starts_with("MsiOpenPackage") => Some("MSI aperto con lo stato della macchina"),
        Some(n) if n.starts_with("CreateProcess") || n == "WinExec" => Some("creazione di processi"),
        _ => None,
    }
}

/// Esito del controllo di un eseguibile.
pub struct Report {
    pub exe: PathBuf,
    pub imports: Vec<Import>,
    pub violations: Vec<(Import, &'static str)>,
    pub dependent_load_flags: Result<u16, String>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.violations.is_empty() && self.dependent_load_flags == Ok(0x800)
    }
}

pub fn check(exe: &Path) -> Result<Report, String> {
    let imports = read_imports(exe)?;
    let violations = imports.iter().filter_map(|i| verdict(i).map(|v| (i.clone(), v))).collect();
    Ok(Report { exe: exe.to_path_buf(), imports, violations, dependent_load_flags: crate::dll_cmd::dependent_load_flags(exe) })
}

/// Tabella per DLL, funzioni di `ws2_32`, funzioni da notare, violazioni ed esito.
pub fn print(r: &Report) {
    println!("Importazioni di {}\n", r.exe.display());
    let mut by_dll: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for i in &r.imports {
        let e = by_dll.entry(i.dll.to_ascii_lowercase()).or_default();
        if i.delayed { e.1 += 1 } else { e.0 += 1 }
        e.2 += usize::from(verdict(i).is_some());
    }
    println!("| DLL | Normali | Ritardate | Esito |\n|---|---|---|---|");
    for (dll, (normal, delayed, bad)) in &by_dll {
        let esito = if *bad > 0 { format!("**{bad} vietate**") } else { "ok".to_owned() };
        println!("| {dll} | {normal} | {delayed} | {esito} |");
    }
    let total_delayed = r.imports.iter().filter(|i| i.delayed).count();
    println!("\n{} DLL, {} funzioni ({} ritardate).", by_dll.len(), r.imports.len(), total_delayed);
    let ws2: Vec<String> = r.imports.iter().filter(|i| i.dll.eq_ignore_ascii_case("ws2_32.dll")).map(Import::label).collect();
    if ws2.is_empty() {
        println!("Funzioni di ws2_32.dll: nessuna.");
    } else {
        println!("Funzioni di ws2_32.dll: {}.", ws2.join(", "));
    }
    let named = |pred: &dyn Fn(&str) -> bool| -> Vec<String> {
        let mut v: Vec<String> = r.imports.iter().filter(|i| i.name().is_some_and(pred)).map(|i| format!("{} ({})", i.label(), i.dll.to_ascii_lowercase())).collect();
        v.dedup();
        v
    };
    let shell = named(&|n| n.starts_with("ShellExecute"));
    println!("Avvio con la shell: {} (atteso ShellExecuteExW: helper e lanciatore degli URI tipizzati).", if shell.is_empty() { "nessuno".to_owned() } else { shell.join(", ") });
    let dynamic = named(&|n| n.starts_with("LoadLibrary") || n == "GetProcAddress");
    println!("Caricamento dinamico: {} (fuori dalla tabella delle importazioni).", if dynamic.is_empty() { "nessuno".to_owned() } else { dynamic.join(", ") });
    match &r.dependent_load_flags {
        Ok(f) => println!("DependentLoadFlags: {f:#06x} ({}).", if *f == 0x800 { "ok" } else { "**atteso 0x0800**" }),
        Err(e) => println!("DependentLoadFlags: non letto ({e})."),
    }
    if r.violations.is_empty() {
        println!("Violazioni: nessuna.");
    } else {
        println!("Violazioni:");
        for (i, why) in &r.violations {
            println!("- {}!{}{}: {why}", i.dll, i.label(), if i.delayed { " (ritardata)" } else { "" });
        }
    }
    println!("Esito: {}", if r.passed() { "superato" } else { "NON superato" });
}

/// Controllo per `installa`: stampa e restituisce un errore se il controllo non passa.
pub fn verify(exe: &Path) -> Result<(), String> {
    let r = check(exe)?;
    print(&r);
    if r.passed() { Ok(()) } else { Err(format!("controllo delle importazioni non superato: {}", exe.display())) }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let exe = match args {
        [] => PathBuf::from(DEFAULT_EXE),
        [one] => PathBuf::from(one),
        _ => return Err("uso: cargo xtask importazioni [EXE]".into()),
    };
    verify(&exe)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Costruttore di un PE minimo con una sezione: intestazioni, poi i dati a partire da `SECTION_RVA`.
    struct Builder {
        pe64: bool,
        data: Vec<u8>,
    }

    const SECTION_RVA: u32 = 0x1000;
    const SECTION_RAW: u32 = 0x200;
    const IMAGE_BASE: u64 = 0x40_0000;

    impl Builder {
        fn new(pe64: bool) -> Self {
            Self { pe64, data: Vec::new() }
        }

        /// RVA del prossimo byte scritto.
        fn rva(&self) -> u32 {
            SECTION_RVA + self.data.len() as u32
        }

        fn put(&mut self, bytes: &[u8]) -> u32 {
            let at = self.rva();
            self.data.extend_from_slice(bytes);
            at
        }

        fn u32(&mut self, v: u32) -> u32 {
            self.put(&v.to_le_bytes())
        }

        fn cstr(&mut self, s: &str) -> u32 {
            let at = self.put(s.as_bytes());
            self.put(&[0]);
            at
        }

        fn hint_name(&mut self, s: &str) -> u32 {
            let at = self.put(&[0, 0]);
            self.cstr(s);
            at
        }

        /// Tabella di thunk: nomi (`Ok`) e ordinali (`Err`), con lo zero finale.
        fn thunks(&mut self, items: &[Result<&str, u16>]) -> u32 {
            let values: Vec<u64> = items
                .iter()
                .map(|it| match it {
                    Ok(n) => u64::from(self.hint_name(n)),
                    Err(o) => u64::from(*o) | if self.pe64 { 1 << 63 } else { 1 << 31 },
                })
                .collect();
            let at = self.rva();
            for v in values.into_iter().chain([0]) {
                if self.pe64 { self.put(&v.to_le_bytes()) } else { self.put(&(v as u32).to_le_bytes()) };
            }
            at
        }

        /// Il file: intestazioni con le directory 1 e 13, una sezione con i dati.
        fn finish(&self, imports: u32, delay: u32) -> Vec<u8> {
            let mut b = vec![0u8; SECTION_RAW as usize];
            b[0..2].copy_from_slice(b"MZ");
            b[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
            b[0x80..0x84].copy_from_slice(b"PE\0\0");
            let coff = 0x84;
            let opt_size: u16 = if self.pe64 { 240 } else { 224 };
            b[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
            b[coff + 16..coff + 18].copy_from_slice(&opt_size.to_le_bytes());
            let opt = coff + 20;
            let (magic, dirs_at, n_at): (u16, usize, usize) = if self.pe64 { (0x20b, opt + 112, opt + 108) } else { (0x10b, opt + 96, opt + 92) };
            b[opt..opt + 2].copy_from_slice(&magic.to_le_bytes());
            if self.pe64 {
                b[opt + 24..opt + 32].copy_from_slice(&IMAGE_BASE.to_le_bytes());
            } else {
                b[opt + 28..opt + 32].copy_from_slice(&(IMAGE_BASE as u32).to_le_bytes());
            }
            b[opt + 60..opt + 64].copy_from_slice(&SECTION_RAW.to_le_bytes());
            b[n_at..n_at + 4].copy_from_slice(&16u32.to_le_bytes());
            b[dirs_at + 8..dirs_at + 12].copy_from_slice(&imports.to_le_bytes());
            b[dirs_at + 13 * 8..dirs_at + 13 * 8 + 4].copy_from_slice(&delay.to_le_bytes());
            let s = opt + opt_size as usize;
            b[s..s + 6].copy_from_slice(b".idata");
            let len = self.data.len() as u32;
            for (k, v) in [len, SECTION_RVA, len, SECTION_RAW].into_iter().enumerate() {
                b[s + 8 + k * 4..s + 12 + k * 4].copy_from_slice(&v.to_le_bytes());
            }
            b.extend_from_slice(&self.data);
            b
        }
    }

    /// PE con KERNEL32 (CreateProcessW, GetTickCount), WS2_32 (ordinale 116 e connect) e, ritardata, urlmon
    /// (URLDownloadToFileW). `rva_based`: formato dei descrittori ritardati (bit 0 degli attributi).
    fn sample(pe64: bool, rva_based: bool) -> Vec<u8> {
        let mut b = Builder::new(pe64);
        let k32 = b.cstr("KERNEL32.dll");
        let k32_t = b.thunks(&[Ok("CreateProcessW"), Ok("GetTickCount")]);
        let ws2 = b.cstr("WS2_32.dll");
        let ws2_t = b.thunks(&[Err(116), Ok("connect")]);
        let url = b.cstr("urlmon.dll");
        let url_t = b.thunks(&[Ok("URLDownloadToFileW")]);
        while !b.data.len().is_multiple_of(4) {
            b.put(&[0]);
        }
        let imports = b.rva();
        for (name, table) in [(k32, k32_t), (ws2, ws2_t)] {
            for v in [table, 0, 0, name, table] {
                b.u32(v);
            }
        }
        b.put(&[0; 20]);
        let delay = b.rva();
        let va = |r: u32| if rva_based { r } else { (IMAGE_BASE as u32) + r };
        for v in [u32::from(rva_based), va(url), 0, va(url_t), va(url_t), 0, 0, 0] {
            b.u32(v);
        }
        b.put(&[0; 32]);
        b.finish(imports, delay)
    }

    fn names(imports: &[Import]) -> Vec<String> {
        imports.iter().map(|i| format!("{}!{}{}", i.dll, i.symbol, if i.delayed { " (r)" } else { "" })).collect()
    }

    #[test]
    fn the_parser_reads_normal_and_delayed_imports_in_both_formats() {
        let expected = ["KERNEL32.dll!CreateProcessW", "KERNEL32.dll!GetTickCount", "WS2_32.dll!#116", "WS2_32.dll!connect", "urlmon.dll!URLDownloadToFileW (r)"];
        for (pe64, rva_based) in [(true, true), (false, true), (false, false)] {
            let bytes = sample(pe64, rva_based);
            let pe = Pe::parse(&bytes).unwrap();
            assert_eq!(pe.pe64, pe64);
            assert_eq!(names(&pe.imports().unwrap()), expected, "pe64 {pe64}, rva {rva_based}");
        }
    }

    #[test]
    fn the_rules_flag_processes_network_and_unresolved_ws2_ordinals() {
        let bytes = sample(true, true);
        let mut imports = Pe::parse(&bytes).unwrap().imports().unwrap();
        let bad: Vec<(String, &str)> = imports.iter().filter_map(|i| verdict(i).map(|v| (i.symbol.to_string(), v))).collect();
        assert_eq!(
            bad,
            [
                ("CreateProcessW".to_owned(), "creazione di processi"),
                ("#116".to_owned(), "ordinale di ws2_32 non risolto"),
                ("connect".to_owned(), "connessione o risoluzione dei nomi"),
                ("URLDownloadToFileW".to_owned(), "libreria di rete"),
            ]
        );
        // L'ordinale 116 di ws2_32 è WSACleanup: risolto, è ammesso.
        imports[2].resolved = Some("WSACleanup".into());
        assert_eq!(verdict(&imports[2]), None);
        let ws2 = |n: &str| Import { dll: "ws2_32.dll".into(), symbol: Symbol::Name(n.into()), delayed: false, resolved: None };
        for n in ["WSAConnectByNameW", "GetAddrInfoExW", "getaddrinfo", "gethostbyname", "WSAConnect"] {
            assert!(verdict(&ws2(n)).is_some(), "{n}");
        }
        for n in ["WSAStartup", "WSACleanup", "closesocket", "connectx"] {
            assert_eq!(verdict(&ws2(n)), None, "{n}");
        }
        let msi = Import { dll: "msi.dll".into(), symbol: Symbol::Name("MsiOpenPackageExW".into()), delayed: true, resolved: None };
        assert!(verdict(&msi).is_some());
    }

    #[test]
    fn a_truncated_or_corrupted_file_gives_an_error_not_a_panic() {
        let bytes = sample(true, true);
        for n in 0..bytes.len() {
            if let Ok(pe) = Pe::parse(&bytes[..n]) {
                let _ = pe.imports();
            }
        }
        // Byte per byte a 0xff: nessun panico, nessun ciclo senza fine.
        for k in 0..bytes.len() {
            let mut b = bytes.clone();
            b[k] = 0xff;
            if let Ok(pe) = Pe::parse(&b) {
                let _ = pe.imports();
                let _ = pe.named_exports();
            }
        }
        assert!(Pe::parse(b"MZ").is_err());
        assert!(Pe::parse(&[]).is_err());
    }

    fn system32(name: &str) -> PathBuf {
        PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())).join("System32").join(name)
    }

    #[test]
    fn a_known_system_executable() {
        // cmd.exe crea processi: il controllo lo vede. Solo lettura del file.
        let cmd = system32("cmd.exe");
        let imports = read_imports(&cmd).unwrap();
        assert!(imports.iter().any(|i| i.name() == Some("CreateProcessW")), "{:?}", names(&imports));
        assert!(imports.iter().any(|i| verdict(i) == Some("creazione di processi")));
        // Le esportazioni di ws2_32: gli ordinali storici di connect (4) e WSACleanup (116).
        let ws2 = std::fs::read(system32("ws2_32.dll")).unwrap();
        let exports = Pe::parse(&ws2).unwrap().named_exports().unwrap();
        assert_eq!(exports.get(&4).map(String::as_str), Some("connect"));
        assert_eq!(exports.get(&116).map(String::as_str), Some("WSACleanup"));
    }

    #[test]
    fn this_test_executable_creates_processes_and_the_check_sees_it() {
        // xtask usa std::process::Command: la tabella del suo eseguibile di test contiene CreateProcessW.
        let exe = std::env::current_exe().unwrap();
        let imports = read_imports(&exe).unwrap();
        assert!(imports.iter().any(|i| i.name() == Some("CreateProcessW")), "{:?}", names(&imports));
    }

    #[test]
    fn quado_has_no_forbidden_import() {
        // L'eseguibile di Quado, se è già compilato in release (`cargo xtask importazioni` lo compila).
        let exe = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(DEFAULT_EXE);
        if !exe.is_file() {
            eprintln!("{} assente: controllo saltato", exe.display());
            return;
        }
        let r = check(&exe).unwrap();
        assert!(r.imports.len() > 50, "poche importazioni: {}", r.imports.len());
        assert!(r.imports.iter().any(|i| i.name() == Some("ShellExecuteExW")), "manca ShellExecuteExW");
        let bad: Vec<String> = r.violations.iter().map(|(i, why)| format!("{}!{}: {why}", i.dll, i.symbol)).collect();
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(r.dependent_load_flags, Ok(0x800));
    }
}
