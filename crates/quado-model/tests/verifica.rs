// SPDX-License-Identifier: GPL-3.0-or-later
//! Stimato contro ottenuto: un caso per ogni stato della stima e per ogni voce di spiegazione, e gli
//! scenari del flusso di verifica.

use quado_model::intervention::{BinCount, Comparison, EXPLAIN_THRESHOLD_BP, Explanation, Measure, MeasurePoint};
use quado_model::record::{Estimate, NotEstimable};
use quado_model::verification::{ExplainFacts, TOLERANCE_FIXED, compare, within_tolerance};

const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;
/// Spazio libero alla partenza, lontano da 0 per provare anche le variazioni negative.
const FREE: u64 = 100 * GIB;

fn bin(items: u64, bytes: u64) -> Option<BinCount> {
    Some(BinCount { items, bytes })
}

fn m(point: MeasurePoint, free: u64, bin: Option<BinCount>, still_present: Option<u64>) -> Measure {
    Measure { point, at: 0, free, bin, still_present }
}

fn start() -> Measure {
    m(MeasurePoint::Start, FREE, bin(0, 0), None)
}

/// Partenza e una misura dopo lo svuotamento con variazione `delta` e Cestino vuoto.
fn after_empty(delta: i64, still_present: Option<u64>) -> Vec<Measure> {
    vec![start(), m(MeasurePoint::AfterEmpty, FREE.checked_add_signed(delta).expect("spazio libero"), bin(0, 0), still_present)]
}

/// Senza riscansione.
fn emptied(delta: i64) -> Vec<Measure> {
    after_empty(delta, None)
}

/// Con la riscansione che non trova più la selezione al suo posto.
fn rescanned(delta: i64) -> Vec<Measure> {
    after_empty(delta, Some(0))
}

fn run(estimate: Estimate, measures: &[Measure], facts: ExplainFacts) -> Comparison {
    compare(&estimate, measures, &facts).expect("almeno due misure")
}

