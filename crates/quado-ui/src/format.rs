// SPDX-License-Identifier: GPL-3.0-or-later
//! Numeri, dimensioni e date (03 F-GE-3, 05 D13).
//!
//! - Separatori e data breve dal formato dell'area geografica di Windows, come Esplora file,
//!   indipendenti dalla lingua dell'interfaccia.
//! - Dimensioni come Esplora sul singolo file: 1 GB = 1024³ byte, tre cifre significative troncate;
//!   in alternativa SI (1 GB = 10⁹ byte), scelto in Impostazioni.
//! - Le etichette delle unità vengono dai file di lingua.

use quado_scan_win::system::{self, RegionFormat};

use crate::i18n::Lingue;

/// Sistema delle unità.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Units {
    /// Come Esplora file: 1 KB = 1024 byte.
    #[default]
    Explorer,
    /// SI: 1 kB = 1000 byte.
    Si,
}

impl Units {
    pub fn code(self) -> &'static str {
        match self {
            Self::Explorer => "esplora",
            Self::Si => "si",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        match s {
            "esplora" => Some(Self::Explorer),
            "si" => Some(Self::Si),
            _ => None,
        }
    }

    fn base(self) -> f64 {
        match self {
            Self::Explorer => 1024.0,
            Self::Si => 1000.0,
        }
    }
}

/// Etichette delle unità nella lingua corrente.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitLabels {
    pub byte_one: String,
    pub byte_many: String,
    /// KB, MB, GB, TB, PB per Esplora; kB, MB, GB, TB, PB per SI.
    pub explorer: [String; 5],
    pub si: [String; 5],
    /// Simbolo dei secondi.
    pub seconds: String,
}

impl UnitLabels {
    pub fn from(l: &Lingue) -> Self {
        let k = |key: &str| l.t(key);
        Self {
            byte_one: l.ta("unita-byte", &[("n", 1u64.into())]),
            byte_many: l.ta("unita-byte", &[("n", 2u64.into())]),
            explorer: [k("unita-kb"), k("unita-mb"), k("unita-gb"), k("unita-tb"), k("unita-pb")],
            si: [k("unita-si-kb"), k("unita-mb"), k("unita-gb"), k("unita-tb"), k("unita-pb")],
            seconds: k("unita-secondi"),
        }
    }
}

impl Default for UnitLabels {
    fn default() -> Self {
        let v = |a: [&str; 5]| a.map(str::to_owned);
        Self {
            byte_one: "byte".into(),
            byte_many: "byte".into(),
            explorer: v(["KB", "MB", "GB", "TB", "PB"]),
            si: v(["kB", "MB", "GB", "TB", "PB"]),
            seconds: "s".into(),
        }
    }
}

/// Tutto ciò che serve a scrivere un numero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formatter {
    pub region: RegionFormat,
    pub units: Units,
    pub labels: UnitLabels,
}

impl Default for Formatter {
    fn default() -> Self {
        Self { region: RegionFormat::default(), units: Units::Explorer, labels: UnitLabels::default() }
    }
}

impl Formatter {
    /// Dimensione leggibile: `0 byte`, `512 byte`, `1,00 KB`, `13,7 GB`, `476 GB`.
    pub fn size(&self, bytes: u64) -> String {
        let base = self.units.base();
        if (bytes as f64) < base {
            let label = if bytes == 1 { &self.labels.byte_one } else { &self.labels.byte_many };
            return format!("{} {label}", self.count(bytes));
        }
        let labels = match self.units {
            Units::Explorer => &self.labels.explorer,
            Units::Si => &self.labels.si,
        };
        let mut value = bytes as f64 / base;
        let mut unit = 0;
        while value >= 1000.0 && unit + 1 < labels.len() {
            value /= base;
            unit += 1;
        }
        // Esplora file, dai GB in su, parte dai MB interi (scarta i byte sotto il MB) e poi tronca:
        // 14.806.917.939 byte sono "13,7 GB", non "13,8 GB" (confronto con `StrFormatByteSizeEx`).
        if self.units == Units::Explorer && unit >= 2 {
            value = (bytes >> 20) as f64 / base.powi(unit as i32 - 1);
        }
        format!("{} {}", self.significant(value), labels[unit])
    }

    /// Tre cifre significative al massimo, troncate come in Esplora: 1.048.575 byte sono "0,99 MB".
    fn significant(&self, value: f64) -> String {
        let decimals = if value >= 100.0 {
            0
        } else if value >= 10.0 {
            1
        } else {
            2
        };
        let factor = 10f64.powi(decimals);
        let truncated = (value * factor).floor() / factor;
        self.decimal(truncated, decimals as usize)
    }

    /// Numero con `decimals` cifre decimali e il separatore dell'area geografica.
    pub fn decimal(&self, value: f64, decimals: usize) -> String {
        let text = format!("{value:.decimals$}");
        let (int, frac) = text.split_once('.').map_or((text.as_str(), None), |(i, f)| (i, Some(f)));
        let (neg, int) = int.strip_prefix('-').map_or((false, int), |r| (true, r));
        let mut out = String::new();
        if neg {
            out.push('-');
        }
        out.push_str(&self.group(int));
        if let Some(f) = frac {
            out.push_str(&self.region.decimal);
            out.push_str(f);
        }
        out
    }

    /// Conteggio con il separatore delle migliaia: `1.734.052`.
    pub fn count(&self, n: u64) -> String {
        self.group(&n.to_string())
    }

