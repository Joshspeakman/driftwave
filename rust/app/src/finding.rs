//! Finding stations: Discover (popular where you are, trending, random) and Search (name, country, language, tag,
//! order), a station's details, adding and removing library stations, adding your own stream, and importing and
//! exporting the library (docs/STATION-EXCHANGE.md). The station directory is called on background threads; a newer
//! request makes an older one's answer stale, and it's dropped.

use super::{log, App, StationCard, APP};
use crate::covers;
use driftwave_core::exchange;
use driftwave_core::radio_browser::{Facet, RadioBrowser, SearchOrder, StationSearch};
use driftwave_core::station::Station;
use driftwave_core::storage::library_file;
use slint::{ComponentHandle, Model, VecModel};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

#[derive(Default)]
pub struct Finding {
    pub discover: Vec<Station>,
    pub results: Vec<Station>,
    pub discover_cards: Rc<VecModel<StationCard>>,
    pub result_cards: Rc<VecModel<StationCard>>,
    pub mode: String,
    /// Discover's country (the computer's, to start) and Search's (any, to start).
    pub country: Option<String>,
    pub search_country: Option<String>,
    pub language: Option<String>,
    pub tag: Option<String>,
    pub order: usize,
    pub query: String,
    pub countries: Vec<Facet>,
    pub languages: Vec<Facet>,
    pub tags: Vec<Facet>,
    pub facets_asked: bool,
    /// Bumped by every directory request; an answer for an older number is stale.
    pub discover_seq: u64,
    pub search_seq: u64,
    pub details: Option<Station>,
    pub add_cover: Option<PathBuf>,
}

/// Runs `work` on a thread, then `done` with its answer on the UI thread.
fn background<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(&mut App, T) + Send + 'static) {
    let _ = std::thread::Builder::new().name("driftwave-directory".into()).spawn(move || {
        let answer = work();
        let _ = slint::invoke_from_event_loop(move || {
            APP.with(|a| {
                if let Some(app) = a.borrow().as_ref().and_then(|w| w.upgrade()) {
                    done(&mut app.borrow_mut(), answer);
                }
            });
        });
    });
}

fn directory() -> RadioBrowser {
    RadioBrowser::new(Duration::from_secs(15))
}

/// The country this computer is set to (from its locale, "en-US" → "US").
pub fn home_country() -> Option<String> {
    let locale = sys_locale::get_locale()?;
    let code = locale.split(['-', '_']).nth(1)?.to_uppercase();
    (code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic())).then_some(code)
}

/// Opens a web page in the person's browser.
pub fn open_url(url: &str) {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return;
    }
    #[cfg(windows)]
    let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

impl App {
    /// The card for any station: its cover if it has arrived, and whether it's the one playing.
    pub(crate) fn card_for(&self, s: &Station) -> StationCard {
        let (c1, c2) = covers::tile_colours(&s.name);
        let cover = self.covers.get(&s.id);
        StationCard {
            name: s.name.clone().into(),
            detail: s.subtitle().into(),
            playing: self.player.engine.is_some() && self.player.current.as_ref().is_some_and(|c| c.id == s.id),
            small: cover.as_ref().is_some_and(covers::is_small),
            has_cover: cover.is_some(),
            cover: cover.unwrap_or_default(),
            c1,
            c2,
        }
    }

    /// Where a station's cover comes from: the picture someone chose (in the covers folder), else its logo.
    pub(crate) fn cover_source(&self, s: &Station) -> Option<String> {
        match &s.custom_cover {
            Some(name) => Some(self.paths.covers().join(name).to_string_lossy().into_owned()),
            None => s.favicon_url.clone(),
        }
    }

    pub(crate) fn want_cover(&mut self, s: &Station) {
        let source = self.cover_source(s);
        self.covers.want(&s.id, source.as_deref());
    }

    /// Redraws the Discover and Search cards (a cover arrived, or the playing station changed).
    pub(crate) fn refresh_found(&self) {
        let d: Vec<StationCard> = self.find.discover.iter().map(|s| self.card_for(s)).collect();
        let r: Vec<StationCard> = self.find.results.iter().map(|s| self.card_for(s)).collect();
        for (model, cards) in [(&self.find.discover_cards, d), (&self.find.result_cards, r)] {
            if model.row_count() == cards.len() {
                for (i, c) in cards.into_iter().enumerate() {
                    model.set_row_data(i, c);
                }
            } else {
                model.set_vec(cards);
            }
        }
    }

