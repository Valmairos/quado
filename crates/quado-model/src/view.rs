// SPDX-License-Identifier: GPL-3.0-or-later
//! Indici per l'interfaccia: figli ordinati per dimensione, righe della tabella ad albero, elementi più
//! grandi senza sovrapposizioni, file più grandi, ricerca e filtri.
//!
//! Le dimensioni sono sempre quelle allocate ("Dimensioni su disco"). I nodi staccati da una
//! riscansione e i nomi secondari degli hard link non compaiono negli elenchi di file.

use std::cmp::Ordering;

use rayon::prelude::*;

use crate::kind::Family;
use crate::names::Name;
use crate::tree::{Flags, NONE, Tree};

/// Ordine di presentazione dei figli: allocata decrescente, poi nome senza distinzione di maiuscole.
fn by_size_then_name(tree: &Tree, a: u32, b: u32) -> Ordering {
    tree.alloc(b).cmp(&tree.alloc(a)).then_with(|| cmp_names(tree.name(a), tree.name(b))).then(a.cmp(&b))
}

/// Confronto dei nomi come in un elenco: senza distinzione di maiuscole per i caratteri ASCII.
pub fn cmp_names(a: Name<'_>, b: Name<'_>) -> Ordering {
    match (a, b) {
        (Name::Ascii(x), Name::Ascii(y)) => {
            x.iter().map(u8::to_ascii_lowercase).cmp(y.iter().map(u8::to_ascii_lowercase)).then_with(|| x.cmp(y))
        }
        _ => {
            let low = |n: Name<'_>| n.to_utf16().into_iter().map(|u| if u < 0x80 { u16::from((u as u8).to_ascii_lowercase()) } else { u });
            low(a).cmp(low(b))
        }
    }
}

/// Figli di ogni cartella in ordine di presentazione, in un unico array (CSR).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ordered {
    start: Vec<u32>,
    kids: Vec<u32>,
}

impl Ordered {
    pub fn build(tree: &Tree) -> Self {
        let n = tree.len();
        let mut count = vec![0u32; n + 1];
        for i in 1..n as u32 {
            if tree.flags(i).has(Flags::DETACHED) {
                continue;
            }
            let p = tree.parent(i);
            if p != NONE {
                count[p as usize + 1] += 1;
            }
        }
        for i in 0..n {
            count[i + 1] += count[i];
        }
        let start = count.clone();
        let mut fill = count;
        let mut kids = vec![0u32; start[n] as usize];
        for i in 1..n as u32 {
            if tree.flags(i).has(Flags::DETACHED) {
                continue;
            }
            let p = tree.parent(i);
            if p != NONE {
                let slot = &mut fill[p as usize];
                kids[*slot as usize] = i;
                *slot += 1;
            }
        }
        // Una fetta per cartella, ordinate in parallelo.
        let mut slices: Vec<&mut [u32]> = Vec::new();
        let mut rest: &mut [u32] = &mut kids;
        for i in 0..n {
            let len = (start[i + 1] - start[i]) as usize;
            let (head, tail) = std::mem::take(&mut rest).split_at_mut(len);
            if len > 1 {
                slices.push(head);
            }
            rest = tail;
        }
        slices.par_iter_mut().for_each(|s| s.sort_unstable_by(|&a, &b| by_size_then_name(tree, a, b)));
        Self { start, kids }
    }

