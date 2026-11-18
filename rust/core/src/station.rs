//! A radio station, from radio-browser.info or added by the user. Port of `ShortwaveCore/Station.cs`.

use serde_json::{Map, Value};
use std::cmp::Ordering;

/// A radio station, either from the radio-browser.info database or added locally by the user.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Station {
    pub id: String,
    pub name: String,
    pub stream_url: String,
    pub homepage: Option<String>,
    pub favicon_url: Option<String>,
    pub tags: Vec<String>,
    pub country: String,
    pub country_code: String,
    pub state: String,
    pub language: String,
    pub codec: String,
    pub bitrate: i32,
    pub votes: i32,
    pub is_hls: bool,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    /// Exists only in the user's library, not in the public database.
    pub is_local: bool,
    /// A public station that could no longer be found in the database.
    pub is_orphaned: bool,
    /// File name of a user-chosen cover image (in the app's covers folder).
    pub custom_cover: Option<String>,
}

impl Station {
    pub fn new(id: impl Into<String>, name: impl Into<String>, stream_url: impl Into<String>) -> Self {
        Station { id: id.into(), name: name.into(), stream_url: stream_url.into(), ..Default::default() }
    }

    /// A station the user added by hand; its id starts with `local-`.
    pub fn local(name: impl Into<String>, stream_url: impl Into<String>, custom_cover: Option<String>) -> Self {
        Station {
            id: format!("local-{}", uuid::Uuid::new_v4()),
            name: name.into(),
            stream_url: stream_url.into(),
            is_local: true,
            custom_cover,
            ..Default::default()
        }
    }

    pub fn has_location(&self) -> bool {
        self.latitude.is_some() && self.longitude.is_some()
    }

    /// Short human description, e.g. "MP3 · 128 kbps · Germany".
    pub fn subtitle(&self) -> String {
        let mut parts = Vec::new();
        if !self.codec.is_empty() {
            parts.push(self.codec.clone());
        }
        if self.bitrate > 0 {
            parts.push(format!("{} kbps", self.bitrate));
        }
        if !self.country.is_empty() {
            parts.push(self.country.clone());
        }
        parts.join(" · ")
    }

    /// Copies public metadata from a fresh database entry, keeping user-owned fields.
    pub fn refreshed(&self, fresh: &Station) -> Station {
        let mut s = fresh.clone();
        s.custom_cover = self.custom_cover.clone();
        s.is_orphaned = false;
        s.is_local = false;
        s
    }

    // MARK: library.json encoding (same keys and types as the Mac's Codable Station)

    /// The station as library.json writes it: sorted keys; optional values are left out rather than null.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new(); // serde_json's Map is sorted by key
        m.insert("bitrate".into(), self.bitrate.into());
        m.insert("codec".into(), self.codec.clone().into());
        m.insert("country".into(), self.country.clone().into());
        m.insert("countryCode".into(), self.country_code.clone().into());
        if let Some(c) = &self.custom_cover {
            m.insert("customCover".into(), c.clone().into());
        }
        if let Some(f) = &self.favicon_url {
            m.insert("faviconURL".into(), f.clone().into());
        }
        if let Some(h) = &self.homepage {
            m.insert("homepage".into(), h.clone().into());
        }
        m.insert("id".into(), self.id.clone().into());
        m.insert("isHLS".into(), self.is_hls.into());
        m.insert("isLocal".into(), self.is_local.into());
        m.insert("isOrphaned".into(), self.is_orphaned.into());
        m.insert("language".into(), self.language.clone().into());
        if let Some(lat) = self.latitude {
            m.insert("latitude".into(), number(lat));
        }
        if let Some(lon) = self.longitude {
            m.insert("longitude".into(), number(lon));
        }
        m.insert("name".into(), self.name.clone().into());
        m.insert("state".into(), self.state.clone().into());
        m.insert("streamURL".into(), self.stream_url.clone().into());
        m.insert("tags".into(), Value::Array(self.tags.iter().map(|t| Value::String(t.clone())).collect()));
        m.insert("votes".into(), self.votes.into());
        Value::Object(m)
    }

    /// Reads a library.json station (written by this app, the Windows app or the Mac app).
    /// `None` without an id, name and stream URL.
    pub fn from_json(e: &Value) -> Option<Station> {
        let o = e.as_object()?;
        let id = str_of(o, "id")?;
        let name = str_of(o, "name")?;
        let url = str_of(o, "streamURL")?;
        Some(Station {
            id,
            name,
            stream_url: url,
            homepage: str_of(o, "homepage"),
            favicon_url: str_of(o, "faviconURL"),
            tags: tag_list(o).unwrap_or_default(),
            country: str_of(o, "country").unwrap_or_default(),
            country_code: str_of(o, "countryCode").unwrap_or_default(),
            state: str_of(o, "state").unwrap_or_default(),
            language: str_of(o, "language").unwrap_or_default(),
            codec: str_of(o, "codec").unwrap_or_default(),
            bitrate: int_of(o, "bitrate"),
            votes: int_of(o, "votes"),
            is_hls: bool_of(o, "isHLS"),
            latitude: num_of(o, "latitude"),
            longitude: num_of(o, "longitude"),
            is_local: bool_of(o, "isLocal"),
            is_orphaned: bool_of(o, "isOrphaned"),
            custom_cover: str_of(o, "customCover"),
        })
    }
}

