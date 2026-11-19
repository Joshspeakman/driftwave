//! Now Playing and the media keys: macOS's Now Playing (MPNowPlayingInfoCenter and
//! MPRemoteCommandCenter), Windows' media overlay (SMTC) and Linux's MPRIS, through souvlaki.
//!
//! **Test runs keep this off** (`CLAUDE.md`): a muted test copy must never take the media keys from
//! the music Josh is playing. Only create a `Media` for a real run (or `-mediaControls`).
//!
//! On macOS it must be created and updated on the main thread, with the app's event loop running.

use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::ffi::c_void;
use std::sync::mpsc::{channel, Receiver};

/// What the keys and the system overlay ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCommand {
    Play,
    Pause,
    Toggle,
    Stop,
    /// Next / previous station in the library (radio has no tracks to skip).
    Next,
    Previous,
}

/// What to show in the system's Now Playing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NowPlaying<'a> {
    /// The song, or the station's name when there's no title.
    pub title: &'a str,
    pub artist: Option<&'a str>,
    pub station: &'a str,
    /// `file://` or `https://` artwork (the station cover).
    pub cover_url: Option<&'a str>,
    pub playing: bool,
}

/// The last `NowPlaying` sent, owned, to skip repeats.
#[derive(Debug, Clone, PartialEq)]
struct Shown {
    title: String,
    artist: Option<String>,
    station: String,
    cover_url: Option<String>,
    playing: bool,
}

pub struct Media {
    controls: MediaControls,
    commands: Receiver<MediaCommand>,
    last: Option<Shown>,
}

impl Media {
    /// `display_name`: "Driftwave". `dbus_name`: "driftwave" (Linux). `hwnd`: the main window (Windows).
    pub fn new(display_name: &str, dbus_name: &str, hwnd: Option<*mut c_void>) -> Result<Media, String> {
        let mut controls = MediaControls::new(PlatformConfig { display_name, dbus_name, hwnd })
            .map_err(|e| format!("media controls: {e:?}"))?;
        let (tx, commands) = channel();
        controls
            .attach(move |e| {
                let c = match e {
                    MediaControlEvent::Play => MediaCommand::Play,
                    MediaControlEvent::Pause => MediaCommand::Pause,
                    MediaControlEvent::Toggle => MediaCommand::Toggle,
                    MediaControlEvent::Stop => MediaCommand::Stop,
                    MediaControlEvent::Next => MediaCommand::Next,
                    MediaControlEvent::Previous => MediaCommand::Previous,
                    _ => return, // seeking and the rest mean nothing for live radio
                };
                let _ = tx.send(c);
            })
            .map_err(|e| format!("media controls: {e:?}"))?;
        Ok(Media { controls, commands, last: None })
    }

    /// Commands since the last call (media keys, the overlay's buttons).
    pub fn commands(&self) -> Vec<MediaCommand> {
        self.commands.try_iter().collect()
    }

    /// Shows what's playing. Cheap to call often: it only talks to the system when something changed.
    pub fn update(&mut self, now: &NowPlaying) {
        let key = Shown {
            title: now.title.to_string(),
            artist: now.artist.map(str::to_string),
            station: now.station.to_string(),
            cover_url: now.cover_url.map(str::to_string),
            playing: now.playing,
        };
        if self.last.as_ref() == Some(&key) {
            return;
        }
        let _ = self.controls.set_metadata(MediaMetadata {
            title: Some(now.title),
            artist: Some(now.artist.unwrap_or(now.station)),
            album: Some(now.station),
            cover_url: now.cover_url,
            duration: None,
        });
        let _ = self.controls.set_playback(if now.playing { MediaPlayback::Playing { progress: None } } else { MediaPlayback::Paused { progress: None } });
        self.last = Some(key);
    }

    /// Nothing selected: leave Now Playing.
    pub fn clear(&mut self) {
        let _ = self.controls.set_metadata(MediaMetadata::default());
        let _ = self.controls.set_playback(MediaPlayback::Stopped);
        self.last = None;
    }
}

impl Drop for Media {
    fn drop(&mut self) {
        let _ = self.controls.detach();
    }
}
