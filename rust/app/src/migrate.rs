//! The first launch after the C# (WinUI) Driftwave: its settings, your own themes and your background come across.
//! Its folder (`%APPDATA%\Driftwave`) is only read, never written; the station list is copied separately (main).
//! Runs once: only when this app has no settings of its own yet.

use driftwave_core::storage::AppPaths;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// Settings with the same key and meaning in both apps.
const SAME: [&str; 14] = [
    "appearance",
    "backgroundDim",
    "backgroundPlayback",
    "checkForUpdates",
    "giphyKey",
    "lastStation",
    "notifications",
    "pastTracksCount",
    "recordingMaximumDuration",
    "recordingMinimumDuration",
    "recordingMode",
    "theme",
    "trackDirectory",
    "volume",
];

/// The C# app's settings as this app's.
pub(crate) fn map_settings(theirs: &Value) -> Value {
    let mut out = Map::new();
    for k in SAME {
        if let Some(v) = theirs.get(k).filter(|v| !v.is_null()) {
            out.insert(k.into(), v.clone());
        }
    }
    if let Some(v) = theirs.get("librarySorting").filter(|v| v.is_string()) {
        out.insert("sorting".into(), v.clone());
    }
    // Its effects switch was the moving scenes'.
    if let Some(v) = theirs.get("effects").and_then(Value::as_bool) {
        out.insert("sceneMotion".into(), json!(v));
    }
    Value::Object(out)
}

/// Their settings file: the release's, else the nightly's (whichever was used last).
fn their_settings(support: &Path) -> Option<(PathBuf, Value)> {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = ["settings.json", "settings-nightly.json"]
        .iter()
        .map(|n| support.join(n))
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    found.sort();
    let (_, path) = found.pop()?;
    let text = std::fs::read_to_string(&path).ok()?;
    let value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    Some((path, value))
}

/// The Mac's Swift app (Shortwave), release and Nightly: its settings were in UserDefaults under these domains,
/// its station list in `~/Library/Application Support/Shortwave`.
#[cfg(target_os = "macos")]
const SWIFT_DOMAINS: [&str; 2] = ["io.github.joshspeakman.Shortwave", "io.github.joshspeakman.Shortwave.Nightly"];

