// SPDX-License-Identifier: GPL-3.0-or-later
//! Lente Download (03 F-DL, 05 D6): famiglie secondo l'estensione, suffisso "(N)" come solo filtro,
//! file della stessa dimensione da confrontare, archivi con una cartella omonima accanto, download
//! incompleti. Tutto dal modello della scansione: costruire la lente non legge il contenuto di nessun
//! file (F-DL-1). Le letture su richiesta (hash, directory centrale degli zip, Restart Manager) le fa
//! chi chiama, e arrivano qui come valori di questo modulo.
//!
//! I tipi marcati "interfaccia M4" sono condivisi tra i crate: non si cambiano senza accordo.

use std::collections::HashMap;

use crate::kind::Family;
use crate::names::Name;
use crate::tree::{Flags, NONE, Tree};

/// Voce della directory centrale di uno zip (F-DL-5): nome e dimensione, mai il contenuto.
/// Interfaccia M4: la produce `quado_scan_win::zip`, la usa il confronto con la cartella.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ArchiveEntry {
    /// Percorso dentro l'archivio, con `/` come separatore, così come è scritto nell'archivio.
    pub path: String,
    /// Dimensione non compressa dichiarata dall'archivio.
    pub size: u64,
    /// Voce di cartella (nome che finisce con `/`).
    pub dir: bool,
}

/// Uso di un file secondo Restart Manager (F-DL-6). Interfaccia M4: la produce l'interfaccia da
/// `quado_actions::restart`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FileUse {
    /// Nessun processo registrato lo usa.
    Free,
    /// In uso: nomi dei processi come li riporta Restart Manager.
    Used(Vec<String>),
    /// Interrogazione non riuscita: codice di errore di Windows. Non vale come "non in uso".
    Unknown(u32),
}

/// Impronta del contenuto di un file letta su richiesta (F-DL-4). Interfaccia M4: la produce
/// l'interfaccia da `quado_scan_win::content`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hashed {
    pub node: u32,
    /// Dimensione logica al momento della lettura.
    pub size: u64,
    /// SHA-256 del contenuto.
    pub digest: [u8; 32],
    /// Data di creazione (`FILETIME`), per tenere il più vecchio.
    pub created: i64,
}

// ---------------------------------------------------------------------------------------------
// Famiglie

/// Famiglia della lente (F-DL-1): le sei famiglie mostrate più "altro". Il tipo viene solo dal nome,
/// quindi l'interfaccia lo dichiara "secondo l'estensione"; i nomi stanno nei file di lingua
/// (`lente-famiglia-<codice>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LensFamily {
    Installers,
    Archives,
    DiskImages,
    /// Video e audio.
    Media,
    /// Documenti e immagini.
    DocsImages,
    /// Download incompleti: solo `.crdownload` e `.part` (F-DL-6).
    Incomplete,
    /// Tutto il resto: non ha un riquadro, ma entra nei totali.
    Other,
}

/// Estensioni dei download incompleti (Chromium ed Edge `.crdownload`, Firefox `.part`).
const INCOMPLETE: [&[u8]; 2] = [b"crdownload", b"part"];

impl LensFamily {
    /// Le sei famiglie dei riquadri, nell'ordine di 04 §6.6.
    pub const SHOWN: [Self; 6] = [Self::Installers, Self::Archives, Self::DiskImages, Self::Media, Self::DocsImages, Self::Incomplete];

    /// Tutte, nell'ordine di [`Lens::totals`].
    pub const ALL: [Self; 7] = [Self::Installers, Self::Archives, Self::DiskImages, Self::Media, Self::DocsImages, Self::Incomplete, Self::Other];

