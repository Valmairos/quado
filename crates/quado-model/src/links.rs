// SPDX-License-Identifier: GPL-3.0-or-later
//! File con più nomi (hard link): contati una volta per volume, per identità.
//!
//! Durante la scansione ogni file registra (ID a 128 bit, nodo) in un array laterale. A fine scansione
//! si ordina, si trovano gli ID con più nomi e si sceglie quale nome porta le dimensioni: quello con
//! il percorso minore in ordine alfabetico senza distinzione di maiuscole. La scelta è deterministica e
//! dichiarata (per esempio `Windows\System32` prima di `Windows\WinSxS`). Gli altri nomi diventano
//! secondari: mostrano la dimensione del file ma non entrano nei totali. Poi l'array si libera.

use crate::tree::{Flags, Tree};

/// Coppia (ID, nodo) in 20 byte: un `u128` porterebbe l'allineamento a 16 e la coppia a 32 byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct IdEntry {
    pub id: [u32; 4],
    pub node: u32,
}

impl IdEntry {
    pub fn new(id: u128, node: u32) -> Self {
        let b = id.to_le_bytes();
        let part = |k: usize| u32::from_le_bytes([b[k], b[k + 1], b[k + 2], b[k + 3]]);
        // Ordine dal più significativo, così l'ordinamento della coppia segue quello dell'ID.
        Self { id: [part(12), part(8), part(4), part(0)], node }
    }
}

/// Risultato della deduplica, conservato per l'ispettore e per la nota della card dei conti.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Links {
    /// Nodi di tutti i gruppi, il primario per primo in ogni gruppo.
    pub nodes: Vec<u32>,
    /// (inizio, lunghezza) di ogni gruppo in `nodes`.
    pub groups: Vec<(u32, u32)>,
    /// Byte allocati dei nomi secondari: quanto si conterebbe in più senza deduplica.
    pub extra_alloc: u64,
    pub extra_logical: u64,
    /// Nomi secondari.
    pub extra_names: u64,
}

impl Links {
    pub fn group_of(&self, node: u32) -> Option<&[u32]> {
        self.groups.iter().map(|&(s, l)| &self.nodes[s as usize..(s + l) as usize]).find(|g| g.contains(&node))
    }
}

/// Confronto dei percorsi senza distinzione di maiuscole per le lettere ASCII, poi per unità UTF-16:
/// deterministico anche con nomi non validi.
fn path_key(tree: &Tree, node: u32) -> Vec<u16> {
    tree.path(node)
        .into_iter()
        .map(|u| if (u16::from(b'a')..=u16::from(b'z')).contains(&u) { u - 32 } else { u })
        .collect()
}

/// Marca i nomi secondari nell'albero. Con `inject_no_dedup` (solo per il test "errore iniettato"
/// del bilancio) i gruppi si trovano ma nessun nome diventa secondario.
///
/// Ordinamento e confronto dei percorsi girano in parallelo: con 3 milioni di voci sono la parte più
/// lunga della fine della scansione, e contano anche per l'annullamento (≤ 500 ms).
pub fn resolve(mut ids: Vec<IdEntry>, tree: &mut Tree, inject_no_dedup: bool) -> Links {
    use rayon::prelude::*;
    ids.par_sort_unstable_by_key(|a| a.id);
    // Gruppi di voci con la stessa identità: (inizio, fine) in `ids`.
    let mut ranges = Vec::new();
    let mut i = 0;
    while i < ids.len() {
        let mut j = i + 1;
        while j < ids.len() && ids[j].id == ids[i].id {
            j += 1;
        }
        if j - i > 1 {
            ranges.push((i, j));
        }
        i = j;
    }
    // Il primario è il percorso che viene prima in ordine alfabetico.
    let shared: &Tree = tree;
    let groups: Vec<Vec<u32>> = ranges
        .par_iter()
        .map(|&(i, j)| {
            let mut members: Vec<(Vec<u16>, u32)> = ids[i..j].iter().map(|e| (path_key(shared, e.node), e.node)).collect();
            members.sort();
            members.into_iter().map(|(_, n)| n).collect()
        })
        .collect();
    let mut out = Links::default();
    for g in groups {
        let start = out.nodes.len() as u32;
        for (k, &node) in g.iter().enumerate() {
            tree.add_flags(node, Flags::MULTI_LINK);
            if k > 0 {
                out.extra_alloc += tree.alloc(node);
                out.extra_logical += tree.logical(node);
                out.extra_names += 1;
                if !inject_no_dedup {
                    tree.add_flags(node, Flags::SECONDARY_LINK);
                }
            }
            out.nodes.push(node);
        }
        out.groups.push((start, g.len() as u32));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{NodeData, w};

    #[test]
    fn id_entry_is_20_bytes_and_sorts_like_the_id() {
        assert_eq!(std::mem::size_of::<IdEntry>(), 20);
        let a = IdEntry::new(1u128 << 100, 0);
        let b = IdEntry::new(5, 0);
        assert!(b.id < a.id);
    }

    #[test]
    fn primary_is_the_alphabetically_first_path() {
        let mut t = Tree::with_root(&w("C:\\"), 0);
        let win = t.push(Tree::ROOT, &w("Windows"), NodeData { flags: Flags::DIR, ..Default::default() });
        let sxs = t.push(win, &w("WinSxS"), NodeData { flags: Flags::DIR, ..Default::default() });
        let s32 = t.push(win, &w("System32"), NodeData { flags: Flags::DIR, ..Default::default() });
        let f = NodeData { logical: 100, alloc: 4096, ..Default::default() };
        let in_sxs = t.push(sxs, &w("kernel32.dll"), f);
        let in_s32 = t.push(s32, &w("kernel32.dll"), f);
        let alone = t.push(s32, &w("other.dll"), f);
        let ids = vec![IdEntry::new(42, in_sxs), IdEntry::new(42, in_s32), IdEntry::new(7, alone)];
        let links = resolve(ids, &mut t, false);
        t.aggregate();
        assert_eq!(links.groups.len(), 1);
        assert_eq!(links.group_of(in_sxs).unwrap(), &[in_s32, in_sxs]);
        assert!(t.flags(in_sxs).has(Flags::SECONDARY_LINK));
        assert!(!t.flags(in_s32).has(Flags::SECONDARY_LINK));
        assert_eq!((links.extra_alloc, links.extra_names), (4096, 1));
        assert_eq!(t.alloc(Tree::ROOT), 8192);
        assert_eq!(t.alloc(sxs), 0);
    }

    #[test]
    fn injected_error_counts_links_twice() {
        let mut t = Tree::with_root(&w("C:\\"), 0);
        let f = NodeData { logical: 1, alloc: 4096, ..Default::default() };
        let a = t.push(Tree::ROOT, &w("a"), f);
        let b = t.push(Tree::ROOT, &w("b"), f);
        let links = resolve(vec![IdEntry::new(9, a), IdEntry::new(9, b)], &mut t, true);
        t.aggregate();
        assert_eq!(links.extra_alloc, 4096);
        assert_eq!(t.alloc(Tree::ROOT), 8192);
    }
}
