// SPDX-License-Identifier: GPL-3.0-or-later
//! Protocollo dell'helper (05 D3, 06 §11): codifica e decodifica di richieste e risposte, validazione dei
//! percorsi, ambito delle cartelle. Ingressi casuali (`proptest`): nessun panico e nessuna allocazione
//! oltre i limiti, misurata con un allocatore che registra la richiesta più grande del thread.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use proptest::prelude::*;
use quado_model::{DirExtra, Flags, NONE, NodeData, Tree};
use quado_scan_win::Problem;
use quado_scan_win::helper::proto::{
    MAX_FOLDERS, MAX_PATH_UNITS, MAX_REQUEST_BYTES, PathIssue, ProtoError, Reply, Request, Scope, ShadowStorage, Totals, VERSION,
    classify, decode_reply, decode_request, encode_reply, encode_request,
};

// ---------------------------------------------------------------------------------------------------
// Allocazione più grande, per thread

struct Peak;

thread_local! {
    static PEAK: Cell<usize> = const { Cell::new(0) };
}

fn note(n: usize) {
    let _ = PEAK.try_with(|p| {
        if n > p.get() {
            p.set(n);
        }
    });
}

// SAFETY: inoltra ogni chiamata a `System` con gli stessi argomenti; registra solo la dimensione in una
// cella del thread inizializzata a costante (nessuna allocazione dentro l'allocatore).
unsafe impl GlobalAlloc for Peak {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: stesso contratto di `GlobalAlloc::alloc`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        // SAFETY: stesso contratto di `GlobalAlloc::alloc_zeroed`.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        // SAFETY: stesso contratto di `GlobalAlloc::realloc`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: stesso contratto di `GlobalAlloc::dealloc`.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOC: Peak = Peak;

/// Allocazione singola più grande durante `f`, in questo thread.
fn peak<T>(f: impl FnOnce() -> T) -> (T, usize) {
    PEAK.with(|p| p.set(0));
    let r = f();
    (r, PEAK.with(Cell::get))
}

/// Limite per la decodifica di `len` byte: proporzionale all'ingresso, più i blocchi fissi del pool dei
/// nomi del modello (4 Mi unità: 4 MiB per i nomi ASCII, 8 MiB per gli altri).
fn alloc_bound(len: usize) -> usize {
    (12 << 20) + 16 * len
}

#[test]
fn the_peak_counter_sees_allocations() {
    // Controllo del controllo: un'allocazione da 1 MiB nel thread si vede.
    let (v, p) = peak(|| vec![1u8; 1 << 20]);
    assert!(p >= 1 << 20 && v.len() == 1 << 20, "{p}");
}

// ---------------------------------------------------------------------------------------------------
// Utilità

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn req(volume: &str, folders: &[&str]) -> Request {
    Request { volume: volume.to_owned(), folders: folders.iter().map(|s| (*s).to_owned()).collect() }
}

fn put_u32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}

fn put_str(b: &mut Vec<u8>, units: &[u16]) {
    put_u32(b, units.len() as u32);
    for u in units {
        b.extend_from_slice(&u.to_le_bytes());
    }
}

/// Corpo di una richiesta scritto a mano (per ingressi che `encode_request` non produrrebbe).
fn raw_request(version: u16, volume: &[u16], folders: &[Vec<u16>]) -> Vec<u8> {
    let mut b = vec![0x01];
    b.extend_from_slice(&version.to_le_bytes());
    put_str(&mut b, volume);
    put_u32(&mut b, folders.len() as u32);
    for f in folders {
        put_str(&mut b, f);
    }
    b
}

fn dir() -> NodeData {
    NodeData { flags: Flags::DIR, ..Default::default() }
}

fn file(logical: u64, alloc: u64) -> NodeData {
    NodeData { logical, alloc, mtime: 133_000_000_000_000_000, attrs: 0x20, ..Default::default() }
}

/// Albero di prova con un nome non valido in UTF-16 (surrogato spaiato), dati delle cartelle e un file
/// con più nomi.
fn sample_tree() -> Tree {
    let mut t = Tree::with_root(&w(r"C:\Users\io\AppData\Negata"), 0);
    t.add_flags(Tree::ROOT, Flags::PARTIAL);
    let a = t.push(Tree::ROOT, &w("Università"), dir());
    let bad = vec![u16::from(b'x'), 0xD800, u16::from(b'y')];
    let b = t.push(a, &bad, dir());
    t.push(b, &w("dati.bin"), file(1000, 4096));
    let l = t.push(a, &w("collegamento.bin"), file(1000, 4096));
    t.add_flags(l, Flags::SECONDARY_LINK | Flags::MULTI_LINK);
    t.push(Tree::ROOT, &[0xDC00], file(7, 0));
    let d = t.push(Tree::ROOT, &w("negata"), NodeData { flags: Flags::DIR | Flags::DENIED, ..Default::default() });
    t.set_dir_extra(DirExtra { node: b, index_alloc: 4096, error: 0 });
    t.set_dir_extra(DirExtra { node: d, index_alloc: 0, error: 5 });
    t.set_dir_extra(DirExtra { node: a, index_alloc: 8192, error: 0 });
    t.seal();
    t.aggregate();
    t
}

