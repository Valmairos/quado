// SPDX-License-Identifier: GPL-3.0-or-later
//! Durante identità, hash e indice degli zip nessun handle di file ha accesso in scrittura.
//! Un thread campiona di continuo gli handle del processo, come `no_write_handles.rs`. Un solo test in
//! questo eseguibile: gli handle aperti da altri test in parallelo falserebbero il campione.

mod common;

use std::os::windows::io::AsRawHandle;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};
use std::time::{Duration, Instant};

use quado_scan_win::content::{ACCESS, identity, sha256};
use quado_scan_win::process::own_handles;
use quado_scan_win::win::WRITE_BITS;
use quado_scan_win::zip::zip_index;
use quado_testkit::trees::{self, TestDir};

#[test]
fn reading_content_opens_no_handle_with_write_access() {
    for a in ACCESS {
        assert_eq!(a & WRITE_BITS, 0, "{a:#x}");
    }
    let d = TestDir::new("contenuto-handle");
    let big = d.join("grande.bin");
    trees::write_file(&big, 32 << 20, 9);
    let zip = d.join("archivio.zip");
    std::fs::write(&zip, common::build_zip(&common::sample_entries(), &Default::default()).bytes).unwrap();
    // Tipo "File" degli oggetti: quello di un handle di file aperto qui, per riconoscerli nel campione.
    let probe = std::fs::File::open(&zip).unwrap();
    let probe_value = probe.as_raw_handle() as usize;
    let file_type = own_handles().iter().find(|h| h.value == probe_value).expect("handle di prova").object_type;
    drop(probe);
    let before: Vec<usize> = own_handles().iter().filter(|h| h.object_type == file_type).map(|h| h.value).collect();

    let done = AtomicBool::new(false);
    let (seen, offenders, granted) = std::thread::scope(|s| {
        let sampler = s.spawn(|| {
            let (mut seen, mut offenders, mut granted) = (0u64, Vec::new(), Vec::new());
            while !done.load(SeqCst) {
                for h in own_handles() {
                    if h.object_type == file_type && !before.contains(&h.value) {
                        seen += 1;
                        if !granted.contains(&h.granted_access) {
                            granted.push(h.granted_access);
                        }
                        if h.granted_access & WRITE_BITS != 0 {
                            offenders.push(h);
                        }
                    }
                }
            }
            (seen, offenders, granted)
        });
        let (zip_path, big_path) = (zip.to_str().unwrap(), big.to_str().unwrap());
        let deadline = Instant::now() + Duration::from_millis(1500);
        while Instant::now() < deadline {
            sha256(big_path, &AtomicBool::new(false), &AtomicU64::new(0)).unwrap();
            for _ in 0..20 {
                zip_index(zip_path).unwrap();
                identity(zip_path).unwrap();
            }
        }
        done.store(true, SeqCst);
        sampler.join().unwrap()
    });
    let granted: Vec<String> = granted.iter().map(|g| format!("{g:#x}")).collect();
    eprintln!("handle di file osservati: {seen}; accessi concessi: {granted:?}");
    assert!(seen > 10, "il campionamento non ha visto abbastanza handle: {seen}");
    assert!(offenders.is_empty(), "handle con accesso in scrittura: {offenders:?}");
}
