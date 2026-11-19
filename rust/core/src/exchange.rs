//! Moving saved stations between apps and machines. Port of `ShortwaveCore/Exchange.cs` (itself 1:1 with the
//! Mac's `StationExchange`); the format is in docs/STATION-EXCHANGE.md. An exchange file is also a valid
//! `library.json`, and a bare `library.json` can be imported.

use crate::station::Station;
use base64::Engine;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub const FORMAT: &str = "shortwave-stations";
pub const VERSION: i64 = 1;
/// Suggested file name for an export.
pub const DEFAULT_FILE_NAME: &str = "Driftwave Stations.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    NotJson,
    NoStations,
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ReadError::NotJson => "This file isn’t a station file.",
            ReadError::NoStations => "This file doesn’t contain a list of stations.",
        })
    }
}

impl std::error::Error for ReadError {}

/// What an exchange file holds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Contents {
    pub stations: Vec<Station>,
    /// Cover images by their file name in the exporting app.
    pub covers: HashMap<String, Vec<u8>>,
    /// Entries in the file's station list that couldn't be read.
    pub unreadable: usize,
}

/// Writes an exchange file. `cover` returns the image data for a `customCover` file name.
pub fn encode(
    stations: &[Station],
    app: &str,
    date: Option<SystemTime>,
    cover: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Vec<u8> {
    let mut covers = BTreeMap::new();
    for s in stations {
        if let Some(name) = &s.custom_cover {
            if !covers.contains_key(name) {
                if let Some(data) = cover(name).filter(|d| is_image(d)) {
                    covers.insert(name.clone(), base64::engine::general_purpose::STANDARD.encode(data));
                }
            }
        }
    }
    let secs = date.unwrap_or_else(SystemTime::now).duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // Keys in sorted order, like the Mac's JSONEncoder with .sortedKeys (serde_json's Map is sorted).
    let mut m = Map::new();
    m.insert("app".into(), app.into());
    m.insert("covers".into(), Value::Object(covers.into_iter().map(|(k, v)| (k, Value::String(v))).collect()));
    m.insert("exported".into(), iso8601_utc(secs).into());
    m.insert("format".into(), FORMAT.into());
    m.insert("stations".into(), Value::Array(stations.iter().map(Station::to_json).collect()));
    m.insert("version".into(), VERSION.into());
    serde_json::to_vec_pretty(&Value::Object(m)).expect("JSON values always serialize")
}

/// `yyyy-MM-ddTHH:mm:ssZ` for seconds since 1970 (UTC).
pub fn iso8601_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Reads an exchange file, a `library.json`, or a bare JSON array of stations.
pub fn decode(data: &[u8]) -> Result<Contents, ReadError> {
    let root: Value = serde_json::from_slice(data).map_err(|_| ReadError::NotJson)?;
    let mut covers = HashMap::new();
    let list = match &root {
        Value::Array(a) => a,
        Value::Object(o) => match o.get("stations") {
            Some(Value::Array(a)) => {
                if let Some(Value::Object(c)) = o.get("covers") {
                    for (name, v) in c {
                        if let Some(d) = v.as_str().and_then(lenient_base64).filter(|d| is_image(d)) {
                            covers.insert(name.clone(), d);
                        }
                    }
                }
                a
            }
            _ => return Err(ReadError::NoStations),
        },
        _ => return Err(ReadError::NoStations),
    };
    let stations: Vec<Station> = list
        .iter()
        .filter_map(Station::from_json)
        .filter(|s| !s.id.is_empty() && !is_blank(&s.stream_url))
        .collect();
    let unreadable = list.len() - stations.len();
    Ok(Contents { stations, covers, unreadable })
}

// Swift's .whitespaces: spaces (category Zs) and tab, but not newlines. An empty string is blank.
fn is_blank(s: &str) -> bool {
    s.chars().all(|c| {
        matches!(c, '\t' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
    })
}

// Like Data(base64Encoded:options: .ignoreUnknownCharacters).
fn lenient_base64(s: &str) -> Option<Vec<u8>> {
    let kept: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')).collect();
    base64::engine::general_purpose::STANDARD.decode(kept).ok()
}

/// The outcome of importing stations into a library.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MergeResult {
    /// The library with the new stations appended.
    pub library: Vec<Station>,
    /// The stations that were added.
    pub added: Vec<Station>,
    /// Stations skipped because the library already has them.
    pub already_present: usize,
    /// Entries in the file's station list that couldn't be read.
    pub unreadable: usize,
    /// Cover images to write to the covers folder, by their new file names.
    pub new_covers: HashMap<String, Vec<u8>>,
}

/// Adds the stations the library doesn't have yet. Never changes or removes existing ones.
///
/// A station is already present when its `id` matches, or, for local stations, when a local station with the same
/// stream address exists. Imported covers get new file names from `new_cover_name` (without extension: `.png` or
/// `.jpg` is added); a station whose cover image isn't in the file loses its `customCover`.
pub fn merge(contents: &Contents, library: &[Station], new_cover_name: &mut dyn FnMut() -> String) -> MergeResult {
    let key = |url: &str| url.trim().to_lowercase();
    let mut r = MergeResult { library: library.to_vec(), unreadable: contents.unreadable, ..Default::default() };
    let mut ids: HashSet<String> = r.library.iter().map(|s| s.id.clone()).collect();
    let mut local_urls: HashSet<String> = r.library.iter().filter(|s| s.is_local).map(|s| key(&s.stream_url)).collect();
    for incoming in &contents.stations {
        if ids.contains(&incoming.id) || (incoming.is_local && local_urls.contains(&key(&incoming.stream_url))) {
            r.already_present += 1;
            continue;
        }
        let mut s = incoming.clone();
        if let Some(name) = &s.custom_cover {
            s.custom_cover = contents.covers.get(name).map(|data| {
                let fresh = format!("{}{}", new_cover_name(), if data.starts_with(PNG) { ".png" } else { ".jpg" });
                r.new_covers.insert(fresh.clone(), data.clone());
                fresh
            });
        }
        ids.insert(s.id.clone());
        if s.is_local {
            local_urls.insert(key(&s.stream_url));
        }
        r.library.push(s.clone());
        r.added.push(s);
    }
    r
}

const PNG: &[u8] = &[0x89, 0x50, 0x4E, 0x47];
const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF];