    /// Raggruppa le cifre secondo `LOCALE_SGROUPING` (`3;0`, `3;2;0`, `0`).
    fn group(&self, digits: &str) -> String {
        let sizes: Vec<usize> = self.region.grouping.split(';').filter_map(|s| s.trim().parse().ok()).collect();
        let first = sizes.first().copied().unwrap_or(3);
        if first == 0 || self.region.thousands.is_empty() {
            return digits.to_owned();
        }
        // Dimensioni dei gruppi da destra: l'ultima diversa da zero si ripete se la lista finisce con 0.
        let repeat = sizes.last() == Some(&0);
        let sizes: Vec<usize> = sizes.into_iter().filter(|&s| s > 0).collect();
        let mut groups: Vec<&str> = Vec::new();
        let mut end = digits.len();
        let mut k = 0;
        while end > 0 {
            let size = match sizes.get(k) {
                Some(&s) => s,
                None if repeat => *sizes.last().unwrap_or(&3),
                None => end,
            };
            let start = end.saturating_sub(size);
            groups.push(&digits[start..end]);
            end = start;
            k += 1;
        }
        groups.reverse();
        groups.join(&self.region.thousands)
    }

    /// Percentuale con un decimale: `13,7%`.
    pub fn percent(&self, ratio: f64) -> String {
        format!("{}%", self.decimal(ratio * 100.0, 1))
    }

    /// Data breve dell'area geografica per un `FILETIME`, nell'ora locale di quel giorno.
    pub fn date(&self, filetime: i64) -> String {
        match system::local_time(filetime) {
            Some(t) => short_date(&self.region.short_date, t.year, t.month, t.day),
            None => String::new(),
        }
    }

    /// Data del calendario (data di verifica di una regola) nel formato dell'area geografica.
    pub fn day(&self, d: quado_model::record::Date) -> String {
        short_date(&self.region.short_date, d.year, u16::from(d.month), u16::from(d.day))
    }

    /// Ora locale `16:42`.
    pub fn time(&self, filetime: i64) -> String {
        system::local_time(filetime).map(|t| format!("{:02}:{:02}", t.hour, t.minute)).unwrap_or_default()
    }

    /// Durata in secondi con un decimale: `14,7 s`.
    pub fn seconds(&self, s: f64) -> String {
        format!("{} {}", self.decimal(s, 1), self.labels.seconds)
    }
}

/// Scrive una data secondo il modello di Windows (`dd/MM/yyyy`, `d.M.yy`, `yyyy-MM-dd`, ...).
/// Nomi dei mesi e dei giorni (`MMM`, `ddd`) si scrivono come numeri: le date brevi di Windows li usano di rado.
pub fn short_date(pattern: &str, year: u16, month: u16, day: u16) -> String {
    let mut out = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let mut n = 1;
        while i + n < chars.len() && chars[i + n] == c {
            n += 1;
        }
        match c {
            'd' => out.push_str(&if n >= 2 { format!("{day:02}") } else { day.to_string() }),
            'M' => out.push_str(&if n >= 2 { format!("{month:02}") } else { month.to_string() }),
            'y' => out.push_str(&if n >= 3 { format!("{year:04}") } else { format!("{:02}", year % 100) }),
            '\'' => {
                // Testo letterale fra apici.
                let end = chars[i + 1..].iter().position(|&x| x == '\'').map_or(chars.len(), |p| i + 1 + p);
                out.extend(&chars[i + 1..end]);
                i = end + 1;
                continue;
            }
            other => {
                for _ in 0..n {
                    out.push(other);
                }
            }
        }
        i += n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it() -> Formatter {
        Formatter::default()
    }

    #[test]
    fn sizes_like_explorer() {
        let f = it();
        assert_eq!(f.size(0), "0 byte");
        assert_eq!(f.size(1023), "1.023 byte");
        assert_eq!(f.size(1024), "1,00 KB");
        assert_eq!(f.size(1536), "1,50 KB");
        assert_eq!(f.size(1_048_575), "0,99 MB");
        assert_eq!(f.size(1_000_000), "976 KB");
        // Come Esplora file: dai GB in su si parte dai MB interi.
        assert_eq!(f.size(14_806_917_939), "13,7 GB");
        assert_eq!(f.size(476 * 1024 * 1024 * 1024), "476 GB");
    }

    #[test]
    fn si_units() {
        let f = Formatter { units: Units::Si, ..it() };
        assert_eq!(f.size(999), "999 byte");
        assert_eq!(f.size(1000), "1,00 kB");
        assert_eq!(f.size(14_806_917_939), "14,8 GB");
    }

    #[test]
    fn counts_and_grouping() {
        let f = it();
        assert_eq!(f.count(0), "0");
        assert_eq!(f.count(999), "999");
        assert_eq!(f.count(1000), "1.000");
        assert_eq!(f.count(1_734_052), "1.734.052");
        let en = Formatter { region: RegionFormat { decimal: ".".into(), thousands: ",".into(), ..RegionFormat::default() }, ..it() };
        assert_eq!(en.count(1_734_052), "1,734,052");
        assert_eq!(en.size(14_806_917_939), "13.7 GB");
        let india = Formatter { region: RegionFormat { grouping: "3;2;0".into(), thousands: ",".into(), ..RegionFormat::default() }, ..it() };
        assert_eq!(india.count(12_345_678), "1,23,45,678");
        assert_eq!(it().percent(0.137), "13,7%");
    }

    #[test]
    fn short_dates() {
        assert_eq!(short_date("dd/MM/yyyy", 2026, 10, 3), "03/10/2026");
        assert_eq!(short_date("d.M.yy", 2026, 10, 3), "3.10.26");
        assert_eq!(short_date("yyyy-MM-dd", 2026, 1, 13), "2026-01-13");
        assert_eq!(short_date("M/d/yyyy", 2026, 10, 13), "10/13/2026");
        assert_eq!(short_date("dd 'de' MM", 2026, 10, 13), "13 de 10");
    }
}
