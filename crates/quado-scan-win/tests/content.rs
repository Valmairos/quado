// SPDX-License-Identifier: GPL-3.0-or-later
//! Identità e SHA-256 dei file su richiesta. Vettori noti e un oracolo scritto
//! nei test, annullamento, file cambiati durante la lettura, cartelle e collegamenti, file compressi e
//! sparse, segnaposto cloud della sync root di prova, nessun handle in scrittura.

mod common;

use std::io::{Seek, SeekFrom, Write};
use std::os::windows::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};
use std::time::Instant;

use common::{hex, pseudo_random};
use quado_scan_win::content::{BLOCK, ContentError, FileIdentity, identity, sha256, sha256_bytes};
use quado_scan_win::win;
use quado_scan_win::zip::{ZipError, zip_index};
use quado_testkit::cloud::{Placeholder, TestSyncRoot};
use quado_testkit::trees::{self, TestDir};

fn hash(p: &Path) -> Result<(FileIdentity, [u8; 32]), ContentError> {
    sha256(p.to_str().unwrap(), &AtomicBool::new(false), &AtomicU64::new(0))
}

fn digest(p: &Path) -> String {
    hex(&hash(p).unwrap_or_else(|e| panic!("{}: {e:?}", p.display())).1)
}

const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
const MILLION_A: &str = "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0";
const TWO_BLOCKS: &str = "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1";
const TWO_BLOCKS_INPUT: &[u8] = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";

