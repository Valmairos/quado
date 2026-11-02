// SPDX-License-Identifier: GPL-3.0-or-later
//! Messaggi tra interfaccia e helper.
//!
//! Cornice: lunghezza `u32` little endian, poi il corpo; il primo byte del corpo è il tipo. Codifica a
//! mano, senza dipendenze: la richiesta è l'unico ingresso dell'helper elevato e si decodifica con limiti
//! espliciti prima di ogni allocazione.
//!
//! Il core non produce frasi: errori e stati sono codici.
//!
//! Formato del corpo (interi little endian; stringa = `u32` di lunghezza in unità UTF-16, poi le unità):
//!
//! | Tipo | Campi |
//! |---|---|
//! | `0x01` richiesta | versione `u16`, volume, numero di cartelle `u32`, cartelle |
//! | `0x10` `Started` | `u8` (0 o 1) |
//! | `0x11` `Folder` | indice `u32`, albero, problemi (`u32` numero; nodo `u32`, codice `u32`) |
//! | `0x12` `Totals` | indice `u32`, totali |
//! | `0x13` `Bin` | indice `u32`, SID, totali, orfano `u8` |
//! | `0x14` `Failed` | indice `u32`, codice `u32` |
//! | `0x15` `Shadow` | `u8` 0 = nessuno, 1 = `used`, `allocated`, `max` `u64`, 2 = errore `u32` |
//! | `0x16` `Done` | codice `u32` |
//!
//! Albero: numero di nodi `u32`, poi i nodi in ordine di indice (genitore `u32`, nome, logical `u64`, alloc
//! `u64`, mtime `i64`, attrs `u32`, flags `u16`), poi i dati delle cartelle (`u32` numero; nodo `u32`,
//! allocata dell'indice `u64`, errore `u32`, in ordine di nodo). Totali: logical, alloc, files, dirs `u64`,
//! unreadable `u32`.

use quado_model::{DirExtra, Flags, NONE, Name, NodeData, Tree};

use crate::scan::Problem;

/// Versione del protocollo: interfaccia e helper sono lo stesso eseguibile, ma una richiesta con una
/// versione diversa si rifiuta lo stesso.
pub const VERSION: u16 = 1;
/// Cartelle al massimo in una richiesta.
pub const MAX_FOLDERS: usize = 4096;
/// Lunghezza massima di un percorso, in unità UTF-16 (limite di Windows con `\\?\`).
pub const MAX_PATH_UNITS: usize = 32_767;
/// Dimensione massima del corpo della richiesta.
pub const MAX_REQUEST_BYTES: usize = 4 << 20;
/// Dimensione massima del corpo di una risposta (un sottoalbero grande).
pub const MAX_REPLY_BYTES: usize = 512 << 20;

const T_REQUEST: u8 = 0x01;
const T_STARTED: u8 = 0x10;
const T_FOLDER: u8 = 0x11;
const T_TOTALS: u8 = 0x12;
const T_BIN: u8 = 0x13;
const T_FAILED: u8 = 0x14;
const T_SHADOW: u8 = 0x15;
const T_DONE: u8 = 0x16;

/// Un SID in forma testuale ha al massimo 184 caratteri (15 sottoautorità): margine abbondante.
const MAX_SID_UNITS: usize = 256;
/// Nome di un nodo: limite del pool dei nomi del modello (bit alto della lunghezza riservato).
const MAX_NAME_UNITS: usize = 0x7FFF;
/// Byte minimi di un nodo (nome vuoto): il numero dei nodi si controlla contro i byte rimasti prima di
/// riservare la memoria dell'albero.
const NODE_MIN_BYTES: usize = 4 + 4 + 8 + 8 + 8 + 4 + 2;
const EXTRA_BYTES: usize = 4 + 8 + 4;
const PROBLEM_BYTES: usize = 4 + 4;
/// Flag che un albero dell'helper può portare. `MEASURED` no: il numero di file di un nodo non viaggia
/// (si ricalcola), quindi una cartella misurata non si ricostruirebbe uguale.
const WIRE_FLAGS: u16 = (1 << 12) - 1;

