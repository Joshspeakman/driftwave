//! Wraps a raw ADTS (AAC) recording in an MPEG-4 audio file (.m4a) with iTunes-style tags, without re-encoding:
//! every AAC frame is copied unchanged. Port of `ShortwaveCore/M4A.cs`.

use crate::audio_frames::{adts, FrameHeader};
use std::time::{SystemTime, UNIX_EPOCH};

/// The AAC frames found in an ADTS stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frames {
    /// (offset, length) of each frame's payload, without its ADTS header.
    pub payloads: Vec<(usize, usize)>,
    /// MPEG-4 audio object type (2 = AAC LC).
    pub object_type: u8,
    pub sample_rate_index: u8,
    pub sample_rate: u32,
    pub channels: u8,
}

fn matches(a: &FrameHeader, b: &FrameHeader) -> bool {
    a.profile == b.profile && a.sample_rate_index == b.sample_rate_index && a.channels == b.channels
}

/// Finds the AAC frames in an ADTS byte stream. Recordings are cut at arbitrary bytes, so leading bytes before the
/// first real frame and a partial frame at the end are dropped. `None` when the stream can't be passed through
/// (no frames, multiple raw blocks, or a format change mid-stream).
pub fn parse_adts(data: &[u8]) -> Option<Frames> {
    let mut payloads = Vec::new();
    let mut first: Option<FrameHeader> = None;
    let mut p = 0;
    while p + 7 <= data.len() {
        let h = adts(&data[p..]);
        let mut ok = false;
        if let Some(f) = h.filter(|f| p + f.length <= data.len()) {
            ok = match first {
                Some(fh) => matches(&fh, &f),
                None => {
                    // Lock on only when the next frame lines up too (two frames in a row), unless this frame ends
                    // the data.
                    let next = p + f.length;
                    next == data.len() || adts(&data[next..]).is_some_and(|g| matches(&f, &g))
                }
            };
        }
        if !ok {
            if let (Some(fh), Some(other)) = (first, h) {
                if p + other.length > data.len() {
                    break; // partial last frame
                }
                if !matches(&fh, &other)
                    && other.length >= other.header_length
                    && (p + other.length == data.len() || adts(&data[p + other.length..]).is_some())
                {
                    return None; // a real format change: don't produce a broken file
                }
            }
            p += 1;
            continue;
        }
        let fr = h.expect("ok implies a header");
        if fr.raw_blocks != 0 {
            return None;
        }
        first.get_or_insert(fr);
        payloads.push((p + fr.header_length, fr.length - fr.header_length));
        p += fr.length;
    }
    let hdr = first?;
    if payloads.is_empty() || hdr.channels == 0 {
        return None;
    }
    Some(Frames {
        payloads,
        object_type: hdr.profile + 1,
        sample_rate_index: hdr.sample_rate_index,
        sample_rate: hdr.sample_rate,
        channels: hdr.channels,
    })
}

/// Builds the .m4a file. `None` when the ADTS data can't be passed through.
pub fn from_adts(data: &[u8], title: &str, artist: Option<&str>, album: Option<&str>) -> Option<Vec<u8>> {
    let frames = parse_adts(data)?;
    let count = frames.payloads.len();
    let data_size: u64 = frames.payloads.iter().map(|&(_, l)| l as u64).sum();
    if data_size > u32::MAX as u64 - 1024 {
        return None;
    }
    let samples = 1024u64 * count as u64;
    let rate = frames.sample_rate;
    let seconds = samples as f64 / rate as f64;
    let avg_bitrate = if seconds > 0.0 { (data_size as f64 * 8.0 / seconds) as u32 } else { 0 };
    let max_frame = frames.payloads.iter().map(|&(_, l)| l).max().unwrap_or(0) as u32;
    let max_bitrate = (avg_bitrate as f64).max(max_frame as f64 * 8.0 * rate as f64 / 1024.0) as u32;

    let mut ftyp_body = b"M4A ".to_vec();
    ftyp_body.extend(u32b(0));
    ftyp_body.extend(b"M4A mp42isom");
    let ftyp = make_box("ftyp", &ftyp_body);
    let now = (SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) + 2_082_844_800) as u32; // since 1904
    let moov = |offset: u32| build_moov(&frames, samples, rate, avg_bitrate, max_bitrate, max_frame, offset, now, title, artist, album);
    let moov_size = moov(0).len();
    let offset = (ftyp.len() + moov_size + 8) as u32;

    let mut out = Vec::with_capacity(offset as usize + data_size as usize);
    out.extend(ftyp);
    out.extend(moov(offset));
    out.extend(u32b((data_size + 8) as u32));
    out.extend(b"mdat");
    for &(o, l) in &frames.payloads {
        out.extend_from_slice(&data[o..o + l]);
    }
    Some(out)
}

