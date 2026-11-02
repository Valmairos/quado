// SPDX-License-Identifier: GPL-3.0-or-later
//! Stimato contro ottenuto: differenza, soglia del 5% e voci di spiegazione, ognuna
//! sostenuta da un fatto misurato. Funzioni pure sui tipi di `intervention`.
//!
//! # Differenza e percentuale
//!
//! L'ottenuto è la variazione del volume: spazio libero all'ultima misura meno spazio libero alla prima
//! (la partenza), con il segno. La stima dà un intervallo atteso: `[e, e]` per una stima esatta,
//! `[min, max]` per un intervallo. La differenza è la distanza con segno dell'ottenuto dall'intervallo
//! (0 se ci cade dentro), e la percentuale si calcola sull'estremo più vicino. I punti base si
//! arrotondano per eccesso: così `basis_points` oltre [`EXPLAIN_THRESHOLD_BP`] equivale esattamente a
//! "differenza oltre il 5%", e una differenza diversa da zero non si mostra mai come 0.
//!
//! # Contributo di ogni voce
//!
//! Oltre la soglia ogni voce, solo se il suo fatto misurato c'è, sposta l'ottenuto atteso di un
//! contributo con segno (positivo: il volume ha liberato più della stima). Si toglie dall'ottenuto la
//! somma dei contributi e si guarda di nuovo la distanza dall'intervallo: quello che resta è il resto.
//!
//! | Voce | Contributo | Perché |
//! |---|---|---|
//! | [`Explanation::SkippedOrFailed`] | − byte | Stimati, ma rimasti al loro posto |
//! | [`Explanation::ExcludedNames`] | 0 | La stima li esclude già e il volume non li libera finché esiste un altro nome: la voce dice perché stimato e ottenuto sono minori del totale della selezione, non sposta la differenza |
//! | [`Explanation::StillInBin`] | − byte | Spostati ma non ancora liberati: subito dopo lo spostamento l'ottenuto è circa 0 e questa voce spiega la stima intera |
//! | [`Explanation::OtherBinItems`] | + byte | Lo svuotamento libera anche ciò che era già nel Cestino: dopo lo svuotamento l'ottenuto è più grande della stima |
//! | [`Explanation::StillOpen`] | 0 | Non quantificabile (Restart Manager dà i file, non i byte); impedisce però di attribuire ad altri processi un ottenuto minore del previsto |
//! | [`Explanation::OtherActivity`] | il resto | Solo se la riscansione dice che la selezione non occupa più spazio (`still_present == Some(0)`): la differenza è dichiarata e non attribuita all'intervento |
//!
//! Il resto che nessuna voce spiega e che supera ancora la soglia è `unexplained` ("Differenza non
//! spiegata"). Con la riscansione che conferma la selezione sparita il resto va invece in
//! [`Explanation::OtherActivity`], tranne quando è negativo e ci sono file ancora aperti: allora lo
//! spazio può non essere tornato per quei file, e il resto resta non spiegato.
//!
//! Per una stima esatta vale `difference = somma dei contributi + resto`; per un intervallo il resto
//! è la distanza dell'ottenuto corretto dall'intervallo.

use crate::intervention::{Comparison, EXPLAIN_THRESHOLD_BP, Explanation, Measure};
use crate::record::Estimate;

/// Fatti misurati che possono spiegare la differenza (le voci della tabella nel doc del modulo). Li
/// raccoglie l'interfaccia dal riepilogo dell'esecuzione, dalla simulazione e dalla riscansione.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExplainFacts {
    /// Elementi saltati o falliti nel riepilogo: quanti e i loro byte stimati. Vanno contati gli
    /// elementi rimasti al loro posto (saltati, falliti, non raggiunti); non quelli spariti senza
    /// arrivare nel Cestino, il cui spazio può essere stato liberato.
    pub skipped_or_failed: (u64, u64),
    /// File esclusi dalla stima perché hanno altri nomi fuori dalla selezione: quanti e i loro byte.
    pub excluded_names: (u64, u64),
    /// Byte stimati degli elementi che questo intervento ha spostato nel Cestino.
    pub recycled_bytes: u64,
    /// Di questi, byte ancora nel Cestino all'ultima misura; `None` se non noto.
    pub still_in_bin: Option<u64>,
    /// File della selezione ancora aperti al momento dello svuotamento (Restart Manager).
    pub still_open_files: u64,
}

/// Tolleranza tra stima esatta e variazione reale su VHDX, parte proporzionale: 1% della stima, in
/// punti base. Confermata dalle misure su VHDX di `quado-actions/tests/fine_vhdx.rs`: chi la cambia
/// scrive qui la misura che lo giustifica.
pub const TOLERANCE_BP: u64 = 100;

/// Tolleranza, parte fissa: 1 MiB (metadati, cluster, `$I` del Cestino). Confermata come
/// [`TOLERANCE_BP`].
pub const TOLERANCE_FIXED: u64 = 1 << 20;

/// La variazione reale `obtained` cade entro la tolleranza dalla stima esatta `estimate`:
/// |ottenuto − stima| ≤ 1% della stima + 1 MiB, senza arrotondamenti.
pub fn within_tolerance(estimate: u64, obtained: i64) -> bool {
    let diff = (i128::from(obtained) - i128::from(estimate)).unsigned_abs();
    diff * 10_000 <= u128::from(estimate) * u128::from(TOLERANCE_BP) + u128::from(TOLERANCE_FIXED) * 10_000
}

