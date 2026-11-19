//! MP3 and ADTS (AAC) frame headers, and a clock that counts audio time by walking frames.
//! Port of `ShortwaveCore/AudioFrames.cs`.

use crate::icy::RecordableFormat;

/// Header of one MP3 or ADTS (AAC) frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Whole frame, header included, in bytes.
    pub length: usize,
    pub header_length: usize,
    pub samples: u32,
    pub sample_rate: u32,
    /// ADTS: the audio object type minus one (1 = AAC LC). MP3: 0.
    pub profile: u8,
    pub sample_rate_index: u8,
    pub channels: u8,
    /// ADTS: raw data blocks in the frame minus one.
    pub raw_blocks: u8,
}

const ADTS_RATES: [u32; 13] = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];
const MP3_RATES: [[u32; 3]; 4] = [[11025, 12000, 8000], [0, 0, 0], [22050, 24000, 16000], [44100, 48000, 32000]];
const V1L1: [u32; 15] = [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448];
const V1L2: [u32; 15] = [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384];
const V1L3: [u32; 15] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
const V2L1: [u32; 15] = [0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256];
const V2L23: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];

/// Parses an ADTS header at the start of `b`; `None` if it isn't one.
pub fn adts(b: &[u8]) -> Option<FrameHeader> {
    if b.len() < 7 || b[0] != 0xFF || b[1] & 0xF6 != 0xF0 {
        return None; // sync, layer 00
    }
    let crc = b[1] & 0x01 == 0;
    let profile = (b[2] >> 6) & 0x3;
    let sfi = (b[2] >> 2) & 0xF;
    let rate = *ADTS_RATES.get(sfi as usize)?;
    let channels = ((b[2] & 0x1) << 2) | ((b[3] >> 6) & 0x3);
    let length = ((b[3] as usize & 0x3) << 11) | ((b[4] as usize) << 3) | ((b[5] as usize >> 5) & 0x7);
    let blocks = b[6] & 0x3;
    let head = if crc { 9 } else { 7 };
    if length < head {
        return None;
    }
    Some(FrameHeader {
        length,
        header_length: head,
        samples: 1024 * (blocks as u32 + 1),
        sample_rate: rate,
        profile,
        sample_rate_index: sfi,
        channels,
        raw_blocks: blocks,
    })
}

/// Parses an MPEG audio header at the start of `b`; `None` if it isn't one.
pub fn mpeg(b: &[u8]) -> Option<FrameHeader> {
    if b.len() < 4 || b[0] != 0xFF || b[1] & 0xE0 != 0xE0 {
        return None;
    }
    let version = (b[1] >> 3) & 0x3; // 0: 2.5, 2: 2, 3: 1
    let layer = (b[1] >> 1) & 0x3; // 1: III, 2: II, 3: I
    let bri = ((b[2] >> 4) & 0xF) as usize;
    let sri = (b[2] >> 2) & 0x3;
    let pad = ((b[2] >> 1) & 0x1) as u32;
    if version == 1 || layer == 0 || bri == 0 || bri == 15 || sri == 3 {
        return None;
    }
    let rate = MP3_RATES[version as usize][sri as usize];
    let v1 = version == 3;
    let kbps = match (v1, layer) {
        (true, 3) => V1L1[bri],
        (true, 2) => V1L2[bri],
        (true, _) => V1L3[bri],
        (false, 3) => V2L1[bri],
        _ => V2L23[bri],
    };
    let br = kbps * 1000;
    let (length, samples) = match layer {
        3 => ((12 * br / rate + pad) * 4, 384),
        2 => (144 * br / rate + pad, 1152),
        _ => ((if v1 { 144 } else { 72 }) * br / rate + pad, if v1 { 1152 } else { 576 }),
    };
    let channels = if (b[3] >> 6) & 0x3 == 3 { 1 } else { 2 };
    Some(FrameHeader {
        length: length as usize,
        header_length: 4,
        samples,
        sample_rate: rate,
        profile: 0,
        sample_rate_index: sri,
        channels,
        raw_blocks: 0,
    })
}

pub fn header(format: RecordableFormat, b: &[u8]) -> Option<FrameHeader> {
    match format {
        RecordableFormat::Aac => adts(b),
        RecordableFormat::Mp3 => mpeg(b),
    }
}