    /// Codice stabile per le chiavi dei file di lingua (`lente-famiglia-<codice>`) e per le impostazioni.
    pub fn code(self) -> &'static str {
        match self {
            Self::Installers => "installatori",
            Self::Archives => "archivi",
            Self::DiskImages => "immagini-disco",
            Self::Media => "video-audio",
            Self::DocsImages => "documenti-immagini",
            Self::Incomplete => "download-incompleti",
            Self::Other => "altro",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.code() == code)
    }

    /// Posizione in [`Self::ALL`] e in [`Lens::totals`].
    fn index(self) -> usize {
        self as usize
    }

    /// Famiglia di un nome di file. Prima i download incompleti (solo le due estensioni di F-DL-6), poi la
    /// famiglia di [`Family::of_name`]. Senza allocazioni per i nomi ASCII, come `Family::of_name`.
    pub fn of_name(name: Name<'_>) -> Self {
        if is_incomplete(name) {
            return Self::Incomplete;
        }
        match Family::of_name(name) {
            Family::Installers => Self::Installers,
            Family::Archives => Self::Archives,
            Family::DiskImages => Self::DiskImages,
            Family::Video | Family::Audio => Self::Media,
            Family::Documents | Family::Images => Self::DocsImages,
            _ => Self::Other,
        }
    }
}

/// L'estensione (ciò che segue l'ultimo punto, se non è il primo carattere, come in `kind`) è
/// `crdownload` o `part`, in qualunque maiuscolo.
fn is_incomplete(name: Name<'_>) -> bool {
    match name {
        Name::Ascii(s) => match s.iter().rposition(|&b| b == b'.') {
            Some(0) | None => false,
            Some(i) => INCOMPLETE.iter().any(|x| s[i + 1..].eq_ignore_ascii_case(x)),
        },
        Name::Wide(s) => match s.iter().rposition(|&u| u == u16::from(b'.')) {
            Some(0) | None => false,
            Some(i) => {
                let ext = &s[i + 1..];
                let same = |x: &[u8]| ext.len() == x.len() && ext.iter().zip(x).all(|(&u, c)| u < 0x80 && (u as u8).eq_ignore_ascii_case(c));
                INCOMPLETE.iter().any(|x| same(x))
            }
        },
    }
}

// ---------------------------------------------------------------------------------------------
// Suffisso "(N)"

/// Ultime estensioni che Chromium tratta come estensioni doppie (`archivio.tar.gz`), da
/// `base/files/file_path.cc` (`kCommonDoubleExtensionSuffixes`).
const DOUBLE_SUFFIXES: [&[u8]; 5] = [b"gz", b"xz", b"bz2", b"z", b"bz"];

/// Inizio dell'estensione come la calcola Chromium (`FilePath::Extension`), che decide dove inserire
/// " (N)": l'ultimo punto; il penultimo per `user.js` e per `gz`, `xz`, `bz2`, `z`, `bz` preceduti da
/// un'estensione di 1-4 caratteri (`.tar.gz`). Senza punto, la fine del nome; un punto iniziale apre
/// un'estensione che è tutto il nome.
fn extension_start(b: &[u8]) -> usize {
    let Some(last) = b.iter().rposition(|&c| c == b'.') else { return b.len() };
    if last == 0 {
        return 0;
    }
    let Some(pen) = b[..last].iter().rposition(|&c| c == b'.') else { return last };
    if b[pen + 1..].eq_ignore_ascii_case(b"user.js") {
        return pen;
    }
    let ext = &b[last + 1..];
    if DOUBLE_SUFFIXES.iter().any(|s| ext.eq_ignore_ascii_case(s)) && last - pen > 1 && last - pen <= 5 {
        return pen;
    }
    last
}

