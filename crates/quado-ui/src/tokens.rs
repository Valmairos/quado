// SPDX-License-Identifier: GPL-3.0-or-later
//! Token della variante A "Apple" della proposta visiva (04 §6); i contrasti sono controllati in
//! `color_check.rs`. Tutti i colori sono piatti, senza trasparenze, tranne
//! l'oscuramento sotto i dialoghi.
//!
//! Come si usano:
//! - i colori arrivano in un [`Palette`] già risolto per tema × accento × colori dei dati: ogni schermata
//!   riceve `p: &Palette` e legge `p.text`, `p.panel`, `p.accent`, `p.data_color(g)`...;
//! - la densità (comoda o compatta) dà le altezze delle righe: [`metrics`] le legge dal contesto di egui,
//!   [`set_metrics`] le scrive (lo fa `App` quando cambia l'impostazione);
//! - la dimensione del testo (90-150%) è lo zoom di egui (`Context::set_zoom_factor`): scala testo,
//!   spaziature, icone e controlli insieme, quindi le schermate non moltiplicano niente;
//! - scala tipografica in [`Type`], spaziature in [`space`], raggi in [`radius`], ombre in
//!   [`Palette::shadow`], durate delle transizioni in [`motion`], anello di focus in [`focus`].
//!
//! Il colore delle azioni è l'accento (Grafite predefinito, Viola, Rosa): colora solo pulsanti primari,
//! interruttori, selezione e focus. Il colore dei dati resta ai quattro gestori, ognuno con la sua forma
//! (cerchio, rombo, triangolo, anello) perché il colore non sia mai l'unico segno (04 §6.1).

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, Id, Stroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Self; 2] = [Self::Light, Self::Dark];

    pub fn toggled(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }
}

/// Tema scelto in Impostazioni.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn code(self) -> &'static str {
        match self {
            Self::System => "sistema",
            Self::Light => "chiaro",
            Self::Dark => "scuro",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.code() == s)
    }
}

/// Chi gestisce uno spazio (03 §6.1): decide il colore e la forma dei dati.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Gestore {
    /// Programmi e dati delle app.
    App,
    /// Tuoi file.
    Mine,
    /// Windows.
    Windows,
    /// Non classificato, o ipotesi non abbastanza affidabile per colorare.
    Unknown,
}

impl Gestore {
    pub const ALL: [Self; 4] = [Self::App, Self::Mine, Self::Windows, Self::Unknown];

    /// Chiave del nome nei file di lingua.
    pub fn key(self) -> &'static str {
        match self {
            Self::App => "gestore-app",
            Self::Mine => "gestore-tuoi",
            Self::Windows => "gestore-windows",
            Self::Unknown => "gestore-non-classificato",
        }
    }

    /// Forma del glifo del gestore: il colore non è mai l'unico segno.
    pub fn shape(self) -> Glyph {
        match self {
            Self::App => Glyph::Circle,
            Self::Mine => Glyph::Diamond,
            Self::Windows => Glyph::Triangle,
            Self::Unknown => Glyph::Ring,
        }
    }
}

/// Forme dei glifi dei dati (legende, righe, mappa), disegnate da `widgets::glyph`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// Programmi e dati delle app.
    Circle,
    /// Tuoi file.
    Diamond,
    /// Windows.
    Triangle,
    /// Non classificato: anello vuoto.
    Ring,
    /// Metadati di NTFS e spazio riservato: barretta orizzontale.
    Bar,
    /// Non attribuito o non leggibile: quadrato tratteggiato.
    Hatch,
    /// Spazio libero: quadrato vuoto.
    Square,
}

/// Accento scelto in Impostazioni: pulsanti primari, interruttori, selezione, focus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Accent {
    #[default]
    Graphite,
    Violet,
    Rose,
}

impl Accent {
    pub const ALL: [Self; 3] = [Self::Graphite, Self::Violet, Self::Rose];

