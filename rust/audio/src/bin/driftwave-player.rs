//! A headless Driftwave player for tests: the same flags and log lines as the Mac and Windows apps,
//! so `scripts/smoke-test.sh`, `behaviour-test.sh` and `recording-test.sh` can drive it
//! (`SHORTWAVE_PLAYER=rust`).
//!
//! It's always a test run: its own data folder (`DriftwaveTestRun`), and silent unless `-audible`.
//!
//! Flags: -playURL <url> (required), -audible, -muted, -quitAfter <s>, -volume <0–1>,
//! -recordingMode everything|decide|nothing, -recordingMinimumDuration <s>,
//! -recordingMaximumDuration <s>, -trackDirectory <dir>, -stationName <name>, -logLevels,
//! -noProbe (don't peek at unknown links for playlists).

use driftwave_audio::{Engine, EngineConfig, Event, PlayState};
use driftwave_core::radio_browser::USER_AGENT;
use driftwave_core::recording::{RecorderConfig, RecorderEvent, RecordingMode};
use driftwave_core::storage::AppPaths;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

const BOOLEAN_FLAGS: [&str; 5] = ["audible", "muted", "logLevels", "noProbe", "help"];

fn log(s: &str) {
    eprintln!("[driftwave] {s}");
}

fn parse_args() -> (HashMap<String, String>, Vec<String>) {
    let mut values = HashMap::new();
    let mut flags = Vec::new();
    let mut args = std::env::args().skip(1).peekable();
    while let Some(a) = args.next() {
        let Some(key) = a.strip_prefix('-').map(|k| k.trim_start_matches('-').to_string()) else { continue };
        if BOOLEAN_FLAGS.contains(&key.as_str()) {
            flags.push(key);
        } else if let Some(v) = args.next() {
            values.insert(key, v);
        }
    }
    (values, flags)
}

fn main() {
    let (values, flags) = parse_args();
    let flag = |f: &str| flags.iter().any(|x| x == f);
    let Some(url) = values.get("playURL").cloned() else {
        eprintln!("usage: driftwave-player -playURL <url> [-audible] [-quitAfter s] [-recordingMode everything ...]");
        std::process::exit(2);
    };
    let num = |k: &str| values.get(k).and_then(|v| v.parse::<f64>().ok());
    let silent = !flag("audible");
    let paths = AppPaths::for_app("DriftwaveTestRun");
    clear_dead_runs(&paths.recording());
    let recording_dir = paths.recording().join(std::process::id().to_string());
    let track_dir = values.get("trackDirectory").map(PathBuf::from).unwrap_or_else(|| paths.support.join("Tracks"));
    let mode = values.get("recordingMode").and_then(|m| RecordingMode::from_raw_value(m)).unwrap_or(RecordingMode::Decide);
    let config = EngineConfig {
        silent,
        user_agent: USER_AGENT.to_string(),
        recorder: RecorderConfig {
            mode,
            minimum_duration: num("recordingMinimumDuration").unwrap_or(30.0),
            maximum_duration: num("recordingMaximumDuration").unwrap_or(900.0),
        },
        recording_dir: recording_dir.clone(),
        track_dir,
        station_name: values.get("stationName").cloned().unwrap_or_else(|| "Test Stream".into()),
        probe_playlists: !flag("noProbe"),
    };
    log(&format!("launch testRun=true muted={silent} player=rust"));
    let (engine, events) = Engine::play(&url, config);
    engine.set_volume(num("volume").unwrap_or(1.0) as f32);
    engine.set_muted(flag("muted"));
    let deadline = num("quitAfter").map(|s| Instant::now() + Duration::from_secs_f64(s));
    let mut next_levels = Instant::now() + Duration::from_secs(1);
    loop {
        if deadline.is_some_and(|d| Instant::now() >= d) {
            break;
        }
        match events.recv_timeout(Duration::from_millis(100)) {
            Ok(e) => report(e),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if flag("logLevels") && Instant::now() >= next_levels {
            next_levels += Duration::from_secs(1);
            let m = engine.meters(16);
            let lv: Vec<String> = m.levels.iter().map(|l| format!("{l:.3}")).collect();
            log(&format!(
                "levels rms={} dominant={:.0}Hz buffered={:.1}s underruns={}",
                lv.join("/"),
                m.dominant_hz,
                engine.buffered_seconds(),
                engine.underruns()
            ));
        }
    }
    log(&format!("quit: played {:.1}s, underruns {}", engine.played_seconds(), engine.underruns()));
    engine.stop();
    let _ = std::fs::remove_dir_all(&recording_dir);
    std::process::exit(0);
}

/// Test runs a script killed leave their temporary recordings (one folder per process); drop the ones
/// whose process is gone.
fn clear_dead_runs(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.parse::<u32>().is_err() {
            continue;
        }
        let alive = std::process::Command::new("kill").args(["-0", &name]).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success());
        if !alive {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

fn report(e: Event) {
    match e {
        Event::State(PlayState::Loading) => log("state loading"),
        Event::State(PlayState::Playing) => log("state playing"),
        Event::State(PlayState::Failed(m)) => log(&format!("state failed({m:?})")),
        Event::Connected { titles, format, kind } => {
            log(&format!("icy connected titles={titles} format={} decoder={kind}", format.map(|f| f.raw_value()).unwrap_or("none")))
        }
        Event::NowPlaying { title, at } => log(&format!("now playing \"{title}\" at {at:.2}s")),
        Event::Recorder(RecorderEvent::Started { title, state, file, .. }) => log(&format!(
            "track started \"{title}\" {} {}",
            state.raw_value(),
            file.map(|f| f.display().to_string()).unwrap_or_else(|| "-".into())
        )),
        Event::Recorder(RecorderEvent::Finished { state, duration, .. }) => {
            log(&format!("track finished {} {}s", state.raw_value(), duration.round() as i64))
        }
        Event::Saved(p) => log(&format!("saved {}", p.display())),
        Event::SaveFailed(m) => log(&format!("save failed: {m}")),
        Event::Info(m) => log(&m),
    }
}