/// Counts how many seconds of audio have passed through a stream by walking its MP3 or ADTS frames.
#[derive(Debug, Clone)]
pub struct AudioClock {
    format: RecordableFormat,
    pending: Vec<u8>,
    /// Bytes of the current frame still to pass.
    skip: usize,
    last: Option<FrameHeader>,
    seconds: f64,
}

impl AudioClock {
    pub fn new(format: RecordableFormat) -> AudioClock {
        AudioClock { format, pending: Vec::new(), skip: 0, last: None, seconds: 0.0 }
    }

    pub fn seconds(&self) -> f64 {
        self.seconds
    }

    pub fn format(&self) -> RecordableFormat {
        self.format
    }

    pub fn feed(&mut self, data: &[u8]) {
        let mut i = 0;
        if self.skip > 0 {
            let n = self.skip.min(data.len());
            self.skip -= n;
            i = n;
        }
        if i >= data.len() {
            return;
        }
        self.pending.extend_from_slice(&data[i..]);
        let buf = &self.pending;
        let mut p = 0;
        while buf.len() - p >= 9 {
            let h = header(self.format, &buf[p..]);
            // Accept a frame when it matches the previous one's format, or (to lock on) when the next frame lines
            // up too.
            let ok = h.is_some_and(|f| {
                self.last.is_some_and(|l| same(&l, &f))
                    || (buf.len() - p >= f.length + 9
                        && header(self.format, &buf[p + f.length..]).is_some_and(|g| same(&f, &g)))
            });
            if !ok {
                if let Some(f) = h {
                    if self.last.is_none() && buf.len() - p < f.length + 9 {
                        break; // wait for more data to confirm
                    }
                }
                p += 1;
                continue;
            }
            let f = h.expect("checked above");
            self.last = Some(f);
            self.seconds += f.samples as f64 / f.sample_rate as f64;
            if buf.len() - p >= f.length {
                p += f.length;
            } else {
                self.skip = f.length - (buf.len() - p);
                p = buf.len();
            }
        }
        self.pending.drain(..p);
        if self.pending.len() > 1 << 16 {
            let excess = self.pending.len() - 16;
            self.pending.drain(..excess); // garbage: don't grow forever
        }
    }
}

fn same(a: &FrameHeader, b: &FrameHeader) -> bool {
    a.sample_rate == b.sample_rate && a.channels == b.channels && a.profile == b.profile
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fake ADTS frame: valid 7-byte header (AAC LC, 44.1 kHz, stereo) and a payload.
    pub(crate) fn adts_frame(payload_length: usize, fill: u8) -> Vec<u8> {
        let len = payload_length + 7;
        let mut f = vec![
            0xFF,
            0xF1,
            (1 << 6) | (4 << 2),
            (2 << 6) | ((len >> 11) & 0x3) as u8,
            (len >> 3) as u8,
            (((len & 0x7) << 5) | 0x1F) as u8,
            0xFC,
        ];
        f.extend(std::iter::repeat_n(fill, payload_length));
        f
    }

    #[test]
    fn clock_counts_adts_frames_across_chunks() {
        let mut stream = vec![0x12, 0x34, 0xFF]; // junk before the first frame
        for i in 0..43 {
            stream.extend(adts_frame(100 + i % 7, i as u8));
        }
        for chunk in [1, 5, 64, 100_000] {
            let mut clock = AudioClock::new(RecordableFormat::Aac);
            for c in stream.chunks(chunk) {
                clock.feed(c);
            }
            // The last frame can't be confirmed by a following one until more data arrives, but it is counted once
            // the previous frame has locked the format.
            let s = clock.seconds();
            assert!(s >= 42.0 * 1024.0 / 44100.0 - 1e-9 && s <= 43.0 * 1024.0 / 44100.0 + 1e-9, "chunk {chunk}: {s}");
        }
    }

    #[test]
    fn clock_counts_mp3_frames() {
        // MPEG-1 Layer III, 128 kbps, 44.1 kHz, no padding: 417 bytes, 1152 samples.
        let mut frame = vec![0xFF, 0xFB, 0x90, 0x64];
        frame.extend([0u8; 413]);
        assert_eq!(mpeg(&frame).unwrap().length, 417);
        let mut clock = AudioClock::new(RecordableFormat::Mp3);
        for _ in 0..100 {
            clock.feed(&frame);
        }
        let s = clock.seconds();
        assert!(s >= 99.0 * 1152.0 / 44100.0 - 1e-9 && s <= 100.0 * 1152.0 / 44100.0 + 1e-9);
    }
}
