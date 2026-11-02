// SPDX-License-Identifier: GPL-3.0-or-later
//! Raccolta dell'helper (`helper::collect::Live`) senza elevazione: cartelle e totali
//! contro l'oracolo, privilegio di backup non assegnato, SID orfani, profilo dal registro, copie shadow
//! con WMI. Le cartelle di prova stanno in `%TEMP%` e si cancellano alla fine (ACL ripristinata prima).
//!
//! La prova sul contenuto di `R:` è `#[ignore]` (richiede il VHDX di prova):
//! `cargo test -p quado-scan-win --test helper_collect -- --ignored --test-threads=1`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use quado_model::{Flags, Tree};
use quado_scan_win::helper::collect::Live;
use quado_scan_win::helper::proto::Totals;
use quado_scan_win::helper::serve::Collector;
use quado_scan_win::{Options, Problem, Progress, RootError, scan};
use quado_testkit::helper as content;
use quado_testkit::oracle::{self, Oracle};
use quado_testkit::trees::{self, TestDir};


/// Vero se le prove girano come amministratore, come sui runner Windows di GitHub Actions: lì le cartelle
/// negate si aprono e il privilegio di backup è assegnato, quindi le prove del comportamento senza
/// elevazione non hanno senso e si saltano dicendolo.
fn elevated_runner(test: &str) -> bool {
    let elevated = quado_scan_win::process::elevated().unwrap_or(false);
    if elevated {
        eprintln!("{test}: saltata, il processo delle prove gira come amministratore");
    }
    elevated
}

/// `WBEM_E_ACCESS_DENIED`.
const WBEM_E_ACCESS_DENIED: u32 = 0x8004_1003;
/// `WBEM_E_INITIALIZATION_FAILURE`: il provider delle copie shadow non parte per un utente che non è
/// amministratore (su questo PC, come `Get-CimInstance Win32_ShadowStorage` non elevato).
const WBEM_E_INITIALIZATION_FAILURE: u32 = 0x8004_1014;
/// `ERROR_SERVICE_DISABLED`: servizio Copia shadow del volume disattivato.
const ERROR_SERVICE_DISABLED: u32 = 1058;

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// Totali attesi dall'oracolo: file unici, cartelle esclusa la radice.
fn oracle_totals(o: &Oracle) -> Totals {
    let ids: HashSet<u128> = o.files.values().map(|f| f.id).collect();
    Totals {
        logical: o.unique_logical,
        alloc: o.unique_alloc,
        files: ids.len() as u64,
        dirs: o.dirs - 1,
        unreadable: o.unreadable.len() as u32,
    }
}

/// Albero con file piccoli e medi, sottocartelle e hard link tra cartelle diverse.
fn sample_tree(root: &Path) {
    trees::small_files(&root.join("piccoli"), 700, 5000);
    for k in 0..4 {
        trees::write_file(&root.join("medi").join(format!("m{k}.bin")), 200_000 + k * 77_777, k as u8);
    }
    trees::deep(&root.join("profondo"), 30);
    let original = root.join("medi").join("m1.bin");
    std::fs::create_dir_all(root.join("link\\dentro")).unwrap();
    std::fs::hard_link(&original, root.join("link\\l1.bin")).unwrap();
    std::fs::hard_link(&original, root.join("link\\dentro\\l2.bin")).unwrap();
}

#[test]
fn folder_and_totals_match_the_oracle() {
    let d = TestDir::new("helper-collect");
    sample_tree(&d.path);
    let o = oracle::walk(&d.path);
    assert!(o.unreadable.is_empty());

    let f = Live.folder(s(&d.path)).expect("cartella");
    let t = &f.tree;
    assert!(f.problems.is_empty(), "{:?}", f.problems);
    let mut names: HashMap<Vec<u16>, (u64, u64)> = HashMap::new();
    for i in 0..t.len() as u32 {
        if !t.is_dir(i) {
            names.insert(t.path(i), (t.logical(i), t.alloc(i)));
        }
    }
    assert_eq!(names.len(), o.files.len(), "nomi di file");
    for (p, of) in &o.files {
        assert_eq!(names.get(p), Some(&(of.logical, of.alloc)), "{}", String::from_utf16_lossy(p));
    }
    assert_eq!(t.alloc(Tree::ROOT), o.unique_alloc);
    assert_eq!(t.logical(Tree::ROOT), o.unique_logical);

    let totals = Live.totals(s(&d.path)).expect("totali");
    assert_eq!(totals, oracle_totals(&o));
    assert_eq!(totals.unreadable, 0);
    // Due nomi in più per lo stesso file: i file unici sono due meno dei nomi.
    assert_eq!(totals.files + 2, o.files.len() as u64);
}