    /// Valore nel file delle impostazioni.
    pub fn code(self) -> &'static str {
        match self {
            Self::Graphite => "grafite",
            Self::Violet => "viola",
            Self::Rose => "rosa",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.code() == s)
    }

    /// Chiave del nome nei file di lingua.
    pub fn key(self) -> &'static str {
        match self {
            Self::Graphite => "accento-grafite",
            Self::Violet => "accento-viola",
            Self::Rose => "accento-rosa",
        }
    }

    pub fn colors(self, theme: Theme) -> AccentColors {
        let d = theme == Theme::Dark;
        match self {
            Self::Graphite if d => AccentColors { fill: hex(0xF5F5F7), on: hex(0x1C1C1E), text: hex(0xF5F5F7), ring: hex(0xE3E3E8), select: hex(0x3A3A3E) },
            Self::Graphite => AccentColors { fill: hex(0x1D1D1F), on: hex(0xFFFFFF), text: hex(0x1D1D1F), ring: hex(0x3A3A3C), select: hex(0xDADADF) },
            Self::Violet if d => AccentColors { fill: hex(0xA58BF7), on: hex(0x16121F), text: hex(0xB9A6FA), ring: hex(0xB9A6FA), select: hex(0x383049) },
            Self::Violet => AccentColors { fill: hex(0x6440D8), on: hex(0xFFFFFF), text: hex(0x5A36CE), ring: hex(0x6440D8), select: hex(0xE6E0F8) },
            Self::Rose if d => AccentColors { fill: hex(0xF08AA0), on: hex(0x1F1215), text: hex(0xF49CB0), ring: hex(0xF49CB0), select: hex(0x4A2F36) },
            Self::Rose => AccentColors { fill: hex(0xB8325A), on: hex(0xFFFFFF), text: hex(0xB02E55), ring: hex(0xB8325A), select: hex(0xF6E0E6) },
        }
    }
}

/// Colori di un accento in un tema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccentColors {
    /// Pulsante primario, interruttore acceso, casella spuntata.
    pub fill: Color32,
    /// Testo e segni sopra `fill`.
    pub on: Color32,
    /// Accento come testo (collegamenti nel testo).
    pub text: Color32,
    /// Anello del focus da tastiera.
    pub ring: Color32,
    /// Fondo della riga o della voce scelta.
    pub select: Color32,
}

/// Colori dei dati scelti in Impostazioni (04 §6.2): la forma e il nome dei gestori non cambiano.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DataPalette {
    /// Tinte tenui, poca croma.
    #[default]
    Default,
    /// Tinte più luminose e sature, etichette nere sulla mappa.
    HighContrast,
    /// Okabe-Ito ritoccati: blu, arancio, porpora; grigi a luminanze distanti.
    ColorBlind,
}

impl DataPalette {
    pub const ALL: [Self; 3] = [Self::Default, Self::HighContrast, Self::ColorBlind];

    /// Valore nel file delle impostazioni.
    pub fn code(self) -> &'static str {
        match self {
            Self::Default => "predefiniti",
            Self::HighContrast => "contrasto",
            Self::ColorBlind => "discromatopsie",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.code() == s)
    }

    /// Chiave del nome nei file di lingua.
    pub fn key(self) -> &'static str {
        match self {
            Self::Default => "colori-dati-predefiniti",
            Self::HighContrast => "colori-dati-contrasto",
            Self::ColorBlind => "colori-dati-discromatopsie",
        }
    }

    pub fn colors(self, theme: Theme) -> DataColors {
        let d = theme == Theme::Dark;
        let h = |v: [u32; 4]| v.map(hex);
        match self {
            Self::Default if d => DataColors {
                data: h([0x6494D6, 0xE29260, 0x79C5BD, 0x7B7C7F]),
                meta: hex(0xE3E3E6),
                tile: h([0x28436B, 0x694028, 0x2E625E, 0x3A3A3D]),
                tile_text: [hex(0xF2F2F4); 4],
                hatch: hex(0x7A7A80),
            },
            Self::Default => DataColors {
                data: h([0x4077B9, 0xB96A26, 0x2D6557, 0x858586]),
                meta: hex(0x2E2F2F),
                tile: h([0xB5CCF0, 0xF4C9A3, 0x8CBEB9, 0xE2E3E5]),
                tile_text: [hex(0x1D1D1F); 4],
                hatch: hex(0x8E8E93),
            },
            Self::HighContrast if d => DataColors {
                data: h([0x63A5EE, 0xF8AC5D, 0x5ADEC6, 0x9B9B9B]),
                meta: hex(0xF9F9F9),
                tile: h([0x55B0FE, 0xFA9E38, 0x80F5DB, 0x969696]),
                tile_text: [hex(0x000000); 4],
                hatch: hex(0xB0B0B0),
            },
            Self::HighContrast => DataColors {
                data: h([0x032FA1, 0x9E4A12, 0x0C7068, 0x303030]),
                meta: hex(0x0A0A0A),
                tile: h([0x87AFF3, 0xFDA964, 0xA1F6E4, 0xC0C0C0]),
                tile_text: [hex(0x000000); 4],
                hatch: hex(0x5A5A5A),
            },
            Self::ColorBlind if d => DataColors {
                data: h([0x559BFA, 0xFC9C26, 0xB56087, 0xBDBDBD]),
                meta: hex(0xF5F5F5),
                tile: h([0x36ADFE, 0xFCAE4E, 0xBA6C8D, 0xE3E3E3]),
                tile_text: [hex(0x141414); 4],
                hatch: hex(0x9A9A9A),
            },
            Self::ColorBlind => DataColors {
                data: h([0x1565AA, 0xA15019, 0x730E46, 0x7F7F7F]),
                meta: hex(0x121212),
                tile: h([0x36ADFE, 0xFCAE4E, 0xBA6C8D, 0xE3E3E3]),
                tile_text: [hex(0x141414); 4],
                hatch: hex(0x6E6E6E),
            },
        }
    }
}

