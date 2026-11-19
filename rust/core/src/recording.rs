//! Cutting a live stream into one file per track. Port of `ShortwaveCore/Recording.cs`.
//!
//! The recording states, what each is called and how it's explained, follow GNOME Shortwave's
//! (`src/audio/recording_state.rs`, Copyright (C) 2024-2025 Felix Häcker, GPL-3.0-or-later), from which Driftwave's
//! recording design is drawn.

use crate::icy::RecordableFormat;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingMode {
    Everything,
    Decide,
    Nothing,
}

impl RecordingMode {
    pub const ALL: [RecordingMode; 3] = [RecordingMode::Everything, RecordingMode::Decide, RecordingMode::Nothing];

    /// The Swift rawValue, also the settings value: "everything", "decide", "nothing".
    pub fn raw_value(self) -> &'static str {
        match self {
            RecordingMode::Everything => "everything",
            RecordingMode::Decide => "decide",
            RecordingMode::Nothing => "nothing",
        }
    }

    pub fn from_raw_value(s: &str) -> Option<RecordingMode> {
        Self::ALL.into_iter().find(|m| m.raw_value() == s)
    }

    pub fn title(self) -> &'static str {
        match self {
            RecordingMode::Everything => "Save All Tracks",
            RecordingMode::Decide => "Decide for Each Track",
            RecordingMode::Nothing => "Record Nothing",
        }
    }

    pub fn explanation(self) -> &'static str {
        match self {
            RecordingMode::Everything => "All recorded tracks are automatically saved in the track folder.",
            RecordingMode::Decide => "Tracks are temporarily recorded and can be saved if desired.",
            RecordingMode::Nothing => "Stations are played without recording.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingState {
    IdleDisabled,
    IdleIncomplete,
    IdleUnsupported,
    Recording,
    Recorded,
    RecordedReachedMaxDuration,
    DiscardedBelowMinDuration,
    DiscardedCancelled,
    DiscardedStreamFailure,
}

impl RecordingState {
    pub fn include_in_past_tracks(self) -> bool {
        self != RecordingState::DiscardedBelowMinDuration
    }

    pub fn is_recorded(self) -> bool {
        matches!(self, RecordingState::Recorded | RecordingState::RecordedReachedMaxDuration)
    }

