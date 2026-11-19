//! A small MPEG-TS demuxer for HLS radio: the audio elementary stream (ADTS AAC or MP3) and the
//! timed-ID3 stream that carries titles.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum TsOut {
    /// Elementary-stream bytes: ADTS frames or MP3 frames.
    Audio(Vec<u8>),
    /// One ID3v2 tag from the metadata stream.
    Id3(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioKind {
    Adts,
    Mp3,
}

#[derive(Default)]
pub struct TsDemuxer {
    pmt_pid: Option<u16>,
    audio: Option<(u16, AudioKind)>,
    id3_pid: Option<u16>,
    pes: HashMap<u16, Vec<u8>>,
    carry: Vec<u8>,
}

impl TsDemuxer {
    pub fn new() -> TsDemuxer {
        TsDemuxer::default()
    }

    pub fn audio_kind(&self) -> Option<AudioKind> {
        self.audio.map(|a| a.1)
    }

    /// Feeds TS bytes (whole segments or any chunks). PES packets are emitted once the next one starts;
    /// call [`finish`](Self::finish) at the end of a segment to flush them.
    pub fn feed(&mut self, data: &[u8]) -> Vec<TsOut> {
        let mut out = Vec::new();
        let mut buf = std::mem::take(&mut self.carry);
        buf.extend_from_slice(data);
        let mut i = 0;
        while i + 188 <= buf.len() {
            if buf[i] != 0x47 {
                i += 1; // resync
                continue;
            }
            self.packet(&buf[i..i + 188], &mut out);
            i += 188;
        }
        self.carry = buf[i..].to_vec();
        out
    }

    pub fn finish(&mut self) -> Vec<TsOut> {
        let mut out = Vec::new();
        let pids: Vec<u16> = self.pes.keys().copied().collect();
        for pid in pids {
            self.flush(pid, &mut out);
        }
        out
    }

    fn packet(&mut self, p: &[u8], out: &mut Vec<TsOut>) {
        let pusi = p[1] & 0x40 != 0;
        let pid = (((p[1] & 0x1F) as u16) << 8) | p[2] as u16;
        let afc = (p[3] >> 4) & 0x3;
        let mut start = 4;
        if afc == 2 || afc == 0 {
            return; // no payload
        }
        if afc == 3 {
            start += 1 + p[4] as usize;
        }
        if start >= 188 {
            return;
        }
        let payload = &p[start..];
        if pid == 0 {
            if pusi {
                self.pat(payload);
            }
        } else if Some(pid) == self.pmt_pid {
            if pusi {
                self.pmt(payload);
            }
        } else if self.audio.map(|a| a.0) == Some(pid) || self.id3_pid == Some(pid) {
            if pusi {
                self.flush(pid, out);
            }
            if pusi || self.pes.contains_key(&pid) {
                self.pes.entry(pid).or_default().extend_from_slice(payload);
            }
        }
    }

    /// The section after the pointer field, if it's whole in this packet.
    fn section(payload: &[u8]) -> Option<&[u8]> {
        let ptr = *payload.first()? as usize;
        let s = payload.get(1 + ptr..)?;
        let len = (((*s.get(1)? as usize) & 0x0F) << 8) | *s.get(2)? as usize;
        s.get(..3 + len)
    }

    fn pat(&mut self, payload: &[u8]) {
        let Some(s) = Self::section(payload) else { return };
        // Programs from byte 8 to the CRC.
        let mut i = 8;
        while i + 4 <= s.len().saturating_sub(4) {
            let program = ((s[i] as u16) << 8) | s[i + 1] as u16;
            let pid = (((s[i + 2] & 0x1F) as u16) << 8) | s[i + 3] as u16;
            if program != 0 {
                self.pmt_pid = Some(pid);
                return;
            }
            i += 4;
        }
    }

    fn pmt(&mut self, payload: &[u8]) {
        let Some(s) = Self::section(payload) else { return };
        if s.len() < 12 {
            return;
        }
        let info_len = (((s[10] & 0x0F) as usize) << 8) | s[11] as usize;
        let mut i = 12 + info_len;
        let end = s.len().saturating_sub(4);
        while i + 5 <= end {
            let stype = s[i];
            let pid = (((s[i + 1] & 0x1F) as u16) << 8) | s[i + 2] as u16;
            let es_len = (((s[i + 3] & 0x0F) as usize) << 8) | s[i + 4] as usize;
            match stype {
                0x0F if self.audio.is_none() => self.audio = Some((pid, AudioKind::Adts)),
                0x03 | 0x04 if self.audio.is_none() => self.audio = Some((pid, AudioKind::Mp3)),
                0x15 => self.id3_pid = Some(pid),
                _ => {}
            }
            i += 5 + es_len;
        }
    }

    fn flush(&mut self, pid: u16, out: &mut Vec<TsOut>) {
        let Some(pes) = self.pes.remove(&pid) else { return };
        if pes.len() < 9 || pes[0..3] != [0, 0, 1] {
            return;
        }
        let header = 9 + pes[8] as usize;
        let Some(body) = pes.get(header..) else { return };
        if body.is_empty() {
            return;
        }
        if self.id3_pid == Some(pid) {
            out.push(TsOut::Id3(body.to_vec()));
        } else {
            out.push(TsOut::Audio(body.to_vec()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds TS packets carrying `es` as one PES on `pid`.
    fn pes_packets(pid: u16, es: &[u8]) -> Vec<u8> {
        let mut pes = vec![0, 0, 1, 0xC0, 0, 0, 0x80, 0, 0];
        pes.extend_from_slice(es);
        let mut out = Vec::new();
        for (n, chunk) in pes.chunks(184).enumerate() {
            let mut p = vec![0x47, ((pid >> 8) as u8 & 0x1F) | if n == 0 { 0x40 } else { 0 }, pid as u8, 0x10];
            p.extend_from_slice(chunk);
            if p.len() < 188 {
                // Pad with an adaptation field at the front instead of trailing junk.
                let pad = 188 - p.len();
                let mut q = vec![p[0], p[1], p[2], 0x30, (pad - 1) as u8];
                if pad > 1 {
                    q.push(0);
                    q.extend(std::iter::repeat_n(0xFF, pad - 2));
                }
                q.extend_from_slice(chunk);
                p = q;
            }
            out.extend(p);
        }
        out
    }

    fn psi(pid: u16, table: &[u8]) -> Vec<u8> {
        let mut p = vec![0x47, 0x40 | ((pid >> 8) as u8 & 0x1F), pid as u8, 0x10, 0];
        p.extend_from_slice(table);
        p.resize(188, 0xFF);
        p
    }

    #[test]
    fn demuxes_audio_and_id3() {
        // PAT: program 1 → PMT on 0x100. PMT: ADTS on 0x101, ID3 on 0x102.
        let pat = [0x00, 0xB0, 13, 0, 1, 0xC1, 0, 0, 0, 1, 0xE1, 0x00, 0, 0, 0, 0];
        let pmt = [0x02, 0xB0, 23, 0, 1, 0xC1, 0, 0, 0xE1, 0x01, 0xF0, 0, 0x0F, 0xE1, 0x01, 0xF0, 0, 0x15, 0xE1, 0x02, 0xF0, 0, 0, 0, 0, 0];
        let mut ts = psi(0, &pat);
        ts.extend(psi(0x100, &pmt));
        let audio: Vec<u8> = (0..400u16).map(|i| i as u8).collect();
        ts.extend(pes_packets(0x101, &audio));
        ts.extend(pes_packets(0x102, b"ID3\x04\x00\x00\x00\x00\x00\x00"));
        let mut d = TsDemuxer::new();
        let mut got = Vec::new();
        for chunk in ts.chunks(100) {
            got.extend(d.feed(chunk));
        }
        got.extend(d.finish());
        assert_eq!(d.audio_kind(), Some(AudioKind::Adts));
        let a: Vec<u8> = got.iter().filter_map(|o| if let TsOut::Audio(b) = o { Some(b.clone()) } else { None }).flatten().collect();
        assert_eq!(a, audio);
        assert!(got.iter().any(|o| matches!(o, TsOut::Id3(b) if b.starts_with(b"ID3"))));
    }
}