/// Colori dei dati in un tema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataColors {
    /// Barra del bilancio, legende, indicatori, per gestore.
    pub data: [Color32; 4],
    /// Metadati di NTFS e spazio riservato.
    pub meta: Color32,
    /// Riempimento delle aree della mappa, per gestore.
    pub tile: [Color32; 4],
    /// Etichetta sopra `tile` (≥ 4,5:1).
    pub tile_text: [Color32; 4],
    /// Linee del tratteggio del non attribuito, su fondo `track`.
    pub hatch: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Superfici, testo e stati di un tema (pagina Token, "Superfici, testo, stati").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chrome {
    /// Fondo del contenuto (e della barra superiore e di stato).
    pub bg: Color32,
    /// Barra laterale e ispettore.
    pub sidebar: Color32,
    /// Gruppi e card.
    pub panel: Color32,
    /// Riquadri dentro card e dialoghi.
    pub panel2: Color32,
    /// Dialoghi, palette, suggerimenti, menu.
    pub raised: Color32,
    /// Campi, pulsanti secondari, controlli segmentati.
    pub field: Color32,
    /// Passaggio del mouse.
    pub hover: Color32,
    /// Filetti di 1 px.
    pub separator: Color32,
    /// Contorni dei controlli.
    pub border: Color32,
    /// Fondo delle barre (spazio libero) e del tratteggio.
    pub track: Color32,
    pub text: Color32,
    pub text_2: Color32,
    /// Segnaposto, testo terziario (≥ 4,5:1 come gli altri).
    pub text_3: Color32,
    /// Solo controlli disattivati (esenti dal 4,5:1).
    pub disabled: Color32,
    /// Stato Da verificare, avvisi.
    pub warn: Color32,
    pub warn_bg: Color32,
    /// Controlli superati.
    pub ok: Color32,
    /// Oscuramento sotto i dialoghi.
    pub scrim: Color32,
}

impl Chrome {
    pub const DARK: Self = Self {
        bg: hex(0x1C1C1E),
        sidebar: hex(0x232325),
        panel: hex(0x262628),
        panel2: hex(0x2A2A2D),
        raised: hex(0x2C2C2E),
        field: hex(0x313134),
        hover: hex(0x2F2F32),
        separator: hex(0x343437),
        border: hex(0x3E3E42),
        track: hex(0x353538),
        text: hex(0xF5F5F7),
        text_2: hex(0xB4B4BA),
        text_3: hex(0xA3A3AA),
        disabled: hex(0x6C6C72),
        warn: hex(0xF2C25B),
        warn_bg: hex(0x3A3020),
        ok: hex(0x7ED99A),
        scrim: Color32::from_black_alpha(140),
    };

