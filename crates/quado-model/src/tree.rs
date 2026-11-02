// SPDX-License-Identifier: GPL-3.0-or-later
//! Albero della scansione: array paralleli con indici `u32`, nomi in un unico pool `u16`.
//!
//! - I file portano le proprie dimensioni; le cartelle, dopo [`Tree::aggregate`], i totali del loro
//!   contenuto (ogni file con più nomi contato una volta, vedi [`crate::links`]).
//! - Le dimensioni degli indici delle cartelle (`$I30`) sono metadati del volume, non contenuto:
//!   stanno a parte, in [`DirExtra`], e vanno nella voce "metadati misurati" del bilancio.
//! - I nomi restano esattamente come li restituisce Windows (UTF-16, anche con surrogati spaiati).

use crate::names::{Name, NamePool, NameRef};

pub const NONE: u32 = u32::MAX;

/// Flag di un nodo.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Flags(pub u16);

impl Flags {
    pub const DIR: Self = Self(1);
    /// Ha un reparse point.
    pub const REPARSE: Self = Self(1 << 1);
    /// Reparse con il bit "name surrogate" (junction, symlink, punto di montaggio): non seguito.
    pub const NOT_FOLLOWED: Self = Self(1 << 2);
    /// Segnaposto cloud: la dimensione logica è "nel cloud", l'allocata "su questo disco".
    pub const CLOUD: Self = Self(1 << 3);
    /// Cartella cloud non elencata per non scaricarne l'elenco (`RECALL_ON_DATA_ACCESS`).
    pub const CLOUD_NOT_LISTED: Self = Self(1 << 4);
    /// Cartella che non si è potuta aprire per accesso negato.
    pub const DENIED: Self = Self(1 << 5);
    /// Cartella non elencata per un altro errore (in uso, sparita, ...).
    pub const ERROR: Self = Self(1 << 6);
    /// Allocata corretta leggendo `FileStandardInfo` (file compresso WOF).
    pub const ALLOC_FIXED: Self = Self(1 << 7);
    /// Altro nome di un file con più nomi: le sue dimensioni sono già contate altrove.
    pub const SECONDARY_LINK: Self = Self(1 << 8);
    /// Staccato dall'albero da una riscansione: ignorato da tutto.
    pub const DETACHED: Self = Self(1 << 9);
    /// Elenco della cartella incompleto (errore a metà, scansione annullata).
    pub const PARTIAL: Self = Self(1 << 10);
    /// Ha più nomi (primario o secondario).
    pub const MULTI_LINK: Self = Self(1 << 11);
    /// Cartella misurata dall'helper come totale, senza elenco (profilo o Cestino di un altro
    /// account): dimensioni e numero di file sono quelli dell'helper, i figli non ci sono.
    pub const MEASURED: Self = Self(1 << 12);

    pub fn has(self, f: Self) -> bool {
        self.0 & f.0 == f.0
    }

    pub fn any(self, f: Self) -> bool {
        self.0 & f.0 != 0
    }
}

impl std::ops::BitOr for Flags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for Flags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Dati delle sole cartelle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirExtra {
    pub node: u32,
    /// Allocata dell'indice della cartella (`$I30`), da `FileStandardInfo` sull'handle della cartella.
    pub index_alloc: u64,
    /// Codice di errore Windows se la cartella non si è potuta elencare.
    pub error: u32,
}

/// Valori di un nodo da aggiungere.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeData {
    pub logical: u64,
    pub alloc: u64,
    /// Ultima modifica, FILETIME (intervalli di 100 ns dal 1601).
    pub mtime: i64,
    pub attrs: u32,
    pub flags: Flags,
}

#[derive(Clone, Debug, Default)]
pub struct Tree {
    parent: Vec<u32>,
    first_child: Vec<u32>,
    next_sibling: Vec<u32>,
    name_off: Vec<u32>,
    name_len: Vec<u16>,
    names: NamePool,
    logical: Vec<u64>,
    alloc: Vec<u64>,
    /// File nel sottoalbero (per un file: 1, o 0 se è un nome secondario).
    files: Vec<u32>,
    mtime: Vec<i64>,
    attrs: Vec<u32>,
    flags: Vec<Flags>,
    /// Dati delle cartelle, ordinati per nodo dopo [`Tree::seal`].
    dirs: Vec<DirExtra>,
    dirs_sorted: bool,
}