fn assert_same_tree(a: &Tree, b: &Tree) {
    assert_eq!(a.len(), b.len());
    for i in 0..a.len() as u32 {
        assert_eq!(a.parent(i), b.parent(i), "genitore di {i}");
        assert_eq!(a.name(i).to_utf16(), b.name(i).to_utf16(), "nome di {i}");
        assert_eq!((a.logical(i), a.alloc(i), a.files(i)), (b.logical(i), b.alloc(i), b.files(i)), "totali di {i}");
        assert_eq!((a.mtime(i), a.attrs(i), a.flags(i)), (b.mtime(i), b.attrs(i), b.flags(i)), "dati di {i}");
        assert_eq!(a.dir_extra(i), b.dir_extra(i), "cartella {i}");
    }
    assert_eq!(a.dir_extras(), b.dir_extras());
}

fn codes(p: &[(u32, Problem)]) -> Vec<(u32, u32)> {
    p.iter().map(|(n, p)| (*n, p.code())).collect()
}

/// Confronto di due risposte (`Reply` non ha `PartialEq`: l'albero non lo ha).
fn assert_same_reply(a: &Reply, b: &Reply) {
    match (a, b) {
        (Reply::Folder { index: i1, tree: t1, problems: p1 }, Reply::Folder { index: i2, tree: t2, problems: p2 }) => {
            assert_eq!(i1, i2);
            assert_same_tree(t1, t2);
            assert_eq!(codes(p1), codes(p2));
        }
        _ => assert_eq!(format!("{a:?}"), format!("{b:?}")),
    }
}

fn round_trip(r: &Reply) -> Reply {
    decode_reply(&encode_reply(r)).expect("la risposta si decodifica")
}

// ---------------------------------------------------------------------------------------------------
// Richiesta: casi a mano