    pub const LIGHT: Self = Self {
        bg: hex(0xF5F5F7),
        sidebar: hex(0xEAEAED),
        panel: hex(0xFFFFFF),
        panel2: hex(0xFAFAFB),
        raised: hex(0xFFFFFF),
        field: hex(0xDEDEE3),
        hover: hex(0xEFEFF2),
        separator: hex(0xE3E3E8),
        border: hex(0xCDCDD3),
        track: hex(0xE6E6EA),
        text: hex(0x1D1D1F),
        text_2: hex(0x545458),
        text_3: hex(0x5F5F64),
        disabled: hex(0xA1A1A6),
        warn: hex(0x8A5A00),
        warn_bg: hex(0xFBF1DA),
        ok: hex(0x24703A),
        scrim: Color32::from_black_alpha(71),
    };

    pub fn of(theme: Theme) -> Self {
        match theme {
            Theme::Light => Self::LIGHT,
            Theme::Dark => Self::DARK,
        }
    }
}

/// Livello di un'ombra (`epaint::Shadow`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    /// Gruppi e card: solo in chiaro.
    Card,
    /// Suggerimenti, menu, palette.
    Popover,
    /// Dialoghi.
    Dialog,
    /// Voce scelta del controllo segmentato: solo in chiaro.
    Segment,
}

/// Colori risolti per tema × accento × colori dei dati. Si copia: le schermate lo ricevono come `&Palette`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub theme: Theme,
    pub bg: Color32,
    pub sidebar: Color32,
    pub panel: Color32,
    pub panel2: Color32,
    pub raised: Color32,
    pub field: Color32,
    pub hover: Color32,
    pub separator: Color32,
    pub border: Color32,
    pub track: Color32,
    pub text: Color32,
    pub text_2: Color32,
    pub text_3: Color32,
    pub disabled: Color32,
    pub warn: Color32,
    pub warn_bg: Color32,
    pub ok: Color32,
    pub scrim: Color32,
    /// Accento: pulsante primario, interruttore acceso, casella spuntata.
    pub accent: Color32,
    /// Testo e segni sopra `accent`.
    pub on_accent: Color32,
    /// Accento come testo.
    pub accent_text: Color32,
    /// Anello del focus.
    pub ring: Color32,
    /// Fondo della riga o della voce scelta.
    pub select: Color32,
    /// Colore dei dati in barre e indicatori, per gestore.
    pub data: [Color32; 4],
    /// Metadati di NTFS e spazio riservato nella barra del bilancio.
    pub meta: Color32,
    /// Riempimento delle aree della mappa, per gestore (contrasto verificato con `tile_text`).
    pub tile: [Color32; 4],
    pub tile_text: [Color32; 4],
    /// Linee del tratteggio del non attribuito (su `track`).
    pub hatch: Color32,
}

impl Palette {
    pub fn new(theme: Theme, accent: Accent, data: DataPalette) -> Self {
        let c = Chrome::of(theme);
        let a = accent.colors(theme);
        let d = data.colors(theme);
        Self {
            theme,
            bg: c.bg,
            sidebar: c.sidebar,
            panel: c.panel,
            panel2: c.panel2,
            raised: c.raised,
            field: c.field,
            hover: c.hover,
            separator: c.separator,
            border: c.border,
            track: c.track,
            text: c.text,
            text_2: c.text_2,
            text_3: c.text_3,
            disabled: c.disabled,
            warn: c.warn,
            warn_bg: c.warn_bg,
            ok: c.ok,
            scrim: c.scrim,
            accent: a.fill,
            on_accent: a.on,
            accent_text: a.text,
            ring: a.ring,
            select: a.select,
            data: d.data,
            meta: d.meta,
            tile: d.tile,
            tile_text: d.tile_text,
            hatch: d.hatch,
        }
    }

    /// Tema con accento e colori dei dati predefiniti.
    pub fn of(theme: Theme) -> Self {
        Self::new(theme, Accent::default(), DataPalette::default())
    }

    pub fn data_color(&self, g: Gestore) -> Color32 {
        self.data[g as usize]
    }

    pub fn tile_color(&self, g: Gestore) -> (Color32, Color32) {
        (self.tile[g as usize], self.tile_text[g as usize])
    }

