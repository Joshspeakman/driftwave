//! Played Tracks and recording (the C# app's TrackRow, Player.Save and its Playback and Recording settings).
//!
//! The songs heard this session, newest first, as many as Settings keeps. With recording on, the recorder writes each
//! song from its start to a temporary file. "Save All Tracks" saves every one (the engine does that); "Decide for
//! Each Track" keeps them until you press Save on the ones you want, or press it while a song is still recording to
//! save it when it ends. Temporary recordings last until Driftwave quits, or until their song drops off the list.

use super::{finding, log, App, MainWindow, Prefs, TrackRow};
use driftwave_core::recording::{RecordingMode, RecordingState};
use serde_json::{json, Value};
use slint::ComponentHandle;
use std::path::PathBuf;
use uuid::Uuid;

struct Played {
    title: String,
    station: String,
    at: String,
    id: Option<Uuid>,
    file: Option<PathBuf>,
    state: Option<RecordingState>,
    saved: bool,
    saving: bool,
    failed: bool,
    /// Pressed Save while it was recording: saved when it ends.
    save_when_done: bool,
}

#[derive(Default)]
pub(crate) struct Tracks {
    list: Vec<Played>,
    /// The recorder runs ahead of what you hear (by the buffer): songs it started before they were heard.
    pending: Vec<(Uuid, String, RecordingState, Option<PathBuf>)>,
    /// Save All: the engine's save of the song whose recording is about to end.
    just_saved: Option<Result<PathBuf, String>>,
}

/// Now, as the system writes a time without seconds ("2:46 PM", "14:46").
fn short_time() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::{GetTimeFormatEx, TIME_NOSECONDS};
        let mut buf = [0u16; 64];
        // SAFETY: a null locale and time mean the user's own and now; the buffer's length is passed.
        let n = unsafe { GetTimeFormatEx(std::ptr::null(), TIME_NOSECONDS, std::ptr::null(), std::ptr::null(), buf.as_mut_ptr(), buf.len() as i32) };
        if n > 1 {
            return String::from_utf16_lossy(&buf[..n as usize - 1]);
        }
    }
    chrono::Local::now().format("%H:%M").to_string()
}

/// "45 s", "15 min", "1 min 30 s".
fn duration_text(s: i64) -> String {
    match (s / 60, s % 60) {
        (0, 0) => "Off".into(),
        (0, s) => format!("{s} s"),
        (m, 0) => format!("{m} min"),
        (m, s) => format!("{m} min {s} s"),
    }
}

impl App {
    /// What recording does when it's on: save every song, or let you decide (the default, as in the other apps).
    pub(crate) fn record_mode_on(&self) -> RecordingMode {
        let raw = self.setting_str("recordingModeOn").or_else(|| self.setting_str("recordingMode"));
        match raw.as_deref().and_then(RecordingMode::from_raw_value) {
            Some(RecordingMode::Everything) => RecordingMode::Everything,
            _ => RecordingMode::Decide,
        }
    }

    /// Whether recording is on: the setting, or the earlier REC switch's, else on (deciding per track).
    pub(crate) fn record_armed_setting(&self) -> bool {
        match self.setting_str("recordingMode").as_deref().and_then(RecordingMode::from_raw_value) {
            Some(m) => m != RecordingMode::Nothing,
            None => self.settings.get("record").and_then(Value::as_bool).unwrap_or(true),
        }
    }

    pub(crate) fn record_mode(&self) -> RecordingMode {
        if self.player.armed {
            self.record_mode_on()
        } else {
            RecordingMode::Nothing
        }
    }

    fn setting_secs(&self, k: &str, dflt: i64) -> i64 {
        self.settings.get(k).and_then(Value::as_i64).unwrap_or(dflt)
    }

    pub(crate) fn min_secs(&self) -> f64 {
        self.setting_secs("recordingMinimumDuration", 30).clamp(0, 600) as f64
    }

    pub(crate) fn max_secs(&self) -> f64 {
        self.setting_secs("recordingMaximumDuration", 900).clamp(60, 7200) as f64
    }

