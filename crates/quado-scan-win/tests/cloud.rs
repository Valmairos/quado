// SPDX-License-Identifier: GPL-3.0-or-later
//! Segnaposto cloud riconosciuti senza idratarli né scaricare elenchi.

use quado_model::Flags;
use quado_scan_win::{Options, Progress, scan};
use quado_testkit::cloud::{Placeholder, TestSyncRoot};

#[test]
fn placeholders_are_measured_without_requests() {
    let root = match TestSyncRoot::new("scan") {
        Ok(r) => r,
        Err(e) => panic!("provider di prova non registrato: {e:?}"),
    };
    root.create(
        "",
        &[
            Placeholder { name: "nel-cloud.bin", size: 5 << 20, dir: false, populated: false },
            Placeholder { name: "cartella-non-popolata", size: 0, dir: true, populated: false },
            Placeholder { name: "cartella-popolata", size: 0, dir: true, populated: true },
        ],
    )
    .unwrap();
    root.create("cartella-popolata", &[Placeholder { name: "dentro.bin", size: 3 << 20, dir: false, populated: false }]).unwrap();
    std::fs::write(root.path.join("locale.txt"), vec![b'x'; 10_000]).unwrap();
    let before = TestSyncRoot::requests();

    let r = scan(root.path.to_str().unwrap(), Options::default(), &Progress::default()).unwrap();
    let after = TestSyncRoot::requests();
    let t = &r.tree;
    let find = |name: &str| (0..t.len() as u32).find(|&i| t.name_lossy(i) == name).unwrap_or_else(|| panic!("manca {name}"));
    for name in ["nel-cloud.bin", "cartella-non-popolata", "cartella-popolata", "locale.txt"] {
        let n = find(name);
        eprintln!("{name}: attributi {:#x}, flag {:?}, logica {}, allocata {}", t.attrs(n), t.flags(n), t.logical(n), t.alloc(n));
    }
    assert_eq!(after, before, "il provider ha ricevuto richieste di dati o di elenco durante la scansione");

    // "Nel cloud" = logica dichiarata dal provider; "su questo disco" = allocata.
    let f = find("nel-cloud.bin");
    assert!(t.flags(f).has(Flags::CLOUD));
    assert_eq!(t.logical(f), 5 << 20);
    assert_eq!(t.alloc(f), 0);
    // La cartella non popolata non si elenca; quella popolata sì, senza scaricare i dati.
    let np = find("cartella-non-popolata");
    assert!(t.flags(np).has(Flags::CLOUD_NOT_LISTED), "{:?}", t.flags(np));
    let inside = find("dentro.bin");
    assert_eq!(t.logical(inside), 3 << 20);
    assert_eq!(t.alloc(inside), 0);
    assert_eq!(r.stats.cloud_dirs_not_listed, 1);
}