#[test]
fn request_round_trip() {
    let r = req(r"C:\", &[r"C:\Users\altro", r"c:\$Recycle.Bin\S-1-5-21-1-2-3-1001", r"C:\Programmi\Università\cartella con spazi"]);
    let body = encode_request(&r);
    assert_eq!(body[0], 0x01);
    assert_eq!(u16::from_le_bytes([body[1], body[2]]), VERSION);
    assert_eq!(decode_request(&body), Ok(r));
}

#[test]
fn request_framing_errors() {
    let good = encode_request(&req(r"C:\", &[r"C:\a"]));
    assert_eq!(decode_request(&[]), Err(ProtoError::Malformed));
    for cut in 1..good.len() {
        assert!(decode_request(&good[..cut]).is_err(), "troncata a {cut}");
    }
    let mut extra = good.clone();
    extra.push(0);
    assert_eq!(decode_request(&extra), Err(ProtoError::Malformed), "byte in più");
    let mut kind = good.clone();
    kind[0] = 0x10;
    assert_eq!(decode_request(&kind), Err(ProtoError::Malformed), "tipo di una risposta");
    assert_eq!(decode_request(&raw_request(2, &w(r"C:\"), &[w(r"C:\a")])), Err(ProtoError::Version(2)));
    assert_eq!(decode_request(&vec![0x01; MAX_REQUEST_BYTES + 1]), Err(ProtoError::TooLarge));
    // Numero di cartelle dichiarato più grande dei byte presenti: nessuna allocazione proporzionale.
    let mut liar = raw_request(VERSION, &w(r"C:\"), &[]);
    let n = liar.len();
    liar[n - 4..].copy_from_slice(&(MAX_FOLDERS as u32).to_le_bytes());
    let (r, peak) = peak(|| decode_request(&liar));
    assert_eq!(r, Err(ProtoError::Malformed));
    assert!(peak < 4096, "{peak}");
    // Lunghezza di una cartella oltre il limite, dichiarata e non presente: rifiutata prima di leggere.
    let mut long = raw_request(VERSION, &w(r"C:\"), &[w("x")]);
    let n = long.len();
    long[n - 6..n - 2].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(decode_request(&long), Err(ProtoError::BadPath(0, PathIssue::TooLong)));
}

#[test]
fn request_volume_must_be_a_drive_root() {
    for v in ["C:", r"C:\x", "C:/", r"1:\", r"\\?\C:\", "", r"CC\"] {
        assert_eq!(req(v, &[r"C:\a"]).validate(), Err(ProtoError::BadVolume), "{v}");
        assert_eq!(decode_request(&raw_request(VERSION, &w(v), &[w(r"C:\a")])), Err(ProtoError::BadVolume), "{v}");
    }
    // Volume lungo dichiarato ma non presente: rifiutato dalla lunghezza.
    let mut b = vec![0x01];
    b.extend_from_slice(&VERSION.to_le_bytes());
    put_u32(&mut b, u32::MAX);
    assert_eq!(decode_request(&b), Err(ProtoError::BadVolume));
    assert_eq!(decode_request(&raw_request(VERSION, &[u16::from(b'C'), u16::from(b':'), 0xD800], &[])), Err(ProtoError::BadVolume));
    assert_eq!(req(r"r:\", &[r"R:\a"]).validate(), Ok(()));
}

#[test]
fn request_rejects_each_kind_of_bad_path() {
    use PathIssue::*;
    let cases: &[(&str, PathIssue)] = &[
        ("a", Relative),
        (r"a\b", Relative),
        (r"\Windows", Relative),
        ("C:", Relative),
        ("C:Windows", Relative),
        (r"..\Windows", Relative),
        (r"D:\Windows", OtherVolume),
        (r"C:\", VolumeRoot),
        (r"c:\", VolumeRoot),
        (r"C:\Windows\..\Users", NotNormalized),
        (r"C:\..", NotNormalized),
        (r"C:\.\Windows", NotNormalized),
        (r"C:\Windows\.", NotNormalized),
        (r"C:\Windows\\System32", NotNormalized),
        (r"C:\Windows\", NotNormalized),
        ("C:/Windows", NotNormalized),
        (r"C:\Windows/System32", NotNormalized),
        ("C:\\Win\u{1}dows", NotNormalized),
        ("C:\\Win\ndows", NotNormalized),
        (r"\\?\C:\Windows", NotNormalized),
        (r"\\.\C:\Windows", NotNormalized),
        (r"\\server\share\x", NotNormalized),
        (r"\\?\UNC\server\share", NotNormalized),
        (r"C:\file.txt:stream", Stream),
        (r"C:\dir:$I30:$INDEX_ALLOCATION", Stream),
        (r"C:\a\b::$DATA", Stream),
    ];
    for (p, issue) in cases {
        let r = req(r"C:\", &[r"C:\ok", p]);
        assert_eq!(r.validate(), Err(ProtoError::BadPath(1, *issue)), "{p}");
        assert_eq!(decode_request(&encode_request(&r)), Err(ProtoError::BadPath(1, *issue)), "{p}");
    }
    // Surrogato spaiato: non è un percorso che l'helper possa ricevere come testo.
    let raw = raw_request(VERSION, &w(r"C:\"), &[w(r"C:\ok"), vec![u16::from(b'C'), u16::from(b':'), u16::from(b'\\'), 0xDC00]]);
    assert_eq!(decode_request(&raw), Err(ProtoError::BadPath(1, NotNormalized)));
}

#[test]
fn request_limits_length_and_count() {
    let max = format!(r"C:\{}", "a".repeat(MAX_PATH_UNITS - 3));
    assert_eq!(req(r"C:\", &[&max]).validate(), Ok(()));
    let over = format!(r"C:\{}", "a".repeat(MAX_PATH_UNITS - 2));
    let r = req(r"C:\", &[r"C:\x", &over]);
    assert_eq!(r.validate(), Err(ProtoError::BadPath(1, PathIssue::TooLong)));
    assert_eq!(decode_request(&encode_request(&r)), Err(ProtoError::BadPath(1, PathIssue::TooLong)));
    // Le unità si contano in UTF-16: un carattere fuori dal piano di base ne vale due.
    let wide = format!(r"C:\{}", "😀".repeat((MAX_PATH_UNITS - 3) / 2 + 1));
    assert_eq!(req(r"C:\", &[&wide]).validate(), Err(ProtoError::BadPath(0, PathIssue::TooLong)));

    let folders: Vec<String> = (0..MAX_FOLDERS).map(|i| format!(r"C:\d{i}")).collect();
    let ok = Request { volume: r"C:\".into(), folders: folders.clone() };
    assert_eq!(decode_request(&encode_request(&ok)), Ok(ok));
    let mut many = folders;
    many.push(r"C:\ultima".into());
    let r = Request { volume: r"C:\".into(), folders: many };
    assert_eq!(r.validate(), Err(ProtoError::TooManyFolders));
    assert_eq!(decode_request(&encode_request(&r)), Err(ProtoError::TooManyFolders));
}

#[test]
fn request_rejects_equal_and_nested_folders() {
    let dup = |folders: &[&str]| req(r"C:\", folders).validate();
    assert_eq!(dup(&[r"C:\a", r"C:\b", r"C:\a"]), Err(ProtoError::BadPath(2, PathIssue::Duplicate)));
    assert_eq!(dup(&[r"C:\Users\Altro", r"c:\USERS\altro"]), Err(ProtoError::BadPath(1, PathIssue::Duplicate)));
    assert_eq!(dup(&[r"C:\a\b\c", r"C:\x", r"C:\a"]), Err(ProtoError::BadPath(0, PathIssue::Duplicate)));
    assert_eq!(dup(&[r"C:\A", r"C:\a!b", r"C:\a\b"]), Err(ProtoError::BadPath(2, PathIssue::Duplicate)));
    // Prefisso senza confine di componente: non annidate.
    assert_eq!(dup(&[r"C:\a", r"C:\ab", r"C:\a b", r"C:\a.b", r"C:\a!"]), Ok(()));
    assert_eq!(dup(&[r"C:\Users\altro", r"C:\Users\altro2"]), Ok(()));
}

// ---------------------------------------------------------------------------------------------------
// Ambito

const OWN_SID: &str = "S-1-5-21-1000-2000-3000-1001";
const OTHER_SID: &str = "S-1-5-21-1000-2000-3000-1002";

#[test]
fn classify_with_the_own_profile() {
    let c = |f: &str| classify(r"C:\", f, Some(r"C:\Users\io"), Some(OWN_SID));
    assert_eq!(c(r"C:\Users\io"), Scope::Tree);
    assert_eq!(c(r"C:\Users\io\AppData\Local\Negata"), Scope::Tree);
    assert_eq!(c(r"c:\users\IO\AppData"), Scope::Tree);
    assert_eq!(c(r"C:\Users\altro"), Scope::Profile);
    assert_eq!(c(r"C:\Users\altro\Documenti\segreto"), Scope::Profile);
    assert_eq!(c(r"C:\Users\io2"), Scope::Profile);
    assert_eq!(c(r"C:\Users\Default"), Scope::Profile);
    assert_eq!(c(r"C:\Users\Public\Documents"), Scope::Profile);
    // Gli antenati dei profili e dei Cestini altrui non si elencano con i nomi.
    assert_eq!(c(r"C:\Users"), Scope::Profile);
    assert_eq!(c(r"C:\$Recycle.Bin"), Scope::Profile);
    assert_eq!(c(&format!(r"C:\$Recycle.Bin\{OTHER_SID}")), Scope::Bin { sid: OTHER_SID.into() });
    assert_eq!(c(&format!(r"C:\$RECYCLE.BIN\{}\$RABC123.docx", OTHER_SID.to_lowercase())), Scope::Bin { sid: OTHER_SID.to_lowercase() });
    assert_eq!(c(&format!(r"C:\$Recycle.Bin\{OWN_SID}")), Scope::Tree);
    assert_eq!(c(&format!(r"C:\$Recycle.Bin\{}\x", OWN_SID.to_lowercase())), Scope::Tree);
    assert_eq!(c(r"C:\$Recycle.Bin\desktop.ini"), Scope::Tree);
    assert_eq!(c(r"C:\Windows\System32\config"), Scope::Tree);
    assert_eq!(c(r"C:\ProgramData\Microsoft\Crypto"), Scope::Tree);
    assert_eq!(c(r"C:\UsersX\altro"), Scope::Tree);
    assert_eq!(c(r"C:\System Volume Information"), Scope::Tree);
    // Altro volume: stesse regole; il profilo proprio è su C:.
    assert_eq!(classify(r"R:\", r"R:\Users\io", Some(r"C:\Users\io"), Some(OWN_SID)), Scope::Profile);
    assert_eq!(classify(r"R:\", r"r:\$Recycle.Bin\S-1-5-21-9", Some(r"C:\Users\io"), Some(OWN_SID)), Scope::Bin { sid: "S-1-5-21-9".into() });
}

#[test]
fn classify_without_the_profile_or_the_sid() {
    // Profilo ignoto: ogni profilo è altrui (si perdono i nomi, non se ne mostrano di altrui).
    assert_eq!(classify(r"C:\", r"C:\Users\io\AppData", None, Some(OWN_SID)), Scope::Profile);
    assert_eq!(classify(r"C:\", r"C:\Windows\x", None, None), Scope::Tree);
    // SID ignoto: ogni Cestino è altrui.
    assert_eq!(classify(r"C:\", &format!(r"C:\$Recycle.Bin\{OWN_SID}"), None, None), Scope::Bin { sid: OWN_SID.into() });
}

#[test]
fn classify_normalizes_the_profile_and_ignores_ancestors_of_users() {
    for own in [r"C:\Users\io\", r"\\?\C:\Users\io", r"c:\users\io"] {
        assert_eq!(classify(r"C:\", r"C:\Users\io\x", Some(own), None), Scope::Tree, "{own}");
    }
    // Un "profilo" che contiene tutta `Users` non rende propri i profili degli altri.
    for own in [r"C:\Users", r"C:\", "C:", r"C:\Users\"] {
        assert_eq!(classify(r"C:\", r"C:\Users\altro\x", Some(own), None), Scope::Profile, "{own}");
    }
    // Profilo fuori da `Users`: tutti i profili in `Users` sono altrui.
    assert_eq!(classify(r"C:\", r"C:\Users\io", Some(r"D:\Profili\io"), None), Scope::Profile);
    assert_eq!(classify(r"C:\", r"D:\Profili\io\x", Some(r"D:\Profili\io"), None), Scope::Tree);
}

// ---------------------------------------------------------------------------------------------------
// Risposte: casi a mano

#[test]
fn every_reply_round_trips() {
    let totals = Totals { logical: 123_456_789, alloc: 130_000_000, files: 4321, dirs: 87, unreadable: 2 };
    let replies = [
        Reply::Started { backup_privilege: true },
        Reply::Started { backup_privilege: false },
        Reply::Totals { index: 3, totals },
        Reply::Bin { index: 4, sid: OTHER_SID.into(), totals, orphan: true },
        Reply::Bin { index: 5, sid: OTHER_SID.into(), totals: Totals::default(), orphan: false },
        Reply::Failed { index: 6, code: 5 },
        Reply::Shadow(Ok(None)),
        Reply::Shadow(Ok(Some(ShadowStorage { used: 1 << 30, allocated: 2 << 30, max: u64::MAX }))),
        Reply::Shadow(Err(0x8004_1010)),
        Reply::Done { code: 0 },
        Reply::Done { code: 1300 },
        Reply::Folder { index: 9, tree: Box::new(sample_tree()), problems: vec![(5, Problem::AccessDenied), (0, Problem::Other(87)), (3, Problem::Cancelled)] },
    ];
    for r in &replies {
        assert_same_reply(r, &round_trip(r));
    }
}

#[test]
fn folder_tree_keeps_invalid_utf16_names_and_totals() {
    let t = sample_tree();
    let Reply::Folder { tree, .. } = round_trip(&Reply::Folder { index: 0, tree: Box::new(t.clone()), problems: vec![] }) else { panic!("Folder") };
    let bad: Vec<u16> = vec![u16::from(b'x'), 0xD800, u16::from(b'y')];
    let names: Vec<Vec<u16>> = (0..tree.len() as u32).map(|i| tree.name(i).to_utf16()).collect();
    assert!(names.contains(&bad));
    assert!(names.contains(&vec![0xDC00]));
    // Il nome secondario non conta: totali come l'originale.
    assert_eq!((tree.logical(Tree::ROOT), tree.alloc(Tree::ROOT), tree.files(Tree::ROOT)), (1007, 4096, 2));
    assert_eq!(tree.dir_index_total(), 12288);
    assert!(tree.flags(Tree::ROOT).has(Flags::PARTIAL));
}

#[test]
fn problems_travel_as_codes() {
    // `Interrupted` torna come `Other` con lo stesso codice (`Problem::from_code`).
    let r = Reply::Folder { index: 0, tree: Box::new(sample_tree()), problems: vec![(1, Problem::Interrupted(1117)), (2, Problem::InUse), (3, Problem::NotFound)] };
    let Reply::Folder { problems, .. } = round_trip(&r) else { panic!("Folder") };
    assert_eq!(problems, vec![(1, Problem::Other(1117)), (2, Problem::InUse), (3, Problem::NotFound)]);
}

/// Nodo scritto a mano: genitore, nome, logical, alloc, mtime, attrs, flags.
type RawNode<'a> = (u32, &'a [u16], u64, u64, i64, u32, u16);

/// Corpo di un `Folder` scritto a mano.
fn raw_folder(nodes: &[RawNode<'_>], extras: &[(u32, u64, u32)], problems: &[(u32, u32)]) -> Vec<u8> {
    let mut b = vec![0x11];
    put_u32(&mut b, 0);
    put_u32(&mut b, nodes.len() as u32);
    for &(parent, name, logical, alloc, mtime, attrs, flags) in nodes {
        put_u32(&mut b, parent);
        put_str(&mut b, name);
        b.extend_from_slice(&logical.to_le_bytes());
        b.extend_from_slice(&alloc.to_le_bytes());
        b.extend_from_slice(&mtime.to_le_bytes());
        put_u32(&mut b, attrs);
        b.extend_from_slice(&flags.to_le_bytes());
    }
    put_u32(&mut b, extras.len() as u32);
    for &(node, index_alloc, error) in extras {
        put_u32(&mut b, node);
        b.extend_from_slice(&index_alloc.to_le_bytes());
        put_u32(&mut b, error);
    }
    put_u32(&mut b, problems.len() as u32);
    for &(node, code) in problems {
        put_u32(&mut b, node);
        put_u32(&mut b, code);
    }
    b
}

#[test]
fn folder_tree_structure_is_checked() {
    let root = w(r"C:\x");
    let a = w("a");
    let d = Flags::DIR.0;
    let ok = raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &a, 0, 0, 0, 0, d), (1, &w("f"), 5, 8, 1, 0x20, 0)], &[(1, 4096, 0)], &[(2, 5)]);
    assert!(decode_reply(&ok).is_ok());
    // Data e attributi della radice non si ricostruiscono (come `Tree::with_root`): ignorati.
    let dated = raw_folder(&[(NONE, &root, 0, 0, 9, 0x10, d)], &[], &[]);
    let Ok(Reply::Folder { tree, .. }) = decode_reply(&dated) else { panic!("radice con data") };
    assert_eq!((tree.mtime(Tree::ROOT), tree.attrs(Tree::ROOT)), (0, 0));
    let bad: Vec<(&str, Vec<u8>)> = vec![
        ("nessun nodo", raw_folder(&[], &[], &[])),
        ("radice con genitore", raw_folder(&[(0, &root, 0, 0, 0, 0, d)], &[], &[])),
        ("radice non cartella", raw_folder(&[(NONE, &root, 0, 0, 0, 0, 0)], &[], &[])),
        ("radice senza nome", raw_folder(&[(NONE, &[], 0, 0, 0, 0, d)], &[], &[])),
        ("figlio prima del genitore", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (2, &a, 0, 0, 0, 0, d), (0, &a, 0, 0, 0, 0, d)], &[], &[])),
        ("genitore se stesso", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (1, &a, 0, 0, 0, 0, d)], &[], &[])),
        ("seconda radice", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (NONE, &a, 0, 0, 0, 0, d)], &[], &[])),
        ("genitore file", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &a, 1, 1, 0, 0, 0), (1, &a, 1, 1, 0, 0, 0)], &[], &[])),
        ("nome vuoto", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &[], 0, 0, 0, 0, d)], &[], &[])),
        ("nome ..", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &w(".."), 0, 0, 0, 0, d)], &[], &[])),
        ("nome .", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &w("."), 0, 0, 0, 0, d)], &[], &[])),
        ("nome con separatore", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &w(r"..\..\Windows"), 0, 0, 0, 0, d)], &[], &[])),
        ("nome con /", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &w("a/b"), 0, 0, 0, 0, 0)], &[], &[])),
        ("nome con flusso", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &w("a:b"), 0, 0, 0, 0, 0)], &[], &[])),
        ("nome con NUL", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &[97, 0, 98], 0, 0, 0, 0, 0)], &[], &[])),
        ("nome troppo lungo", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &vec![97; 0x8000], 0, 0, 0, 0, 0)], &[], &[])),
        ("flag MEASURED", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &a, 0, 0, 0, 0, d | Flags::MEASURED.0)], &[], &[])),
        ("flag sconosciuto", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &a, 0, 0, 0, 0, 1 << 15)], &[], &[])),
        ("somme oltre 64 bit", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &a, u64::MAX, 0, 0, 0, 0), (0, &w("b"), 1, 0, 0, 0, 0)], &[], &[])),
        ("allocate oltre 64 bit", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &a, 0, u64::MAX, 0, 0, 0), (0, &w("b"), 0, 1, 0, 0, 0)], &[], &[])),
        ("dati di un nodo che non c'è", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d)], &[(1, 0, 0)], &[])),
        ("dati di un file", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &a, 0, 0, 0, 0, 0)], &[(1, 0, 0)], &[])),
        ("dati doppi", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d)], &[(0, 0, 0), (0, 0, 0)], &[])),
        ("dati fuori ordine", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &a, 0, 0, 0, 0, d)], &[(1, 0, 0), (0, 0, 0)], &[])),
        ("problema di un nodo che non c'è", raw_folder(&[(NONE, &root, 0, 0, 0, 0, d)], &[], &[(1, 5)])),
    ];
    for (what, body) in bad {
        assert_eq!(decode_reply(&body).map(|_| ()), Err(ProtoError::Malformed), "{what}");
    }
    // Un file grande da solo va bene: è la somma che non deve traboccare.
    let big = raw_folder(&[(NONE, &root, 0, 0, 0, 0, d), (0, &a, u64::MAX - 1, u64::MAX - 1, 0, 0, 0), (0, &w("b"), 1, 1, 0, 0, 0)], &[], &[]);
    let Ok(Reply::Folder { tree, .. }) = decode_reply(&big) else { panic!("albero valido") };
    assert_eq!(tree.alloc(Tree::ROOT), u64::MAX);
}