    /// Come [`Ordered::build`] dopo una modifica del modello, rifacendo solo le cartelle toccate: con 3,6
    /// milioni di nodi la costruzione completa costa 220-260 ms, questa una copia degli elenchi invariati.
    ///
    /// `self` è l'indice di prima, costruito sull'albero di `old_len` nodi (quelli da `old_len` in poi sono
    /// nuovi). `changed` elenca i nodi le cui dimensioni o il cui posto sono cambiati: file aggiornati
    /// (`Tree::update_file`), cartelle misurate (`Tree::set_measured`), radici staccate (`Tree::detach`,
    /// anche dentro `Tree::graft`) e radici innestate. Si rifanno gli elenchi dei loro antenati (figli
    /// cambiati o con dimensioni nuove), dei nodi staccati che avevano figli (ora vuoti) e dei nodi nuovi;
    /// gli altri si copiano. Il risultato è identico a `Ordered::build(tree)`.
    pub fn update(&self, tree: &Tree, old_len: usize, changed: &[u32]) -> Self {
        let n = tree.len();
        if self.start.len() != old_len + 1 || old_len > n {
            return Self::build(tree);
        }
        // Cartelle di prima da rifare.
        let mut dirty = NodeSet::default();
        let mut list: Vec<u32> = Vec::new();
        for &c in changed {
            if (c as usize) < old_len && dirty.insert(c) {
                list.push(c);
            }
            let mut p = tree.parent(c);
            while p != NONE && (p as usize) < old_len && dirty.insert(p) {
                list.push(p);
                p = tree.parent(p);
            }
            // Un sottoalbero staccato perde i suoi elenchi: la costruzione completa non ne dà.
            if (c as usize) < old_len && tree.flags(c).has(Flags::DETACHED) {
                let mut stack = vec![c];
                while let Some(x) = stack.pop() {
                    let kids = self.children(x);
                    if !kids.is_empty() {
                        if dirty.insert(x) {
                            list.push(x);
                        }
                        stack.extend_from_slice(kids);
                    }
                }
            }
        }
        list.sort_unstable();
        let mut start: Vec<u32> = Vec::with_capacity(n + 1);
        let mut kids: Vec<u32> = Vec::with_capacity(self.kids.len() + (n - old_len));
        // Elenchi rifatti, da ordinare: intervalli crescenti e disgiunti di `kids`.
        let mut fresh: Vec<std::ops::Range<usize>> = Vec::new();
        let copy = |start: &mut Vec<u32>, kids: &mut Vec<u32>, a: usize, b: usize| {
            if a >= b {
                return;
            }
            let (from, to) = (self.start[a], self.start[b]);
            let base = kids.len() as u32;
            start.extend(self.start[a..b].iter().map(|&s| s - from + base));
            kids.extend_from_slice(&self.kids[from as usize..to as usize]);
        };
        let redo = |start: &mut Vec<u32>, kids: &mut Vec<u32>, fresh: &mut Vec<std::ops::Range<usize>>, i: u32| {
            let s = kids.len();
            start.push(s as u32);
            if tree.flags(i).has(Flags::DETACHED) {
                return;
            }
            kids.extend(tree.children(i).filter(|&k| !tree.flags(k).has(Flags::DETACHED)));
            if kids.len() - s > 1 {
                fresh.push(s..kids.len());
            }
        };
        let mut next = 0usize;
        for &d in &list {
            copy(&mut start, &mut kids, next, d as usize);
            redo(&mut start, &mut kids, &mut fresh, d);
            next = d as usize + 1;
        }
        copy(&mut start, &mut kids, next, old_len);
        for i in old_len..n {
            redo(&mut start, &mut kids, &mut fresh, i as u32);
        }
        start.push(kids.len() as u32);
        let mut slices: Vec<&mut [u32]> = Vec::with_capacity(fresh.len());
        let mut rest: &mut [u32] = &mut kids;
        let mut at = 0usize;
        for r in &fresh {
            let (_, tail) = std::mem::take(&mut rest).split_at_mut(r.start - at);
            let (head, tail) = tail.split_at_mut(r.len());
            slices.push(head);
            rest = tail;
            at = r.end;
        }
        slices.par_iter_mut().for_each(|s| s.sort_unstable_by(|&a, &b| by_size_then_name(tree, a, b)));
        Self { start, kids }
    }

    /// Nodi elencati in tutto l'indice (tutti quelli attaccati tranne la radice).
    pub fn listed(&self) -> usize {
        self.kids.len()
    }

    pub fn children(&self, i: u32) -> &[u32] {
        let i = i as usize;
        if i + 1 >= self.start.len() {
            return &[];
        }
        &self.kids[self.start[i] as usize..self.start[i + 1] as usize]
    }

    pub fn has_children(&self, i: u32) -> bool {
        !self.children(i).is_empty()
    }

    pub fn heap_bytes(&self) -> usize {
        (self.start.capacity() + self.kids.capacity()) * 4
    }
}

/// Insieme di nodi come mappa di bit. La tabella con tutto il volume espanso chiede "è espansa?" per ogni
/// riga (3,6 milioni) a ogni ricostruzione: con un `HashSet` quella domanda costava più del resto.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeSet {
    words: Vec<u64>,
}

impl NodeSet {
    pub fn contains(&self, n: u32) -> bool {
        self.words.get((n >> 6) as usize).is_some_and(|w| (w >> (n & 63)) & 1 == 1)
    }

    /// Aggiunge `n`; vero se non c'era.
    pub fn insert(&mut self, n: u32) -> bool {
        let i = (n >> 6) as usize;
        if i >= self.words.len() {
            self.words.resize(i + 1, 0);
        }
        let bit = 1u64 << (n & 63);
        let new = self.words[i] & bit == 0;
        self.words[i] |= bit;
        new
    }

    pub fn remove(&mut self, n: u32) {
        if let Some(w) = self.words.get_mut((n >> 6) as usize) {
            *w &= !(1u64 << (n & 63));
        }
    }
}

/// Righe della tabella ad albero: i figli di `root` e, sotto ogni cartella espansa, i suoi.
/// Le cartelle espanse si ricordano anche quando un antenato si chiude, come in Esplora file.
#[derive(Clone, Debug, Default)]
pub struct Outline {
    pub root: u32,
    rows: Vec<u32>,
    depth: Vec<u16>,
    expanded: NodeSet,
}

/// Pezzo di righe scritto da un thread: righe, profondità e, con "espandi tutto", le cartelle aperte.
#[derive(Default)]
struct Rows {
    rows: Vec<u32>,
    depth: Vec<u16>,
    opened: Vec<u32>,
}

/// Da quanti file nel sottoalbero i figli di una cartella aperta si scrivono in parallelo: con tutto C:
/// espanso (3,6 milioni di righe) un thread solo impiegava decine di millisecondi.
const PAR_FILES: u32 = 1 << 14;
/// Figli consecutivi scritti dallo stesso thread.
const PAR_GROUP: usize = 64;

