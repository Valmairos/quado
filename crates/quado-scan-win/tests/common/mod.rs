// SPDX-License-Identifier: GPL-3.0-or-later
//! Strumenti comuni ai test delle letture di contenuto: SHA-256 di riferimento scritto qui (oracolo
//! indipendente da CNG), CRC-32 e un generatore di archivi zip "stored" (senza compressione), con zip64
//! forzato, commento, byte in coda e nomi UTF-8 o CP437.
#![allow(dead_code)]

/// SHA-256 di riferimento (FIPS 180-4), lento ma indipendente dall'implementazione di Windows.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be,
        0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa,
        0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85,
        0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f,
        0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in chunk.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(v);
        }
    }
    let mut out = [0u8; 32];
    for (o, x) in out.as_chunks_mut::<4>().0.iter_mut().zip(h) {
        *o = x.to_be_bytes();
    }
    out
}

pub fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// CRC-32 degli zip (polinomio 0xEDB88320).
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

/// Contenuto pseudo-casuale riproducibile.
pub fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

/// Voce da mettere nell'archivio.
#[derive(Clone, Debug)]
pub struct ZipEntry {
    /// Nome così come va scritto (UTF-8 o byte CP437).
    pub name: Vec<u8>,
    pub data: Vec<u8>,
    /// Bit 11: nome in UTF-8.
    pub utf8: bool,
}

impl ZipEntry {
    /// File con nome in UTF-8 (bit 11 acceso solo se il nome non è ASCII, come fanno gli strumenti).
    pub fn file(name: &str, data: &[u8]) -> Self {
        Self { name: name.as_bytes().to_vec(), data: data.to_vec(), utf8: !name.is_ascii() }
    }

    /// Cartella: il nome finisce con `/`.
    pub fn dir(name: &str) -> Self {
        assert!(name.ends_with('/'));
        Self::file(name, &[])
    }