/// PNG or JPEG, by their signatures.
pub fn is_image(d: &[u8]) -> bool {
    d.starts_with(PNG) || d.starts_with(JPEG)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{library_file, test_dir::TempDir};
    use std::path::PathBuf;

    const PNG_DATA: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

    /// Shared fixtures at the repo root, written by each app's export (see docs/STATION-EXCHANGE.md).
    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("fixtures").join("exchange")
    }

    fn names() -> impl FnMut() -> String {
        let mut n = 0;
        move || {
            n += 1;
            format!("new{n}")
        }
    }

    #[test]
    fn round_trip_with_cover() {
        let local = Station::local("Mine", "http://x/live", Some("a.png".into()));
        let mut public = Station::new("uuid-1", "Public", "http://p");
        public.tags = vec!["jazz".into()];
        public.bitrate = 128;
        public.latitude = Some(1.5);
        let data = encode(&[local.clone(), public.clone()], "test", None, &|n| (n == "a.png").then(|| PNG_DATA.to_vec()));
        let text = String::from_utf8(data.clone()).unwrap();
        assert!(text.contains("\"format\": \"shortwave-stations\""));
        let c = decode(&data).unwrap();
        assert_eq!(c.stations, vec![local.clone(), public.clone()]);
        assert_eq!(c.covers.keys().collect::<Vec<_>>(), vec!["a.png"]);
        assert_eq!(c.covers["a.png"], PNG_DATA);
        assert_eq!(c.unreadable, 0);
        // An exchange file is also a library file.
        let dir = TempDir::new("exchange");
        let path = dir.path().join("x.json");
        std::fs::write(&path, &data).unwrap();
        assert_eq!(library_file::load(&path), vec![local, public]);
    }

    #[test]
    fn reads_library_file_and_bare_array() {
        let s = Station::local("Mine", "http://x", None);
        assert_eq!(decode(&library_file::encode(&[s.clone()])).unwrap().stations, vec![s.clone()]);
        let array = serde_json::to_vec(&Value::Array(vec![s.to_json()])).unwrap();
        assert_eq!(decode(&array).unwrap().stations, vec![s]);
    }

    #[test]
    fn lenient_stations() {
        let json = r#"{"stations": [
          {"id": "a", "name": "Only the required keys", "streamURL": "http://a"},
          {"id": "b", "name": "Wrong types", "streamURL": "http://b", "bitrate": "96", "isHLS": 1, "tags": ["x", null], "votes": null},
          {"name": "no id", "streamURL": "http://c"},
          {"id": "d", "name": "empty url", "streamURL": " "},
          "not a station"
        ]}"#;
        let c = decode(json.as_bytes()).unwrap();
        assert_eq!(c.stations.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
        let b = &c.stations[1];
        assert!(b.bitrate == 96 && b.is_hls && b.tags == vec!["x"]);
        assert_eq!(c.unreadable, 3);
    }

    #[test]
    fn rejects_other_files() {
        assert_eq!(decode(b"not json"), Err(ReadError::NotJson));
        assert_eq!(decode(b"{\"a\": 1}"), Err(ReadError::NoStations));
    }

    #[test]
    fn merge_adds_only_new_stations() {
        let mut kept = Station::new("uuid-1", "Kept", "http://p");
        kept.custom_cover = Some("mine.png".into());
        let mut mine = Station::new("local-1", "Mine", "http://X/Live ");
        mine.is_local = true;
        let have = vec![kept, mine];
        let mut same_stream = Station::new("local-2", "Same stream", "http://x/live");
        same_stream.is_local = true;
        let mut new_one = Station::new("uuid-2", "New", "http://n");
        new_one.custom_cover = Some("c.png".into());
        let mut missing = Station::new("uuid-3", "Cover missing", "http://m");
        missing.custom_cover = Some("gone.png".into());
        let incoming = Contents {
            stations: vec![
                Station::new("uuid-1", "Renamed elsewhere", "http://p"), // same id: keep ours
                same_stream,                                             // same local stream
                new_one,
                missing,
                Station::new("uuid-2", "Twice in the file", "http://n"),
            ],
            covers: HashMap::from([("c.png".to_string(), PNG_DATA.to_vec())]),
            unreadable: 1,
        };
        let r = merge(&incoming, &have, &mut names());
        let ids = |v: &[Station]| v.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&r.library), vec!["uuid-1", "local-1", "uuid-2", "uuid-3"]);
        assert!(r.library[0].name == "Kept" && r.library[0].custom_cover.as_deref() == Some("mine.png"));
        assert_eq!(ids(&r.added), vec!["uuid-2", "uuid-3"]);
        assert!(r.added[0].custom_cover.as_deref() == Some("new1.png") && r.added[1].custom_cover.is_none());
        assert_eq!(r.new_covers.keys().collect::<Vec<_>>(), vec!["new1.png"]);
        assert_eq!(r.new_covers["new1.png"], PNG_DATA);
        assert_eq!(r.already_present, 3);
        assert_eq!(r.unreadable, 1);
        assert_eq!(incoming.stations[2].custom_cover.as_deref(), Some("c.png"), "the incoming stations aren't changed");
    }

    #[test]
    fn reads_mac_export() {
        check_fixture("mac-export.json");
    }

    #[test]
    fn reads_windows_export() {
        check_fixture("windows-export.json");
    }

    fn check_fixture(name: &str) {
        let c = decode(&std::fs::read(fixtures().join(name)).unwrap()).unwrap();
        // Both fixtures hold the same three stations (docs/STATION-EXCHANGE.md, "Test fixtures").
        assert_eq!(c.unreadable, 0);
        assert_eq!(c.stations.len(), 3);
        let only = |f: &dyn Fn(&Station) -> bool| {
            let v: Vec<&Station> = c.stations.iter().filter(|s| f(s)).collect();
            assert_eq!(v.len(), 1);
            v[0].clone()
        };
        let public = only(&|s| s.id == "960cfd3b-0601-11e8-ae97-52543be04c81");
        assert!(public.name == "SomaFM Groove Salad" && public.tags == vec!["ambient", "chillout"] && public.bitrate == 128);
        assert!(public.latitude == Some(37.775) && public.longitude == Some(-122.419));
        let local = only(&|s| s.is_local);
        assert!(local.name == "Exchange Test Local" && local.stream_url == "http://127.0.0.1:8765/mp3");
        let cover = local.custom_cover.clone().expect("local station has a cover");
        assert!(c.covers.get(&cover).is_some_and(|d| is_image(d)));
        let orphan = only(&|s| s.is_orphaned);
        assert!(orphan.name == "Exchange Test Orphan" && orphan.homepage.as_deref() == Some("https://example.com/"));
        let r = merge(&c, &[], &mut names());
        assert!(r.added.len() == 3 && r.new_covers.len() == 1);
    }

    #[test]
    fn dates() {
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601_utc(1_795_000_000), "2026-11-18T11:06:40Z");
        assert_eq!(iso8601_utc(951_782_400), "2000-02-29T00:00:00Z");
    }
}