    /// The Swift rawValue (lower camel case), used in log lines the test scripts grep.
    pub fn raw_value(self) -> &'static str {
        match self {
            RecordingState::IdleDisabled => "idleDisabled",
            RecordingState::IdleIncomplete => "idleIncomplete",
            RecordingState::IdleUnsupported => "idleUnsupported",
            RecordingState::Recording => "recording",
            RecordingState::Recorded => "recorded",
            RecordingState::RecordedReachedMaxDuration => "recordedReachedMaxDuration",
            RecordingState::DiscardedBelowMinDuration => "discardedBelowMinDuration",
            RecordingState::DiscardedCancelled => "discardedCancelled",
            RecordingState::DiscardedStreamFailure => "discardedStreamFailure",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            RecordingState::IdleDisabled | RecordingState::IdleIncomplete | RecordingState::IdleUnsupported => "Not Recorded",
            RecordingState::Recording => "Recording…",
            RecordingState::Recorded | RecordingState::RecordedReachedMaxDuration => "Recorded",
            RecordingState::DiscardedBelowMinDuration => "Below Threshold",
            RecordingState::DiscardedCancelled => "Cancelled",
            RecordingState::DiscardedStreamFailure => "Interrupted",
        }
    }

    pub fn explanation(self) -> &'static str {
        match self {
            RecordingState::IdleDisabled => "Recording is turned off in Settings.",
            RecordingState::IdleIncomplete => "The track wasn’t played from the beginning, so it can’t be fully recorded.",
            RecordingState::IdleUnsupported => "This station’s stream format can’t be recorded.",
            RecordingState::Recording => "The track will be recorded until a new track gets played.",
            RecordingState::Recorded => "The track has been temporarily recorded.",
            RecordingState::RecordedReachedMaxDuration => "The maximum recording duration has been reached.",
            RecordingState::DiscardedBelowMinDuration => {
                "The track has been discarded as it was shorter than the minimum duration."
            }
            RecordingState::DiscardedCancelled => "Recording has been cancelled.",
            RecordingState::DiscardedStreamFailure => "The stream was interrupted, so the recording was discarded.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecorderConfig {
    pub mode: RecordingMode,
    /// Seconds.
    pub minimum_duration: f64,
    /// Seconds; 0 means no limit.
    pub maximum_duration: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecorderEvent {
    /// A new track began. `file` is set when its audio is being recorded.
    Started { id: Uuid, title: String, state: RecordingState, file: Option<PathBuf> },
    /// Recording of a track ended with `state`; `duration` in seconds.
    Finished { id: Uuid, state: RecordingState, duration: f64 },
}

/// Seconds on some monotonic-enough clock; tests supply their own.
pub type Clock = Box<dyn Fn() -> f64 + Send>;

fn wall_clock() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// Cuts a live stream into one file per track, driven by the stream's title changes. Runs on the stream's own
/// thread so a cut happens at the exact byte where the title changed.
pub struct SegmentRecorder {
    pub config: RecorderConfig,
    format: Option<RecordableFormat>,
    directory: PathBuf,
    clock: Clock,
    had_track: bool,
    current_id: Option<Uuid>,
    current_title: Option<String>,
    handle: Option<File>,
    file_path: Option<PathBuf>,
    started_at: f64,
}

impl SegmentRecorder {
    pub fn new(config: RecorderConfig, format: Option<RecordableFormat>, directory: impl Into<PathBuf>, clock: Option<Clock>) -> Self {
        let clock = clock.unwrap_or_else(|| Box::new(wall_clock));
        let started_at = clock();
        SegmentRecorder {
            config,
            format,
            directory: directory.into(),
            clock,
            had_track: false,
            current_id: None,
            current_title: None,
            handle: None,
            file_path: None,
            started_at,
        }
    }

    pub fn format(&self) -> Option<RecordableFormat> {
        self.format
    }

    pub fn is_recording(&self) -> bool {
        self.handle.is_some()
    }

    pub fn handle_title(&mut self, raw_title: &str) -> Vec<RecorderEvent> {
        let title = raw_title.trim();
        if title.is_empty() || self.current_title.as_deref() == Some(title) {
            return Vec::new();
        }
        let mut events = self.finish_current(false);
        let id = Uuid::new_v4();
        let mut file = None;
        let state = if self.config.mode == RecordingMode::Nothing {
            RecordingState::IdleDisabled
        } else if self.format.is_none() {
            RecordingState::IdleUnsupported
        } else if !self.had_track {
            // The first title we see started before we tuned in.
            RecordingState::IdleIncomplete
        } else {
            let ext = self.format.expect("checked").file_extension();
            let path = self.directory.join(format!("{}.{ext}", id.to_string().to_uppercase()));
            match create_new(&self.directory, &path) {
                Ok(h) => {
                    self.handle = Some(h);
                    self.file_path = Some(path.clone());
                    file = Some(path);
                    RecordingState::Recording
                }
                Err(_) => {
                    self.handle = None;
                    RecordingState::IdleDisabled
                }
            }
        };
        self.had_track = true;
        self.current_id = Some(id);
        self.current_title = Some(title.to_owned());
        self.started_at = (self.clock)();
        events.push(RecorderEvent::Started { id, title: title.to_owned(), state, file });
        events
    }

    pub fn handle_audio(&mut self, data: &[u8]) -> Vec<RecorderEvent> {
        let Some(h) = self.handle.as_mut() else { return Vec::new() };
        let _ = h.write_all(data);
        if self.config.maximum_duration > 0.0 && (self.clock)() - self.started_at >= self.config.maximum_duration {
            return self.close(RecordingState::RecordedReachedMaxDuration, false);
        }
        Vec::new()
    }

    /// The user cancelled the current recording.
    pub fn cancel(&mut self) -> Vec<RecorderEvent> {
        self.finish_current(true)
    }

    /// The stream ended or failed: whatever was being recorded is incomplete.
    pub fn stream_ended(&mut self) -> Vec<RecorderEvent> {
        let events = if self.handle.is_some() { self.close(RecordingState::DiscardedStreamFailure, true) } else { Vec::new() };
        self.had_track = false;
        self.current_title = None;
        self.current_id = None;
        events
    }

    fn finish_current(&mut self, cancelled: bool) -> Vec<RecorderEvent> {
        if self.handle.is_none() {
            return Vec::new();
        }
        if cancelled {
            return self.close(RecordingState::DiscardedCancelled, true);
        }
        if (self.clock)() - self.started_at < self.config.minimum_duration {
            return self.close(RecordingState::DiscardedBelowMinDuration, true);
        }
        self.close(RecordingState::Recorded, false)
    }

    fn close(&mut self, state: RecordingState, discard: bool) -> Vec<RecorderEvent> {
        let (Some(h), Some(id)) = (self.handle.take(), self.current_id) else { return Vec::new() };
        drop(h);
        let duration =
            if discard && state == RecordingState::DiscardedCancelled { 0.0 } else { (self.clock)() - self.started_at };
        if let Some(path) = self.file_path.take() {
            if discard {
                let _ = fs::remove_file(path);
            }
        }
        vec![RecorderEvent::Finished { id, state, duration }]
    }
}

fn create_new(dir: &Path, path: &Path) -> std::io::Result<File> {
    fs::create_dir_all(dir)?;
    File::options().write(true).create_new(true).open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_dir::TempDir;
    use std::sync::{Arc, Mutex};

    fn make(mode: RecordingMode, min: f64, max: f64, format: Option<RecordableFormat>) -> (SegmentRecorder, Arc<Mutex<f64>>, TempDir) {
        let dir = TempDir::new("rec");
        let now = Arc::new(Mutex::new(0.0));
        let n = now.clone();
        let r = SegmentRecorder::new(
            RecorderConfig { mode, minimum_duration: min, maximum_duration: max },
            format,
            dir.path().join("Recording"),
            Some(Box::new(move || *n.lock().unwrap())),
        );
        (r, now, dir)
    }

    fn standard() -> (SegmentRecorder, Arc<Mutex<f64>>, TempDir) {
        make(RecordingMode::Decide, 30.0, 900.0, Some(RecordableFormat::Mp3))
    }

    fn start_state(e: &[RecorderEvent]) -> Option<RecordingState> {
        e.iter().find_map(|x| match x {
            RecorderEvent::Started { state, .. } => Some(*state),
            _ => None,
        })
    }

    fn finish_state(e: &[RecorderEvent]) -> Option<RecordingState> {
        e.iter().find_map(|x| match x {
            RecorderEvent::Finished { state, .. } => Some(*state),
            _ => None,
        })
    }

    fn started_file(e: &[RecorderEvent]) -> Option<PathBuf> {
        match e.last() {
            Some(RecorderEvent::Started { file, .. }) => file.clone(),
            _ => None,
        }
    }

    #[test]
    fn first_track_is_incomplete_then_records() {
        let (mut r, clock, _dir) = standard();
        assert_eq!(start_state(&r.handle_title("One")), Some(RecordingState::IdleIncomplete));
        assert!(r.handle_title("One").is_empty(), "a repeated title is not a change");
        r.handle_audio(&[1, 2, 3]);
        let e = r.handle_title("Two");
        assert_eq!(start_state(&e), Some(RecordingState::Recording));
        let file = started_file(&e).expect("recording to a file");
        r.handle_audio(&[4, 5]);
        *clock.lock().unwrap() += 60.0;
        r.handle_audio(&[6]);
        assert_eq!(finish_state(&r.handle_title("Three")), Some(RecordingState::Recorded));
        assert_eq!(fs::read(&file).unwrap(), vec![4, 5, 6]);
        r.stream_ended();
    }

    #[test]
    fn below_minimum_is_discarded() {
        let (mut r, clock, _dir) = standard();
        r.handle_title("One");
        let file = started_file(&r.handle_title("Two")).unwrap();
        *clock.lock().unwrap() += 5.0;
        assert_eq!(finish_state(&r.handle_title("Three")), Some(RecordingState::DiscardedBelowMinDuration));
        assert!(!file.exists());
        r.stream_ended();
    }

    #[test]
    fn maximum_duration_stops_recording() {
        let (mut r, clock, _dir) = make(RecordingMode::Decide, 30.0, 100.0, Some(RecordableFormat::Mp3));
        r.handle_title("One");
        r.handle_title("Two");
        *clock.lock().unwrap() += 101.0;
        assert_eq!(finish_state(&r.handle_audio(&[1])), Some(RecordingState::RecordedReachedMaxDuration));
        assert!(!r.is_recording());
        assert_eq!(finish_state(&r.handle_title("Three")), None);
        r.stream_ended();
    }

    #[test]
    fn cancel_and_stream_failure() {
        let (mut r, _clock, _dir) = standard();
        r.handle_title("One");
        r.handle_title("Two");
        assert_eq!(finish_state(&r.cancel()), Some(RecordingState::DiscardedCancelled));
        r.handle_title("Three");
        assert_eq!(finish_state(&r.stream_ended()), Some(RecordingState::DiscardedStreamFailure));
        assert_eq!(start_state(&r.handle_title("Four")), Some(RecordingState::IdleIncomplete));
        r.stream_ended();
    }

    #[test]
    fn disabled_and_unsupported() {
        let (mut r, _c, _d) = make(RecordingMode::Nothing, 30.0, 900.0, Some(RecordableFormat::Mp3));
        r.handle_title("One");
        assert_eq!(start_state(&r.handle_title("Two")), Some(RecordingState::IdleDisabled));
        let (mut u, _c2, _d2) = make(RecordingMode::Decide, 30.0, 900.0, None);
        u.handle_title("One");
        assert_eq!(start_state(&u.handle_title("Two")), Some(RecordingState::IdleUnsupported));
    }

    #[test]
    fn raw_values() {
        assert_eq!(RecordingState::RecordedReachedMaxDuration.raw_value(), "recordedReachedMaxDuration");
        assert_eq!(RecordingMode::from_raw_value("decide"), Some(RecordingMode::Decide));
    }
}
