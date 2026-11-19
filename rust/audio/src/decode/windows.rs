//! AAC through Windows' own decoder: Media Foundation's AAC decoder transform, which handles HE-AAC v1
//! (SBR) and v2 (PS) as well as AAC-LC. Driftwave ships no AAC decoder of its own (docs/RUST-SCOPE.md).
//!
//! ADTS frames go in whole, header included (payload type 1). The decoder learns SBR and PS from the
//! audio itself and says so with a stream change, after which its output runs at the full rate (twice the
//! ADTS header's) and, for PS, in stereo. Each block carries the rate and channels it was decoded at.

use super::{DecodeError, FrameDecoder, PcmBlock};
use driftwave_core::audio_frames;
use std::mem::ManuallyDrop;
use std::sync::Once;
use windows::core::GUID;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};

pub struct MediaFoundationAac {
    mft: Option<IMFTransform>,
    /// (profile, sample rate index, channel configuration) of the ADTS stream the decoder was opened for.
    key: Option<(u8, u8, u8)>,
    rate: u32,
    channels: u16,
    float: bool,
    out_size: u32,
    provides_samples: bool,
}

// The transform lives in the multithreaded apartment (every thread that uses it joins it first), so it
// may move between threads with the decoder.
unsafe impl Send for MediaFoundationAac {}

fn err(context: &str, e: windows::core::Error) -> DecodeError {
    DecodeError(format!("aac (Media Foundation) {context}: {e}"))
}

