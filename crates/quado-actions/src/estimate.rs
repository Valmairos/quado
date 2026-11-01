// SPDX-License-Identifier: GPL-3.0-or-later
//! Stima dello spazio recuperabile in simulazione (03 §6.3): allocate dei file i cui nomi stanno tutti
//! nella selezione, una volta per ID; file con altri nomi fuori esclusi e citati; cartelle negate che
//! rendono la stima un intervallo.
//!
//! La simulazione raccoglie per ogni elemento le allocate dei file con un solo nome e, a parte, i file con
//! più nomi raggruppati per identità su tutti gli elementi non fissi, con il numero di nomi visti in
//! ciascun elemento: così la stima si ricalcola per qualunque selezione senza rileggere niente.
//!
//! L'allocata è quella che Windows riporta (`FileStandardInfo.AllocationSize`): per i compressi, WOF e
//! sparse l'allocata, per i segnaposto cloud i soli byte locali. I file residenti nella MFT
//! (`Attrs::resident`) valgono 0 anche se Windows riporta qualche byte: la MFT non si restringe. Delle
//! cartelle conta l'allocata del loro indice (`FileStandardInfo.AllocationSize` di una cartella): lo
//! svuotamento la libera (misurato su R: il 14/10/2026: senza, un albero tipo `node_modules` di 361
//! cartelle liberava il 3% in più della stima).

use std::collections::HashMap;

use quado_model::record::Estimate;

use crate::plan::{SelectionEstimate, SimItem};

/// Identità di un file: `(volume_serial, file_id)`.
pub(crate) type LinkKey = (u64, u128);

/// Un file con più nomi (hard link) visto in uno o più elementi non fissi.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Linked {
    /// Numero di nomi del file (`NumberOfLinks`).
    pub(crate) links: u32,
    pub(crate) alloc: u64,
    /// Nomi visti per elemento: indice dell'elemento e quanti.
    pub(crate) names: Vec<(usize, u32)>,
}

/// Raccolta dei file con più nomi durante la simulazione.
#[derive(Debug, Default)]
pub(crate) struct LinkTable {
    index: HashMap<LinkKey, usize>,
    groups: Vec<Linked>,
}

impl LinkTable {
    /// Un nome del file `key` visto nell'elemento `item`.
    pub(crate) fn add(&mut self, item: usize, key: LinkKey, links: u32, alloc: u64) {
        let k = *self.index.entry(key).or_insert_with(|| {
            self.groups.push(Linked { links, alloc, names: Vec::new() });
            self.groups.len() - 1
        });
        let g = &mut self.groups[k];
        g.links = g.links.max(links);
        g.alloc = g.alloc.max(alloc);
        match g.names.iter_mut().find(|(i, _)| *i == item) {
            Some((_, n)) => *n += 1,
            None => g.names.push((item, 1)),
        }
    }

    pub(crate) fn into_groups(self) -> Vec<Linked> {
        self.groups
    }
}

/// Allocate dell'elemento `item`: i file con un solo nome più, una volta ciascuno, i file con più nomi
/// che hanno almeno un nome nell'elemento (senza deduplicare tra elementi).
pub(crate) fn item_alloc(plain: u64, linked: &[Linked], item: usize) -> u64 {
    plain + linked.iter().filter(|g| g.names.iter().any(|(i, _)| *i == item)).map(|g| g.alloc).sum::<u64>()
}

/// Quota della stima della selezione di ciascun elemento (0 per quelli non scelti): le allocate dei suoi file
/// con un solo nome e delle sue cartelle, più i file con più nomi contati nella stima, attribuiti al primo
/// elemento scelto che ne ha un nome. La somma è l'estremo inferiore della stima: i file esclusi o incerti
/// non sono di nessuno. Serve al confronto stimato contro ottenuto per elemento (riepilogo, registro).
pub(crate) fn shares(items: &[SimItem], plain: impl Fn(usize) -> u64, linked: &[Linked], selected: &[bool]) -> Vec<u64> {
    let chosen = |i: usize| selected.get(i).copied().unwrap_or(false) && items.get(i).is_some_and(|it| it.fixed.is_none());
    let mut out: Vec<u64> = (0..items.len()).map(|i| if chosen(i) { plain(i) } else { 0 }).collect();
    for g in linked {
        let names: u64 = g.names.iter().filter(|(i, _)| chosen(*i)).map(|(_, n)| u64::from(*n)).sum();
        if names == 0 || names < u64::from(g.links) {
            continue;
        }
        if let Some(&(i, _)) = g.names.iter().filter(|(i, _)| chosen(*i)).min_by_key(|(i, _)| *i) {
            out[i] += g.alloc;
        }
    }
    out
}

/// Stima della selezione (03 §6.3). Gli elementi fissi non entrano mai, anche se `selected` li sceglie;
/// un valore mancante in `selected` vale "non scelto".
pub(crate) fn estimate(items: &[SimItem], plain: impl Fn(usize) -> u64, linked: &[Linked], selected: &[bool]) -> SelectionEstimate {
    let chosen = |i: usize| selected.get(i).copied().unwrap_or(false) && items.get(i).is_some_and(|it| it.fixed.is_none());
    let (mut sum, mut logical, mut denied) = (0u64, 0u64, 0u64);
    for (i, it) in items.iter().enumerate() {
        if chosen(i) {
            sum += plain(i);
            logical += it.logical;
            denied += it.denied_dirs;
        }
    }
    let (mut min, mut max) = (sum, sum);
    let mut uncertain = false;
    let mut excluded = (0u64, 0u64);
    for g in linked {
        let names: u64 = g.names.iter().filter(|(i, _)| chosen(*i)).map(|(_, n)| u64::from(*n)).sum();
        if names == 0 {
            continue;
        }
        if names >= u64::from(g.links) {
            min += g.alloc;
            max += g.alloc;
        } else if denied > 0 {
            // Gli altri nomi potrebbero stare nelle cartelle negate: solo nel massimo.
            max += g.alloc;
            uncertain = true;
        } else {
            excluded.0 += 1;
            excluded.1 += g.alloc;
        }
    }
    // Con cartelle negate i nomi non sono conoscibili: intervallo anche se gli estremi coincidono.
    let estimate = if denied > 0 || uncertain { Estimate::Range { min, max } } else { Estimate::Exact(min) };
    SelectionEstimate { estimate, excluded_names: excluded, denied_dirs: denied, logical }
}
