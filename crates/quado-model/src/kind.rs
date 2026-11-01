// SPDX-License-Identifier: GPL-3.0-or-later
//! Tipo di file secondo l'estensione (03 F-IN-3): senza leggere il contenuto, quindi sempre
//! dichiarato "secondo l'estensione" nell'interfaccia. Il modello restituisce codici; i nomi delle
//! famiglie stanno nei file di lingua (05 D13).

use crate::names::Name;

/// Famiglia di un file secondo la sua estensione.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Family {
    Video,
    Audio,
    Images,
    Documents,
    Archives,
    DiskImages,
    Installers,
    Components,
    Code,
    Databases,
    Logs,
    /// Estensione non in elenco.
    Other,
    /// Nessuna estensione.
    NoExtension,
}

impl Family {
    pub const ALL: [Self; 13] = [
        Self::Video,
        Self::Audio,
        Self::Images,
        Self::Documents,
        Self::Archives,
        Self::DiskImages,
        Self::Installers,
        Self::Components,
        Self::Code,
        Self::Databases,
        Self::Logs,
        Self::Other,
        Self::NoExtension,
    ];

    /// Codice stabile, usato come chiave nei file di lingua (`famiglia-<codice>`) e nelle impostazioni.
    pub fn code(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Images => "immagini",
            Self::Documents => "documenti",
            Self::Archives => "archivi",
            Self::DiskImages => "immagini-disco",
            Self::Installers => "installatori",
            Self::Components => "componenti",
            Self::Code => "codice",
            Self::Databases => "database",
            Self::Logs => "log",
            Self::Other => "altro",
            Self::NoExtension => "senza-estensione",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.code() == code)
    }

    /// Famiglia dell'estensione (senza punto, in qualunque maiuscolo).
    pub fn of_extension(ext: &str) -> Self {
        if ext.is_empty() {
            return Self::NoExtension;
        }
        lookup(&ext.to_lowercase())
    }

    /// Famiglia di un nome di file. Senza allocazioni per i nomi ASCII: si usa su milioni di nomi.
    pub fn of_name(name: Name<'_>) -> Self {
        match name {
            Name::Ascii(s) => match s.iter().rposition(|&b| b == b'.') {
                Some(0) | None => Self::NoExtension,
                Some(i) if i + 1 == s.len() => Self::NoExtension,
                Some(i) => {
                    let ext = &s[i + 1..];
                    let mut buf = [0u8; 16];
                    if ext.len() > buf.len() {
                        return Self::Other;
                    }
                    for (d, &c) in buf.iter_mut().zip(ext) {
                        *d = c.to_ascii_lowercase();
                    }
                    // I nomi ASCII danno sempre UTF-8 valido.
                    std::str::from_utf8(&buf[..ext.len()]).map_or(Self::Other, lookup)
                }
            },
            Name::Wide(_) => Self::of_extension(&extension(name)),
        }
    }
}

fn lookup(lower: &str) -> Family {
    static MAP: std::sync::OnceLock<std::collections::HashMap<&'static str, Family>> = std::sync::OnceLock::new();
    let map = MAP.get_or_init(|| TABLE.iter().flat_map(|(f, list)| list.iter().map(move |e| (*e, *f))).collect());
    map.get(lower).copied().unwrap_or(Family::Other)
}

const TABLE: &[(Family, &[&str])] = &[
    (Family::Video, &["mp4", "mkv", "mov", "avi", "wmv", "webm", "m4v", "flv", "mpg", "mpeg", "m2ts", "mts", "3gp", "vob"]),
    (Family::Audio, &["mp3", "wav", "flac", "aac", "ogg", "m4a", "wma", "opus", "aiff", "aif", "mid", "midi"]),
    (
        Family::Images,
        &[
            "jpg", "jpeg", "png", "gif", "bmp", "tif", "tiff", "webp", "heic", "heif", "avif", "raw", "cr2", "cr3", "nef", "arw", "dng",
            "svg", "ico", "psd", "xcf",
        ],
    ),
    (
        Family::Documents,
        &["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "txt", "md", "csv", "epub", "one"],
    ),
    (Family::Archives, &["zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst", "cab", "lz4"]),
    (Family::DiskImages, &["iso", "img", "vhd", "vhdx", "vmdk", "qcow2", "wim", "esd", "dmg"]),
    (Family::Installers, &["exe", "msi", "msix", "msixbundle", "appx", "appxbundle", "msp"]),
    (Family::Components, &["dll", "sys", "ocx", "drv", "mui", "cpl", "node", "pyd", "so"]),
    (
        Family::Code,
        &[
            "rs", "c", "h", "cpp", "hpp", "cc", "cs", "java", "kt", "py", "js", "mjs", "cjs", "ts", "tsx", "jsx", "go", "rb", "php",
            "html", "htm", "css", "scss", "json", "toml", "yaml", "yml", "xml", "sh", "ps1", "bat", "cmd", "lua", "map",
        ],
    ),
    (Family::Databases, &["db", "sqlite", "sqlite3", "mdb", "accdb", "ldb", "edb", "pak"]),
    (Family::Logs, &["log", "etl", "evtx", "dmp", "mdmp"]),
];

/// Estensione di un nome, senza punto: ciò che segue l'ultimo punto, se non è il primo carattere.
pub fn extension(name: Name<'_>) -> String {
    let s = name.to_string_lossy();
    match s.rfind('.') {
        Some(0) | None => String::new(),
        Some(i) if i + 1 < s.len() => s[i + 1..].to_owned(),
        Some(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_by_extension() {
        assert_eq!(Family::of_extension("MP4"), Family::Video);
        assert_eq!(Family::of_extension("vhdx"), Family::DiskImages);
        assert_eq!(Family::of_extension("exe"), Family::Installers);
        assert_eq!(Family::of_extension("xyz"), Family::Other);
        assert_eq!(Family::of_extension(""), Family::NoExtension);
        assert_eq!(Family::of_name(Name::Ascii(b".gitignore")), Family::NoExtension);
        assert_eq!(Family::of_name(Name::Ascii(b"archivio.tar.gz")), Family::Archives);
        assert_eq!(Family::of_name(Name::Ascii(b"strano.")), Family::NoExtension);
    }

    #[test]
    fn codes_round_trip_and_are_unique() {
        for f in Family::ALL {
            assert_eq!(Family::from_code(f.code()), Some(f));
        }
        let mut all: Vec<&str> = TABLE.iter().flat_map(|(_, l)| l.iter().copied()).collect();
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n, "un'estensione in due famiglie");
    }
}