/// Joins this thread to the multithreaded apartment and starts Media Foundation, once.
fn ready() {
    unsafe {
        // S_FALSE (already joined) and RPC_E_CHANGED_MODE (a UI thread) are both fine for an in-proc MFT.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    static START: Once = Once::new();
    START.call_once(|| unsafe {
        let _ = MFStartup(MF_VERSION, MFSTARTUP_LITE);
    });
}

impl MediaFoundationAac {
    pub fn new() -> MediaFoundationAac {
        MediaFoundationAac { mft: None, key: None, rate: 0, channels: 0, float: true, out_size: 0, provides_samples: false }
    }

    fn open(&mut self, h: &audio_frames::FrameHeader) -> Result<(), DecodeError> {
        ready();
        self.mft = None;
        unsafe {
            let mft: IMFTransform = CoCreateInstance(&CMSAACDecMFT, None, CLSCTX_INPROC_SERVER).map_err(|e| err("create", e))?;
            let t = MFCreateMediaType().map_err(|e| err("type", e))?;
            t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(|e| err("type", e))?;
            t.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC).map_err(|e| err("type", e))?;
            t.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, h.sample_rate).map_err(|e| err("type", e))?;
            t.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, h.channels.max(1) as u32).map_err(|e| err("type", e))?;
            t.SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 1).map_err(|e| err("type", e))?; // ADTS
            // The HEAACWAVEINFO fields after WAVEFORMATEX: payload type 1 (ADTS), profile level 0xFE
            // ("unspecified"), struct type 0; no AudioSpecificConfig, since every frame has its header.
            let user_data: [u8; 12] = [1, 0, 0xFE, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            t.SetBlob(&MF_MT_USER_DATA, &user_data).map_err(|e| err("type", e))?;
            mft.SetInputType(0, &t, 0).map_err(|e| err("input type", e))?;
            self.choose_output(&mft)?;
            mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0).map_err(|e| err("start", e))?;
            self.mft = Some(mft);
        }
        Ok(())
    }

    /// Picks 32-bit float output (else 16-bit PCM) and notes its rate, channels and buffer size.
    fn choose_output(&mut self, mft: &IMFTransform) -> Result<(), DecodeError> {
        unsafe {
            let mut chosen: Option<(IMFMediaType, bool)> = None;
            for i in 0..32 {
                let Ok(t) = mft.GetOutputAvailableType(0, i) else { break };
                let sub: GUID = t.GetGUID(&MF_MT_SUBTYPE).unwrap_or_default();
                if sub == MFAudioFormat_Float {
                    chosen = Some((t, true));
                    break;
                }
                if sub == MFAudioFormat_PCM && t.GetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE).unwrap_or(0) == 16 && chosen.is_none() {
                    chosen = Some((t, false));
                }
            }
            let (t, float) = chosen.ok_or_else(|| DecodeError("aac (Media Foundation): no float or 16-bit output".into()))?;
            mft.SetOutputType(0, &t, 0).map_err(|e| err("output type", e))?;
            self.float = float;
            self.rate = t.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).map_err(|e| err("output rate", e))?;
            self.channels = t.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).map_err(|e| err("output channels", e))? as u16;
            let info = mft.GetOutputStreamInfo(0).map_err(|e| err("output info", e))?;
            self.out_size = info.cbSize.max(4096 * 8 * 4);
            self.provides_samples = info.dwFlags & (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32) != 0;
        }
        Ok(())
    }

    fn feed(&mut self, frame: &[u8]) -> Result<(), DecodeError> {
        let mft = self.mft.as_ref().expect("open");
        unsafe {
            let buffer = MFCreateMemoryBuffer(frame.len() as u32).map_err(|e| err("buffer", e))?;
            let mut ptr = std::ptr::null_mut();
            buffer.Lock(&mut ptr, None, None).map_err(|e| err("lock", e))?;
            std::ptr::copy_nonoverlapping(frame.as_ptr(), ptr, frame.len());
            buffer.Unlock().map_err(|e| err("unlock", e))?;
            buffer.SetCurrentLength(frame.len() as u32).map_err(|e| err("length", e))?;
            let sample = MFCreateSample().map_err(|e| err("sample", e))?;
            sample.AddBuffer(&buffer).map_err(|e| err("sample", e))?;
            mft.ProcessInput(0, &sample, 0).map_err(|e| err("input", e))
        }
    }

    /// Everything the decoder has ready.
    fn drain(&mut self) -> Result<Vec<PcmBlock>, DecodeError> {
        let mut blocks = Vec::new();
        loop {
            let mft = self.mft.clone().expect("open");
            unsafe {
                let ours = if self.provides_samples {
                    None
                } else {
                    let s = MFCreateSample().map_err(|e| err("sample", e))?;
                    s.AddBuffer(&MFCreateMemoryBuffer(self.out_size).map_err(|e| err("buffer", e))?).map_err(|e| err("sample", e))?;
                    Some(s)
                };
                let mut out = [MFT_OUTPUT_DATA_BUFFER { dwStreamID: 0, pSample: ManuallyDrop::new(ours), dwStatus: 0, pEvents: ManuallyDrop::new(None) }];
                let mut status = 0u32;
                let result = mft.ProcessOutput(0, &mut out, &mut status);
                let sample = ManuallyDrop::take(&mut out[0].pSample);
                drop(ManuallyDrop::take(&mut out[0].pEvents));
                match result {
                    Ok(()) => {
                        if let Some(s) = sample {
                            if let Some(b) = self.read(&s)? {
                                blocks.push(b);
                            }
                        }
                    }
                    Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => return Ok(blocks),
                    // SBR or PS found (or the stream changed): the output now runs at another rate or channel count.
                    Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => self.choose_output(&mft)?,
                    Err(e) => return Err(err("output", e)),
                }
            }
        }
    }

    unsafe fn read(&self, sample: &IMFSample) -> Result<Option<PcmBlock>, DecodeError> {
        let buffer = sample.ConvertToContiguousBuffer().map_err(|e| err("output buffer", e))?;
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        buffer.Lock(&mut ptr, None, Some(&mut len)).map_err(|e| err("lock", e))?;
        let bytes = std::slice::from_raw_parts(ptr, len as usize);
        let samples: Vec<f32> = if self.float {
            bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        } else {
            bytes.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect()
        };
        let _ = buffer.Unlock();
        if samples.is_empty() || self.channels == 0 {
            return Ok(None);
        }
        Ok(Some(PcmBlock { rate: self.rate, channels: self.channels, samples }))
    }
}

impl Default for MediaFoundationAac {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameDecoder for MediaFoundationAac {
    fn name(&self) -> &'static str {
        "mediafoundation-aac"
    }

