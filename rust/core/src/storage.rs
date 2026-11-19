//! Where the app keeps its files, and the library file. Port of `ShortwaveCore/Storage.cs`.

use crate::station::Station;
use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Where the app keeps its files.
#[derive(Debug, Clone, PartialEq)]
pub struct AppPaths {
    /// Library, covers, settings, themes. Windows: `%APPDATA%\<folder>`; Mac: `~/Library/Application Support/<folder>`;
    /// Linux: `$XDG_DATA_HOME/<folder>` (default `~/.local/share/<folder>`).
    pub support: PathBuf,
    /// Favicon cache, temporary recordings, update downloads. Windows: `%LOCALAPPDATA%\<folder>`; Mac:
    /// `~/Library/Caches/<folder>`; Linux: `$XDG_CACHE_HOME/<folder>` (default `~/.cache/<folder>`).
    pub caches: PathBuf,
}

impl AppPaths {
    /// The platform's usual folders for an app folder named `app_folder` (normally "Driftwave").
    pub fn for_app(app_folder: &str) -> AppPaths {
        AppPaths { support: support_base().join(app_folder), caches: caches_base().join(app_folder) }
    }

    pub fn new(support: impl Into<PathBuf>, caches: impl Into<PathBuf>) -> AppPaths {
        AppPaths { support: support.into(), caches: caches.into() }
    }

    pub fn library(&self) -> PathBuf {
        self.support.join("library.json")
    }
    pub fn covers(&self) -> PathBuf {
        self.support.join("Covers")
    }
    pub fn themes(&self) -> PathBuf {
        self.support.join("Themes")
    }
    pub fn favicons(&self) -> PathBuf {
        self.caches.join("Favicons")
    }
    pub fn recording(&self) -> PathBuf {
        self.caches.join("Recording")
    }
    pub fn updates(&self) -> PathBuf {
        self.caches.join("Updates")
    }
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The user's home folder.
pub fn home_dir() -> PathBuf {
    if cfg!(windows) {
        env_dir("USERPROFILE").unwrap_or_else(|| PathBuf::from("C:\\"))
    } else {
        env_dir("HOME").unwrap_or_else(|| PathBuf::from("/"))
    }
}

fn support_base() -> PathBuf {
    if cfg!(windows) {
        env_dir("APPDATA").unwrap_or_else(|| home_dir().join("AppData").join("Roaming"))
    } else if cfg!(target_os = "macos") {
        home_dir().join("Library").join("Application Support")
    } else {
        env_dir("XDG_DATA_HOME").unwrap_or_else(|| home_dir().join(".local").join("share"))
    }
}

fn caches_base() -> PathBuf {
    if cfg!(windows) {
        env_dir("LOCALAPPDATA").unwrap_or_else(|| home_dir().join("AppData").join("Local"))
    } else if cfg!(target_os = "macos") {
        home_dir().join("Library").join("Caches")
    } else {
        env_dir("XDG_CACHE_HOME").unwrap_or_else(|| home_dir().join(".cache"))
    }
}

/// The user's Music folder. On Windows this is the default location; the app overrides it with the shell's known
/// folder (which follows a moved Music folder) when it can.
pub fn music_dir() -> PathBuf {
    if !cfg!(windows) && !cfg!(target_os = "macos") {
        if let Some(d) = env_dir("XDG_MUSIC_DIR") {
            return d;
        }
    }
    home_dir().join("Music")
}

/// Where saved tracks go unless the user picks another folder: `Music/Driftwave`.
pub fn default_track_directory() -> PathBuf {
    music_dir().join("Driftwave")
}

/// The saved stations, stored as JSON so they survive the station vanishing from the database.
pub mod library_file {
    use super::*;

    pub const VERSION: i64 = 1;

    /// The stations in `path`; an empty list if the file is missing or unreadable.
    pub fn load(path: &Path) -> Vec<Station> {
        let Ok(bytes) = fs::read(path) else { return Vec::new() };
        let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else { return Vec::new() };
        match doc.get("stations").and_then(Value::as_array) {
            Some(list) => list.iter().filter_map(Station::from_json).collect(),
            None => Vec::new(),
        }
    }

    /// Same layout as the Mac: `{"stations": [...], "version": 1}`, pretty-printed with sorted keys.
    pub fn encode(stations: &[Station]) -> Vec<u8> {
        let mut m = Map::new();
        m.insert("stations".into(), Value::Array(stations.iter().map(Station::to_json).collect()));
        m.insert("version".into(), VERSION.into());
        serde_json::to_vec_pretty(&Value::Object(m)).expect("JSON values always serialize")
    }