#[test]
fn reply_framing_errors() {
    let replies = [
        Reply::Started { backup_privilege: true },
        Reply::Bin { index: 4, sid: OTHER_SID.into(), totals: Totals::default(), orphan: true },
        Reply::Shadow(Ok(Some(ShadowStorage::default()))),
        Reply::Folder { index: 1, tree: Box::new(sample_tree()), problems: vec![(1, Problem::AccessDenied)] },
    ];
    for r in &replies {
        let body = encode_reply(r);
        for cut in 0..body.len() {
            assert!(decode_reply(&body[..cut]).is_err(), "{r:?} troncata a {cut}");
        }
        let mut extra = body.clone();
        extra.push(0);
        assert_eq!(decode_reply(&extra).map(|_| ()), Err(ProtoError::Malformed), "byte in più");
    }
    assert_eq!(decode_reply(&[0x01, 0, 0]).map(|_| ()), Err(ProtoError::Malformed), "tipo della richiesta");
    assert_eq!(decode_reply(&[0x10, 2]).map(|_| ()), Err(ProtoError::Malformed), "booleano non valido");
    assert_eq!(decode_reply(&[0x15, 3]).map(|_| ()), Err(ProtoError::Malformed), "copie shadow: variante sconosciuta");
    let mut sid = vec![0x13, 0, 0, 0, 0];
    put_str(&mut sid, &[0xD800]);
    sid.extend_from_slice(&[0; 36]);
    sid.push(0);
    assert_eq!(decode_reply(&sid).map(|_| ()), Err(ProtoError::Malformed), "SID non valido");
}