/// Per una stima esatta: differenza = somma dei contributi + resto (doc di `verification`).
fn accounted(c: &Comparison) -> i64 {
    let mut sum = 0i64;
    for e in &c.explanations {
        sum += match *e {
            Explanation::SkippedOrFailed { bytes, .. } => -(bytes as i64),
            Explanation::ExcludedNames { .. } | Explanation::StillOpen { .. } => 0,
            Explanation::StillInBin { bytes } => -(bytes as i64),
            Explanation::OtherBinItems { bytes, .. } => bytes as i64,
            Explanation::OtherActivity { bytes } => bytes,
        };
    }
    sum + c.unexplained.unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------
// Stati della stima (tabella "Confronto tra stimato e ottenuto")

#[test]
fn exact_estimate_difference_is_obtained_minus_estimated() {
    let c = run(Estimate::Exact(GIB), &emptied(GIB as i64), ExplainFacts::default());
    assert_eq!(c.estimated, Estimate::Exact(GIB));
    assert_eq!(c.obtained, GIB as i64);
    assert_eq!(c.difference, Some(0));
    assert_eq!(c.basis_points, Some(0));
    assert!(c.explanations.is_empty());
    assert_eq!(c.unexplained, None);

    // 900 MiB ottenuti su 1 GiB stimato: −124 MiB, 12,11% arrotondato per eccesso.
    let c = run(Estimate::Exact(GIB), &emptied(900 * MIB as i64), ExplainFacts::default());
    assert_eq!(c.difference, Some(-124 * MIB as i64));
    assert_eq!(c.basis_points, Some(1211));
    // Ottenuto più grande della stima: differenza positiva.
    let c = run(Estimate::Exact(GIB), &emptied(2 * GIB as i64), ExplainFacts::default());
    assert_eq!(c.difference, Some(GIB as i64));
    assert_eq!(c.basis_points, Some(10_000));
}

#[test]
fn basis_points_round_up_so_the_threshold_is_exact() {
    // Esattamente il 5%: non oltre la soglia, niente voci né "non spiegata".
    let c = run(Estimate::Exact(10_000), &emptied(9_500), ExplainFacts::default());
    assert_eq!((c.difference, c.basis_points), (Some(-500), Some(EXPLAIN_THRESHOLD_BP)));
    assert_eq!(c.unexplained, None);
    // Appena oltre il 5% (5,0001%): oltre la soglia.
    let c = run(Estimate::Exact(1_000_000), &emptied(949_999), ExplainFacts::default());
    assert_eq!(c.basis_points, Some(501));
    assert_eq!(c.unexplained, Some(-50_001));
    // Una differenza minima non si mostra come 0.
    let c = run(Estimate::Exact(GIB), &emptied(GIB as i64 - 1), ExplainFacts::default());
    assert_eq!(c.basis_points, Some(1));
}

#[test]
fn range_is_zero_inside_and_distance_from_the_nearest_end_outside() {
    let range = Estimate::Range { min: 100 * MIB, max: 200 * MIB };
    for inside in [100, 150, 200] {
        let c = run(range, &emptied((inside * MIB) as i64), ExplainFacts::default());
        assert_eq!((c.difference, c.basis_points), (Some(0), Some(0)), "{inside} MiB");
        assert!(c.explanations.is_empty() && c.unexplained.is_none());
    }
    // Sotto: distanza dal minimo, percentuale sul minimo.
    let c = run(range, &emptied(80 * MIB as i64), ExplainFacts::default());
    assert_eq!((c.difference, c.basis_points), (Some(-20 * MIB as i64), Some(2_000)));
    assert_eq!(c.unexplained, Some(-20 * MIB as i64));
    // Sopra: distanza dal massimo, percentuale sul massimo.
    let c = run(range, &emptied(230 * MIB as i64), ExplainFacts::default());
    assert_eq!((c.difference, c.basis_points), (Some(30 * MIB as i64), Some(1_500)));
    // Estremi scambiati: lo stesso intervallo.
    let swapped = Estimate::Range { min: 200 * MIB, max: 100 * MIB };
    assert_eq!(run(swapped, &emptied(230 * MIB as i64), ExplainFacts::default()).difference, Some(30 * MIB as i64));
}

#[test]
fn not_estimable_or_zero_shows_only_the_obtained() {
    let facts = ExplainFacts { skipped_or_failed: (2, GIB), still_in_bin: Some(GIB), still_open_files: 3, ..Default::default() };
    let cases = [
        Estimate::NotEstimable(NotEstimable::ExternalProcedure),
        Estimate::NotEstimable(NotEstimable::HardLinksNotCounted),
        Estimate::Exact(0),
        Estimate::Range { min: 0, max: 0 },
    ];
    for est in cases {
        let c = run(est, &emptied(-(5 * GIB as i64)), facts);
        assert_eq!(c.estimated, est);
        assert_eq!(c.obtained, -(5 * GIB as i64), "{est:?}");
        assert_eq!((c.difference, c.basis_points), (None, None), "{est:?}");
        assert!(c.explanations.is_empty(), "{est:?}");
        assert_eq!(c.unexplained, None, "{est:?}");
    }
}

#[test]
fn range_from_zero_below_has_a_difference_but_no_percentage() {
    let c = run(Estimate::Range { min: 0, max: GIB }, &emptied(-(MIB as i64)), ExplainFacts { skipped_or_failed: (1, MIB), ..Default::default() });
    assert_eq!((c.difference, c.basis_points), (Some(-(MIB as i64)), None));
    assert!(c.explanations.is_empty());
    assert_eq!(c.unexplained, None);
}

#[test]
fn fewer_than_two_measures_give_nothing() {
    let est = Estimate::Exact(GIB);
    assert_eq!(compare(&est, &[], &ExplainFacts::default()), None);
    assert_eq!(compare(&est, &[start()], &ExplainFacts::default()), None);
}

#[test]
fn obtained_is_last_minus_first_with_sign_whatever_comes_between() {
    let ms = [
        start(),
        m(MeasurePoint::AfterAction, FREE - 3 * GIB, bin(1, GIB), Some(0)),
        m(MeasurePoint::AfterEmpty, FREE + 2 * GIB, bin(0, 0), Some(0)),
        m(MeasurePoint::AfterProcedure, FREE - 7, bin(0, 0), Some(0)),
    ];
    assert_eq!(run(Estimate::NotEstimable(NotEstimable::ExternalProcedure), &ms, ExplainFacts::default()).obtained, -7);
}

// ---------------------------------------------------------------------------------------------
// Voci di spiegazione (tabella delle voci), ognuna solo con il suo fatto misurato

#[test]
fn under_the_threshold_no_voice_even_with_facts() {
    let facts = ExplainFacts { skipped_or_failed: (1, MIB), excluded_names: (2, MIB), still_in_bin: Some(MIB), still_open_files: 1, ..Default::default() };
    let c = run(Estimate::Exact(GIB), &emptied((GIB - 10 * MIB) as i64), facts);
    assert!(c.basis_points.expect("percentuale") <= EXPLAIN_THRESHOLD_BP);
    assert!(c.explanations.is_empty());
    assert_eq!(c.unexplained, None);
}

#[test]
fn no_voice_without_its_fact() {
    // Oltre la soglia, fatti tutti a zero e nessuna riscansione: solo "non spiegata".
    let ms = [start(), m(MeasurePoint::AfterEmpty, FREE + GIB / 2, None, None)];
    let facts = ExplainFacts { skipped_or_failed: (0, 0), excluded_names: (0, 0), recycled_bytes: GIB, still_in_bin: Some(0), still_open_files: 0 };
    let c = run(Estimate::Exact(GIB), &ms, facts);
    assert!(c.explanations.is_empty(), "{:?}", c.explanations);
    assert_eq!(c.unexplained, Some(-(GIB as i64) / 2));
}

#[test]
fn skipped_item_explains_what_stayed_in_place() {
    // Tre elementi da 1 GiB, uno saltato: dopo lo svuotamento il volume libera 2 GiB.
    let facts = ExplainFacts { skipped_or_failed: (1, GIB), recycled_bytes: 2 * GIB, still_in_bin: Some(0), ..Default::default() };
    let c = run(Estimate::Exact(3 * GIB), &emptied(2 * GIB as i64), facts);
    assert_eq!(c.difference, Some(-(GIB as i64)));
    assert_eq!(c.explanations, vec![Explanation::SkippedOrFailed { items: 1, bytes: GIB }]);
    assert_eq!(c.unexplained, None);
    assert_eq!(accounted(&c), c.difference.expect("differenza"));
}

#[test]
fn excluded_names_are_listed_but_do_not_move_the_difference() {
    // La stima esclude già i file con altri nomi: la voce compare, il resto resta da spiegare.
    let ms = [start(), m(MeasurePoint::AfterEmpty, FREE + GIB / 2, bin(0, 0), None)];
    let facts = ExplainFacts { excluded_names: (4, 300 * MIB), recycled_bytes: GIB, still_in_bin: Some(0), ..Default::default() };
    let c = run(Estimate::Exact(GIB), &ms, facts);
    assert_eq!(c.explanations, vec![Explanation::ExcludedNames { files: 4, bytes: 300 * MIB }]);
    assert_eq!(c.unexplained, Some(-(GIB as i64) / 2));
    assert_eq!(accounted(&c), c.difference.expect("differenza"));
}

#[test]
fn right_after_the_move_the_bin_explains_the_whole_estimate() {
    // Subito dopo lo spostamento, prima dello svuotamento, l'ottenuto è circa 0.
    let ms = [start(), m(MeasurePoint::AfterAction, FREE, bin(3, 2 * GIB), Some(0))];
    let facts = ExplainFacts { recycled_bytes: 2 * GIB, still_in_bin: Some(2 * GIB), ..Default::default() };
    let c = run(Estimate::Exact(2 * GIB), &ms, facts);
    assert_eq!(c.obtained, 0);
    assert_eq!((c.difference, c.basis_points), (Some(-2 * GIB as i64), Some(10_000)));
    assert_eq!(c.explanations, vec![Explanation::StillInBin { bytes: 2 * GIB }]);
    assert_eq!(c.unexplained, None);
    assert_eq!(accounted(&c), c.difference.expect("differenza"));
}

#[test]
fn right_after_the_move_with_other_items_already_in_the_bin() {
    // Il Cestino aveva già 5 elementi: il loro numero sale, quindi nessuna voce "altri elementi".
    let ms = [
        m(MeasurePoint::Start, FREE, bin(5, 4 * GIB), None),
        m(MeasurePoint::AfterAction, FREE - 4096, bin(6, 5 * GIB), Some(0)),
    ];
    let facts = ExplainFacts { recycled_bytes: GIB, still_in_bin: Some(GIB), ..Default::default() };
    let c = run(Estimate::Exact(GIB), &ms, facts);
    // I 4 KiB scritti nel frattempo (il file `$I`, altri processi) sono attività di altri processi:
    // la riscansione dice che la selezione non è più al suo posto.
    assert_eq!(c.explanations, vec![Explanation::StillInBin { bytes: GIB }, Explanation::OtherActivity { bytes: -4096 }]);
    assert_eq!(c.unexplained, None);
    assert_eq!(accounted(&c), c.difference.expect("differenza"));
}

#[test]
fn after_emptying_the_other_bin_items_explain_a_larger_obtained() {
    // Alla partenza il Cestino ha 3 elementi per 2 GiB; lo svuotamento elimina anche quelli.
    let ms = [
        m(MeasurePoint::Start, FREE, bin(3, 2 * GIB), None),
        m(MeasurePoint::AfterAction, FREE, bin(4, 3 * GIB), Some(0)),
        m(MeasurePoint::AfterEmpty, FREE + 3 * GIB, bin(0, 0), Some(0)),
    ];
    let facts = ExplainFacts { recycled_bytes: GIB, still_in_bin: Some(0), ..Default::default() };
    let c = run(Estimate::Exact(GIB), &ms, facts);
    assert_eq!((c.obtained, c.difference), (3 * GIB as i64, Some(2 * GIB as i64)));
    assert_eq!(c.explanations, vec![Explanation::OtherBinItems { items: 3, bytes: 2 * GIB }]);
    assert_eq!(c.unexplained, None);
    assert_eq!(accounted(&c), c.difference.expect("differenza"));
}

#[test]
fn dialog_cancelled_leaves_our_items_in_the_bin_and_no_other_items_voice() {
    // L'utente annulla il dialogo di Windows: niente è eliminato, il Cestino ha ancora tutto.
    let ms = [
        m(MeasurePoint::Start, FREE, bin(3, 2 * GIB), None),
        m(MeasurePoint::AfterEmpty, FREE, bin(4, 3 * GIB), Some(0)),
    ];
    let facts = ExplainFacts { recycled_bytes: GIB, still_in_bin: Some(GIB), ..Default::default() };
    let c = run(Estimate::Exact(GIB), &ms, facts);
    assert_eq!(c.explanations, vec![Explanation::StillInBin { bytes: GIB }]);
    assert_eq!(c.unexplained, None);
}

#[test]
fn open_files_are_listed_and_keep_a_smaller_obtained_unexplained() {
    // Due file erano ancora aperti allo svuotamento: lo spazio può non essere tornato. Anche con la
    // riscansione che non trova più la selezione, il resto negativo non va ad altri processi.
    let facts = ExplainFacts { recycled_bytes: GIB, still_in_bin: Some(0), still_open_files: 2, ..Default::default() };
    let c = run(Estimate::Exact(GIB), &rescanned(GIB as i64 / 2), facts);
    assert_eq!(c.explanations, vec![Explanation::StillOpen { files: 2 }]);
    assert_eq!(c.unexplained, Some(-(GIB as i64) / 2));
    // Con un ottenuto più grande i file aperti non c'entrano: il resto è di altri processi.
    let c = run(Estimate::Exact(GIB), &rescanned(2 * GIB as i64), facts);
    assert_eq!(c.explanations, vec![Explanation::StillOpen { files: 2 }, Explanation::OtherActivity { bytes: GIB as i64 }]);
    assert_eq!(c.unexplained, None);
}

#[test]
fn concurrent_activity_with_rescan_is_declared_not_attributed() {
    // Un processo scrive 300 MiB durante la verifica; la riscansione dice che la selezione è
    // sparita. La differenza è dichiarata come attività di altri processi, non come non spiegata.
    let facts = ExplainFacts { recycled_bytes: GIB, still_in_bin: Some(0), ..Default::default() };
    let c = run(Estimate::Exact(GIB), &rescanned((GIB - 300 * MIB) as i64), facts);
    assert_eq!(c.difference, Some(-300 * MIB as i64));
    assert_eq!(c.explanations, vec![Explanation::OtherActivity { bytes: -300 * MIB as i64 }]);
    assert_eq!(c.unexplained, None);
    // Lo stesso con un processo che elimina: attività positiva.
    let c = run(Estimate::Exact(GIB), &rescanned((GIB + 300 * MIB) as i64), facts);
    assert_eq!(c.explanations, vec![Explanation::OtherActivity { bytes: 300 * MIB as i64 }]);
}

#[test]
fn without_rescan_the_difference_is_unexplained() {
    let ms = [start(), m(MeasurePoint::AfterEmpty, FREE + GIB - 300 * MIB, bin(0, 0), None)];
    let facts = ExplainFacts { recycled_bytes: GIB, still_in_bin: Some(0), ..Default::default() };
    let c = run(Estimate::Exact(GIB), &ms, facts);
    assert!(c.explanations.is_empty());
    assert_eq!(c.unexplained, Some(-300 * MIB as i64));
    // Riscansione che trova ancora parte della selezione al suo posto: nemmeno lì si attribuisce.
    let ms = [start(), m(MeasurePoint::AfterEmpty, FREE + GIB - 300 * MIB, bin(0, 0), Some(300 * MIB))];
    let c = run(Estimate::Exact(GIB), &ms, facts);
    assert!(c.explanations.is_empty());
    assert_eq!(c.unexplained, Some(-300 * MIB as i64));
}

#[test]
fn voices_together_leave_only_a_small_rest() {
    // Saltato 1 GiB, 512 MiB ancora nel Cestino, 2 GiB di altri elementi eliminati; 1 MiB di rumore
    // senza riscansione: sotto la soglia, quindi niente "non spiegata".
    let ms = [
        m(MeasurePoint::Start, FREE, bin(2, 2 * GIB), None),
        m(MeasurePoint::AfterEmpty, FREE + 2 * GIB + 2 * GIB + 512 * MIB - MIB, bin(1, 512 * MIB), None),
    ];
    let facts = ExplainFacts {
        skipped_or_failed: (1, GIB),
        excluded_names: (1, 10 * MIB),
        recycled_bytes: 3 * GIB,
        still_in_bin: Some(512 * MIB),
        still_open_files: 0,
    };
    let c = run(Estimate::Exact(4 * GIB), &ms, facts);
    assert_eq!(
        c.explanations,
        vec![
            Explanation::SkippedOrFailed { items: 1, bytes: GIB },
            Explanation::ExcludedNames { files: 1, bytes: 10 * MIB },
            Explanation::StillInBin { bytes: 512 * MIB },
            Explanation::OtherBinItems { items: 1, bytes: 2 * GIB },
        ]
    );
    assert_eq!(c.unexplained, None);
}

#[test]
fn range_outside_explained_by_the_bin() {
    // Intervallo [1, 2] GiB subito dopo lo spostamento: l'ottenuto 0 è sotto il minimo; con 1,5 GiB
    // ancora nel Cestino l'ottenuto corretto cade nell'intervallo.
    let ms = [start(), m(MeasurePoint::AfterAction, FREE, bin(2, 2 * GIB), Some(0))];
    let facts = ExplainFacts { recycled_bytes: 3 * GIB / 2, still_in_bin: Some(3 * GIB / 2), ..Default::default() };
    let c = run(Estimate::Range { min: GIB, max: 2 * GIB }, &ms, facts);
    assert_eq!((c.difference, c.basis_points), (Some(-(GIB as i64)), Some(10_000)));
    assert_eq!(c.explanations, vec![Explanation::StillInBin { bytes: 3 * GIB / 2 }]);
    assert_eq!(c.unexplained, None);
}

#[test]
fn range_outside_without_rescan_is_unexplained_from_the_nearest_end() {
    let ms = [start(), m(MeasurePoint::AfterEmpty, FREE + 3 * GIB, bin(0, 0), None)];
    let c = run(Estimate::Range { min: GIB, max: 2 * GIB }, &ms, ExplainFacts::default());
    assert_eq!((c.difference, c.basis_points), (Some(GIB as i64), Some(5_000)));
    assert_eq!(c.unexplained, Some(GIB as i64));
}

// ---------------------------------------------------------------------------------------------
// Tolleranza: 1% della stima più 1 MiB

#[test]
fn tolerance_is_one_percent_plus_one_mib() {
    let e = 10 * GIB;
    let tol = (e / 100 + TOLERANCE_FIXED) as i64;
    assert!(within_tolerance(e, e as i64));
    assert!(within_tolerance(e, e as i64 + tol));
    assert!(within_tolerance(e, e as i64 - tol));
    assert!(!within_tolerance(e, e as i64 + tol + 1));
    assert!(!within_tolerance(e, e as i64 - tol - 1));
    // Con stima 0 resta solo la parte fissa (residenti nella MFT, disco virtuale, subito dopo lo
    // spostamento).
    assert!(within_tolerance(0, MIB as i64));
    assert!(within_tolerance(0, -(MIB as i64)));
    assert!(!within_tolerance(0, MIB as i64 + 1));
    // Senza arrotondamenti: 1% di 150 byte è 1,5 byte.
    assert!(within_tolerance(150, 150 + MIB as i64 + 1));
    assert!(!within_tolerance(150, 150 + MIB as i64 + 2));
    // Valori estremi senza overflow.
    assert!(!within_tolerance(u64::MAX, i64::MIN));
    assert!(!within_tolerance(u64::MAX, i64::MAX));
    assert!(within_tolerance(i64::MAX as u64, i64::MAX));
}
