//! Client for the community station database at radio-browser.info. Port of `ShortwaveCore/RadioBrowser.cs`.
//! Blocking (ureq): the app calls it from worker threads.

use crate::station::{localized_standard_compare, num_of, stations_from_api, str_of, Station};
use serde_json::Value;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioBrowserError(pub String);

impl RadioBrowserError {
    pub fn no_server() -> Self {
        RadioBrowserError("Unable to reach the radio-browser.info station database.".into())
    }
    pub fn http(code: u16) -> Self {
        RadioBrowserError(format!("The station database answered with HTTP {code}."))
    }
}

impl std::fmt::Display for RadioBrowserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RadioBrowserError {}

pub type Result<T> = std::result::Result<T, RadioBrowserError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOrder {
    Votes,
    Clickcount,
    Name,
    Bitrate,
    Random,
}

impl SearchOrder {
    pub const ALL: [SearchOrder; 5] =
        [SearchOrder::Votes, SearchOrder::Clickcount, SearchOrder::Name, SearchOrder::Bitrate, SearchOrder::Random];

    pub fn raw_value(self) -> &'static str {
        match self {
            SearchOrder::Votes => "votes",
            SearchOrder::Clickcount => "clickcount",
            SearchOrder::Name => "name",
            SearchOrder::Bitrate => "bitrate",
            SearchOrder::Random => "random",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            SearchOrder::Votes => "Most Voted",
            SearchOrder::Clickcount => "Most Played",
            SearchOrder::Name => "Name",
            SearchOrder::Bitrate => "Bitrate",
            SearchOrder::Random => "Random",
        }
    }

    fn reverse(self) -> bool {
        self != SearchOrder::Name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StationSearch {
    pub name: String,
    pub tag: String,
    pub country_code: String,
    pub language: String,
    pub order: SearchOrder,
    pub limit: u32,
}

impl Default for StationSearch {
    fn default() -> Self {
        StationSearch {
            name: String::new(),
            tag: String::new(),
            country_code: String::new(),
            language: String::new(),
            order: SearchOrder::Votes,
            limit: 150,
        }
    }
}

impl StationSearch {
    pub fn is_empty(&self) -> bool {
        self.name.is_empty() && self.tag.is_empty() && self.country_code.is_empty() && self.language.is_empty()
    }

    /// The query parameters of `/json/stations/search` for this search.
    pub fn query(&self) -> Vec<(String, String)> {
        let mut q = vec![
            ("order".to_string(), self.order.raw_value().to_string()),
            ("reverse".into(), self.order.reverse().to_string()),
            ("limit".into(), self.limit.to_string()),
            ("hidebroken".into(), "true".into()),
        ];
        for (k, v) in [("name", &self.name), ("tag", &self.tag), ("countrycode", &self.country_code), ("language", &self.language)] {
            if !v.is_empty() {
                q.push((k.into(), v.clone()));
            }
        }
        q
    }
}

/// A country, language or tag with its station count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facet {
    pub name: String,
    pub station_count: i32,
    pub iso_3166_1: Option<String>,
}

impl Facet {
    pub fn id(&self) -> &str {
        self.iso_3166_1.as_deref().unwrap_or(&self.name)
    }
}

/// "Driftwave/1.0 (Windows)", "(macOS)" or "(Linux)".
pub const USER_AGENT: &str = if cfg!(windows) {
    "Driftwave/1.0 (Windows)"
} else if cfg!(target_os = "macos") {
    "Driftwave/1.0 (macOS)"
} else {
    "Driftwave/1.0 (Linux)"
};
pub const LOOKUP_HOST: &str = "all.api.radio-browser.info";
pub const FALLBACK_SERVERS: [&str; 3] = ["de1.api.radio-browser.info", "de2.api.radio-browser.info", "fi1.api.radio-browser.info"];

/// Like `Uri.EscapeDataString`: everything but unreserved characters is percent-encoded (UTF-8).
pub fn escape_data_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

pub fn query_string(query: &[(String, String)]) -> String {
    if query.is_empty() {
        return String::new();
    }
    let parts: Vec<String> =
        query.iter().map(|(k, v)| format!("{}={}", escape_data_string(k), escape_data_string(v))).collect();
    format!("?{}", parts.join("&"))
}

/// A Fisher-Yates shuffle seeded from the OS random source (via a v4 UUID); no extra crate needed.
pub(crate) fn shuffle<T>(v: &mut [T]) {
    let mut x = uuid::Uuid::new_v4().as_u128() as u64 | 1;
    for i in (1..v.len()).rev() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        v.swap(i, (x % (i as u64 + 1)) as usize);
    }
}

pub struct RadioBrowser {
    agent: ureq::Agent,
    servers: Mutex<Vec<String>>,
}

impl Default for RadioBrowser {
    fn default() -> Self {
        Self::new(Duration::from_secs(15))
    }
}