    /// Ombra morbida di un livello (pagina Token, "Ombre"). In scuro le card non hanno ombra: si separano
    /// dal fondo con il tono.
    pub fn shadow(&self, e: Elevation) -> Shadow {
        let dark = self.theme == Theme::Dark;
        let (y, blur, alpha) = match (e, dark) {
            (Elevation::Card, true) | (Elevation::Segment, true) => return Shadow::NONE,
            (Elevation::Card, false) => (1, 3, 14),
            (Elevation::Segment, false) => (1, 2, 36),
            (Elevation::Popover, true) => (8, 24, 140),
            (Elevation::Popover, false) => (8, 24, 36),
            (Elevation::Dialog, true) => (18, 48, 160),
            (Elevation::Dialog, false) => (18, 48, 48),
        };
        Shadow { offset: [0, y], blur, spread: 0, color: Color32::from_black_alpha(alpha) }
    }
}

/// Ruoli della scala tipografica (Inter con le cifre tabulari; IBM Plex Mono solo per percorsi e comandi).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    /// 48/300: cifre chiave (spazio usato, numeri del dialogo).
    Display,
    /// 28/600: titolo di schermata.
    LargeTitle,
    /// 20/600: titolo del dialogo, cifra dell'ispettore.
    Title,
    /// 15/600: titolo di gruppo.
    Headline,
    /// 13/400: testo di base, righe, controlli.
    Body,
    /// 13/500: nome della voce.
    BodyStrong,
    /// 12/400: seconda riga, note.
    Callout,
    /// 11/500: intestazioni di colonna, legende.
    Caption,
}

/// Peso di Inter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Light,
    Regular,
    Medium,
    SemiBold,
}

impl Type {
    pub const ALL: [Self; 8] = [Self::Display, Self::LargeTitle, Self::Title, Self::Headline, Self::Body, Self::BodyStrong, Self::Callout, Self::Caption];

    /// Dimensione in punti (a 100%: la dimensione del testo è lo zoom di egui).
    pub fn size(self) -> f32 {
        match self {
            Self::Display => 48.0,
            Self::LargeTitle => 28.0,
            Self::Title => 20.0,
            Self::Headline => 15.0,
            Self::Body | Self::BodyStrong => 13.0,
            Self::Callout => 12.0,
            Self::Caption => 11.0,
        }
    }

    pub fn weight(self) -> Weight {
        match self {
            Self::Display => Weight::Light,
            Self::LargeTitle | Self::Title | Self::Headline => Weight::SemiBold,
            Self::Body | Self::Callout => Weight::Regular,
            Self::BodyStrong | Self::Caption => Weight::Medium,
        }
    }

    /// Spaziatura tra le lettere in em (negativa nei titoli, come la proposta).
    pub fn tracking_em(self) -> f32 {
        match self {
            Self::Display => -0.024,
            Self::LargeTitle => -0.016,
            Self::Title => -0.012,
            Self::Headline => -0.006,
            Self::Body | Self::BodyStrong => -0.003,
            Self::Callout => 0.0,
            Self::Caption => 0.01,
        }
    }

    /// Spaziatura tra le lettere in punti, per `TextFormat::extra_letter_spacing`.
    pub fn tracking(self) -> f32 {
        self.tracking_em() * self.size()
    }
}

/// Spaziature della variante A (multipli di 4).
pub mod space {
    pub const S4: f32 = 4.0;
    pub const S8: f32 = 8.0;
    pub const S12: f32 = 12.0;
    pub const S16: f32 = 16.0;
    pub const S20: f32 = 20.0;
    pub const S24: f32 = 24.0;
    pub const S32: f32 = 32.0;
    pub const S40: f32 = 40.0;
    pub const S56: f32 = 56.0;
    /// La scala intera, in ordine.
    pub const SCALE: [f32; 9] = [S4, S8, S12, S16, S20, S24, S32, S40, S56];
}

