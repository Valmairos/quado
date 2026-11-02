// SPDX-License-Identifier: GPL-3.0-or-later
//! Simulazione e piano confermato.
//!
//! - [`Simulazione`] la produce solo `simulate::simulate`: i campi sono privati, si leggono con i metodi.
//! - [`PianoConfermato`] lo costruisce solo [`conferma`], dalla simulazione e dalla scelta dell'utente
//!   nel dialogo. I campi sono privati e non esiste un altro costruttore; `execute::esegui` lo consuma,
//!   quindi un piano vale una volta.
//! - Esc, Annulla e la X chiudono il dialogo senza chiamare [`conferma`].
//!
//! I test di compilazione sotto riportano il codice d'errore atteso, controllato a mano il
//! 14/10/2026 come in `launch.rs`: rustdoc stabile non lo verifica.
//!
//! ```compile_fail,E0451
//! // Fuori dal crate un piano non si costruisce con i suoi campi.
//! use quado_actions::plan::PianoConfermato;
//! let _ = PianoConfermato { items: Vec::new(), volume: String::new(), estimate: unimplemented!(), origin: String::new() };
//! ```
//!
//! ```compile_fail,E0451
//! // Nemmeno una simulazione: si ottiene solo da `simulate::simulate`.
//! use quado_actions::plan::Simulazione;
//! let _ = Simulazione { checked_at: 0, ..unimplemented!() };
//! ```
//!
//! ```compile_fail,E0599
//! // Nessun piano vuoto o predefinito.
//! use quado_actions::plan::PianoConfermato;
//! let _ = PianoConfermato::default();
//! ```
//!
//! ```compile_fail,E0599
//! // Un piano non si copia: vale una volta.
//! use quado_actions::plan::PianoConfermato;
//! fn copia(piano: PianoConfermato) -> (PianoConfermato, PianoConfermato) {
//!     (piano.clone(), piano)
//! }
//! ```
//!
//! ```compile_fail,E0382
//! // Un piano eseguito è consumato: la seconda esecuzione non compila.
//! use quado_actions::execute::{ExecProgress, Mover, esegui};
//! use quado_actions::plan::PianoConfermato;
//! use quado_actions::probe::Probe;
//! fn due_volte(piano: PianoConfermato, probe: &dyn Probe, mover: &dyn Mover) {
//!     let p = ExecProgress::default();
//!     let _ = esegui(piano, probe, mover, &p);
//!     let _ = esegui(piano, probe, mover, &p);
//! }
//! ```
//!
//! ```compile_fail,E0308
//! // L'esecuzione accetta solo un piano confermato, non una simulazione.
//! use quado_actions::execute::{ExecProgress, Mover, esegui};
//! use quado_actions::plan::Simulazione;
//! use quado_actions::probe::Probe;
//! fn senza_conferma(sim: Simulazione, probe: &dyn Probe, mover: &dyn Mover) {
//!     let _ = esegui(sim, probe, mover, &ExecProgress::default());
//! }
//! ```
//!
//! ```
//! // L'unico percorso: simulazione, scelta, conferma, esecuzione (qui con le letture simulate).
//! use quado_actions::execute::{ExecProgress, esegui};
//! use quado_actions::fake::{self, FakeMover, FakeProbe};
//! use quado_actions::plan::conferma;
//! use quado_actions::simulate::{Request, RequestItem, SimProgress, simulate};
//! use quado_model::intervention::ItemOutcome;
//! use quado_model::record::{Estimate, State};
//!
//! let probe = FakeProbe::new().volume("R:\\").file("R:\\prova\\a.bin", 1000, 4096).file("R:\\prova\\b.txt", 10, 0);
//! let record = fake::record(State::Manageable(fake::proof()));
//! let req = Request { items: vec![RequestItem { path: "R:\\prova".into(), fixed: None }], record, protected: Vec::new() };
//! let sim = simulate(req, &probe, &SimProgress::default()).expect("non annullata");
//! let scelta = sim.default_choice();
//! assert_eq!(sim.estimate(&scelta.selected).estimate, Estimate::Exact(4096));
//! let piano = conferma(&sim, &scelta).expect("piano confermato");
//! let mover = FakeMover::new(&probe);
//! let riepilogo = esegui(piano, &probe, &mover, &ExecProgress::default());
//! assert_eq!(riepilogo.items[0].outcome, ItemOutcome::Recycled);
//! assert!(!probe.exists("R:\\prova"));
//! assert_eq!(mover.calls(), ["R:\\prova"]);
//! ```