/// Suffisso " (N)" che Chromium aggiunge prima dell'estensione quando il nome esiste già
/// (`setup (1).exe`, `foto (12).jpg`). Solo un filtro (F-DL-3): non raggruppa, non è una prova, non
/// cambia lo stato.
///
/// Casi decisi:
/// - l'estensione è quella di Chromium: in `archivio (2).tar.gz` il suffisso sta prima di `.tar.gz`
///   (dà 2), in `dati.abcdef (3).gz` prima di `.gz` (dà 3);
/// - un nome senza estensione porta il suffisso in fondo: `LEGGIMI (3)` dà 3;
/// - serve uno spazio prima della parentesi e un nome prima dello spazio: `file(1).exe` (Firefox) e
///   ` (1).exe` non contano;
/// - N è un numero decimale senza zeri iniziali, da 1 a `u32::MAX`: `(0)`, `(01)` e i numeri oltre
///   `u32::MAX` non contano (Chromium usa da 1 a 100, poi un orario; numeri più grandi vengono da
///   altri programmi o da chi rinomina, e restano un filtro).
pub fn copy_suffix(name: &str) -> Option<u32> {
    suffix_of(name.as_bytes())
}

/// Lavora sui byte UTF-8: i caratteri cercati sono ASCII, e un byte ASCII in UTF-8 è sempre un carattere.
fn suffix_of(b: &[u8]) -> Option<u32> {
    let stem = &b[..extension_start(b)];
    let inner = stem.strip_suffix(b")")?;
    let open = inner.iter().rposition(|&c| c == b'(')?;
    let digits = &inner[open + 1..];
    let base = inner[..open].strip_suffix(b" ")?;
    if base.is_empty() || digits.first().is_none_or(|&d| d == b'0') || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

fn suffix_of_name(name: Name<'_>) -> Option<u32> {
    match name {
        Name::Ascii(s) => suffix_of(s),
        Name::Wide(_) => suffix_of(name.to_string_lossy().as_bytes()),
    }
}

// ---------------------------------------------------------------------------------------------
// Lente

/// Un file della lente.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LensItem {
    pub node: u32,
    pub family: LensFamily,
    pub logical: u64,
    pub alloc: u64,
    /// Ultima modifica (`FILETIME`).
    pub modified: i64,
    /// Suffisso " (N)" del nome ([`copy_suffix`]): solo per il filtro.
    pub copy_suffix: Option<u32>,
    /// Segnaposto cloud secondo la scansione (`Flags::CLOUD`): il contenuto non si legge mai (05 D6).
    pub placeholder: bool,
    /// Nome secondario di un hard link (`Flags::SECONDARY_LINK`): gli stessi dati di un altro nome, non
    /// una copia.
    pub secondary: bool,
}

/// Dimensione e numero di file di una famiglia.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FamilyTotal {
    pub family: LensFamily,
    pub files: u64,
    pub logical: u64,
    pub alloc: u64,
}

/// La lente su una cartella (F-DL-1).
///
/// - `items`: tutti i file del sottoalbero, in ordine di nodo, compresi i nomi secondari degli hard link
///   (sono nomi che si vedono nella cartella).
/// - `totals` e i totali: ogni file contato una volta, come nel resto di Quado: i nomi secondari
///   (`Flags::SECONDARY_LINK`) non contano. La somma delle famiglie è il totale.
/// - Esclusi i nodi staccati e i reparse non seguiti (junction, collegamenti simbolici) con ciò che
///   contengono: non sono contenuto della cartella. Per questo `files` può essere minore di
///   `Tree::files(root)` del numero dei collegamenti simbolici a file, che nella scansione pesano 0 byte.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lens {
    pub root: u32,
    pub items: Vec<LensItem>,
    /// Sette voci, nell'ordine di [`LensFamily::ALL`]: le sei famiglie mostrate e "altro".
    pub totals: Vec<FamilyTotal>,
    pub files: u64,
    pub logical: u64,
    pub alloc: u64,
    /// Cartelle del sottoalbero non elencate o elencate in parte (accesso negato, errore, scansione
    /// interrotta, cartella cloud non elencata): se non è 0, i totali sono un minimo.
    pub unread_dirs: u64,
}

/// Flag delle cartelle con un elenco mancante o incompleto.
const UNREAD: Flags = Flags(Flags::DENIED.0 | Flags::ERROR.0 | Flags::PARTIAL.0 | Flags::CLOUD_NOT_LISTED.0);