impl std::fmt::Display for Station {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// Whole numbers are written as integers (`52`, not `52.0`), like Foundation and System.Text.Json.
fn number(d: f64) -> Value {
    if d.fract() == 0.0 && d.abs() < 9.0e15 {
        Value::from(d as i64)
    } else {
        serde_json::Number::from_f64(d).map(Value::Number).unwrap_or(Value::Null)
    }
}

// Like the Mac's lenient decoder, which reads tags as [String?]: nulls are dropped, and any other kind of
// element makes the whole list empty.
fn tag_list(o: &Map<String, Value>) -> Option<Vec<String>> {
    let arr = o.get("tags")?.as_array()?;
    let mut list = Vec::new();
    for x in arr {
        match x {
            Value::String(s) => list.push(s.clone()),
            Value::Null => {}
            _ => return None,
        }
    }
    Some(list)
}

pub(crate) fn str_of(o: &Map<String, Value>, key: &str) -> Option<String> {
    o.get(key).and_then(|v| v.as_str()).map(str::to_owned)
}

/// A number, or a string holding one (sign, decimal point and exponent allowed; no thousands separators).
pub(crate) fn num_of(o: &Map<String, Value>, key: &str) -> Option<f64> {
    match o.get(key)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => parse_number(s),
        _ => None,
    }
}

fn parse_number(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E')) {
        return None;
    }
    t.parse::<f64>().ok().filter(|d| d.is_finite())
}

fn int_of(o: &Map<String, Value>, key: &str) -> i32 {
    match num_of(o, key) {
        Some(d) if d.is_finite() && d.abs() < i32::MAX as f64 => d.trunc() as i32,
        _ => 0,
    }
}

/// true, or any non-zero number (also as a string).
pub(crate) fn bool_of(o: &Map<String, Value>, key: &str) -> bool {
    match o.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(_) => num_of(o, key).unwrap_or(0.0) != 0.0,
        None => false,
    }
}

// MARK: radio-browser.info JSON