    pub(crate) fn start_finding(&mut self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let dir = ui.global::<super::Dir>();
        dir.set_discover(self.find.discover_cards.clone().into());
        dir.set_results(self.find.result_cards.clone().into());
        self.find.mode = "popular".into();
        self.find.country = home_country();
        self.find.order = 0;
        self.load_facets();
        self.discover();
    }

    /// The country, language and tag lists for the pickers (once).
    pub(crate) fn load_facets(&mut self) {
        if self.find.facets_asked {
            return;
        }
        self.find.facets_asked = true;
        background(
            || {
                let d = directory();
                (d.countries(), d.languages(), d.tags())
            },
            |app, (countries, languages, tags)| {
                let mut countries = countries.unwrap_or_default();
                countries.sort_by_key(|f| f.name.to_lowercase());
                let mut languages = languages.unwrap_or_default();
                languages.sort_by_key(|f| -f.station_count);
                languages.truncate(120);
                languages.sort_by_key(|f| f.name.to_lowercase());
                let mut tags = tags.unwrap_or_default();
                tags.sort_by_key(|f| -f.station_count);
                tags.truncate(250);
                tags.sort_by_key(|f| f.name.to_lowercase());
                app.find.countries = countries;
                app.find.languages = languages;
                app.find.tags = tags;
                app.show_filters();
            },
        );
    }

    fn show_filters(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let dir = ui.global::<super::Dir>();
        let names = |f: &[Facet]| -> slint::ModelRc<slint::SharedString> {
            Rc::new(VecModel::from(f.iter().map(|x| slint::SharedString::from(x.name.as_str())).collect::<Vec<_>>())).into()
        };
        let index = |f: &[Facet], v: &Option<String>| v.as_ref().and_then(|v| f.iter().position(|x| x.id() == v)).map(|i| i as i32).unwrap_or(-1);
        dir.set_countries(names(&self.find.countries));
        dir.set_country(index(&self.find.countries, &self.find.country));
        dir.set_search_country(index(&self.find.countries, &self.find.search_country));
        dir.set_languages(names(&self.find.languages));
        dir.set_language(index(&self.find.languages, &self.find.language));
        dir.set_tags(names(&self.find.tags));
        dir.set_tag(index(&self.find.tags, &self.find.tag));
        dir.set_order(self.find.order as i32);
        dir.set_discover_mode(self.find.mode.clone().into());
    }

    pub(crate) fn discover(&mut self) {
        self.find.discover_seq += 1;
        let seq = self.find.discover_seq;
        let (mode, country) = (self.find.mode.clone(), self.find.country.clone().unwrap_or_default());
        if let Some(ui) = self.ui.upgrade() {
            ui.global::<super::Dir>().set_discover_status("Loading stations…".into());
        }
        log(&format!("discover {mode} {country}"));
        background(
            move || {
                let d = directory();
                match mode.as_str() {
                    "trending" => d.trending(60),
                    "random" => d.random_stations(&country, 60),
                    _ => d.popular(&country, 60),
                }
            },
            move |app, answer| {
                if seq != app.find.discover_seq {
                    return; // a newer request is on its way
                }
                let status = match answer {
                    Ok(list) => {
                        app.find.discover = list;
                        if app.find.discover.is_empty() { "No stations here. Try another country.".to_string() } else { String::new() }
                    }
                    Err(e) => {
                        app.find.discover.clear();
                        format!("Couldn't reach the station directory ({}). Try again in a moment.", e.0)
                    }
                };
                for s in app.find.discover.clone() {
                    app.want_cover(&s);
                }
                app.find.discover_cards.set_vec(Vec::new());
                app.refresh_found();
                if let Some(ui) = app.ui.upgrade() {
                    ui.global::<super::Dir>().set_discover_status(status.into());
                }
            },
        );
    }

