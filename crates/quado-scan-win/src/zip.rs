// SPDX-License-Identifier: GPL-3.0-or-later
//! Directory centrale degli archivi zip, compreso zip64: solo il record di fine directory e la
//! directory centrale, letti dalla coda del file; nessuna decompressione.
//!
//! Il lettore è puro sopra [`ReadAt`]: lo stesso codice legge un file ([`ZipFile`]) o dei byte in memoria
//! (test di proprietà). Letture, nell'ordine:
//! - gli ultimi 22 byte, dove sta il record di fine directory di un archivio senza commento;
//! - con un commento, la ricerca prosegue all'indietro a passi di [`SEARCH_STEP`] byte fino al record il
//!   cui commento arriva esattamente alla fine del file: al più 63 byte letti prima del record. Con dei
//!   byte dopo l'archivio nessun record arriva alla fine, e la ricerca legge tutti gli ultimi 65.557 byte;
//! - se il record ha campi saturi (0xFFFF, 0xFFFFFFFF), il localizzatore e il record di fine directory
//!   zip64, che stanno tra la directory centrale e il record;
//! - la directory centrale, in una lettura.
//!
//! Mai gli header locali né i dati compressi. Ogni offset e ogni lunghezza si controllano con aritmetica
//! verificata prima di leggere; un formato inatteso dà un codice d'errore, mai un dato ricostruito:
//! niente correzioni degli offset per i dati anteposti all'archivio, niente archivi su più dischi.

use quado_model::downloads::ArchiveEntry;

use crate::content::{ContentFile, FileIdentity, OpenError};

/// Dimensione massima della directory centrale che si legge.
pub const MAX_CENTRAL_DIRECTORY: u64 = 64 << 20;
/// Numero massimo di voci.
pub const MAX_ENTRIES: u64 = 1_000_000;
/// Record di fine directory (22 byte) più il commento più lungo (65.535 byte).
pub const TAIL_WINDOW: u64 = EOCD_LEN + 65_535;
/// Passo della ricerca all'indietro del record di fine directory, quando l'archivio ha un commento.
pub const SEARCH_STEP: u64 = 64;

const SIG_EOCD: u32 = 0x0605_4b50;
const SIG_ZIP64_LOCATOR: u32 = 0x0706_4b50;
const SIG_ZIP64_EOCD: u32 = 0x0606_4b50;
const SIG_CENTRAL: u32 = 0x0201_4b50;
const SIG_DIGITAL_SIGNATURE: u32 = 0x0505_4b50;
const EOCD_LEN: u64 = 22;
const LOCATOR_LEN: u64 = 20;
const ZIP64_EOCD_LEN: u64 = 56;
const CENTRAL_LEN: usize = 46;
/// Bit 11 dei flag: nome e commento in UTF-8.
const FLAG_UTF8: u16 = 1 << 11;
const ZIP64_EXTRA: u16 = 0x0001;

/// Perché l'indice non è stato letto. Codici, non frasi.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ZipError {
    /// Errore di Windows nell'apertura o nella lettura.
    Io(u32),
    /// Segnaposto cloud: il contenuto non si legge, perché leggerlo lo scaricherebbe.
    CloudPlaceholder,
    /// Nessun record di fine directory negli ultimi 65.557 byte (oppure una cartella o un collegamento).
    NotZip,
    /// Il file finisce prima di ciò che l'archivio dichiara: record tagliato, commento o directory oltre
    /// la fine del file.
    Truncated,
    /// Strutture incoerenti dentro il file: firme, offset, conteggi, nomi UTF-8 non validi.
    Corrupt,
    /// Directory centrale oltre [`MAX_CENTRAL_DIRECTORY`] byte o oltre [`MAX_ENTRIES`] voci.
    TooLarge,
}

impl From<OpenError> for ZipError {
    fn from(e: OpenError) -> Self {
        match e {
            OpenError::Io(c) => Self::Io(c),
            OpenError::CloudPlaceholder => Self::CloudPlaceholder,
            OpenError::NotAFile => Self::NotZip,
        }
    }
}

/// Indice di un archivio: le voci della directory centrale, nell'ordine dell'archivio.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ZipIndex {
    pub entries: Vec<ArchiveEntry>,
    /// L'archivio usa zip64: record di fine directory zip64, o dimensioni di voci nel campo extra zip64.
    pub zip64: bool,
}

