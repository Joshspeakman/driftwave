//! ID3v2 tags: a minimal writer for saved MP3s and a title/artist reader for HLS timed metadata.
//! Port of `ShortwaveCore/ID3.cs`.

/// Builds a minimal ID3v2.3 tag (UTF-16 text frames) to put in front of a saved MP3.
pub fn tag(title: &str, artist: Option<&str>, album: Option<&str>, comment: Option<&str>) -> Vec<u8> {
    let mut frames = text_frame("TIT2", title);
    if let Some(a) = artist.filter(|s| !s.is_empty()) {
        frames.extend(text_frame("TPE1", a));
    }
    if let Some(a) = album.filter(|s| !s.is_empty()) {
        frames.extend(text_frame("TALB", a));
    }
    if let Some(c) = comment.filter(|s| !s.is_empty()) {
        frames.extend(comment_frame(c));
    }
    let mut out = b"ID3".to_vec();
    out.extend([3, 0, 0]); // v2.3.0, no flags
    out.extend(synchsafe(frames.len() as u32));
    out.extend(frames);
    out
}

pub(crate) fn utf16_with_bom(s: &str) -> Vec<u8> {
    let mut d = vec![0xFF, 0xFE];
    for unit in s.encode_utf16() {
        d.extend(unit.to_le_bytes());
    }
    d
}

pub(crate) fn text_frame(id: &str, value: &str) -> Vec<u8> {
    let mut body = vec![1]; // encoding: UTF-16 with BOM
    body.extend(utf16_with_bom(value));
    frame(id, &body)
}

pub(crate) fn comment_frame(value: &str) -> Vec<u8> {
    let mut body = vec![1];
    body.extend(b"eng");
    body.extend(utf16_with_bom("")); // empty description
    body.extend([0, 0]);
    body.extend(utf16_with_bom(value));
    frame("COMM", &body)
}

pub(crate) fn frame(id: &str, body: &[u8]) -> Vec<u8> {
    let mut f = id.as_bytes().to_vec();
    f.extend((body.len() as u32).to_be_bytes());
    f.extend([0, 0]);
    f.extend(body);
    f
}

pub(crate) fn synchsafe(n: u32) -> [u8; 4] {
    [((n >> 21) & 0x7F) as u8, ((n >> 14) & 0x7F) as u8, ((n >> 7) & 0x7F) as u8, (n & 0x7F) as u8]
}