    pub(crate) fn search(&mut self, text: &str) {
        self.find.query = text.trim().to_string();
        let order = SearchOrder::ALL[self.find.order.min(SearchOrder::ALL.len() - 1)];
        let query = StationSearch {
            name: self.find.query.clone(),
            tag: self.find.tag.clone().unwrap_or_default(),
            country_code: self.find.search_country.clone().unwrap_or_default(),
            language: self.find.language.clone().unwrap_or_default(),
            order,
            limit: 150,
        };
        let Some(ui) = self.ui.upgrade() else { return };
        let dir = ui.global::<super::Dir>();
        if query.is_empty() {
            dir.set_search_status("Type a station's name, or choose a country, language or tag.".into());
            return;
        }
        self.find.search_seq += 1;
        let seq = self.find.search_seq;
        dir.set_search_status("Searching…".into());
        log(&format!("search {:?}", query.query()));
        background(
            move || directory().search(&query),
            move |app, answer| {
                if seq != app.find.search_seq {
                    return;
                }
                let status = match answer {
                    Ok(list) => {
                        app.find.results = list;
                        if app.find.results.is_empty() { "No stations found.".to_string() } else { String::new() }
                    }
                    Err(e) => {
                        app.find.results.clear();
                        format!("Couldn't reach the station directory ({}). Try again in a moment.", e.0)
                    }
                };
                for s in app.find.results.clone() {
                    app.want_cover(&s);
                }
                app.find.result_cards.set_vec(Vec::new());
                app.refresh_found();
                if let Some(ui) = app.ui.upgrade() {
                    ui.global::<super::Dir>().set_search_status(status.into());
                }
            },
        );
    }

    pub(crate) fn filter_chosen(&mut self, kind: &str, i: i32) {
        let pick = |f: &[Facet]| usize::try_from(i).ok().and_then(|i| f.get(i)).map(|x| x.id().to_string());
        match kind {
            "country" => self.find.search_country = pick(&self.find.countries),
            "language" => self.find.language = pick(&self.find.languages).map(|l| l.to_lowercase()),
            "tag" => self.find.tag = pick(&self.find.tags),
            _ => self.find.order = usize::try_from(i).unwrap_or(0),
        }
        // The language facet's id is its name; the search wants it as listed.
        if kind == "language" {
            self.find.language = usize::try_from(i).ok().and_then(|i| self.find.languages.get(i)).map(|x| x.name.clone());
        }
        self.show_filters();
        let q = self.find.query.clone();
        self.search(&q);
    }

    pub(crate) fn country_chosen(&mut self, i: i32) {
        self.find.country = usize::try_from(i).ok().and_then(|i| self.find.countries.get(i)).map(|x| x.id().to_string());
        self.show_filters();
        self.discover();
    }

    fn station_from(&self, source: &str, i: i32) -> Option<Station> {
        let i = usize::try_from(i).ok()?;
        match source {
            "library" => self.stations.get(i),
            "discover" => self.find.discover.get(i),
            _ => self.find.results.get(i),
        }
        .cloned()
    }

    pub(crate) fn open_details(&mut self, source: &str, i: i32) {
        let Some(s) = self.station_from(source, i) else { return };
        self.want_cover(&s);
        self.find.details = Some(s);
        self.show_details();
    }

    pub(crate) fn show_details(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let dir = ui.global::<super::Dir>();
        let Some(s) = &self.find.details else {
            dir.set_details_open(false);
            return;
        };
        let card = self.card_for(s);
        let place: Vec<&str> = [s.state.as_str(), s.country.as_str()].into_iter().filter(|x| !x.is_empty()).collect();
        let audio = match (s.codec.is_empty(), s.bitrate > 0) {
            (false, true) => format!("{} · {} kbps", s.codec, s.bitrate),
            (false, false) => s.codec.clone(),
            (true, true) => format!("{} kbps", s.bitrate),
            _ => String::new(),
        };
        dir.set_details(super::Details {
            name: s.name.clone().into(),
            subtitle: s.subtitle().into(),
            tags: s.tags.join(", ").into(),
            location: place.join(", ").into(),
            language: s.language.clone().into(),
            codec: audio.into(),
            votes: if s.votes > 0 { s.votes.to_string().into() } else { "".into() },
            homepage: s.homepage.clone().unwrap_or_default().into(),
            stream: s.stream_url.clone().into(),
            in_library: self.stations.iter().any(|x| x.id == s.id),
            local: s.is_local,
            orphaned: s.is_orphaned,
            playing: card.playing,
            cover: card.cover,
            has_cover: card.has_cover,
            small: card.small,
            c1: card.c1,
            c2: card.c2,
        });
        dir.set_details_open(true);
    }

    pub(crate) fn details_play(&mut self) {
        let Some(s) = self.find.details.clone() else { return };
        let playing = self.player.engine.is_some() && self.player.current.as_ref().is_some_and(|c| c.id == s.id);
        if playing {
            self.stop();
        } else {
            self.play_station(s);
        }
        self.show_details();
    }

