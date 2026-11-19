//! Playlist files (.pls, .m3u, .xspf) and safe file names. Port of `ShortwaveCore/Playlist.cs`.

use std::path::{Path, PathBuf};
use unicode_segmentation::UnicodeSegmentation;
use url::Url;

fn path_extension(url: &Url) -> String {
    let last = url.path().rsplit('/').next().unwrap_or("");
    match last.rfind('.') {
        Some(dot) if dot > 0 => last[dot + 1..].to_lowercase(),
        _ => String::new(),
    }
}

/// Whether a link points to a playlist (rather than the stream itself), by its extension or content type.
/// HLS (`.m3u8`) is not a playlist in this sense: the player plays it directly.
pub fn looks_like_playlist(url: &Url, content_type: Option<&str>) -> bool {
    let ext = path_extension(url);
    if matches!(ext.as_str(), "pls" | "m3u" | "xspf") {
        return true;
    }
    let ct = content_type.unwrap_or("").to_lowercase();
    ct.contains("scpls") || (ct.contains("x-mpegurl") && ext != "m3u8") || ct.contains("audio/mpegurl") || ct.contains("xspf")
}

const NEWLINES: [char; 7] = ['\n', '\r', '\u{000B}', '\u{000C}', '\u{0085}', '\u{2028}', '\u{2029}'];

/// First stream URL in the playlist text, resolved against `base`.
pub fn first_stream(text: &str, base: &Url) -> Option<Url> {
    let lines: Vec<&str> =
        text.split(NEWLINES).filter(|l| !l.is_empty()).map(|l| l.trim_matches([' ', '\t'])).collect();
    // PLS: File1=http://...
    for line in &lines {
        if line.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("file")) {
            if let Some(eq) = line.find('=') {
                if let Some(u) = resolve(&line[eq + 1..], base) {
                    return Some(u);
                }
            }
        }
    }
    // XSPF: <location>http://...</location>
    if let Some(r) = text.find("<location>") {
        let s = r + "<location>".len();
        if let Some(e) = text[s..].find("</location>") {
            if let Some(u) = resolve(&text[s..s + e], base) {
                return Some(u);
            }
        }
    }
    // M3U: first non-comment line
    for line in &lines {
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') || line.contains('=') {
            continue;
        }
        if let Some(u) = resolve(line, base) {
            return Some(u);
        }
    }
    None
}

fn resolve(s: &str, base: &Url) -> Option<Url> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    // Swift's URL(string:) rejects strings with spaces and other characters that need escaping.
    if t.chars().any(|c| matches!(c, ' ' | '"' | '<' | '>' | '\\' | '^' | '`' | '{' | '|' | '}') || c.is_control()) {
        return None;
    }
    let u = base.join(t).ok()?;
    matches!(u.scheme(), "http" | "https").then_some(u)
}

/// Safe file names for recorded tracks.
pub mod file_names {
    use super::*;

    const FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    const TRIMMED: [char; 17] = [
        ' ', '\t', '.', '\u{a0}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}',
        '\u{2007}', '\u{2008}', '\u{2009}', '\u{200a}', '\u{202f}', '\u{205f}',
    ];

    fn trim_spaces_and_dots(s: &str) -> &str {
        s.trim_matches(|c| TRIMMED.contains(&c) || c == '\u{3000}')
    }

    /// Windows device names that can't be used as a file name, even with an extension.
    fn is_reserved(stem: &str) -> bool {
        let u = stem.to_uppercase();
        if matches!(u.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
            return true;
        }
        let mut chars = u.chars();
        let prefix: String = chars.by_ref().take(3).collect();
        let rest: Vec<char> = chars.collect();
        (prefix == "COM" || prefix == "LPT")
            && rest.len() == 1
            && (rest[0].is_ascii_digit() || matches!(rest[0], '¹' | '²' | '³'))
    }

    /// Makes a string safe to use as a file name (at most 150 characters by default).
    pub fn sanitized(s: &str) -> String {
        sanitized_max(s, 150)
    }

