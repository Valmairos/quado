// SPDX-License-Identifier: GPL-3.0-or-later
//! Registro degli interventi: `%APPDATA%\Quado\interventi.jsonl`. Non esiste prima
//! del primo intervento; ogni riga è un intervento completo in JSON, e per uno stesso `id` vale l'ultima
//! riga (le misure dopo lo svuotamento o la procedura arrivano dopo). Salva codici e valori, mai frasi:
//! si rilegge nella lingua corrente. Si cancella dalle impostazioni.
//!
//! Formato di una riga (versione 1): chiavi in italiano, stati ed esiti con i codici del modello
//! (`Estimate::code`, `ItemOutcome::code`, `MeasurePoint::code`) o con i codici di questo modulo dove il
//! modello non ne ha (motivi, voci di spiegazione, tipo di intervento). I campi assenti valgono `None`.
//!
//! ```text
//! {"versione":1,"id":…,"volume":"C:\\","tipo":"spostamento","origine":"capcut-versioni",
//!  "metodo":"…","stima":{"stato":"esatta","byte":…},
//!  "elementi":[{"percorso":"…","byte":…,"esito":"saltato","motivo":"in-uso"}],
//!  "misure":[{"punto":"partenza","ora":…,"libero":…,"cestino":{"elementi":…,"byte":…}}],
//!  "confronto":{"stimato":{…},"ottenuto":…,"differenza":…,"punti_base":…,
//!               "spiegazioni":[{"voce":"ancora-nel-cestino","byte":…}],"non_spiegata":…}}
//! ```

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use quado_model::intervention::{
    BinCount, Comparison, Explanation, InterventionKind, ItemOutcome, JournalEntry, JournalItem, Measure, MeasurePoint, SkipReason,
};
use quado_model::record::{Estimate, NotEstimable};
use serde::{Deserialize, Serialize};

/// Nome del file nella cartella dei dati di Quado.
pub const FILE_NAME: &str = "interventi.jsonl";

/// Versione del formato delle righe. Una riga con un'altra versione si conta tra quelle illeggibili.
pub const VERSION: u32 = 1;

/// Errore di lettura o scrittura: codice di errore di Windows (0 se non noto).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JournalError {
    Io(u32),
}

/// Il registro letto: un intervento per `id` (l'ultima riga), in ordine di `id`, e quante righe non si
/// sono potute leggere (scritte a metà o da una versione diversa).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Journal {
    pub entries: Vec<JournalEntry>,
    pub unreadable_lines: usize,
}

/// Percorso del registro in `data_dir` (`%APPDATA%\Quado`).
pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_NAME)
}

fn io(e: std::io::Error) -> JournalError {
    JournalError::Io(e.raw_os_error().map_or(0, |c| c as u32))
}

/// Aggiunge una riga; crea cartella e file al primo intervento. La riga si scrive per intero con una
/// sola scrittura, poi `flush` e `sync_all`. Se l'ultima riga del file è rimasta a metà (interruzione
/// durante una scrittura precedente), la nuova comincia su una riga sua.
pub fn append(data_dir: &Path, entry: &JournalEntry) -> Result<(), JournalError> {
    let mut line = serde_json::to_vec(&Riga::from_entry(entry)).map_err(|_| JournalError::Io(0))?;
    line.push(b'\n');
    std::fs::create_dir_all(data_dir).map_err(io)?;
    let mut f = OpenOptions::new().read(true).append(true).create(true).open(path(data_dir)).map_err(io)?;
    if f.metadata().map_err(io)?.len() > 0 {
        let mut last = [0u8; 1];
        f.seek(SeekFrom::End(-1)).map_err(io)?;
        f.read_exact(&mut last).map_err(io)?;
        if last[0] != b'\n' {
            line.insert(0, b'\n');
        }
    }
    f.write_all(&line).map_err(io)?;
    f.flush().map_err(io)?;
    f.sync_all().map_err(io)
}

/// Legge il registro; senza file, un registro vuoto. Le righe vuote non contano; quelle che non si
/// leggono (troncate, di un'altra versione, con codici sconosciuti) si contano e si saltano.
pub fn read(data_dir: &Path) -> Result<Journal, JournalError> {
    let bytes = match std::fs::read(path(data_dir)) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Journal::default()),
        Err(e) => return Err(io(e)),
    };
    let mut by_id = BTreeMap::new();
    let mut unreadable_lines = 0;
    for line in bytes.split(|&b| b == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match parse_line(line) {
            Some(e) => {
                by_id.insert(e.id, e);
            }
            None => unreadable_lines += 1,
        }
    }
    Ok(Journal { entries: by_id.into_values().collect(), unreadable_lines })
}