    /// Nome in byte grezzi senza il bit 11 (CP437).
    pub fn cp437(name: &[u8], data: &[u8]) -> Self {
        Self { name: name.to_vec(), data: data.to_vec(), utf8: false }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ZipOptions {
    /// zip64 forzato: dimensioni e offset a 0xFFFFFFFF con il campo extra zip64, record e localizzatore
    /// zip64, campi del record di fine directory saturi.
    pub zip64: bool,
    pub comment: Vec<u8>,
    /// Byte dopo l'archivio.
    pub trailing: Vec<u8>,
    /// Byte prima dell'archivio (come un autoestraente); gli offset restano assoluti.
    pub prefix: Vec<u8>,
    /// Senza zip64, oltre 65.535 voci: conteggio a 16 bit troncato, come fanno alcuni strumenti.
    pub wrap_count: bool,
}

/// Archivio generato e posizioni delle sue strutture.
#[derive(Clone, Debug)]
pub struct BuiltZip {
    pub bytes: Vec<u8>,
    pub cd_start: u64,
    pub cd_size: u64,
    pub eocd: u64,
}

fn put16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn put32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn put64(v: &mut Vec<u8>, x: u64) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Zip "stored" secondo APPNOTE 6.3.
pub fn build_zip(entries: &[ZipEntry], o: &ZipOptions) -> BuiltZip {
    let mut out = o.prefix.clone();
    let mut central = Vec::new();
    let version: u16 = if o.zip64 { 45 } else { 20 };
    for e in entries {
        let offset = out.len() as u64;
        let crc = crc32(&e.data);
        let size = e.data.len() as u64;
        let flags: u16 = if e.utf8 { 1 << 11 } else { 0 };
        let small = |x: u64| if o.zip64 { u32::MAX } else { u32::try_from(x).expect("serve zip64") };
        // Header locale.
        let mut local_extra = Vec::new();
        if o.zip64 {
            put16(&mut local_extra, 1);
            put16(&mut local_extra, 16);
            put64(&mut local_extra, size);
            put64(&mut local_extra, size);
        }
        put32(&mut out, 0x0403_4b50);
        put16(&mut out, version);
        put16(&mut out, flags);
        put16(&mut out, 0);
        put16(&mut out, 0);
        put16(&mut out, 0x21);
        put32(&mut out, crc);
        put32(&mut out, small(size));
        put32(&mut out, small(size));
        put16(&mut out, e.name.len() as u16);
        put16(&mut out, local_extra.len() as u16);
        out.extend_from_slice(&e.name);
        out.extend_from_slice(&local_extra);
        out.extend_from_slice(&e.data);
        // Voce della directory centrale.
        let mut extra = Vec::new();
        if o.zip64 {
            put16(&mut extra, 1);
            put16(&mut extra, 24);
            put64(&mut extra, size);
            put64(&mut extra, size);
            put64(&mut extra, offset);
        }
        put32(&mut central, 0x0201_4b50);
        put16(&mut central, version);
        put16(&mut central, version);
        put16(&mut central, flags);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, 0x21);
        put32(&mut central, crc);
        put32(&mut central, small(size));
        put32(&mut central, small(size));
        put16(&mut central, e.name.len() as u16);
        put16(&mut central, extra.len() as u16);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put32(&mut central, if e.name.ends_with(b"/") { 0x10 } else { 0x20 });
        put32(&mut central, small(offset));
        central.extend_from_slice(&e.name);
        central.extend_from_slice(&extra);
    }
    let cd_start = out.len() as u64;
    let cd_size = central.len() as u64;
    out.extend_from_slice(&central);
    let n = entries.len() as u64;
    if o.zip64 {
        let z_pos = out.len() as u64;
        put32(&mut out, 0x0606_4b50);
        put64(&mut out, 44);
        put16(&mut out, 45);
        put16(&mut out, 45);
        put32(&mut out, 0);
        put32(&mut out, 0);
        put64(&mut out, n);
        put64(&mut out, n);
        put64(&mut out, cd_size);
        put64(&mut out, cd_start);
        put32(&mut out, 0x0706_4b50);
        put32(&mut out, 0);
        put64(&mut out, z_pos);
        put32(&mut out, 1);
    }
    let eocd = out.len() as u64;
    put32(&mut out, 0x0605_4b50);
    put16(&mut out, 0);
    put16(&mut out, 0);
    let count = if o.zip64 {
        u16::MAX
    } else if o.wrap_count {
        n as u16
    } else {
        u16::try_from(n).expect("serve zip64")
    };
    put16(&mut out, count);
    put16(&mut out, count);
    put32(&mut out, if o.zip64 { u32::MAX } else { cd_size as u32 });
    put32(&mut out, if o.zip64 { u32::MAX } else { cd_start as u32 });
    put16(&mut out, o.comment.len() as u16);
    out.extend_from_slice(&o.comment);
    out.extend_from_slice(&o.trailing);
    BuiltZip { bytes: out, cd_start, cd_size, eocd }
}

/// Voci di esempio: file, sottocartelle, nomi accentati in UTF-8 e in CP437, un file vuoto.
pub fn sample_entries() -> Vec<ZipEntry> {
    vec![
        ZipEntry::dir("progetto/"),
        ZipEntry::file("progetto/leggimi.txt", b"Rilievo: archivio di prova.\r\n"),
        ZipEntry::dir("progetto/sotto/"),
        ZipEntry::file("progetto/sotto/dati.bin", &pseudo_random(5000, 1)),
        ZipEntry::file("progetto/sotto/vuoto.txt", b""),
        ZipEntry::file("progetto/perché è così.txt", b"accenti in UTF-8"),
        // "città.txt" in CP437: 0x85 = à.
        ZipEntry::cp437(b"progetto/citt\x85.txt", b"accento in CP437"),
    ]
}

/// Voci attese da `sample_entries`: (percorso, dimensione, cartella).
pub fn sample_expected() -> Vec<(String, u64, bool)> {
    vec![
        ("progetto/".into(), 0, true),
        ("progetto/leggimi.txt".into(), 29, false),
        ("progetto/sotto/".into(), 0, true),
        ("progetto/sotto/dati.bin".into(), 5000, false),
        ("progetto/sotto/vuoto.txt".into(), 0, false),
        ("progetto/perché è così.txt".into(), 16, false),
        ("progetto/città.txt".into(), 16, false),
    ]
}
