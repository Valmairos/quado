// SPDX-License-Identifier: GPL-3.0-or-later
//! Cartelle sincronizzate che Windows non conosce: spostare nel Cestino un loro elemento lo elimina anche
//! nel cloud e sugli altri dispositivi. `CfGetSyncRootInfoByPath` (`win_probe.rs`) vede solo le cartelle
//! registrate con l'interfaccia dei file nel cloud di Windows, come quelle di OneDrive. Qui, senza rete e
//! in sola lettura:
//!
//! - **cartelle dichiarate dalla configurazione di un programma** ([`SyncRoot`]), lette da `quado-kb`:
//!   - Dropbox: `info.json` in `%LOCALAPPDATA%\Dropbox` o `%APPDATA%\Dropbox`, con `path` dei conti
//!     `personal` e `business` (formato documentato da Dropbox, "How to programmatically find the Dropbox
//!     folder paths");
//!   - Google Drive per desktop: l'unità o la cartella del valore `DefaultMountPoint` in
//!     `HKLM\Software\Policies\Google\DriveFS`, `HKCU\Software\Google\DriveFS` o
//!     `HKLM\Software\Google\DriveFS` (impostazioni documentate da Google per gli amministratori);
//! - **marcatori dentro la cartella sincronizzata** ([`MARKERS`]), cercati nell'elemento e nelle cartelle
//!   che lo contengono:
//!   - Syncthing: `.stfolder`, cartella (o file nelle versioni vecchie) che Syncthing crea quando la cartella
//!     viene aggiunta (FAQ di Syncthing);
//!   - Resilio Sync: la cartella `.sync` con `IgnoreList` o `ID` dentro (guida di Resilio).
//!
//! Limiti noti, detti nella conferma ("le cartelle di altri programmi controllale tu"): le cartelle in
//! mirroring di Google Drive non hanno un marcatore né una voce di registro documentati; Syncthing permette
//! un nome di marcatore diverso; MEGA non ha un nome documentato in modo univoco per la sua cartella locale.

/// Come Quado ha riconosciuto una cartella sincronizzata: decide il testo della conferma.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SyncKind {
    /// Registrata in Windows con l'interfaccia dei file nel cloud (OneDrive e simili).
    Registered,
    /// Cartella di un servizio con un cloud, riconosciuta dalla sua configurazione (Dropbox, Google Drive).
    Cloud,
    /// Cartella sincronizzata direttamente tra dispositivi, senza un cloud (Syncthing, Resilio Sync).
    Devices,
}

/// Una cartella sincronizzata che contiene l'elemento (o che l'elemento contiene).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncFolder {
    /// Nome del programma, come lo scrive il programma.
    pub provider: String,
    pub kind: SyncKind,
}

/// Cartella dichiarata dalla configurazione di un programma di sincronizzazione.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncRoot {
    pub path: String,
    pub provider: String,
    pub kind: SyncKind,
}

/// Marcatori: percorso relativo alla cartella sincronizzata (uno qualunque basta) e programma.
pub const MARKERS: [(&[&str], &str); 2] = [(&[".stfolder"], "Syncthing"), (&[".sync\\IgnoreList", ".sync\\ID"], "Resilio Sync")];