/// Converts one station object as returned by radio-browser.info.
pub fn station_from_api(e: &Value) -> Station {
    let empty = Map::new();
    let o = e.as_object().unwrap_or(&empty);
    let clean = |k: &str| str_of(o, k).unwrap_or_default().trim().to_owned();
    let url = str_of(o, "url");
    let resolved = match str_of(o, "url_resolved") {
        Some(r) if !r.is_empty() => r,
        _ => url.unwrap_or_default(),
    };
    let tags = clean("tags").split(',').map(|x| x.trim().to_owned()).filter(|x| !x.is_empty()).collect();
    let name = clean("name");
    let homepage = clean("homepage");
    let favicon = clean("favicon");
    let language = clean("language")
        .split(',')
        .filter(|x| !x.is_empty())
        .map(|x| capitalized(x.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let codec = clean("codec");
    Station {
        id: str_of(o, "stationuuid").unwrap_or_default(),
        name: if name.is_empty() { "Unnamed Station".into() } else { name },
        stream_url: resolved,
        homepage: (!homepage.is_empty()).then_some(homepage),
        favicon_url: (!favicon.is_empty()).then_some(favicon),
        tags,
        country: clean("country"),
        country_code: clean("countrycode"),
        state: clean("state"),
        language,
        codec: if codec.eq_ignore_ascii_case("UNKNOWN") { String::new() } else { codec },
        bitrate: num_of(o, "bitrate").unwrap_or(0.0) as i32,
        votes: num_of(o, "votes").unwrap_or(0.0) as i32,
        is_hls: num_of(o, "hls").unwrap_or(0.0) != 0.0,
        latitude: num_of(o, "geo_lat"),
        longitude: num_of(o, "geo_long"),
        ..Default::default()
    }
}

/// A radio-browser.info station list; entries without a `stationuuid` are skipped.
pub fn stations_from_api(json: &str) -> Result<Vec<Station>, serde_json::Error> {
    let v: Value = serde_json::from_str(json)?;
    Ok(v.as_array()
        .map(|a| {
            a.iter()
                .filter(|e| e.as_object().is_some_and(|o| str_of(o, "stationuuid").is_some()))
                .map(station_from_api)
                .collect()
        })
        .unwrap_or_default())
}

/// Like Swift's `String.capitalized`: first letter of each word upper case, the rest lower case.
pub fn capitalized(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut start = true;
    for c in s.chars() {
        if c.is_alphanumeric() || c == '\'' {
            if start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            start = false;
        } else {
            out.push(c);
            start = true;
        }
    }
    out
}

// MARK: sorting

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationSorting {
    Name,
    Language,
    Country,
    State,
    Codec,
    Votes,
    Bitrate,
}

impl StationSorting {
    pub const ALL: [StationSorting; 7] = [
        StationSorting::Name,
        StationSorting::Language,
        StationSorting::Country,
        StationSorting::State,
        StationSorting::Codec,
        StationSorting::Votes,
        StationSorting::Bitrate,
    ];

    pub fn raw_value(self) -> &'static str {
        match self {
            StationSorting::Name => "name",
            StationSorting::Language => "language",
            StationSorting::Country => "country",
            StationSorting::State => "state",
            StationSorting::Codec => "codec",
            StationSorting::Votes => "votes",
            StationSorting::Bitrate => "bitrate",
        }
    }

    pub fn from_raw_value(raw: &str) -> Option<StationSorting> {
        Self::ALL.into_iter().find(|s| s.raw_value() == raw)
    }

    pub fn title(self) -> &'static str {
        match self {
            StationSorting::Name => "Name",
            StationSorting::Language => "Language",
            StationSorting::Country => "Country",
            StationSorting::State => "State",
            StationSorting::Codec => "Codec",
            StationSorting::Votes => "Votes",
            StationSorting::Bitrate => "Bitrate",
        }
    }
}