impl Tree {
    /// Albero con la sola radice. `capacity` è una stima dei nodi (per esempio dai record della MFT),
    /// usata per riservare la memoria una volta sola invece di raddoppiarla crescendo.
    pub fn with_root(root_name: &[u16], capacity: usize) -> Self {
        let mut t = Self::default();
        t.reserve(capacity);
        t.push(NONE, root_name, NodeData { flags: Flags::DIR, ..Default::default() });
        t
    }

    pub fn reserve(&mut self, nodes: usize) {
        self.parent.reserve(nodes);
        self.first_child.reserve(nodes);
        self.next_sibling.reserve(nodes);
        self.name_off.reserve(nodes);
        self.name_len.reserve(nodes);
        self.logical.reserve(nodes);
        self.alloc.reserve(nodes);
        self.files.reserve(nodes);
        self.mtime.reserve(nodes);
        self.attrs.reserve(nodes);
        self.flags.reserve(nodes);
    }

    pub const ROOT: u32 = 0;

    pub fn len(&self) -> usize {
        self.parent.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// Aggiunge un nodo come figlio di `parent` (in testa alla lista dei figli).
    pub fn push(&mut self, parent: u32, name: &[u16], d: NodeData) -> u32 {
        let i = u32::try_from(self.parent.len()).expect("più di 4 miliardi di nodi");
        let r = self.names.push(name);
        self.parent.push(parent);
        self.first_child.push(NONE);
        self.name_off.push(r.off);
        self.name_len.push(r.len);
        self.logical.push(d.logical);
        self.alloc.push(d.alloc);
        let is_dir = d.flags.has(Flags::DIR);
        self.files.push(u32::from(!is_dir));
        self.mtime.push(d.mtime);
        self.attrs.push(d.attrs);
        self.flags.push(d.flags);
        if parent == NONE {
            self.next_sibling.push(NONE);
        } else {
            let p = parent as usize;
            self.next_sibling.push(self.first_child[p]);
            self.first_child[p] = i;
        }
        i
    }

    /// Registra i dati di una cartella (indice, errore).
    pub fn set_dir_extra(&mut self, e: DirExtra) {
        self.dirs.push(e);
        self.dirs_sorted = false;
    }

    pub fn add_flags(&mut self, i: u32, f: Flags) {
        self.flags[i as usize] |= f;
    }

    pub fn set_alloc(&mut self, i: u32, alloc: u64) {
        self.alloc[i as usize] = alloc;
    }

    /// Ordina i dati delle cartelle per nodo (da chiamare a fine scansione).
    pub fn seal(&mut self) {
        if !self.dirs_sorted {
            self.dirs.sort_unstable_by_key(|d| d.node);
            self.dirs_sorted = true;
        }
    }

    pub fn dir_extra(&self, i: u32) -> Option<&DirExtra> {
        debug_assert!(self.dirs_sorted, "seal() prima di leggere i dati delle cartelle");
        self.dirs.binary_search_by_key(&i, |d| d.node).ok().map(|k| &self.dirs[k])
    }

    pub fn dir_extras(&self) -> &[DirExtra] {
        &self.dirs
    }

    pub fn parent(&self, i: u32) -> u32 {
        self.parent[i as usize]
    }

    pub fn name(&self, i: u32) -> Name<'_> {
        self.names.get(NameRef { off: self.name_off[i as usize], len: self.name_len[i as usize] })
    }

    /// Nome per la visualizzazione: i surrogati spaiati diventano U+FFFD (il nome vero resta in [`Self::name`]).
    pub fn name_lossy(&self, i: u32) -> String {
        self.name(i).to_string_lossy()
    }

    pub fn logical(&self, i: u32) -> u64 {
        self.logical[i as usize]
    }

    pub fn alloc(&self, i: u32) -> u64 {
        self.alloc[i as usize]
    }

    pub fn files(&self, i: u32) -> u32 {
        self.files[i as usize]
    }

    pub fn mtime(&self, i: u32) -> i64 {
        self.mtime[i as usize]
    }

    pub fn attrs(&self, i: u32) -> u32 {
        self.attrs[i as usize]
    }

    pub fn flags(&self, i: u32) -> Flags {
        self.flags[i as usize]
    }

    pub fn is_dir(&self, i: u32) -> bool {
        self.flags[i as usize].has(Flags::DIR)
    }

