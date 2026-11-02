// SPDX-License-Identifier: GPL-3.0-or-later
//! Lingue dell'interfaccia: file Fluent, uno per lingua.
//!
//! - Incorporati: `lingue/it.ftl` (riferimento, con un commento di contesto per chiave) e `lingue/en.ftl`.
//! - Modificabili senza ricompilare: i file in `%APPDATA%\Quado\lingue\<tag>.ftl` sostituiscono le
//!   chiavi di una lingua incorporata o aggiungono una lingua nuova. Le righe con errori si ignorano;
//!   gli errori si elencano in Impostazioni.
//! - Ripiego per chiave: lingua scelta, poi inglese, poi italiano.
//! - Numeri e date non passano da Fluent: li scrive `format` con il formato dell'area geografica.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

include!(concat!(env!("OUT_DIR"), "/lingue.rs"));

/// Testo di un file di lingua incorporato.
pub fn embedded(tag: &str) -> Option<&'static str> {
    EMBEDDED.iter().find(|(t, _)| *t == tag).map(|(_, s)| *s)
}

/// La lingua di riferimento, con un commento di contesto per ogni chiave.
pub const REFERENCE: &str = "it";

/// Lingua usata dai test per trovare testi scritti nel codice e testi tagliati.
pub const PSEUDO: &str = "x-pseudo";

/// Valore di un segnaposto.
#[derive(Clone, Debug)]
pub enum Arg<'a> {
    /// Testo già pronto (numeri formattati, nomi, percorsi).
    Str(Cow<'a, str>),
    /// Numero per la scelta del plurale (`[one]`, `[other]`); non si mostra così com'è.
    Num(f64),
}

impl<'a> From<&'a str> for Arg<'a> {
    fn from(s: &'a str) -> Self {
        Self::Str(Cow::Borrowed(s))
    }
}

impl From<String> for Arg<'_> {
    fn from(s: String) -> Self {
        Self::Str(Cow::Owned(s))
    }
}

impl From<u64> for Arg<'_> {
    fn from(n: u64) -> Self {
        Self::Num(n as f64)
    }
}

impl From<usize> for Arg<'_> {
    fn from(n: usize) -> Self {
        Self::Num(n as f64)
    }
}

/// Una lingua disponibile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Language {
    pub tag: String,
    /// Nome della lingua nella lingua stessa (chiave `lingua-nome`).
    pub name: String,
    /// Viene (anche) da un file in `%APPDATA%\Quado\lingue`.
    pub user_file: bool,
}

/// Errore in un file di lingua dell'utente: il file, la riga e il testo dell'errore di Fluent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileError {
    pub file: PathBuf,
    pub line: usize,
    pub detail: String,
}

type Bundle = FluentBundle<FluentResource>;
/// Per tag: il file incorporato, se c'è, e i file dell'utente (testo, percorso).
type Sources = HashMap<String, (Option<String>, Vec<(String, PathBuf)>)>;

pub struct Lingue {
    selected: String,
    chain: Vec<Bundle>,
    pub available: Vec<Language>,
    pub errors: Vec<FileError>,
    cache: RefCell<HashMap<String, String>>,
}

fn langid(tag: &str) -> LanguageIdentifier {
    tag.parse().unwrap_or_default()
}

/// Testi della pseudo-lingua: accenti, segni di inizio e fine, il 40% più lunghi.
pub fn pseudo(s: &str) -> Cow<'_, str> {
    if s.trim().is_empty() {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() * 2);
    out.push('[');
    let mut letters: usize = 0;
    for c in s.chars() {
        let m = match c {
            'a' => 'á',
            'e' => 'é',
            'i' => 'í',
            'o' => 'ó',
            'u' => 'ú',
            'c' => 'ç',
            'n' => 'ñ',
            'A' => 'Á',
            'E' => 'É',
            'I' => 'Í',
            'O' => 'Ó',
            'U' => 'Ú',
            'C' => 'Ç',
            'N' => 'Ñ',
            other => other,
        };
        if c.is_alphanumeric() {
            letters += 1;
        }
        out.push(m);
    }
    for _ in 0..(letters * 2).div_ceil(5) {
        out.push('~');
    }
    out.push(']');
    Cow::Owned(out)
}