/// Costruisce la lente su `root` (una cartella; su un file, la lente di quel solo file). Solo il modello
/// della scansione: nessuna lettura di file. Un nodo fuori dall'albero dà una lente vuota.
pub fn build(tree: &Tree, root: u32) -> Lens {
    let mut lens = Lens {
        root,
        totals: LensFamily::ALL.iter().map(|&family| FamilyTotal { family, files: 0, logical: 0, alloc: 0 }).collect(),
        ..Lens::default()
    };
    if root as usize >= tree.len() {
        return lens;
    }
    // I file del sottoalbero secondo l'albero: una sola allocazione nel caso comune.
    lens.items.reserve(tree.files(root) as usize);
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let f = tree.flags(n);
        if f.any(Flags::DETACHED | Flags::NOT_FOLLOWED) {
            continue;
        }
        if f.has(Flags::DIR) {
            if f.any(UNREAD) {
                lens.unread_dirs += 1;
            }
            stack.extend(tree.children(n));
            continue;
        }
        let name = tree.name(n);
        let item = LensItem {
            node: n,
            family: LensFamily::of_name(name),
            logical: tree.logical(n),
            alloc: tree.alloc(n),
            modified: tree.mtime(n),
            copy_suffix: suffix_of_name(name),
            placeholder: f.has(Flags::CLOUD),
            secondary: f.has(Flags::SECONDARY_LINK),
        };
        if !f.has(Flags::SECONDARY_LINK) {
            let t = &mut lens.totals[item.family.index()];
            t.files += 1;
            t.logical += item.logical;
            t.alloc += item.alloc;
            lens.files += 1;
            lens.logical += item.logical;
            lens.alloc += item.alloc;
        }
        lens.items.push(item);
    }
    lens.items.sort_unstable_by_key(|i| i.node);
    lens
}

impl Lens {
    /// Totale di una famiglia.
    pub fn total(&self, family: LensFamily) -> FamilyTotal {
        self.totals.get(family.index()).copied().unwrap_or(FamilyTotal { family, files: 0, logical: 0, alloc: 0 })
    }

    /// File di una famiglia, in ordine di nodo.
    pub fn of_family(&self, family: LensFamily) -> impl Iterator<Item = &LensItem> {
        self.items.iter().filter(move |i| i.family == family)
    }
}

// ---------------------------------------------------------------------------------------------
// File della stessa dimensione e copie identiche (F-DL-4)

/// File della stessa dimensione logica: i soli che "Confronta il contenuto" legge.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SizeGroup {
    pub logical: u64,
    /// In ordine di nodo.
    pub nodes: Vec<u32>,
}

/// Byte che resterebbero liberi tenendo un solo file del gruppo: ordine dei gruppi.
fn recoverable(logical: u64, files: usize) -> u64 {
    logical.saturating_mul(files.saturating_sub(1) as u64)
}

/// Gruppi di almeno due file con la stessa dimensione logica. Esclusi i file vuoti, i segnaposto cloud
/// (il contenuto non si legge), i nomi secondari degli hard link e i download incompleti (cambiano mentre
/// si scaricano): i file di dimensione unica non compaiono, quindi non si leggeranno. Ordine: byte
/// potenzialmente recuperabili decrescenti (dimensione per numero di file meno uno), poi dimensione, poi
/// primo nodo.
pub fn same_size_groups(lens: &Lens) -> Vec<SizeGroup> {
    let mut v: Vec<(u64, u32)> = lens
        .items
        .iter()
        // I nomi secondari degli hard link sono gli stessi dati di un altro nome: leggerli rifarebbe la
        // stessa impronta e il "gruppo" non sarebbe di copie (toglierne uno non libera spazio). Sulla
        // cartella Download di questo PC erano almeno 44.000 nomi su 184.000.
        .filter(|i| i.logical > 0 && !i.placeholder && !i.secondary && i.family != LensFamily::Incomplete)
        .map(|i| (i.logical, i.node))
        .collect();
    v.sort_unstable();
    v.dedup();
    let mut out: Vec<SizeGroup> = v
        .chunk_by(|a, b| a.0 == b.0)
        .filter(|run| run.len() >= 2)
        .map(|run| SizeGroup { logical: run[0].0, nodes: run.iter().map(|x| x.1).collect() })
        .collect();
    out.sort_by(|a, b| {
        recoverable(b.logical, b.nodes.len())
            .cmp(&recoverable(a.logical, a.nodes.len()))
            .then(b.logical.cmp(&a.logical))
            .then(a.nodes.first().cmp(&b.nodes.first()))
    });
    out
}