fn parse_line(line: &[u8]) -> Option<JournalEntry> {
    let r: Riga = serde_json::from_slice(line).ok()?;
    if r.versione != VERSION {
        return None;
    }
    r.into_entry()
}

/// Dimensione del file, se esiste (pagina "Dati e privacy").
pub fn size(data_dir: &Path) -> Option<u64> {
    std::fs::metadata(path(data_dir)).ok().map(|m| m.len())
}

/// Cancella il registro (dalle impostazioni). Senza file non fa niente. La cartella resta: contiene anche
/// le impostazioni e il log.
pub fn delete(data_dir: &Path) -> Result<(), JournalError> {
    match std::fs::remove_file(path(data_dir)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io(e)),
    }
}

// ---------------------------------------------------------------------------------------------
// Codici: quelli del modello, più il tipo di intervento che il modello non ha

const KIND_RECYCLE: &str = "spostamento";
const KIND_PROCEDURE: &str = "procedura";
const KIND_COMMAND: &str = "comando";

const NOT_ESTIMABLE: [NotEstimable; 4] = NotEstimable::ALL;

fn not_estimable_code(n: NotEstimable) -> &'static str {
    n.code()
}

const SKIP_CHANGED: &str = SkipReason::Changed.code();
const SKIP_MISSING: &str = SkipReason::Missing.code();
const SKIP_IN_USE: &str = SkipReason::InUse.code();
const SKIP_UNREADABLE: &str = SkipReason::Unreadable(0).code();

const VOICE_SKIPPED: &str = Explanation::SkippedOrFailed { items: 0, bytes: 0 }.code();
const VOICE_EXCLUDED: &str = Explanation::ExcludedNames { files: 0, bytes: 0 }.code();
const VOICE_IN_BIN: &str = Explanation::StillInBin { bytes: 0 }.code();
const VOICE_OTHER_BIN: &str = Explanation::OtherBinItems { items: 0, bytes: 0 }.code();
const VOICE_OPEN: &str = Explanation::StillOpen { files: 0 }.code();
const VOICE_ACTIVITY: &str = Explanation::OtherActivity { bytes: 0 }.code();

const POINTS: [MeasurePoint; 4] = [MeasurePoint::Start, MeasurePoint::AfterAction, MeasurePoint::AfterEmpty, MeasurePoint::AfterProcedure];

// ---------------------------------------------------------------------------------------------
// Strutture di serializzazione (private): rispecchiano `JournalEntry` con chiavi in italiano.

#[derive(Serialize, Deserialize)]
struct Riga {
    versione: u32,
    id: i64,
    volume: String,
    tipo: String,
    /// Codice della procedura o della regola del comando; assente per uno spostamento.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    codice: Option<String>,
    origine: String,
    metodo: String,
    stima: Stima,
    elementi: Vec<Elemento>,
    misure: Vec<Misura>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    confronto: Option<Confronto>,
}

#[derive(Serialize, Deserialize)]
struct Stima {
    stato: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    byte: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    motivo: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Elemento {
    percorso: String,
    byte: u64,
    esito: String,
    /// Motivo di un elemento saltato.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    motivo: Option<String>,
    /// Codice di errore di Windows di un ricontrollo illeggibile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    errore: Option<u32>,
    /// HRESULT di un elemento fallito o non arrivato nel Cestino.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hresult: Option<i32>,
}

#[derive(Serialize, Deserialize)]
struct Misura {
    punto: String,
    ora: i64,
    libero: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cestino: Option<Cestino>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ancora_presenti: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct Cestino {
    elementi: u64,
    byte: u64,
}

#[derive(Serialize, Deserialize)]
struct Confronto {
    stimato: Stima,
    ottenuto: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    differenza: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    punti_base: Option<u64>,
    spiegazioni: Vec<Voce>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    non_spiegata: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct Voce {
    voce: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    elementi: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    byte: Option<u64>,
    /// Byte con segno (attività di altri processi).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    variazione: Option<i64>,
}

impl Riga {
    fn from_entry(e: &JournalEntry) -> Self {
        let (tipo, codice) = match &e.kind {
            InterventionKind::Recycle => (KIND_RECYCLE, None),
            InterventionKind::Procedure(c) => (KIND_PROCEDURE, Some(c.clone())),
            InterventionKind::Command(c) => (KIND_COMMAND, Some(c.clone())),
        };
        Riga {
            versione: VERSION,
            id: e.id,
            volume: e.volume.clone(),
            tipo: tipo.to_owned(),
            codice,
            origine: e.origin.clone(),
            metodo: e.method.clone(),
            stima: Stima::from_estimate(&e.estimate),
            elementi: e.items.iter().map(Elemento::from_item).collect(),
            misure: e.measures.iter().map(Misura::from_measure).collect(),
            confronto: e.comparison.as_ref().map(Confronto::from_comparison),
        }
    }

