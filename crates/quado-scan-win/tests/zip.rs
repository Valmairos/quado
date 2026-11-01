// SPDX-License-Identifier: GPL-3.0-or-later
//! F-DL-5: indice degli zip dalla directory centrale (06 §3.3). Archivi generati qui e uno prodotto da
//! .NET; file troncati, corrotti, non zip; limiti; test di proprietà. Le letture partono dalla directory
//! centrale: mai header locali né dati compressi.

mod common;

use std::cell::RefCell;
use std::process::Command;
use std::sync::OnceLock;

use common::{BuiltZip, ZipEntry, ZipOptions, build_zip, pseudo_random, sample_entries, sample_expected};
use proptest::prelude::*;
use quado_scan_win::zip::{
    MAX_CENTRAL_DIRECTORY, MAX_ENTRIES, ReadAt, SEARCH_STEP, TAIL_WINDOW, ZipError, ZipFile, ZipIndex, read_index, zip_index,
};
use quado_testkit::trees::{self, TestDir};

fn listing(ix: &ZipIndex) -> Vec<(String, u64, bool)> {
    ix.entries.iter().map(|e| (e.path.clone(), e.size, e.dir)).collect()
}

/// Sorgente che registra ogni lettura: (offset, lunghezza).
struct Recorder<'a> {
    data: &'a [u8],
    reads: RefCell<Vec<(u64, usize)>>,
}

impl<'a> Recorder<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, reads: RefCell::new(Vec::new()) }
    }
}

impl ReadAt for Recorder<'_> {
    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), ZipError> {
        self.reads.borrow_mut().push((offset, buf.len()));
        self.data.read_at(offset, buf)
    }
}

/// Commento con una firma di fine directory finta che "sta nel file" (commento vuoto): va preferito il
/// record vero, il cui commento arriva esattamente alla fine.
fn tricky_comment() -> Vec<u8> {
    let mut c = b"Commento: ".to_vec();
    c.extend_from_slice(b"PK\x05\x06");
    c.extend_from_slice(&[0; 18]);
    c.extend_from_slice(b" fine del commento.");
    c
}

