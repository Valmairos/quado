// SPDX-License-Identifier: GPL-3.0-or-later
//! Sorgente astratta del registro (06 §2.3): gli estrattori leggono da qui. In uso c'è il registro
//! vero, in sola lettura (`win`); nei test un file nel formato di `reg export` (fixture
//! sintetiche), così i test non dipendono dal registro del PC su cui girano.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Root {
    LocalMachine,
    CurrentUser,
}

impl Root {
    fn prefix(self) -> &'static str {
        match self {
            Self::LocalMachine => "HKEY_LOCAL_MACHINE",
            Self::CurrentUser => "HKEY_CURRENT_USER",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegValue {
    Sz(String),
    ExpandSz(String),
    MultiSz(Vec<String>),
    Dword(u32),
    Qword(u64),
    Binary(Vec<u8>),
    /// Tipo non gestito: si conserva il codice, mai il valore ricostruito.
    Other(u32),
}

impl RegValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Sz(s) | Self::ExpandSz(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        match self {
            Self::Dword(v) => Some(*v),
            _ => None,
        }
    }
}

/// Lettura in sola lettura di chiavi e valori. Gli errori sono codici di Windows (2 = non trovata,
/// 5 = accesso negato).
pub trait RegistrySource: Sync {
    fn subkeys(&self, root: Root, path: &str) -> Result<Vec<String>, u32>;
    fn values(&self, root: Root, path: &str) -> Result<Vec<(String, RegValue)>, u32>;
    fn value(&self, root: Root, path: &str, name: &str) -> Result<Option<RegValue>, u32> {
        Ok(self.values(root, path)?.into_iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v))
    }
}

/// Errore nel file di fixture: riga (da 1) e motivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegFileError {
    pub line: usize,
    pub truncated: bool,
}

/// Registro letto da un file `.reg` (testo UTF-8, senza BOM o con BOM).
#[derive(Clone, Debug, Default)]
pub struct RegFile {
    /// Percorso completo in minuscolo -> (nome originale dell'ultimo segmento, valori).
    keys: BTreeMap<String, (String, Vec<(String, RegValue)>)>,
}

fn unquote(s: &str) -> Option<(String, &str)> {
    let s = s.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some((_, e)) => out.push(e),
                None => return None,
            },
            '"' => return Some((out, &s[i + 1..])),
            _ => out.push(c),
        }
    }
    None
}

fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if s.is_empty() {
        return Some(Vec::new());
    }
    s.split(',').map(|b| u8::from_str_radix(b.trim(), 16).ok()).collect()
}

fn utf16(bytes: &[u8]) -> Option<Vec<u16>> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    Some(bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect())
}

fn parse_data(d: &str) -> Option<RegValue> {
    if d.starts_with('"') {
        let (s, rest) = unquote(d)?;
        return rest.trim().is_empty().then_some(RegValue::Sz(s));
    }
    if let Some(h) = d.strip_prefix("dword:") {
        return u32::from_str_radix(h.trim(), 16).ok().map(RegValue::Dword);
    }
    if let Some(h) = d.strip_prefix("hex(b):") {
        let b = hex_bytes(h)?;
        let b: [u8; 8] = b.try_into().ok()?;
        return Some(RegValue::Qword(u64::from_le_bytes(b)));
    }
    if let Some(h) = d.strip_prefix("hex(2):") {
        let w = utf16(&hex_bytes(h)?)?;
        let end = w.iter().position(|&c| c == 0).unwrap_or(w.len());
        return Some(RegValue::ExpandSz(String::from_utf16_lossy(&w[..end])));
    }
    if let Some(h) = d.strip_prefix("hex(7):") {
        let w = utf16(&hex_bytes(h)?)?;
        let parts = w.split(|&c| c == 0).filter(|p| !p.is_empty()).map(String::from_utf16_lossy).collect();
        return Some(RegValue::MultiSz(parts));
    }
    if let Some(h) = d.strip_prefix("hex:") {
        return hex_bytes(h).map(RegValue::Binary);
    }
    if let Some(rest) = d.strip_prefix("hex(") {
        let (code, data) = rest.split_once(')')?;
        hex_bytes(data.strip_prefix(':')?)?;
        return u32::from_str_radix(code, 16).ok().map(RegValue::Other);
    }
    None
}