/// La richiesta: l'unico messaggio che l'helper legge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// Radice del volume, `X:\`.
    pub volume: String,
    /// Cartelle negate da completare: percorsi assoluti su `volume`, normalizzati, senza `..`, nessuna
    /// dentro un'altra.
    pub folders: Vec<String>,
}

/// Perché una richiesta o una risposta non si accetta. Codici, non frasi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtoError {
    /// Cornice o corpo troncati, tipo sconosciuto, campi oltre la fine.
    Malformed,
    Version(u16),
    TooLarge,
    TooManyFolders,
    /// Volume non nella forma `X:\`.
    BadVolume,
    /// Indice della cartella nella richiesta, e motivo.
    BadPath(u32, PathIssue),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathIssue {
    Relative,
    OtherVolume,
    /// Componenti `.` o `..`, separatori doppi, `/`, caratteri di controllo, prefissi `\\?\` o `\\.\`.
    NotNormalized,
    TooLong,
    /// La radice del volume: l'helper completa cartelle, non rifà la scansione.
    VolumeRoot,
    /// Uguale a un'altra cartella della richiesta o dentro un'altra.
    Duplicate,
    /// Flussi alternativi (`:` dopo la lettera del volume).
    Stream,
}

/// Come l'helper tratta una cartella della richiesta: lo decide l'helper dal percorso, non l'interfaccia.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Si elenca e si trasmette il sottoalbero con i nomi.
    Tree,
    /// Profilo di un altro utente (`<volume>Users\<nome>` diverso dal profilo di chi ha avviato l'helper,
    /// e tutto ciò che c'è sotto): solo totali, nessun nome.
    Profile,
    /// Cestino di un altro account (`<volume>$Recycle.Bin\S-...` diverso dal SID di chi ha avviato
    /// l'helper, e tutto ciò che c'è sotto): totale per SID.
    Bin { sid: String },
}

/// Totali di una cartella elencata senza trasmetterne i nomi.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub logical: u64,
    /// Allocata unica (hard link contati una volta dentro la cartella).
    pub alloc: u64,
    pub files: u64,
    pub dirs: u64,
    /// Sottocartelle che neanche l'helper ha potuto elencare.
    pub unreadable: u32,
}

/// Spazio delle copie shadow sul volume (`Win32_ShadowStorage`), in byte. I file delle copie stanno in
/// `System Volume Information`: se l'helper elenca quella cartella sono già nell'albero, quindi questo
/// dato spiega una parte di quello spazio e non si somma al bilancio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShadowStorage {
    pub used: u64,
    pub allocated: u64,
    pub max: u64,
}

/// Messaggi dell'helper, nell'ordine: `Started`, una risposta per cartella (`Folder`, `Totals`, `Bin`
/// o `Failed`, con l'indice della cartella nella richiesta), `Shadow`, `Done`.
#[derive(Clone, Debug)]
pub enum Reply {
    /// `backup_privilege`: `SeBackupPrivilege` abilitato. Se è falso le cartelle negate restano tali.
    Started { backup_privilege: bool },
    /// Cartella di [`Scope::Tree`]: sottoalbero aggregato e sigillato, come quello di una scansione
    /// (radice = la cartella), e i problemi con gli indici di quell'albero.
    Folder { index: u32, tree: Box<Tree>, problems: Vec<(u32, Problem)> },
    /// Cartella di [`Scope::Profile`].
    Totals { index: u32, totals: Totals },
    /// Cartella di [`Scope::Bin`]: `orphan` se il SID non corrisponde più a un account.
    Bin { index: u32, sid: String, totals: Totals, orphan: bool },
    /// La cartella non si è potuta aprire neanche dall'helper: codice di Windows.
    Failed { index: u32, code: u32 },
    /// `Ok(None)`: nessuno spazio per le copie shadow sul volume. `Err`: codice (WMI o Windows).
    Shadow(Result<Option<ShadowStorage>, u32>),
    /// Fine: l'helper esce subito dopo. `code` 0 se tutto è andato, altrimenti il primo errore grave.
    Done { code: u32 },
}

// ---------------------------------------------------------------------------------------------------
// Scrittura e lettura dei campi

fn put_u16(w: &mut Vec<u8>, v: u16) {
    w.extend_from_slice(&v.to_le_bytes());
}

fn put_u32(w: &mut Vec<u8>, v: u32) {
    w.extend_from_slice(&v.to_le_bytes());
}