#[test]
fn declared_counts_never_drive_the_allocation() {
    // Numeri enormi dichiarati in un corpo di pochi byte: rifiutati senza riservare memoria.
    let mut nodes = vec![0x11, 0, 0, 0, 0];
    put_u32(&mut nodes, u32::MAX);
    nodes.extend_from_slice(&[0; 64]);
    // Un nodo solo, con il nome della radice lungo quanto il massimo ma assente.
    let mut name = vec![0x11, 0, 0, 0, 0];
    put_u32(&mut name, 1);
    put_u32(&mut name, NONE);
    put_u32(&mut name, 0x7FFF);
    name.extend_from_slice(&[0; 30]);
    let mut problems = raw_folder(&[(NONE, &w("r"), 0, 0, 0, 0, Flags::DIR.0)], &[], &[]);
    let n = problems.len();
    problems[n - 4..].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut extras = raw_folder(&[(NONE, &w("r"), 0, 0, 0, 0, Flags::DIR.0)], &[], &[]);
    let n = extras.len();
    extras[n - 8..n - 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut sid = vec![0x13, 0, 0, 0, 0];
    put_u32(&mut sid, u32::MAX);
    for body in [nodes, name, problems, extras, sid] {
        let (r, peak) = peak(|| decode_reply(&body).map(|_| ()));
        assert_eq!(r, Err(ProtoError::Malformed));
        assert!(peak <= alloc_bound(body.len()), "{peak}");
    }
    // Oltre il limite: rifiutato prima di guardare il contenuto (il buffer di prova è azzerato e non
    // toccato, quindi non occupa davvero 512 MiB).
    let mut huge = vec![0u8; (512 << 20) + 1];
    huge[0] = 0x11;
    let (r, peak) = peak(|| decode_reply(&huge).map(|_| ()));
    assert_eq!(r, Err(ProtoError::TooLarge));
    assert!(peak < 4096, "{peak}");
}

// ---------------------------------------------------------------------------------------------------
// Proprietà

/// Componente di un percorso valido: lettere, cifre, spazi, accentate, caratteri fuori dal piano di base.
fn component() -> impl Strategy<Value = String> {
    proptest::collection::vec(prop_oneof![
        prop::char::range('a', 'z'),
        prop::char::range('A', 'Z'),
        prop::char::range('0', '9'),
        Just(' '),
        Just('.'),
        Just('$'),
        prop::char::range('à', 'ÿ'),
        prop::char::range('😀', '😆'),
    ], 1..12)
    .prop_map(|v| v.into_iter().collect::<String>())
    .prop_filter("né . né ..", |s| s != "." && s != "..")
}

fn components(max: usize) -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(component(), 1..max)
}

