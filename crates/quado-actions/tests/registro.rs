// SPDX-License-Identifier: GPL-3.0-or-later
//! Registro degli interventi: file creato al primo intervento, andata e
//! ritorno esatti di ogni campo, ultima riga per `id`, righe illeggibili contate, cancellazione, solo
//! codici e valori. Tutto in una cartella temporanea propria, cancellata alla fine.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use quado_actions::journal::{self, FILE_NAME, Journal, JournalError, VERSION};
use quado_model::intervention::{
    BinCount, Comparison, Explanation, InterventionKind, ItemOutcome, JournalEntry, JournalItem, Measure, MeasurePoint, SkipReason,
};
use quado_model::record::{Estimate, NotEstimable};

/// Cartella temporanea unica (processo, contatore, ora), cancellata al drop. I test non scrivono altrove.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static N: AtomicU32 = AtomicU32::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let base = std::env::temp_dir().join(format!("quado-registro-{}-{}-{nanos}-{name}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        assert!(!base.exists());
        // La cartella dei dati non esiste ancora: `append` la crea (`%APPDATA%\Quado` al primo intervento).
        TempDir(base)
    }

    fn data_dir(&self) -> PathBuf {
        self.0.join("Quado")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const VOLUME: &str = r"C:\";
const PATHS: [&str; 8] = [
    r"C:\Users\demo\AppData\Local\CapCut\Apps\4.0.0.1",
    r"C:\Users\demo\AppData\Local\CapCut\Apps\4.1.0.2",
    r"C:\Users\demo\Downloads\setup vecchio (1).exe",
    r"C:\Users\demo\Downloads\è un file con l'accento.zip",
    r"C:\Users\demo\Downloads\illeggibile.bin",
    r"C:\Users\demo\Downloads\in uso.mp4",
    r"C:\Users\demo\Downloads\sparito.iso",
    r"C:\Users\demo\Downloads\non raggiunto.msi",
];

/// Uno spostamento con tutti i campi valorizzati: ogni esito, ogni punto di misura, ogni voce.
fn full_entry(id: i64) -> JournalEntry {
    let outcomes = [
        ItemOutcome::Recycled,
        ItemOutcome::Skipped(SkipReason::Changed),
        ItemOutcome::Skipped(SkipReason::Missing),
        ItemOutcome::Skipped(SkipReason::InUse),
        ItemOutcome::Skipped(SkipReason::Unreadable(1920)),
        ItemOutcome::Failed(0x8007_0020_u32 as i32),
        ItemOutcome::NotInRecycleBin(0x8027_0000_u32 as i32),
        ItemOutcome::NotReached,
    ];
    let items = PATHS.iter().zip(outcomes).enumerate().map(|(k, (p, o))| JournalItem { path: (*p).to_owned(), bytes: 1_000_003 * k as u64 + 7, outcome: o }).collect();
    JournalEntry {
        id,
        volume: VOLUME.to_owned(),
        kind: InterventionKind::Recycle,
        origin: "capcut-versioni".to_owned(),
        method: "sposta-nel-cestino".to_owned(),
        estimate: Estimate::Exact(4_294_967_311),
        items,
        measures: vec![
            Measure { point: MeasurePoint::Start, at: id, free: 123_456_789_012, bin: Some(BinCount { items: 12, bytes: 3_000_000_001 }), still_present: None },
            Measure { point: MeasurePoint::AfterAction, at: id + 10, free: 123_456_700_000, bin: Some(BinCount { items: 13, bytes: 7_000_000_000 }), still_present: Some(0) },
            Measure { point: MeasurePoint::AfterEmpty, at: id + 20, free: 130_000_000_000, bin: None, still_present: Some(4096) },
            Measure { point: MeasurePoint::AfterProcedure, at: id + 30, free: u64::MAX, bin: Some(BinCount { items: 0, bytes: 0 }), still_present: Some(u64::MAX) },
        ],
        comparison: Some(Comparison {
            estimated: Estimate::Exact(4_294_967_311),
            obtained: -9_876_543_210,
            difference: Some(i64::MIN),
            basis_points: Some(u64::MAX),
            explanations: vec![
                Explanation::SkippedOrFailed { items: 5, bytes: 11 },
                Explanation::ExcludedNames { files: 3, bytes: 12 },
                Explanation::StillInBin { bytes: 13 },
                Explanation::OtherBinItems { items: 4, bytes: 14 },
                Explanation::StillOpen { files: 2 },
                Explanation::OtherActivity { bytes: -15 },
                Explanation::OtherActivity { bytes: i64::MAX },
            ],
            unexplained: Some(-1),
        }),
    }
}

/// Una procedura con stima a intervallo e confronto senza differenza.
fn procedure_entry(id: i64) -> JournalEntry {
    JournalEntry {
        id,
        volume: r"R:\".to_owned(),
        kind: InterventionKind::Procedure("file-temporanei".to_owned()),
        origin: "file-temporanei".to_owned(),
        method: "ms-settings:storagesense".to_owned(),
        estimate: Estimate::Range { min: 1, max: u64::MAX },
        items: Vec::new(),
        measures: vec![Measure { point: MeasurePoint::Start, at: id, free: 0, bin: None, still_present: None }],
        comparison: Some(Comparison {
            estimated: Estimate::Range { min: 1, max: u64::MAX },
            obtained: 0,
            difference: None,
            basis_points: None,
            explanations: Vec::new(),
            unexplained: None,
        }),
    }
}

/// Un comando con stima non stimabile e nessun confronto.
fn command_entry(id: i64, why: NotEstimable) -> JournalEntry {
    JournalEntry {
        id,
        volume: VOLUME.to_owned(),
        kind: InterventionKind::Command("npm-cache".to_owned()),
        origin: "npm-cache".to_owned(),
        method: "npm-cache-verify".to_owned(),
        estimate: Estimate::NotEstimable(why),
        items: Vec::new(),
        measures: Vec::new(),
        comparison: None,
    }
}

fn file(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

fn raw_append(dir: &Path, bytes: &[u8]) {
    let mut f = std::fs::OpenOptions::new().append(true).open(file(dir)).expect("registro esistente");
    f.write_all(bytes).expect("scrittura di prova");
}

#[test]
fn the_file_exists_only_after_the_first_intervention() {
    let t = TempDir::new("primo");
    let dir = t.data_dir();
    assert_eq!(journal::path(&dir), file(&dir));
    assert!(!file(&dir).exists());
    assert_eq!(journal::size(&dir), None);
    // Leggere non crea niente.
    assert_eq!(journal::read(&dir), Ok(Journal::default()));
    assert!(!dir.exists());
    journal::append(&dir, &full_entry(133_000_000_000_000_000)).expect("primo intervento");
    assert!(file(&dir).is_file());
    assert!(journal::size(&dir).is_some_and(|n| n > 0));
}

#[test]
fn every_field_round_trips() {
    let t = TempDir::new("campi");
    let dir = t.data_dir();
    let mut written = vec![full_entry(30), procedure_entry(10)];
    for (k, why) in [NotEstimable::HardLinksNotCounted, NotEstimable::ExternalProcedure, NotEstimable::NoAction, NotEstimable::AtSimulation].into_iter().enumerate() {
        written.push(command_entry(40 + k as i64, why));
    }
    written.push(full_entry(-5));
    for e in &written {
        journal::append(&dir, e).expect("aggiunta");
    }
    let j = journal::read(&dir).expect("lettura");
    assert_eq!(j.unreadable_lines, 0);
    written.sort_by_key(|e| e.id);
    assert_eq!(j.entries, written, "ordine per id e valori identici");
    // Stimato e ottenuto ci sono.
    let full = j.entries.iter().find(|e| e.id == 30).expect("spostamento");
    assert_eq!(full.estimate, Estimate::Exact(4_294_967_311));
    assert_eq!(full.comparison.as_ref().map(|c| c.obtained), Some(-9_876_543_210));
}

#[test]
fn for_the_same_id_the_last_line_wins() {
    let t = TempDir::new("ultima");
    let dir = t.data_dir();
    let mut first = full_entry(7);
    first.measures.truncate(1);
    first.comparison = None;
    journal::append(&dir, &first).expect("partenza");
    journal::append(&dir, &procedure_entry(3)).expect("altro intervento");
    let last = full_entry(7);
    journal::append(&dir, &last).expect("misure successive");
    let j = journal::read(&dir).expect("lettura");
    assert_eq!(j.entries, vec![procedure_entry(3), last]);
    assert_eq!(j.unreadable_lines, 0);
    let text = std::fs::read_to_string(file(&dir)).expect("testo");
    assert_eq!(text.lines().count(), 3, "il registro è solo in aggiunta");
}

#[test]
fn truncated_and_foreign_lines_are_counted_and_skipped() {
    let t = TempDir::new("troncata");
    let dir = t.data_dir();
    journal::append(&dir, &full_entry(1)).expect("prima riga");
    // Una riga scritta a metà da un'interruzione, senza a capo finale.
    let mut line = serde_json::to_string(&serde_json::json!({"x": 1})).expect("json");
    line.truncate(3);
    let whole = std::fs::read(file(&dir)).expect("registro");
    let cut = &whole[..whole.len() / 2];
    raw_append(&dir, cut);
    let j = journal::read(&dir).expect("lettura");
    assert_eq!((j.entries.len(), j.unreadable_lines), (1, 1));
    // La riga successiva non si attacca a quella troncata.
    journal::append(&dir, &procedure_entry(2)).expect("dopo l'interruzione");
    let j = journal::read(&dir).expect("lettura");
    assert_eq!(j.entries, vec![full_entry(1), procedure_entry(2)]);
    assert_eq!(j.unreadable_lines, 1);
    // Righe vuote: non contano. Versione diversa, codici sconosciuti, testo qualsiasi: illeggibili.
    let v2 = serde_json::to_string(&serde_json::json!({"versione": VERSION + 1})).expect("json");
    let good = std::fs::read_to_string(file(&dir)).expect("testo").lines().next().expect("riga").to_owned();
    let unknown_point = good.replacen("\"partenza\"", "\"altrove\"", 1);
    let unknown_kind = good.replacen("\"spostamento\"", "\"altro\"", 1);
    assert_ne!(unknown_point, good);
    assert_ne!(unknown_kind, good);
    raw_append(&dir, format!("\n\r\n   \n{v2}\n{unknown_point}\n{unknown_kind}\n{line}\nnon è json\n").as_bytes());
    let j = journal::read(&dir).expect("lettura");
    assert_eq!(j.entries.len(), 2);
    assert_eq!(j.unreadable_lines, 1 + 5);
}

#[test]
fn delete_removes_the_file_and_is_idempotent() {
    let t = TempDir::new("cancella");
    let dir = t.data_dir();
    assert_eq!(journal::delete(&dir), Ok(()), "senza file non fa niente");
    journal::append(&dir, &full_entry(1)).expect("intervento");
    assert!(file(&dir).exists());
    assert_eq!(journal::delete(&dir), Ok(()));
    assert!(!file(&dir).exists());
    assert!(dir.exists(), "la cartella dei dati resta");
    assert_eq!(journal::size(&dir), None);
    assert_eq!(journal::read(&dir), Ok(Journal::default()));
    assert_eq!(journal::delete(&dir), Ok(()));
}

#[test]
fn errors_are_windows_codes() {
    let t = TempDir::new("errori");
    // La cartella dei dati è un file: Windows rifiuta di crearci dentro il registro.
    std::fs::create_dir_all(&t.0).expect("cartella di prova");
    let not_a_dir = t.0.join("file");
    std::fs::write(&not_a_dir, b"x").expect("file di prova");
    let err = journal::append(&not_a_dir, &full_entry(1)).expect_err("cartella impossibile");
    assert!(matches!(err, JournalError::Io(c) if c != 0), "{err:?}");
    // Il registro è una cartella: non si legge e non si cancella come file.
    let dir = t.0.join("dati");
    std::fs::create_dir_all(file(&dir)).expect("cartella al posto del registro");
    assert!(matches!(journal::read(&dir), Err(JournalError::Io(c)) if c != 0));
    assert!(matches!(journal::delete(&dir), Err(JournalError::Io(c)) if c != 0));
}

/// Una stringa che è un codice: minuscole ASCII, cifre e trattini.
fn is_code(s: &str) -> bool {
    !s.is_empty() && s.split('-').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
}

fn walk(v: &serde_json::Value, data: &[&str], problems: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, x) in map {
                if !k.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
                    problems.push(format!("chiave {k:?}"));
                }
                walk(x, data, problems);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, data, problems)),
        serde_json::Value::String(s) if !is_code(s) && !data.contains(&s.as_str()) => problems.push(format!("testo {s:?}")),
        _ => {}
    }
}