/// Copie con la stessa impronta, dentro un gruppo della stessa dimensione.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IdenticalGroup {
    pub logical: u64,
    /// Per data di creazione crescente, a parità per nodo.
    pub nodes: Vec<u32>,
    /// Il file che resta: per default il più vecchio per data di creazione, a parità il nodo più basso
    /// (cioè `nodes[0]`); l'utente ne sceglie un altro con [`with_keep`].
    pub keep: u32,
}

/// Esito del confronto di un gruppo della stessa dimensione.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GroupComparison {
    /// Non tutti i file del gruppo sono stati letti (confronto non chiesto, o annullato a metà): `read`
    /// file letti. Nessun risultato parziale (06 §3.2).
    NotRead { read: usize },
    /// Tutti letti, ma `changed` file hanno una dimensione diversa da quella della scansione: le
    /// impronte non dicono niente sul gruppo finché il modello non si aggiorna.
    Changed { changed: usize },
    /// Tutti letti con la dimensione della scansione: copie (gruppi di almeno due file con la stessa
    /// impronta) e file senza copie nel gruppo, in ordine di nodo.
    Compared { identical: Vec<IdenticalGroup>, unique: Vec<u32> },
}

/// Ultima lettura di ogni nodo: con più letture dello stesso file vale l'ultima.
fn readings(hashed: &[Hashed]) -> HashMap<u32, &Hashed> {
    let mut m = HashMap::with_capacity(hashed.len());
    for h in hashed {
        m.insert(h.node, h);
    }
    m
}

fn compare_one(group: &SizeGroup, read: &HashMap<u32, &Hashed>) -> GroupComparison {
    let got: Vec<&Hashed> = group.nodes.iter().filter_map(|n| read.get(n).copied()).collect();
    if got.len() < group.nodes.len() {
        return GroupComparison::NotRead { read: got.len() };
    }
    let changed = got.iter().filter(|h| h.size != group.logical).count();
    if changed > 0 {
        return GroupComparison::Changed { changed };
    }
    let mut by_digest: Vec<(&[u8; 32], i64, u32)> = got.iter().map(|h| (&h.digest, h.created, h.node)).collect();
    by_digest.sort_unstable();
    let mut identical = Vec::new();
    let mut unique = Vec::new();
    for run in by_digest.chunk_by(|a, b| a.0 == b.0) {
        if run.len() >= 2 {
            // Dentro la stessa impronta l'ordine è già (creazione, nodo).
            let nodes: Vec<u32> = run.iter().map(|x| x.2).collect();
            identical.push(IdenticalGroup { logical: group.logical, keep: nodes[0], nodes });
        } else {
            unique.extend(run.iter().map(|x| x.2));
        }
    }
    sort_identical(&mut identical);
    unique.sort_unstable();
    GroupComparison::Compared { identical, unique }
}

fn sort_identical(v: &mut [IdenticalGroup]) {
    v.sort_by(|a, b| {
        recoverable(b.logical, b.nodes.len())
            .cmp(&recoverable(a.logical, a.nodes.len()))
            .then(b.logical.cmp(&a.logical))
            .then(a.nodes.first().cmp(&b.nodes.first()))
    });
}