#[test]
fn totals_count_hard_links_once_and_the_denied_subfolder() {
    if elevated_runner("totals_count_hard_links_once_and_the_denied_subfolder") {
        return;
    }
    let mut d = TestDir::new("helper-totali");
    let root = d.path.clone();
    let a = root.join("a.bin");
    trees::write_file(&a, 333_333, 1);
    std::fs::create_dir_all(root.join("x\\y")).unwrap();
    std::fs::hard_link(&a, root.join("x\\a-link.bin")).unwrap();
    std::fs::hard_link(&a, root.join("x\\y\\a-link2.bin")).unwrap();
    trees::write_file(&root.join("x\\b.bin"), 4321, 2);
    let denied = root.join("x\\negata");
    trees::write_file(&denied.join("segreto.bin"), 50_000, 3);
    assert!(d.deny_listing(&denied), "icacls /deny");

    let o = oracle::walk(&root);
    assert_eq!(o.unreadable.len(), 1);
    let t = Live.totals(s(&root)).expect("totali");
    assert_eq!(t.unreadable, 1, "{t:?}");
    assert_eq!(t, oracle_totals(&o));
    // a.bin una volta sola (tre nomi), b.bin; il contenuto della cartella negata non si vede.
    assert_eq!(t.files, 2);
    assert_eq!(t.dirs, 3, "x, x\\y, x\\negata");

    // Lo stesso con l'elenco: la cartella negata è un problema dell'albero, con il motivo.
    let f = Live.folder(s(&root)).unwrap();
    let n = (0..f.tree.len() as u32).find(|&i| f.tree.path_lossy(i).ends_with("negata")).unwrap();
    assert!(f.tree.flags(n).has(Flags::DENIED));
    assert_eq!(f.problems, vec![(n, Problem::AccessDenied)]);
}

#[test]
fn root_that_cannot_be_opened_is_a_root_error() {
    if elevated_runner("root_that_cannot_be_opened_is_a_root_error") {
        return;
    }
    let mut d = TestDir::new("helper-radice");
    let denied = d.path.join("chiusa");
    trees::write_file(&denied.join("f.bin"), 10, 1);
    assert!(d.deny_listing(&denied));
    assert_eq!(Live.folder(s(&denied)).err().map(|e| e.0), Some(5));
    assert_eq!(Live.totals(s(&denied)).err(), Some(RootError(5)));
    assert_eq!(Live.totals(s(&d.path.join("non-esiste"))).err(), Some(RootError(2)));
}

/// Senza elevazione il token filtrato non ha `SeBackupPrivilege`: `ERROR_NOT_ALL_ASSIGNED`.
#[test]
fn backup_privilege_is_not_assigned_without_elevation() {
    if elevated_runner("backup_privilege_is_not_assigned_without_elevation") {
        return;
    }
    assert_eq!(Live.enable_backup_privilege(), Err(1300));
}

#[test]
fn orphan_sids() {
    let me = content::current_user_sid().expect("SID dell'utente");
    assert_eq!(Live.sid_is_orphan(&me), Ok(false), "{me}");
    assert_eq!(Live.sid_is_orphan("S-1-5-18"), Ok(false));
    assert_eq!(Live.sid_is_orphan(content::PROFILE_SID), Ok(true));
    assert_eq!(Live.sid_is_orphan(content::ORPHAN_BIN_SID), Ok(true));
    // Stringa che non è un SID: ERROR_INVALID_SID.
    assert_eq!(Live.sid_is_orphan("non-un-sid"), Err(1337));
    assert!(Live.sid_is_orphan("").is_err());
}

#[test]
fn profile_of_reads_profile_list() {
    let me = content::current_user_sid().unwrap();
    let mine = Live.profile_of(&me).expect("ProfileImagePath dell'utente");
    let expected = std::env::var("USERPROFILE").unwrap();
    assert!(mine.eq_ignore_ascii_case(&expected), "{mine} contro {expected}");
    // SYSTEM: REG_EXPAND_SZ con %systemroot%, espanso.
    let system = Live.profile_of("S-1-5-18").expect("profilo di SYSTEM");
    assert!(!system.contains('%') && system.to_ascii_lowercase().ends_with(r"\system32\config\systemprofile"), "{system}");
    assert_eq!(Live.profile_of(content::PROFILE_SID), None);
    assert_eq!(Live.profile_of(r"S-1-5-18\..\.."), None);
}