use quado_model::intervention::{ProtectedKind, Reference, ReferenceReport};
use quado_model::record::{Action, Check, CheckKind, CheckOutcome, Estimate, Expiry, Record, State};

use crate::estimate::{self, Linked};
use crate::probe::VolumeBin;
use crate::restart::{ProcessUse, RmError};
use crate::simulate::{CHECK_CAPACITY, CHECK_REFERENCES, CHECK_RESTART_MANAGER, FixedReason};

/// Perché un elemento non si può spostare: esito negativo di un controllo automatico.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// Elemento protetto, senza l'eccezione via regola o con un suo controllo non soddisfatto.
    Protected(ProtectedKind),
    /// Il volume non ha Cestino (rimovibile, rete).
    NoRecycleBin,
    /// `NukeOnDelete` = 1: Windows eliminerebbe senza passare dal Cestino.
    NukeOnDelete,
    /// `MaxCapacity` non registrata: la capacità non si conosce e non si sposta niente (scelta prudente).
    CapacityUnknown,
    /// Dimensione logica dell'elemento oltre la capacità del Cestino: Windows lo eliminerebbe per sempre
    /// (Windows conta la dimensione logica, `docs/m1/cestino-verifiche.md`).
    LargerThanBin { logical: u64, capacity: u64 },
    /// Processi che usano l'elemento, con il nome dato da Restart Manager.
    InUse(Vec<ProcessUse>),
    /// Restart Manager non ha risposto: l'uso non si conosce.
    RestartManager(RmError),
    /// Riferimenti che puntano all'elemento.
    Referenced(Vec<Reference>),
    /// Percorso dell'elemento di 260 caratteri UTF-16 o più (`MAX_PATH`): la shell non lo sposta nel
    /// Cestino e chiede di eliminarlo per sempre. Blocca anche se esiste un nome breve 8.3 con cui
    /// spostarlo (scelta prudente della v1).
    LongPath,
    /// Cartella con almeno un elemento dentro il cui percorso completo ha 260 caratteri UTF-16 o più:
    /// stesso motivo e stessa scelta prudente di [`Block::LongPath`].
    ContainsLongPath,
    /// Non si è potuto leggere (codice di errore di Windows).
    Unreadable(u32),
    /// Non esiste.
    Missing,
    /// Su un volume diverso da quello degli altri elementi.
    OtherVolume,
}

/// Controllo manuale da spuntare nella conferma.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ManualCheck {
    /// L'elemento sta in una cartella di sincronizzazione: spostarlo nel Cestino lo elimina anche nel
    /// cloud e sugli altri dispositivi. I dati sono il nome del programma e come Quado ha riconosciuto la
    /// cartella (`crate::sync`), che decide il testo.
    Synced { provider: String, kind: crate::sync::SyncKind },
}

/// Un elemento della simulazione.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimItem {
    pub path: String,
    pub directory: bool,
    /// Visibile ma non selezionabile (versione in uso, versione tenuta, copia che resta).
    pub fixed: Option<FixedReason>,
    /// Esiti negativi dei controlli automatici: vuoto se tutti passano.
    pub blocks: Vec<Block>,
    /// Controlli manuali propri dell'elemento.
    pub manual: Vec<ManualCheck>,
    /// File e cartelle letti nell'elemento.
    pub files: u64,
    pub dirs: u64,
    /// Dimensione logica totale (capacità del Cestino: Windows conta la logica, `docs/m1/cestino-verifiche.md`).
    pub logical: u64,
    /// Byte allocati dell'elemento, ogni file contato una volta (senza dedup tra elementi).
    pub alloc: u64,
    /// Cartelle negate dentro l'elemento: il loro contenuto non entra nella stima.
    pub denied_dirs: u64,
}

/// Stima della selezione, con i fatti che la spiegano.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionEstimate {
    /// Esatta, intervallo (nomi non conoscibili per cartelle negate) o non stimabile.
    pub estimate: Estimate,
    /// File esclusi perché hanno altri nomi fuori dalla selezione: quanti e i loro byte allocati.
    pub excluded_names: (u64, u64),
    /// Cartelle negate nella selezione.
    pub denied_dirs: u64,
    /// Dimensione logica della selezione (controllo di capacità).
    pub logical: u64,
}

