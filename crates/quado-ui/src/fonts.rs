// SPDX-License-Identifier: GPL-3.0-or-later
//! Font: Inter per tutta l'interfaccia, numeri compresi (pesi 300, 400, 500, 600), IBM Plex Mono solo per
//! percorsi e comandi. Tutti incorporati nell'eseguibile, sotto SIL OFL 1.1 (`assets/fonts/Inter-LICENSE.txt`,
//! `assets/fonts/OFL.txt`).
//!
//! egui non applica le funzioni OpenType, quindi non attiva `tnum`: i file `Inter-*-tnum.ttf` hanno le cifre
//! tabulari fisse nella tabella `cmap` (`assets/fonts/cifre_tabulari.py`). Così colonne, cifre chiave e
//! contatori stanno in colonna anche in Inter.
//!
//! Inter copre l'alfabeto latino esteso, il greco e il cirillico, non il cinese, il giapponese, il coreano,
//! l'arabo o l'ebraico; Plex Mono copre meno. egui 0.36 non ripiega da solo sui font di sistema (annunciato
//! per la 0.37): i nomi di file in quelle scritture comparirebbero come quadratini. Per questo, se un
//! nome lo richiede, si aggiungono come ripiego alcuni font di Windows da `%WINDIR%\Fonts`, senza copiarli
//! né distribuirli ([`needs_fallbacks`]). Solo quelli che coprono i caratteri mancanti ([`choose_fallbacks`]):
//! tutti e sette sono 52 MB, e su C: li chiedevano 71 nomi su 3,6 milioni. I file si proiettano in memoria
//! (`system::map_font`): egui li usa senza copiarli e Windows carica solo le pagine dei glifi usati.

use std::sync::Mutex;
use std::time::Instant;

use egui::{FontData, FontDefinitions, FontFamily, FontId};
use skrifa::MetadataProvider as _;

const INTER_LIGHT: &[u8] = include_bytes!("../../../assets/fonts/Inter-Light-tnum.ttf");
const INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular-tnum.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/Inter-Medium-tnum.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold-tnum.ttf");
const PLEX_MONO: &[u8] = include_bytes!("../../../assets/fonts/IBMPlexMono-Regular.ttf");
const PLEX_MONO_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/IBMPlexMono-Medium.ttf");

/// Famiglia di Inter light (300): cifre chiave grandi.
pub fn light() -> FontFamily {
    FontFamily::Name("inter-light".into())
}

/// Famiglia di Inter medium (500): nomi delle voci, intestazioni di colonna, legende.
pub fn medium() -> FontFamily {
    FontFamily::Name("inter-medium".into())
}

/// Famiglia di Inter semibold (600): titoli, pulsanti primari.
pub fn semibold() -> FontFamily {
    FontFamily::Name("inter-semibold".into())
}

/// Famiglia di Plex Mono medium (comandi in evidenza).
pub fn mono_medium() -> FontFamily {
    FontFamily::Name("plex-mono-medium".into())
}

/// Segoe UI resa da egui: serve solo al confronto del testo con ClearType (stesso font, rasterizzatore diverso).
pub fn segoe_egui() -> FontFamily {
    FontFamily::Name("segoe-ui-egui".into())
}

/// Le famiglie che l'interfaccia usa per i testi che vengono dal disco (nomi e percorsi): il controllo dei
/// font di ripiego le prova tutte.
pub fn data_families() -> [FontFamily; 2] {
    [FontFamily::Proportional, FontFamily::Monospace]
}