/// Righe (da 1) dei byte di inizio degli errori di Fluent.
fn line_of(source: &str, byte: usize) -> usize {
    source[..byte.min(source.len())].matches('\n').count() + 1
}

fn parse(source: String, file: Option<&Path>, errors: &mut Vec<FileError>) -> FluentResource {
    match FluentResource::try_new(source) {
        Ok(r) => r,
        Err((r, errs)) => {
            if let Some(f) = file {
                for e in errs {
                    errors.push(FileError { file: f.to_owned(), line: line_of(r.source(), e.pos.start), detail: format!("{:?}", e.kind) });
                }
            }
            r
        }
    }
}

fn new_bundle(tag: &str) -> Bundle {
    let mut b = FluentBundle::new(vec![langid(tag)]);
    // Niente caratteri di isolamento bidirezionale attorno ai segnaposto: egui li disegnerebbe.
    b.set_use_isolating(false);
    b
}

/// File di lingua dell'utente: `(tag, percorso)`, ordinati per tag.
fn user_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<(String, PathBuf)> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ftl")))
        .filter_map(|p| Some((p.file_stem()?.to_str()?.to_owned(), p)))
        .filter(|(t, _)| t.parse::<LanguageIdentifier>().is_ok())
        .collect();
    v.sort();
    v
}

/// Lingua da usare al primo avvio: la prima delle lingue di Windows disponibile, altrimenti inglese.
pub fn first_run_language(windows: &[String], available: &[Language]) -> String {
    for w in windows {
        let primary = w.split('-').next().unwrap_or("").to_ascii_lowercase();
        if let Some(l) = available.iter().find(|l| l.tag.eq_ignore_ascii_case(w) || l.tag.eq_ignore_ascii_case(&primary)) {
            return l.tag.clone();
        }
    }
    "en".to_owned()
}

impl Lingue {
    /// Carica le lingue. `user_dir` è `%APPDATA%\Quado\lingue` (può non esistere).
    pub fn load(selected: &str, user_dir: Option<&Path>) -> Self {
        let mut errors = Vec::new();
        let users = user_dir.map(user_files).unwrap_or_default();
        let mut sources: Sources = HashMap::new();
        for (tag, src) in EMBEDDED {
            sources.entry((*tag).to_owned()).or_default().0 = Some((*src).to_owned());
        }
        for (tag, path) in &users {
            match std::fs::read_to_string(path) {
                Ok(s) => sources.entry(tag.clone()).or_default().1.push((s, path.clone())),
                Err(e) => errors.push(FileError { file: path.clone(), line: 0, detail: e.to_string() }),
            }
        }

        let build = |tag: &str, errors: &mut Vec<FileError>| -> Bundle {
            let mut b = new_bundle(tag);
            if tag == PSEUDO {
                b.set_transform(Some(pseudo));
                // Pseudo-lingua costruita sull'italiano, la lingua di riferimento.
                if let Some((Some(src), _)) = sources.get("it") {
                    let _ = b.add_resource(parse(src.clone(), None, errors));
                }
                return b;
            }
            if let Some((embedded, user)) = sources.get(tag) {
                if let Some(src) = embedded {
                    let _ = b.add_resource(parse(src.clone(), None, errors));
                }
                for (src, path) in user {
                    b.add_resource_overriding(parse(src.clone(), Some(path), errors));
                }
            }
            b
        };

        let mut tags: Vec<String> = sources.keys().cloned().collect();
        tags.sort();
        let mut available = Vec::new();
        for t in &tags {
            let b = build(t, &mut Vec::new());
            let name = message(&b, "lingua-nome", None).unwrap_or_else(|| t.clone());
            available.push(Language { tag: t.clone(), name, user_file: users.iter().any(|(u, _)| u == t) });
        }
        // Gli errori dei file dell'utente si raccolgono una volta sola.
        for t in &tags {
            if let Some((_, user)) = sources.get(t) {
                for (src, path) in user {
                    parse(src.clone(), Some(path), &mut errors);
                }
            }
        }

        let selected = if selected == PSEUDO || tags.iter().any(|t| t == selected) { selected.to_owned() } else { "en".to_owned() };
        let mut chain = vec![build(&selected, &mut Vec::new())];
        for fallback in ["en", "it"] {
            if fallback != selected {
                chain.push(build(fallback, &mut Vec::new()));
            }
        }
        Self { selected, chain, available, errors, cache: RefCell::new(HashMap::new()) }
    }

