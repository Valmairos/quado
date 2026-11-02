// SPDX-License-Identifier: GPL-3.0-or-later
//! Durante la scansione nessun handle di file ha accesso in scrittura.
//! Un thread campiona di continuo gli handle del processo e il loro accesso concesso.

use std::os::windows::io::AsRawHandle;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};

use quado_scan_win::process::own_handles;
use quado_scan_win::win::WRITE_BITS;
use quado_scan_win::{Options, Progress, scan};
use quado_testkit::trees::{self, TestDir};

#[test]
fn scanning_opens_no_handle_with_write_access() {
    let d = TestDir::new("handle");
    trees::small_files(&d.path, 20_000, 3000);
    let big = d.path.join("sparso.bin");
    trees::sparse_file(&big, 1 << 30);
    // Tipo "File" degli oggetti: quello di un handle di file aperto qui, per riconoscerli nel campione.
    let probe = std::fs::File::open(&big).unwrap();
    let probe_value = probe.as_raw_handle() as usize;
    let file_type = own_handles().iter().find(|h| h.value == probe_value).expect("handle di prova").object_type;
    drop(probe);
    // Handle di file già aperti prima della scansione (per esempio dal runtime dei test): esclusi.
    let before: Vec<usize> = own_handles().iter().filter(|h| h.object_type == file_type).map(|h| h.value).collect();

    let done = AtomicBool::new(false);
    let (samples, offenders) = std::thread::scope(|s| {
        let sampler = s.spawn(|| {
            let mut samples = 0u64;
            let mut seen_files = 0u64;
            let mut offenders = Vec::new();
            while !done.load(SeqCst) {
                for h in own_handles() {
                    if h.object_type == file_type && !before.contains(&h.value) {
                        seen_files += 1;
                        if h.granted_access & WRITE_BITS != 0 {
                            offenders.push(h);
                        }
                    }
                }
                samples += 1;
            }
            (samples, seen_files, offenders)
        });
        for root in [d.path.to_str().unwrap(), r"C:\Windows\WinSxS"] {
            let r = scan(root, Options::default(), &Progress::default()).unwrap();
            assert!(r.tree.len() > 1000);
        }
        done.store(true, SeqCst);
        let (samples, seen, offenders) = sampler.join().unwrap();
        eprintln!("campioni {samples}, handle di file osservati {seen}");
        assert!(seen > 100, "il campionamento non ha visto abbastanza handle: {seen}");
        (samples, offenders)
    });
    assert!(samples > 50);
    assert!(offenders.is_empty(), "handle con accesso in scrittura: {offenders:?}");
}

#[test]
fn the_sampler_sees_a_write_handle() {
    let d = TestDir::new("controllo-handle");
    let p = d.path.join("scrivibile.txt");
    std::fs::write(&p, b"x").unwrap();
    let f = std::fs::OpenOptions::new().write(true).open(&p).unwrap();
    let value = f.as_raw_handle() as usize;
    let h = own_handles().into_iter().find(|h| h.value == value).expect("handle visibile");
    assert_ne!(h.granted_access & WRITE_BITS, 0, "accesso {:#x}", h.granted_access);
}