    fn into_entry(self) -> Option<JournalEntry> {
        let kind = match (self.tipo.as_str(), self.codice) {
            (KIND_RECYCLE, None) => InterventionKind::Recycle,
            (KIND_PROCEDURE, Some(c)) => InterventionKind::Procedure(c),
            (KIND_COMMAND, Some(c)) => InterventionKind::Command(c),
            _ => return None,
        };
        Some(JournalEntry {
            id: self.id,
            volume: self.volume,
            kind,
            origin: self.origine,
            method: self.metodo,
            estimate: self.stima.to_estimate()?,
            items: self.elementi.into_iter().map(Elemento::into_item).collect::<Option<_>>()?,
            measures: self.misure.into_iter().map(Misura::into_measure).collect::<Option<_>>()?,
            comparison: match self.confronto {
                Some(c) => Some(c.into_comparison()?),
                None => None,
            },
        })
    }
}

impl Stima {
    fn from_estimate(e: &Estimate) -> Self {
        let mut s = Stima { stato: e.code().to_owned(), byte: None, min: None, max: None, motivo: None };
        match *e {
            Estimate::Exact(b) => s.byte = Some(b),
            Estimate::Range { min, max } => (s.min, s.max) = (Some(min), Some(max)),
            Estimate::NotEstimable(n) => s.motivo = Some(not_estimable_code(n).to_owned()),
        }
        s
    }

    fn to_estimate(&self) -> Option<Estimate> {
        let stato = self.stato.as_str();
        if stato == Estimate::Exact(0).code() {
            Some(Estimate::Exact(self.byte?))
        } else if stato == (Estimate::Range { min: 0, max: 0 }).code() {
            Some(Estimate::Range { min: self.min?, max: self.max? })
        } else if stato == Estimate::NotEstimable(NotEstimable::NoAction).code() {
            let motivo = self.motivo.as_deref()?;
            NOT_ESTIMABLE.into_iter().find(|n| not_estimable_code(*n) == motivo).map(Estimate::NotEstimable)
        } else {
            None
        }
    }
}

impl Elemento {
    fn from_item(i: &JournalItem) -> Self {
        let mut e = Elemento { percorso: i.path.clone(), byte: i.bytes, esito: i.outcome.code().to_owned(), motivo: None, errore: None, hresult: None };
        match i.outcome {
            ItemOutcome::Recycled | ItemOutcome::NotReached => {}
            ItemOutcome::Skipped(r) => {
                let motivo = match r {
                    SkipReason::Changed => SKIP_CHANGED,
                    SkipReason::Missing => SKIP_MISSING,
                    SkipReason::InUse => SKIP_IN_USE,
                    SkipReason::Unreadable(code) => {
                        e.errore = Some(code);
                        SKIP_UNREADABLE
                    }
                };
                e.motivo = Some(motivo.to_owned());
            }
            ItemOutcome::Failed(hr) | ItemOutcome::NotInRecycleBin(hr) => e.hresult = Some(hr),
        }
        e
    }

    fn into_item(self) -> Option<JournalItem> {
        let esito = self.esito.as_str();
        let outcome = if esito == ItemOutcome::Recycled.code() {
            ItemOutcome::Recycled
        } else if esito == ItemOutcome::NotReached.code() {
            ItemOutcome::NotReached
        } else if esito == ItemOutcome::Failed(0).code() {
            ItemOutcome::Failed(self.hresult?)
        } else if esito == ItemOutcome::NotInRecycleBin(0).code() {
            ItemOutcome::NotInRecycleBin(self.hresult?)
        } else if esito == ItemOutcome::Skipped(SkipReason::Changed).code() {
            ItemOutcome::Skipped(match self.motivo.as_deref()? {
                SKIP_CHANGED => SkipReason::Changed,
                SKIP_MISSING => SkipReason::Missing,
                SKIP_IN_USE => SkipReason::InUse,
                SKIP_UNREADABLE => SkipReason::Unreadable(self.errore?),
                _ => return None,
            })
        } else {
            return None;
        };
        Some(JournalItem { path: self.percorso, bytes: self.byte, outcome })
    }
}

impl Misura {
    fn from_measure(m: &Measure) -> Self {
        Misura {
            punto: m.point.code().to_owned(),
            ora: m.at,
            libero: m.free,
            cestino: m.bin.map(|b| Cestino { elementi: b.items, byte: b.bytes }),
            ancora_presenti: m.still_present,
        }
    }