/// Unità di un nome di file: comprese quelle che non formano UTF-16 valido.
fn name_units() -> impl Strategy<Value = Vec<u16>> {
    proptest::collection::vec(prop_oneof![
        4 => u16::from(b'a')..=u16::from(b'z'),
        1 => Just(u16::from(b'.')),
        1 => Just(u16::from(b' ')),
        1 => 0x00C0u16..0x0180,
        1 => 0x4E00u16..0x4E40,
        1 => 0xD800u16..0xE000,
    ], 1..10)
    .prop_map(|mut v| {
        if v.iter().all(|&u| u == u16::from(b'.')) && v.len() <= 2 {
            v.push(u16::from(b'x'));
        }
        v
    })
}

#[derive(Clone, Debug)]
struct NodeSpec {
    parent: usize,
    dir: bool,
    name: Vec<u16>,
    logical: u64,
    alloc: u64,
    mtime: i64,
    attrs: u32,
    flags: u16,
}

fn node_spec() -> impl Strategy<Value = NodeSpec> {
    (any::<usize>(), any::<bool>(), name_units(), 0u64..1 << 40, 0u64..1 << 40, any::<i64>(), any::<u32>(), 0u16..(1 << 11))
        .prop_map(|(parent, dir, name, logical, alloc, mtime, attrs, extra)| NodeSpec { parent, dir, name, logical, alloc, mtime, attrs, flags: (extra << 1) & !Flags::DIR.0 })
}