/// Font di Windows usati come ripiego, nell'ordine in cui si cercano i glifi mancanti.
/// `(file, indice nella collezione, cosa copre)`.
pub const SYSTEM_FALLBACKS: &[(&str, u32, &str)] = &[
    ("segoeui.ttf", 0, "latino, greco, cirillico, arabo, ebraico"),
    ("seguisym.ttf", 0, "simboli"),
    ("msyh.ttc", 0, "cinese"),
    ("YuGothM.ttc", 0, "giapponese"),
    ("malgun.ttf", 0, "coreano"),
    ("Nirmala.ttf", 0, "scritture indiane"),
    ("LeelawUI.ttf", 0, "thai"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FontOptions {
    /// Aggiunge come ripiego tutti i font di Windows di [`SYSTEM_FALLBACKS`] (misure e confronti del testo;
    /// l'app aggiunge solo quelli che servono, [`install_fallbacks`]).
    pub system_fallbacks: bool,
    /// Registra Segoe UI come famiglia a sé, per il confronto del testo con ClearType.
    pub segoe_family: bool,
}

impl Default for FontOptions {
    fn default() -> Self {
        Self { system_fallbacks: true, segoe_family: false }
    }
}

/// Cosa è stato caricato, per le misure delle prestazioni.
#[derive(Clone, Debug, Default)]
pub struct FontReport {
    /// Font di sistema caricati: nome del file e byte del file.
    pub system: Vec<(String, usize)>,
    /// Font di sistema richiesti ma non trovati o non leggibili.
    pub missing: Vec<String>,
    pub load_ms: f64,
}

impl FontReport {
    pub fn system_bytes(&self) -> usize {
        self.system.iter().map(|(_, b)| b).sum()
    }
}

/// Un font di Windows pronto per egui: file, indice nella collezione, contenuto proiettato in memoria.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemFont {
    pub file: &'static str,
    pub index: u32,
    pub bytes: &'static [u8],
}

fn windows_fonts_dir() -> Option<std::path::PathBuf> {
    let windir = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot"))?;
    Some(std::path::PathBuf::from(windir).join("Fonts"))
}

/// Contenuto di un font di Windows, una volta sola per processo: proiettato in memoria
/// (`system::map_font`), o letto e tenuto se la proiezione non riesce.
fn system_font_bytes(file: &'static str) -> Result<&'static [u8], String> {
    static OPEN: Mutex<Vec<(&'static str, &'static [u8])>> = Mutex::new(Vec::new());
    let mut open = OPEN.lock().map_err(|e| e.to_string())?;
    if let Some(&(_, b)) = open.iter().find(|(f, _)| *f == file) {
        return Ok(b);
    }
    let path = windows_fonts_dir().ok_or("WINDIR non definita")?.join(file);
    let bytes = match quado_scan_win::system::map_font(&path) {
        Ok(b) => b,
        Err(code) => {
            log::info!(target: "quado", "font {file} non proiettato (codice {code}): lo leggo");
            let v = std::fs::read(&path).map_err(|e| e.to_string())?;
            // Resta per tutto il processo, come la proiezione: si legge una volta sola (`OPEN`).
            &*Box::leak(v.into_boxed_slice())
        }
    };
    open.push((file, bytes));
    Ok(bytes)
}

/// Il font di Windows `file` di [`SYSTEM_FALLBACKS`], se c'è e si apre.
pub fn system_font(file: &str) -> Result<SystemFont, String> {
    let &(file, index, _) = SYSTEM_FALLBACKS.iter().find(|(f, ..)| *f == file).ok_or_else(|| format!("{file}: non è un ripiego"))?;
    let bytes = system_font_bytes(file)?;
    skrifa::FontRef::from_index(bytes, index).map_err(|e| format!("{file}: {e}"))?;
    Ok(SystemFont { file, index, bytes })
}

/// Vero se il font ha un glifo per `c`, con la stessa regola di egui (mappa dei caratteri, glifo diverso
/// da `.notdef`).
fn covers(font: &skrifa::FontRef<'_>, c: char) -> bool {
    font.charmap().map(c).is_some_and(|g| g != skrifa::GlyphId::NOTDEF)
}

/// Font di ripiego scelti per un insieme di caratteri.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fallbacks {
    /// Nell'ordine di [`SYSTEM_FALLBACKS`]: quelli già installati più quelli nuovi.
    pub fonts: Vec<SystemFont>,
    /// Caratteri che nessun font di Windows copre: restano quadratini, non si cercano di nuovo.
    pub uncovered: Vec<char>,
    /// Font che non si sono aperti: file ed errore.
    pub missing: Vec<(&'static str, String)>,
    pub ms: f64,
}

/// Il carattere che egui disegna al posto di un glifo mancante: lo prende dal primo font della catena che lo
/// ha (il "font di sostituzione").
pub const REPLACEMENT: char = '◻';

/// Sceglie i font di Windows per i caratteri `missing` (quelli che i font installati non hanno): nell'ordine
/// di [`SYSTEM_FALLBACKS`], un font entra se copre almeno un carattere che nessuno dei precedenti copre.
/// Così ogni carattere usa lo stesso font che userebbe con tutti e sette caricati (il primo che lo copre).
/// Se Inter non ha [`REPLACEMENT`], entra anche il primo font che lo ha, come con tutti e
/// sette: egui considera "senza glifo" ogni carattere che cade sul font di sostituzione, e senza questo un
/// font scelto per una scrittura (Leelawadee UI per il thai) diventerebbe quello di sostituzione.
/// `have` sono i font già installati: restano. Fuori dal thread dell'interfaccia: apre i file.
pub fn choose_fallbacks(missing: &[char], have: &[SystemFont]) -> Fallbacks {
    let t = Instant::now();
    let mut out = pick(missing, have);
    // Inter apre tutte le catene dei dati (Plex Mono ripiega su Inter): se ha il carattere di sostituzione, è
    // lui il font di sostituzione. Altrimenti, se serve almeno un ripiego, si sceglie anche per quello.
    let inter_has = |c: char| skrifa::FontRef::new(INTER).is_ok_and(|f| covers(&f, c));
    if !out.fonts.is_empty() && !missing.contains(&REPLACEMENT) && !inter_has(REPLACEMENT) {
        let mut with: Vec<char> = missing.to_vec();
        with.push(REPLACEMENT);
        let mut again = pick(&with, have);
        again.uncovered.retain(|&c| c != REPLACEMENT);
        out = again;
    }
    out.ms = t.elapsed().as_secs_f64() * 1000.0;
    out
}

/// La scelta di [`choose_fallbacks`] per i caratteri dati.
fn pick(chars: &[char], have: &[SystemFont]) -> Fallbacks {
    let mut out = Fallbacks::default();
    let mut left: Vec<char> = chars.to_vec();
    for &(file, _, _) in SYSTEM_FALLBACKS {
        let installed = have.iter().find(|f| f.file == file).copied();
        if left.is_empty() && installed.is_none() {
            continue;
        }
        let font = match installed.map_or_else(|| system_font(file), Ok) {
            Ok(f) => f,
            Err(e) => {
                out.missing.push((file, e));
                continue;
            }
        };
        let Ok(face) = skrifa::FontRef::from_index(font.bytes, font.index) else { continue };
        let before = left.len();
        left.retain(|&c| !covers(&face, c));
        if installed.is_some() || left.len() < before {
            out.fonts.push(font);
        }
    }
    out.uncovered = left;
    out
}

impl SystemFont {
    /// Vero se il font ha un glifo per `c` (la regola di egui, senza il suo contesto).
    pub fn covers(&self, c: char) -> bool {
        skrifa::FontRef::from_index(self.bytes, self.index).is_ok_and(|f| covers(&f, c))
    }
}

/// Costruisce le definizioni dei font. Separata da [`install`] per poterla provare senza contesto.
pub fn definitions(opts: FontOptions) -> (FontDefinitions, FontReport) {
    let start = Instant::now();
    let mut report = FontReport::default();
    let mut fallbacks: Vec<SystemFont> = Vec::new();
    let mut segoe = None;
    for &(file, _, _) in SYSTEM_FALLBACKS {
        let wanted = opts.system_fallbacks || (opts.segoe_family && file == "segoeui.ttf");
        if !wanted {
            continue;
        }
        match system_font(file) {
            Ok(f) => {
                report.system.push((file.to_owned(), f.bytes.len()));
                if file == "segoeui.ttf" {
                    segoe = Some(f);
                }
                if opts.system_fallbacks {
                    fallbacks.push(f);
                }
            }
            Err(e) => {
                log::warn!("font di sistema {file}: {e}");
                report.missing.push(file.to_owned());
            }
        }
    }
    let defs = build(&fallbacks, segoe.filter(|_| opts.segoe_family));
    report.load_ms = start.elapsed().as_secs_f64() * 1000.0;
    (defs, report)
}

/// Definizioni con i font incorporati, i ripieghi di Windows dati e, se c'è, la famiglia Segoe UI del confronto.
fn build(fallbacks: &[SystemFont], segoe: Option<SystemFont>) -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    let builtin: Vec<String> = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();

    for (name, bytes) in [
        ("inter-light", INTER_LIGHT),
        ("inter", INTER),
        ("inter-medium", INTER_MEDIUM),
        ("inter-semibold", INTER_SEMIBOLD),
        ("plex-mono", PLEX_MONO),
        ("plex-mono-medium", PLEX_MONO_MEDIUM),
    ] {
        defs.font_data.insert(name.to_owned(), FontData::from_static(bytes).into());
    }
    // `from_static`: egui usa il contenuto proiettato senza copiarlo (con `from_owned` lo copiava una volta
    // in più, e i 52 MB dei sette font diventavano 100 MiB di memoria privata).
    let mut add = |f: SystemFont| -> String {
        let key = format!("sys:{}", f.file);
        let mut data = FontData::from_static(f.bytes);
        data.index = f.index;
        defs.font_data.insert(key.clone(), data.into());
        key
    };
    let keys: Vec<String> = fallbacks.iter().map(|&f| add(f)).collect();
    let segoe_key = segoe.map(&mut add);

    // Plex Mono ripiega su Inter prima che sui font di sistema: un carattere che manca al mono (greco,
    // cirillico esteso) resta nello stesso disegno del resto dell'interfaccia.
    let chain = |primary: &[&str]| -> Vec<String> {
        let mut v: Vec<String> = primary.iter().map(|s| (*s).to_owned()).collect();
        v.extend(keys.iter().cloned());
        v.extend(builtin.iter().cloned());
        v
    };
    defs.families.insert(FontFamily::Proportional, chain(&["inter"]));
    defs.families.insert(FontFamily::Monospace, chain(&["plex-mono", "inter"]));
    defs.families.insert(light(), chain(&["inter-light"]));
    defs.families.insert(medium(), chain(&["inter-medium"]));
    defs.families.insert(semibold(), chain(&["inter-semibold"]));
    defs.families.insert(mono_medium(), chain(&["plex-mono-medium", "inter-medium"]));
    match segoe_key {
        Some(k) => defs.families.insert(segoe_egui(), chain(&[k.as_str()])),
        // La famiglia deve esistere anche senza Segoe UI, altrimenti egui va in panico al primo uso.
        None => defs.families.insert(segoe_egui(), chain(&["inter"])),
    };
    defs
}

/// Installa i font nel contesto egui.
pub fn install(ctx: &egui::Context, opts: FontOptions) -> FontReport {
    let (defs, report) = definitions(opts);
    ctx.set_fonts(defs);
    report
}

/// Installa i font incorporati con i ripieghi di Windows dati ([`choose_fallbacks`]). Valgono dal frame
/// successivo.
pub fn install_fallbacks(ctx: &egui::Context, fallbacks: &[SystemFont]) {
    ctx.set_fonts(build(fallbacks, None));
}

/// Vero se un carattere di `names` manca a una delle famiglie dei dati (Inter e Plex Mono, con i loro
/// ripieghi incorporati): allora servono i font di Windows. I caratteri di controllo non contano.
pub fn needs_fallbacks<S: AsRef<str>>(ctx: &egui::Context, names: impl Iterator<Item = S>) -> bool {
    let fonts: Vec<FontId> = data_families().into_iter().map(|f| FontId::new(13.0, f)).collect();
    ctx.fonts_mut(|f| {
        let mut names = names;
        names.any(|n| n.as_ref().chars().any(|c| !c.is_control() && fonts.iter().any(|font| !f.has_glyph(font, c))))
    })
}

/// I caratteri di `chars` che mancano a una delle famiglie dei dati con i font installati adesso.
pub fn missing_glyphs(ctx: &egui::Context, chars: impl Iterator<Item = char>) -> Vec<char> {
    let fonts: Vec<FontId> = data_families().into_iter().map(|f| FontId::new(13.0, f)).collect();
    ctx.fonts_mut(|f| chars.filter(|&c| !c.is_control() && fonts.iter().any(|font| !f.has_glyph(font, c))).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Esegue `f` dentro un frame di un contesto con i font di Quado. Le modifiche all'atlante dei glifi
    /// prodotte dai frame si scartano: qui non c'è un renderer che le applichi.
    fn with_fonts<R>(f: impl FnOnce(&egui::Context) -> R) -> R {
        let ctx = egui::Context::default();
        install(&ctx, FontOptions { system_fallbacks: false, segoe_family: false });
        // I font installati valgono dal frame successivo.
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        let mut out = None;
        let mut f = Some(f);
        ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(f) = f.take() {
                out = Some(f(ui.ctx()));
            }
        })
        .textures_delta
        .clear();
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        out.expect("frame eseguito")
    }

    #[test]
    fn inter_families_come_first() {
        let (defs, _) = definitions(FontOptions { system_fallbacks: false, segoe_family: false });
        assert_eq!(defs.families[&FontFamily::Proportional][0], "inter");
        assert_eq!(defs.families[&FontFamily::Monospace][0], "plex-mono");
        assert_eq!(defs.families[&light()][0], "inter-light");
        assert_eq!(defs.families[&medium()][0], "inter-medium");
        assert_eq!(defs.families[&semibold()][0], "inter-semibold");
        assert!(defs.families.contains_key(&segoe_egui()));
        // IBM Plex Sans non è più nell'eseguibile.
        assert!(defs.font_data.keys().all(|k| !k.starts_with("plex-sans")));
    }

    #[test]
    fn digits_are_tabular_in_every_weight() {
        // Le cifre di Inter sono proporzionali; i file -tnum hanno quelle tabulari: dieci larghezze uguali
        // in ogni peso, anche come le dispone egui (con la crenatura, se c'è).
        with_fonts(|ctx| {
            for family in [FontFamily::Proportional, light(), medium(), semibold()] {
                let font = FontId::new(20.0, family.clone());
                let widths: Vec<f32> = ctx.fonts_mut(|f| ('0'..='9').map(|c| f.glyph_width(&font, c)).collect());
                let (min, max) = widths.iter().fold((f32::MAX, f32::MIN), |(a, b), &w| (a.min(w), b.max(w)));
                assert!(max - min < 0.01, "{family:?}: larghezze delle cifre {widths:?}");
                let run = |s: &str| ctx.fonts_mut(|f| f.layout_no_wrap(s.to_owned(), font.clone(), egui::Color32::WHITE).size().x);
                assert!((run("1111111111") - run("0000000000")).abs() < 0.05, "{family:?}: righe di cifre diverse");
                assert!((run("476 GB") - run("111 GB")).abs() < 0.05, "{family:?}: numeri diversi");
            }
        });
    }

    #[test]
    fn fallback_check_sees_missing_scripts() {
        with_fonts(|ctx| {
            assert!(!needs_fallbacks(ctx, ["Program Files", "Ελληνικά", "Кириллица", "Café"].into_iter()));
            assert!(needs_fallbacks(ctx, ["文件夹"].into_iter()));
            assert!(needs_fallbacks(ctx, ["Downloads", "עברית"].into_iter()));
            assert_eq!(missing_glyphs(ctx, "Café עב".chars()), ['ע', 'ב']);
        });
    }

    /// Le prove che leggono `%WINDIR%\Fonts` non hanno senso senza i font di Windows.
    fn windows_fonts() -> bool {
        windows_fonts_dir().is_some_and(|d| SYSTEM_FALLBACKS.iter().all(|(f, ..)| d.join(f).is_file()))
    }

    /// Il primo font di `fonts` che copre `c`, come la catena di egui.
    fn first_covering(fonts: &[SystemFont], c: char) -> Option<&'static str> {
        fonts.iter().find(|f| skrifa::FontRef::from_index(f.bytes, f.index).is_ok_and(|face| covers(&face, c))).map(|f| f.file)
    }

    #[test]
    fn only_fonts_for_the_missing_characters_are_chosen() {
        if !windows_fonts() {
            return;
        }
        // Ebraico e thai: Segoe UI e Leelawadee UI, più il font del carattere di sostituzione se Inter non lo
        // ha; non i font cinesi, giapponesi e coreani (decine di MB).
        let all: Vec<SystemFont> = SYSTEM_FALLBACKS.iter().filter_map(|(f, ..)| system_font(f).ok()).collect();
        let r = choose_fallbacks(&['ש', 'ก'], &[]);
        let files: Vec<&str> = r.fonts.iter().map(|f| f.file).collect();
        let mut want = vec!["segoeui.ttf", "LeelawUI.ttf"];
        if let Some(f) = first_covering(&all, REPLACEMENT).filter(|_| !skrifa::FontRef::new(INTER).is_ok_and(|f| covers(&f, REPLACEMENT))) {
            want.push(f);
        }
        want.sort_by_key(|f| SYSTEM_FALLBACKS.iter().position(|(x, ..)| x == f));
        want.dedup();
        assert_eq!(files, want);
        assert!(r.uncovered.is_empty() && r.missing.is_empty());
        assert!(choose_fallbacks(&[], &[]).fonts.is_empty());
        // I font già installati restano, nell'ordine della catena.
        let more = choose_fallbacks(&['漢'], &r.fonts);
        assert_eq!(more.fonts.first().map(|f| f.file), Some("segoeui.ttf"));
        assert_eq!(more.fonts.last().map(|f| f.file), Some("LeelawUI.ttf"));
        assert!(more.fonts.len() == r.fonts.len() + 1 && first_covering(&more.fonts, '漢').is_some());
        // Un carattere che nessun font copre (uso privato) resta scoperto e non fa caricare niente.
        let none = choose_fallbacks(&['\u{10FFFD}'], &[]);
        assert_eq!(none.uncovered, ['\u{10FFFD}']);
        assert!(none.fonts.is_empty());
    }

    #[test]
    fn each_character_keeps_the_font_it_had_with_all_seven() {
        if !windows_fonts() {
            return;
        }
        // Con tutti e sette caricati un carattere usa il primo font che lo copre: con la scelta, lo stesso.
        let all: Vec<SystemFont> = SYSTEM_FALLBACKS.iter().filter_map(|(f, ..)| system_font(f).ok()).collect();
        assert_eq!(all.len(), SYSTEM_FALLBACKS.len());
        let samples: [&[char]; 6] = [&['ש', 'ก', 'か', '한', 'क', '漢', '☯', 'ع'], &['か'], &['漢', 'か'], &['한', 'ש'], &['क', 'ก'], &['☯']];
        for chars in samples {
            let r = choose_fallbacks(chars, &[]);
            for &c in chars {
                assert_eq!(first_covering(&r.fonts, c), first_covering(&all, c), "{c}");
            }
            // Nessun font in più: ognuno è il primo per almeno un carattere, o per quello di sostituzione.
            assert_eq!(first_covering(&r.fonts, REPLACEMENT), first_covering(&all, REPLACEMENT));
            for f in &r.fonts {
                let needed = chars.iter().chain([&REPLACEMENT]).any(|&c| first_covering(&all, c) == Some(f.file));
                assert!(needed, "{} non serve a {chars:?}", f.file);
            }
        }
    }

    #[test]
    fn a_name_in_a_rare_script_is_still_drawn() {
        if !windows_fonts() {
            return;
        }
        let name = "ภาษาไทย שלום";
        let ctx = egui::Context::default();
        install(&ctx, FontOptions { system_fallbacks: false, segoe_family: false });
        let frame = |f: &mut dyn FnMut(&egui::Context)| ctx.run_ui(egui::RawInput::default(), |ui| f(ui.ctx())).textures_delta.clear();
        frame(&mut |_| {});
        let mut missing = Vec::new();
        frame(&mut |ctx| missing = missing_glyphs(ctx, name.chars()));
        assert!(!missing.is_empty());
        let r = choose_fallbacks(&missing, &[]);
        assert!(r.uncovered.is_empty());
        install_fallbacks(&ctx, &r.fonts);
        frame(&mut |_| {});
        frame(&mut |ctx| missing = missing_glyphs(ctx, name.chars()));
        assert!(missing.is_empty(), "ancora senza glifi: {missing:?}");
        frame(&mut |ctx| assert!(!needs_fallbacks(ctx, [name].into_iter())));
    }
}
