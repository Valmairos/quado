// SPDX-License-Identifier: GPL-3.0-or-later
//! Misure del volume per la verifica: spazio libero, Cestino secondo Windows, ora.
//! La riscansione dei sottoalberi toccati la fa l'interfaccia, che aggiorna il modello e completa
//! `Measure::still_present`.

use quado_model::intervention::{Measure, MeasurePoint};

use crate::probe::Probe;

/// Misura il volume che contiene `path`: spazio libero (`Probe::free_space`), contenuto del Cestino
/// secondo Windows (`Probe::volume_bin`) e ora (`Probe::now`), presa dopo le due letture: entrambe
/// precedono `at`, e la misura di partenza si prende prima della prima chiamata a Windows.
///
/// Errore (codice di Windows) solo se lo spazio libero non si legge. Un Cestino non letto, o un volume
/// senza Cestino, dà `bin: None`: la misura vale lo stesso e la voce del Cestino non compare.
/// `still_present` resta `None`: lo completa la riscansione.
pub fn measure(probe: &dyn Probe, path: &str, point: MeasurePoint) -> Result<Measure, u32> {
    let free = probe.free_space(path)?;
    let bin = probe.volume_bin(path).ok().and_then(|v| v.content);
    let at = probe.now();
    Ok(Measure { point, at, free, bin, still_present: None })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use quado_model::intervention::{BinCount, ReferenceReport};

    use super::*;
    use crate::probe::{Attrs, VolumeBin};
    use crate::restart::{ProcessUse, RmError};

    /// Sonda di prova: risponde solo alle tre letture della misura e registra l'ordine delle chiamate.
    struct Probe3 {
        free: Result<u64, u32>,
        bin: Result<VolumeBin, u32>,
        calls: Mutex<Vec<&'static str>>,
    }

    impl Probe3 {
        fn new(free: Result<u64, u32>, content: Option<BinCount>) -> Self {
            let bin = Ok(VolumeBin { root: r"R:\".into(), has_bin: true, nuke_on_delete: false, capacity: None, content });
            Probe3 { free, bin, calls: Mutex::new(Vec::new()) }
        }

        fn log(&self, c: &'static str) {
            self.calls.lock().expect("registro delle chiamate").push(c);
        }
    }

    impl Probe for Probe3 {
        fn attributes(&self, _: &str) -> Result<Attrs, u32> {
            unreachable!("la misura non rilegge gli attributi")
        }
        fn children(&self, _: &str) -> Result<Vec<String>, u32> {
            unreachable!("la misura non elenca cartelle")
        }
        fn processes_using(&self, _: &[String]) -> Result<Vec<ProcessUse>, RmError> {
            unreachable!("la misura non usa Restart Manager")
        }
        fn references(&self, _: &[String]) -> ReferenceReport {
            unreachable!("la misura non cerca riferimenti")
        }
        fn volume_bin(&self, path: &str) -> Result<VolumeBin, u32> {
            assert_eq!(path, r"R:\prove\a");
            self.log("cestino");
            self.bin.clone()
        }
        fn storage_sense_days(&self) -> Result<Option<u32>, u32> {
            unreachable!("la misura non legge Sensore memoria")
        }
        fn sync_root(&self, _: &str) -> Option<String> {
            unreachable!("la misura non cerca cartelle di sincronizzazione")
        }
        fn free_space(&self, path: &str) -> Result<u64, u32> {
            assert_eq!(path, r"R:\prove\a");
            self.log("libero");
            self.free
        }
        fn now(&self) -> i64 {
            self.log("ora");
            133_000_000_000_000_000
        }
    }

    #[test]
    fn reads_free_space_bin_and_time() {
        let p = Probe3::new(Ok(5_000), Some(BinCount { items: 3, bytes: 700 }));
        let m = measure(&p, r"R:\prove\a", MeasurePoint::Start).expect("misura");
        assert_eq!(m, Measure { point: MeasurePoint::Start, at: 133_000_000_000_000_000, free: 5_000, bin: Some(BinCount { items: 3, bytes: 700 }), still_present: None });
        // L'ora si prende dopo le letture.
        assert_eq!(*p.calls.lock().expect("chiamate"), vec!["libero", "cestino", "ora"]);
        for point in [MeasurePoint::AfterAction, MeasurePoint::AfterEmpty, MeasurePoint::AfterProcedure] {
            assert_eq!(measure(&p, r"R:\prove\a", point).expect("misura").point, point);
        }
    }

    #[test]
    fn unread_bin_is_not_an_error() {
        let mut p = Probe3::new(Ok(5_000), None);
        assert_eq!(measure(&p, r"R:\prove\a", MeasurePoint::AfterAction).expect("misura").bin, None);
        p.bin = Err(2);
        let m = measure(&p, r"R:\prove\a", MeasurePoint::AfterAction).expect("misura senza Cestino");
        assert_eq!((m.free, m.bin, m.still_present), (5_000, None, None));
    }

    #[test]
    fn unread_free_space_is_the_only_error() {
        let p = Probe3::new(Err(21), Some(BinCount { items: 1, bytes: 1 }));
        assert_eq!(measure(&p, r"R:\prove\a", MeasurePoint::Start), Err(21));
    }
}