/// L'oracolo dei test riproduce i vettori di FIPS 180-2: vale come riferimento indipendente da CNG.
#[test]
fn oracle_matches_the_known_vectors() {
    assert_eq!(hex(&common::sha256(b"")), EMPTY);
    assert_eq!(hex(&common::sha256(b"abc")), ABC);
    assert_eq!(hex(&common::sha256(TWO_BLOCKS_INPUT)), TWO_BLOCKS);
    assert_eq!(hex(&common::sha256(&vec![b'a'; 1_000_000])), MILLION_A);
    assert_eq!(common::crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn known_vectors_from_files() {
    let d = TestDir::new("hash-vettori");
    for (name, data, want) in [
        ("vuoto", b"".to_vec(), EMPTY),
        ("abc", b"abc".to_vec(), ABC),
        ("due-blocchi", TWO_BLOCKS_INPUT.to_vec(), TWO_BLOCKS),
        ("milione", vec![b'a'; 1_000_000], MILLION_A),
    ] {
        let p = d.join(name);
        std::fs::write(&p, &data).unwrap();
        assert_eq!(digest(&p), want, "{name}");
        assert_eq!(hex(&sha256_bytes(&data).unwrap()), want, "{name} in memoria");
    }
}

/// File più grandi di un blocco, attorno ai bordi dei blocchi; avanzamento uguale ai byte letti, anche
/// condiviso tra più file.
#[test]
fn files_larger_than_a_block() {
    let d = TestDir::new("hash-blocchi");
    let progress = AtomicU64::new(0);
    let cancel = AtomicBool::new(false);
    let mut total = 0u64;
    for (k, size) in [BLOCK - 1, BLOCK, BLOCK + 1, 2 * BLOCK + 4097].into_iter().enumerate() {
        let data = pseudo_random(size, k as u64 + 10);
        let p = d.join(&format!("f{k}.bin"));
        std::fs::write(&p, &data).unwrap();
        let before = progress.load(SeqCst);
        let (id, got) = sha256(p.to_str().unwrap(), &cancel, &progress).unwrap();
        assert_eq!(got, common::sha256(&data), "{size} byte");
        assert_eq!(id.size, size as u64);
        assert_eq!(progress.load(SeqCst) - before, size as u64);
        total += size as u64;
    }
    assert_eq!(progress.load(SeqCst), total);
}

#[test]
fn identity_matches_the_file_system() {
    let d = TestDir::new("identita");
    let p = d.join("file.txt");
    std::fs::write(&p, b"contenuto").unwrap();
    let id = identity(p.to_str().unwrap()).unwrap();
    let m = std::fs::metadata(&p).unwrap();
    assert_eq!(id.size, 9);
    assert_eq!(id.created as u64, m.creation_time());
    assert_eq!(id.modified as u64, m.last_write_time());
    assert_eq!(id.accessed as u64, m.last_access_time());
    assert_eq!(id.attributes, m.file_attributes());
    assert_ne!(id.file_id, 0);
    assert_ne!(id.volume_serial, 0);
    // Un hard link è lo stesso file; un altro file no.
    let link = d.join("collegamento.txt");
    std::fs::hard_link(&p, &link).unwrap();
    let other = d.join("altro.txt");
    std::fs::write(&other, b"contenuto").unwrap();
    let (l, o) = (identity(link.to_str().unwrap()).unwrap(), identity(other.to_str().unwrap()).unwrap());
    assert_eq!((l.file_id, l.volume_serial), (id.file_id, id.volume_serial));
    assert_ne!(o.file_id, id.file_id);
    assert_eq!(o.volume_serial, id.volume_serial);
    // Dopo l'hash l'identità restituita è quella di prima della lettura.
    let (before, _) = hash(&p).unwrap();
    assert_eq!((before.file_id, before.size, before.modified), (id.file_id, id.size, id.modified));
}

#[test]
fn folders_links_and_missing_files() {
    let d = TestDir::new("non-file");
    let dir = d.join("cartella");
    std::fs::create_dir(&dir).unwrap();
    let file = d.join("bersaglio.txt");
    std::fs::write(&file, b"bersaglio").unwrap();
    let s = |p: &Path| p.to_str().unwrap().to_owned();
    assert_eq!(identity(&s(&dir)), Err(ContentError::NotAFile));
    assert_eq!(hash(&dir).map(|_| ()), Err(ContentError::NotAFile));
    let j = d.join("junction");
    assert!(trees::junction(&j, &dir));
    assert_eq!(hash(&j).map(|_| ()), Err(ContentError::NotAFile));
    assert_eq!(identity(&s(&j)), Err(ContentError::NotAFile));
    let missing = d.join("manca.bin");
    assert_eq!(identity(&s(&missing)), Err(ContentError::Io(2)));
    assert_eq!(hash(&missing).map(|_| ()), Err(ContentError::Io(2)));
    assert_eq!(hash(&d.join("manca").join("x")).map(|_| ()), Err(ContentError::Io(3)));
    // File aperto da un altro senza condivisione: l'identità si legge, il contenuto no (32, file in uso).
    let locked = d.join("bloccato.zip");
    std::fs::write(&locked, b"PK").unwrap();
    let hold = trees::hold_exclusive(&locked);
    assert_eq!(identity(&s(&locked)).unwrap().size, 2);
    assert_eq!(hash(&locked).map(|_| ()), Err(ContentError::Io(32)));
    assert_eq!(zip_index(&s(&locked)), Err(ZipError::Io(32)));
    drop(hold);
    // Un carattere nullo non tronca il percorso (si aprirebbe la cartella `d`).
    let nul = format!("{}\0\\bersaglio.txt", d.path.display());
    assert_eq!(identity(&nul), Err(ContentError::Io(123)));
    assert_eq!(sha256(&nul, &AtomicBool::new(false), &AtomicU64::new(0)).map(|_| ()), Err(ContentError::Io(123)));
    assert_eq!(zip_index(&nul), Err(ZipError::Io(123)));
    // Collegamento simbolico a un file: richiede la Modalità sviluppatore.
    let sym = d.join("simbolico.txt");
    if std::os::windows::fs::symlink_file(&file, &sym).is_ok() {
        assert_eq!(hash(&sym).map(|_| ()), Err(ContentError::NotAFile));
        let id = identity(&s(&sym)).unwrap();
        assert_ne!(id.attributes & 0x400, 0, "identità del collegamento, non del bersaglio");
        assert_eq!(zip_index(&s(&sym)), Err(ZipError::NotZip));
    } else {
        eprintln!("collegamento simbolico non creato (Modalità sviluppatore spenta): caso non provato");
    }
    // Percorso oltre 260 caratteri.
    let deep = trees::long_path(&d.path, 300).join("lungo.txt");
    std::fs::write(&deep, b"abc").unwrap();
    assert!(deep.as_os_str().len() > 260);
    assert_eq!(digest(&deep), ABC);
    assert_eq!(identity(&s(&deep)).unwrap().size, 3);
}

/// Contenuto logico dei file compressi (NTFS e WOF, quest'ultimo un reparse point) e sparse.
#[test]
fn compressed_and_sparse_files() {
    let d = TestDir::new("hash-compressi");
    let ntfs = d.join("ntfs.txt");
    trees::write_compressible(&ntfs, 3_000_000);
    let want = common::sha256(&std::fs::read(&ntfs).unwrap());
    assert!(trees::ntfs_compress(&ntfs), "compact /c");
    let id = identity(ntfs.to_str().unwrap()).unwrap();
    assert_ne!(id.attributes & 0x800, 0, "attributo compresso: {:#x}", id.attributes);
    assert_eq!(hash(&ntfs).unwrap().1, want);

    // WOF: il filtro nasconde il reparse point agli attributi; che il file sia compresso lo dice l'allocata.
    let wof = d.join("wof.txt");
    trees::write_compressible(&wof, 3_000_000);
    let want = common::sha256(&std::fs::read(&wof).unwrap());
    assert!(trees::wof_compress(&wof), "compact /exe:lzx");
    let w: Vec<u16> = wof.as_os_str().to_str().unwrap().encode_utf16().collect();
    let si = win::standard_info(&win::open_attributes(&w).unwrap()).unwrap();
    let id = identity(wof.to_str().unwrap()).unwrap();
    eprintln!("WOF: attributi {:#x}, logica {}, allocata {}", id.attributes, si.logical, si.alloc);
    assert!(si.alloc < si.logical / 2, "il file non risulta compresso: {si:?}");
    assert_eq!(hash(&wof).unwrap().1, want);

    let sparse = d.join("sparso.bin");
    trees::sparse_file(&sparse, 8 << 20);
    let mut data = vec![0u8; 8 << 20];
    data[(8 << 20) - 4096..].fill(7);
    assert_eq!(hash(&sparse).unwrap().1, common::sha256(&data));
}

#[test]
fn cancel_before_and_during_the_read() {
    let d = TestDir::new("hash-annulla");
    let p = d.join("grande.bin");
    trees::write_file(&p, 64 << 20, 3);
    let path = p.to_str().unwrap();

    let progress = AtomicU64::new(0);
    assert_eq!(sha256(path, &AtomicBool::new(true), &progress), Err(ContentError::Cancelled));
    assert_eq!(progress.load(SeqCst), 0, "annullato prima di leggere");

    let cancel = AtomicBool::new(false);
    let r = std::thread::scope(|s| {
        s.spawn(|| {
            while progress.load(SeqCst) == 0 {
                std::hint::spin_loop();
            }
            cancel.store(true, SeqCst);
        });
        sha256(path, &cancel, &progress)
    });
    let read = progress.load(SeqCst);
    eprintln!("annullato dopo {read} byte su {}", 64 << 20);
    assert_eq!(r, Err(ContentError::Cancelled));
    assert!(read > 0 && read < 64 << 20);
}

/// Il file cambia durante la lettura: cresce, si accorcia, è riscritto sul posto. L'hash non vale (`Changed`).
#[test]
fn file_changed_during_the_read() {
    let d = TestDir::new("hash-cambiato");
    let size = 64u64 << 20;
    for (case, change) in [("cresce", 0u8), ("si accorcia", 1), ("riscritto sul posto", 2)] {
        let p = d.join(&format!("{change}.bin"));
        trees::write_file(&p, size as usize, 5);
        let progress = AtomicU64::new(0);
        let r = std::thread::scope(|s| {
            s.spawn(|| {
                while progress.load(SeqCst) == 0 {
                    std::hint::spin_loop();
                }
                let mut f = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
                if change == 0 {
                    f.seek(SeekFrom::End(0)).unwrap();
                    f.write_all(b"in coda").unwrap();
                } else if change == 1 {
                    f.set_len(size / 2).unwrap();
                } else {
                    // Stessa dimensione: lo dice la data di modifica.
                    f.seek(SeekFrom::Start(size - 4096)).unwrap();
                    f.write_all(&[0xAB; 16]).unwrap();
                }
            });
            sha256(p.to_str().unwrap(), &AtomicBool::new(false), &progress)
        });
        eprintln!("{case}: {r:?} dopo {} byte", progress.load(SeqCst));
        assert_eq!(r.map(|_| ()), Err(ContentError::Changed), "{case}");
    }
}

/// Misura (non verifica): costo di un hash su un file vuoto e velocità su un file grande.
#[test]
#[ignore = "misura: scrive 256 MiB nella cartella temporanea"]
fn measure_costs() {
    let d = TestDir::new("hash-misure");
    let empty = d.join("vuoto");
    std::fs::write(&empty, b"").unwrap();
    let n = 500;
    let t = Instant::now();
    for _ in 0..n {
        hash(&empty).unwrap();
    }
    let per_file = t.elapsed() / n;
    let t = Instant::now();
    for _ in 0..n {
        identity(empty.to_str().unwrap()).unwrap();
    }
    let per_identity = t.elapsed() / n;
    let big = d.join("grande.bin");
    let data = pseudo_random(256 << 20, 1);
    std::fs::write(&big, &data).unwrap();
    hash(&big).unwrap();
    let t = Instant::now();
    hash(&big).unwrap();
    let file = 256.0 / t.elapsed().as_secs_f64();
    let t = Instant::now();
    sha256_bytes(&data).unwrap();
    let memory = 256.0 / t.elapsed().as_secs_f64();
    eprintln!(
        "hash di un file vuoto: {per_file:?}; identità: {per_identity:?}; 256 MiB in cache: {file:.0} MiB/s; solo CNG in memoria: {memory:.0} MiB/s"
    );
}

/// Sui segnaposto cloud né hash né indice; l'identità sì, senza richiamarli. Il provider di
/// prova non riceve richieste di dati.
#[test]
fn cloud_placeholders_are_never_read() {
    let root = match TestSyncRoot::new("contenuto") {
        Ok(r) => r,
        Err(e) => panic!("provider di prova non registrato: {e:?}"),
    };
    root.create(
        "",
        &[
            Placeholder { name: "nel-cloud.bin", size: 5 << 20, dir: false, populated: false },
            Placeholder { name: "nel-cloud.zip", size: 1 << 20, dir: false, populated: false },
        ],
    )
    .unwrap();
    let local = root.path.join("locale.txt");
    std::fs::write(&local, b"abc").unwrap();
    let before = TestSyncRoot::requests();

    for name in ["nel-cloud.bin", "nel-cloud.zip"] {
        let p = root.path.join(name);
        let path = p.to_str().unwrap();
        let id = identity(path).unwrap();
        eprintln!("{name}: attributi {:#x}, dimensione {}", id.attributes, id.size);
        assert_eq!(hash(&p).map(|_| ()), Err(ContentError::CloudPlaceholder), "{name}");
        assert_eq!(zip_index(path), Err(ZipError::CloudPlaceholder), "{name}");
    }
    assert_eq!(identity(root.path.join("nel-cloud.bin").to_str().unwrap()).unwrap().size, 5 << 20);
    // Un file creato in locale nella sync root non è un segnaposto: si legge.
    assert_eq!(digest(&local), ABC);
    let after = TestSyncRoot::requests();
    assert_eq!(after, before, "il provider ha ricevuto richieste di dati o di elenco");
}