    pub fn children(&self, i: u32) -> Children<'_> {
        Children { tree: self, next: self.first_child[i as usize] }
    }

    /// Profondità (radice = 0).
    pub fn depth(&self, mut i: u32) -> usize {
        let mut d = 0;
        while self.parent[i as usize] != NONE {
            i = self.parent[i as usize];
            d += 1;
        }
        d
    }

    /// Percorso in UTF-16 dalla radice (il nome della radice è il primo componente).
    pub fn path(&self, i: u32) -> Vec<u16> {
        let mut parts = Vec::new();
        let mut n = i;
        while n != NONE {
            parts.push(n);
            n = self.parent[n as usize];
        }
        let mut out: Vec<u16> = Vec::new();
        for (k, &p) in parts.iter().rev().enumerate() {
            if k > 0 && out.last() != Some(&(b'\\' as u16)) {
                out.push(b'\\' as u16);
            }
            self.name(p).extend_into(&mut out);
        }
        out
    }

    pub fn path_lossy(&self, i: u32) -> String {
        String::from_utf16_lossy(&self.path(i))
    }

    /// Ricalcola i totali delle cartelle dal basso verso l'alto. I figli hanno sempre indici maggiori
    /// del genitore (si aggiungono dopo), quindi basta un passaggio all'indietro.
    pub fn aggregate(&mut self) {
        let n = self.len();
        for i in 0..n {
            // Una cartella misurata dall'helper non ha figli: i suoi totali restano quelli misurati.
            if self.flags[i].has(Flags::MEASURED) {
                continue;
            }
            if self.flags[i].has(Flags::DIR) {
                self.logical[i] = 0;
                self.alloc[i] = 0;
                self.files[i] = 0;
            } else {
                self.files[i] = u32::from(!self.flags[i].has(Flags::SECONDARY_LINK));
            }
        }
        for i in (1..n).rev() {
            let f = self.flags[i];
            if f.has(Flags::DETACHED) {
                continue;
            }
            let p = self.parent[i];
            if p == NONE {
                continue;
            }
            let (l, a) = if f.has(Flags::SECONDARY_LINK) { (0, 0) } else { (self.logical[i], self.alloc[i]) };
            let files = self.files[i];
            let p = p as usize;
            self.logical[p] += l;
            self.alloc[p] += a;
            self.files[p] += files;
        }
    }

    /// Stacca il sottoalbero di `i` (riscansione): toglie i suoi totali dagli antenati e lo marca.
    pub fn detach(&mut self, i: u32) {
        let p = self.parent[i as usize];
        assert_ne!(p, NONE, "la radice non si stacca");
        let (l, a, files) = if self.flags[i as usize].has(Flags::SECONDARY_LINK) {
            (0, 0, 0)
        } else {
            (self.logical[i as usize], self.alloc[i as usize], self.files[i as usize])
        };
        // Toglie dalla lista dei figli del genitore.
        let pu = p as usize;
        if self.first_child[pu] == i {
            self.first_child[pu] = self.next_sibling[i as usize];
        } else {
            let mut c = self.first_child[pu];
            while c != NONE {
                let next = self.next_sibling[c as usize];
                if next == i {
                    self.next_sibling[c as usize] = self.next_sibling[i as usize];
                    break;
                }
                c = next;
            }
        }
        let mut stack = vec![i];
        while let Some(n) = stack.pop() {
            self.flags[n as usize] |= Flags::DETACHED;
            stack.extend(self.children(n));
        }
        let mut anc = p;
        while anc != NONE {
            let au = anc as usize;
            self.logical[au] -= l;
            self.alloc[au] -= a;
            self.files[au] -= files;
            anc = self.parent[au];
        }
    }

    /// Aggiunge ai totali degli antenati di `i` quelli del nodo `i` (dopo una riscansione).
    pub fn propagate_up(&mut self, i: u32) {
        let f = self.flags[i as usize];
        if f.has(Flags::SECONDARY_LINK) {
            return;
        }
        let (l, a, files) = (self.logical[i as usize], self.alloc[i as usize], self.files[i as usize]);
        let mut anc = self.parent[i as usize];
        while anc != NONE {
            let au = anc as usize;
            self.logical[au] += l;
            self.alloc[au] += a;
            self.files[au] += files;
            anc = self.parent[au];
        }
    }

    /// Nuove dimensioni di un file già nel modello ("Rileggi dal disco"): gli antenati
    /// cambiano della sola differenza.
    pub fn update_file(&mut self, i: u32, logical: u64, alloc: u64) {
        let iu = i as usize;
        debug_assert!(!self.flags[iu].has(Flags::DIR), "update_file solo sui file");
        let counted = !self.flags[iu].has(Flags::SECONDARY_LINK) && !self.flags[iu].has(Flags::DETACHED);
        let (old_l, old_a) = (self.logical[iu], self.alloc[iu]);
        self.logical[iu] = logical;
        self.alloc[iu] = alloc;
        if !counted {
            return;
        }
        let mut anc = self.parent[iu];
        while anc != NONE {
            let au = anc as usize;
            self.logical[au] = self.logical[au] - old_l + logical;
            self.alloc[au] = self.alloc[au] - old_a + alloc;
            anc = self.parent[au];
        }
    }

    /// Totali di una cartella misurata senza elenco (helper): la cartella non ha figli nel
    /// modello e prende `logical`, `alloc` e `files`; gli antenati cambiano della sola differenza.
    pub fn set_measured(&mut self, i: u32, logical: u64, alloc: u64, files: u32) {
        let iu = i as usize;
        assert!(self.flags[iu].has(Flags::DIR) && self.first_child[iu] == NONE, "set_measured solo su una cartella senza figli");
        let counted = !self.flags[iu].has(Flags::DETACHED);
        let (old_l, old_a, old_f) = (self.logical[iu], self.alloc[iu], self.files[iu]);
        self.logical[iu] = logical;
        self.alloc[iu] = alloc;
        self.files[iu] = files;
        self.flags[iu] |= Flags::MEASURED;
        if !counted {
            return;
        }
        let mut anc = self.parent[iu];
        while anc != NONE {
            let au = anc as usize;
            self.logical[au] = self.logical[au] - old_l + logical;
            self.alloc[au] = self.alloc[au] - old_a + alloc;
            self.files[au] = self.files[au] - old_f + files;
            anc = self.parent[au];
        }
    }

    /// Sostituisce il nodo `at` con la radice di `sub`, una riscansione dello stesso percorso già
    /// aggregata ("Rileggi dal disco", riscansione dopo un intervento). Il vecchio sottoalbero resta in
    /// memoria, staccato; il nuovo si aggiunge in coda, quindi i figli continuano ad avere indici maggiori
    /// del genitore. Restituisce il nuovo indice di `at` e la corrispondenza degli indici di `sub`
    /// (`map[k]` = nuovo indice di `k`).
    ///
    /// Gli hard link si deduplicano dentro `sub`, non con il resto del volume: un file con un nome
    /// dentro e uno fuori dal sottoalbero conta due volte fino alla scansione completa successiva.
    pub fn graft(&mut self, at: u32, sub: &Tree) -> (u32, Vec<u32>) {
        let parent = self.parent[at as usize];
        assert_ne!(parent, NONE, "la radice si sostituisce con una scansione completa");
        let name = self.name(at).to_utf16();
        // La radice di una scansione non ha data né attributi propri: restano quelli letti dal genitore.
        let (old_mtime, old_attrs) = (self.mtime(at), self.attrs(at));
        self.detach(at);
        self.reserve(sub.len());
        let mut map = vec![NONE; sub.len()];
        for k in 0..sub.len() {
            let ku = k as u32;
            let (p, nm) = if k == 0 { (parent, name.clone()) } else { (map[sub.parent(ku) as usize], sub.name(ku).to_utf16()) };
            let d = NodeData {
                logical: sub.logical(ku),
                alloc: sub.alloc(ku),
                mtime: if k == 0 { old_mtime } else { sub.mtime(ku) },
                attrs: if k == 0 { old_attrs } else { sub.attrs(ku) },
                flags: sub.flags(ku),
            };
            let n = self.push(p, &nm, d);
            self.files[n as usize] = sub.files(ku);
            map[k] = n;
        }
        for e in sub.dir_extras() {
            self.set_dir_extra(DirExtra { node: map[e.node as usize], ..*e });
        }
        self.seal();
        let root = map[0];
        self.propagate_up(root);
        (root, map)
    }

    /// Totale delle allocate degli indici delle cartelle.
    pub fn dir_index_total(&self) -> u64 {
        self.dirs.iter().filter(|d| !self.flags[d.node as usize].has(Flags::DETACHED)).map(|d| d.index_alloc).sum()
    }

    /// Byte di memoria occupati dagli array (capacità, non lunghezza).
    pub fn heap_bytes(&self) -> usize {
        self.parent.capacity() * 4
            + self.first_child.capacity() * 4
            + self.next_sibling.capacity() * 4
            + self.name_off.capacity() * 4
            + self.name_len.capacity() * 2
            + self.names.capacity_bytes()
            + self.logical.capacity() * 8
            + self.alloc.capacity() * 8
            + self.files.capacity() * 4
            + self.mtime.capacity() * 8
            + self.attrs.capacity() * 4
            + self.flags.capacity() * 2
            + self.dirs.capacity() * std::mem::size_of::<DirExtra>()
    }

    /// Unità nel pool dei nomi (byte ASCII più unità UTF-16) e, di queste, quelle UTF-16.
    pub fn name_units(&self) -> (usize, usize) {
        (self.names.units(), self.names.wide_units())
    }
}