impl Outline {
    pub fn new(ord: &Ordered, tree: &Tree, root: u32) -> Self {
        let mut o = Self { root, rows: Vec::new(), depth: Vec::new(), expanded: NodeSet::default() };
        o.rebuild(ord, tree);
        o
    }

    /// Ricostruisce le righe da zero (dopo una riscansione o un cambio di radice).
    pub fn rebuild(&mut self, ord: &Ordered, tree: &Tree) {
        self.fill(ord, tree, false);
    }

    /// Righe sotto la radice, scritte in parallelo per i sottoalberi grandi e copiate una volta sola.
    /// `all`: ogni cartella con figli si apre e si aggiunge alle espanse (`expand_all`).
    fn fill(&mut self, ord: &Ordered, tree: &Tree, all: bool) {
        let parts = self.chunks(ord, tree, self.root, 0, all);
        let total: usize = parts.iter().map(|p| p.rows.len()).sum();
        // Una copia sola, in parallelo, ogni pezzo al suo posto.
        let mut rows = vec![0u32; total];
        let mut depth = vec![0u16; total];
        let mut jobs = Vec::with_capacity(parts.len());
        let (mut r, mut d): (&mut [u32], &mut [u16]) = (&mut rows, &mut depth);
        for p in &parts {
            let (a, rest) = std::mem::take(&mut r).split_at_mut(p.rows.len());
            let (b, rest_d) = std::mem::take(&mut d).split_at_mut(p.rows.len());
            jobs.push((a, b, p));
            (r, d) = (rest, rest_d);
        }
        jobs.into_par_iter().for_each(|(a, b, p)| {
            a.copy_from_slice(&p.rows);
            b.copy_from_slice(&p.depth);
        });
        self.rows = rows;
        self.depth = depth;
        for p in &parts {
            for &k in &p.opened {
                self.expanded.insert(k);
            }
        }
    }

    /// Vero se `k` si apre: in "espandi tutto" ogni cartella con figli, altrimenti le espanse.
    fn opens(&self, ord: &Ordered, tree: &Tree, k: u32, all: bool) -> bool {
        if all { tree.is_dir(k) && ord.has_children(k) } else { self.expanded.contains(k) }
    }

    /// Righe dei figli di `node` (già aperto) in pezzi ordinati.
    fn chunks(&self, ord: &Ordered, tree: &Tree, node: u32, depth: u16, all: bool) -> Vec<Rows> {
        if tree.files(node) < PAR_FILES {
            let mut r = Rows::default();
            self.emit_rows(ord, tree, node, depth, all, &mut r);
            return vec![r];
        }
        // Pezzi di lavoro: ogni figlio grande da solo (il suo sottoalbero si divide di nuovo), i figli piccoli
        // consecutivi a gruppi. Con gruppi fissi i pochi figli grandi della radice (Users, Windows) finivano
        // nello stesso thread.
        let kids = ord.children(node);
        let mut groups: Vec<&[u32]> = Vec::new();
        let mut from = 0;
        for (i, &k) in kids.iter().enumerate() {
            if tree.files(k) >= PAR_FILES {
                if from < i {
                    groups.push(&kids[from..i]);
                }
                groups.push(&kids[i..=i]);
                from = i + 1;
            } else if i + 1 - from == PAR_GROUP {
                groups.push(&kids[from..=i]);
                from = i + 1;
            }
        }
        if from < kids.len() {
            groups.push(&kids[from..]);
        }
        let groups: Vec<Vec<Rows>> = groups
            .into_par_iter()
            .map(|group| {
                let mut out = vec![Rows::default()];
                for &k in group {
                    let cur = out.last_mut().expect("pezzo corrente");
                    cur.rows.push(k);
                    cur.depth.push(depth);
                    if !self.opens(ord, tree, k, all) {
                        continue;
                    }
                    if all {
                        cur.opened.push(k);
                    }
                    if tree.files(k) >= PAR_FILES {
                        out.extend(self.chunks(ord, tree, k, depth + 1, all));
                        out.push(Rows::default());
                    } else {
                        self.emit_rows(ord, tree, k, depth + 1, all, cur);
                    }
                }
                out
            })
            .collect();
        groups.into_iter().flatten().filter(|r| !r.rows.is_empty()).collect()
    }

    /// Come `emit`, in un thread solo, con "espandi tutto".
    fn emit_rows(&self, ord: &Ordered, tree: &Tree, node: u32, depth: u16, all: bool, out: &mut Rows) {
        let mut stack: Vec<(u32, u16)> = ord.children(node).iter().rev().map(|&k| (k, depth)).collect();
        while let Some((k, d)) = stack.pop() {
            out.rows.push(k);
            out.depth.push(d);
            if self.opens(ord, tree, k, all) {
                if all {
                    out.opened.push(k);
                }
                stack.extend(ord.children(k).iter().rev().map(|&c| (c, d + 1)));
            }
        }
    }