    fn into_measure(self) -> Option<Measure> {
        let point = POINTS.into_iter().find(|p| p.code() == self.punto)?;
        Some(Measure {
            point,
            at: self.ora,
            free: self.libero,
            bin: self.cestino.map(|c| BinCount { items: c.elementi, bytes: c.byte }),
            still_present: self.ancora_presenti,
        })
    }
}

impl Confronto {
    fn from_comparison(c: &Comparison) -> Self {
        Confronto {
            stimato: Stima::from_estimate(&c.estimated),
            ottenuto: c.obtained,
            differenza: c.difference,
            punti_base: c.basis_points,
            spiegazioni: c.explanations.iter().map(Voce::from_explanation).collect(),
            non_spiegata: c.unexplained,
        }
    }

    fn into_comparison(self) -> Option<Comparison> {
        Some(Comparison {
            estimated: self.stimato.to_estimate()?,
            obtained: self.ottenuto,
            difference: self.differenza,
            basis_points: self.punti_base,
            explanations: self.spiegazioni.into_iter().map(Voce::into_explanation).collect::<Option<_>>()?,
            unexplained: self.non_spiegata,
        })
    }
}

impl Voce {
    fn from_explanation(e: &Explanation) -> Self {
        let mut v = Voce { voce: String::new(), elementi: None, file: None, byte: None, variazione: None };
        let code = match *e {
            Explanation::SkippedOrFailed { items, bytes } => {
                (v.elementi, v.byte) = (Some(items), Some(bytes));
                VOICE_SKIPPED
            }
            Explanation::ExcludedNames { files, bytes } => {
                (v.file, v.byte) = (Some(files), Some(bytes));
                VOICE_EXCLUDED
            }
            Explanation::StillInBin { bytes } => {
                v.byte = Some(bytes);
                VOICE_IN_BIN
            }
            Explanation::OtherBinItems { items, bytes } => {
                (v.elementi, v.byte) = (Some(items), Some(bytes));
                VOICE_OTHER_BIN
            }
            Explanation::StillOpen { files } => {
                v.file = Some(files);
                VOICE_OPEN
            }
            Explanation::OtherActivity { bytes } => {
                v.variazione = Some(bytes);
                VOICE_ACTIVITY
            }
        };
        v.voce = code.to_owned();
        v
    }

    fn into_explanation(self) -> Option<Explanation> {
        Some(match self.voce.as_str() {
            VOICE_SKIPPED => Explanation::SkippedOrFailed { items: self.elementi?, bytes: self.byte? },
            VOICE_EXCLUDED => Explanation::ExcludedNames { files: self.file?, bytes: self.byte? },
            VOICE_IN_BIN => Explanation::StillInBin { bytes: self.byte? },
            VOICE_OTHER_BIN => Explanation::OtherBinItems { items: self.elementi?, bytes: self.byte? },
            VOICE_OPEN => Explanation::StillOpen { files: self.file? },
            VOICE_ACTIVITY => Explanation::OtherActivity { bytes: self.variazione? },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_not_estimable_reason_has_its_own_code() {
        let mut codes: Vec<&str> = NOT_ESTIMABLE.iter().map(|n| not_estimable_code(*n)).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), NOT_ESTIMABLE.len());
    }

    /// Posizione in `POINTS`: il `match` non compila più se il modello aggiunge un punto di misura.
    fn point_index(p: MeasurePoint) -> usize {
        match p {
            MeasurePoint::Start => 0,
            MeasurePoint::AfterAction => 1,
            MeasurePoint::AfterEmpty => 2,
            MeasurePoint::AfterProcedure => 3,
        }
    }

    /// Come sopra per i motivi della stima non stimabile.
    fn not_estimable_index(n: NotEstimable) -> usize {
        match n {
            NotEstimable::HardLinksNotCounted => 0,
            NotEstimable::ExternalProcedure => 1,
            NotEstimable::NoAction => 2,
            NotEstimable::AtSimulation => 3,
        }
    }

    #[test]
    fn code_tables_list_every_variant() {
        for (k, p) in POINTS.into_iter().enumerate() {
            assert_eq!(point_index(p), k);
            assert_eq!(POINTS.iter().filter(|q| q.code() == p.code()).count(), 1);
        }
        for (k, n) in NOT_ESTIMABLE.into_iter().enumerate() {
            assert_eq!(not_estimable_index(n), k);
        }
    }
}
