//! The app's side of `driftwave-platform`: the menu bar / tray icon, Now Playing and the media keys,
//! new-track notifications, and messages from a second copy (`--quit`, or just opening it again).
//!
//! It only reads the player's state and drives the window's own callbacks (play, stop, next, …), so
//! it does exactly what the buttons do.
//!
//! Test runs: no Now Playing or media keys unless `-mediaControls` (CLAUDE.md), and no tray icon unless
//! `-tray`; the single-instance check is skipped so test copies can run side by side.

use driftwave_platform::media::{Media, MediaCommand, NowPlaying};
use driftwave_platform::notify::Notifier;
use driftwave_platform::single::{self, Instance, Listener};
use driftwave_platform::tray::{Tray, TrayAction, TrayState};

/// The single-instance slot: one per data folder, so Release, Nightly and test runs never meet.
pub fn instance_name(folder: &str) -> String {
    format!("io.github.joshspeakman.{folder}")
}

/// What a second copy asks of the first, from its command line.
pub fn message_from_args() -> &'static str {
    if std::env::args().any(|a| a == "--quit" || a == "-quit") { "quit" } else { "show" }
}

/// `--quit` with no copy running: nothing to do. Returns the listener when this copy should run.
pub fn claim(folder: &str) -> Option<Listener> {
    let message = message_from_args();
    match single::claim(&instance_name(folder), message) {
        Instance::Secondary => None,
        Instance::Primary(_) if message == "quit" => None,
        Instance::Primary(l) => Some(l),
    }
}

/// What the integration asks the app to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Play,
    Stop,
    TogglePlay,
    Next,
    Prev,
    Station(usize),
    Show,
    MiniPlayer,
    Quit,
}

/// The player's state, as the integration needs it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct View {
    /// Station names in the order the app lists them.
    pub stations: Vec<String>,
    pub current: Option<usize>,
    pub station_name: Option<String>,
    /// "Artist - Title" from the stream, or empty.
    pub title: String,
    pub playing: bool,
    pub cover_url: Option<String>,
    pub notifications: bool,
}

#[derive(Default)]
pub struct Integration {
    tray: Option<Tray>,
    media: Option<Media>,
    notifier: Option<Notifier>,
    listener: Option<Listener>,
    last_title: Option<String>,
    last_view: Option<View>,
}

impl Integration {
    pub fn new(listener: Option<Listener>) -> Integration {
        Integration { listener, ..Default::default() }
    }

    /// Creates the tray icon and Now Playing; call once the window is showing (the Mac needs the event
    /// loop running and the main thread).
    pub fn start(&mut self, tray: bool, media: bool, tray_icon: (Vec<u8>, u32, u32)) {
        if tray {
            match Tray::new(tray_icon.0, tray_icon.1, tray_icon.2, "Driftwave") {
                Ok(t) => self.tray = Some(t),
                Err(e) => crate::log(&format!("tray: {e}")),
            }
        }
        if media {
            match Media::new("Driftwave", "driftwave", None) {
                Ok(m) => self.media = Some(m),
                Err(e) => crate::log(&format!("media controls: {e}")),
            }
        }
        self.notifier = Some(Notifier::new());
        crate::log(&format!("integration: tray={} media={}", self.tray.is_some(), self.media.is_some()));
    }

    /// Brings the tray menu, Now Playing and notifications up to date. Cheap when nothing changed.
    pub fn sync(&mut self, view: &View) {
        if self.last_view.as_ref() == Some(view) {
            return;
        }
        let (song, artist) = split(&view.title);
        if let Some(t) = &mut self.tray {
            t.update(&TrayState {
                station: view.station_name.clone(),
                // The song only while it's playing, as the Mac app did.
                track: (view.playing && !view.title.is_empty()).then(|| view.title.clone()),
                playing: view.playing,
                stations: view.stations.clone(),
                current: view.current,
            });
        }
        if let Some(m) = &mut self.media {
            match &view.station_name {
                Some(station) => m.update(&NowPlaying {
                    title: if song.is_empty() { station } else { &song },
                    artist: artist.as_deref(),
                    station,
                    cover_url: view.cover_url.as_deref(),
                    playing: view.playing,
                }),
                None => m.clear(),
            }
        }
        // A notification for each new song, when the setting is on.
        if view.playing && !view.title.is_empty() && self.last_title.as_deref() != Some(view.title.as_str()) {
            if let (true, Some(n), Some(station)) = (view.notifications, &self.notifier, &view.station_name) {
                n.track(&song, artist.as_deref(), station);
            }
            self.last_title = Some(view.title.clone());
        }
        self.last_view = Some(view.clone());
    }

    /// Call when the Notifications setting is turned on: asks macOS once. `done(false)` means it can't
    /// notify (not allowed, or not running as a bundled app), so the setting should go back off.
    #[allow(dead_code)] // for the Notifications toggle when Settings gains it
    pub fn request_notifications(&self, done: impl Fn(bool) + Send + Sync + 'static) {
        match &self.notifier {
            Some(n) => n.request_permission(done),
            None => done(false),
        }
    }

    /// Requests since the last call: the tray menu, media keys, and other copies of the app.
    pub fn actions(&mut self) -> Vec<Action> {
        let mut out = Vec::new();
        if let Some(t) = &self.tray {
            out.extend(t.actions().into_iter().map(|a| match a {
                TrayAction::TogglePlay => Action::TogglePlay,
                TrayAction::Station(i) => Action::Station(i),
                TrayAction::Show => Action::Show,
                TrayAction::MiniPlayer => Action::MiniPlayer,
                TrayAction::Quit => Action::Quit,
            }));
        }
        if let Some(m) = &self.media {
            out.extend(m.commands().into_iter().map(|c| match c {
                MediaCommand::Play => Action::Play,
                // Live radio: pause stops, and play picks up live again.
                MediaCommand::Pause | MediaCommand::Stop => Action::Stop,
                MediaCommand::Toggle => Action::TogglePlay,
                MediaCommand::Next => Action::Next,
                MediaCommand::Previous => Action::Prev,
            }));
        }
        if let Some(l) = &self.listener {
            out.extend(l.messages.try_iter().map(|m| if m == "quit" { Action::Quit } else { Action::Show }));
        }
        out
    }
}

fn split(t: &str) -> (String, Option<String>) {
    match t.split_once(" - ") {
        Some((a, s)) if !a.trim().is_empty() && !s.trim().is_empty() => (s.trim().to_string(), Some(a.trim().to_string())),
        _ => (t.trim().to_string(), None),
    }
}
