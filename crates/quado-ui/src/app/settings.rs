// SPDX-License-Identifier: GPL-3.0-or-later
//! Impostazioni: un file di testo in `%APPDATA%\Quado`, scritto solo quando l'utente cambia qualcosa.
//! Righe `chiave = valore`; le chiavi sconosciute e i valori non validi si ignorano (vale il predefinito).

use std::path::{Path, PathBuf};

use quado_actions::{journal, legacy};

use crate::format::Units;
use crate::tokens::{Accent, DataPalette, Density, Motion, ThemeChoice};

pub const FILE_NAME: &str = "impostazioni.txt";

/// Dimensione del testo: da 90% a 150%, a passi di 10.
pub const TEXT_SCALE_MIN: u16 = 90;
pub const TEXT_SCALE_MAX: u16 = 150;
pub const TEXT_SCALE_STEP: u16 = 10;

/// `%APPDATA%\Quado`, l'unica sede dei dati di Quado.
pub fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| legacy::dirs(Path::new(&a)).0)
}

/// `%APPDATA%\Rilievo`, la cartella dei dati della versione precedente: Quado la legge solo per copiarne i
/// dati al primo avvio (`legacy::migrate`) e la mostra in Dati e privacy.
pub fn old_data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| legacy::dirs(Path::new(&a)).1)
}

/// Nome della cartella delle lingue dentro la cartella dei dati.
pub const LANG_DIR: &str = "lingue";

/// Cosa si copia dalla cartella di Rilievo al primo avvio: impostazioni, registro degli interventi, lingue.
pub const MIGRATED: [&str; 3] = [FILE_NAME, journal::FILE_NAME, LANG_DIR];

/// Copia i dati di Rilievo nella cartella di Quado, se quella di Quado non c'è ancora (`legacy::migrate`).
pub fn migrate(old: &Path, new: &Path) -> legacy::Migration {
    legacy::migrate(old, new, &MIGRATED)
}

/// Con cosa si apre Esplora.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExploreView {
    #[default]
    MapAndList,
    ListOnly,
}

impl ExploreView {
    pub const ALL: [Self; 2] = [Self::MapAndList, Self::ListOnly];

    pub fn code(self) -> &'static str {
        match self {
            Self::MapAndList => "mappa",
            Self::ListOnly => "elenco",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.code() == s)
    }
}

/// Colonne facoltative della tabella di Esplora. Nome e Dimensioni su disco si vedono sempre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Columns {
    /// Quota della cartella.
    pub share: bool,
    /// Numero di file.
    pub files: bool,
    /// Ultima modifica.
    pub date: bool,
    /// Gestito da.
    pub managed: bool,
}

impl Default for Columns {
    fn default() -> Self {
        Self { share: true, files: true, date: true, managed: true }
    }
}

impl Columns {
    const CODES: [&'static str; 4] = ["quota", "file", "data", "gestore"];

    fn flags(self) -> [bool; 4] {
        [self.share, self.files, self.date, self.managed]
    }

    /// Valore nel file: le colonne visibili separate da spazi (vuoto: nessuna).
    pub fn code(self) -> String {
        Self::CODES.iter().zip(self.flags()).filter(|(_, on)| *on).map(|(c, _)| *c).collect::<Vec<_>>().join(" ")
    }

    pub fn from_code(s: &str) -> Self {
        let has = |c: &str| s.split([' ', ',']).any(|w| w.trim() == c);
        Self { share: has("quota"), files: has("file"), date: has("data"), managed: has("gestore") }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Tag della lingua; `None` finché l'utente non ne sceglie una (vale la lingua di Windows).
    pub language: Option<String>,
    pub theme: ThemeChoice,
    pub units: Units,
    /// Colore delle azioni, della selezione e del focus.
    pub accent: Accent,
    /// Colori dei quattro gestori in barra, legende e mappa.
    pub data_palette: DataPalette,
    /// Altezza di righe ed elenchi.
    pub density: Density,
    /// Dimensione del testo in percento (90-150): è lo zoom dell'interfaccia.
    pub text_scale: u16,
    /// Animazioni: come Windows o spente.
    pub motion: Motion,
    /// Con cosa si apre Esplora.
    pub explore_view: ExploreView,
    /// Colonne facoltative della tabella di Esplora.
    pub columns: Columns,
    /// Barra laterale chiusa: solo le icone.
    pub sidebar_collapsed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
            theme: ThemeChoice::default(),
            units: Units::default(),
            accent: Accent::default(),
            data_palette: DataPalette::default(),
            density: Density::default(),
            text_scale: 100,
            motion: Motion::default(),
            explore_view: ExploreView::default(),
            columns: Columns::default(),
            sidebar_collapsed: false,
        }
    }
}