    fn emit(&self, ord: &Ordered, node: u32, depth: u16, rows: &mut Vec<u32>, depths: &mut Vec<u16>) {
        // Iterativo: una cartella con un milione di figli o un albero profondo non consumano lo stack.
        let mut stack: Vec<(u32, u16)> = ord.children(node).iter().rev().map(|&k| (k, depth)).collect();
        while let Some((k, d)) = stack.pop() {
            rows.push(k);
            depths.push(d);
            if self.expanded.contains(k) {
                stack.extend(ord.children(k).iter().rev().map(|&c| (c, d + 1)));
            }
        }
    }

    pub fn set_root(&mut self, ord: &Ordered, tree: &Tree, root: u32) {
        self.root = root;
        self.rebuild(ord, tree);
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn row(&self, r: usize) -> u32 {
        self.rows[r]
    }

    pub fn depth(&self, r: usize) -> u16 {
        self.depth[r]
    }

    pub fn row_of(&self, node: u32) -> Option<usize> {
        self.rows.iter().position(|&n| n == node)
    }

    pub fn is_expanded(&self, node: u32) -> bool {
        self.expanded.contains(node)
    }

    /// Espande o chiude la cartella alla riga `r`.
    pub fn set_expanded(&mut self, ord: &Ordered, r: usize, open: bool) {
        let node = self.rows[r];
        if open == self.expanded.contains(node) || !ord.has_children(node) {
            return;
        }
        let d = self.depth[r];
        if open {
            self.expanded.insert(node);
            let mut rows = Vec::new();
            let mut depths = Vec::new();
            self.emit(ord, node, d + 1, &mut rows, &mut depths);
            self.rows.splice(r + 1..r + 1, rows);
            self.depth.splice(r + 1..r + 1, depths);
        } else {
            self.expanded.remove(node);
            let end = (r + 1..self.rows.len()).find(|&k| self.depth[k] <= d).unwrap_or(self.rows.len());
            self.rows.drain(r + 1..end);
            self.depth.drain(r + 1..end);
        }
    }

    /// Espande tutte le cartelle sotto la radice (`*` del tastierino, come nell'albero di Esplora file).
    /// Un passaggio solo, in parallelo sui sottoalberi grandi: righe e cartelle espanse insieme.
    pub fn expand_all(&mut self, ord: &Ordered, tree: &Tree) {
        self.fill(ord, tree, true);
    }

    /// Espande gli antenati di `node` sotto la radice, così che diventi una riga; restituisce la riga.
    pub fn reveal(&mut self, ord: &Ordered, tree: &Tree, node: u32) -> Option<usize> {
        let mut chain = Vec::new();
        let mut n = tree.parent(node);
        while n != NONE && n != self.root {
            chain.push(n);
            n = tree.parent(n);
        }
        if n != self.root {
            return None;
        }
        let mut changed = false;
        for a in chain {
            changed |= self.expanded.insert(a);
        }
        if changed {
            self.rebuild(ord, tree);
        }
        self.row_of(node)
    }
}

/// `true` se `a` è un antenato di `b` (o lo stesso nodo).
pub fn is_ancestor(tree: &Tree, a: u32, b: u32) -> bool {
    let mut n = b;
    while n != NONE {
        if n == a {
            return true;
        }
        n = tree.parent(n);
    }
    false
}

/// "Dove finisce lo spazio" senza regole di classificazione: al massimo `n` elementi
/// sotto `root`, file o cartelle, mai un elemento insieme a un suo antenato.
///
/// Si parte dai figli di `root`. Una cartella dell'elenco si sostituisce con il suo contenuto quando,
/// dopo la sostituzione, i suoi figli che entrano nei primi `n` coprono almeno metà dei suoi byte.
/// Così una cartella che è quasi tutta una sottocartella lascia il posto a quella, mentre una cartella
/// con molte cose medie (Download con cento installatori) resta un elemento solo.
/// Con le regole di classificazione si usa [`top_elements_with`]: `closed` dice quali elementi non si
/// aprono mai (un gioco di Steam, la cache di npm, il Cestino), mentre ciò che nessuna regola riconosce
/// segue la regola strutturale.
pub fn top_elements(tree: &Tree, ord: &Ordered, root: u32, n: usize) -> Vec<u32> {
    top_elements_with(tree, ord, root, n, &|_| false)
}

pub fn top_elements_with(tree: &Tree, ord: &Ordered, root: u32, n: usize, closed: &dyn Fn(u32) -> bool) -> Vec<u32> {
    if n == 0 {
        return Vec::new();
    }
    let order = |a: u32, b: u32| by_size_then_name(tree, a, b);
    let mut frontier: Vec<u32> = ord.children(root).to_vec();
    frontier.sort_unstable_by(|&a, &b| order(a, b));
    // Byte dei figli di `f` che entrerebbero nei primi `n` se `f` lasciasse il posto al suo contenuto.
    let kept_by_opening = |frontier: &[u32], f: u32| -> u64 {
        let kids = ord.children(f);
        let others = frontier.iter().copied().filter(|&x| x != f);
        let mut others = others.peekable();
        let mut kids = kids.iter().copied().peekable();
        let mut bytes = 0;
        for _ in 0..n {
            let take_kid = match (others.peek(), kids.peek()) {
                (Some(&o), Some(&k)) => order(k, o) == Ordering::Less,
                (None, Some(_)) => true,
                _ => false,
            };
            if take_kid {
                bytes += tree.alloc(kids.next().unwrap_or(NONE));
            } else if others.next().is_none() {
                break;
            }
        }
        bytes
    };
    loop {
        let opened = frontier.iter().take(n).position(|&f| {
            if !ord.has_children(f) || tree.alloc(f) == 0 || closed(f) {
                return false;
            }
            let kept = kept_by_opening(&frontier, f);
            kept > 0 && kept * 2 >= tree.alloc(f)
        });
        let Some(k) = opened else { break };
        let f = frontier.remove(k);
        frontier.extend_from_slice(ord.children(f));
        frontier.sort_unstable_by(|&a, &b| order(a, b));
    }
    frontier.truncate(n);
    frontier.retain(|&e| tree.alloc(e) > 0);
    frontier
}

/// Filtri della ricerca e della vista File grandi. `None` = nessun vincolo.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// Allocata minima in byte.
    pub min_alloc: Option<u64>,
    /// Solo file di questa famiglia secondo l'estensione (esclude le cartelle).
    pub family: Option<Family>,
    /// Ultima modifica uguale o successiva (`FILETIME`).
    pub modified_after: Option<i64>,
    /// Ultima modifica precedente (`FILETIME`).
    pub modified_before: Option<i64>,
    /// Solo elementi di questo gruppo di colore (gestore): il codice lo dà chi chiama.
    pub group: Option<u8>,
}