/// Raggi degli angoli (`CornerRadius::same`).
pub mod radius {
    /// Pulsanti, campi, controlli segmentati.
    pub const CONTROL: u8 = 8;
    /// Righe di elenchi e voci della barra laterale.
    pub const ROW: u8 = 8;
    /// Gruppi di righe (Impostazioni), riquadri dentro le card, mappa.
    pub const GROUP: u8 = 12;
    /// Card.
    pub const CARD: u8 = 14;
    /// Dialoghi.
    pub const DIALOG: u8 = 16;
    /// Gruppi della mappa.
    pub const TILE: u8 = 6;
    /// Aree interne della mappa.
    pub const TILE_INNER: u8 = 4;
    /// Barra del bilancio.
    pub const BAR: u8 = 7;
}

/// Durate delle transizioni, in secondi. Zero con "Effetti di animazione" spenti o con Animazioni
/// spente in Impostazioni (egui usa `Style::animation_time`, impostata da [`apply`]).
pub mod motion {
    /// Opacità.
    pub const FAST: f32 = 0.09;
    /// Posizione.
    pub const BASE: f32 = 0.12;
}

/// Anello del focus da tastiera: 2 px nel colore `ring`, 2 px fuori dal controllo (dentro le righe).
pub mod focus {
    pub const WIDTH: f32 = 2.0;
    pub const OFFSET: f32 = 2.0;
    /// Area cliccabile minima, in punti, per lato.
    pub const MIN_TARGET: f32 = 32.0;
}

/// Densità scelta in Impostazioni: altezza di righe ed elenchi. I controlli restano alti 32.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

impl Density {
    pub const ALL: [Self; 2] = [Self::Comfortable, Self::Compact];

    /// Valore nel file delle impostazioni.
    pub fn code(self) -> &'static str {
        match self {
            Self::Comfortable => "comoda",
            Self::Compact => "compatta",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.code() == s)
    }

    pub fn metrics(self) -> Metrics {
        match self {
            Self::Comfortable => Metrics { density: self, row: 40.0, row_two_line: 56.0, control: 32.0, pad: 16.0 },
            Self::Compact => Metrics { density: self, row: 32.0, row_two_line: 44.0, control: 32.0, pad: 10.0 },
        }
    }
}

/// Misure che dipendono dalla densità.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub density: Density,
    /// Riga di una tabella o di un elenco su una riga.
    pub row: f32,
    /// Riga con nome e seconda riga.
    pub row_two_line: f32,
    /// Altezza dei controlli (uguale nelle due densità: area cliccabile ≥ 32).
    pub control: f32,
    /// Margine interno dei gruppi.
    pub pad: f32,
}

impl Default for Metrics {
    fn default() -> Self {
        Density::default().metrics()
    }
}

fn metrics_id() -> Id {
    Id::new("quado_densita")
}

/// Misure della densità in uso (comoda se nessuno le ha impostate).
pub fn metrics(ctx: &egui::Context) -> Metrics {
    ctx.data(|d| d.get_temp(metrics_id())).unwrap_or_default()
}

/// Imposta la densità per i frame successivi.
pub fn set_metrics(ctx: &egui::Context, m: Metrics) {
    ctx.data_mut(|d| d.insert_temp(metrics_id(), m));
}

/// Animazioni scelte in Impostazioni.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    /// Come in Windows: nessuna animazione se "Effetti di animazione" è spento.
    #[default]
    System,
    /// Sempre spente.
    Off,
}

impl Motion {
    pub const ALL: [Self; 2] = [Self::System, Self::Off];

    pub fn code(self) -> &'static str {
        match self {
            Self::System => "windows",
            Self::Off => "spente",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.code() == s)
    }
}