impl RegFile {
    /// Legge un file nel formato di `reg export`. Una riga che non si capisce è un errore con il suo
    /// numero; un file che finisce a metà (riga continuata o stringa aperta) è "troncato".
    pub fn parse(text: &str) -> Result<Self, RegFileError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut out = Self::default();
        let mut current: Option<String> = None;
        let mut lines = text.lines().enumerate().peekable();
        let mut seen_header = false;
        while let Some((n, raw)) = lines.next() {
            let line_no = n + 1;
            // Righe continuate con "\" a fine riga (valori esadecimali lunghi).
            let mut line = raw.trim_end().to_owned();
            while line.ends_with('\\') {
                line.pop();
                match lines.next() {
                    Some((_, next)) => line.push_str(next.trim()),
                    None => return Err(RegFileError { line: line_no, truncated: true }),
                }
            }
            let t = line.trim();
            if t.is_empty() || t.starts_with(';') {
                continue;
            }
            if !seen_header {
                if t == "Windows Registry Editor Version 5.00" {
                    seen_header = true;
                    continue;
                }
                return Err(RegFileError { line: line_no, truncated: false });
            }
            if let Some(k) = t.strip_prefix('[') {
                let Some(k) = k.strip_suffix(']') else { return Err(RegFileError { line: line_no, truncated: true }) };
                let name = k.rsplit('\\').next().unwrap_or(k).to_owned();
                let lower = k.to_ascii_lowercase();
                out.keys.entry(lower.clone()).or_insert_with(|| (name, Vec::new()));
                // Anche le chiavi intermedie esistono.
                let mut p = lower.as_str();
                while let Some((parent, _)) = p.rsplit_once('\\') {
                    let orig = &k[..parent.len()];
                    let pname = orig.rsplit('\\').next().unwrap_or(orig).to_owned();
                    out.keys.entry(parent.to_owned()).or_insert_with(|| (pname, Vec::new()));
                    p = parent;
                }
                current = Some(lower);
                continue;
            }
            let Some(key) = current.as_ref() else { return Err(RegFileError { line: line_no, truncated: false }) };
            let (name, rest) = if let Some(r) = t.strip_prefix('@') {
                (String::new(), r)
            } else {
                match unquote(t) {
                    Some(x) => x,
                    None => return Err(RegFileError { line: line_no, truncated: true }),
                }
            };
            let Some(data) = rest.trim_start().strip_prefix('=') else { return Err(RegFileError { line: line_no, truncated: false }) };
            let data = data.trim();
            let Some(v) = parse_data(data) else {
                let truncated = data.starts_with('"') && unquote(data).is_none();
                return Err(RegFileError { line: line_no, truncated });
            };
            if let Some(e) = out.keys.get_mut(key) {
                e.1.push((name, v));
            }
        }
        if !seen_header {
            return Err(RegFileError { line: 1, truncated: true });
        }
        Ok(out)
    }

    fn full(root: Root, path: &str) -> String {
        format!("{}\\{}", root.prefix(), path).to_ascii_lowercase()
    }
}

impl RegistrySource for RegFile {
    fn subkeys(&self, root: Root, path: &str) -> Result<Vec<String>, u32> {
        let base = Self::full(root, path);
        if !self.keys.contains_key(&base) {
            return Err(2);
        }
        let prefix = format!("{base}\\");
        Ok(self
            .keys
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .filter(|(k, _)| !k[prefix.len()..].contains('\\'))
            .map(|(_, (name, _))| name.clone())
            .collect())
    }

    fn values(&self, root: Root, path: &str) -> Result<Vec<(String, RegValue)>, u32> {
        self.keys.get(&Self::full(root, path)).map(|(_, v)| v.clone()).ok_or(2)
    }
}

/// Più sorgenti insieme (per esempio HKLM e HKCU da due file di fixture).
pub struct Layered<'a>(pub Vec<&'a dyn RegistrySource>);

impl RegistrySource for Layered<'_> {
    fn subkeys(&self, root: Root, path: &str) -> Result<Vec<String>, u32> {
        let mut last = 2;
        let mut out: Vec<String> = Vec::new();
        let mut found = false;
        for s in &self.0 {
            match s.subkeys(root, path) {
                Ok(v) => {
                    found = true;
                    out.extend(v);
                }
                Err(e) => last = e,
            }
        }
        if found { Ok(out) } else { Err(last) }
    }

    fn values(&self, root: Root, path: &str) -> Result<Vec<(String, RegValue)>, u32> {
        let mut last = 2;
        for s in &self.0 {
            match s.values(root, path) {
                Ok(v) => return Ok(v),
                Err(e) => last = e,
            }
        }
        Err(last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Windows Registry Editor Version 5.00\r\n\r\n[HKEY_CURRENT_USER\\Software\\Prova\\Uno]\r\n\"Nome\"=\"CapCut\"\r\n\"Percorso\"=\"C:\\\\Users\\\\demo\"\r\n\"Numero\"=dword:0000001e\r\n\"Grande\"=hex(b):01,00,00,00,00,00,00,00\r\n\"Esp\"=hex(2):25,00,41,00,00,00\r\n\"Bin\"=hex:01,02,\\\r\n  03\r\n@=\"predefinito\"\r\n";

    #[test]
    fn reads_the_reg_export_format() {
        let f = RegFile::parse(SAMPLE).expect("formato");
        let v = f.values(Root::CurrentUser, r"Software\Prova\Uno").expect("chiave");
        assert_eq!(v[0], ("Nome".into(), RegValue::Sz("CapCut".into())));
        assert_eq!(v[1].1, RegValue::Sz(r"C:\Users\demo".into()));
        assert_eq!(v[2].1, RegValue::Dword(30));
        assert_eq!(v[3].1, RegValue::Qword(1));
        assert_eq!(v[4].1, RegValue::ExpandSz("%A".into()));
        assert_eq!(v[5].1, RegValue::Binary(vec![1, 2, 3]));
        assert_eq!(v[6], (String::new(), RegValue::Sz("predefinito".into())));
        assert_eq!(f.subkeys(Root::CurrentUser, r"Software\Prova").expect("padre"), vec!["Uno".to_owned()]);
        assert_eq!(f.values(Root::LocalMachine, r"Software\Prova\Uno"), Err(2));
    }

    #[test]
    fn truncated_and_changed_files_are_errors() {
        let cut = &SAMPLE[..SAMPLE.find("\"Percorso\"").expect("punto") + 18];
        assert!(RegFile::parse(cut).is_err());
        assert_eq!(RegFile::parse("[HKEY_CURRENT_USER\\X]\r\n").map(|_| ()), Err(RegFileError { line: 1, truncated: false }));
        let open = "Windows Registry Editor Version 5.00\r\n[HKEY_CURRENT_USER\\X]\r\n\"Nome\"=\"senza fine\r\n";
        assert_eq!(RegFile::parse(open).map(|_| ()), Err(RegFileError { line: 3, truncated: true }));
    }
}