/// Gruppo di colore di un nodo, dato dalla classificazione (che il modello non conosce).
pub type GroupOf<'a> = Option<&'a (dyn Fn(u32) -> u8 + Sync)>;

impl Filter {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn accepts(&self, tree: &Tree, i: u32) -> bool {
        if self.min_alloc.is_some_and(|m| tree.alloc(i) < m) {
            return false;
        }
        if let Some(f) = self.family
            && (tree.is_dir(i) || Family::of_name(tree.name(i)) != f)
        {
            return false;
        }
        let t = tree.mtime(i);
        if self.modified_after.is_some_and(|a| t < a) || self.modified_before.is_some_and(|b| t >= b) {
            return false;
        }
        true
    }

    /// Come `accepts`, con il filtro per gestore.
    pub fn accepts_in(&self, tree: &Tree, i: u32, group_of: GroupOf<'_>) -> bool {
        if let (Some(g), Some(f)) = (self.group, group_of)
            && f(i) != g
        {
            return false;
        }
        self.accepts(tree, i)
    }
}

/// Nodi che possono comparire in un elenco: non staccati (`detach` marca tutto il sottoalbero),
/// non nomi secondari di un hard link.
fn listable(tree: &Tree, i: u32) -> bool {
    let f = tree.flags(i);
    !f.has(Flags::DETACHED) && !f.has(Flags::SECONDARY_LINK)
}

/// Confronto per gli elenchi di file: allocata decrescente, poi percorso in unità UTF-16.
fn by_size_then_path(tree: &Tree, a: u32, b: u32) -> Ordering {
    tree.alloc(b).cmp(&tree.alloc(a)).then_with(|| tree.path(a).cmp(&tree.path(b)))
}

/// I file più grandi del volume per dimensione su disco, al massimo `limit`.
pub fn largest_files(tree: &Tree, filter: &Filter, limit: usize, group_of: GroupOf<'_>) -> Vec<u32> {
    let mut all: Vec<u32> = (1..tree.len() as u32)
        .into_par_iter()
        .filter(|&i| !tree.is_dir(i) && listable(tree, i) && filter.accepts_in(tree, i, group_of))
        .collect();
    top_by_size(tree, &mut all, limit);
    all
}

fn top_by_size(tree: &Tree, v: &mut Vec<u32>, limit: usize) {
    if v.len() > limit {
        // Prima la soglia per sola dimensione (veloce), poi l'ordine completo delle prime voci.
        v.select_nth_unstable_by(limit, |&a, &b| tree.alloc(b).cmp(&tree.alloc(a)));
        let cut = tree.alloc(v[limit]);
        // A parità di dimensione col limite, l'ordine per percorso decide chi entra: si tengono tutti i pari.
        let mut keep: Vec<u32> = v.iter().copied().filter(|&i| tree.alloc(i) >= cut).collect();
        keep.par_sort_unstable_by(|&a, &b| by_size_then_path(tree, a, b));
        keep.truncate(limit);
        *v = keep;
    } else {
        v.par_sort_unstable_by(|&a, &b| by_size_then_path(tree, a, b));
    }
}

/// Testo da cercare, già preparato: minuscole, unità UTF-16.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Needle {
    lower: Vec<u16>,
    ascii: Option<Vec<u8>>,
}