/// Applica i token a egui per un tema: fondi, testo, controlli di egui (menu a tendina, campi di testo,
/// suggerimenti) e il focus visibile anche sui controlli di egui.
pub fn apply(ctx: &egui::Context, p: &Palette, animations: bool) {
    let mut v = match p.theme {
        Theme::Light => egui::Visuals::light(),
        Theme::Dark => egui::Visuals::dark(),
    };
    v.panel_fill = p.bg;
    v.window_fill = p.raised;
    v.window_stroke = Stroke::new(1.0, p.separator);
    v.window_corner_radius = CornerRadius::same(radius::GROUP);
    v.menu_corner_radius = CornerRadius::same(10);
    v.extreme_bg_color = p.field;
    v.text_edit_bg_color = Some(p.field);
    v.faint_bg_color = p.panel2;
    v.code_bg_color = p.panel2;
    v.override_text_color = Some(p.text);
    v.weak_text_color = Some(p.text_3);
    v.hyperlink_color = p.accent_text;
    v.selection.bg_fill = p.select;
    // Bordo dei campi di testo con il focus e testo selezionato.
    v.selection.stroke = Stroke::new(focus::WIDTH, p.ring);
    v.warn_fg_color = p.warn;
    v.error_fg_color = p.warn;
    v.text_cursor.stroke = Stroke::new(1.5, p.text);
    let r = CornerRadius::same(radius::CONTROL);
    for (w, fill) in [
        (&mut v.widgets.noninteractive, p.bg),
        (&mut v.widgets.inactive, p.field),
        (&mut v.widgets.hovered, p.hover),
        (&mut v.widgets.active, p.select),
        (&mut v.widgets.open, p.hover),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.corner_radius = r;
        w.fg_stroke = Stroke::new(1.0, p.text);
        w.bg_stroke = Stroke::NONE;
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text_2);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.separator);
    // egui disegna i controlli con il focus nello stile "active": l'anello li rende visibili.
    v.widgets.active.bg_stroke = Stroke::new(focus::WIDTH, p.ring);
    v.popup_shadow = p.shadow(Elevation::Popover);
    v.window_shadow = p.shadow(Elevation::Dialog);
    ctx.set_visuals_of(
        match p.theme {
            Theme::Light => egui::Theme::Light,
            Theme::Dark => egui::Theme::Dark,
        },
        v,
    );
    ctx.all_styles_mut(|s| {
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.interact_size.y = focus::MIN_TARGET;
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.menu_margin = egui::Margin::same(6);
        s.interaction.selectable_labels = false;
        // "Effetti di animazione" di Windows spenti: nessuna animazione, nemmeno nello scorrimento (04 §6.1).
        s.animation_time = if animations { motion::BASE } else { 0.0 };
        s.scroll_animation = if animations { egui::style::ScrollAnimation::default() } else { egui::style::ScrollAnimation::none() };
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color_check::{MIN_DELTA_E, contrast, worst_pair};

    /// Ogni combinazione tema × colori dei dati × accento.
    fn all() -> Vec<(String, Palette)> {
        let mut out = Vec::new();
        for t in Theme::ALL {
            for d in DataPalette::ALL {
                for a in Accent::ALL {
                    out.push((format!("{t:?}/{d:?}/{a:?}"), Palette::new(t, a, d)));
                }
            }
        }
        out
    }

    #[test]
    fn every_text_on_every_surface_and_state_is_readable() {
        // 06 §7, F-GE-2, 04 §6.2: testo ≥ 4,5:1 su ogni superficie e in ogni stato (normale, hover,
        // selezionato), in ogni combinazione di tema, accento e colori dei dati.
        let mut bad = Vec::new();
        for (name, p) in all() {
            let surfaces = [
                ("bg", p.bg),
                ("sidebar", p.sidebar),
                ("panel", p.panel),
                ("panel2", p.panel2),
                ("raised", p.raised),
                ("field", p.field),
                ("hover", p.hover),
                ("select", p.select),
            ];
            for (tn, t) in [("text", p.text), ("text-2", p.text_2), ("text-3", p.text_3)] {
                for (sn, s) in surfaces {
                    let c = contrast(t, s);
                    if c < 4.5 {
                        bad.push(format!("{name}: {tn} su {sn} {c:.2}:1"));
                    }
                }
            }
            let text_pairs = [
                ("on-accent su accent", p.on_accent, p.accent),
                ("accent-text su panel", p.accent_text, p.panel),
                ("accent-text su raised", p.accent_text, p.raised),
                ("accent-text su bg", p.accent_text, p.bg),
                ("warn su panel", p.warn, p.panel),
                ("warn su raised", p.warn, p.raised),
                ("warn su warn-bg", p.warn, p.warn_bg),
                ("ok su raised", p.ok, p.raised),
                ("ok su panel", p.ok, p.panel),
                ("text su warn-bg", p.text, p.warn_bg),
            ];
            for (pn, a, b) in text_pairs {
                let c = contrast(a, b);
                if c < 4.5 {
                    bad.push(format!("{name}: {pn} {c:.2}:1"));
                }
            }
            // Anello del focus e riempimento dell'accento: componenti grafici, ≥ 3:1 (WCAG 1.4.11).
            let ui_pairs = [
                ("ring su bg", p.ring, p.bg),
                ("ring su panel", p.ring, p.panel),
                ("ring su sidebar", p.ring, p.sidebar),
                ("ring su raised", p.ring, p.raised),
                ("accent su panel", p.accent, p.panel),
            ];
            for (pn, a, b) in ui_pairs {
                let c = contrast(a, b);
                if c < 3.0 {
                    bad.push(format!("{name}: {pn} {c:.2}:1"));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn data_colours_are_visible_and_distinct() {
        // 04 §6.2: tinte dei dati e metadati ≥ 3:1 con ogni fondo su cui compaiono; coppie con ΔE OKLab
        // ≥ 3,0 in visione normale e con protanopia, deuteranopia e tritanopia (Machado 2009).
        let mut bad = Vec::new();
        for (name, p) in all() {
            let set: Vec<Color32> = p.data.iter().copied().chain([p.meta]).collect();
            for (k, c) in set.iter().enumerate() {
                for (sn, s) in [("bg", p.bg), ("sidebar", p.sidebar), ("panel", p.panel), ("raised", p.raised)] {
                    let r = contrast(*c, s);
                    if r < 3.0 {
                        bad.push(format!("{name}: tinta {k} su {sn} {r:.2}:1"));
                    }
                }
            }
            let (d, v, i, j) = worst_pair(&set);
            if d < MIN_DELTA_E {
                bad.push(format!("{name}: tinte {i} e {j} con {v:?}, ΔE {d:.2}"));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn treemap_tiles_keep_labels_readable_and_fills_distinct() {
        // 04 §6.2: etichetta sulla mappa ≥ 4,5:1; aree distinguibili con ΔE ≥ 3,0 anche nelle simulazioni.
        let mut bad = Vec::new();
        for (name, p) in all() {
            for g in Gestore::ALL {
                let (fill, text) = p.tile_color(g);
                let c = contrast(fill, text);
                if c < 4.5 {
                    bad.push(format!("{name}: etichetta su {g:?} {c:.2}:1"));
                }
            }
            let (d, v, i, j) = worst_pair(&p.tile);
            if d < MIN_DELTA_E {
                bad.push(format!("{name}: aree {i} e {j} con {v:?}, ΔE {d:.2}"));
            }
            // Il tratteggio resta visibile sul fondo delle barre (soglia della proposta, 1,8:1).
            let h = contrast(p.hatch, p.track);
            if h < 1.8 {
                bad.push(format!("{name}: tratteggio {h:.2}:1"));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn accent_changes_only_actions_and_selection() {
        // L'accento non tocca i colori dei dati né le superfici.
        for t in Theme::ALL {
            let a = Palette::new(t, Accent::Graphite, DataPalette::Default);
            let b = Palette::new(t, Accent::Rose, DataPalette::Default);
            assert_eq!(a.data, b.data);
            assert_eq!(a.tile, b.tile);
            assert_eq!((a.bg, a.panel, a.text), (b.bg, b.panel, b.text));
            assert_ne!(a.accent, b.accent);
        }
    }

    #[test]
    fn codes_round_trip_and_glyphs_differ() {
        for a in Accent::ALL {
            assert_eq!(Accent::from_code(a.code()), Some(a));
        }
        for d in DataPalette::ALL {
            assert_eq!(DataPalette::from_code(d.code()), Some(d));
        }
        for d in Density::ALL {
            assert_eq!(Density::from_code(d.code()), Some(d));
        }
        for m in Motion::ALL {
            assert_eq!(Motion::from_code(m.code()), Some(m));
        }
        let shapes: std::collections::HashSet<_> = Gestore::ALL.iter().map(|g| format!("{:?}", g.shape())).collect();
        assert_eq!(shapes.len(), 4, "ogni gestore ha una forma propria");
        assert!(Density::Compact.metrics().row < Density::Comfortable.metrics().row);
        assert!(Density::ALL.iter().all(|d| d.metrics().control >= focus::MIN_TARGET));
    }
}
