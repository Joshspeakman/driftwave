//! Song titles from an Ogg Vorbis or Opus stream's comment headers. Icecast sends a new chained stream, with new
//! headers, when the song changes. Port of `ShortwaveCore/Ogg.cs`.

use std::collections::HashMap;

fn le32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Reads titles out of an Ogg byte stream fed in chunks of any size.
#[derive(Debug, Default)]
pub struct OggCommentReader {
    buffer: Vec<u8>,
    partial: HashMap<u32, Vec<u8>>,
    /// Sample rate and pre-skip per logical stream.
    streams: HashMap<u32, (u32, i64)>,
    /// Audio in chained streams that already ended.
    finished_seconds: f64,
    current_seconds: f64,
}

impl OggCommentReader {
    pub fn new() -> OggCommentReader {
        OggCommentReader::default()
    }

    /// Seconds of audio seen so far (from granule positions).
    pub fn seconds(&self) -> f64 {
        self.finished_seconds + self.current_seconds
    }

    /// Feeds stream bytes; returns titles found, each with the stream time it starts at.
    pub fn feed(&mut self, data: &[u8]) -> Vec<(String, f64)> {
        let mut titles = Vec::new();
        self.buffer.extend_from_slice(data);
        let buf = std::mem::take(&mut self.buffer);
        let mut p = 0usize;
        loop {
            let Some(start) = buf[p..].windows(4).position(|w| w == b"OggS") else {
                p = p.max(buf.len().saturating_sub(3));
                break;
            };
            p += start;
            if buf.len() - p < 27 {
                break;
            }
            let nseg = buf[p + 26] as usize;
            if buf.len() - p < 27 + nseg {
                break;
            }
            let lacing = &buf[p + 27..p + 27 + nseg];
            let body_len: usize = lacing.iter().map(|&l| l as usize).sum();
            if buf.len() - p < 27 + nseg + body_len {
                break;
            }
            let kind = buf[p + 5];
            let granule = i64::from_le_bytes(buf[p + 6..p + 14].try_into().expect("8 bytes"));
            let serial = le32(&buf, p + 14).expect("in range");
            if buf[p + 4] != 0 {
                p += 4; // unknown version: not a page
                continue;
            }
            if kind & 0x02 != 0 {
                // A new chained stream begins: the previous one's audio is complete.
                self.finished_seconds += self.current_seconds;
                self.current_seconds = 0.0;
                self.partial.remove(&serial);
                self.streams.remove(&serial);
            }
            if kind & 0x01 == 0 {
                self.partial.remove(&serial); // not a continuation: drop stale data
            }
            let body = &buf[p + 27 + nseg..p + 27 + nseg + body_len];
            let mut offset = 0;
            let mut packet = self.partial.remove(&serial).unwrap_or_default();
            for &l in lacing {
                packet.extend_from_slice(&body[offset..offset + l as usize]);
                offset += l as usize;
                if l < 255 {
                    if let Some(t) = self.handle_packet(serial, &packet) {
                        titles.push((t, self.seconds()));
                    }
                    packet.clear();
                }
            }
            if !packet.is_empty() && packet.len() < 1 << 20 {
                self.partial.insert(serial, packet);
            }
            if granule > 0 {
                if let Some(&(rate, pre_skip)) = self.streams.get(&serial).filter(|s| s.0 > 0) {
                    self.current_seconds = ((granule - pre_skip) as f64 / rate as f64).max(0.0);
                }
            }
            p += 27 + nseg + body_len;
        }
        self.buffer = buf;
        self.buffer.drain(..p.min(self.buffer.len()));
        if self.buffer.len() > 1 << 20 {
            self.buffer.clear();
        }
        titles
    }

    fn handle_packet(&mut self, serial: u32, s: &[u8]) -> Option<String> {
        if s.len() >= 16 && s[0] == 1 && &s[1..7] == b"vorbis" {
            self.streams.insert(serial, (le32(s, 12)?, 0));
            return None;
        }
        if s.len() >= 12 && &s[..8] == b"OpusHead" {
            self.streams.insert(serial, (48000, u16::from_le_bytes([s[10], s[11]]) as i64));
            return None;
        }
        if s.len() >= 7 && s[0] == 3 && &s[1..7] == b"vorbis" {
            return title(&parse_comments(&s[7..]));
        }
        if s.len() >= 8 && &s[..8] == b"OpusTags" {
            return title(&parse_comments(&s[8..]));
        }
        None
    }
}

