//! Decoders: whole compressed frames in, interleaved f32 blocks out.
//!
//! Each platform's own AAC decoder lives in its own file behind [`FrameDecoder`]:
//! `macos.rs` (AudioToolbox) and, from the PC, `windows.rs` (Media Foundation). They handle HE-AAC
//! (SBR and PS), which symphonia doesn't. Elsewhere AAC falls back to symphonia (AAC-LC only: an
//! HE-AAC stream then plays its base layer at half rate).
//!
//! ADTS headers can't say whether a stream carries SBR, so on a platform with an OS decoder every ADTS
//! stream goes to it; don't sniff and choose.

use driftwave_core::audio_frames::{self, FrameHeader};
use driftwave_core::icy::RecordableFormat;
use std::fmt;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;

/// Decoded audio: interleaved samples, carrying its own rate and channel count (HE-AAC output is
/// twice the ADTS header's rate, and PS turns mono into stereo).
#[derive(Debug, Clone, PartialEq)]
pub struct PcmBlock {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl PcmBlock {
    pub fn frames(&self) -> usize {
        if self.channels == 0 { 0 } else { self.samples.len() / self.channels as usize }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(pub String);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DecodeError {}

/// One compressed frame at a time (an MP3 frame, or an ADTS frame with its header).
/// A decoder may hold frames back while it learns the stream's format, so a call can return nothing.
pub trait FrameDecoder: Send {
    fn name(&self) -> &'static str;
    fn decode(&mut self, frame: &[u8]) -> Result<Vec<PcmBlock>, DecodeError>;
    /// The next frame doesn't follow the last one (a reconnect).
    fn reset(&mut self);
}

/// The decoder for a stream of whole frames in `format`.
pub fn for_format(format: RecordableFormat) -> Box<dyn FrameDecoder> {
    match format {
        RecordableFormat::Mp3 => Box::new(SymphoniaMp3::new()),
        RecordableFormat::Aac => aac(),
    }
}

/// The best AAC decoder on this platform.
pub fn aac() -> Box<dyn FrameDecoder> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::AudioToolboxAac::new())
    }
    #[cfg(windows)]
    {
        Box::new(self::windows::MediaFoundationAac::new())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Box::new(SymphoniaAac::new())
    }
}

// MARK: Frame splitting

/// Cuts a byte stream into whole MP3 or ADTS frames, skipping anything that isn't one (an ID3 tag at
/// the start, a cut-off frame after a reconnect).
pub struct FrameSplitter {
    format: RecordableFormat,
    buf: Vec<u8>,
    synced: bool,
}

impl FrameSplitter {
    pub fn new(format: RecordableFormat) -> FrameSplitter {
        FrameSplitter { format, buf: Vec::new(), synced: false }
    }

    pub fn format(&self) -> RecordableFormat {
        self.format
    }

    /// Forget any partial frame (a new connection starts mid-frame).
    pub fn reset(&mut self) {
        self.buf.clear();
        self.synced = false;
    }

    /// Feeds bytes; returns the whole frames now complete.
    pub fn feed(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        self.buf.extend_from_slice(data);
        let mut frames = Vec::new();
        let mut i = 0;
        while i + 7 <= self.buf.len() {
            let Some(h) = audio_frames::header(self.format, &self.buf[i..]) else {
                self.synced = false;
                i += 1;
                continue;
            };
            if h.length < h.header_length.max(4) {
                i += 1;
                continue;
            }
            if i + h.length > self.buf.len() {
                break; // wait for the rest
            }
            // Before locking on, insist the next frame starts where this one says it ends.
            if !self.synced {
                if i + h.length + 7 > self.buf.len() {
                    break;
                }
                match audio_frames::header(self.format, &self.buf[i + h.length..]) {
                    Some(next) if compatible(&h, &next) => self.synced = true,
                    _ => {
                        i += 1;
                        continue;
                    }
                }
            }
            frames.push(self.buf[i..i + h.length].to_vec());
            i += h.length;
        }
        self.buf.drain(..i);
        // Don't let garbage pile up without a sync.
        if self.buf.len() > 1 << 16 {
            let keep = self.buf.len() - 8;
            self.buf.drain(..keep);
        }
        frames
    }
}

fn compatible(a: &FrameHeader, b: &FrameHeader) -> bool {
    a.sample_rate == b.sample_rate && a.profile == b.profile
}

/// Average bitrate from frame headers, for stations that don't state one (`icy-br`).
#[derive(Debug, Default)]
pub struct BitrateMeter {
    bits: u64,
    seconds: f64,
    frames: u32,
}

impl BitrateMeter {
    pub fn add(&mut self, format: RecordableFormat, frame: &[u8]) {
        if let Some(h) = audio_frames::header(format, frame) {
            if h.sample_rate > 0 && h.samples > 0 {
                self.bits += h.length as u64 * 8;
                self.seconds += h.samples as f64 / h.sample_rate as f64;
                self.frames += 1;
            }
        }
    }

    /// kbps once enough frames are in (about a second), rounded.
    pub fn kbps(&self) -> Option<u32> {
        (self.frames >= 40 && self.seconds > 0.0).then(|| (self.bits as f64 / self.seconds / 1000.0).round() as u32)
    }
}