/// Albero casuale costruito con l'API del modello: ogni genitore è una cartella già presente.
fn tree_from(root_flags: u16, specs: &[NodeSpec], extra_mask: u64, extra_vals: &[(u64, u32)]) -> Tree {
    let mut t = Tree::with_root(&w(r"R:\Negata"), specs.len() + 1);
    t.add_flags(Tree::ROOT, Flags(root_flags & ((1 << 12) - 1)));
    let mut dirs = vec![Tree::ROOT];
    for s in specs {
        let parent = dirs[s.parent % dirs.len()];
        let flags = if s.dir { Flags(s.flags | Flags::DIR.0) } else { Flags(s.flags) };
        let i = t.push(parent, &s.name, NodeData { logical: s.logical, alloc: s.alloc, mtime: s.mtime, attrs: s.attrs, flags });
        if s.dir {
            dirs.push(i);
        }
    }
    for (k, &d) in dirs.iter().enumerate() {
        if extra_mask >> (k % 64) & 1 == 1 {
            let (index_alloc, error) = extra_vals[k % extra_vals.len()];
            t.set_dir_extra(DirExtra { node: d, index_alloc, error });
        }
    }
    t.seal();
    t.aggregate();
    t
}

/// Corpi validi di ogni tipo, da mutare.
fn valid_bodies() -> Vec<Vec<u8>> {
    vec![
        encode_request(&req(r"C:\", &[r"C:\Users\altro", r"C:\Windows\x"])),
        encode_reply(&Reply::Started { backup_privilege: true }),
        encode_reply(&Reply::Folder { index: 2, tree: Box::new(sample_tree()), problems: vec![(1, Problem::AccessDenied)] }),
        encode_reply(&Reply::Bin { index: 1, sid: OTHER_SID.into(), totals: Totals::default(), orphan: false }),
        encode_reply(&Reply::Shadow(Ok(Some(ShadowStorage::default())))),
        encode_reply(&Reply::Done { code: 0 }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn valid_requests_round_trip(letter in prop::char::range('A', 'Z'), paths in proptest::collection::vec(components(5), 0..20), lower in any::<bool>()) {
        let vol_letter = if lower { letter.to_ascii_lowercase() } else { letter };
        // Primo componente diverso per ogni cartella: nessuna dentro un'altra.
        let folders: Vec<String> = paths.iter().enumerate().map(|(i, c)| format!(r"{letter}:\d{i}\{}", c.join("\\"))).collect();
        let r = Request { volume: format!(r"{vol_letter}:\"), folders };
        prop_assert_eq!(r.validate(), Ok(()));
        prop_assert_eq!(decode_request(&encode_request(&r)), Ok(r));
    }

    #[test]
    fn relative_paths_are_rejected(comps in components(6), drive_relative in any::<bool>()) {
        let p = if drive_relative { format!("C:{}", comps.join("\\")) } else { comps.join("\\") };
        let r = req(r"C:\", &[r"C:\ok", &p]);
        prop_assert_eq!(r.validate(), Err(ProtoError::BadPath(1, PathIssue::Relative)));
        prop_assert_eq!(decode_request(&encode_request(&r)), Err(ProtoError::BadPath(1, PathIssue::Relative)));
    }

    #[test]
    fn dot_components_are_rejected(mut comps in components(6), at in any::<prop::sample::Index>(), dots in prop_oneof![Just(".."), Just(".")]) {
        let k = at.index(comps.len() + 1);
        comps.insert(k, dots.to_owned());
        let p = format!(r"C:\{}", comps.join("\\"));
        prop_assert_eq!(req(r"C:\", &[&p]).validate(), Err(ProtoError::BadPath(0, PathIssue::NotNormalized)));
    }

    #[test]
    fn other_volumes_are_rejected(letter in prop::char::range('A', 'Z'), comps in components(5)) {
        prop_assume!(letter != 'C');
        let p = format!(r"{letter}:\{}", comps.join("\\"));
        prop_assert_eq!(req(r"C:\", &[&p]).validate(), Err(ProtoError::BadPath(0, PathIssue::OtherVolume)));
        let p = format!(r"{}:\{}", letter.to_ascii_lowercase(), comps.join("\\"));
        prop_assert_eq!(req(r"c:\", &[&p]).validate(), Err(ProtoError::BadPath(0, PathIssue::OtherVolume)));
    }

    #[test]
    fn too_long_paths_are_rejected(extra in 1usize..2000, unit in prop_oneof![Just("a"), Just("é"), Just("😀")]) {
        let n = (MAX_PATH_UNITS - 3) / unit.encode_utf16().count() + extra;
        let p = format!(r"C:\{}", unit.repeat(n));
        let r = req(r"C:\", &[&p]);
        prop_assert_eq!(r.validate(), Err(ProtoError::BadPath(0, PathIssue::TooLong)));
        prop_assert_eq!(decode_request(&encode_request(&r)), Err(ProtoError::BadPath(0, PathIssue::TooLong)));
    }

    #[test]
    fn nested_folders_are_rejected(outer in components(4), inner in components(4), upper in any::<bool>(), inner_first in any::<bool>()) {
        let a = format!(r"C:\{}", outer.join("\\"));
        let b = format!(r"{a}\{}", inner.join("\\"));
        let a = if upper { a.to_ascii_uppercase() } else { a };
        let (first, second) = if inner_first { (b, a) } else { (a, b) };
        let r = req(r"C:\", &[&first, &second]);
        let expected = if inner_first { 0 } else { 1 };
        prop_assert_eq!(r.validate(), Err(ProtoError::BadPath(expected, PathIssue::Duplicate)));
    }

    #[test]
    fn random_trees_round_trip(
        root_flags in 0u16..(1 << 12),
        specs in proptest::collection::vec(node_spec(), 0..80),
        extra_mask in any::<u64>(),
        extra_vals in proptest::collection::vec((any::<u64>(), any::<u32>()), 1..4),
        problems in proptest::collection::vec((any::<u32>(), prop_oneof![Just(5u32), Just(32), Just(2), Just(1223), Just(87), any::<u32>()]), 0..6),
        index in any::<u32>(),
    ) {
        let t = tree_from(root_flags | Flags::DIR.0, &specs, extra_mask, &extra_vals);
        let problems: Vec<(u32, Problem)> = problems.into_iter().map(|(n, c)| (n % t.len() as u32, Problem::from_code(c))).collect();
        let r = Reply::Folder { index, tree: Box::new(t), problems };
        assert_same_reply(&r, &round_trip(&r));
    }

    #[test]
    fn random_bytes_never_panic_or_overallocate(body in proptest::collection::vec(any::<u8>(), 0..512)) {
        let (_, p1) = peak(|| decode_request(&body));
        let (_, p2) = peak(|| decode_reply(&body).map(|_| ()));
        prop_assert!(p1 <= alloc_bound(body.len()) && p2 <= alloc_bound(body.len()), "{p1} {p2}");
    }

    #[test]
    fn typed_random_bodies_never_panic_or_overallocate(
        kind in prop_oneof![Just(0x01u8), (0x10u8..=0x16)],
        rest in proptest::collection::vec(prop_oneof![3 => any::<u8>(), 1 => Just(0u8), 1 => Just(0xFFu8)], 0..512),
    ) {
        let mut body = vec![kind];
        body.extend_from_slice(&rest);
        let (_, p1) = peak(|| decode_request(&body));
        let (_, p2) = peak(|| decode_reply(&body).map(|_| ()));
        prop_assert!(p1 <= alloc_bound(body.len()) && p2 <= alloc_bound(body.len()), "{p1} {p2}");
    }

    #[test]
    fn mutated_bodies_never_panic_or_overallocate(
        which in any::<prop::sample::Index>(),
        edits in proptest::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..8),
        cut in any::<prop::sample::Index>(),
        truncate in any::<bool>(),
    ) {
        let bodies = valid_bodies();
        let mut body = bodies[which.index(bodies.len())].clone();
        for (at, v) in edits {
            let k = at.index(body.len());
            body[k] = v;
        }
        if truncate {
            body.truncate(cut.index(body.len() + 1));
        }
        let (_, p1) = peak(|| decode_request(&body));
        let (_, p2) = peak(|| decode_reply(&body).map(|_| ()));
        prop_assert!(p1 <= alloc_bound(body.len()) && p2 <= alloc_bound(body.len()), "{p1} {p2}");
    }
}