/// macOS: the Swift Shortwave's settings, when this app has no settings yet (read only; it's left as it is).
#[cfg(target_os = "macos")]
pub(crate) fn from_swift() -> Option<Value> {
    use driftwave_platform::system::{app_prefs, PrefValue};
    for domain in SWIFT_DOMAINS {
        let prefs = app_prefs(domain, &SAME);
        if prefs.is_empty() {
            continue;
        }
        let theirs: Map<String, Value> = prefs
            .into_iter()
            .map(|(k, v)| {
                let v = match v {
                    PrefValue::Str(s) => json!(s),
                    PrefValue::Num(n) if n.fract() == 0.0 && n.abs() < 1e15 => json!(n as i64),
                    PrefValue::Num(n) => json!(n),
                    PrefValue::Bool(b) => json!(b),
                };
                (k, v)
            })
            .collect();
        crate::log(&format!("migrated settings from the Swift app ({domain})"));
        return Some(map_settings(&Value::Object(theirs)));
    }
    None
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn from_swift() -> Option<Value> {
    None
}

/// The station list to start from on the first launch: the C# Driftwave's, else (Mac) the Swift Shortwave's.
pub(crate) fn their_library() -> Option<PathBuf> {
    let mut candidates = vec![AppPaths::for_app("Driftwave").library()];
    if cfg!(target_os = "macos") {
        candidates.push(AppPaths::for_app("Shortwave").library());
    }
    candidates.into_iter().find(|p| p.exists())
}

/// The C# app's settings, themes and background, when this app has no settings yet. Returns the settings to use.
pub(crate) fn from_csharp(paths: &AppPaths) -> Option<Value> {
    from_folder(&AppPaths::for_app("Driftwave"), paths)
}

fn from_folder(theirs: &AppPaths, paths: &AppPaths) -> Option<Value> {
    if theirs.support == paths.support {
        return None;
    }
    let (file, old) = their_settings(&theirs.support)?;
    let mut settings = map_settings(&old);
    // Your own themes (built-in ones are the same in both).
    if let Ok(entries) = std::fs::read_dir(theirs.support.join("Themes")) {
        let _ = std::fs::create_dir_all(paths.themes());
        for e in entries.flatten() {
            let p = e.path();
            let dest = paths.themes().join(e.file_name());
            if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("json")) && !dest.exists() {
                let _ = std::fs::copy(&p, &dest);
            }
        }
    }
    // Your background: a full path, or a name in their Backgrounds folder.
    if let Some(name) = old.get("backgroundFile").and_then(Value::as_str).filter(|s| !s.is_empty()) {
        let src = if Path::new(name).is_absolute() { PathBuf::from(name) } else { theirs.support.join("Backgrounds").join(name) };
        if let Some(fname) = src.file_name().filter(|_| src.exists()) {
            let dir = crate::bg::folder(paths);
            let _ = std::fs::create_dir_all(&dir);
            if std::fs::copy(&src, dir.join(fname)).is_ok() {
                settings["background"] = json!(fname.to_string_lossy());
            }
        }
    }
    crate::log(&format!("migrated settings from {}", file.display()));
    Some(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the first launch would take from this Mac's Swift app (read only). Prints it; ignored by default:
    /// `cargo test --release -p driftwave-app swift_settings -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn swift_settings_on_this_mac() {
        println!("settings: {:?}", from_swift());
        println!("library: {:?}", their_library());
    }

    #[test]
    fn the_old_apps_settings_carry_over() {
        let old = json!({
            "trackDirectory": "F:\\Music",
            "theme": "dos-prompt",
            "volume": 0.11,
            "librarySorting": "votes",
            "effects": false,
            "recordingMode": "decide",
            "lastUpdateCheck": "2026-11-19T04:40:20+00:00",
            "backgroundFile": null
        });
        let s = map_settings(&old);
        assert_eq!(s["trackDirectory"], "F:\\Music");
        assert_eq!(s["theme"], "dos-prompt");
        assert_eq!(s["volume"], 0.11);
        assert_eq!(s["sorting"], "votes");
        assert_eq!(s["sceneMotion"], false);
        assert_eq!(s["recordingMode"], "decide");
        // Its own bookkeeping doesn't come across.
        assert!(s.get("lastUpdateCheck").is_none());
        assert!(s.get("backgroundFile").is_none());
    }

    #[test]
    fn themes_and_background_come_across_and_theirs_are_left_alone() {
        let root = std::env::temp_dir().join(format!("driftwave-migrate-{}", std::process::id()));
        let theirs = AppPaths::new(root.join("old"), root.join("old-cache"));
        let mine = AppPaths::new(root.join("new"), root.join("new-cache"));
        std::fs::create_dir_all(theirs.support.join("Themes")).unwrap();
        std::fs::create_dir_all(theirs.support.join("Backgrounds")).unwrap();
        std::fs::write(theirs.support.join("Themes/custom-1.json"), "{}").unwrap();
        std::fs::write(theirs.support.join("Backgrounds/sky.gif"), "GIF89a").unwrap();
        std::fs::write(theirs.support.join("settings-nightly.json"), "\u{feff}{\"theme\":\"custom-1\",\"backgroundFile\":\"sky.gif\"}").unwrap();
        let s = from_folder(&theirs, &mine).expect("migrated");
        assert_eq!(s["theme"], "custom-1");
        assert_eq!(s["background"], "sky.gif");
        assert!(mine.themes().join("custom-1.json").exists());
        assert!(crate::bg::folder(&mine).join("sky.gif").exists());
        // Theirs is untouched.
        assert!(theirs.support.join("Backgrounds/sky.gif").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