/// Sorgente a lettura posizionale. `read_at` riempie tutto `buf` oppure fallisce: oltre la fine dà
/// `Truncated`.
pub trait ReadAt {
    fn len(&self) -> u64;
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), ZipError>;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ReadAt for &[u8] {
    fn len(&self) -> u64 {
        <[u8]>::len(self) as u64
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), ZipError> {
        let start = usize::try_from(offset).map_err(|_| ZipError::Truncated)?;
        let end = start.checked_add(buf.len()).ok_or(ZipError::Truncated)?;
        buf.copy_from_slice(self.get(start..end).ok_or(ZipError::Truncated)?);
        Ok(())
    }
}

impl ReadAt for Vec<u8> {
    fn len(&self) -> u64 {
        <[u8]>::len(self) as u64
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), ZipError> {
        self.as_slice().read_at(offset, buf)
    }
}

/// Archivio aperto in sola lettura (`GENERIC_READ`, condivisione lettura, scrittura ed eliminazione)
/// dopo il controllo con i soli attributi: cartelle e collegamenti danno `NotZip`, i segnaposto cloud
/// `CloudPlaceholder`.
pub struct ZipFile {
    file: ContentFile,
}

impl ZipFile {
    pub fn open(path: &str) -> Result<Self, ZipError> {
        Ok(Self { file: ContentFile::open(path, false)? })
    }

    /// Identità del file all'apertura (per la cache: percorso, dimensione, data di modifica, ID), con la
    /// data di ultimo accesso precedente alla lettura.
    pub fn identity(&self) -> FileIdentity {
        self.file.opened
    }
}

impl ReadAt for ZipFile {
    fn len(&self) -> u64 {
        self.file.opened.size
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), ZipError> {
        let mut done = 0usize;
        while done < buf.len() {
            let at = offset.checked_add(done as u64).ok_or(ZipError::Truncated)?;
            let n = self.file.read_at(at, &mut buf[done..]).map_err(ZipError::Io)?;
            if n == 0 {
                return Err(ZipError::Truncated);
            }
            done += n;
        }
        Ok(())
    }
}

/// Indice dell'archivio `path`: apre il file (rifiutando i segnaposto cloud) e legge la directory
/// centrale. La lettura aggiorna la data di ultimo accesso del file.
pub fn zip_index(path: &str) -> Result<ZipIndex, ZipError> {
    read_index(&ZipFile::open(path)?)
}

fn le16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

fn le64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at.checked_add(8)?)?.try_into().ok()?))
}

/// Campo di una struttura già letta per intero: se manca, la struttura è incoerente.
fn field<T>(v: Option<T>) -> Result<T, ZipError> {
    v.ok_or(ZipError::Corrupt)
}

/// Posizione e contenuto del record di fine directory.
///
/// Preferisce il record il cui commento arriva esattamente alla fine del file; se nessuno ci arriva
/// (byte in coda dopo l'archivio), il più vicino alla fine il cui commento sta nel file. Una firma con
/// il record o il commento tagliati dalla fine del file dà `Truncated`; nessuna firma `NotZip`.
fn find_eocd(r: &dyn ReadAt) -> Result<(u64, [u8; EOCD_LEN as usize]), ZipError> {
    let len = r.len();
    if len < EOCD_LEN {
        return Err(ZipError::NotZip);
    }
    let base = len.saturating_sub(TAIL_WINDOW);
    let n = (len - base) as usize;
    let rec = EOCD_LEN as usize;
    // `tail[k]` è il byte `base + k`; `tail[lo..]` è già letto.
    let mut tail = vec![0u8; n];
    let mut lo = n - rec;
    r.read_at(base + lo as u64, &mut tail[lo..])?;
    // Posizioni da esaminare, scendendo: da `i` fino a `lo`. Le prime 18 hanno il record tagliato.
    let mut i = n - 4;
    let (mut exact, mut fitting, mut cut) = (None, None, false);
    'search: loop {
        loop {
            if le32(&tail, i) == Some(SIG_EOCD) {
                match le16(&tail, i + 20) {
                    Some(comment) => {
                        let end = i + rec + usize::from(comment);
                        if end == n {
                            exact = Some(i);
                            break 'search;
                        } else if end < n {
                            fitting = fitting.or(Some(i));
                        } else {
                            cut = true;
                        }
                    }
                    None => cut = true,
                }
            }
            if i == lo {
                break;
            }
            i -= 1;
        }
        if lo == 0 {
            break;
        }
        let next = lo.saturating_sub(SEARCH_STEP as usize);
        r.read_at(base + next as u64, &mut tail[next..lo])?;
        i = lo - 1;
        lo = next;
    }
    match exact.or(fitting) {
        Some(f) => {
            let mut out = [0u8; EOCD_LEN as usize];
            out.copy_from_slice(&tail[f..f + rec]);
            Ok((base + f as u64, out))
        }
        None if cut => Err(ZipError::Truncated),
        None => Err(ZipError::NotZip),
    }
}

