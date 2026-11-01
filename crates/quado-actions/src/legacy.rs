// SPDX-License-Identifier: GPL-3.0-or-later
//! Dati della versione che si chiamava Rilievo (fino al 26/10/2026): la cartella `%APPDATA%\Rilievo`.
//!
//! Al primo avvio di Quado, se `%APPDATA%\Quado` non c'è e `%APPDATA%\Rilievo` sì, [`migrate`] copia le
//! impostazioni, il registro degli interventi e i file delle lingue. La copia si prepara in una cartella
//! accanto ([`STAGING_NAME`]) e prende il nome definitivo solo alla fine: se si interrompe, la cartella di
//! Quado non c'è e al prossimo avvio si riprova. La cartella di Rilievo resta com'è; Dati e privacy la
//! mostra con la dimensione e la cancella dopo una conferma ([`delete_old`]).

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Nome della cartella dei dati di Quado in `%APPDATA%`.
pub const DIR_NAME: &str = "Quado";
/// Nome della cartella dei dati della versione precedente, in `%APPDATA%`.
pub const OLD_DIR_NAME: &str = "Rilievo";
/// Cartella della copia in corso, accanto a quella di Quado.
pub const STAGING_NAME: &str = "Quado.copia-da-Rilievo";

/// Esito della copia al primo avvio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Migration {
    /// La cartella di Quado c'è già, o quella di Rilievo no: niente da copiare.
    Nothing,
    /// Copiati questi elementi (file o cartelle) dalla cartella di Rilievo, nell'ordine chiesto.
    Copied(Vec<String>),
    /// Copia non riuscita, con il codice di Windows (0 se non c'è): la cartella di Quado non c'è e al
    /// prossimo avvio si riprova.
    Failed(u32),
}

fn code(e: &std::io::Error) -> u32 {
    e.raw_os_error().map_or(0, |c| c as u32)
}

/// `%APPDATA%\Quado` e `%APPDATA%\Rilievo` dentro `appdata`.
pub fn dirs(appdata: &Path) -> (PathBuf, PathBuf) {
    (appdata.join(DIR_NAME), appdata.join(OLD_DIR_NAME))
}

/// Copia da `old` a `new` gli elementi `names` (file o cartelle, quelli che ci sono), se `new` non c'è e
/// `old` sì. `old` non cambia. La copia si fa in [`STAGING_NAME`], accanto a `new`, che prende il nome
/// di `new` solo quando tutto è copiato.
pub fn migrate(old: &Path, new: &Path, names: &[&str]) -> Migration {
    if new.exists() || !old.is_dir() {
        return Migration::Nothing;
    }
    let Some(parent) = new.parent() else { return Migration::Nothing };
    let staging = parent.join(STAGING_NAME);
    // Una copia rimasta a metà da un avvio precedente: è di Quado, si rifà da capo.
    if let Err(e) = remove_if_present(&staging) {
        return Migration::Failed(code(&e));
    }
    let copied = std::fs::create_dir_all(&staging).and_then(|()| copy_names(old, &staging, names));
    let copied = match copied {
        Ok(c) => c,
        Err(e) => {
            let _ = remove_if_present(&staging);
            return Migration::Failed(code(&e));
        }
    };
    match std::fs::rename(&staging, new) {
        Ok(()) => Migration::Copied(copied),
        Err(e) => {
            let _ = remove_if_present(&staging);
            // Un'altra copia di Quado partita insieme ha fatto prima: i dati ci sono già.
            if new.is_dir() { Migration::Nothing } else { Migration::Failed(code(&e)) }
        }
    }
}

fn remove_if_present(p: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(p) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Copia gli elementi di `names` che esistono in `from`; restituisce i nomi copiati.
fn copy_names(from: &Path, to: &Path, names: &[&str]) -> std::io::Result<Vec<String>> {
    let mut copied = Vec::new();
    for &name in names {
        let src = from.join(name);
        let meta = match std::fs::symlink_metadata(&src) {
            Ok(m) => m,
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        if meta.is_dir() {
            copy_dir(&src, &to.join(name))?;
        } else if meta.is_file() {
            std::fs::copy(&src, to.join(name))?;
        } else {
            // Collegamenti: non si seguono.
            continue;
        }
        copied.push(name.to_owned());
    }
    Ok(copied)
}

/// Copia una cartella con le sottocartelle: solo file e cartelle, i collegamenti non si seguono.
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let t = e.file_type()?;
        if t.is_dir() {
            copy_dir(&e.path(), &to.join(e.file_name()))?;
        } else if t.is_file() {
            std::fs::copy(e.path(), to.join(e.file_name()))?;
        }
    }
    Ok(())
}

/// Cancella per sempre la cartella dei dati di Rilievo, dopo la conferma in Dati e privacy. Cancella solo
/// una cartella che si chiama [`OLD_DIR_NAME`]; i collegamenti al suo interno si tolgono senza seguirli.
/// Se la cartella non c'è più non fa niente. L'errore porta il codice di Windows (0 se non c'è).
pub fn delete_old(old: &Path) -> Result<(), u32> {
    if old.file_name().is_none_or(|n| !n.eq_ignore_ascii_case(OLD_DIR_NAME)) {
        return Err(0);
    }
    remove_if_present(old).map_err(|e| code(&e))
}
