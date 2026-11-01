// SPDX-License-Identifier: GPL-3.0-or-later
//! Prove raccolte dagli estrattori durante la scansione: l'ingresso del motore insieme all'albero.

use crate::extract::{BinSettings, Epic, ExtractError, MsixPackage, Power, Steam, StorageSense, ToolPath, TrackProgs, Uninstall, UserAssist};

/// Cartelle note di Windows (`SHGetKnownFolderPath`), percorsi assoluti senza barra finale.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KnownFolders {
    pub profile: Option<String>,
    pub local: Option<String>,
    /// `AppData\LocalLow` (residui, F-AP-6).
    pub local_low: Option<String>,
    pub roaming: Option<String>,
    pub program_data: Option<String>,
    pub windows: Option<String>,
    pub program_files: Option<String>,
    pub program_files_x86: Option<String>,
    pub documents: Option<String>,
    pub downloads: Option<String>,
    pub desktop: Option<String>,
    pub pictures: Option<String>,
    pub videos: Option<String>,
    pub music: Option<String>,
}

/// Cartella nota con il codice usato nelle regole (`cartella = "local"`).
pub const FOLDER_CODES: [&str; 15] = [
    "radice",
    "profilo",
    "local",
    "roaming",
    "programdata",
    "windows",
    "programmi",
    "programmi-x86",
    "documenti",
    "download",
    "desktop",
    "immagini",
    "video",
    "musica",
    "ovunque",
];

impl KnownFolders {
    /// Percorso della cartella con questo codice. `radice` è la radice del volume, `ovunque` non ha
    /// un percorso (regole strutturali).
    pub fn get<'a>(&'a self, code: &str, volume_root: &'a str) -> Option<&'a str> {
        match code {
            "radice" => Some(volume_root),
            "profilo" => self.profile.as_deref(),
            "local" => self.local.as_deref(),
            "roaming" => self.roaming.as_deref(),
            "programdata" => self.program_data.as_deref(),
            "windows" => self.windows.as_deref(),
            "programmi" => self.program_files.as_deref(),
            "programmi-x86" => self.program_files_x86.as_deref(),
            "documenti" => self.documents.as_deref(),
            "download" => self.downloads.as_deref(),
            "desktop" => self.desktop.as_deref(),
            "immagini" => self.pictures.as_deref(),
            "video" => self.videos.as_deref(),
            "musica" => self.music.as_deref(),
            _ => None,
        }
    }

    /// Tutte le cartelle note: un percorso di installazione uguale a una di queste, o che ne contiene
    /// una, non è la cartella di un programma (voci di registro sbagliate o scritte dall'utente).
    pub fn all(&self) -> impl Iterator<Item = &str> {
        [
            &self.profile,
            &self.local,
            &self.local_low,
            &self.roaming,
            &self.program_data,
            &self.windows,
            &self.program_files,
            &self.program_files_x86,
            &self.documents,
            &self.downloads,
            &self.desktop,
            &self.pictures,
            &self.videos,
            &self.music,
        ]
        .into_iter()
        .filter_map(|x| x.as_deref())
    }

    /// Cartelle dell'utente (Documenti, Download, Desktop, Immagini, Video, Musica).
    pub fn user_folders(&self) -> [(&'static str, Option<&str>); 6] {
        [
            ("documenti", self.documents.as_deref()),
            ("download", self.downloads.as_deref()),
            ("desktop", self.desktop.as_deref()),
            ("immagini", self.pictures.as_deref()),
            ("video", self.videos.as_deref()),
            ("musica", self.music.as_deref()),
        ]
    }
}

/// Dimensione del Cestino del volume secondo Windows (`SHQueryRecycleBinW`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecycleQuery {
    pub bytes: u64,
    pub items: u64,
}

#[derive(Clone, Debug)]
pub struct Evidence {
    /// Radice del volume scansionato (`C:\`).
    pub volume_root: String,
    /// Il volume contiene la cartella di Windows.
    pub system_volume: bool,
    pub folders: KnownFolders,
    pub user_sid: Option<String>,
    pub uninstall: Result<Uninstall, ExtractError>,
    pub msix: Result<Vec<MsixPackage>, ExtractError>,
    pub steam: Result<Steam, ExtractError>,
    pub epic: Result<Epic, ExtractError>,
    /// Versione di CapCut da `Apps\ProductInfo.xml`.
    pub capcut: Result<String, ExtractError>,
    /// Cartella delle bozze di CapCut (`currentCustomDraftPath`).
    pub capcut_drafts: Result<String, ExtractError>,
    pub track_progs: TrackProgs,
    pub userassist: Result<UserAssist, ExtractError>,
    pub power: Result<Power, ExtractError>,
    pub storage_sense: Result<StorageSense, ExtractError>,
    pub bin: Result<BinSettings, ExtractError>,
    pub recycle: Result<RecycleQuery, ExtractError>,
    pub npm_cache: ToolPath,
    pub pnpm_store: ToolPath,
    /// Protezione del sistema (ripristino configurazione): `Err(Io(5))` senza amministratore.
    pub restore: Result<bool, ExtractError>,
}

impl Evidence {
    /// Prove vuote per un volume: nessun estrattore ha dati. Base dei test.
    pub fn empty(volume_root: &str) -> Self {
        let local = format!(r"{volume_root}Users\demo\AppData\Local");
        Self {
            volume_root: volume_root.to_owned(),
            system_volume: false,
            folders: KnownFolders::default(),
            user_sid: None,
            uninstall: Err(ExtractError::NotPresent),
            msix: Err(ExtractError::NotPresent),
            steam: Err(ExtractError::NotPresent),
            epic: Err(ExtractError::NotPresent),
            capcut: Err(ExtractError::NotPresent),
            capcut_drafts: Err(ExtractError::NotPresent),
            track_progs: TrackProgs::Absent,
            userassist: Err(ExtractError::NotPresent),
            power: Err(ExtractError::NotPresent),
            storage_sense: Err(ExtractError::NotPresent),
            bin: Err(ExtractError::NotPresent),
            recycle: Err(ExtractError::NotPresent),
            npm_cache: ToolPath { path: format!(r"{local}\npm-cache"), origin: crate::extract::ConfigOrigin::Default },
            pnpm_store: ToolPath { path: format!(r"{local}\pnpm\store"), origin: crate::extract::ConfigOrigin::Default },
            restore: Err(ExtractError::NotPresent),
        }
    }

    /// Cartelle note di un profilo `C:\Users\<nome>` costruite come quelle predefinite di Windows.
    pub fn default_folders(volume_root: &str, user: &str) -> KnownFolders {
        let profile = format!(r"{volume_root}Users\{user}");
        KnownFolders {
            local: Some(format!(r"{profile}\AppData\Local")),
            local_low: Some(format!(r"{profile}\AppData\LocalLow")),
            roaming: Some(format!(r"{profile}\AppData\Roaming")),
            program_data: Some(format!(r"{volume_root}ProgramData")),
            windows: Some(format!(r"{volume_root}Windows")),
            program_files: Some(format!(r"{volume_root}Program Files")),
            program_files_x86: Some(format!(r"{volume_root}Program Files (x86)")),
            documents: Some(format!(r"{profile}\Documents")),
            downloads: Some(format!(r"{profile}\Downloads")),
            desktop: Some(format!(r"{profile}\Desktop")),
            pictures: Some(format!(r"{profile}\Pictures")),
            videos: Some(format!(r"{profile}\Videos")),
            music: Some(format!(r"{profile}\Music")),
            profile: Some(profile),
        }
    }
}