    pub fn selected(&self) -> &str {
        &self.selected
    }

    /// Testo di una chiave senza segnaposto.
    pub fn t(&self, key: &str) -> String {
        if let Some(s) = self.cache.borrow().get(key) {
            return s.clone();
        }
        let s = self.format(key, None);
        self.cache.borrow_mut().insert(key.to_owned(), s.clone());
        s
    }

    /// Testo di una chiave con segnaposto.
    pub fn ta(&self, key: &str, args: &[(&str, Arg<'_>)]) -> String {
        let mut fa = FluentArgs::new();
        for (k, v) in args {
            match v {
                Arg::Str(s) => fa.set(*k, FluentValue::from(s.as_ref())),
                Arg::Num(n) => fa.set(*k, FluentValue::from(*n)),
            }
        }
        self.format(key, Some(&fa))
    }

    fn format(&self, key: &str, args: Option<&FluentArgs<'_>>) -> String {
        for b in &self.chain {
            if let Some(s) = message(b, key, args) {
                return s;
            }
        }
        format!("[{key}]")
    }

    /// `true` se la chiave esiste in almeno una lingua della catena.
    pub fn has(&self, key: &str) -> bool {
        self.chain.iter().any(|b| b.has_message(key))
    }
}

fn message(b: &Bundle, key: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
    let (id, attr) = match key.split_once('.') {
        Some((i, a)) => (i, Some(a)),
        None => (key, None),
    };
    let msg = b.get_message(id)?;
    let pattern = match attr {
        Some(a) => msg.get_attribute(a)?.value(),
        None => msg.value()?,
    };
    let mut errs = Vec::new();
    let s = b.format_pattern(pattern, args, &mut errs);
    Some(s.into_owned())
}

/// Chiavi e segnaposto di un file, per i controlli di completezza (`cargo xtask lingue`).
pub mod check {
    use std::collections::{BTreeMap, BTreeSet};

    use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};

    /// Per ogni chiave (anche `chiave.attributo`): i segnaposto usati e se ha un commento.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct KeyInfo {
        pub vars: BTreeSet<String>,
        pub commented: bool,
    }

    pub struct FileReport {
        pub keys: BTreeMap<String, KeyInfo>,
        /// Errori di sintassi: (riga, descrizione).
        pub errors: Vec<(usize, String)>,
    }

    fn vars_of(p: &Pattern<&str>, out: &mut BTreeSet<String>) {
        for e in &p.elements {
            if let PatternElement::Placeable { expression } = e {
                vars_of_expr(expression, out);
            }
        }
    }

    fn vars_of_expr(e: &Expression<&str>, out: &mut BTreeSet<String>) {
        match e {
            Expression::Inline(i) => vars_of_inline(i, out),
            Expression::Select { selector, variants } => {
                vars_of_inline(selector, out);
                for v in variants {
                    vars_of(&v.value, out);
                }
            }
        }
    }

    fn vars_of_inline(i: &InlineExpression<&str>, out: &mut BTreeSet<String>) {
        match i {
            InlineExpression::VariableReference { id } => {
                out.insert(id.name.to_owned());
            }
            InlineExpression::Placeable { expression } => vars_of_expr(expression, out),
            _ => {}
        }
    }