    /// Where saved songs go: the folder chosen in Settings, else Music\Driftwave. A test run's stay in its own folder.
    pub(crate) fn track_dir(&self) -> PathBuf {
        let chosen = self.setting_str("trackDirectory").filter(|s| !s.is_empty()).map(PathBuf::from);
        if self.args.test_run() {
            return chosen.unwrap_or_else(|| self.paths.support.join("Tracks"));
        }
        chosen.unwrap_or_else(driftwave_core::storage::default_track_directory)
    }

    fn keep(&self) -> usize {
        self.setting_secs("pastTracksCount", 10).clamp(1, 50) as usize
    }

    /// Settings' Playback and Recording rows, from the settings.
    pub(crate) fn show_prefs(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let p = ui.global::<Prefs>();
        p.set_background(self.settings.get("backgroundPlayback").and_then(Value::as_bool).unwrap_or(true));
        p.set_notifications(self.settings.get("notifications").and_then(Value::as_bool).unwrap_or(false));
        p.set_keep(self.keep() as i32);
        let mode = self.record_mode();
        p.set_record_mode(mode.raw_value().into());
        p.set_record_note(
            match mode {
                RecordingMode::Everything => "Every song is saved to your track folder as it ends.",
                RecordingMode::Decide => "Songs are recorded for a while: press Save on the ones you want in Played Tracks.",
                RecordingMode::Nothing => "Nothing is recorded.",
            }
            .into(),
        );
        p.set_track_folder(self.track_dir().display().to_string().into());
        p.set_min_secs(self.min_secs() as i32);
        p.set_max_secs(self.max_secs() as i32);
        p.set_min_text(duration_text(self.min_secs() as i64).into());
        p.set_max_text(duration_text(self.max_secs() as i64).into());
        ui.set_record_armed(self.player.armed);
    }

    pub(crate) fn set_pref(&mut self, k: &str, v: Value) {
        self.settings[k] = v;
        self.save_settings();
        self.show_prefs();
    }

    pub(crate) fn set_keep(&mut self, n: i32) {
        self.set_pref("pastTracksCount", json!(n.clamp(1, 50)));
        self.trim_played();
        self.show_played();
    }

    /// Recording off, saving all, or deciding per track. Applies from the start of a stream, so the station that's
    /// playing starts again (as REC does).
    pub(crate) fn choose_record_mode(&mut self, raw: &str) {
        let Some(mode) = RecordingMode::from_raw_value(raw) else { return };
        if mode == self.record_mode() {
            return;
        }
        self.settings["recordingMode"] = json!(mode.raw_value());
        if mode != RecordingMode::Nothing {
            self.settings["recordingModeOn"] = json!(mode.raw_value());
        }
        self.player.armed = mode != RecordingMode::Nothing;
        self.save_settings();
        self.show_prefs();
        self.restart_for_recording();
    }

    /// REC: recording on (as last chosen) or off.
    pub(crate) fn toggle_record(&mut self) {
        let on = !self.player.armed;
        let mode = if on { self.record_mode_on() } else { RecordingMode::Nothing };
        self.settings["recordingMode"] = json!(mode.raw_value());
        self.player.armed = on;
        self.save_settings();
        self.show_prefs();
        self.restart_for_recording();
    }

    fn restart_for_recording(&mut self) {
        if let Some(s) = self.player.current.clone().filter(|_| self.player.engine.is_some()) {
            self.play_station(s);
        }
    }

    pub(crate) fn choose_track_folder(&mut self) {
        if self.args.test_run() {
            return;
        }
        let picked = rfd::FileDialog::new().set_directory(self.track_dir()).pick_folder();
        if let Some(dir) = picked {
            self.set_pref("trackDirectory", json!(dir.display().to_string()));
        }
    }

