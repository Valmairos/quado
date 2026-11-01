// SPDX-License-Identifier: GPL-3.0-or-later
//! Bilancio del volume (03 §6.4): usato = allocato unico letto + metadati misurati + non attribuito.
//!
//! A + M + N = U vale per costruzione, quindi controllarlo non prova nulla: i test veri sono il residuo
//! su un volume fermo e l'errore iniettato che deve spostarlo (06 §1.4).

/// Spazio del volume in un istante (`GetDiskFreeSpaceExW`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Space {
    pub capacity: u64,
    pub free: u64,
    /// Libero per l'utente corrente: minore di `free` se c'è una quota.
    pub free_to_caller: u64,
}

impl Space {
    pub fn used(&self) -> u64 {
        self.capacity.saturating_sub(self.free)
    }
}

/// Metadati del file system misurati senza privilegi.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    /// `$MFT` (`MftValidDataLength` da `FSCTL_GET_NTFS_VOLUME_DATA`). `None` fuori da NTFS.
    pub mft: Option<u64>,
    /// Indici delle cartelle (`$I30`), somma delle allocate lette sugli handle delle cartelle.
    pub dir_indexes: u64,
    /// `$UsnJrnl:$J`, intervallo attivo (`NextUsn - FirstUsn` da `FSCTL_QUERY_USN_JOURNAL`). `None` se assente.
    pub usn_journal: Option<u64>,
    /// Spazio riservato e non ancora usato (`TotalReservedAllocationUnits` di `GetDiskSpaceInformationW`,
    /// archiviazione riservata compresa): Windows lo conta nell'usato ma non è in nessun file. Si legge
    /// senza privilegi (M6). `None` se la lettura non è riuscita.
    pub reserved: Option<u64>,
}

impl Metadata {
    pub fn total(&self) -> u64 {
        self.mft.unwrap_or(0) + self.dir_indexes + self.usn_journal.unwrap_or(0) + self.reserved.unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Balance {
    pub capacity: u64,
    /// Usato a inizio scansione (U0).
    pub used_start: u64,
    /// Usato a fine scansione (U).
    pub used: u64,
    /// U - U0: si mostra come incertezza sul non attribuito.
    pub drift: i64,
    /// Allocato unico letto (A): ogni file con più nomi contato una volta.
    pub allocated: u64,
    pub metadata: Metadata,
    /// Non attribuito (N) = U - A - M. Può essere negativo per la deriva o per i file residenti
    /// nella MFT, contati sia nella `$MFT` sia come allocata minima (doppio conteggio dichiarato, 02 §3).
    pub unattributed: i64,
    /// Nota: byte che si conterebbero in più senza la deduplica degli hard link.
    pub hardlinks_counted_once: u64,
    /// Nota: differenza tra libero e libero per l'utente (quota), se c'è.
    pub quota: Option<u64>,
}

impl Balance {
    pub fn compute(start: Space, end: Space, allocated: u64, metadata: Metadata, hardlinks_counted_once: u64) -> Self {
        let used = end.used();
        let used_start = start.used();
        let quota = (end.free_to_caller < end.free).then(|| end.free - end.free_to_caller);
        Self {
            capacity: end.capacity,
            used_start,
            used,
            drift: used as i64 - used_start as i64,
            allocated,
            metadata,
            unattributed: used as i64 - allocated as i64 - metadata.total() as i64,
            hardlinks_counted_once,
            quota,
        }
    }

    /// Incertezza del non attribuito: ± il valore assoluto della deriva.
    pub fn uncertainty(&self) -> u64 {
        self.drift.unsigned_abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unattributed_is_the_difference() {
        let s0 = Space { capacity: 1000, free: 400, free_to_caller: 400 };
        let s1 = Space { capacity: 1000, free: 390, free_to_caller: 380 };
        let m = Metadata { mft: Some(50), dir_indexes: 20, usn_journal: Some(5), reserved: Some(7) };
        let b = Balance::compute(s0, s1, 500, m, 77);
        assert_eq!(b.used, 610);
        assert_eq!(b.drift, 10);
        assert_eq!(b.uncertainty(), 10);
        assert_eq!(b.unattributed, 610 - 500 - 82);
        assert_eq!(b.quota, Some(10));
        assert_eq!(b.hardlinks_counted_once, 77);
    }

    #[test]
    fn non_ntfs_has_no_mft() {
        let m = Metadata { mft: None, dir_indexes: 0, usn_journal: None, reserved: None };
        assert_eq!(m.total(), 0);
    }
}