/// Like Foundation's `localizedStandardCompare`: case-insensitive, runs of digits compared by value.
pub fn localized_standard_compare(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(&c) = it.peek() {
                        if !c.is_ascii_digit() {
                            break;
                        }
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (n, m) = (take(&mut x), take(&mut y));
                let (n, m) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let ord = n.len().cmp(&m.len()).then_with(|| n.cmp(m));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(c), Some(d)) => {
                let ord = fold(c).cmp(&fold(d));
                if ord != Ordering::Equal {
                    return ord;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// Case- and accent-insensitive key for one character (enough for station names).
fn fold(c: char) -> String {
    let base = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
        'ç' | 'Ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
        'ñ' | 'Ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
        'ý' | 'ÿ' | 'Ý' => 'y',
        _ => c,
    };
    base.to_lowercase().collect()
}

/// Stations sorted by `sorting`, ties broken by name; stable, like Swift's sort.
pub fn sorted_by(stations: &[Station], sorting: StationSorting, ascending: bool) -> Vec<Station> {
    let primary = |a: &Station, b: &Station| match sorting {
        StationSorting::Name => localized_standard_compare(&a.name, &b.name),
        StationSorting::Language => localized_standard_compare(&a.language, &b.language),
        StationSorting::Country => localized_standard_compare(&a.country, &b.country),
        StationSorting::State => localized_standard_compare(&a.state, &b.state),
        StationSorting::Codec => localized_standard_compare(&a.codec, &b.codec),
        StationSorting::Votes => a.votes.cmp(&b.votes),
        StationSorting::Bitrate => a.bitrate.cmp(&b.bitrate),
    };
    let mut v = stations.to_vec();
    v.sort_by(|a, b| match primary(a, b) {
        Ordering::Equal => localized_standard_compare(&a.name, &b.name),
        p if ascending => p,
        p => p.reverse(),
    });
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_keeps_every_field_and_sorts_keys() {
        let mut s = Station::new("abc", "Radio", "http://x/stream");
        s.tags = vec!["jazz".into(), "chill".into()];
        s.latitude = Some(52.5);
        s.longitude = Some(13.0);
        s.custom_cover = Some("c.png".into());
        s.bitrate = 128;
        let v = s.to_json();
        let text = serde_json::to_string(&v).unwrap();
        assert!(text.starts_with(r#"{"bitrate":128,"codec":"","#));
        assert!(text.contains(r#""longitude":13,"#));
        assert!(!text.contains("homepage"));
        assert_eq!(Station::from_json(&v).unwrap(), s);
    }

    #[test]
    fn lenient_reading_like_the_mac() {
        let v: Value = serde_json::from_str(
            r#"{"id":"1","name":"N","streamURL":"u","bitrate":"96","isHLS":1,"tags":["a",null,"b"],"votes":3.9}"#,
        )
        .unwrap();
        let s = Station::from_json(&v).unwrap();
        assert_eq!((s.bitrate, s.votes, s.is_hls), (96, 3, true));
        assert_eq!(s.tags, vec!["a", "b"]);
        let bad: Value = serde_json::from_str(r#"{"id":"1","name":"N","streamURL":"u","tags":["a",1]}"#).unwrap();
        assert!(Station::from_json(&bad).unwrap().tags.is_empty());
        let missing: Value = serde_json::from_str(r#"{"id":"1","name":"N"}"#).unwrap();
        assert!(Station::from_json(&missing).is_none());
    }

    #[test]
    fn api_station_conversion() {
        let list = stations_from_api(
            r#"[{"stationuuid":"u1","name":"  ","url":"http://a","url_resolved":"","tags":"jazz, smooth ,,",
                 "language":"english,GERMAN","codec":"UNKNOWN","bitrate":128,"hls":0,"geo_lat":null},
                {"name":"no id"}]"#,
        )
        .unwrap();
        assert_eq!(list.len(), 1);
        let s = &list[0];
        assert_eq!(s.name, "Unnamed Station");
        assert_eq!(s.stream_url, "http://a");
        assert_eq!(s.tags, vec!["jazz", "smooth"]);
        assert_eq!(s.language, "English, German");
        assert_eq!(s.codec, "");
        assert_eq!(s.latitude, None);
    }

    #[test]
    fn sorting_is_natural_and_stable() {
        let names = ["Radio 10", "radio 9", "Alpha", "émile"];
        let st: Vec<Station> = names.iter().map(|n| Station::new(*n, *n, "u")).collect();
        let sorted: Vec<String> = sorted_by(&st, StationSorting::Name, true).into_iter().map(|s| s.name).collect();
        assert_eq!(sorted, vec!["Alpha", "émile", "radio 9", "Radio 10"]);
        assert_eq!(StationSorting::from_raw_value("votes"), Some(StationSorting::Votes));
    }

    #[test]
    fn subtitle_and_local() {
        let mut s = Station::new("1", "N", "u");
        s.codec = "MP3".into();
        s.bitrate = 128;
        s.country = "Germany".into();
        assert_eq!(s.subtitle(), "MP3 · 128 kbps · Germany");
        let l = Station::local("Mine", "http://m", None);
        assert!(l.id.starts_with("local-") && l.is_local);
    }
}