    fn decode(&mut self, frame: &[u8]) -> Result<Vec<PcmBlock>, DecodeError> {
        ready();
        let h = audio_frames::adts(frame).ok_or_else(|| DecodeError("aac: not an ADTS frame".into()))?;
        let key = (h.profile, h.sample_rate_index, h.channels);
        if self.key != Some(key) || self.mft.is_none() {
            self.open(&h)?;
            self.key = Some(key);
        }
        match self.feed(frame) {
            Ok(()) => self.drain(),
            Err(_) => {
                // Not accepting: take what's ready, then try once more.
                let mut blocks = self.drain()?;
                self.feed(frame)?;
                blocks.extend(self.drain()?);
                Ok(blocks)
            }
        }
    }

    fn reset(&mut self) {
        if let Some(m) = &self.mft {
            unsafe {
                let _ = m.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// A 6 s stereo AAC-LC test file (440 Hz left, 660 Hz right) as ADTS, made with ffmpeg.
    fn make_adts(rate: u32) -> Option<Vec<u8>> {
        let dir = std::env::temp_dir().join(format!("driftwave-mf-aac-{}-{rate}", std::process::id()));
        std::fs::create_dir_all(&dir).ok()?;
        let out = dir.join("t.aac");
        let ok = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=6", "-f", "lavfi", "-i", "sine=frequency=660:duration=6",
                   "-filter_complex", "[0][1]amerge=inputs=2", "-ar", &rate.to_string(), "-c:a", "aac", "-b:a", "128k", "-f", "adts"])
            .arg(&out)
            .status()
            .ok()?
            .success();
        let bytes = if ok { std::fs::read(&out).ok() } else { None };
        let _ = std::fs::remove_dir_all(&dir);
        bytes
    }

    fn frequency(samples: &[f32], channels: usize, ch: usize, rate: u32) -> f64 {
        // Zero crossings over the middle of the signal.
        let mono: Vec<f32> = samples.chunks(channels).map(|f| f[ch]).collect();
        let mid = &mono[mono.len() / 4..mono.len() * 3 / 4];
        let crossings = mid.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        crossings as f64 * rate as f64 / mid.len() as f64
    }

    #[test]
    fn aac_lc_decodes_with_its_tones() {
        for rate in [44100, 48000] {
            let Some(adts) = make_adts(rate) else { return eprintln!("skipped: needs ffmpeg") };
            let mut split = crate::decode::FrameSplitter::new(driftwave_core::icy::RecordableFormat::Aac);
            let mut dec = MediaFoundationAac::new();
            let (mut out, mut r, mut c) = (Vec::new(), 0, 0);
            for f in split.feed(&adts) {
                for b in dec.decode(&f).expect("decode") {
                    r = b.rate;
                    c = b.channels;
                    out.extend(b.samples);
                }
            }
            assert_eq!((r, c), (rate, 2));
            assert!(out.len() / 2 > rate as usize * 5, "frames {}", out.len() / 2);
            let (l, rr) = (frequency(&out, 2, 0, r), frequency(&out, 2, 1, r));
            assert!((l - 440.0).abs() < 8.0 && (rr - 660.0).abs() < 8.0, "{l} / {rr}");
        }
    }

    /// Live (`--ignored`): SomaFM's 64k AAC streams are HE-AAC. Their ADTS headers say 22.05 or 24 kHz, and
    /// the decoder must find the SBR and play at twice that.
    #[test]
    #[ignore]
    fn he_aac_stream_plays_at_full_rate() {
        use std::io::Read;
        let resp = ureq::get("https://ice6.somafm.com/groovesalad-64-aac").call().expect("connect");
        let mut bytes = vec![0u8; 96_000];
        resp.into_reader().take(96_000).read_exact(&mut bytes).expect("read");
        let mut split = crate::decode::FrameSplitter::new(driftwave_core::icy::RecordableFormat::Aac);
        let frames = split.feed(&bytes);
        let header_rate = audio_frames::adts(&frames[0]).unwrap().sample_rate;
        let mut dec = MediaFoundationAac::new();
        let mut last = None;
        for f in &frames {
            for b in dec.decode(f).expect("decode") {
                last = Some((b.rate, b.channels));
            }
        }
        let (rate, channels) = last.expect("some audio");
        eprintln!("ADTS header {header_rate} Hz, decoded {rate} Hz, {channels} ch");
        assert!(header_rate <= 24_000, "expected HE-AAC (half-rate ADTS), got {header_rate}");
        assert_eq!(rate, header_rate * 2);
        assert_eq!(channels, 2);
    }
}