/// Esito del confronto per ogni gruppo, nello stesso ordine di `candidates`.
pub fn compare_groups(candidates: &[SizeGroup], hashed: &[Hashed]) -> Vec<GroupComparison> {
    let read = readings(hashed);
    candidates.iter().map(|g| compare_one(g, &read)).collect()
}

/// Copie identiche dopo "Confronta il contenuto" (F-DL-4). Un gruppo della stessa dimensione conta solo
/// se tutti i suoi file sono stati letti e hanno ancora quella dimensione; dentro, gruppi per impronta
/// con almeno due file. Il booleano dice se tutti i candidati sono stati letti: con un annullamento a
/// metà è falso, e i gruppi restituiti sono solo quelli letti per intero (06 §3.2). Ordine: byte
/// potenzialmente recuperabili decrescenti.
pub fn identical_groups(candidates: &[SizeGroup], hashed: &[Hashed]) -> (Vec<IdenticalGroup>, bool) {
    let mut out = Vec::new();
    let mut complete = true;
    for c in compare_groups(candidates, hashed) {
        match c {
            GroupComparison::NotRead { .. } => complete = false,
            GroupComparison::Changed { .. } => {}
            GroupComparison::Compared { identical, .. } => out.extend(identical),
        }
    }
    sort_identical(&mut out);
    (out, complete)
}

/// Lo stesso gruppo con `node` da tenere (scelta dell'utente); `None` se `node` non è nel gruppo.
pub fn with_keep(group: &IdenticalGroup, node: u32) -> Option<IdenticalGroup> {
    group.nodes.contains(&node).then(|| IdenticalGroup { keep: node, ..group.clone() })
}

// ---------------------------------------------------------------------------------------------
// Archivi zip con la cartella estratta (F-DL-5)

/// Uno zip con una cartella sorella con lo stesso nome senza `.zip`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArchivePair {
    pub archive: u32,
    pub folder: u32,
}

/// Confronto per i nomi di file come in NTFS: ogni carattere al suo maiuscolo semplice, quando è un
/// solo carattere (`ß` resta `ß`, come nella tabella `$UpCase`).
fn fold_str(s: &str) -> String {
    if s.is_ascii() {
        return s.to_ascii_uppercase();
    }
    s.chars()
        .map(|c| {
            let mut u = c.to_uppercase();
            match (u.next(), u.next()) {
                (Some(x), None) => x,
                _ => c,
            }
        })
        .collect()
}

fn fold_name(name: Name<'_>) -> String {
    match name {
        Name::Ascii(s) => s.iter().map(|&b| char::from(b.to_ascii_uppercase())).collect(),
        Name::Wide(_) => fold_str(&name.to_string_lossy()),
    }
}

/// Nome senza `.zip` (in qualunque maiuscolo), già confrontabile; `None` se non è uno zip o se senza
/// estensione non resta niente.
fn zip_stem(name: Name<'_>) -> Option<String> {
    let s = name.to_string_lossy();
    let k = s.len().checked_sub(4)?;
    if k == 0 || !s.is_char_boundary(k) || !s[k..].eq_ignore_ascii_case(".zip") {
        return None;
    }
    Some(fold_str(&s[..k]))
}