/// Percorso senza `\\?\`, con le barre rovesciate, senza la barra finale (salvo la radice `C:\`), in minuscolo.
fn norm(p: &str) -> String {
    let p = p.trim().replace('/', "\\");
    let p = p.strip_prefix(r"\\?\").unwrap_or(&p).to_lowercase();
    let t = p.trim_end_matches('\\');
    if t.len() == 2 && t.ends_with(':') { format!("{t}\\") } else { t.to_owned() }
}

/// `inner` è `outer` o sta dentro `outer` (percorsi normalizzati).
fn within(inner: &str, outer: &str) -> bool {
    inner == outer || (inner.starts_with(outer) && (outer.ends_with('\\') || inner[outer.len()..].starts_with('\\')))
}

/// La cartella sincronizzata dell'elemento `path`: prima le cartelle dichiarate dalla configurazione (quella
/// più profonda; vale anche un elemento che ne contiene una), poi i marcatori nell'elemento e nelle cartelle
/// che lo contengono, dal più vicino. `exists` dice se un percorso c'è (sola lettura degli attributi).
pub fn find(path: &str, roots: &[SyncRoot], exists: &dyn Fn(&str) -> bool) -> Option<SyncFolder> {
    let p = norm(path);
    if p.is_empty() {
        return None;
    }
    // La cartella dichiarata più profonda che contiene l'elemento; altrimenti la più esterna che l'elemento
    // contiene.
    let mut inside: Option<(&SyncRoot, usize)> = None;
    let mut contained: Option<(&SyncRoot, usize)> = None;
    for r in roots {
        let rp = norm(&r.path);
        if rp.is_empty() {
            continue;
        }
        if within(&p, &rp) {
            if inside.is_none_or(|(_, len)| rp.len() > len) {
                inside = Some((r, rp.len()));
            }
        } else if within(&rp, &p) && contained.is_none_or(|(_, len)| rp.len() < len) {
            contained = Some((r, rp.len()));
        }
    }
    if let Some((r, _)) = inside.or(contained) {
        return Some(SyncFolder { provider: r.provider.clone(), kind: r.kind });
    }
    // Dall'elemento in su, fino alla radice dell'unità compresa.
    let original = path.trim().trim_end_matches(['\\', '/']);
    let mut dir: &str = original;
    loop {
        if dir.is_empty() {
            return None;
        }
        let base = format!("{dir}\\");
        for (files, provider) in MARKERS {
            if files.iter().any(|f| exists(&format!("{base}{f}"))) {
                return Some(SyncFolder { provider: (*provider).to_owned(), kind: SyncKind::Devices });
            }
        }
        dir = &dir[..dir.rfind(['\\', '/'])?];
        // `\\?\C:` o `\\server\share`: la radice si controlla, poi ci si ferma.
        if dir.ends_with('?') || dir.is_empty() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exists_in<'a>(set: &'a [&'a str]) -> impl Fn(&str) -> bool + 'a {
        move |p: &str| set.iter().any(|s| s.eq_ignore_ascii_case(p))
    }

    #[test]
    fn syncthing_and_resilio_markers_in_the_item_or_above() {
        let fs = [r"D:\Sync\.stfolder", r"C:\Users\demo\Resilio\.sync\IgnoreList", r"C:\Users\demo\Resilio2\.sync\ID"];
        let e = exists_in(&fs);
        let st = Some(SyncFolder { provider: "Syncthing".into(), kind: SyncKind::Devices });
        assert_eq!(find(r"D:\Sync\progetto\node_modules", &[], &e), st);
        assert_eq!(find(r"D:\Sync", &[], &e), st, "la cartella stessa");
        assert_eq!(find(r"D:\Sync\", &[], &e), st);
        let rs = Some(SyncFolder { provider: "Resilio Sync".into(), kind: SyncKind::Devices });
        assert_eq!(find(r"C:\Users\demo\Resilio\Download\a.zip", &[], &e), rs);
        assert_eq!(find(r"C:\Users\demo\Resilio2\b", &[], &e), rs);
        assert_eq!(find(r"C:\Users\demo\Documents\a.zip", &[], &e), None);
        assert_eq!(find(r"D:\Altro\a", &[], &e), None);
        // Una cartella `.sync` senza i file di Resilio non basta.
        let other = [r"C:\Progetto\.sync\config.json"];
        assert_eq!(find(r"C:\Progetto\x", &[], &exists_in(&other)), None);
        // Il marcatore alla radice dell'unità vale per tutta l'unità.
        let root = [r"E:\.stfolder"];
        assert_eq!(find(r"E:\cartella\file.txt", &[], &exists_in(&root)), st);
    }

    #[test]
    fn configured_roots_win_and_the_deepest_one_counts() {
        let roots = vec![
            SyncRoot { path: r"C:\Users\demo\Dropbox".into(), provider: "Dropbox".into(), kind: SyncKind::Cloud },
            SyncRoot { path: r"C:\Users\demo\Dropbox\Lavoro".into(), provider: "Dropbox (Lavoro)".into(), kind: SyncKind::Cloud },
            SyncRoot { path: "G:\\".into(), provider: "Google Drive".into(), kind: SyncKind::Cloud },
        ];
        let none = |_: &str| false;
        let f = |p: &str| find(p, &roots, &none).map(|s| s.provider);
        assert_eq!(f(r"C:\Users\demo\Dropbox\foto\a.jpg"), Some("Dropbox".into()));
        assert_eq!(f(r"c:\users\DEMO\dropbox\lavoro\b.pdf"), Some("Dropbox (Lavoro)".into()));
        assert_eq!(f(r"C:\Users\demo\Dropbox2\a"), None, "solo a confine di cartella");
        assert_eq!(f(r"G:\My Drive\x"), Some("Google Drive".into()));
        assert_eq!(f(r"\\?\G:\Il mio Drive\y"), Some("Google Drive".into()));
        // Un elemento che contiene la cartella sincronizzata: spostarlo la elimina.
        assert_eq!(f(r"C:\Users\demo"), Some("Dropbox".into()));
        // La configurazione vince sui marcatori.
        let marker = [r"C:\Users\demo\Dropbox\.stfolder"];
        assert_eq!(find(r"C:\Users\demo\Dropbox\a", &roots, &exists_in(&marker)).map(|s| s.kind), Some(SyncKind::Cloud));
    }
}