/// Archivi di prova: (nome, archivio, zip64). Costruiti una volta sola (servono anche ai test di proprietà).
fn variants() -> &'static [(&'static str, BuiltZip, bool)] {
    static V: OnceLock<Vec<(&'static str, BuiltZip, bool)>> = OnceLock::new();
    V.get_or_init(build_variants)
}

fn build_variants() -> Vec<(&'static str, BuiltZip, bool)> {
    let e = sample_entries();
    let o = ZipOptions::default;
    vec![
        ("semplice", build_zip(&e, &o()), false),
        ("commento con firma finta", build_zip(&e, &ZipOptions { comment: tricky_comment(), ..o() }), false),
        ("commento di 65.535 byte", build_zip(&e, &ZipOptions { comment: vec![b'c'; 65_535], ..o() }), false),
        ("byte in coda", build_zip(&e, &ZipOptions { trailing: b"coda".repeat(50), ..o() }), false),
        ("autoestraente con offset assoluti", build_zip(&e, &ZipOptions { prefix: pseudo_random(10_000, 7), ..o() }), false),
        ("zip64 forzato", build_zip(&e, &ZipOptions { zip64: true, ..o() }), true),
        ("zip64 con commento", build_zip(&e, &ZipOptions { zip64: true, comment: tricky_comment(), ..o() }), true),
        ("dati grandi prima della directory", build_zip(&[ZipEntry::file("grande.bin", &pseudo_random(300_000, 3)), e[1].clone()], &o()), false),
    ]
}

#[test]
fn generated_archives_are_read() {
    for (name, z, zip64) in variants() {
        let ix = read_index(&z.bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(ix.zip64, *zip64, "{name}");
        if name.starts_with("dati grandi") {
            assert_eq!(listing(&ix), vec![("grande.bin".into(), 300_000, false), ("progetto/leggimi.txt".into(), 29, false)]);
        } else {
            assert_eq!(listing(&ix), sample_expected(), "{name}");
        }
    }
    assert_eq!(read_index(&build_zip(&[], &ZipOptions::default()).bytes), Ok(ZipIndex::default()));
    let empty64 = read_index(&build_zip(&[], &ZipOptions { zip64: true, ..Default::default() }).bytes).unwrap();
    assert!(empty64.entries.is_empty() && empty64.zip64);
}

/// 06 §3.3: letture solo nella coda, dalla directory centrale in poi. Unica eccezione dichiarata: con dei
/// byte dopo l'archivio la ricerca del record legge tutti gli ultimi 65.557 byte.
#[test]
fn reads_start_at_the_central_directory() {
    for (name, z, _) in variants() {
        let r = Recorder::new(&z.bytes);
        read_index(&r).unwrap();
        let reads = r.reads.into_inner();
        let first = reads.iter().map(|&(o, _)| o).min().unwrap();
        let bytes: usize = reads.iter().map(|&(_, l)| l).sum();
        eprintln!("{name}: {} letture, {bytes} byte, la prima a {first}; directory centrale da {}", reads.len(), z.cd_start);
        let len = z.bytes.len() as u64;
        let limit = if name.starts_with("byte in coda") { z.cd_start.min(len.saturating_sub(TAIL_WINDOW)) } else { z.cd_start };
        assert!(first >= limit, "{name}: lettura a {first} prima della directory centrale ({})", z.cd_start);
        for &(o, l) in &reads {
            assert!(o + l as u64 <= len, "{name}");
        }
    }
    // Senza commento: il record di fine directory e la directory centrale, nient'altro.
    let z = build_zip(&sample_entries(), &ZipOptions::default());
    let r = Recorder::new(&z.bytes);
    read_index(&r).unwrap();
    assert_eq!(r.reads.into_inner(), vec![(z.eocd, 22), (z.cd_start, z.cd_size as usize)]);
}

/// Con un commento e una directory centrale più corta del passo di ricerca, la ricerca legge al più 63 byte
/// prima del record di fine directory (limite dichiarato in `zip.rs`); la directory si legge a parte.
#[test]
fn comment_search_reads_at_most_one_step_before_the_record() {
    for comment in [1usize, 10, 41, 42, 43, 63, 64, 65, 200, 5000] {
        let z = build_zip(&[ZipEntry::file("a", b"x")], &ZipOptions { comment: vec![b'#'; comment], ..Default::default() });
        let r = Recorder::new(&z.bytes);
        assert_eq!(listing(&read_index(&r).unwrap()), vec![("a".into(), 1, false)]);
        for &(o, l) in r.reads.borrow().iter() {
            let directory = o == z.cd_start && l as u64 == z.cd_size;
            assert!(directory || o + (SEARCH_STEP - 1) >= z.eocd, "commento {comment}: lettura a {o}, record a {}", z.eocd);
        }
    }
}

/// Dopo le voci può stare solo la firma digitale dell'archivio (`PK\x05\x05`).
#[test]
fn digital_signature_after_the_entries() {
    let z = build_zip(&sample_entries(), &ZipOptions::default());
    let with = |sig: &[u8]| {
        let mut b = z.bytes[..z.eocd as usize].to_vec();
        b.extend_from_slice(sig);
        b.extend_from_slice(&[3, 0, 1, 2, 3]);
        let mut e = z.bytes[z.eocd as usize..].to_vec();
        e[12..16].copy_from_slice(&(z.cd_size as u32 + 9).to_le_bytes());
        b.extend_from_slice(&e);
        read_index(&b)
    };
    assert_eq!(listing(&with(b"PK\x05\x05").unwrap()), sample_expected());
    assert_eq!(with(b"XK\x05\x05"), Err(ZipError::Corrupt));
}

fn powershell(script: &str) {
    let out = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", script]).output().expect("powershell");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

/// Zip prodotti da .NET Framework (`ZipFile.CreateFromDirectory`) e da `Compress-Archive` di PowerShell
/// 5.1, compressi con deflate: produttori indipendenti dal generatore dei test. Entrambi scrivono i
/// separatori come `\` (contro APPNOTE): il lettore li restituisce come `/`.
#[test]
fn archives_made_by_windows_tools() {
    let d = TestDir::new("zip-dotnet");
    let src = d.join("sorgente");
    std::fs::create_dir_all(src.join("sotto").join("vuota")).unwrap();
    std::fs::write(src.join("a.txt"), b"0123456789").unwrap();
    std::fs::write(src.join("sotto").join("b.bin"), pseudo_random(70_000, 5)).unwrap();
    std::fs::write(src.join("perché.txt"), "testo".repeat(1000)).unwrap();
    let (src_n, net, ps) = (trees::native(&src), trees::native(&d.join("dotnet.zip")), trees::native(&d.join("compress.zip")));
    powershell(&format!(
        "Add-Type -AssemblyName System.IO.Compression.FileSystem; [IO.Compression.ZipFile]::CreateFromDirectory('{}', '{}')",
        src_n.display(),
        net.display()
    ));
    powershell(&format!("Compress-Archive -Path '{}\\*' -DestinationPath '{}'", src_n.display(), ps.display()));

    // Ordinate; la cartella vuota c'è, quella con dei file no (misurato il 14/10/2026 per entrambi).
    let want: Vec<(String, u64, bool)> = vec![
        ("a.txt".into(), 10, false),
        ("perché.txt".into(), 5000, false),
        ("sotto/b.bin".into(), 70_000, false),
        ("sotto/vuota/".into(), 0, true),
    ];
    for zip in [net, ps] {
        let raw = std::fs::read(&zip).unwrap();
        let backslashes = raw.windows(6).any(|w| w == b"sotto\\");
        let ix = zip_index(zip.to_str().unwrap()).unwrap();
        let mut got = listing(&ix);
        got.sort();
        eprintln!("{}: {got:?}; separatori \\ nell'archivio: {backslashes}", zip.display());
        assert_eq!(got, want, "{}", zip.display());
        assert!(!ix.zip64);

        // Anche qui: niente letture prima della directory centrale (archivio senza commento).
        let eocd = raw.len() - 22;
        let cd_start = u32::from_le_bytes(raw[eocd + 16..eocd + 20].try_into().unwrap()) as u64;
        let r = Recorder::new(&raw);
        assert_eq!(read_index(&r).unwrap(), ix);
        assert!(r.reads.borrow().iter().all(|&(o, _)| o >= cd_start), "{:?} contro {cd_start}", r.reads.borrow());
    }
}

#[test]
fn truncated_archives() {
    for zip64 in [false, true] {
        let z = build_zip(&sample_entries(), &ZipOptions { zip64, comment: b"fine".to_vec(), ..Default::default() });
        let len = z.bytes.len() as u64;
        for cut in 0..len {
            let got = read_index(&z.bytes[..cut as usize].to_vec());
            let want = if cut < z.eocd + 4 { ZipError::NotZip } else { ZipError::Truncated };
            assert_eq!(got, Err(want), "zip64 {zip64}, troncato a {cut} di {len}");
        }
    }
}

fn set16(b: &mut [u8], at: u64, v: u16) {
    b[at as usize..at as usize + 2].copy_from_slice(&v.to_le_bytes());
}

fn set32(b: &mut [u8], at: u64, v: u32) {
    b[at as usize..at as usize + 4].copy_from_slice(&v.to_le_bytes());
}

fn set64(b: &mut [u8], at: u64, v: u64) {
    b[at as usize..at as usize + 8].copy_from_slice(&v.to_le_bytes());
}

#[test]
fn corrupt_archives() {
    let z = build_zip(&sample_entries(), &ZipOptions::default());
    let n = sample_entries().len() as u16;
    let (cd, eocd, len) = (z.cd_start, z.eocd, z.bytes.len() as u64);
    type Mutation = Box<dyn Fn(&mut Vec<u8>)>;
    let cases: Vec<(&str, Mutation, ZipError)> = vec![
        ("firma della directory centrale", Box::new(move |b| b[cd as usize] = b'X'), ZipError::Corrupt),
        (
            "una voce in più dichiarata",
            Box::new(move |b| {
                set16(b, eocd + 8, n + 1);
                set16(b, eocd + 10, n + 1);
            }),
            ZipError::Corrupt,
        ),
        (
            "una voce in meno dichiarata",
            Box::new(move |b| {
                set16(b, eocd + 8, n - 1);
                set16(b, eocd + 10, n - 1);
            }),
            ZipError::Corrupt,
        ),
        ("voci che non stanno nella directory", Box::new(move |b| set16(b, eocd + 10, 60_000)), ZipError::Corrupt),
        ("nome oltre la directory", Box::new(move |b| set16(b, cd + 28, u16::MAX)), ZipError::Corrupt),
        ("directory oltre la fine del file", Box::new(move |b| set32(b, eocd + 16, len as u32 + 100)), ZipError::Truncated),
        ("directory che copre il record finale", Box::new(move |b| set32(b, eocd + 16, cd as u32 + 10)), ZipError::Corrupt),
        ("directory che punta agli header locali", Box::new(move |b| set32(b, eocd + 16, 0)), ZipError::Corrupt),
        ("dimensione enorme della directory", Box::new(move |b| set32(b, eocd + 12, 0xFFFF_FFF0)), ZipError::Truncated),
        ("campi saturi senza localizzatore", Box::new(move |b| set32(b, eocd + 12, u32::MAX)), ZipError::Truncated),
        ("archivio su più dischi", Box::new(move |b| set16(b, eocd + 4, 1)), ZipError::Corrupt),
        (
            "nome UTF-8 non valido",
            Box::new(move |b| {
                let flags = u16::from_le_bytes([b[cd as usize + 8], b[cd as usize + 9]]);
                set16(b, cd + 8, flags | 1 << 11);
                b[cd as usize + 46] = 0xFF;
            }),
            ZipError::Corrupt,
        ),
        ("dimensione zip64 senza campo extra", Box::new(move |b| set32(b, cd + 24, u32::MAX)), ZipError::Corrupt),
        ("directory dichiarata più corta delle voci", Box::new(move |b| set32(b, eocd + 12, (eocd - cd) as u32 - 10)), ZipError::Corrupt),
    ];
    for (name, mutate, want) in cases {
        let mut b = z.bytes.clone();
        mutate(&mut b);
        assert_eq!(read_index(&b), Err(want), "{name}");
    }

    // zip64: localizzatore e record.
    let z = build_zip(&sample_entries(), &ZipOptions { zip64: true, ..Default::default() });
    let loc = z.eocd - 20;
    let rec = loc - 56;
    let len = z.bytes.len() as u64;
    // Lunghezza dei dati del campo extra zip64 della prima voce ("progetto/", 9 byte di nome).
    let extra_len = z.cd_start + 46 + 9 + 2;
    let cases: Vec<(&str, Mutation, ZipError)> = vec![
        ("record zip64 oltre la fine", Box::new(move |b| set64(b, loc + 8, len)), ZipError::Truncated),
        ("record zip64 sovrapposto al localizzatore", Box::new(move |b| set64(b, loc + 8, loc - 30)), ZipError::Corrupt),
        ("firma del record zip64", Box::new(move |b| b[rec as usize] = b'X'), ZipError::Corrupt),
        ("lunghezza del record zip64", Box::new(move |b| set64(b, rec + 4, 10)), ZipError::Corrupt),
        ("record zip64 dichiarato più lungo", Box::new(move |b| set64(b, rec + 4, 44 + 100)), ZipError::Corrupt),
        ("dischi nel localizzatore", Box::new(move |b| set32(b, loc + 16, 2)), ZipError::Corrupt),
        ("disco nel record zip64", Box::new(move |b| set32(b, rec + 16, 1)), ZipError::Corrupt),
        ("voci zip64 in più", Box::new(move |b| set64(b, rec + 32, 8)), ZipError::Corrupt),
        ("voci zip64 enormi", Box::new(move |b| set64(b, rec + 32, u64::MAX)), ZipError::Corrupt),
        ("offset zip64 oltre ogni limite", Box::new(move |b| set64(b, rec + 48, u64::MAX - 3)), ZipError::Corrupt),
        ("campo extra zip64 senza la dimensione", Box::new(move |b| set16(b, extra_len, 4)), ZipError::Corrupt),
    ];
    for (name, mutate, want) in cases {
        let mut b = z.bytes.clone();
        mutate(&mut b);
        assert_eq!(read_index(&b), Err(want), "zip64: {name}");
    }

    // Dati anteposti senza correggere gli offset: nessuna ricostruzione.
    let mut b = pseudo_random(1000, 9);
    b.extend_from_slice(&build_zip(&sample_entries(), &ZipOptions::default()).bytes);
    assert!(read_index(&b).is_err());
}

#[test]
fn not_zip() {
    assert_eq!(read_index(&Vec::new()), Err(ZipError::NotZip));
    assert_eq!(read_index(&b"ciao".to_vec()), Err(ZipError::NotZip));
    assert_eq!(read_index(&vec![0u8; 21]), Err(ZipError::NotZip));
    let random = pseudo_random(100_000, 11);
    assert!(!random.windows(4).any(|w| w == b"PK\x05\x06"));
    assert_eq!(read_index(&random), Err(ZipError::NotZip));
    let text = "Non sono uno zip, solo testo.\r\n".repeat(5000).into_bytes();
    assert_eq!(read_index(&text), Err(ZipError::NotZip));
}

/// File virtuale: zeri ovunque tranne la coda.
struct Virtual {
    len: u64,
    tail: Vec<u8>,
}

impl ReadAt for Virtual {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), ZipError> {
        let start = self.len - self.tail.len() as u64;
        let end = offset.checked_add(buf.len() as u64).filter(|&e| e <= self.len).ok_or(ZipError::Truncated)?;
        buf.fill(0);
        if end > start {
            let from = offset.max(start);
            buf[(from - offset) as usize..].copy_from_slice(&self.tail[(from - start) as usize..(end - start) as usize]);
        }
        Ok(())
    }
}

/// Coda zip64 (record, localizzatore, fine directory) per una directory centrale di `size` byte e
/// `entries` voci all'inizio del file.
fn zip64_tail(size: u64, entries: u64) -> Virtual {
    let mut t = Vec::new();
    t.extend_from_slice(&0x0606_4b50u32.to_le_bytes());
    t.extend_from_slice(&44u64.to_le_bytes());
    t.extend_from_slice(&[45, 0, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    t.extend_from_slice(&entries.to_le_bytes());
    t.extend_from_slice(&entries.to_le_bytes());
    t.extend_from_slice(&size.to_le_bytes());
    t.extend_from_slice(&0u64.to_le_bytes());
    t.extend_from_slice(&0x0706_4b50u32.to_le_bytes());
    t.extend_from_slice(&0u32.to_le_bytes());
    t.extend_from_slice(&size.to_le_bytes());
    t.extend_from_slice(&1u32.to_le_bytes());
    t.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    t.extend_from_slice(&[0, 0, 0, 0]);
    t.extend_from_slice(&[0xFF; 12]);
    t.extend_from_slice(&[0, 0]);
    Virtual { len: size + t.len() as u64, tail: t }
}

#[test]
fn limits() {
    assert_eq!(read_index(&zip64_tail(MAX_CENTRAL_DIRECTORY + 1, 1)), Err(ZipError::TooLarge));
    assert_eq!(read_index(&zip64_tail(60 << 20, MAX_ENTRIES + 1)), Err(ZipError::TooLarge));
    // Al limite si legge (e la directory di zeri non è valida).
    assert_eq!(read_index(&zip64_tail(MAX_CENTRAL_DIRECTORY, 1)), Err(ZipError::Corrupt));
    // Voci incompatibili con la dimensione dichiarata: incoerente, non "troppo grande".
    assert_eq!(read_index(&zip64_tail(1000, MAX_ENTRIES + 1)), Err(ZipError::Corrupt));
    assert_eq!(read_index(&zip64_tail(4600, 101)), Err(ZipError::Corrupt));
}

/// Senza zip64, oltre 65.535 voci il conteggio a 16 bit si azzera: si contano le voci vere.
#[test]
fn more_than_65535_entries_without_zip64() {
    let entries: Vec<ZipEntry> = (0..65_537u32).map(|i| ZipEntry::file(&format!("{i:x}"), b"")).collect();
    let z = build_zip(&entries, &ZipOptions { wrap_count: true, ..Default::default() });
    let ix = read_index(&z.bytes).unwrap();
    assert_eq!(ix.entries.len(), 65_537);
    assert_eq!(ix.entries[65_536].path, "10000");
}

#[test]
fn files_on_disk() {
    let d = TestDir::new("zip-file");
    for (k, (name, z, _)) in variants().iter().enumerate() {
        let p = d.join(&format!("a{k}.zip"));
        std::fs::write(&p, &z.bytes).unwrap();
        let path = p.to_str().unwrap();
        assert_eq!(zip_index(path), read_index(&z.bytes), "{name}");
        let f = ZipFile::open(path).unwrap();
        assert_eq!(f.identity().size, z.bytes.len() as u64);
        assert_eq!(ReadAt::len(&f), z.bytes.len() as u64);
        let mut buf = [0u8; 4];
        assert_eq!(f.read_at(z.bytes.len() as u64 - 2, &mut buf), Err(ZipError::Truncated));
    }
    // Percorso oltre 260 caratteri.
    let deep = trees::long_path(&d.path, 300);
    let p = deep.join("lungo.zip");
    std::fs::write(&p, build_zip(&sample_entries(), &ZipOptions::default()).bytes).unwrap();
    assert!(p.as_os_str().len() > 260);
    assert_eq!(listing(&zip_index(p.to_str().unwrap()).unwrap()), sample_expected());
    // Cartella, file inesistente, file vuoto, junction.
    assert_eq!(zip_index(d.path.to_str().unwrap()), Err(ZipError::NotZip));
    assert_eq!(zip_index(d.join("manca.zip").to_str().unwrap()), Err(ZipError::Io(2)));
    std::fs::write(d.join("vuoto.zip"), b"").unwrap();
    assert_eq!(zip_index(d.join("vuoto.zip").to_str().unwrap()), Err(ZipError::NotZip));
    let j = d.join("collegamento");
    assert!(trees::junction(&j, &deep));
    assert_eq!(zip_index(j.to_str().unwrap()), Err(ZipError::NotZip));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn random_bytes_never_panic(b in proptest::collection::vec(any::<u8>(), 0..3000)) {
        let _ = read_index(&b);
    }

    /// Byte casuali con una firma di fine directory e un commento: si arriva fino al parser delle voci.
    #[test]
    fn random_records_never_panic(
        head in proptest::collection::vec(any::<u8>(), 0..600),
        record in proptest::collection::vec(any::<u8>(), 18),
        comment in proptest::collection::vec(any::<u8>(), 0..80),
        central in any::<bool>(),
    ) {
        let mut b = head;
        if central && b.len() >= 4 {
            b[..4].copy_from_slice(b"PK\x01\x02");
        }
        b.extend_from_slice(b"PK\x05\x06");
        b.extend_from_slice(&record);
        b.extend_from_slice(&comment);
        let _ = read_index(&b);
    }

    #[test]
    fn mutated_archives_never_panic(
        which in 0usize..8,
        edits in proptest::collection::vec((any::<u32>(), any::<u8>(), 0u8..4), 1..12),
    ) {
        let mut b = variants()[which].1.bytes.clone();
        for (at, byte, kind) in edits {
            if b.is_empty() {
                break;
            }
            // Le modifiche cadono di preferenza nella coda, dove stanno le strutture lette.
            let tail = b.len().min(400);
            let k = b.len() - 1 - (at as usize % tail);
            match kind {
                0 => b[k] = byte,
                1 => b[k] ^= 1 << (byte % 8),
                2 => b.truncate(k),
                _ => b.insert(k, byte),
            }
        }
        if let Ok(ix) = read_index(&b) {
            prop_assert!(ix.entries.len() <= b.len() / 46);
        }
    }

    /// Ciò che il generatore scrive, il lettore lo rilegge uguale.
    #[test]
    fn round_trip(
        files in proptest::collection::vec(("[a-z]{1,6}(/[a-zàèéìòù ]{1,6}){0,2}", 0usize..300, any::<bool>()), 0..20),
        zip64 in any::<bool>(),
        comment in proptest::collection::vec(any::<u8>(), 0..200),
    ) {
        let entries: Vec<ZipEntry> = files
            .iter()
            .map(|(n, size, dir)| if *dir { ZipEntry::dir(&format!("{n}/")) } else { ZipEntry::file(n, &vec![7u8; *size]) })
            .collect();
        let z = build_zip(&entries, &ZipOptions { zip64, comment, ..Default::default() });
        let ix = read_index(&z.bytes).unwrap();
        let want: Vec<(String, u64, bool)> = files
            .iter()
            .map(|(n, size, dir)| if *dir { (format!("{n}/"), 0, true) } else { (n.clone(), *size as u64, false) })
            .collect();
        prop_assert_eq!(listing(&ix), want);
        prop_assert_eq!(ix.zip64, zip64);
    }
}