/// Archivi `.zip` della lente con una cartella sorella che ha lo stesso nome senza estensione, senza
/// distinzione di maiuscole. Solo zip: gli altri archivi restano nella loro famiglia, senza confronto
/// (F-DL-5). Le cartelle staccate e i reparse non seguiti non contano. In ordine di archivio.
pub fn archive_pairs(tree: &Tree, lens: &Lens) -> Vec<ArchivePair> {
    let mut by_parent: HashMap<u32, Vec<(u32, String)>> = HashMap::new();
    for it in lens.items.iter().filter(|i| i.family == LensFamily::Archives && (i.node as usize) < tree.len()) {
        let Some(stem) = zip_stem(tree.name(it.node)) else { continue };
        let p = tree.parent(it.node);
        if p != NONE {
            by_parent.entry(p).or_default().push((it.node, stem));
        }
    }
    let mut out = Vec::new();
    for (parent, zips) in by_parent {
        // Un indice per cartella: niente ricerche lineari ripetute in una cartella con migliaia di file.
        let mut dirs: HashMap<String, u32> = HashMap::new();
        for c in tree.children(parent) {
            let f = tree.flags(c);
            if f.has(Flags::DIR) && !f.any(Flags::DETACHED | Flags::NOT_FOLLOWED) {
                dirs.entry(fold_name(tree.name(c))).or_insert(c);
            }
        }
        for (archive, stem) in zips {
            if let Some(&folder) = dirs.get(&stem) {
                out.push(ArchivePair { archive, folder });
            }
        }
    }
    out.sort_unstable_by_key(|p| p.archive);
    out
}

/// Confronto tra l'indice di uno zip e la cartella estratta. Le voci di cartella non contano;
/// `total` = `present` + `different_size` + `missing` + `unreadable`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ArchiveCheck {
    /// M: voci di file dell'archivio.
    pub total: u64,
    /// N: voci presenti nella cartella allo stesso percorso relativo con la stessa dimensione logica.
    pub present: u64,
    /// Voci presenti con una dimensione logica diversa.
    pub different_size: u64,
    /// Voci assenti, comprese quelle con un percorso non valido (`..`, nomi vuoti, percorsi assoluti),
    /// che non si risolvono mai fuori dalla cartella.
    pub missing: u64,
    /// Voci che cadono in una cartella non elencata o elencata in parte (accesso negato, errore,
    /// scansione interrotta, cartella cloud, junction): non si sa se ci sono, quindi né presenti né
    /// mancanti.
    pub unreadable: u64,
}

/// Componenti di un percorso dell'archivio, con `/` e `\` equivalenti; `None` se il percorso non è
/// valido: vuoto, assoluto (separatore iniziale, unità), con un componente vuoto, `.`, `..` o con `:`.
fn components(path: &str) -> Option<Vec<&str>> {
    if path.is_empty() || path.starts_with(['/', '\\']) {
        return None;
    }
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    if parts.iter().any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains(':')) {
        return None;
    }
    Some(parts)
}

/// Voce di cartella: dichiarata dall'archivio o con il separatore in fondo.
fn is_dir_entry(e: &ArchiveEntry) -> bool {
    e.dir || e.path.ends_with(['/', '\\'])
}

/// Figli delle cartelle per nome confrontabile, costruiti alla prima richiesta.
struct ChildIndex<'a> {
    tree: &'a Tree,
    map: HashMap<u32, HashMap<String, u32>>,
}

enum Resolved {
    File(u32),
    Missing,
    Unreadable,
}

impl ChildIndex<'_> {
    fn child(&mut self, dir: u32, key: &str) -> Option<u32> {
        let tree = self.tree;
        let kids = self.map.entry(dir).or_insert_with(|| {
            let mut m = HashMap::new();
            for c in tree.children(dir) {
                if !tree.flags(c).has(Flags::DETACHED) {
                    m.entry(fold_name(tree.name(c))).or_insert(c);
                }
            }
            m
        });
        kids.get(key).copied()
    }

    fn has_dir(&mut self, dir: u32, key: &str) -> bool {
        self.child(dir, key).is_some_and(|c| self.tree.is_dir(c))
    }

    /// Nodo di `parts` sotto `folder`, senza mai uscire dalla cartella.
    fn resolve(&mut self, folder: u32, parts: &[&str]) -> Resolved {
        let tree = self.tree;
        let mut cur = folder;
        for (k, part) in parts.iter().enumerate() {
            let Some(c) = self.child(cur, &fold_str(part)) else {
                return if tree.flags(cur).any(UNREAD) { Resolved::Unreadable } else { Resolved::Missing };
            };
            let f = tree.flags(c);
            if f.has(Flags::NOT_FOLLOWED) {
                // Junction o collegamento simbolico: il contenuto non è nella scansione.
                return Resolved::Unreadable;
            }
            let last = k + 1 == parts.len();
            match (last, f.has(Flags::DIR)) {
                (true, false) => return Resolved::File(c),
                (true, true) | (false, false) => return Resolved::Missing,
                (false, true) => cur = c,
            }
        }
        Resolved::Missing
    }
}