/// Dove sta la directory centrale e quante voci dichiara.
struct Directory {
    offset: u64,
    size: u64,
    entries: u64,
    /// La directory deve finire prima di qui: il record zip64 o quello di fine directory.
    limit: u64,
    zip64: bool,
}

fn locate_directory(r: &dyn ReadAt) -> Result<Directory, ZipError> {
    let len = r.len();
    let (pos, e) = find_eocd(r)?;
    let disk = field(le16(&e, 4))?;
    let cd_disk = field(le16(&e, 6))?;
    let entries_disk = field(le16(&e, 8))?;
    let entries = field(le16(&e, 10))?;
    let size = field(le32(&e, 12))?;
    let offset = field(le32(&e, 16))?;
    let saturated =
        disk == u16::MAX || cd_disk == u16::MAX || entries_disk == u16::MAX || entries == u16::MAX || size == u32::MAX || offset == u32::MAX;
    if saturated && let Some(loc_pos) = pos.checked_sub(LOCATOR_LEN) {
        let mut loc = [0u8; LOCATOR_LEN as usize];
        r.read_at(loc_pos, &mut loc)?;
        // Senza localizzatore i campi saturi valgono così come sono (per esempio 65.535 voci senza zip64).
        if le32(&loc, 0) == Some(SIG_ZIP64_LOCATOR) {
            let z_disk = field(le32(&loc, 4))?;
            let z_pos = field(le64(&loc, 8))?;
            let disks = field(le32(&loc, 16))?;
            if z_disk != 0 || disks > 1 {
                return Err(ZipError::Corrupt);
            }
            let z_end = z_pos.checked_add(ZIP64_EOCD_LEN).ok_or(ZipError::Corrupt)?;
            if z_end > len {
                return Err(ZipError::Truncated);
            }
            if z_end > loc_pos {
                return Err(ZipError::Corrupt);
            }
            let mut z = [0u8; ZIP64_EOCD_LEN as usize];
            r.read_at(z_pos, &mut z)?;
            let record = field(le64(&z, 4))?;
            let declared_end = z_pos.checked_add(12).and_then(|v| v.checked_add(record));
            if le32(&z, 0) != Some(SIG_ZIP64_EOCD) || record < ZIP64_EOCD_LEN - 12 || declared_end.is_none_or(|end| end > loc_pos) {
                return Err(ZipError::Corrupt);
            }
            if field(le32(&z, 16))? != 0 || field(le32(&z, 20))? != 0 {
                return Err(ZipError::Corrupt);
            }
            return Ok(Directory {
                entries: field(le64(&z, 32))?,
                size: field(le64(&z, 40))?,
                offset: field(le64(&z, 48))?,
                limit: z_pos,
                zip64: true,
            });
        }
    }
    // Archivi su più dischi: non si leggono.
    if disk != 0 || cd_disk != 0 {
        return Err(ZipError::Corrupt);
    }
    Ok(Directory { entries: u64::from(entries), size: u64::from(size), offset: u64::from(offset), limit: pos, zip64: false })
}