impl RadioBrowser {
    pub fn new(timeout: Duration) -> RadioBrowser {
        let agent = ureq::AgentBuilder::new().timeout(timeout).user_agent(USER_AGENT).build();
        RadioBrowser { agent, servers: Mutex::new(Vec::new()) }
    }

    // MARK: Queries

    pub fn search(&self, s: &StationSearch) -> Result<Vec<Station>> {
        self.stations("/json/stations/search", &s.query())
    }

    /// Popular stations for a country, with vote-botted entries removed, shuffled down to `count`.
    pub fn popular(&self, country_code: &str, count: usize) -> Result<Vec<Station>> {
        let all = self.stations(
            "/json/stations/search",
            &pairs(&[("countrycode", country_code), ("order", "votes"), ("reverse", "true"), ("limit", "100"), ("hidebroken", "true")]),
        )?;
        // Anything with more than 50k votes is almost certainly botted.
        let mut list: Vec<Station> = all.into_iter().filter(|s| s.votes < 50_000).collect();
        shuffle(&mut list);
        list.truncate(count);
        Ok(list)
    }

    pub fn random_stations(&self, country_code: &str, count: usize) -> Result<Vec<Station>> {
        self.stations(
            "/json/stations/search",
            &pairs(&[("countrycode", country_code), ("order", "random"), ("limit", &count.to_string()), ("hidebroken", "true")]),
        )
    }

    pub fn trending(&self, count: usize) -> Result<Vec<Station>> {
        let mut all = self.stations(
            "/json/stations/search",
            &pairs(&[("order", "clicktrend"), ("reverse", "true"), ("limit", "60"), ("hidebroken", "true")]),
        )?;
        all.truncate(count);
        Ok(all)
    }

    pub fn stations_by_uuids(&self, uuids: &[String]) -> Result<Vec<Station>> {
        let mut result = Vec::new();
        for chunk in uuids.chunks(50) {
            result.extend(self.stations("/json/stations/byuuid", &pairs(&[("uuids", &chunk.join(","))]))?);
        }
        Ok(result)
    }

    pub fn countries(&self) -> Result<Vec<Facet>> {
        let mut list: Vec<Facet> = self
            .facets("/json/countries", &pairs(&[("hidebroken", "true")]))?
            .into_iter()
            .filter(|f| f.station_count > 0 && f.iso_3166_1.as_deref().is_some_and(|i| !i.is_empty()))
            .collect();
        list.sort_by(|a, b| localized_standard_compare(&a.name, &b.name));
        Ok(list)
    }

    pub fn languages(&self) -> Result<Vec<Facet>> {
        Ok(self
            .facets("/json/languages", &pairs(&[("hidebroken", "true"), ("order", "stationcount"), ("reverse", "true")]))?
            .into_iter()
            .filter(|f| f.station_count >= 5 && !f.name.is_empty())
            .collect())
    }

    pub fn tags(&self) -> Result<Vec<Facet>> {
        Ok(self
            .facets("/json/tags", &pairs(&[("hidebroken", "true"), ("order", "stationcount"), ("reverse", "true"), ("limit", "300")]))?
            .into_iter()
            .filter(|f| !f.name.is_empty())
            .collect())
    }

    /// Tells the database a station was played (it uses this for its popularity statistics). Errors are ignored.
    pub fn count_click(&self, station_id: &str) {
        let _ = self.data(&format!("/json/url/{}", escape_data_string(station_id)), &[]);
    }

    // MARK: Transport

    fn stations(&self, path: &str, query: &[(String, String)]) -> Result<Vec<Station>> {
        let json = self.data(path, query)?;
        let list = stations_from_api(&json).map_err(|e| RadioBrowserError(e.to_string()))?;
        Ok(list.into_iter().filter(|s| !s.stream_url.is_empty()).collect())
    }

    fn facets(&self, path: &str, query: &[(String, String)]) -> Result<Vec<Facet>> {
        let json = self.data(path, query)?;
        parse_facets(&json)
    }

    fn data(&self, path: &str, query: &[(String, String)]) -> Result<String> {
        let list = {
            let mut servers = self.servers.lock().unwrap_or_else(|e| e.into_inner());
            if servers.is_empty() {
                *servers = self.discover_servers();
            }
            servers.clone()
        };
        let mut last_error = RadioBrowserError::no_server();
        for (index, host) in list.iter().enumerate() {
            let url = format!("https://{host}{path}{}", query_string(query));
            match self.agent.get(&url).call() {
                Ok(resp) => match resp.into_string() {
                    Ok(body) => {
                        if index > 0 {
                            let mut servers = self.servers.lock().unwrap_or_else(|e| e.into_inner());
                            servers.retain(|s| s != host);
                            servers.insert(0, host.clone());
                        }
                        return Ok(body);
                    }
                    Err(e) => last_error = RadioBrowserError(e.to_string()),
                },
                Err(ureq::Error::Status(code, _)) => last_error = RadioBrowserError::http(code),
                Err(e) => last_error = RadioBrowserError(e.to_string()),
            }
        }
        Err(last_error)
    }