/// Capacità del Cestino contro contenuto attuale più selezione.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapacityCheck {
    /// Contenuto più selezione entro la capacità.
    Fits { capacity: u64, total: u64 },
    /// Oltre la capacità: Windows eliminerebbe da solo gli elementi più vecchi. Spostamento bloccato.
    Exceeded { capacity: u64, total: u64 },
    /// Capacità o contenuto non noti: bloccato.
    Unknown,
}

/// Dati di un elemento che servono alla stima e al piano ma non al dialogo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ItemExtra {
    /// Allocate dei file con un solo nome (o senza identità).
    pub(crate) plain: u64,
    /// Fotografia per il ricontrollo subito prima dello spostamento.
    pub(crate) snapshot: Snapshot,
    /// File registrati con Restart Manager in simulazione: il ricontrollo delle cartelle usa gli stessi.
    pub(crate) sample: Vec<String>,
}

/// Risultato della simulazione. Si costruisce solo con `simulate::simulate`.
#[derive(Clone, Debug)]
pub struct Simulazione {
    pub(crate) record: Record,
    pub(crate) items: Vec<SimItem>,
    pub(crate) volume: VolumeBin,
    pub(crate) storage_sense_days: Option<u32>,
    pub(crate) references: ReferenceReport,
    /// Chiavi dei controlli manuali della regola (`CheckKind::Manual`).
    pub(crate) rule_manual: Vec<String>,
    /// `FILETIME` dei controlli (Restart Manager, riferimenti, capacità).
    pub(crate) checked_at: i64,
    /// Un valore per elemento.
    pub(crate) extra: Vec<ItemExtra>,
    /// File con più nomi, su tutti gli elementi non fissi.
    pub(crate) linked: Vec<Linked>,
}

impl Simulazione {
    pub fn record(&self) -> &Record {
        &self.record
    }

    pub fn items(&self) -> &[SimItem] {
        &self.items
    }

    pub fn volume(&self) -> &VolumeBin {
        &self.volume
    }

    /// Giorni dopo cui Sensore memoria svuota il Cestino, se lo fa (la conferma lo dice).
    pub fn storage_sense_days(&self) -> Option<u32> {
        self.storage_sense_days
    }

    /// Tutti i riferimenti trovati e le fonti non lette per intero ("parziale").
    pub fn references(&self) -> &ReferenceReport {
        &self.references
    }

    /// Controlli manuali della regola da spuntare (chiavi dei testi).
    pub fn rule_manual(&self) -> &[String] {
        &self.rule_manual
    }

    pub fn checked_at(&self) -> i64 {
        self.checked_at
    }

    /// Selezione proposta: gli elementi non fissi e senza blocchi. I controlli manuali non tolgono un
    /// elemento dalla selezione: si spuntano. Per Da verificare, e per Decidi tu con più elementi, le caselle
    /// partono vuote: la scelta la fa l'utente, elemento per elemento o con «Seleziona tutti».
    /// Con un solo elemento il clic che ha aperto l'anteprima era già la scelta.
    pub fn default_selection(&self) -> Vec<bool> {
        let proposed = self.items.iter().filter(|it| it.fixed.is_none()).count();
        let ask = matches!(self.record.state, State::ToVerify(_)) || (self.record.state == State::YouDecide && proposed > 1);
        self.items.iter().map(|it| !ask && it.fixed.is_none() && it.blocks.is_empty()).collect()
    }

    /// Tutti gli elementi che si possono scegliere: non fissi e senza blocchi («Seleziona tutti»).
    pub fn selectable(&self) -> Vec<bool> {
        self.items.iter().map(|it| it.fixed.is_none() && it.blocks.is_empty()).collect()
    }

    /// Stato iniziale del dialogo: selezione proposta, nessun controllo manuale spuntato.
    pub fn default_choice(&self) -> Scelta {
        Scelta {
            selected: self.default_selection(),
            ticked_rule: vec![false; self.rule_manual.len()],
            ticked_items: self.items.iter().map(|it| vec![false; it.manual.len()]).collect(),
        }
    }

    /// Stima della selezione (`selected` ha un valore per elemento). Gli elementi fissi non entrano mai.
    pub fn estimate(&self, selected: &[bool]) -> SelectionEstimate {
        estimate::estimate(&self.items, |i| self.extra.get(i).map_or(0, |x| x.plain), &self.linked, selected)
    }