    pub fn sanitized_max(s: &str, max_length: usize) -> String {
        let chars: String = s
            .chars()
            .map(|c| {
                if FORBIDDEN.contains(&c) || c.is_control() || matches!(c, '\u{2028}' | '\u{2029}' | '\u{0085}') {
                    '_'
                } else {
                    c
                }
            })
            .collect();
        let mut o = trim_spaces_and_dots(&chars).to_owned();
        if o.graphemes(true).count() > max_length {
            o = o.graphemes(true).take(max_length).collect::<String>().trim_matches([' ', '\t']).to_owned();
        }
        // Windows also refuses a name ending in a space or dot, and device names like CON or COM1.
        let o = trim_spaces_and_dots(&o).to_owned();
        if o.is_empty() {
            return "Track".into();
        }
        let stem = o.split('.').next().unwrap_or("").trim_end_matches(' ');
        if is_reserved(stem) {
            format!("_{o}")
        } else {
            o
        }
    }

    /// `dir/name.ext`, or `dir/name (2).ext` etc. when that file already exists.
    pub fn unique_file_path(dir: &Path, name: &str, ext: &str) -> PathBuf {
        let mut candidate = dir.join(format!("{name}.{ext}"));
        let mut n = 2;
        while candidate.exists() {
            candidate = dir.join(format!("{name} ({n}).{ext}"));
            n += 1;
        }
        candidate
    }
}

#[cfg(test)]
mod tests {
    use super::file_names::*;
    use super::*;
    use crate::storage::test_dir::TempDir;

    fn base() -> Url {
        Url::parse("http://example.com/radio/listen.pls").unwrap()
    }

    #[test]
    fn pls() {
        let text = "[playlist]\nNumberOfEntries=2\nFile1=http://stream.example.com:8000/live\nTitle1=Live\n";
        assert_eq!(first_stream(text, &base()).unwrap().as_str(), "http://stream.example.com:8000/live");
    }

    #[test]
    fn m3u() {
        let text = "#EXTM3U\n#EXTINF:-1,Station\nhttps://a.example.com/stream.mp3\n";
        assert_eq!(first_stream(text, &base()).unwrap().as_str(), "https://a.example.com/stream.mp3");
    }

    #[test]
    fn relative() {
        assert_eq!(first_stream("stream.mp3\n", &base()).unwrap().as_str(), "http://example.com/radio/stream.mp3");
    }

    #[test]
    fn xspf_and_rejects() {
        let x = "<playlist><trackList><track><location>http://x.example/s</location></track></trackList></playlist>";
        assert_eq!(first_stream(x, &base()).unwrap().as_str(), "http://x.example/s");
        assert!(first_stream("#EXTM3U\nftp://nope/x\nwith space\n", &base()).is_none());
    }

    #[test]
    fn detection() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert!(looks_like_playlist(&u("http://x/a.pls"), None));
        assert!(looks_like_playlist(&u("http://x/a"), Some("audio/x-scpls")));
        assert!(!looks_like_playlist(&u("http://x/a.m3u8"), Some("application/x-mpegurl")));
        assert!(!looks_like_playlist(&u("http://x/a"), Some("audio/mpeg")));
    }

    #[test]
    fn file_names_basic() {
        assert_eq!(sanitized("AC/DC: Back in Black?"), "AC_DC_ Back in Black_");
        assert_eq!(sanitized("..."), "Track");
    }

    #[test]
    fn reserved_names_and_trailing_dots() {
        assert_eq!(sanitized("CON"), "_CON");
        assert_eq!(sanitized("nul"), "_nul");
        assert_eq!(sanitized("COM1.remix"), "_COM1.remix");
        assert_eq!(sanitized("LPT9 "), "_LPT9");
        assert_eq!(sanitized("Console"), "Console");
        assert_eq!(sanitized("Song. . "), "Song");
        assert_eq!(sanitized("a\tb"), "a_b");
        assert_eq!(sanitized(&"x".repeat(400)).len(), 150);
    }

    #[test]
    fn unique_paths() {
        let dir = TempDir::new("names");
        let a = unique_file_path(dir.path(), "Song", "mp3");
        assert_eq!(a, dir.path().join("Song.mp3"));
        std::fs::write(&a, "").unwrap();
        assert_eq!(unique_file_path(dir.path(), "Song", "mp3"), dir.path().join("Song (2).mp3"));
    }
}