fn put_u64(w: &mut Vec<u8>, v: u64) {
    w.extend_from_slice(&v.to_le_bytes());
}

/// Lunghezza come `u32`: le stringhe e gli elenchi del protocollo stanno molto sotto i 4 G elementi
/// (limiti sopra); oltre, la lunghezza satura e il corpo non si decodifica.
fn len32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn put_str(w: &mut Vec<u8>, s: &str) {
    put_u32(w, len32(s.encode_utf16().count()));
    for u in s.encode_utf16() {
        put_u16(w, u);
    }
}

fn put_name(w: &mut Vec<u8>, n: Name<'_>) {
    put_u32(w, len32(n.len()));
    match n {
        Name::Ascii(s) => s.iter().for_each(|&b| put_u16(w, u16::from(b))),
        Name::Wide(s) => s.iter().for_each(|&u| put_u16(w, u)),
    }
}

fn put_totals(w: &mut Vec<u8>, t: &Totals) {
    put_u64(w, t.logical);
    put_u64(w, t.alloc);
    put_u64(w, t.files);
    put_u64(w, t.dirs);
    put_u32(w, t.unreadable);
}

/// Lettore con controllo dei limiti: ogni lettura oltre la fine è `Malformed`, nessuna lettura alloca.
struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.b.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ProtoError> {
        if n > self.remaining() {
            return Err(ProtoError::Malformed);
        }
        let s = &self.b[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ProtoError> {
        let s = self.take(N)?;
        let mut a = [0u8; N];
        a.copy_from_slice(s);
        Ok(a)
    }

    fn u8(&mut self) -> Result<u8, ProtoError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtoError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, ProtoError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, ProtoError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn i64(&mut self) -> Result<i64, ProtoError> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    fn bool(&mut self) -> Result<bool, ProtoError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ProtoError::Malformed),
        }
    }

    /// Unità UTF-16 di una stringa: lunghezza controllata contro `max` (errore `long`) e contro i byte
    /// rimasti prima di toccare la memoria.
    fn units(&mut self, max: usize, long: ProtoError) -> Result<&'a [u8], ProtoError> {
        let n = self.u32()? as usize;
        if n > max {
            return Err(long);
        }
        self.take(n * 2)
    }

    fn totals(&mut self) -> Result<Totals, ProtoError> {
        Ok(Totals { logical: self.u64()?, alloc: self.u64()?, files: self.u64()?, dirs: self.u64()?, unreadable: self.u32()? })
    }

    /// Fine del corpo: byte in più sono un errore.
    fn finish(&self) -> Result<(), ProtoError> {
        if self.remaining() == 0 { Ok(()) } else { Err(ProtoError::Malformed) }
    }
}

fn wide_units(raw: &[u8]) -> impl Iterator<Item = u16> + '_ {
    raw.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c))
}

/// Stringa da unità UTF-16 little endian; `None` con surrogati spaiati.
fn string_from(raw: &[u8]) -> Option<String> {
    let mut s = String::with_capacity(raw.len() / 2);
    for c in char::decode_utf16(wide_units(raw)) {
        s.push(c.ok()?);
    }
    Some(s)
}

// ---------------------------------------------------------------------------------------------------
// Richiesta

/// Corpo della richiesta (senza cornice).
pub fn encode_request(r: &Request) -> Vec<u8> {
    let units: usize = r.folders.iter().map(|f| f.len()).sum();
    let mut w = Vec::with_capacity(16 + r.folders.len() * 4 + units * 2);
    w.push(T_REQUEST);
    put_u16(&mut w, VERSION);
    put_str(&mut w, &r.volume);
    put_u32(&mut w, len32(r.folders.len()));
    for f in &r.folders {
        put_str(&mut w, f);
    }
    w
}