    /// Capacità del Cestino con questa selezione: contenuto attuale più la dimensione logica della
    /// selezione (Windows conta la logica e non le cartelle orfane, provato su un VHDX di prova,
    /// `docs/m1/cestino-verifiche.md`).
    pub fn capacity(&self, selected: &[bool]) -> CapacityCheck {
        let (Some(capacity), Some(content)) = (self.volume.capacity, self.volume.content) else {
            return CapacityCheck::Unknown;
        };
        let total = content.bytes + self.estimate(selected).logical;
        if total <= capacity { CapacityCheck::Fits { capacity, total } } else { CapacityCheck::Exceeded { capacity, total } }
    }

    /// I controlli del record con l'esito della simulazione al posto di quello del record per i tre
    /// controlli che la simulazione esegue: Restart Manager e riferimenti non superati se un elemento
    /// proposto (non fisso) ne è bloccato; capacità non superata se la selezione proposta non ci sta o un
    /// elemento proposto è più grande del Cestino o la capacità non si conosce. Gli altri controlli
    /// restano come sono.
    pub fn rule_checks(&self) -> Vec<Check> {
        let proposed = || self.items.iter().filter(|it| it.fixed.is_none()).flat_map(|it| &it.blocks);
        let rm = !proposed().any(|b| matches!(b, Block::InUse(_) | Block::RestartManager(_)));
        let refs = !proposed().any(|b| matches!(b, Block::Referenced(_)));
        let fits = matches!(self.capacity(&self.selectable()), CapacityCheck::Fits { .. });
        let cap = fits && !proposed().any(|b| matches!(b, Block::LargerThanBin { .. } | Block::CapacityUnknown));
        let outcome = |ok: bool| if ok { CheckOutcome::Passed } else { CheckOutcome::Failed };
        self.record
            .checks
            .iter()
            .map(|c| {
                let mut c = c.clone();
                if c.kind == CheckKind::Automatic {
                    match c.key.as_str() {
                        CHECK_RESTART_MANAGER => c.outcome = outcome(rm),
                        CHECK_REFERENCES => c.outcome = outcome(refs),
                        CHECK_CAPACITY => c.outcome = outcome(cap),
                        _ => {}
                    }
                }
                c
            })
            .collect()
    }
}

/// Scelta dell'utente nel dialogo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scelta {
    /// Un valore per elemento della simulazione.
    pub selected: Vec<bool>,
    /// Un valore per controllo manuale della regola (`Simulazione::rule_manual`).
    pub ticked_rule: Vec<bool>,
    /// Per elemento, un valore per ciascuno dei suoi controlli manuali (`SimItem::manual`).
    pub ticked_items: Vec<Vec<bool>>,
}

/// Perché `conferma` non costruisce il piano. Codici, non frasi.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rifiuto {
    /// Il record non propone uno spostamento nel Cestino, o il suo stato non lo ammette.
    NotARecycle,
    /// La scelta non ha la forma della simulazione (lunghezze diverse).
    Shape,
    /// Nessun elemento scelto.
    Empty,
    /// Elemento fisso scelto (indice).
    Fixed(usize),
    /// Elemento con un blocco (indice e blocco).
    Blocked(usize, Block),
    /// Controllo manuale dell'elemento non spuntato (indice dell'elemento, indice del controllo).
    ItemNotTicked(usize, usize),
    /// Controllo manuale della regola non spuntato (indice).
    RuleNotTicked(usize),
    /// Capacità del Cestino superata o non nota.
    Capacity(CapacityCheck),
}

/// Un elemento del piano, con la fotografia per il ricontrollo subito prima dello spostamento.
#[derive(Debug)]
pub struct PlanItem {
    pub(crate) path: String,
    pub(crate) directory: bool,
    pub(crate) alloc: u64,
    pub(crate) snapshot: Snapshot,
    /// Campione di Restart Manager della simulazione, ripetuto al ricontrollo delle cartelle.
    pub(crate) sample: Vec<String>,
}

impl PlanItem {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn directory(&self) -> bool {
        self.directory
    }

    /// Byte allocati stimati dell'elemento.
    pub fn alloc(&self) -> u64 {
        self.alloc
    }
}

/// Fotografia di un elemento alla simulazione: identità, dimensione, data di modifica; per le cartelle
/// anche numero di file e dimensione logica totale.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub(crate) file_id: u128,
    pub(crate) volume_serial: u64,
    /// Logica del file; per una cartella il totale della visita.
    pub(crate) logical: u64,
    /// Data di modifica dell'elemento stesso.
    pub(crate) modified: i64,
    pub(crate) files: u64,
}