    fn discover_servers(&self) -> Vec<String> {
        let found = self
            .agent
            .get(&format!("https://{LOOKUP_HOST}/json/servers"))
            .call()
            .ok()
            .and_then(|r| r.into_string().ok())
            .and_then(|json| serde_json::from_str::<Value>(&json).ok())
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        let mut names: Vec<String> = Vec::new();
        for e in &found {
            if let Some(n) = e.as_object().and_then(|o| str_of(o, "name")) {
                if !names.contains(&n) {
                    names.push(n);
                }
            }
        }
        shuffle(&mut names);
        if names.is_empty() {
            return FALLBACK_SERVERS.iter().map(|s| s.to_string()).collect();
        }
        for f in FALLBACK_SERVERS {
            if !names.iter().any(|n| n == f) {
                names.push(f.to_string());
            }
        }
        names
    }
}

fn pairs(p: &[(&str, &str)]) -> Vec<(String, String)> {
    p.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// Countries, languages or tags as radio-browser.info lists them.
pub fn parse_facets(json: &str) -> Result<Vec<Facet>> {
    let v: Value = serde_json::from_str(json).map_err(|e| RadioBrowserError(e.to_string()))?;
    Ok(v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_object)
                .map(|o| Facet {
                    name: str_of(o, "name").unwrap_or_default(),
                    station_count: num_of(o, "stationcount").unwrap_or(0.0) as i32,
                    iso_3166_1: str_of(o, "iso_3166_1"),
                })
                .collect()
        })
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::station::{sorted_by, StationSorting};

    #[test]
    fn decode_api() {
        let json = r#"[{"stationuuid":"abc","name":" Test FM ","url":"http://a","url_resolved":"http://b","homepage":"","favicon":"http://f.png",
          "tags":"jazz, blues,","country":"Germany","countrycode":"DE","state":"","language":"german,english","votes":5,
          "codec":"MP3","bitrate":128,"hls":0,"lastcheckok":1,"geo_lat":null,"geo_long":null,"extra":"ignored"}]"#;
        let s = &stations_from_api(json).unwrap()[0];
        assert_eq!(s.name, "Test FM");
        assert_eq!(s.stream_url, "http://b");
        assert_eq!(s.homepage, None);
        assert_eq!(s.tags, vec!["jazz", "blues"]);
        assert_eq!(s.language, "German, English");
        assert!(!s.has_location());
        assert_eq!(s.subtitle(), "MP3 · 128 kbps · Germany");
    }

    #[test]
    fn sorting() {
        let mut a = Station::new("1", "Bravo", "x");
        a.bitrate = 64;
        a.votes = 10;
        let mut b = Station::new("2", "alpha", "x");
        b.bitrate = 128;
        b.votes = 30;
        let ids = |v: Vec<Station>| v.into_iter().map(|s| s.id).collect::<Vec<_>>();
        let both = [a, b];
        assert_eq!(ids(sorted_by(&both, StationSorting::Name, true)), vec!["2", "1"]);
        assert_eq!(ids(sorted_by(&both, StationSorting::Votes, false)), vec!["2", "1"]);
        assert_eq!(ids(sorted_by(&both, StationSorting::Bitrate, true)), vec!["1", "2"]);
    }

    #[test]
    fn search_query_and_escaping() {
        let s = StationSearch { name: "Groove Salad & co/é".into(), order: SearchOrder::Name, ..Default::default() };
        assert_eq!(
            query_string(&s.query()),
            "?order=name&reverse=false&limit=150&hidebroken=true&name=Groove%20Salad%20%26%20co%2F%C3%A9"
        );
        assert!(StationSearch::default().is_empty());
        assert_eq!(query_string(&[]), "");
    }

    #[test]
    fn facets() {
        let f = parse_facets(r#"[{"name":"Germany","iso_3166_1":"DE","stationcount":"12"},{"name":"jazz","stationcount":40},3]"#).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!((f[0].id(), f[0].station_count), ("DE", 12));
        assert_eq!(f[1].id(), "jazz");
    }

    #[test]
    fn shuffle_keeps_items() {
        let mut v: Vec<u32> = (0..50).collect();
        shuffle(&mut v);
        let mut s = v.clone();
        s.sort();
        assert_eq!(s, (0..50).collect::<Vec<_>>());
    }

    /// Talks to the real radio-browser.info; run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn live_search() {
        let rb = RadioBrowser::default();
        let list = rb.search(&StationSearch { name: "groove salad".into(), limit: 5, ..Default::default() }).unwrap();
        assert!(!list.is_empty());
    }
}
