// SPDX-License-Identifier: GPL-3.0-or-later
//! Pool dei nomi: i nomi solo ASCII in un pool a 8 bit, gli altri in UTF-16.
//! Tutti restano esatti, compresi quelli con surrogati spaiati.
//!
//! I pool sono a blocchi di dimensione fissa: crescono di un blocco alla volta invece di raddoppiare,
//! così una stima sbagliata dei nomi non porta mai il picco di memoria al doppio. Un nome non
//! attraversa mai due blocchi.

const CHUNK: usize = 1 << 22;
/// Bit di `len` che indica un nome in UTF-16.
const WIDE: u16 = 0x8000;

#[derive(Clone, Debug, Default)]
struct Pool<T> {
    chunks: Vec<Vec<T>>,
}

impl<T: Copy> Pool<T> {
    fn push(&mut self, s: &[T]) -> u32 {
        self.push_with(s.len(), |c| c.extend_from_slice(s))
    }

    /// Riserva `len` unità nel blocco corrente (o in uno nuovo) e lascia a `fill` il compito di scriverle.
    fn push_with(&mut self, len: usize, fill: impl FnOnce(&mut Vec<T>)) -> u32 {
        let fits = self.chunks.last().is_some_and(|c| c.len() + len <= CHUNK);
        if !fits {
            self.chunks.push(Vec::with_capacity(CHUNK.max(len)));
        }
        let k = self.chunks.len() - 1;
        let c = &mut self.chunks[k];
        let off = k * CHUNK + c.len();
        fill(c);
        debug_assert_eq!(k * CHUNK + c.len(), off + len);
        u32::try_from(off).expect("pool dei nomi oltre 4 G unità")
    }

    fn get(&self, off: u32, len: usize) -> &[T] {
        let off = off as usize;
        let c = &self.chunks[off / CHUNK];
        let p = off % CHUNK;
        &c[p..p + len]
    }

    fn units(&self) -> usize {
        self.chunks.iter().map(Vec::len).sum()
    }

    fn capacity_bytes(&self) -> usize {
        self.chunks.iter().map(Vec::capacity).sum::<usize>() * size_of::<T>()
    }
}

/// Un nome letto dal pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Name<'a> {
    Ascii(&'a [u8]),
    Wide(&'a [u16]),
}

impl Name<'_> {
    pub fn len(&self) -> usize {
        match self {
            Self::Ascii(s) => s.len(),
            Self::Wide(s) => s.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn extend_into(&self, out: &mut Vec<u16>) {
        match self {
            Self::Ascii(s) => out.extend(s.iter().map(|&b| u16::from(b))),
            Self::Wide(s) => out.extend_from_slice(s),
        }
    }

    pub fn to_utf16(&self) -> Vec<u16> {
        let mut v = Vec::with_capacity(self.len());
        self.extend_into(&mut v);
        v
    }

    /// Per la visualizzazione: i surrogati spaiati diventano U+FFFD.
    pub fn to_string_lossy(&self) -> String {
        match self {
            Self::Ascii(s) => s.iter().map(|&b| char::from(b)).collect(),
            Self::Wide(s) => String::from_utf16_lossy(s),
        }
    }
}

/// Riferimento a un nome: offset nel pool e lunghezza, con il bit [`WIDE`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NameRef {
    pub off: u32,
    pub len: u16,
}

#[derive(Clone, Debug, Default)]
pub struct NamePool {
    ascii: Pool<u8>,
    wide: Pool<u16>,
}

impl NamePool {
    pub fn push(&mut self, name: &[u16]) -> NameRef {
        let len = u16::try_from(name.len()).ok().filter(|l| l & WIDE == 0).expect("nome oltre 32767 unità");
        if name.iter().all(|&u| u < 0x80) {
            let off = self.ascii.push_with(name.len(), |c| c.extend(name.iter().map(|&u| u as u8)));
            NameRef { off, len }
        } else {
            NameRef { off: self.wide.push(name), len: len | WIDE }
        }
    }

    pub fn get(&self, r: NameRef) -> Name<'_> {
        let len = usize::from(r.len & !WIDE);
        if r.len & WIDE == 0 { Name::Ascii(self.ascii.get(r.off, len)) } else { Name::Wide(self.wide.get(r.off, len)) }
    }

    /// Unità di nome memorizzate (byte ASCII più unità UTF-16).
    pub fn units(&self) -> usize {
        self.ascii.units() + self.wide.units()
    }

    pub fn wide_units(&self) -> usize {
        self.wide.units()
    }

    pub fn capacity_bytes(&self) -> usize {
        self.ascii.capacity_bytes() + self.wide.capacity_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn ascii_and_wide_round_trip() {
        let mut p = NamePool::default();
        let a = p.push(&w("kernel32.dll"));
        let b = p.push(&w("Università"));
        let bad = vec![0x61, 0xDC00, 0x62];
        let c = p.push(&bad);
        assert!(matches!(p.get(a), Name::Ascii(_)));
        assert!(matches!(p.get(b), Name::Wide(_)));
        assert_eq!(p.get(a).to_utf16(), w("kernel32.dll"));
        assert_eq!(p.get(b).to_string_lossy(), "Università");
        assert_eq!(p.get(c).to_utf16(), bad);
    }

    #[test]
    fn names_never_straddle_chunks() {
        let mut p = NamePool::default();
        let long = vec![b'x' as u16; 200];
        let mut refs = Vec::new();
        for _ in 0..(CHUNK / 200 + 10) {
            refs.push(p.push(&long));
        }
        assert_eq!(p.ascii.chunks.len(), 2);
        for r in refs {
            assert_eq!(p.get(r).len(), 200);
        }
    }
}
