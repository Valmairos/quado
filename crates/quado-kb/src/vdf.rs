// SPDX-License-Identifier: GPL-3.0-or-later
//! Lettore del formato KeyValues di Steam (`libraryfolders.vdf`, `appmanifest_*.acf`,
//! `localconfig.vdf`). Formato non documentato (05 D6 [F]): ingresso non fidato, con limiti di
//! profondità e di dimensione, mai un panic (06 §2.3).

use crate::extract::FormatIssue;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Vdf {
    Str(String),
    Obj(Vec<(String, Vdf)>),
}

impl Vdf {
    /// Primo figlio con questa chiave, senza distinzione di maiuscole.
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Self::Obj(v) => v.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, x)| x),
            Self::Str(_) => None,
        }
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Self::Str(s) => Some(s),
            Self::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Vdf)] {
        match self {
            Self::Obj(v) => v,
            Self::Str(_) => &[],
        }
    }

    /// Percorso di chiavi.
    pub fn path(&self, keys: &[&str]) -> Option<&Vdf> {
        keys.iter().try_fold(self, |v, k| v.get(k))
    }
}

const MAX_DEPTH: usize = 64;

enum Tok {
    Str(String),
    Open,
    Close,
}

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lexer<'_> {
    fn next(&mut self) -> Result<Option<Tok>, FormatIssue> {
        loop {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"//") {
                while self.i < self.s.len() && self.s[self.i] != b'\n' {
                    self.i += 1;
                }
                continue;
            }
            break;
        }
        let Some(&c) = self.s.get(self.i) else { return Ok(None) };
        match c {
            b'{' => {
                self.i += 1;
                Ok(Some(Tok::Open))
            }
            b'}' => {
                self.i += 1;
                Ok(Some(Tok::Close))
            }
            b'"' => {
                self.i += 1;
                let mut out = Vec::new();
                loop {
                    let Some(&b) = self.s.get(self.i) else { return Err(FormatIssue::Truncated) };
                    self.i += 1;
                    match b {
                        b'"' => break,
                        b'\\' => {
                            let Some(&e) = self.s.get(self.i) else { return Err(FormatIssue::Truncated) };
                            self.i += 1;
                            out.push(match e {
                                b'n' => b'\n',
                                b't' => b'\t',
                                other => other,
                            });
                        }
                        _ => out.push(b),
                    }
                }
                String::from_utf8(out).map(|s| Some(Tok::Str(s))).map_err(|_| FormatIssue::Encoding)
            }
            _ => {
                // Token senza virgolette: fino a spazio o graffa.
                let start = self.i;
                while self.i < self.s.len() && !self.s[self.i].is_ascii_whitespace() && !matches!(self.s[self.i], b'{' | b'}' | b'"') {
                    self.i += 1;
                }
                std::str::from_utf8(&self.s[start..self.i]).map(|s| Some(Tok::Str(s.to_owned()))).map_err(|_| FormatIssue::Encoding)
            }
        }
    }
}

fn object(lx: &mut Lexer<'_>, depth: usize, top: bool) -> Result<Vec<(String, Vdf)>, FormatIssue> {
    if depth > MAX_DEPTH {
        return Err(FormatIssue::TooDeep);
    }
    let mut out = Vec::new();
    loop {
        let key = match lx.next()? {
            None if top => return Ok(out),
            None => return Err(FormatIssue::Truncated),
            Some(Tok::Close) if !top => return Ok(out),
            Some(Tok::Close | Tok::Open) => return Err(FormatIssue::Unexpected),
            Some(Tok::Str(k)) => k,
        };
        match lx.next()? {
            None => return Err(FormatIssue::Truncated),
            Some(Tok::Str(v)) => out.push((key, Vdf::Str(v))),
            Some(Tok::Open) => out.push((key, Vdf::Obj(object(lx, depth + 1, false)?))),
            Some(Tok::Close) => return Err(FormatIssue::Unexpected),
        }
    }
}

/// Legge un documento VDF: un oggetto anonimo con le chiavi di primo livello.
pub fn parse(text: &str) -> Result<Vdf, FormatIssue> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lx = Lexer { s: text.as_bytes(), i: 0 };
    let top = object(&mut lx, 0, true)?;
    if top.is_empty() {
        return Err(FormatIssue::Empty);
    }
    Ok(Vdf::Obj(top))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_objects_and_escapes() {
        let v = parse("\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"apps\" { \"3999910\" \"123\" }\n\t}\n}\n").expect("vdf");
        let lib = v.path(&["libraryfolders", "0"]).expect("libreria");
        assert_eq!(lib.str("path"), Some(r"C:\Program Files (x86)\Steam"));
        assert_eq!(lib.path(&["apps"]).expect("apps").entries().len(), 1);
    }

    #[test]
    fn truncated_and_unbalanced_are_errors() {
        assert_eq!(parse("\"a\" { \"b\" \"c\""), Err(FormatIssue::Truncated));
        assert_eq!(parse("\"a\" { \"b\" \"c"), Err(FormatIssue::Truncated));
        assert_eq!(parse("\"a\" }"), Err(FormatIssue::Unexpected));
        assert_eq!(parse(""), Err(FormatIssue::Empty));
        let deep = "\"a\" {".repeat(100);
        assert_eq!(parse(&deep), Err(FormatIssue::TooDeep));
    }
}
