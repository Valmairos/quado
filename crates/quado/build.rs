// SPDX-License-Identifier: GPL-3.0-or-later
//! Build dell'eseguibile, solo con il linker di Microsoft:
//! - le dipendenze statiche si cercano solo in System32 (06 §11, RS11, 07 §3 riga 5);
//! - le icone di `icona/` (generate da `cargo xtask icona`) diventano risorse dell'eseguibile. Il file `.res`
//!   si scrive qui, senza strumenti esterni, e il linker lo incorpora come fa con quelli di `rc.exe`. Gruppo
//!   1 `quado.ico`, l'icona dell'eseguibile (la usano Esplora file e il collegamento nel menu Start) e della
//!   finestra, grande e piccola (`quado_scan_win::window`).

use std::path::{Path, PathBuf};

/// Tipi di risorsa di Windows (`winuser.h`).
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
/// File delle icone, nell'ordine dei gruppi (da 1).
const ICONS: [&str; 1] = ["quado.ico"];

fn main() {
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if msvc && std::env::var_os("QUADO_SENZA_DEPENDENTLOADFLAG").is_none() {
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    }
    println!("cargo:rerun-if-env-changed=QUADO_SENZA_DEPENDENTLOADFLAG");
    if msvc {
        let dir = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR")).join("icona");
        let res = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("quado.res");
        std::fs::write(&res, resources(&dir)).expect("quado.res");
        println!("cargo:rustc-link-arg-bins={}", res.display());
    }
}

/// Un'immagine di un file `.ico`: i campi della sua voce nella directory e i byte (bitmap o PNG).
struct Image {
    entry: [u8; 12],
    data: Vec<u8>,
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Le immagini di un file `.ico` (ICONDIR, poi una voce di 16 byte per immagine).
fn read_ico(path: &Path) -> Vec<Image> {
    println!("cargo:rerun-if-changed={}", path.display());
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e} (cargo xtask icona)", path.display()));
    assert!(b.len() >= 6 && u16_at(&b, 0) == 0 && u16_at(&b, 2) == 1, "{}: non è un file .ico", path.display());
    (0..usize::from(u16_at(&b, 4)))
        .map(|i| {
            let e = 6 + 16 * i;
            let (len, offset) = (u32_at(&b, e + 8) as usize, u32_at(&b, e + 12) as usize);
            let mut entry = [0u8; 12];
            entry.copy_from_slice(&b[e..e + 12]);
            Image { entry, data: b[offset..offset + len].to_vec() }
        })
        .collect()
}

/// Una voce del file `.res`: intestazione di 32 byte (tipo e nome numerici, lingua neutra), dati, allineamento
/// a 4 byte.
fn push_resource(out: &mut Vec<u8>, kind: u16, id: u16, memory_flags: u16, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes());
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&kind.to_le_bytes());
    out.extend_from_slice(&[0xFF, 0xFF]);
    out.extend_from_slice(&id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&memory_flags.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(data);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

/// Il file `.res`: la voce vuota iniziale, poi per ogni file le sue immagini (RT_ICON, numeri da 1) e il
/// gruppo (RT_GROUP_ICON) che le elenca con il loro numero.
fn resources(dir: &Path) -> Vec<u8> {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut out = Vec::new();
    push_resource(&mut out, 0, 0, 0, &[]);
    let mut next_id = 1u16;
    for (group, name) in ICONS.iter().enumerate() {
        let images = read_ico(&dir.join(name));
        let mut dir_data = Vec::new();
        dir_data.extend_from_slice(&0u16.to_le_bytes());
        dir_data.extend_from_slice(&1u16.to_le_bytes());
        dir_data.extend_from_slice(&(images.len() as u16).to_le_bytes());
        for img in &images {
            push_resource(&mut out, RT_ICON, next_id, 0x1010, &img.data);
            // GRPICONDIRENTRY: i primi 12 byte della voce del file, poi il numero della risorsa.
            dir_data.extend_from_slice(&img.entry);
            dir_data.extend_from_slice(&next_id.to_le_bytes());
            next_id += 1;
        }
        push_resource(&mut out, RT_GROUP_ICON, group as u16 + 1, 0x1030, &dir_data);
    }
    out
}