/// Piano confermato: elementi scelti, ciascuno con la sua fotografia. Si costruisce solo con
/// [`conferma`] e si consuma con `execute::esegui`.
#[derive(Debug)]
pub struct PianoConfermato {
    pub(crate) items: Vec<PlanItem>,
    pub(crate) volume: String,
    pub(crate) estimate: SelectionEstimate,
    pub(crate) origin: String,
}

impl PianoConfermato {
    pub fn items(&self) -> &[PlanItem] {
        &self.items
    }

    /// Radice del volume.
    pub fn volume(&self) -> &str {
        &self.volume
    }

    pub fn estimate(&self) -> &SelectionEstimate {
        &self.estimate
    }

    /// Regola o funzione del record (`Record::origin_id`).
    pub fn origin(&self) -> &str {
        &self.origin
    }
}

/// Il record ammette uno spostamento nel Cestino: azione Cestino, stato Gestibile, Da verificare o
/// Decidi tu, regola non scaduta.
fn admits_recycle(r: &Record) -> bool {
    let state = matches!(r.state, State::Manageable(_) | State::ToVerify(_) | State::YouDecide);
    r.action == Action::Recycle && state && r.expiry != Expiry::Expired
}

/// Motivi per cui la scelta non darebbe un piano; vuoto se `conferma` lo costruirebbe. Serve al dialogo
/// per attivare il pulsante principale. Con una scelta di forma sbagliata si ferma a [`Rifiuto::Shape`].
pub fn check(sim: &Simulazione, scelta: &Scelta) -> Vec<Rifiuto> {
    let mut out = Vec::new();
    if !admits_recycle(&sim.record) {
        out.push(Rifiuto::NotARecycle);
    }
    let n = sim.items.len();
    let shape = scelta.selected.len() == n
        && scelta.ticked_items.len() == n
        && scelta.ticked_rule.len() == sim.rule_manual.len()
        && scelta.ticked_items.iter().zip(&sim.items).all(|(t, it)| t.len() == it.manual.len());
    if !shape {
        out.push(Rifiuto::Shape);
        return out;
    }
    if !scelta.selected.iter().any(|&s| s) {
        out.push(Rifiuto::Empty);
    }
    for (i, it) in sim.items.iter().enumerate() {
        if !scelta.selected[i] {
            continue;
        }
        if it.fixed.is_some() {
            out.push(Rifiuto::Fixed(i));
            continue;
        }
        out.extend(it.blocks.iter().map(|b| Rifiuto::Blocked(i, b.clone())));
        out.extend(scelta.ticked_items[i].iter().enumerate().filter(|(_, t)| !**t).map(|(k, _)| Rifiuto::ItemNotTicked(i, k)));
    }
    out.extend(scelta.ticked_rule.iter().enumerate().filter(|(_, t)| !**t).map(|(k, _)| Rifiuto::RuleNotTicked(k)));
    match sim.capacity(&scelta.selected) {
        CapacityCheck::Fits { .. } => {}
        c => out.push(Rifiuto::Capacity(c)),
    }
    out
}

/// L'unico costruttore di [`PianoConfermato`]: lo chiama il pulsante principale del dialogo di conferma.
/// Respinge elementi protetti (salvo l'eccezione via regola con tutti i controlli soddisfatti), elementi
/// con un controllo automatico fallito, controlli manuali non spuntati e capacità superata.
pub fn conferma(sim: &Simulazione, scelta: &Scelta) -> Result<PianoConfermato, Vec<Rifiuto>> {
    let reasons = check(sim, scelta);
    if !reasons.is_empty() {
        return Err(reasons);
    }
    // La quota di ogni elemento nella stima della selezione, non la sua allocata da solo: un file con un
    // nome fuori dalla selezione è escluso dalla stima e non deve tornare nel confronto per elemento.
    let shares = estimate::shares(&sim.items, |i| sim.extra.get(i).map_or(0, |x| x.plain), &sim.linked, &scelta.selected);
    let items = sim
        .items
        .iter()
        .zip(&sim.extra)
        .zip(&scelta.selected)
        .zip(&shares)
        .filter(|((_, s), _)| **s)
        .map(|(((it, x), _), share)| PlanItem {
            path: it.path.clone(),
            directory: it.directory,
            alloc: *share,
            snapshot: x.snapshot,
            sample: x.sample.clone(),
        })
        .collect();
    Ok(PianoConfermato {
        items,
        volume: sim.volume.root.clone(),
        estimate: sim.estimate(&scelta.selected),
        origin: sim.record.origin_id().to_owned(),
    })
}