/// Copie shadow senza elevazione: nessun panico; l'esito si documenta (`collect.rs`).
#[test]
fn shadow_storage_without_elevation() {
    if elevated_runner("shadow_storage_without_elevation") {
        return;
    }
    let mut volumes = vec![r"C:\".to_owned()];
    if let Some((_, r)) = trees::mounted_vhdx().into_iter().find(|(n, _)| n == "ntfs") {
        volumes.push(r.to_string_lossy().into_owned());
    }
    for v in volumes {
        let r = Live.shadow_storage(&v);
        eprintln!("copie shadow di {v}: {r:?}");
        assert!(matches!(r, Ok(_) | Err(WBEM_E_ACCESS_DENIED | WBEM_E_INITIALIZATION_FAILURE)) || r == Err(ERROR_SERVICE_DISABLED), "{v}: {r:x?}");
        // Con il servizio VSS disattivato (questo PC) il motivo è quello, non il codice di WMI.
        if quado_scan_win::helper::collect::vss_disabled() {
            assert_eq!(r, Err(ERROR_SERVICE_DISABLED), "{v}");
        }
        // Due volte di seguito: la seconda trova COM e la sicurezza già impostate.
        assert_eq!(Live.shadow_storage(&v), r);
    }
    // Un volume che non c'è: errore di GetVolumeNameForVolumeMountPointW, non WMI.
    assert!(matches!(Live.shadow_storage(r"Q:\"), Err(c) if c < 0x8000_0000), "{:?}", Live.shadow_storage(r"Q:\"));
}

/// Pulizia del contenuto di `R:` anche se la prova fallisce.
struct Clean;

impl Drop for Clean {
    fn drop(&mut self) {
        if let Err(e) = content::cleanup() {
            eprintln!("pulizia di R: incompleta: {e}");
        }
    }
}

/// Nomi nella radice del volume e in `$Recycle.Bin` (le cartelle per SID si vedono, anche se non si
/// elencano).
fn root_entries(vol: &Path) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for e in std::fs::read_dir(vol).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.eq_ignore_ascii_case("$Recycle.Bin") {
            v.extend(std::fs::read_dir(e.path()).unwrap().flatten().map(|b| format!(r"{name}\{}", b.file_name().to_string_lossy())));
        }
        v.push(name);
    }
    v.sort();
    v
}

/// Contenuto di prova su `R:`: la scansione normale trova le quattro cartelle negate, l'helper
/// senza privilegio non le apre (errore 5, come la scansione), i valori attesi sono coerenti; alla fine
/// `R:` torna com'era.
#[test]
#[ignore = "richiede il VHDX di prova R:"]
fn contenuto_di_prova_su_r() {
    let vol = content::volume();
    let before = root_entries(&vol);
    let clean = Clean;
    let e = content::prepare();
    eprintln!("{}", e.to_markdown());

    let r = scan(s(&vol), Options::default(), &Progress::default()).unwrap();
    let denied: Vec<String> = r.problems.iter().filter(|(_, p)| *p == Problem::AccessDenied).map(|(n, _)| r.tree.path_lossy(*n)).collect();
    eprintln!("cartelle negate su {}: {denied:?}", vol.display());
    for f in e.folders() {
        assert!(denied.iter().any(|d| d.eq_ignore_ascii_case(&f.path)), "{} non negata: {denied:?}", f.path);
        assert_eq!(Live.folder(&f.path).err().map(|e| e.0), Some(5), "{}", f.path);
        assert_eq!(Live.totals(&f.path).err(), Some(RootError(5)), "{}", f.path);
        assert!(f.totals.files > 0 && f.totals.alloc > 0);
        assert!(f.files.len() as u64 >= f.totals.files);
    }
    // Hard link: nella cartella negata e nel profilo un nome in più del numero di file.
    assert_eq!(e.denied.files.len() as u64, e.denied.totals.files + 1);
    assert_eq!(e.profile.files.len() as u64, e.profile.totals.files + 1);
    assert_eq!(e.denied.totals.dirs, 2, "sotto, sotto\\profonda");
    assert!(e.bins.iter().any(|b| b.orphan) && e.bins.iter().any(|b| !b.orphan));

    // Idempotente: una seconda preparazione rifà lo stesso contenuto con gli stessi valori.
    let again = content::prepare();
    assert_eq!(again.denied.totals, e.denied.totals);
    assert_eq!(again.profile.files, e.profile.files);

    drop(clean);
    content::cleanup().expect("seconda pulizia");
    assert_eq!(root_entries(&vol), before, "R: com'era");
}