impl Needle {
    pub fn new(text: &str) -> Self {
        let lower: String = text.trim().to_lowercase();
        let ascii = lower.is_ascii().then(|| lower.as_bytes().to_vec());
        Self { lower: lower.encode_utf16().collect(), ascii }
    }

    pub fn is_empty(&self) -> bool {
        self.lower.is_empty()
    }

    pub fn matches(&self, name: Name<'_>) -> bool {
        match (name, &self.ascii) {
            (Name::Ascii(s), Some(q)) => contains_ascii_ci(s, q),
            // Un nome ASCII non può contenere caratteri non ASCII cercati.
            (Name::Ascii(_), None) => false,
            (Name::Wide(w), _) => {
                let low: Vec<u16> = String::from_utf16_lossy(w).to_lowercase().encode_utf16().collect();
                low.windows(self.lower.len()).any(|win| win == self.lower.as_slice())
            }
        }
    }
}

fn contains_ascii_ci(hay: &[u8], needle: &[u8]) -> bool {
    if needle.len() > hay.len() {
        return false;
    }
    if needle.is_empty() {
        return true;
    }
    let first = needle[0];
    (0..=hay.len() - needle.len()).any(|i| {
        hay[i].to_ascii_lowercase() == first && hay[i + 1..i + needle.len()].iter().zip(&needle[1..]).all(|(h, n)| h.to_ascii_lowercase() == *n)
    })
}