    pub(crate) fn open_track_folder(&self) {
        let dir = self.track_dir();
        let _ = std::fs::create_dir_all(&dir);
        #[cfg(windows)]
        let _ = std::process::Command::new("explorer").arg(&dir).spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&dir).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
    }

    /// A new song is audible.
    pub(crate) fn heard(&mut self, title: &str) {
        let station = self.player.current.as_ref().map(|s| s.name.clone()).unwrap_or_default();
        let mut p = Played {
            title: title.to_string(),
            station,
            at: short_time(),
            id: None,
            file: None,
            state: None,
            saved: false,
            saving: false,
            failed: false,
            save_when_done: false,
        };
        let t = &mut self.played;
        if let Some(k) = t.pending.iter().rposition(|(_, name, _, _)| name == title) {
            let (id, _, state, file) = t.pending.remove(k);
            (p.id, p.state, p.file) = (Some(id), Some(state), file);
        }
        t.list.insert(0, p);
        self.trim_played();
        self.show_played();
    }

    pub(crate) fn recorder_started(&mut self, id: Uuid, title: String, state: RecordingState, file: Option<PathBuf>) {
        log(&format!("recorder started \"{title}\" {}", state.raw_value()));
        let t = &mut self.played;
        // Normally ahead of the song being heard; matched to it when it is.
        match t.list.iter_mut().take(2).find(|p| p.id.is_none() && p.title == title) {
            Some(p) => (p.id, p.state, p.file) = (Some(id), Some(state), file),
            None => {
                t.pending.push((id, title, state, file));
                if t.pending.len() > 4 {
                    t.pending.remove(0);
                }
            }
        }
        self.show_played();
    }

    pub(crate) fn recorder_finished(&mut self, id: Uuid, state: RecordingState) {
        log(&format!("recorder finished {}", state.raw_value()));
        let saved = self.played.just_saved.take();
        let Some(i) = self.played.list.iter().position(|p| p.id == Some(id)) else {
            if let Some(k) = self.played.pending.iter().position(|(pid, ..)| *pid == id) {
                self.played.pending[k].2 = state;
            }
            return;
        };
        let p = &mut self.played.list[i];
        p.state = Some(state);
        match saved {
            Some(Ok(_)) => (p.saved, p.file) = (true, None),
            Some(Err(_)) => p.failed = true,
            None => {}
        }
        if !state.is_recorded() {
            // The recorder has already deleted it.
            p.file = None;
        }
        let save = state.is_recorded() && p.save_when_done && !p.saved;
        if save {
            self.save_played(i as i32);
        }
        self.show_played();
    }

    /// Save All: the engine saved (or failed to save) the song whose recording ends next.
    pub(crate) fn engine_saved(&mut self, result: Result<PathBuf, String>) {
        match &result {
            Ok(p) => log(&format!("saved {}", p.display())),
            Err(e) => log(&format!("save failed: {e}")),
        }
        self.played.just_saved = Some(result);
    }

    /// Played Tracks' Save: now, or when the song ends if it's still recording (pressed again, not).
    pub(crate) fn save_played(&mut self, i: i32) {
        let dir = self.track_dir();
        let Some(p) = usize::try_from(i).ok().and_then(|i| self.played.list.get_mut(i)) else { return };
        if p.saved || p.saving {
            return;
        }
        match (p.state, p.file.clone()) {
            (Some(RecordingState::Recording), _) => p.save_when_done = !p.save_when_done,
            (Some(s), Some(file)) if s.is_recorded() => {
                p.saving = true;
                p.failed = false;
                let (title, station, id) = (p.title.clone(), p.station.clone(), p.id);
                finding::background(
                    move || {
                        let out = driftwave_audio::save::save_track(&file, &title, &station, &dir);
                        if out.is_ok() {
                            let _ = std::fs::remove_file(&file);
                        }
                        out
                    },
                    move |app, out| app.played_saved(id, out),
                );
            }
            _ => {}
        }
        self.show_played();
    }

    fn played_saved(&mut self, id: Option<Uuid>, out: Result<PathBuf, String>) {
        if let Some(p) = self.played.list.iter_mut().find(|p| p.id == id && p.saving) {
            p.saving = false;
            match out {
                Ok(path) => {
                    log(&format!("saved {}", path.display()));
                    (p.saved, p.file) = (true, None);
                }
                Err(e) => {
                    log(&format!("save failed: {e}"));
                    p.failed = true;
                }
            }
        }
        self.show_played();
    }

    /// The stream stopped: what was recording won't finish.
    pub(crate) fn played_stream_stopped(&mut self) {
        let t = &mut self.played;
        for p in t.list.iter_mut().filter(|p| p.state == Some(RecordingState::Recording)) {
            p.state = Some(RecordingState::DiscardedStreamFailure);
            p.file = None;
            p.save_when_done = false;
        }
        t.pending.clear();
        t.just_saved = None;
        self.show_played();
    }

    fn trim_played(&mut self) {
        let keep = self.keep();
        while self.played.list.len() > keep {
            if let Some(p) = self.played.list.pop() {
                if let (Some(f), false) = (p.file, p.saving) {
                    let _ = std::fs::remove_file(f);
                }
            }
        }
    }

    pub(crate) fn show_played(&self) {
        let decide = self.record_mode() == RecordingMode::Decide;
        let rows: Vec<TrackRow> = self
            .played
            .list
            .iter()
            .map(|p| {
                let recorded = p.state.is_some_and(RecordingState::is_recorded) && p.file.is_some();
                let status = if p.saved {
                    "Saved"
                } else if p.saving {
                    "Saving…"
                } else if p.failed {
                    "Couldn't save"
                } else {
                    match p.state {
                        Some(RecordingState::Recording) if p.save_when_done => "Recording · saved when it ends",
                        Some(RecordingState::Recording) => "Recording",
                        _ if recorded && decide => "Not saved yet",
                        Some(RecordingState::DiscardedBelowMinDuration) => "Too short to keep",
                        Some(RecordingState::IdleIncomplete) if self.player.armed => "Joined partway · not recorded",
                        Some(RecordingState::IdleUnsupported) if self.player.armed => "Can't be recorded",
                        Some(RecordingState::DiscardedStreamFailure) if self.player.armed => "Interrupted",
                        _ => "",
                    }
                };
                TrackRow {
                    title: p.title.clone().into(),
                    note: if status.is_empty() { p.at.clone() } else { format!("{} · {status}", p.at) }.into(),
                    can_save: decide && !p.saved && !p.saving && (recorded || p.state == Some(RecordingState::Recording)),
                    saved: p.saved,
                    recording: p.state == Some(RecordingState::Recording) && !p.saved,
                    queued: p.save_when_done,
                }
            })
            .collect();
        self.tracks.set_vec(rows);
    }

    /// Temporary recordings last as long as Driftwave runs.
    pub(crate) fn forget_recordings(&self) {
        let _ = std::fs::remove_dir_all(self.recording_dir());
    }
}

/// Quit: save, stop, and drop the temporary recordings.
pub(crate) fn quit_app(app: &std::rc::Rc<std::cell::RefCell<App>>) {
    {
        let mut a = app.borrow_mut();
        a.save_settings();
        a.stop();
        a.forget_recordings();
    }
    let _ = slint::quit_event_loop();
}

/// Closing the window: with the notification-area icon, Driftwave keeps running there (still playing, with background
/// playback on); without one, closing quits.
pub(crate) fn close_window(app: &std::rc::Rc<std::cell::RefCell<App>>, ui: &MainWindow, tray: bool) {
    if !tray {
        quit_app(app);
        return;
    }
    let mut a = app.borrow_mut();
    a.save_settings();
    if !a.settings.get("backgroundPlayback").and_then(Value::as_bool).unwrap_or(true) {
        a.stop();
    }
    drop(a);
    let _ = ui.hide();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_naturally() {
        assert_eq!(duration_text(0), "Off");
        assert_eq!(duration_text(45), "45 s");
        assert_eq!(duration_text(900), "15 min");
        assert_eq!(duration_text(90), "1 min 30 s");
    }
}