    pub fn analyze(source: &str) -> FileReport {
        // Il parser completo: quello di `FluentResource` scarta i commenti.
        let (res, errors) = match fluent_syntax::parser::parse(source) {
            Ok(r) => (r, Vec::new()),
            Err((r, errs)) => {
                let lines = errs.iter().map(|e| (super::line_of(source, e.pos.start), format!("{:?}", e.kind))).collect();
                (r, lines)
            }
        };
        let mut keys = BTreeMap::new();
        for entry in &res.body {
            if let Entry::Message(m) = entry {
                let commented = m.comment.is_some();
                if let Some(v) = &m.value {
                    let mut vars = BTreeSet::new();
                    vars_of(v, &mut vars);
                    keys.insert(m.id.name.to_owned(), KeyInfo { vars, commented });
                }
                for a in &m.attributes {
                    let mut vars = BTreeSet::new();
                    vars_of(&a.value, &mut vars);
                    keys.insert(format!("{}.{}", m.id.name, a.id.name), KeyInfo { vars, commented });
                }
            }
        }
        FileReport { keys, errors }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_languages_are_complete_and_aligned() {
        let it = check::analyze(embedded("it").expect("it.ftl"));
        let en = check::analyze(embedded("en").expect("en.ftl"));
        assert!(it.errors.is_empty(), "it.ftl: {:?}", it.errors);
        assert!(en.errors.is_empty(), "en.ftl: {:?}", en.errors);
        let ik: Vec<&String> = it.keys.keys().collect();
        let ek: Vec<&String> = en.keys.keys().collect();
        assert_eq!(ik, ek, "it ed en devono avere le stesse chiavi");
        for (k, info) in &it.keys {
            assert_eq!(info.vars, en.keys[k].vars, "segnaposto diversi in {k}");
            assert!(info.commented, "{k}: manca il commento di contesto in it.ftl");
        }
    }

    /// Ogni chiave scritta nel codice esiste in `it.ftl` (le famiglie si compongono da `famiglia-<codice>`).
    #[test]
    fn every_key_used_in_code_exists() {
        // Stringhe con trattini che non sono chiavi: identificatori di egui, thread, token, segnaposto.
        const NOT_KEYS: &[&str] = &[
            "barra-laterale",
            "barra-stato",
            "barra-titolo",
            "casella-ricerca",
            // Identificativi di regole, non chiavi.
            "cestino-orfane",
            "file-grandi",
            "ink-soft",
            "nel-cloud",
            "npm-cache",
            // Cartella dei dati sintetici (`synthetic.rs`).
            "progetto-rust",
            "quado-lavoro",
            "quado-prove",
            "quado-scansione",
            "tabella-esplora",
            "tabella-file-grandi",
            "tabella-ricerca",
            "text-2",
            "text-3",
        ];
        let it = check::analyze(embedded("it").expect("it.ftl"));
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = vec![dir.join("widgets.rs"), dir.join("tokens.rs"), dir.join("format.rs")];
        for e in std::fs::read_dir(dir.join("app")).expect("src/app") {
            files.push(e.expect("voce").path());
        }
        let mut missing = Vec::new();
        for f in files {
            let src = std::fs::read_to_string(&f).expect("sorgente");
            for lit in src.split('"').skip(1).step_by(2) {
                let is_key = lit.len() > 2
                    && lit.contains('-')
                    && lit.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                    && lit.starts_with(|c: char| c.is_ascii_lowercase())
                    && !lit.ends_with('-');
                if is_key && !NOT_KEYS.contains(&lit) && !it.keys.contains_key(lit) {
                    missing.push(format!("{}: {lit}", f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
                }
            }
        }
        for fam in quado_model::Family::ALL {
            let k = format!("famiglia-{}", fam.code());
            if !it.keys.contains_key(&k) {
                missing.push(k);
            }
        }
        assert!(missing.is_empty(), "chiavi mancanti in it.ftl: {missing:?}");
    }

    /// Nessuna stringa della card dei conti attribuisce una causa allo spazio mancante.
    #[test]
    fn reconciliation_card_has_no_causal_sentence() {
        let causal = ["perché", "a causa", "dovut", "causat", "per via", "because", "due to", "caused", "since "];
        for tag in ["it", "en"] {
            let src = embedded(tag).expect("lingua");
            for line in src.lines().filter(|l| l.starts_with("conti-") || l.starts_with("bilancio-non-attribuito")) {
                if line.starts_with("conti-titolo") {
                    continue;
                }
                let v = line.to_lowercase();
                for c in causal {
                    assert!(!v.contains(c), "{tag}: «{line}» contiene «{c}»");
                }
            }
        }
    }

    #[test]
    fn forbidden_words_are_absent() {
        // Parole vietate (docs/testi.md §3): non devono comparire nei testi.
        let forbidden_it = [
            "sicuro da eliminare",
            "sicuro da disinstallare",
            "puoi ripristinare tutto",
            "inutilizzato",
            "mai usato",
            "non più in uso",
            "spazio sprecato",
            "spazzatura",
            "ottimizza",
            "pulizia intelligente",
        ];
        let forbidden_en = ["safe to delete", "safe to uninstall", "restore everything", "unused", "never used", "wasted space", "junk", "optimize"];
        for (tag, src) in EMBEDDED {
            let lower = src.to_lowercase();
            // Solo i valori: i commenti spiegano proprio cosa non scrivere.
            let values: String = lower.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join("\n");
            for w in forbidden_it.iter().chain(&forbidden_en) {
                assert!(!values.contains(w), "{tag}.ftl contiene «{w}»");
            }
            assert!(!values.contains('!'), "{tag}.ftl: niente punti esclamativi");
            assert!(!values.contains('\u{2014}') && !values.contains('\u{2013}'), "{tag}.ftl: niente trattini lunghi");
        }
    }

    #[test]
    fn fallback_and_user_overrides() {
        let dir = std::env::temp_dir().join(format!("quado-lingue-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("cartella");
        // Una lingua nuova, incompleta e con una riga sbagliata; e una correzione dell'italiano.
        std::fs::write(dir.join("fr.ftl"), "lingua-nome = Français\nnav-panoramica = Vue d'ensemble\nriga sbagliata\n").expect("fr");
        std::fs::write(dir.join("it.ftl"), "nav-panoramica = Quadro\n").expect("it");
        let l = Lingue::load("fr", Some(&dir));
        assert_eq!(l.t("nav-panoramica"), "Vue d'ensemble");
        // Chiave mancante in francese: ripiego sull'inglese.
        assert_eq!(l.t("nav-esplora"), Lingue::load("en", None).t("nav-esplora"));
        assert!(l.errors.iter().any(|e| e.file.ends_with("fr.ftl") && e.line == 3), "{:?}", l.errors);
        assert!(l.available.iter().any(|a| a.tag == "fr" && a.name == "Français" && a.user_file));
        let it = Lingue::load("it", Some(&dir));
        assert_eq!(it.t("nav-panoramica"), "Quadro");
        assert_eq!(it.t("nav-esplora"), Lingue::load("it", None).t("nav-esplora"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn plurals_follow_the_language() {
        let it = Lingue::load("it", None);
        let en = Lingue::load("en", None);
        let one = it.ta("conteggio-file", &[("n", 1u64.into()), ("testo", "1".into())]);
        let many = it.ta("conteggio-file", &[("n", 3u64.into()), ("testo", "3".into())]);
        assert_ne!(one, many);
        assert!(en.ta("conteggio-file", &[("n", 1u64.into()), ("testo", "1".into())]).contains('1'));
    }

    #[test]
    fn first_run_picks_the_windows_language() {
        let l = Lingue::load("it", None);
        assert_eq!(first_run_language(&["it-IT".into()], &l.available), "it");
        assert_eq!(first_run_language(&["en-GB".into()], &l.available), "en");
        assert_eq!(first_run_language(&["ja-JP".into()], &l.available), "en");
        assert_eq!(first_run_language(&["ja-JP".into(), "it-IT".into()], &l.available), "it");
    }

    #[test]
    fn pseudo_marks_and_lengthens() {
        let p = pseudo("Panoramica");
        assert!(p.starts_with('[') && p.ends_with(']'));
        assert!(p.chars().count() as f64 >= "Panoramica".len() as f64 * 1.4);
        let l = Lingue::load(PSEUDO, None);
        assert!(l.t("nav-panoramica").starts_with('['));
    }
}