/// Cartelle e file sotto `root` il cui nome contiene il testo, con i filtri, in ordine di dimensione.
pub fn search(tree: &Tree, root: u32, needle: &Needle, filter: &Filter, group_of: GroupOf<'_>) -> Vec<u32> {
    let mut hits: Vec<u32> = (1..tree.len() as u32)
        .into_par_iter()
        .filter(|&i| listable(tree, i) && (needle.is_empty() || needle.matches(tree.name(i))) && filter.accepts_in(tree, i, group_of))
        .filter(|&i| root == Tree::ROOT || is_ancestor(tree, root, i))
        .collect();
    // Solo per dimensione (a parità, per indice): con due milioni di risultati il confronto dei nomi
    // costerebbe più della ricerca stessa (≤ 300 ms).
    hits.par_sort_unstable_by_key(|&i| (std::cmp::Reverse(tree.alloc(i)), i));
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{NodeData, w};

    fn file(alloc: u64) -> NodeData {
        NodeData { logical: alloc, alloc, ..Default::default() }
    }

    fn dir() -> NodeData {
        NodeData { flags: Flags::DIR, ..Default::default() }
    }

    /// C:\ con Users\demo (AppData\Local\CapCut\Apps..., Downloads con molti file medi), Windows, un file grande.
    fn sample() -> Tree {
        let mut t = Tree::with_root(&w("C:\\"), 0);
        let users = t.push(Tree::ROOT, &w("Users"), dir());
        let demo = t.push(users, &w("demo"), dir());
        let appdata = t.push(demo, &w("AppData"), dir());
        let local = t.push(appdata, &w("Local"), dir());
        let capcut = t.push(local, &w("CapCut"), dir());
        let apps = t.push(capcut, &w("Apps"), dir());
        for k in 0..20 {
            t.push(apps, &w(&format!("9.{k}")), file(2 << 30));
        }
        let ud = t.push(capcut, &w("User Data"), dir());
        t.push(ud, &w("cache.bin"), file(10 << 30));
        let dl = t.push(demo, &w("Downloads"), dir());
        for k in 0..100 {
            t.push(dl, &w(&format!("setup{k}.exe")), file(1 << 30));
        }
        let win = t.push(Tree::ROOT, &w("Windows"), dir());
        t.push(win, &w("explorer.exe"), file(5 << 20));
        t.push(Tree::ROOT, &w("hiberfil.sys"), file(12 << 30));
        t.push(Tree::ROOT, &w("vuoto.txt"), file(0));
        t.aggregate();
        t
    }

    fn names(t: &Tree, v: &[u32]) -> Vec<String> {
        v.iter().map(|&i| t.name_lossy(i)).collect()
    }

    #[test]
    fn children_are_sorted_by_size_then_name() {
        let t = sample();
        let o = Ordered::build(&t);
        assert_eq!(names(&t, o.children(Tree::ROOT)), ["Users", "hiberfil.sys", "Windows", "vuoto.txt"]);
        let total: usize = (0..t.len() as u32).map(|i| o.children(i).len()).sum();
        assert_eq!(total, t.len() - 1);
    }

    #[test]
    fn top_elements_never_overlap() {
        let t = sample();
        let o = Ordered::build(&t);
        let top = top_elements(&t, &o, Tree::ROOT, 10);
        assert!(top.len() <= 10);
        for &a in &top {
            for &b in &top {
                assert!(a == b || !is_ancestor(&t, a, b), "{} contiene {}", t.path_lossy(a), t.path_lossy(b));
            }
        }
        // Users e demo sono contenitori quasi interi: lasciano il posto al contenuto.
        let n = names(&t, &top);
        assert!(!n.contains(&"Users".to_owned()) && !n.contains(&"demo".to_owned()), "{n:?}");
        // Download: cento file medi, nessuno abbastanza grande da entrare da solo. Resta un elemento.
        assert!(n.contains(&"Downloads".to_owned()), "{n:?}");
        assert!(n.contains(&"hiberfil.sys".to_owned()), "{n:?}");
        // Gli elementi vuoti non compaiono.
        assert!(!n.contains(&"vuoto.txt".to_owned()));
    }

    #[test]
    fn top_elements_on_random_trees_are_an_antichain() {
        // Alberi pseudocasuali con antenati e discendenti grandi.
        let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
        let mut rnd = |m: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % m
        };
        for _ in 0..200 {
            let mut t = Tree::with_root(&w("R:\\"), 0);
            let mut dirs = vec![Tree::ROOT];
            for k in 0..60 {
                let p = dirs[rnd(dirs.len() as u64) as usize];
                if rnd(3) == 0 {
                    dirs.push(t.push(p, &w(&format!("d{k}")), dir()));
                } else {
                    t.push(p, &w(&format!("f{k}")), file(rnd(1 << 20) << rnd(12)));
                }
            }
            t.aggregate();
            let o = Ordered::build(&t);
            let top = top_elements(&t, &o, Tree::ROOT, 10);
            assert!(top.len() <= 10);
            for &a in &top {
                assert_ne!(a, Tree::ROOT);
                for &b in &top {
                    assert!(a == b || !is_ancestor(&t, a, b));
                }
            }
            // In ordine di dimensione.
            assert!(top.windows(2).all(|p| t.alloc(p[0]) >= t.alloc(p[1])));
        }
    }

    #[test]
    fn outline_expands_collapses_and_reveals() {
        let t = sample();
        let o = Ordered::build(&t);
        let mut ol = Outline::new(&o, &t, Tree::ROOT);
        assert_eq!(ol.len(), 4);
        ol.set_expanded(&o, 0, true);
        assert_eq!(t.name_lossy(ol.row(1)), "demo");
        assert_eq!(ol.depth(1), 1);
        assert_eq!(ol.len(), 5);
        ol.set_expanded(&o, 0, false);
        assert_eq!(ol.len(), 4);
        // Rivelare un file profondo espande la catena degli antenati.
        let capcut = (0..t.len() as u32).find(|&i| t.name_lossy(i) == "CapCut").expect("CapCut");
        let r = ol.reveal(&o, &t, capcut).expect("riga");
        assert_eq!(ol.row(r), capcut);
        assert_eq!(ol.depth(r), 4);
        // Chiudendo Users e riaprendolo, la catena resta espansa.
        ol.set_expanded(&o, 0, false);
        ol.set_expanded(&o, 0, true);
        assert!(ol.row_of(capcut).is_some());
        let mut all = Outline::new(&o, &t, Tree::ROOT);
        all.expand_all(&o, &t);
        assert_eq!(all.len(), t.len() - 1);
    }

    /// Generatore pseudocasuale dei test (xorshift).
    fn rng(mut seed: u64) -> impl FnMut(u64) -> u64 {
        move |m: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % m.max(1)
        }
    }

    /// Albero pseudocasuale con `n` nodi: nomi ripetuti e dimensioni uguali, così che conti anche
    /// l'ordine per nome.
    fn random_tree(rnd: &mut impl FnMut(u64) -> u64, n: usize, root: &str) -> Tree {
        let mut t = Tree::with_root(&w(root), 0);
        let mut dirs = vec![Tree::ROOT];
        for k in 0..n {
            let p = dirs[rnd(dirs.len() as u64) as usize];
            let name = format!("{}{}", if rnd(2) == 0 { "n" } else { "N" }, k % 7);
            if rnd(4) == 0 {
                dirs.push(t.push(p, &w(&name), dir()));
            } else {
                t.push(p, &w(&name), file(rnd(4) << 12));
            }
        }
        t.aggregate();
        t.seal();
        t
    }

    /// Righe attese dell'albero sotto `root`, ricorsive, con `open` che dice quali nodi si aprono.
    fn expected_rows(o: &Ordered, root: u32, open: &dyn Fn(u32) -> bool) -> Vec<(u32, u16)> {
        fn walk(o: &Ordered, n: u32, d: u16, open: &dyn Fn(u32) -> bool, out: &mut Vec<(u32, u16)>) {
            for &k in o.children(n) {
                out.push((k, d));
                if open(k) {
                    walk(o, k, d + 1, open, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(o, root, 0, open, &mut out);
        out
    }

    fn rows_of(ol: &Outline) -> Vec<(u32, u16)> {
        (0..ol.len()).map(|r| (ol.row(r), ol.depth(r))).collect()
    }

    #[test]
    fn outline_in_parallel_matches_the_recursive_order() {
        // Abbastanza file perché i sottoalberi grandi si scrivano in parallelo (`PAR_FILES`).
        let mut rnd = rng(0x5EED_1234_ABCD_0001);
        let t = random_tree(&mut rnd, 120_000, "C:\\");
        assert!(t.files(Tree::ROOT) >= 4 * PAR_FILES);
        let o = Ordered::build(&t);
        let mut ol = Outline::new(&o, &t, Tree::ROOT);
        ol.expand_all(&o, &t);
        let all = |k: u32| t.is_dir(k) && o.has_children(k);
        assert_eq!(rows_of(&ol), expected_rows(&o, Tree::ROOT, &all));
        assert!((1..t.len() as u32).all(|k| ol.is_expanded(k) == all(k)));
        // Chiuse alcune cartelle, la ricostruzione segue le espanse rimaste.
        for r in (0..ol.len()).rev().step_by(97) {
            let node = ol.row(r);
            if ol.is_expanded(node) {
                ol.set_expanded(&o, r, false);
            }
        }
        let open: Vec<bool> = (0..t.len() as u32).map(|k| ol.is_expanded(k)).collect();
        let by_set = |k: u32| open[k as usize];
        let before = rows_of(&ol);
        ol.rebuild(&o, &t);
        assert_eq!(rows_of(&ol), before);
        assert_eq!(rows_of(&ol), expected_rows(&o, Tree::ROOT, &by_set));
        // Da una sottocartella grande.
        let big = o.children(Tree::ROOT).iter().copied().find(|&k| t.files(k) >= PAR_FILES).expect("sottoalbero grande");
        ol.set_root(&o, &t, big);
        assert_eq!(rows_of(&ol), expected_rows(&o, big, &by_set));
    }

    #[test]
    fn ordered_update_equals_a_full_build() {
        // Modifiche come quelle dell'app (verifica di un file, riscansione di una cartella, helper,
        // cartella sparita), in giri successivi: l'aggiornamento dà sempre gli stessi indici della
        // costruzione completa.
        let mut rnd = rng(0x0B0B_2026_1017_0001);
        for round in 0..40 {
            let mut t = random_tree(&mut rnd, 300 + round * 20, "R:\\");
            let mut ord = Ordered::build(&t);
            for _ in 0..6 {
                let old_len = t.len();
                let alive: Vec<u32> = (1..t.len() as u32).filter(|&i| !t.flags(i).has(Flags::DETACHED)).collect();
                let mut changed = Vec::new();
                for _ in 0..1 + rnd(4) {
                    let alive: Vec<u32> = alive.iter().copied().filter(|&i| !t.flags(i).has(Flags::DETACHED)).collect();
                    let Some(&x) = alive.get(rnd(alive.len() as u64) as usize) else { break };
                    match rnd(4) {
                        0 if !t.is_dir(x) => {
                            t.update_file(x, 1, rnd(5) << 12);
                            changed.push(x);
                        }
                        1 if t.is_dir(x) && t.children(x).next().is_none() => {
                            t.set_measured(x, 10, rnd(5) << 12, 3);
                            changed.push(x);
                        }
                        2 => {
                            t.detach(x);
                            changed.push(x);
                        }
                        _ => {
                            let n = rnd(30) as usize;
                            let mut sub = random_tree(&mut rnd, n, "sub");
                            if !t.is_dir(x) {
                                sub = Tree::with_root(&w("sub"), 0);
                                sub.seal();
                            }
                            let (root, _) = t.graft(x, &sub);
                            changed.push(x);
                            changed.push(root);
                        }
                    }
                }
                let next = ord.update(&t, old_len, &changed);
                assert_eq!(next, Ordered::build(&t), "giro {round}, cambiati {changed:?}");
                ord = next;
            }
        }
    }

    #[test]
    fn ordered_update_without_changes_is_a_copy() {
        let t = sample();
        let o = Ordered::build(&t);
        assert_eq!(o.update(&t, t.len(), &[]), o);
        // Un indice che non è di quell'albero si ricostruisce da capo.
        assert_eq!(Ordered::default().update(&t, t.len(), &[]), o);
    }

    #[test]
    fn largest_files_sorted_and_filtered() {
        let t = sample();
        let all = largest_files(&t, &Filter::default(), 5, None);
        assert_eq!(all.len(), 5);
        assert_eq!(t.name_lossy(all[0]), "hiberfil.sys");
        assert!(all.windows(2).all(|p| by_size_then_path(&t, p[0], p[1]) == Ordering::Less));
        let exe = largest_files(&t, &Filter { family: Some(Family::Installers), ..Default::default() }, 1000, None);
        assert_eq!(exe.len(), 101);
        let big = largest_files(&t, &Filter { min_alloc: Some(2 << 30), ..Default::default() }, 1000, None);
        assert_eq!(big.len(), 22);
    }

    #[test]
    fn search_is_case_insensitive_and_scoped() {
        let t = sample();
        let hits = search(&t, Tree::ROOT, &Needle::new("SETUP1"), &Filter::default(), None);
        assert_eq!(hits.len(), 11);
        let capcut = (0..t.len() as u32).find(|&i| t.name_lossy(i) == "CapCut").expect("CapCut");
        let scoped = search(&t, capcut, &Needle::new("9."), &Filter::default(), None);
        assert_eq!(scoped.len(), 20);
        assert!(Needle::new("università").matches(Name::Wide(&w("Università degli studi"))));
        assert!(!Needle::new("università").matches(Name::Ascii(b"universita")));
    }
}