#[allow(clippy::too_many_arguments)]
fn build_moov(
    f: &Frames,
    samples: u64,
    rate: u32,
    avg_bitrate: u32,
    max_bitrate: u32,
    max_frame: u32,
    chunk_offset: u32,
    now: u32,
    title: &str,
    artist: Option<&str>,
    album: Option<&str>,
) -> Vec<u8> {
    let duration = samples.min(u32::MAX as u64) as u32;
    let matrix: Vec<u8> = [0x00010000u32, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000].iter().flat_map(|v| u32b(*v)).collect();
    let cat = |parts: &[&[u8]]| parts.concat();

    let mvhd = full_box(
        "mvhd",
        0,
        0,
        &cat(&[&u32b(now), &u32b(now), &u32b(rate), &u32b(duration), &u32b(0x00010000), &u16b(0x0100), &[0; 10], &matrix, &[0; 24], &u32b(2)]),
    );
    let tkhd = full_box(
        "tkhd",
        0,
        7,
        &cat(&[&u32b(now), &u32b(now), &u32b(1), &u32b(0), &u32b(duration), &[0; 8], &u16b(0), &u16b(0), &u16b(0x0100), &u16b(0), &matrix, &u32b(0), &u32b(0)]),
    );
    let mdhd = full_box("mdhd", 0, 0, &cat(&[&u32b(now), &u32b(now), &u32b(rate), &u32b(duration), &u16b(0x55C4), &u16b(0)]));
    let hdlr = full_box("hdlr", 0, 0, &cat(&[&u32b(0), b"soun", &[0; 12], b"SoundHandler", &[0]]));

    // AudioSpecificConfig: object type (5 bits), sampling frequency index (4), channel configuration (4), 3 zero bits.
    let asc = ((f.object_type as u16) << 11) | ((f.sample_rate_index as u16) << 7) | ((f.channels as u16) << 3);
    let dec_specific = descriptor(0x05, &asc.to_be_bytes());
    let dec_config = descriptor(
        0x04,
        &cat(&[&[0x40, 0x15, (max_frame >> 16) as u8, (max_frame >> 8) as u8, max_frame as u8], &u32b(max_bitrate), &u32b(avg_bitrate), &dec_specific]),
    );
    let sl_config = descriptor(0x06, &[0x02]);
    let es = descriptor(0x03, &cat(&[&u16b(1), &[0], &dec_config, &sl_config]));
    let esds = full_box("esds", 0, 0, &es);
    let box_rate = if rate <= 0xFFFF { rate << 16 } else { 0 };
    let mp4a = make_box(
        "mp4a",
        &cat(&[&[0; 6], &u16b(1), &[0; 8], &u16b(f.channels.min(2) as u16), &u16b(16), &u16b(0), &u16b(0), &u32b(box_rate), &esds]),
    );
    let n = f.payloads.len() as u32;
    let stsd = full_box("stsd", 0, 0, &cat(&[&u32b(1), &mp4a]));
    let stts = full_box("stts", 0, 0, &cat(&[&u32b(1), &u32b(n), &u32b(1024)]));
    let stsc = full_box("stsc", 0, 0, &cat(&[&u32b(1), &u32b(1), &u32b(n), &u32b(1)]));
    let mut sizes = Vec::with_capacity(8 + 4 * f.payloads.len());
    sizes.extend(u32b(0));
    sizes.extend(u32b(n));
    for &(_, l) in &f.payloads {
        sizes.extend(u32b(l as u32));
    }
    let stsz = full_box("stsz", 0, 0, &sizes);
    let stco = full_box("stco", 0, 0, &cat(&[&u32b(1), &u32b(chunk_offset)]));
    let stbl = make_box("stbl", &cat(&[&stsd, &stts, &stsc, &stsz, &stco]));
    let dinf = make_box("dinf", &full_box("dref", 0, 0, &cat(&[&u32b(1), &full_box("url ", 0, 1, &[])])));
    let smhd = full_box("smhd", 0, 0, &cat(&[&u16b(0), &u16b(0)]));
    let minf = make_box("minf", &cat(&[&smhd, &dinf, &stbl]));
    let mdia = make_box("mdia", &cat(&[&mdhd, &hdlr, &minf]));
    let trak = make_box("trak", &cat(&[&tkhd, &mdia]));

    let mut items = tag_item("©nam", title);
    if let Some(a) = artist.filter(|s| !s.is_empty()) {
        items.extend(tag_item("©ART", a));
    }
    if let Some(a) = album.filter(|s| !s.is_empty()) {
        items.extend(tag_item("©alb", a));
    }
    items.extend(tag_item("©too", "Driftwave"));
    let meta_hdlr = full_box("hdlr", 0, 0, &cat(&[&u32b(0), b"mdir", b"appl", &[0; 8], &[0]]));
    let meta = full_box("meta", 0, 0, &cat(&[&meta_hdlr, &make_box("ilst", &items)]));
    let udta = make_box("udta", &meta);
    make_box("moov", &cat(&[&mvhd, &trak, &udta]))
}

fn tag_item(key: &str, value: &str) -> Vec<u8> {
    make_box(key, &make_box("data", &[&u32b(1)[..], &u32b(0), value.as_bytes()].concat()))
}

fn descriptor(tag: u8, body: &[u8]) -> Vec<u8> {
    // Four-byte length form, as most muxers write it.
    let n = body.len();
    let mut d = vec![tag, 0x80 | ((n >> 21) & 0x7F) as u8, 0x80 | ((n >> 14) & 0x7F) as u8, 0x80 | ((n >> 7) & 0x7F) as u8, (n & 0x7F) as u8];
    d.extend_from_slice(body);
    d
}