    pub(crate) fn details_library(&mut self) {
        let Some(s) = self.find.details.clone() else { return };
        if let Some(i) = self.stations.iter().position(|x| x.id == s.id) {
            self.stations.remove(i);
            self.message(&format!("Removed {} from your library.", s.name));
        } else {
            self.stations.push(s.clone());
            self.message(&format!("Added {} to your library.", s.name));
        }
        self.save_library();
        self.resort();
        self.show_details();
    }

    /// Saves the library (to this app's own data folder; a test run's is its test folder).
    pub(crate) fn save_library(&self) {
        if let Err(e) = library_file::save(&self.stations, &self.paths.library()) {
            self.message(&format!("Couldn't save your library: {e}"));
        }
    }

    pub(crate) fn message(&self, text: &str) {
        if let Some(ui) = self.ui.upgrade() {
            ui.global::<super::Dir>().set_message(text.into());
        }
    }

    pub(crate) fn choose_cover(&mut self) {
        let picked = rfd::FileDialog::new().add_filter("Pictures", &["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico"]).pick_file();
        if let Some(path) = picked {
            if let Some(ui) = self.ui.upgrade() {
                ui.global::<super::Dir>().set_add_cover(path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().into());
            }
            self.find.add_cover = Some(path);
        }
    }

    pub(crate) fn add_local(&mut self, name: &str, url: &str) {
        let Some(ui) = self.ui.upgrade() else { return };
        let dir = ui.global::<super::Dir>();
        let (name, url) = (name.trim(), url.trim());
        if name.is_empty() {
            dir.set_add_error("Give the station a name.".into());
            return;
        }
        if url::Url::parse(url).map(|u| !matches!(u.scheme(), "http" | "https")).unwrap_or(true) {
            dir.set_add_error("The stream address should start with http:// or https://.".into());
            return;
        }
        // A chosen cover is copied into the covers folder under a new name.
        let cover = self.find.add_cover.take().and_then(|src| {
            let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "png".into());
            let file = format!("{}.{ext}", uuid::Uuid::new_v4());
            let _ = std::fs::create_dir_all(self.paths.covers());
            std::fs::copy(&src, self.paths.covers().join(&file)).ok().map(|_| file)
        });
        let station = Station::local(name, url, cover);
        self.stations.push(station.clone());
        self.save_library();
        self.want_cover(&station);
        self.resort();
        dir.set_add_error("".into());
        dir.set_add_cover("".into());
        dir.set_add_open(false);
        self.message(&format!("Added {name} to your library."));
    }

    pub(crate) fn import_stations(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Station files", &["json"]).pick_file() else { return };
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => return self.message(&format!("Couldn't read that file: {e}")),
        };
        let contents = match exchange::decode(&data) {
            Ok(c) => c,
            Err(e) => return self.message(&format!("That isn't a station file Driftwave can read ({e}).")),
        };
        let merged = exchange::merge(&contents, &self.stations, &mut || uuid::Uuid::new_v4().to_string());
        let _ = std::fs::create_dir_all(self.paths.covers());
        for (file, bytes) in &merged.new_covers {
            let _ = std::fs::write(self.paths.covers().join(file), bytes);
        }
        let added = merged.added.len();
        self.stations = merged.library;
        self.save_library();
        for s in self.stations.clone() {
            self.want_cover(&s);
        }
        self.resort();
        let mut text = format!("Imported {added} station{}", if added == 1 { "" } else { "s" });
        if merged.already_present > 0 {
            text += &format!("; {} already in your library", merged.already_present);
        }
        if merged.unreadable > 0 {
            text += &format!("; {} couldn't be read", merged.unreadable);
        }
        self.message(&(text + "."));
    }

    pub(crate) fn export_stations(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Station files", &["json"]).set_file_name("Driftwave Stations.json").save_file() else { return };
        let covers_dir = self.paths.covers();
        let data = exchange::encode(&self.stations, &format!("Driftwave {}", env!("CARGO_PKG_VERSION")), None, &|name| std::fs::read(covers_dir.join(name)).ok());
        match std::fs::write(&path, data) {
            Ok(()) => self.message(&format!("Exported {} stations.", self.stations.len())),
            Err(e) => self.message(&format!("Couldn't save the station file: {e}")),
        }
    }
}
