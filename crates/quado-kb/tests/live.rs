// SPDX-License-Identifier: GPL-3.0-or-later
//! Controllo manuale con data: gli estrattori sul Windows vero di questo PC, in sola lettura.
//! `cargo test -p quado-kb --test live -- --ignored --nocapture`

#[test]
#[ignore = "legge il registro e i file di questo PC: controllo manuale"]
fn gather_on_this_pc() {
    let ev = quado_kb::win::gather("C:\\", None);
    println!("cartelle note: {:?}", ev.folders);
    match &ev.uninstall {
        Ok(u) => println!("disinstallazione: {} voci, {} saltate", u.entries.len(), u.skipped.len()),
        Err(e) => println!("disinstallazione: {e:?}"),
    }
    match &ev.msix {
        Ok(m) => println!("msix: {} pacchetti, {} non framework", m.len(), m.iter().filter(|p| !p.framework).count()),
        Err(e) => println!("msix: {e:?}"),
    }
    match &ev.steam {
        Ok(s) => println!("steam: radice {}, librerie {:?}, {} giochi, {} LastPlayed locali, {} saltati", s.root, s.libraries, s.apps.len(), s.local_last_played.len(), s.skipped.len()),
        Err(e) => println!("steam: {e:?}"),
    }
    match &ev.epic {
        Ok(e) => println!("epic: {:?}", e.items.iter().map(|i| (&i.display_name, &i.install_location)).collect::<Vec<_>>()),
        Err(e) => println!("epic: {e:?}"),
    }
    println!("capcut: {:?}", ev.capcut);
    println!("track: {:?}; userassist: {:?}", ev.track_progs, ev.userassist.as_ref().map(|_| ()));
    println!("alimentazione: {:?}", ev.power);
    println!("sensore memoria: {:?}", ev.storage_sense);
    println!("cestino: {:?} {:?}", ev.bin, ev.recycle);
    println!("npm: {:?}; pnpm: {:?}", ev.npm_cache, ev.pnpm_store);
    println!("ripristino: {:?}; volume di sistema: {}", ev.restore, ev.system_volume);
}