// MARK: symphonia decoders

use symphonia::core::codecs::audio::well_known::{CODEC_ID_AAC, CODEC_ID_MP3};
use symphonia::core::codecs::audio::{AudioCodecParameters, AudioDecoder, AudioDecoderOptions};
use symphonia::core::packet::Packet;

fn block_from(decoder: &dyn AudioDecoder) -> Option<PcmBlock> {
    let buf = decoder.last_decoded();
    if buf.frames() == 0 {
        return None;
    }
    let rate = buf.spec().rate();
    let channels = buf.spec().channels().count() as u16;
    let mut samples: Vec<f32> = Vec::with_capacity(buf.frames() * channels as usize);
    buf.copy_to_vec_interleaved(&mut samples);
    Some(PcmBlock { rate, channels, samples })
}

/// MP3 through symphonia.
pub struct SymphoniaMp3 {
    decoder: symphonia::default::codecs::MpaDecoder,
}

impl SymphoniaMp3 {
    pub fn new() -> SymphoniaMp3 {
        let mut params = AudioCodecParameters::new();
        params.for_codec(CODEC_ID_MP3);
        let decoder = symphonia::default::codecs::MpaDecoder::try_new(&params, &AudioDecoderOptions::default())
            .expect("symphonia MP3 decoder");
        SymphoniaMp3 { decoder }
    }
}

impl Default for SymphoniaMp3 {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameDecoder for SymphoniaMp3 {
    fn name(&self) -> &'static str {
        "symphonia-mp3"
    }

    fn decode(&mut self, frame: &[u8]) -> Result<Vec<PcmBlock>, DecodeError> {
        let packet = Packet::new(0, Default::default(), Default::default(), frame.to_vec());
        match self.decoder.decode(&packet) {
            Ok(_) => Ok(block_from(&self.decoder).into_iter().collect()),
            Err(e) => Err(DecodeError(format!("mp3: {e}"))),
        }
    }

    fn reset(&mut self) {
        self.decoder.reset();
    }
}

/// AAC-LC through symphonia, for platforms without an OS decoder. Plays only the base layer of HE-AAC.
pub struct SymphoniaAac {
    decoder: Option<(symphonia::default::codecs::AacDecoder, u8, u8, u8)>,
}

impl SymphoniaAac {
    pub fn new() -> SymphoniaAac {
        SymphoniaAac { decoder: None }
    }
}

impl Default for SymphoniaAac {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameDecoder for SymphoniaAac {
    fn name(&self) -> &'static str {
        "symphonia-aac"
    }

    fn decode(&mut self, frame: &[u8]) -> Result<Vec<PcmBlock>, DecodeError> {
        let h = audio_frames::adts(frame).ok_or_else(|| DecodeError("aac: not an ADTS frame".into()))?;
        let key = (h.profile, h.sample_rate_index, h.channels);
        if self.decoder.as_ref().map(|d| (d.1, d.2, d.3)) != Some(key) {
            // AudioSpecificConfig: object type (5 bits), rate index (4), channel config (4).
            let object_type = h.profile as u16 + 1;
            let asc = (object_type << 11) | ((h.sample_rate_index as u16) << 7) | ((h.channels as u16) << 3);
            let mut params = AudioCodecParameters::new();
            params.for_codec(CODEC_ID_AAC).with_extra_data(asc.to_be_bytes().to_vec().into_boxed_slice());
            let decoder = symphonia::default::codecs::AacDecoder::try_new(&params, &AudioDecoderOptions::default())
                .map_err(|e| DecodeError(format!("aac: {e}")))?;
            self.decoder = Some((decoder, key.0, key.1, key.2));
        }
        let (decoder, ..) = self.decoder.as_mut().expect("decoder");
        let packet = Packet::new(0, Default::default(), Default::default(), frame[h.header_length..].to_vec());
        match decoder.decode(&packet) {
            Ok(_) => Ok(block_from(decoder).into_iter().collect()),
            Err(e) => Err(DecodeError(format!("aac: {e}"))),
        }
    }

    fn reset(&mut self) {
        if let Some((d, ..)) = self.decoder.as_mut() {
            d.reset();
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Two frames of a 128 kbps MP3 at 44.1 kHz: 417 bytes each with the header 0xFFFB9064.
    fn fake_mp3_frame() -> Vec<u8> {
        let mut f = vec![0u8; 417];
        f[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x64]);
        f
    }

    #[test]
    fn splitter_skips_junk_and_waits_for_whole_frames() {
        let frame = fake_mp3_frame();
        let mut stream = b"ID3junk".to_vec();
        for _ in 0..3 {
            stream.extend_from_slice(&frame);
        }
        let mut s = FrameSplitter::new(RecordableFormat::Mp3);
        let mut got = Vec::new();
        for chunk in stream.chunks(100) {
            got.extend(s.feed(chunk));
        }
        // The third frame can't be confirmed by a following sync yet, but once locked it is emitted.
        assert!(got.len() >= 2, "frames: {}", got.len());
        assert!(got.iter().all(|f| f.len() == 417 && f[0] == 0xFF));
    }
}