impl Settings {
    /// Zoom di egui che corrisponde alla dimensione del testo.
    pub fn zoom(&self) -> f32 {
        f32::from(self.text_scale.clamp(TEXT_SCALE_MIN, TEXT_SCALE_MAX)) / 100.0
    }

    /// Dimensione del testo un passo più grande o più piccola, nei limiti.
    pub fn step_text(&mut self, bigger: bool) {
        self.text_scale = if bigger { self.text_scale + TEXT_SCALE_STEP } else { self.text_scale.saturating_sub(TEXT_SCALE_STEP) }.clamp(TEXT_SCALE_MIN, TEXT_SCALE_MAX);
    }

    pub fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "lingua" if !v.is_empty() => s.language = Some(v.to_owned()),
                "tema" => s.theme = ThemeChoice::from_code(v).unwrap_or_default(),
                "unita" => s.units = Units::from_code(v).unwrap_or_default(),
                "accento" => s.accent = Accent::from_code(v).unwrap_or_default(),
                "colori" => s.data_palette = DataPalette::from_code(v).unwrap_or_default(),
                "densita" => s.density = Density::from_code(v).unwrap_or_default(),
                "testo" => {
                    if let Ok(n) = v.trim_end_matches('%').trim().parse::<u16>()
                        && (TEXT_SCALE_MIN..=TEXT_SCALE_MAX).contains(&n)
                    {
                        s.text_scale = n - n % TEXT_SCALE_STEP;
                    }
                }
                "animazioni" => s.motion = Motion::from_code(v).unwrap_or_default(),
                "esplora" => s.explore_view = ExploreView::from_code(v).unwrap_or_default(),
                "colonne" => s.columns = Columns::from_code(v),
                "barra" => s.sidebar_collapsed = v == "chiusa",
                _ => {}
            }
        }
        s
    }

    pub fn to_text(&self) -> String {
        let mut out = String::from("# Impostazioni di Quado. Si possono modificare a mano: righe chiave = valore.\n");
        if let Some(l) = &self.language {
            out.push_str(&format!("lingua = {l}\n"));
        }
        out.push_str(&format!("tema = {}\n", self.theme.code()));
        out.push_str(&format!("unita = {}\n", self.units.code()));
        out.push_str(&format!("accento = {}\n", self.accent.code()));
        out.push_str(&format!("colori = {}\n", self.data_palette.code()));
        out.push_str(&format!("densita = {}\n", self.density.code()));
        out.push_str(&format!("testo = {}\n", self.text_scale));
        out.push_str(&format!("animazioni = {}\n", self.motion.code()));
        out.push_str(&format!("esplora = {}\n", self.explore_view.code()));
        out.push_str(&format!("colonne = {}\n", self.columns.code()));
        out.push_str(&format!("barra = {}\n", if self.sidebar_collapsed { "chiusa" } else { "aperta" }));
        out
    }

    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_unknown_keys() {
        let s = Settings {
            language: Some("en".into()),
            theme: ThemeChoice::Dark,
            units: Units::Si,
            accent: Accent::Violet,
            data_palette: DataPalette::ColorBlind,
            density: Density::Compact,
            text_scale: 130,
            motion: Motion::Off,
            explore_view: ExploreView::ListOnly,
            columns: Columns { share: false, files: true, date: false, managed: true },
            sidebar_collapsed: true,
        };
        assert_eq!(Settings::parse(&s.to_text()), s);
        let odd = Settings::parse("tema = viola\nfuturo = 1\nlingua =\naccento = blu\ntesto = 400\ndensita = media\n");
        assert_eq!(odd, Settings::default());
        // Un file delle versioni precedenti (solo lingua, tema e unità) vale ancora, con il resto predefinito.
        let old = Settings::parse("lingua = it\ntema = chiaro\nunita = esplora\n");
        assert_eq!(old.theme, ThemeChoice::Light);
        assert_eq!(old.columns, Columns::default());
        assert_eq!(old.text_scale, 100);
    }

    #[test]
    fn text_size_stays_in_range() {
        let mut s = Settings::default();
        for _ in 0..10 {
            s.step_text(true);
        }
        assert_eq!(s.text_scale, TEXT_SCALE_MAX);
        for _ in 0..10 {
            s.step_text(false);
        }
        assert_eq!(s.text_scale, TEXT_SCALE_MIN);
        assert!((Settings::parse("testo = 125").zoom() - 1.2).abs() < 1e-6);
        assert_eq!(Columns::from_code(""), Columns { share: false, files: false, date: false, managed: false });
    }
}