/// Confronta l'indice di uno zip con la cartella estratta `folder` (F-DL-5): M voci di file, N presenti
/// allo stesso percorso relativo con la stessa dimensione logica. Percorsi senza distinzione di
/// maiuscole, `/` e `\` equivalenti. Ogni voce costa una ricerca per componente in un indice dei figli.
///
/// Caso "Estrai tutto" e simili: se tutte le voci stanno sotto una cartella con il nome della cartella
/// estratta (che è il nome dell'archivio) e la cartella estratta non contiene una cartella con quel
/// nome, il prefisso si toglie. Così valgono sia `Progetto\Progetto\...` (estrazione in una cartella
/// nuova, che raddoppia il livello) sia `Progetto\...` (estrazione accanto all'archivio).
pub fn compare_archive(tree: &Tree, folder: u32, entries: &[ArchiveEntry]) -> ArchiveCheck {
    let mut check = ArchiveCheck::default();
    let valid_folder = (folder as usize) < tree.len() && tree.is_dir(folder);
    let mut index = ChildIndex { tree, map: HashMap::new() };
    let strip = valid_folder && {
        let key = fold_name(tree.name(folder));
        let under = |e: &ArchiveEntry| {
            let mut it = e.path.split(['/', '\\']);
            let first_ok = it.next().is_some_and(|p| fold_str(p) == key);
            let deeper = it.next().is_some_and(|p| !p.is_empty());
            first_ok && (deeper || is_dir_entry(e))
        };
        !entries.is_empty() && entries.iter().any(|e| !is_dir_entry(e)) && entries.iter().all(under) && !index.has_dir(folder, &key)
    };
    for e in entries.iter().filter(|e| !is_dir_entry(e)) {
        check.total += 1;
        let parts = match components(&e.path) {
            Some(p) if valid_folder => p,
            _ => {
                check.missing += 1;
                continue;
            }
        };
        let parts = if strip { &parts[1..] } else { &parts[..] };
        if parts.is_empty() {
            check.missing += 1;
            continue;
        }
        match index.resolve(folder, parts) {
            Resolved::File(n) if tree.logical(n) == e.size => check.present += 1,
            Resolved::File(_) => check.different_size += 1,
            Resolved::Missing => check.missing += 1,
            Resolved::Unreadable => check.unreadable += 1,
        }
    }
    check
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chromium_extension_rules() {
        assert_eq!(extension_start(b"setup.exe"), 5);
        assert_eq!(extension_start(b"archivio.tar.gz"), 8);
        assert_eq!(extension_start(b"dati.abcdef.gz"), 11);
        assert_eq!(extension_start(b"script.user.js"), 6);
        assert_eq!(extension_start(b"LEGGIMI"), 7);
        assert_eq!(extension_start(b".bashrc"), 0);
        assert_eq!(extension_start(b"a..gz"), 2);
    }

    #[test]
    fn incomplete_only_two_extensions() {
        let w: Vec<u16> = "Non confermato è.CRDOWNLOAD".encode_utf16().collect();
        assert!(is_incomplete(Name::Wide(&w)));
        assert!(is_incomplete(Name::Ascii(b"video.mp4.part")));
        assert!(!is_incomplete(Name::Ascii(b".part")));
        assert!(!is_incomplete(Name::Ascii(b"file.partial")));
        assert!(!is_incomplete(Name::Ascii(b"file.tmp")));
    }
}