fn synchsafe_value(b: &[u8]) -> usize {
    ((b[0] as usize & 0x7F) << 21) | ((b[1] as usize & 0x7F) << 14) | ((b[2] as usize & 0x7F) << 7) | (b[3] as usize & 0x7F)
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Reads the title and artist (TIT2 / TPE1, or v2.2's TT2 / TP1) from an ID3v2 tag, as HLS streams send them in
/// timed metadata. Blank values come back as `None`.
pub fn read_title_artist(tag: &[u8]) -> (Option<String>, Option<String>) {
    let (mut title, mut artist) = (None, None);
    let Some(start) = find(tag, b"ID3") else { return (None, None) };
    if tag.len() - start < 10 {
        return (None, None);
    }
    let tag = &tag[start..];
    let version = tag[3];
    let unsync = tag[5] & 0x80 != 0;
    let extended = tag[5] & 0x40 != 0;
    let size = synchsafe_value(&tag[6..10]);
    let mut body = tag[10..10 + size.min(tag.len() - 10)].to_vec();
    if unsync && version < 4 {
        body = remove_unsync(&body);
    }
    let mut p = 0;
    if extended && body.len() >= 4 {
        let ext = if version >= 4 {
            synchsafe_value(&body[..4])
        } else {
            u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize + 4
        };
        p = ext.min(body.len());
    }
    let (id_len, head_len) = if version == 2 { (3, 6) } else { (4, 10) };
    while p + head_len <= body.len() {
        let id = &body[p..p + id_len];
        if id[0] == 0 {
            break;
        }
        let fsize = match version {
            2 => (body[p + 3] as usize) << 16 | (body[p + 4] as usize) << 8 | body[p + 5] as usize,
            4 => synchsafe_value(&body[p + 4..p + 8]),
            _ => u32::from_be_bytes([body[p + 4], body[p + 5], body[p + 6], body[p + 7]]) as usize,
        };
        p += head_len;
        if fsize == 0 || p + fsize > body.len() {
            break;
        }
        let f = &body[p..p + fsize];
        match id {
            b"TIT2" | b"TT2" => title = Some(decode_text_frame(f)),
            b"TPE1" | b"TP1" => artist = Some(decode_text_frame(f)),
            _ => {}
        }
        p += fsize;
    }
    let clean = |s: Option<String>| s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    (clean(title), clean(artist))
}

fn remove_unsync(data: &[u8]) -> Vec<u8> {
    let mut o = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        o.push(data[i]);
        if data[i] == 0xFF && i + 1 < data.len() && data[i + 1] == 0 {
            i += 1;
        }
        i += 1;
    }
    o
}

fn utf16(bytes: &[u8], big_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks(2)
        .map(|c| match (c, big_endian) {
            ([a, b], true) => u16::from_be_bytes([*a, *b]),
            ([a, b], false) => u16::from_le_bytes([*a, *b]),
            _ => 0xFFFD,
        })
        .collect();
    String::from_utf16_lossy(&units)
}

fn decode_text_frame(f: &[u8]) -> String {
    if f.is_empty() {
        return String::new();
    }
    let text = &f[1..];
    let s = match f[0] {
        0 => text.iter().map(|&b| b as char).collect(),
        1 => match text {
            [0xFE, 0xFF, rest @ ..] => utf16(rest, true),
            [0xFF, 0xFE, rest @ ..] => utf16(rest, false),
            _ => utf16(text, false),
        },
        2 => utf16(text, true),
        _ => String::from_utf8_lossy(text).into_owned(),
    };
    // Multiple values are NUL-separated; keep the first.
    match s.find('\0') {
        Some(n) => s[..n].to_owned(),
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header() {
        let t = tag("Song", Some("Band"), Some("Station"), None);
        assert_eq!(&t[..3], b"ID3");
        assert_eq!(t[3], 3);
        let size = (t[6] as usize) << 21 | (t[7] as usize) << 14 | (t[8] as usize) << 7 | t[9] as usize;
        assert_eq!(size, t.len() - 10);
        assert!(find(&t, b"TIT2").is_some() && find(&t, b"TPE1").is_some());
    }

    #[test]
    fn reads_back_own_tag() {
        let t = tag("Parlons-nous", Some("Caroline Dublanche"), Some("RTL"), Some("Recorded with Driftwave"));
        assert_eq!(read_title_artist(&t), (Some("Parlons-nous".into()), Some("Caroline Dublanche".into())));
    }

    #[test]
    fn reads_v4_latin1() {
        let frame = |id: &str, v: &str| {
            let mut body = vec![0u8];
            body.extend(v.chars().map(|c| c as u8));
            let n = body.len();
            let mut f = id.as_bytes().to_vec();
            f.extend([0, (n >> 14 & 0x7F) as u8, (n >> 7 & 0x7F) as u8, (n & 0x7F) as u8, 0, 0]);
            f.extend(body);
            f
        };
        let mut frames = frame("TIT2", "Café");
        frames.extend(frame("TPE1", "Band"));
        let s = frames.len();
        let mut t = b"ID3".to_vec();
        t.extend([4, 0, 0, (s >> 21 & 0x7F) as u8, (s >> 14 & 0x7F) as u8, (s >> 7 & 0x7F) as u8, (s & 0x7F) as u8]);
        t.extend(frames);
        assert_eq!(read_title_artist(&t), (Some("Café".into()), Some("Band".into())));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(read_title_artist(b"nothing here"), (None, None));
        assert_eq!(read_title_artist(b"ID3"), (None, None));
    }
}