/// Intervallo atteso dalla stima (estremi ordinati); `None` se non c'è un valore o vale 0.
fn expected(estimate: &Estimate) -> Option<(u64, u64)> {
    match *estimate {
        Estimate::Exact(e) if e > 0 => Some((e, e)),
        Estimate::Range { min, max } if min.max(max) > 0 => Some((min.min(max), min.max(max))),
        _ => None,
    }
}

/// Distanza con segno di `x` dall'intervallo `[lo, hi]` e l'estremo più vicino; `None` se `x` ci cade
/// dentro (estremi compresi).
fn distance(x: i128, lo: u64, hi: u64) -> Option<(i128, u64)> {
    if x < i128::from(lo) {
        Some((x - i128::from(lo), lo))
    } else if x > i128::from(hi) {
        Some((x - i128::from(hi), hi))
    } else {
        None
    }
}

/// |d| / riferimento in punti base, arrotondati per eccesso; `None` con riferimento 0.
fn basis_points(d: i128, reference: u64) -> Option<u64> {
    if reference == 0 {
        return None;
    }
    let bp = (d.unsigned_abs() * 10_000).div_ceil(u128::from(reference));
    Some(u64::try_from(bp).unwrap_or(u64::MAX))
}

fn clamp(x: i128) -> i64 {
    i64::try_from(x).unwrap_or(if x < 0 { i64::MIN } else { i64::MAX })
}

/// Altri elementi eliminati dal Cestino tra la partenza e l'ultima misura (dimensione del Cestino
/// alla partenza e dopo lo svuotamento), secondo il conteggio di Windows. Quelli che c'erano
/// alla partenza e non ci sono più: `cestino alla partenza − (cestino ora − ciò che questo intervento
/// vi ha spostato e che c'è ancora)`, al più quanto c'era alla partenza. Gli elementi spostati da questo
/// intervento non entrano nel conto: alla partenza non c'erano.
///
/// La voce c'è solo se il numero di elementi è sceso sotto quello della partenza: senza uno svuotamento
/// (per esempio subito dopo lo spostamento, o con il dialogo annullato) non compare, anche se la stima
/// in byte allocati e il conteggio di Windows in byte logici non coincidono. Con parte dell'intervento
/// ancora nel Cestino il numero di elementi è per difetto.
fn other_bin_items(first: &Measure, last: &Measure, ours_still: u64) -> Option<(u64, u64)> {
    let (start, now) = (first.bin?, last.bin?);
    let items = start.items.saturating_sub(now.items);
    if items == 0 {
        return None;
    }
    let bytes = start.bytes.saturating_add(ours_still).saturating_sub(now.bytes).min(start.bytes);
    (bytes > 0).then_some((items, bytes))
}

/// Confronto all'ultima misura di `measures` (la prima è la partenza). `None` con meno di due misure.
///
/// Con una stima non stimabile o pari a 0 c'è solo l'ottenuto. Con un intervallo che comincia da 0 e un
/// ottenuto negativo la differenza c'è ma la percentuale no (l'estremo più vicino è 0), quindi niente
/// voci: come per una stima pari a 0.
pub fn compare(estimate: &Estimate, measures: &[Measure], facts: &ExplainFacts) -> Option<Comparison> {
    let [first, .., last] = measures else { return None };
    let obtained = i128::from(last.free) - i128::from(first.free);
    let mut c = Comparison {
        estimated: *estimate,
        obtained: clamp(obtained),
        difference: None,
        basis_points: None,
        explanations: Vec::new(),
        unexplained: None,
    };
    let Some((lo, hi)) = expected(estimate) else { return Some(c) };
    let Some((difference, reference)) = distance(obtained, lo, hi) else {
        c.difference = Some(0);
        c.basis_points = Some(0);
        return Some(c);
    };
    c.difference = Some(clamp(difference));
    c.basis_points = basis_points(difference, reference);
    match c.basis_points {
        Some(bp) if bp > EXPLAIN_THRESHOLD_BP => {}
        _ => return Some(c),
    }

    // Voci nell'ordine della tabella nel doc del modulo, ciascuna con il suo contributo.
    let mut contributions: i128 = 0;
    let (sf_items, sf_bytes) = facts.skipped_or_failed;
    if sf_items > 0 {
        c.explanations.push(Explanation::SkippedOrFailed { items: sf_items, bytes: sf_bytes });
        contributions -= i128::from(sf_bytes);
    }
    let (ex_files, ex_bytes) = facts.excluded_names;
    if ex_files > 0 {
        c.explanations.push(Explanation::ExcludedNames { files: ex_files, bytes: ex_bytes });
    }
    let ours_still = facts.still_in_bin.unwrap_or(0);
    if ours_still > 0 {
        c.explanations.push(Explanation::StillInBin { bytes: ours_still });
        contributions -= i128::from(ours_still);
    }
    if let Some((items, bytes)) = other_bin_items(first, last, ours_still) {
        c.explanations.push(Explanation::OtherBinItems { items, bytes });
        contributions += i128::from(bytes);
    }
    if facts.still_open_files > 0 {
        c.explanations.push(Explanation::StillOpen { files: facts.still_open_files });
    }

    // Il resto: distanza dall'intervallo dell'ottenuto senza i contributi spiegati.
    let rest = distance(obtained - contributions, lo, hi).map_or(0, |(d, _)| d);
    if rest == 0 {
        return Some(c);
    }
    let selection_gone = last.still_present == Some(0);
    let open_files_may_hold_space = rest < 0 && facts.still_open_files > 0;
    if selection_gone && !open_files_may_hold_space {
        c.explanations.push(Explanation::OtherActivity { bytes: clamp(rest) });
    } else if basis_points(rest, reference).is_some_and(|bp| bp > EXPLAIN_THRESHOLD_BP) {
        c.unexplained = Some(clamp(rest));
    }
    Some(c)
}
