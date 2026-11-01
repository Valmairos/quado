// SPDX-License-Identifier: GPL-3.0-or-later
//! Elementi protetti (03 F-AC-4, 05 D7 controllo 1): cartella di Windows, WinSxS, `Windows\Installer`,
//! System Volume Information, file di paging, ibernazione e swap, cartelle dei programmi registrati.
//! Dalle prove già raccolte (cartelle note, voci di disinstallazione, Steam, Epic, MSIX) e dalla vista
//! Programmi: nessuna lettura nuova.
//!
//! Scelte:
//! - Le radici di sistema stanno sul volume delle prove: la cartella di Windows (con WinSxS e
//!   `Windows\Installer`) solo se è su quel volume; System Volume Information, `pagefile.sys`,
//!   `hiberfil.sys` e `swapfile.sys` sempre, alla radice del volume, anche se non ci sono (un file di
//!   paging può comparire dopo la scansione e non si sposta mai). Se la cartella nota di Windows non è
//!   stata letta si protegge `<radice>Windows`, per prudenza: non è una lettura, è il nome predefinito.
//! - Le cartelle dei programmi registrati sono le `Location::Folder` di `Programs::items`, cioè le
//!   cartelle con una prova del percorso (`InstallLocation`, indizio di `UninstallString` o
//!   `DisplayIcon`, manifest di Steam e di Epic, `InstalledLocation` di MSIX), voci nascoste
//!   (`SystemComponent=1`) comprese. Una cartella dichiarata ma non trovata dalla scansione
//!   (`node: None`) resta protetta: il programma è comunque registrato e la cartella può comparire.
//!   Le `Location::Implausible` (radice del disco, cartella nota o un suo antenato) no: proteggerle
//!   bloccherebbe il profilo o `Program Files` interi per una voce di registro sbagliata. I pacchetti
//!   MSIX framework non sono programmi della vista e restano fuori (stanno in `WindowsApps`, che
//!   l'utente non può modificare).
//! - Niente doppioni, senza distinzione di maiuscole: vale la prima radice, e le radici di sistema
//!   vengono prima, così un percorso che è insieme di sistema e di un programma non ha l'eccezione
//!   di regola (03 §6.1), che vale solo per `RegisteredProgram`.

use std::collections::HashSet;

use quado_model::intervention::{ProtectedKind, ProtectedRoot};

use crate::evidence::Evidence;
use crate::extract;
use crate::programs::{Location, Programs};

/// Radice del volume con la barra finale (`C:\`), come la scrivono le prove.
fn volume_prefix(root: &str) -> String {
    let r = root.trim().replace('/', "\\");
    if r.ends_with('\\') { r } else { format!("{r}\\") }
}

/// Raccolta delle radici senza doppioni (percorso senza distinzione di maiuscole).
struct Roots {
    prefix: String,
    seen: HashSet<String>,
    out: Vec<ProtectedRoot>,
}

impl Roots {
    fn on_volume(&self, path: &str) -> bool {
        let p = path.to_lowercase();
        p.len() > self.prefix.len() && p.starts_with(&self.prefix)
    }

    fn push(&mut self, path: &str, kind: ProtectedKind) {
        let path = extract::normalize(path);
        if !self.on_volume(&path) {
            return;
        }
        if self.seen.insert(path.to_lowercase()) {
            self.out.push(ProtectedRoot { path, kind });
        }
    }
}

/// Radici protette sul volume delle prove (`ev.volume_root`).
pub fn protected_roots(ev: &Evidence, programs: &Programs) -> Vec<ProtectedRoot> {
    let root = volume_prefix(&ev.volume_root);
    let mut r = Roots { prefix: root.to_lowercase(), seen: HashSet::new(), out: Vec::new() };
    let windows = ev.folders.windows.as_deref().map(extract::normalize).unwrap_or_else(|| format!("{root}Windows"));
    if r.on_volume(&windows) {
        r.push(&windows, ProtectedKind::WindowsDir);
        r.push(&extract::join(&windows, "WinSxS"), ProtectedKind::WinSxS);
        r.push(&extract::join(&windows, "Installer"), ProtectedKind::WindowsInstaller);
    }
    r.push(&format!("{root}System Volume Information"), ProtectedKind::SystemVolumeInformation);
    r.push(&format!("{root}pagefile.sys"), ProtectedKind::PagingFile);
    r.push(&format!("{root}hiberfil.sys"), ProtectedKind::Hibernation);
    r.push(&format!("{root}swapfile.sys"), ProtectedKind::SwapFile);
    for p in &programs.items {
        if let Location::Folder { path, .. } = &p.location {
            r.push(path, ProtectedKind::RegisteredProgram);
        }
    }
    r.out
}