/// Vorbis comments (`KEY=value`), keys upper-cased; the first value of a key wins.
pub(crate) fn parse_comments(s: &[u8]) -> HashMap<String, String> {
    let mut d = HashMap::new();
    let Some(vendor) = le32(s, 0) else { return d };
    let mut p = 4 + vendor as usize;
    let Some(count) = le32(s, p) else { return d };
    p += 4;
    let mut i = 0;
    while i < count && p + 4 <= s.len() {
        let len = le32(s, p).expect("checked") as usize;
        p += 4;
        if p + len > s.len() {
            break;
        }
        let kv = String::from_utf8_lossy(&s[p..p + len]).into_owned();
        p += len;
        if let Some(eq) = kv.find('=').filter(|&e| e > 0) {
            d.entry(kv[..eq].to_uppercase()).or_insert_with(|| kv[eq + 1..].trim().to_owned());
        }
        i += 1;
    }
    d
}

fn title(c: &HashMap<String, String>) -> Option<String> {
    let title = c.get("TITLE").filter(|t| !t.trim().is_empty())?;
    match c.get("ARTIST").filter(|a| !a.trim().is_empty()) {
        Some(artist) => Some(format!("{artist} - {title}")),
        None => Some(title.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(serial: u32, kind: u8, granule: i64, seq: u32, packets: &[Vec<u8>]) -> Vec<u8> {
        let (mut lacing, mut body) = (Vec::<u8>::new(), Vec::<u8>::new());
        for p in packets {
            let mut n = p.len();
            while n >= 255 {
                lacing.push(255);
                n -= 255;
            }
            lacing.push(n as u8);
            body.extend(p);
        }
        let mut h = b"OggS".to_vec();
        h.extend([0, kind]);
        h.extend(granule.to_le_bytes());
        h.extend(serial.to_le_bytes());
        h.extend(seq.to_le_bytes());
        h.extend([0; 4]); // checksum (not checked)
        h.push(lacing.len() as u8);
        h.extend(lacing);
        h.extend(body);
        h
    }

    fn vorbis_id(rate: u32) -> Vec<u8> {
        let mut b = vec![0u8; 30];
        b[0] = 1;
        b[1..7].copy_from_slice(b"vorbis");
        b[11] = 2;
        b[12..16].copy_from_slice(&rate.to_le_bytes());
        b
    }

    fn vorbis_comments(kv: &[&str]) -> Vec<u8> {
        let mut b = vec![3];
        b.extend(b"vorbis");
        b.extend(4u32.to_le_bytes());
        b.extend(b"test");
        b.extend((kv.len() as u32).to_le_bytes());
        for s in kv {
            b.extend((s.len() as u32).to_le_bytes());
            b.extend(s.as_bytes());
        }
        b.push(1);
        b
    }

    #[test]
    fn reads_chained_vorbis_titles() {
        let stream = [
            page(1, 2, 0, 0, &[vorbis_id(44100)]),
            page(1, 0, 0, 1, &[vorbis_comments(&["ARTIST=Band", "TITLE=First Song"])]),
            page(1, 0, 441000, 2, &[vec![0; 300]]),
            page(2, 2, 0, 0, &[vorbis_id(44100)]),
            page(2, 0, 0, 1, &[vorbis_comments(&["title=Only Title"])]),
        ]
        .concat();
        for chunk in [1, 17, 100_000] {
            let mut r = OggCommentReader::new();
            let titles: Vec<(String, f64)> = stream.chunks(chunk).flat_map(|c| r.feed(c)).collect();
            assert_eq!(titles.len(), 2, "chunk {chunk}");
            assert_eq!(titles[0].0, "Band - First Song");
            assert!(titles[0].1.abs() < 1e-3);
            assert_eq!(titles[1].0, "Only Title");
            assert!((titles[1].1 - 10.0).abs() < 1e-3);
        }
    }

    #[test]
    fn comments_survive_bad_lengths() {
        assert!(parse_comments(&[1, 2]).is_empty());
        let mut b = 0u32.to_le_bytes().to_vec();
        b.extend(5u32.to_le_bytes());
        b.extend(1000u32.to_le_bytes()); // claims more than there is
        assert!(parse_comments(&b).is_empty());
    }
}
