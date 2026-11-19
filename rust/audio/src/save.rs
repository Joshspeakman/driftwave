//! Saving a recorded track: MP3 with an ID3v2.3 tag in front, AAC remuxed (not re-encoded) into a
//! tagged .m4a, or the raw .aac when that can't be done. Same as the Mac and Windows apps.

use driftwave_core::icy::split_artist_title;
use driftwave_core::playlist::file_names;
use driftwave_core::{id3, m4a};
use std::fs;
use std::path::{Path, PathBuf};

pub fn save_track(file: &Path, title: &str, station: &str, dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let data = fs::read(file).map_err(|e| e.to_string())?;
    let (artist, song) = split_artist_title(title);
    let name = file_names::sanitized(title);
    let comment = format!("Recorded with Driftwave from {station}");
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let (dest, bytes) = match ext.as_str() {
        "mp3" => {
            let mut out = id3::tag(&song, artist.as_deref(), Some(station), Some(&comment));
            out.extend_from_slice(&data);
            (file_names::unique_file_path(dir, &name, "mp3"), out)
        }
        "aac" => match m4a::from_adts(&data, &song, artist.as_deref(), Some(station)) {
            Some(m) => (file_names::unique_file_path(dir, &name, "m4a"), m),
            None => (file_names::unique_file_path(dir, &name, "aac"), data),
        },
        other => return Err(format!("unknown recording format: {other}")),
    };
    let tmp = dest.with_extension("partial");
    fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}