/// Indice della directory centrale letto da `r`. Nessun panic su ingressi qualsiasi.
pub fn read_index(r: &dyn ReadAt) -> Result<ZipIndex, ZipError> {
    let d = locate_directory(r)?;
    let end = d.offset.checked_add(d.size).ok_or(ZipError::Corrupt)?;
    if end > r.len() {
        return Err(ZipError::Truncated);
    }
    if end > d.limit {
        return Err(ZipError::Corrupt);
    }
    // Ogni voce occupa almeno 46 byte: il numero dichiarato deve stare nella directory.
    if d.entries.checked_mul(CENTRAL_LEN as u64).is_none_or(|min| min > d.size) {
        return Err(ZipError::Corrupt);
    }
    if d.size > MAX_CENTRAL_DIRECTORY || d.entries > MAX_ENTRIES {
        return Err(ZipError::TooLarge);
    }
    let size = d.size as usize;
    let mut cd = Vec::new();
    cd.try_reserve_exact(size).map_err(|_| ZipError::TooLarge)?;
    cd.resize(size, 0);
    r.read_at(d.offset, &mut cd)?;
    let (entries, zip64_sizes) = parse_directory(&cd, d.entries, d.zip64)?;
    Ok(ZipIndex { entries, zip64: d.zip64 || zip64_sizes })
}

/// Voci della directory centrale (già letta per intero) e se qualche dimensione viene dal campo zip64.
fn parse_directory(cd: &[u8], declared: u64, zip64: bool) -> Result<(Vec<ArchiveEntry>, bool), ZipError> {
    let mut out: Vec<ArchiveEntry> = Vec::with_capacity(declared as usize);
    let mut zip64_sizes = false;
    let mut pos = 0usize;
    while le32(cd, pos) == Some(SIG_CENTRAL) {
        if out.len() as u64 >= MAX_ENTRIES {
            return Err(ZipError::TooLarge);
        }
        let h = cd.get(pos..pos + CENTRAL_LEN).ok_or(ZipError::Corrupt)?;
        let flags = field(le16(h, 8))?;
        let uncompressed = field(le32(h, 24))?;
        let name_len = usize::from(field(le16(h, 28))?);
        let extra_len = usize::from(field(le16(h, 30))?);
        let comment_len = usize::from(field(le16(h, 32))?);
        // Somme senza overflow: `pos` sta nella directory (al più 64 MiB) e ogni lunghezza è a 16 bit.
        let name_start = pos + CENTRAL_LEN;
        let name_end = name_start + name_len;
        let extra_end = name_end + extra_len;
        let next = extra_end + comment_len;
        if next > cd.len() {
            return Err(ZipError::Corrupt);
        }
        let size = if uncompressed == u32::MAX {
            zip64_sizes = true;
            zip64_size(&cd[name_end..extra_end]).ok_or(ZipError::Corrupt)?
        } else {
            u64::from(uncompressed)
        };
        let path = decode_name(&cd[name_start..name_end], flags & FLAG_UTF8 != 0)?;
        let dir = path.ends_with('/');
        out.push(ArchiveEntry { path, size, dir });
        pos = next;
    }
    // Dopo le voci può esserci solo la firma digitale dell'archivio.
    if pos < cd.len() && le32(cd, pos) != Some(SIG_DIGITAL_SIGNATURE) {
        return Err(ZipError::Corrupt);
    }
    // Senza zip64 il conteggio a 16 bit può essersi azzerato oltre 65.535 voci: conta il resto.
    let count = out.len() as u64;
    let coherent = if zip64 { count == declared } else { count & 0xFFFF == declared };
    if !coherent {
        return Err(ZipError::Corrupt);
    }
    Ok((out, zip64_sizes))
}

/// Dimensione non compressa dal campo extra zip64 (0x0001): è il primo valore quando quella del record
/// vale 0xFFFFFFFF.
fn zip64_size(extra: &[u8]) -> Option<u64> {
    let mut p = 0usize;
    while p + 4 <= extra.len() {
        let id = le16(extra, p)?;
        let n = usize::from(le16(extra, p + 2)?);
        let data = extra.get(p + 4..p + 4 + n)?;
        if id == ZIP64_EXTRA {
            return le64(data, 0);
        }
        p += 4 + n;
    }
    None
}

/// Nome della voce: UTF-8 con il bit 11, altrimenti CP437. Le barre rovesciate diventano `/`: le scrivono
/// .NET Framework e `Compress-Archive` di PowerShell 5.1 (misurato il 14/10/2026), contro APPNOTE 4.4.17,
/// e in un nome di Windows non possono stare.
fn decode_name(raw: &[u8], utf8: bool) -> Result<String, ZipError> {
    let decoded = if utf8 {
        std::str::from_utf8(raw).map_err(|_| ZipError::Corrupt)?.to_owned()
    } else {
        raw.iter().map(|&b| if b < 0x80 { char::from(b) } else { CP437_HIGH[usize::from(b - 0x80)] }).collect()
    };
    Ok(if decoded.contains('\\') { decoded.replace('\\', "/") } else { decoded })
}