    /// Writes the library atomically: to `<path>.tmp` first, then renamed over the old file.
    pub fn save(stations: &[Station], path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        fs::write(&tmp, encode(stations))?;
        fs::rename(&tmp, path)
    }
}

#[cfg(test)]
pub(crate) mod test_dir {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// A fresh, empty folder under the system temp folder, removed when dropped.
    pub struct TempDir(pub PathBuf);

    impl TempDir {
        pub fn new(tag: &str) -> TempDir {
            let n = NEXT.fetch_add(1, Ordering::SeqCst);
            let p = std::env::temp_dir().join(format!("driftwave-test-{tag}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        pub fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_dir::TempDir;
    use super::*;

    #[test]
    fn library_round_trip_and_layout() {
        let dir = TempDir::new("library");
        let path = dir.path().join("sub").join("library.json");
        let mut a = Station::new("a", "Ä Radio", "http://a");
        a.tags = vec!["x".into()];
        let b = Station::local("Mine", "http://b", Some("cover.png".into()));
        library_file::save(&[a.clone(), b.clone()], &path).unwrap();
        assert_eq!(library_file::load(&path), vec![a, b]);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("{\n  \"stations\": [\n    {\n      \"bitrate\": 0,"));
        assert!(text.trim_end().ends_with("\"version\": 1\n}"));
        assert!(text.contains("Ä Radio"), "non-ASCII is written as is");
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn library_load_is_forgiving() {
        let dir = TempDir::new("library-bad");
        let p = dir.path().join("library.json");
        assert!(library_file::load(&p).is_empty());
        fs::write(&p, "not json").unwrap();
        assert!(library_file::load(&p).is_empty());
        fs::write(&p, r#"{"stations":[{"id":"1","name":"ok","streamURL":"u"},{"name":"no id"},7]}"#).unwrap();
        assert_eq!(library_file::load(&p).len(), 1);
    }

    /// A library.json in the Mac's format (hand-written to match Swift's JSONEncoder: escaped slashes, " : "
    /// separators, optional values left out).
    #[test]
    fn loads_mac_library() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/mac-library.json");
        let list = library_file::load(&fixture);
        assert_eq!(list.len(), 2);
        let soma = &list[0];
        assert_eq!(soma.id, "960cfd3b-0601-11e8-ae97-52543be04c81");
        assert_eq!(soma.stream_url, "http://ice1.somafm.com/groovesalad-128-mp3");
        assert_eq!(soma.homepage.as_deref(), Some("https://somafm.com/groovesalad/"));
        assert_eq!(soma.tags, vec!["ambient", "chillout"]);
        assert_eq!((soma.latitude, soma.longitude), (Some(37.775), Some(-122.419)));
        assert_eq!(soma.votes, 4321);
        assert_eq!(soma.custom_cover.as_deref(), Some("5C1A7F0E-2B7D-4F64-9E1B-1D2C3B4A5F60.png"));
        let local = &list[1];
        assert!(local.is_local && local.homepage.is_none() && local.latitude.is_none() && local.tags.is_empty());
        assert_eq!(local.name, "Café Radio été");
    }

    #[test]
    fn writes_mac_compatible_layout() {
        let mut s = Station::new("id1", "A/B", "http://x/y");
        s.tags = vec!["t".into()];
        s.latitude = Some(1.5);
        let json = String::from_utf8(library_file::encode(&[s.clone()])).unwrap();
        assert!(!json.contains("null") && !json.contains("homepage"));
        assert!(json.find("\"stations\"") < json.find("\"version\""));
        let keys: Vec<&str> = json
            .split('"')
            .collect::<Vec<_>>()
            .windows(2)
            .filter(|w| w[1].starts_with(':'))
            .map(|w| w[0])
            .filter(|k| *k != "stations" && *k != "version")
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
        assert_eq!(
            keys,
            vec!["bitrate", "codec", "country", "countryCode", "id", "isHLS", "isLocal", "isOrphaned", "language",
                 "latitude", "name", "state", "streamURL", "tags", "votes"]
        );
        let dir = TempDir::new("layout");
        let path = dir.path().join("library.json");
        library_file::save(&[s.clone()], &path).unwrap();
        assert_eq!(library_file::load(&path), vec![s]);
    }

    #[test]
    fn paths() {
        let p = AppPaths::new("S", "C");
        assert_eq!(p.library(), PathBuf::from("S").join("library.json"));
        assert_eq!(p.recording(), PathBuf::from("C").join("Recording"));
        assert!(AppPaths::for_app("Driftwave").support.ends_with("Driftwave"));
    }
}