fn make_box(kind: &str, body: &[u8]) -> Vec<u8> {
    let mut b = u32b((body.len() + 8) as u32).to_vec();
    b.extend(type_bytes(kind));
    b.extend_from_slice(body);
    b
}

fn full_box(kind: &str, version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut b = vec![version, (flags >> 16) as u8, (flags >> 8) as u8, flags as u8];
    b.extend_from_slice(body);
    make_box(kind, &b)
}

/// Box types are Latin-1: © is 0xA9.
fn type_bytes(kind: &str) -> Vec<u8> {
    kind.chars().map(|c| c as u32 as u8).collect()
}

fn u32b(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

fn u16b(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_frames::tests::adts_frame;
    use crate::storage::test_dir::TempDir;
    use std::process::Command;

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    fn be32(b: &[u8], at: usize) -> u32 {
        u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
    }

    #[test]
    fn drops_partial_frames_and_keeps_payloads() {
        let frames: Vec<Vec<u8>> = (0..20).map(|i| adts_frame(50 + i, (i + 1) as u8)).collect();
        // Cut in the middle of a frame at both ends, as a recording is.
        let all: Vec<u8> = frames.concat();
        let cut = &all[frames[0].len() - 10..all.len() - 25];
        let m4a = from_adts(cut, "Song", Some("Band"), Some("Station")).expect("muxes");
        // Frames 1..18 survive (frame 0 starts before the cut, frame 19 ends after it).
        let expected: Vec<u8> = frames[1..19].iter().flat_map(|f| f[7..].to_vec()).collect();
        let mdat = find(&m4a, b"mdat").unwrap();
        assert_eq!(&m4a[mdat + 4..], &expected[..]);
        let stsz = find(&m4a, b"stsz").unwrap();
        assert_eq!(be32(&m4a, stsz + 12), 18);
        let stco = find(&m4a, b"stco").unwrap();
        assert_eq!(be32(&m4a, stco + 12), (mdat + 4) as u32);
        assert!(find(&m4a, b"Station").is_some());
        assert!(find(&m4a, &[0xA9, b't', b'o', b'o']).is_some());
    }

    #[test]
    fn rejects_non_adts() {
        assert!(from_adts(&[0u8; 5000], "x", None, None).is_none());
    }

    fn tool(name: &str) -> bool {
        Command::new(name).arg("-version").output().is_ok_and(|o| o.status.success())
    }

    fn run(exe: &str, args: &[&str]) -> String {
        let o = Command::new(exe).args(args).output().expect("runs");
        assert!(o.status.success(), "{exe} {args:?} failed: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
    }

    /// With ffmpeg installed: encode real AAC, cut it at odd bytes, mux it, and check with ffprobe/ffmpeg that the
    /// result plays, carries the tags and holds the same AAC frames (passthrough, no re-encoding).
    #[test]
    fn real_aac_passthrough_with_ffmpeg() {
        if !tool("ffmpeg") || !tool("ffprobe") {
            return;
        }
        let dir = TempDir::new("m4a");
        let src = dir.path().join("src.aac");
        let s = |p: &std::path::Path| p.to_string_lossy().into_owned();
        run("ffmpeg", &["-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=500:duration=8", "-ac", "2", "-ar", "44100", "-c:a", "aac", "-b:a", "128k", "-f", "adts", &s(&src)]);
        let all = std::fs::read(&src).unwrap();
        let cut = &all[1234..all.len() - 777];
        let m4a = dir.path().join("out.m4a");
        std::fs::write(&m4a, from_adts(cut, "Test Song", Some("Test Artist"), Some("Test Station")).unwrap()).unwrap();

        let probe = run("ffprobe", &["-v", "error", "-show_entries", "stream=codec_name,sample_rate,channels:format_tags=title,artist,album", "-of", "default=nw=1", &s(&m4a)]);
        for want in ["codec_name=aac", "sample_rate=44100", "channels=2", "TAG:title=Test Song", "TAG:artist=Test Artist", "TAG:album=Test Station"] {
            assert!(probe.contains(want), "{want} missing from {probe}");
        }
        run("ffmpeg", &["-v", "error", "-xerror", "-i", &s(&m4a), "-f", "null", "-"]);

        // Copy the frames back out to ADTS: the payloads must equal the source's whole frames.
        let back = dir.path().join("back.aac");
        run("ffmpeg", &["-loglevel", "error", "-i", &s(&m4a), "-c", "copy", "-f", "adts", &s(&back)]);
        let back_bytes = std::fs::read(&back).unwrap();
        let (a, b) = (parse_adts(cut).unwrap(), parse_adts(&back_bytes).unwrap());
        assert_eq!(a.payloads.len(), b.payloads.len());
        for (i, (&(ao, al), &(bo, bl))) in a.payloads.iter().zip(&b.payloads).enumerate() {
            assert_eq!(&cut[ao..ao + al], &back_bytes[bo..bo + bl], "frame {i}");
        }
    }
}
