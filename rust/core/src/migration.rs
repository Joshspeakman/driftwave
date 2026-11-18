//! First launch under the Driftwave name: copy the old Shortwave data folder. Port of `ShortwaveCore/Migration.cs`.
//!
//! The Windows app was renamed from Shortwave to Driftwave (2026-11-18). On the first launch under the new name,
//! the old data folder (library, settings, covers, the user's themes) is copied to the new one. Never moves, never
//! overwrites: the old folder stays as it was, and an existing new folder is left alone.

use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Copies `old_folder` to `new_folder` if the new one doesn't exist yet and the old one does. Settings files that
/// didn't choose a track folder get `old_track_folder` (if it exists), so recordings keep going where they always
/// went. Returns true if it copied.
pub fn copy_if_first_run(old_folder: &Path, new_folder: &Path, old_track_folder: Option<&Path>) -> io::Result<bool> {
    if new_folder.is_dir() || !old_folder.is_dir() {
        return Ok(false);
    }
    // Copy into a staging folder first, so a failure halfway never leaves a half-made data folder.
    let mut staging = new_folder.as_os_str().to_owned();
    staging.push(".migrating");
    let staging = PathBuf::from(staging);
    if staging.is_dir() {
        fs::remove_dir_all(&staging)?;
    }
    copy_tree(old_folder, &staging)?;
    if let Some(track) = old_track_folder.filter(|t| t.is_dir()) {
        for entry in fs::read_dir(&staging)? {
            let path = entry?.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with("settings") && name.ends_with(".json") && path.is_file() {
                add_track_directory(&path, track); // an unreadable settings file is copied as it was
            }
        }
    }
    fs::rename(&staging, new_folder)?;
    Ok(true)
}

fn add_track_directory(settings: &Path, track: &Path) {
    let Ok(text) = fs::read_to_string(settings) else { return };
    let Ok(parsed) = serde_json::from_str::<Value>(&text) else { return };
    let mut json = match parsed {
        Value::Object(o) => o,
        _ => Map::new(),
    };
    if json.contains_key("trackDirectory") {
        return;
    }
    json.insert("trackDirectory".into(), Value::String(track.to_string_lossy().into_owned()));
    if let Ok(out) = serde_json::to_string_pretty(&Value::Object(json)) {
        let _ = fs::write(settings, out);
    }
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_dir::TempDir;

    fn setup(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let old = dir.join("Shortwave");
        let new = dir.join("Driftwave");
        let music = dir.join("Music").join("Shortwave");
        fs::create_dir_all(old.join("Covers")).unwrap();
        fs::create_dir_all(&music).unwrap();
        fs::write(old.join("library.json"), r#"{"stations":[],"version":1}"#).unwrap();
        fs::write(old.join("Covers").join("a.png"), [1u8, 2, 3]).unwrap();
        fs::write(old.join("settings-nightly.json"), r#"{"volume":0.5}"#).unwrap();
        fs::write(old.join("settings.json"), r#"{"trackDirectory":"D:\\Mine"}"#).unwrap();
        (old, new, music)
    }

    #[test]
    fn copies_everything_and_keeps_old() {
        let t = TempDir::new("migrate");
        let (old, new, music) = setup(t.path());
        assert!(copy_if_first_run(&old, &new, Some(&music)).unwrap());
        assert_eq!(fs::read(new.join("Covers").join("a.png")).unwrap(), vec![1, 2, 3]);
        assert!(old.join("library.json").exists(), "the old folder is kept");
        assert!(!t.path().join("Driftwave.migrating").exists());
    }

    #[test]
    fn settings_get_the_old_track_folder_unless_they_chose_one() {
        let t = TempDir::new("migrate-settings");
        let (old, new, music) = setup(t.path());
        copy_if_first_run(&old, &new, Some(&music)).unwrap();
        let nightly: Value = serde_json::from_str(&fs::read_to_string(new.join("settings-nightly.json")).unwrap()).unwrap();
        assert_eq!(nightly["trackDirectory"], Value::String(music.to_string_lossy().into_owned()));
        assert_eq!(nightly["volume"], 0.5);
        let release: Value = serde_json::from_str(&fs::read_to_string(new.join("settings.json")).unwrap()).unwrap();
        assert_eq!(release["trackDirectory"], "D:\\Mine");
    }

    #[test]
    fn never_overwrites_an_existing_new_folder() {
        let t = TempDir::new("migrate-exists");
        let (old, new, music) = setup(t.path());
        fs::create_dir_all(&new).unwrap();
        fs::write(new.join("library.json"), "mine").unwrap();
        assert!(!copy_if_first_run(&old, &new, Some(&music)).unwrap());
        assert_eq!(fs::read_to_string(new.join("library.json")).unwrap(), "mine");
    }

    #[test]
    fn nothing_to_do_without_an_old_folder_or_track_folder() {
        let t = TempDir::new("migrate-none");
        assert!(!copy_if_first_run(&t.path().join("nope"), &t.path().join("new"), None).unwrap());
        let (old, new, _) = setup(t.path());
        let missing = t.path().join("no-music");
        assert!(copy_if_first_run(&old, &new, Some(&missing)).unwrap());
        let nightly = fs::read_to_string(new.join("settings-nightly.json")).unwrap();
        assert!(!nightly.contains("trackDirectory"));
    }
}