#[test]
fn the_file_holds_codes_and_values_not_sentences() {
    let t = TempDir::new("codici");
    let dir = t.data_dir();
    let entries = [full_entry(1), procedure_entry(2), command_entry(3, NotEstimable::ExternalProcedure)];
    for e in &entries {
        journal::append(&dir, e).expect("aggiunta");
    }
    // I soli testi ammessi sono i dati degli interventi: volume, percorsi, origine e metodo.
    let mut data: Vec<&str> = PATHS.to_vec();
    data.extend([VOLUME, r"R:\", "ms-settings:storagesense"]);
    let text = std::fs::read_to_string(file(&dir)).expect("testo");
    let mut problems = Vec::new();
    for line in text.lines() {
        let v: serde_json::Value = serde_json::from_str(line).expect("una riga, un oggetto JSON");
        assert_eq!(v["versione"], serde_json::json!(VERSION));
        walk(&v, &data, &mut problems);
    }
    assert!(problems.is_empty(), "{problems:?}");
    // Gli stati e gli esiti sono i codici del modello.
    let first: serde_json::Value = serde_json::from_str(text.lines().next().expect("riga")).expect("json");
    assert_eq!(first["stima"]["stato"], Estimate::Exact(0).code());
    assert_eq!(first["misure"][0]["punto"], MeasurePoint::Start.code());
    assert_eq!(first["misure"][2]["punto"], MeasurePoint::AfterEmpty.code());
    assert_eq!(first["elementi"][0]["esito"], ItemOutcome::Recycled.code());
    assert_eq!(first["elementi"][1]["esito"], ItemOutcome::Skipped(SkipReason::Changed).code());
    assert_eq!(first["elementi"][5]["esito"], ItemOutcome::Failed(0).code());
    assert_eq!(first["elementi"][5]["hresult"], serde_json::json!(0x8007_0020_u32 as i32));
    let second: serde_json::Value = serde_json::from_str(text.lines().nth(1).expect("riga")).expect("json");
    assert_eq!(second["stima"]["stato"], (Estimate::Range { min: 0, max: 0 }).code());
}