/// Decodifica e valida (`validate`) una richiesta. Unico punto in cui l'helper interpreta dati esterni.
pub fn decode_request(body: &[u8]) -> Result<Request, ProtoError> {
    if body.len() > MAX_REQUEST_BYTES {
        return Err(ProtoError::TooLarge);
    }
    let mut r = Reader::new(body);
    if r.u8()? != T_REQUEST {
        return Err(ProtoError::Malformed);
    }
    let version = r.u16()?;
    if version != VERSION {
        return Err(ProtoError::Version(version));
    }
    let volume = r.units(3, ProtoError::BadVolume)?;
    let volume = string_from(volume).ok_or(ProtoError::BadVolume)?;
    let count = r.u32()? as usize;
    if count > MAX_FOLDERS {
        return Err(ProtoError::TooManyFolders);
    }
    // Ogni cartella occupa almeno i 4 byte della lunghezza: il numero dichiarato non può superare i byte
    // rimasti, quindi la memoria riservata qui sotto è proporzionale al corpo ricevuto.
    if count > r.remaining() / 4 {
        return Err(ProtoError::Malformed);
    }
    let mut folders = Vec::with_capacity(count);
    for i in 0..count {
        let idx = i as u32;
        let raw = r.units(MAX_PATH_UNITS, ProtoError::BadPath(idx, PathIssue::TooLong))?;
        folders.push(string_from(raw).ok_or(ProtoError::BadPath(idx, PathIssue::NotNormalized))?);
    }
    r.finish()?;
    let req = Request { volume, folders };
    req.validate()?;
    Ok(req)
}

/// Controlli di un percorso: `letter` è la lettera del volume della richiesta.
fn check_path(letter: u8, f: &str) -> Result<(), PathIssue> {
    if f.chars().map(char::len_utf16).sum::<usize>() > MAX_PATH_UNITS {
        return Err(PathIssue::TooLong);
    }
    if f.chars().any(|c| c == '/' || (c as u32) < 0x20) {
        return Err(PathIssue::NotNormalized);
    }
    let b = f.as_bytes();
    // `\\server\share`, `\\?\`, `\\.\`: prefissi che cambiano il significato del percorso.
    if b.starts_with(b"\\\\") {
        return Err(PathIssue::NotNormalized);
    }
    // `X:\...`; `X:` e `X:nome` sono relativi alla cartella corrente di quel volume, `\nome` alla sua radice.
    if b.len() < 3 || !b[0].is_ascii_alphabetic() || b[1] != b':' || b[2] != b'\\' {
        return Err(PathIssue::Relative);
    }
    if !b[0].eq_ignore_ascii_case(&letter) {
        return Err(PathIssue::OtherVolume);
    }
    if b.len() == 3 {
        return Err(PathIssue::VolumeRoot);
    }
    let rest = &f[3..];
    if rest.ends_with('\\') {
        return Err(PathIssue::NotNormalized);
    }
    for comp in rest.split('\\') {
        if comp.is_empty() || comp == "." || comp == ".." {
            return Err(PathIssue::NotNormalized);
        }
        if comp.contains(':') {
            return Err(PathIssue::Stream);
        }
    }
    Ok(())
}

/// Chiave di confronto senza maiuscole ASCII, con il separatore portato a 0: così una cartella e il
/// suo contenuto (`a`, `a\...`) sono adiacenti nell'ordine e un percorso è dentro un altro se e solo se
/// la sua chiave inizia con quella dell'altro seguita da 0. I caratteri di controllo sono già esclusi.
fn nesting_key(f: &str) -> Vec<u8> {
    f.bytes().map(|b| if b == b'\\' { 0 } else { b.to_ascii_lowercase() }).collect()
}