/// Code page 437 (OEM degli Stati Uniti), byte da 0x80 a 0xFF: la codifica dei nomi senza il bit 11.
const CP437_HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', 'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', //
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', 'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', //
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', //
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', //
    '└', '┴', '┬', '├', '─', '┼', '╞', '╟', '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', //
    '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', //
    'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', 'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', //
    '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}',
];

#[cfg(test)]
mod tests {
    use super::*;

    /// La tabella coincide con la conversione di Windows per la code page 437.
    #[test]
    fn cp437_matches_windows() {
        use windows_sys::Win32::Globalization::MultiByteToWideChar;
        let bytes: Vec<u8> = (0x80..=0xFF).collect();
        let mut wide = [0u16; 128];
        // SAFETY: ingresso di 128 byte e uscita di 128 caratteri UTF-16, entrambi locali e delle
        // lunghezze dichiarate; nessun flag.
        let n = unsafe { MultiByteToWideChar(437, 0, bytes.as_ptr(), 128, wide.as_mut_ptr(), 128) };
        assert_eq!(n, 128);
        let windows: Vec<char> = char::decode_utf16(wide).map(|c| c.unwrap()).collect();
        assert_eq!(windows, CP437_HIGH.to_vec());
        assert_eq!(decode_name(&[b'c', 0x8A, b'.', b't', b'x', b't'], false).unwrap(), "cè.txt");
        assert_eq!(decode_name(&[0x85, 0x8D, 0x95, 0x97], false).unwrap(), "àìòù");
    }

    #[test]
    fn utf8_names_must_be_valid() {
        assert_eq!(decode_name("però/".as_bytes(), true).unwrap(), "però/");
        assert_eq!(decode_name(&[0xC3, 0x28], true), Err(ZipError::Corrupt));
    }

    #[test]
    fn backslashes_become_slashes() {
        assert_eq!(decode_name(br"sotto\vuota\", false).unwrap(), "sotto/vuota/");
        assert_eq!(decode_name("sotto\\è.txt".as_bytes(), true).unwrap(), "sotto/è.txt");
    }

    #[test]
    fn zip64_extra_field() {
        // Un campo estraneo, poi il campo zip64 con dimensione non compressa e compressa.
        let mut extra = vec![0x55, 0x54, 2, 0, 9, 9];
        extra.extend_from_slice(&[1, 0, 16, 0]);
        extra.extend_from_slice(&(5u64 << 32).to_le_bytes());
        extra.extend_from_slice(&7u64.to_le_bytes());
        assert_eq!(zip64_size(&extra), Some(5 << 32));
        assert_eq!(zip64_size(&extra[..10]), None, "campo tagliato");
        assert_eq!(zip64_size(&[1, 0, 4, 0, 1, 2, 3, 4]), None, "campo zip64 troppo corto");
        assert_eq!(zip64_size(&[]), None);
    }

    #[test]
    fn slices_read_exactly() {
        let data: &[u8] = &[1, 2, 3, 4];
        let mut b = [0u8; 2];
        data.read_at(2, &mut b).unwrap();
        assert_eq!(b, [3, 4]);
        assert_eq!(data.read_at(3, &mut b), Err(ZipError::Truncated));
        assert_eq!(data.read_at(u64::MAX, &mut b), Err(ZipError::Truncated));
        assert!(!ReadAt::is_empty(&data));
        assert!(ReadAt::is_empty(&Vec::<u8>::new()));
    }

    #[test]
    fn empty_archive() {
        let mut z = vec![0x50, 0x4b, 0x05, 0x06];
        z.extend_from_slice(&[0; 18]);
        assert_eq!(read_index(&z), Ok(ZipIndex::default()));
        assert_eq!(read_index(&z[..21].to_vec()), Err(ZipError::NotZip));
        assert_eq!(read_index(&Vec::new()), Err(ZipError::NotZip));
    }
}