pub struct Children<'a> {
    tree: &'a Tree,
    next: u32,
}

impl Iterator for Children<'_> {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        if self.next == NONE {
            return None;
        }
        let c = self.next;
        self.next = self.tree.next_sibling[c as usize];
        Some(c)
    }
}

/// Utilità per i test: nome UTF-16 da `&str`.
pub fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(logical: u64, alloc: u64) -> NodeData {
        NodeData { logical, alloc, ..Default::default() }
    }

    fn dir() -> NodeData {
        NodeData { flags: Flags::DIR, ..Default::default() }
    }

    fn sample() -> (Tree, u32, u32, u32) {
        let mut t = Tree::with_root(&w("C:\\"), 0);
        let a = t.push(Tree::ROOT, &w("a"), dir());
        let b = t.push(a, &w("b"), dir());
        t.push(b, &w("x.bin"), file(100, 4096));
        t.push(a, &w("y.bin"), file(10, 0));
        let c = t.push(Tree::ROOT, &w("c.txt"), file(5000, 8192));
        t.aggregate();
        (t, a, b, c)
    }

    #[test]
    fn aggregates_bottom_up() {
        let (t, a, b, _) = sample();
        assert_eq!((t.logical(b), t.alloc(b), t.files(b)), (100, 4096, 1));
        assert_eq!((t.logical(a), t.alloc(a), t.files(a)), (110, 4096, 2));
        assert_eq!((t.logical(Tree::ROOT), t.alloc(Tree::ROOT), t.files(Tree::ROOT)), (5110, 12288, 3));
        assert_eq!(t.path_lossy(b), "C:\\a\\b");
        assert_eq!(t.depth(b), 2);
    }

    #[test]
    fn secondary_links_are_not_counted_twice() {
        let (mut t, a, _, c) = sample();
        // Un secondo nome dello stesso file di c.txt dentro a.
        let l = t.push(a, &w("link.txt"), file(5000, 8192));
        t.add_flags(l, Flags::SECONDARY_LINK | Flags::MULTI_LINK);
        t.add_flags(c, Flags::MULTI_LINK);
        t.aggregate();
        assert_eq!(t.alloc(Tree::ROOT), 12288);
        assert_eq!(t.files(Tree::ROOT), 3);
        // Il nome secondario mostra comunque la dimensione del file.
        assert_eq!(t.alloc(l), 8192);
    }

    #[test]
    fn detach_and_rescan_keep_totals_consistent() {
        let (mut t, a, b, _) = sample();
        t.detach(b);
        assert_eq!(t.alloc(a), 0);
        assert_eq!(t.alloc(Tree::ROOT), 8192);
        assert!(t.children(a).all(|c| c != b));
        // Riscansione: la cartella torna con un file diverso.
        let b2 = t.push(a, &w("b"), dir());
        let f = t.push(b2, &w("z.bin"), file(1, 4096));
        t.propagate_up(f);
        assert_eq!(t.alloc(b2), 4096);
        assert_eq!(t.alloc(Tree::ROOT), 12288);
        // Un'aggregazione completa dà lo stesso risultato.
        let before = (t.alloc(Tree::ROOT), t.files(Tree::ROOT));
        t.aggregate();
        assert_eq!((t.alloc(Tree::ROOT), t.files(Tree::ROOT)), before);
    }

    #[test]
    fn update_file_moves_ancestors_by_the_difference() {
        let (mut t, a, b, _) = sample();
        let x = t.children(b).next().expect("x.bin");
        t.update_file(x, 300, 8192);
        assert_eq!((t.logical(b), t.alloc(b)), (300, 8192));
        assert_eq!((t.logical(a), t.alloc(a)), (310, 8192));
        assert_eq!(t.alloc(Tree::ROOT), 16384);
        let before = (t.logical(Tree::ROOT), t.alloc(Tree::ROOT));
        t.aggregate();
        assert_eq!((t.logical(Tree::ROOT), t.alloc(Tree::ROOT)), before);
    }

    #[test]
    fn graft_replaces_a_subtree_with_its_rescan() {
        let (mut t, a, b, _) = sample();
        // Riscansione di "b": ora ha due file e una sottocartella.
        let mut sub = Tree::with_root(&w("C:\\a\\b"), 0);
        let d = sub.push(Tree::ROOT, &w("d"), dir());
        sub.push(d, &w("n.bin"), file(7, 4096));
        sub.push(Tree::ROOT, &w("x.bin"), file(100, 4096));
        sub.set_dir_extra(DirExtra { node: d, index_alloc: 4096, error: 0 });
        sub.aggregate();
        sub.seal();
        let (nb, map) = t.graft(b, &sub);
        assert_eq!(t.name_lossy(nb), "b");
        assert_eq!(t.parent(nb), a);
        assert_eq!((t.alloc(nb), t.files(nb)), (8192, 2));
        assert_eq!((t.alloc(a), t.files(a)), (8192, 3));
        assert_eq!(t.alloc(Tree::ROOT), 16384);
        assert!(t.flags(b).has(Flags::DETACHED));
        assert_eq!(t.path_lossy(map[2]), "C:\\a\\b\\d\\n.bin");
        assert_eq!(t.dir_extra(map[1]).map(|e| e.index_alloc), Some(4096));
        // Un'aggregazione completa dà gli stessi totali.
        let before = (t.alloc(Tree::ROOT), t.files(Tree::ROOT), t.alloc(a));
        t.aggregate();
        assert_eq!((t.alloc(Tree::ROOT), t.files(Tree::ROOT), t.alloc(a)), before);
    }

    #[test]
    fn measured_folder_carries_the_helper_totals() {
        let mut t = Tree::with_root(&w("C:\\"), 0);
        let users = t.push(Tree::ROOT, &w("Users"), dir());
        let other = t.push(users, &w("altro"), dir());
        t.push(users, &w("x.bin"), file(100, 4096));
        t.aggregate();
        t.set_measured(other, 5000, 8192, 3);
        assert!(t.flags(other).has(Flags::MEASURED));
        assert_eq!((t.logical(other), t.alloc(other), t.files(other)), (5000, 8192, 3));
        assert_eq!((t.alloc(users), t.files(users)), (12288, 4));
        assert_eq!((t.alloc(Tree::ROOT), t.files(Tree::ROOT)), (12288, 4));
        // Una seconda misura cambia gli antenati della sola differenza.
        t.set_measured(other, 10, 4096, 1);
        assert_eq!((t.alloc(Tree::ROOT), t.files(Tree::ROOT)), (8192, 2));
        // L'aggregazione completa tiene i totali misurati.
        t.aggregate();
        assert_eq!((t.alloc(other), t.files(other), t.alloc(Tree::ROOT), t.files(Tree::ROOT)), (4096, 1, 8192, 2));
    }

    #[test]
    fn names_with_unpaired_surrogates_stay_exact() {
        let mut t = Tree::with_root(&w("C:\\"), 0);
        let bad: Vec<u16> = vec![b'a' as u16, 0xD800, b'b' as u16];
        let n = t.push(Tree::ROOT, &bad, file(1, 1));
        assert_eq!(t.name(n).to_utf16(), bad);
        assert_eq!(t.name_lossy(n), "a\u{FFFD}b");
    }

    #[test]
    fn dir_extras_lookup_after_seal() {
        let (mut t, a, b, _) = sample();
        t.set_dir_extra(DirExtra { node: b, index_alloc: 4096, error: 0 });
        t.set_dir_extra(DirExtra { node: a, index_alloc: 8192, error: 0 });
        t.seal();
        assert_eq!(t.dir_extra(a).unwrap().index_alloc, 8192);
        assert_eq!(t.dir_index_total(), 12288);
        assert!(t.dir_extra(Tree::ROOT).is_none());
    }
}