impl Request {
    /// Controlli della richiesta, l'unico ingresso dell'helper elevato: volume `X:\`, percorsi assoluti su
    /// quel volume, normalizzati, senza `..`, con lunghezza e numero massimi, nessuno dentro un altro.
    pub fn validate(&self) -> Result<(), ProtoError> {
        let v = self.volume.as_bytes();
        if !(v.len() == 3 && v[0].is_ascii_alphabetic() && v[1] == b':' && v[2] == b'\\') {
            return Err(ProtoError::BadVolume);
        }
        if self.folders.len() > MAX_FOLDERS {
            return Err(ProtoError::TooManyFolders);
        }
        for (i, f) in self.folders.iter().enumerate() {
            check_path(v[0], f).map_err(|e| ProtoError::BadPath(i as u32, e))?;
        }
        // Uguali o annidate: dopo l'ordinamento basta confrontare le vicine (vedi `nesting_key`).
        // L'ordinamento è stabile: fra due uguali l'errore va alla seconda della richiesta.
        let keys: Vec<Vec<u8>> = self.folders.iter().map(|f| nesting_key(f)).collect();
        let mut order: Vec<usize> = (0..keys.len()).collect();
        order.sort_by(|&a, &b| keys[a].cmp(&keys[b]));
        for pair in order.windows(2) {
            let (outer, inner) = (&keys[pair[0]], &keys[pair[1]]);
            if inner == outer || (inner.starts_with(outer) && inner.get(outer.len()) == Some(&0)) {
                return Err(ProtoError::BadPath(pair[1] as u32, PathIssue::Duplicate));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------
// Ambito

/// Senza il prefisso `prefix`, confrontato senza maiuscole ASCII.
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &s[prefix.len()..])
}

/// `path` è `dir` o sta dentro `dir` (confine di componente, senza maiuscole ASCII).
fn within_ci(path: &str, dir: &str) -> bool {
    strip_prefix_ci(path, dir).is_some_and(|rest| rest.is_empty() || rest.starts_with('\\'))
}

/// Percorso confrontabile: senza `\\?\` e senza `\` finale.
fn plain(p: &str) -> &str {
    let p = p.strip_prefix(r"\\?\").unwrap_or(p);
    p.trim_end_matches('\\')
}

/// Ambito di una cartella già validata. `own_profile`: cartella del profilo di chi ha avviato l'helper
/// (`ProfileImagePath`), se nota; `own_sid`: il suo SID. Senza `own_profile` ogni cartella sotto
/// `Users\<nome>` è un profilo altrui (scelta prudente: si perdono i nomi, non si mostrano quelli altrui).
///
/// Anche `<volume>Users` e `<volume>$Recycle.Bin` stessi sono [`Scope::Profile`]: il loro elenco
/// conterrebbe i nomi dei profili e dei Cestini altrui. Un `own_profile` che contiene `<volume>Users`
/// (la radice o `Users` stessa) non vale come profilo proprio. I confronti sono senza maiuscole ASCII:
/// alias come i nomi brevi 8.3 o le junction li risolve chi chiama, classificando anche il percorso
/// finale (`serve`).
pub fn classify(volume: &str, folder: &str, own_profile: Option<&str>, own_sid: Option<&str>) -> Scope {
    let rest = match strip_prefix_ci(folder, volume) {
        Some(r) => r,
        // Fuori dal volume (una richiesta validata non lo è mai): stesse regole sul volume del percorso.
        None => match folder.as_bytes() {
            [l, b':', b'\\', ..] if l.is_ascii_alphabetic() => &folder[3..],
            _ => return Scope::Tree,
        },
    };
    let root = &folder[..folder.len() - rest.len()];
    let mut comps = rest.split('\\');
    let first = comps.next().unwrap_or("");
    if first.eq_ignore_ascii_case("$Recycle.Bin") {
        return match comps.next() {
            None => Scope::Profile,
            Some(sid) if strip_prefix_ci(sid, "S-1-").is_some() => {
                if own_sid.is_some_and(|o| o.eq_ignore_ascii_case(sid)) { Scope::Tree } else { Scope::Bin { sid: sid.to_owned() } }
            }
            Some(_) => Scope::Tree,
        };
    }
    if first.eq_ignore_ascii_case("Users") {
        if comps.next().is_none() {
            return Scope::Profile;
        }
        let users = format!("{root}Users");
        let own = own_profile.map(plain).filter(|p| !within_ci(&users, p));
        return if own.is_some_and(|p| within_ci(folder, p)) { Scope::Tree } else { Scope::Profile };
    }
    Scope::Tree
}

// ---------------------------------------------------------------------------------------------------
// Risposte

fn put_tree(w: &mut Vec<u8>, t: &Tree) {
    let n = t.len();
    let (units, _) = t.name_units();
    w.reserve(n * NODE_MIN_BYTES + units * 2);
    put_u32(w, len32(n));
    for i in 0..n as u32 {
        put_u32(w, t.parent(i));
        put_name(w, t.name(i));
        put_u64(w, t.logical(i));
        put_u64(w, t.alloc(i));
        w.extend_from_slice(&t.mtime(i).to_le_bytes());
        put_u32(w, t.attrs(i));
        put_u16(w, t.flags(i).0);
    }
    // In ordine di nodo, uno per nodo, anche se l'albero non è stato sigillato.
    let mut extras: Vec<DirExtra> = t.dir_extras().to_vec();
    extras.sort_by_key(|e| e.node);
    extras.dedup_by_key(|e| e.node);
    put_u32(w, len32(extras.len()));
    for e in extras {
        put_u32(w, e.node);
        put_u64(w, e.index_alloc);
        put_u32(w, e.error);
    }
}

pub fn encode_reply(r: &Reply) -> Vec<u8> {
    let mut w = Vec::with_capacity(64);
    match r {
        Reply::Started { backup_privilege } => {
            w.push(T_STARTED);
            w.push(u8::from(*backup_privilege));
        }
        Reply::Folder { index, tree, problems } => {
            w.push(T_FOLDER);
            put_u32(&mut w, *index);
            put_tree(&mut w, tree);
            put_u32(&mut w, len32(problems.len()));
            for (node, p) in problems {
                put_u32(&mut w, *node);
                put_u32(&mut w, p.code());
            }
        }
        Reply::Totals { index, totals } => {
            w.push(T_TOTALS);
            put_u32(&mut w, *index);
            put_totals(&mut w, totals);
        }
        Reply::Bin { index, sid, totals, orphan } => {
            w.push(T_BIN);
            put_u32(&mut w, *index);
            put_str(&mut w, sid);
            put_totals(&mut w, totals);
            w.push(u8::from(*orphan));
        }
        Reply::Failed { index, code } => {
            w.push(T_FAILED);
            put_u32(&mut w, *index);
            put_u32(&mut w, *code);
        }
        Reply::Shadow(s) => {
            w.push(T_SHADOW);
            match s {
                Ok(None) => w.push(0),
                Ok(Some(s)) => {
                    w.push(1);
                    put_u64(&mut w, s.used);
                    put_u64(&mut w, s.allocated);
                    put_u64(&mut w, s.max);
                }
                Err(code) => {
                    w.push(2);
                    put_u32(&mut w, *code);
                }
            }
        }
        Reply::Done { code } => {
            w.push(T_DONE);
            put_u32(&mut w, *code);
        }
    }
    w
}

/// Nome di un nodo diverso dalla radice: non vuoto, senza separatori, `:` o NUL, non `.` o `..`. Un nome
/// così non diventa mai un percorso diverso quando l'interfaccia lo unisce a quello del genitore.
fn valid_child_name(units: &[u16]) -> bool {
    let dot = u16::from(b'.');
    !units.is_empty()
        && units != [dot]
        && units != [dot, dot]
        && !units.iter().any(|&u| u == 0 || u == u16::from(b'\\') || u == u16::from(b'/') || u == u16::from(b':'))
}

struct RawNode {
    parent: u32,
    data: NodeData,
}

fn read_node(r: &mut Reader<'_>, name: &mut Vec<u16>) -> Result<RawNode, ProtoError> {
    let parent = r.u32()?;
    let raw = r.units(MAX_NAME_UNITS, ProtoError::Malformed)?;
    name.clear();
    name.extend(wide_units(raw));
    let data = NodeData { logical: r.u64()?, alloc: r.u64()?, mtime: r.i64()?, attrs: r.u32()?, flags: Flags(r.u16()?) };
    if data.flags.0 & !WIRE_FLAGS != 0 {
        return Err(ProtoError::Malformed);
    }
    Ok(RawNode { parent, data })
}

/// Albero dai byte: genitore prima del figlio, genitore cartella, nomi e numeri controllati prima di
/// riservare la memoria; poi sigillato e riaggregato.
fn read_tree(r: &mut Reader<'_>) -> Result<Tree, ProtoError> {
    let n = r.u32()? as usize;
    if n == 0 || n > r.remaining() / NODE_MIN_BYTES {
        return Err(ProtoError::Malformed);
    }
    let mut name = Vec::new();
    // Radice: una cartella, come quella di una scansione. Data e attributi della radice non si
    // ricostruiscono (`Tree::with_root` li lascia a zero; l'innesto usa quelli del nodo sostituito).
    let root = read_node(r, &mut name)?;
    if root.parent != NONE || name.is_empty() || !root.data.flags.has(Flags::DIR) {
        return Err(ProtoError::Malformed);
    }
    let mut t = Tree::with_root(&name, n);
    t.add_flags(Tree::ROOT, root.data.flags);
    // Somme dei file: se non stanno in 64 bit l'aggregazione traboccherebbe, quindi il corpo è falso.
    let (mut logical, mut alloc) = (0u64, 0u64);
    for k in 1..n {
        let node = read_node(r, &mut name)?;
        if node.parent as usize >= k || !t.is_dir(node.parent) || !valid_child_name(&name) {
            return Err(ProtoError::Malformed);
        }
        if !node.data.flags.has(Flags::DIR) {
            logical = logical.checked_add(node.data.logical).ok_or(ProtoError::Malformed)?;
            alloc = alloc.checked_add(node.data.alloc).ok_or(ProtoError::Malformed)?;
        }
        t.push(node.parent, &name, node.data);
    }
    let m = r.u32()? as usize;
    if m > n || m > r.remaining() / EXTRA_BYTES {
        return Err(ProtoError::Malformed);
    }
    let mut prev: Option<u32> = None;
    for _ in 0..m {
        let node = r.u32()?;
        if node as usize >= n || !t.is_dir(node) || prev.is_some_and(|p| p >= node) {
            return Err(ProtoError::Malformed);
        }
        prev = Some(node);
        t.set_dir_extra(DirExtra { node, index_alloc: r.u64()?, error: r.u32()? });
    }
    t.seal();
    t.aggregate();
    Ok(t)
}

/// Decodifica una risposta. L'albero di `Folder` si ricostruisce con i controlli (genitore prima del
/// figlio, lunghezze) e si riaggrega.
pub fn decode_reply(body: &[u8]) -> Result<Reply, ProtoError> {
    if body.len() > MAX_REPLY_BYTES {
        return Err(ProtoError::TooLarge);
    }
    let mut r = Reader::new(body);
    let reply = match r.u8()? {
        T_STARTED => Reply::Started { backup_privilege: r.bool()? },
        T_FOLDER => {
            let index = r.u32()?;
            let tree = read_tree(&mut r)?;
            let p = r.u32()? as usize;
            if p > r.remaining() / PROBLEM_BYTES {
                return Err(ProtoError::Malformed);
            }
            let mut problems = Vec::with_capacity(p);
            for _ in 0..p {
                let node = r.u32()?;
                if node as usize >= tree.len() {
                    return Err(ProtoError::Malformed);
                }
                problems.push((node, Problem::from_code(r.u32()?)));
            }
            Reply::Folder { index, tree: Box::new(tree), problems }
        }
        T_TOTALS => Reply::Totals { index: r.u32()?, totals: r.totals()? },
        T_BIN => {
            let index = r.u32()?;
            let sid = string_from(r.units(MAX_SID_UNITS, ProtoError::Malformed)?).ok_or(ProtoError::Malformed)?;
            Reply::Bin { index, sid, totals: r.totals()?, orphan: r.bool()? }
        }
        T_FAILED => Reply::Failed { index: r.u32()?, code: r.u32()? },
        T_SHADOW => Reply::Shadow(match r.u8()? {
            0 => Ok(None),
            1 => Ok(Some(ShadowStorage { used: r.u64()?, allocated: r.u64()?, max: r.u64()? })),
            2 => Err(r.u32()?),
            _ => return Err(ProtoError::Malformed),
        }),
        T_DONE => Reply::Done { code: r.u32()? },
        _ => return Err(ProtoError::Malformed),
    };
    r.finish()?;
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_key_puts_children_right_after_the_parent() {
        // `a!b` viene prima di `a\b` in ordine di byte (`!` < `\`), ma non dopo con la chiave.
        let mut v = [nesting_key(r"C:\a\b"), nesting_key(r"C:\a!b"), nesting_key(r"C:\a")];
        v.sort();
        assert_eq!(v[0], nesting_key(r"C:\a"));
        assert_eq!(v[1], nesting_key(r"C:\a\b"));
    }

    #[test]
    fn plain_strips_prefix_and_trailing_separator() {
        assert_eq!(plain(r"\\?\C:\Users\io\"), r"C:\Users\io");
        assert_eq!(plain(r"C:\Users\io"), r"C:\Users\io");
    }
}
