//! AAC on the Mac through AudioToolbox, which decodes HE-AAC v1 and v2 (SBR and parametric stereo).
//!
//! ADTS headers only describe the base layer (for HE-AAC: half the rate, and mono for v2), so the
//! decoder asks AudioFileStream for the stream's *format list* and decodes its first, richest entry.
//! AudioFileStream works that out from the bitstream itself.

use super::{DecodeError, FrameDecoder, PcmBlock};
use std::ffi::c_void;
use std::ptr;

type OSStatus = i32;
type AudioFileStreamID = *mut c_void;
type AudioConverterRef = *mut c_void;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct AudioStreamBasicDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct AudioStreamPacketDescription {
    start_offset: i64,
    variable_frames_in_packet: u32,
    data_byte_size: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct AudioFormatListItem {
    asbd: AudioStreamBasicDescription,
    channel_layout_tag: u32,
}

#[repr(C)]
struct AudioBuffer {
    number_channels: u32,
    data_byte_size: u32,
    data: *mut c_void,
}

#[repr(C)]
struct AudioBufferList {
    number_buffers: u32,
    buffers: [AudioBuffer; 1],
}

type PropertyListenerProc = extern "C" fn(*mut c_void, AudioFileStreamID, u32, *mut u32);
type PacketsProc = extern "C" fn(*mut c_void, u32, u32, *const c_void, *mut AudioStreamPacketDescription);
type ComplexInputDataProc =
    extern "C" fn(AudioConverterRef, *mut u32, *mut AudioBufferList, *mut *mut AudioStreamPacketDescription, *mut c_void) -> OSStatus;

#[link(name = "AudioToolbox", kind = "framework")]
extern "C" {
    fn AudioFileStreamOpen(client: *mut c_void, props: PropertyListenerProc, packets: PacketsProc, hint: u32, out: *mut AudioFileStreamID) -> OSStatus;
    fn AudioFileStreamParseBytes(stream: AudioFileStreamID, size: u32, data: *const c_void, flags: u32) -> OSStatus;
    fn AudioFileStreamGetPropertyInfo(stream: AudioFileStreamID, id: u32, size: *mut u32, writable: *mut u8) -> OSStatus;
    fn AudioFileStreamGetProperty(stream: AudioFileStreamID, id: u32, size: *mut u32, data: *mut c_void) -> OSStatus;
    fn AudioFileStreamClose(stream: AudioFileStreamID) -> OSStatus;
    fn AudioConverterNew(from: *const AudioStreamBasicDescription, to: *const AudioStreamBasicDescription, out: *mut AudioConverterRef) -> OSStatus;
    fn AudioConverterDispose(c: AudioConverterRef) -> OSStatus;
    fn AudioConverterReset(c: AudioConverterRef) -> OSStatus;
    fn AudioConverterSetProperty(c: AudioConverterRef, id: u32, size: u32, data: *const c_void) -> OSStatus;
    fn AudioConverterFillComplexBuffer(
        c: AudioConverterRef,
        input: ComplexInputDataProc,
        user: *mut c_void,
        packets: *mut u32,
        out: *mut AudioBufferList,
        out_descs: *mut AudioStreamPacketDescription,
    ) -> OSStatus;
}

const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | s[3] as u32
}

const ADTS_FILE: u32 = fourcc(b"adts");
const PROP_READY: u32 = fourcc(b"redy");
const PROP_DATA_FORMAT: u32 = fourcc(b"dfmt");
const PROP_FORMAT_LIST: u32 = fourcc(b"flst");
const PROP_MAGIC_COOKIE: u32 = fourcc(b"mgic");
const CONVERTER_COOKIE: u32 = fourcc(b"dmgc");
const LINEAR_PCM: u32 = fourcc(b"lpcm");
const FLOAT_PACKED: u32 = 1 | 8; // kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked
const PARSE_DISCONTINUITY: u32 = 1;
/// Returned by our input callback when it has no more packets for this call.
const NO_MORE_INPUT: OSStatus = fourcc(b"nmin") as OSStatus;

/// State the C callbacks write into (boxed, so its address stays put).
#[derive(Default)]
struct Shared {
    ready: bool,
    packets: Vec<Vec<u8>>,
    /// The packet to hand the converter next; then the one it's reading, and its description.
    feeding: Option<Vec<u8>>,
    holding: Option<Vec<u8>>,
    feeding_desc: AudioStreamPacketDescription,
}

extern "C" fn on_property(client: *mut c_void, _stream: AudioFileStreamID, id: u32, _flags: *mut u32) {
    if id == PROP_READY {
        // SAFETY: client is the Box<Shared> the decoder owns for the stream's whole life.
        unsafe { (*(client as *mut Shared)).ready = true };
    }
}

extern "C" fn on_packets(client: *mut c_void, bytes: u32, count: u32, data: *const c_void, descs: *mut AudioStreamPacketDescription) {
    // SAFETY: data holds `bytes` bytes and descs `count` entries for the duration of this call.
    unsafe {
        let shared = &mut *(client as *mut Shared);
        let all = std::slice::from_raw_parts(data as *const u8, bytes as usize);
        if descs.is_null() {
            shared.packets.push(all.to_vec());
            return;
        }
        for d in std::slice::from_raw_parts(descs, count as usize) {
            let start = d.start_offset as usize;
            let end = start + d.data_byte_size as usize;
            if end <= all.len() {
                shared.packets.push(all[start..end].to_vec());
            }
        }
    }
}

extern "C" fn feed_converter(
    _c: AudioConverterRef,
    packets: *mut u32,
    data: *mut AudioBufferList,
    descs: *mut *mut AudioStreamPacketDescription,
    user: *mut c_void,
) -> OSStatus {
    // SAFETY: user is our Box<Shared>; the converter gives valid out-pointers.
    unsafe {
        let shared = &mut *(user as *mut Shared);
        let Some(packet) = shared.feeding.take() else {
            *packets = 0;
            return NO_MORE_INPUT;
        };
        // The converter reads the bytes after we return, so they're kept in `holding` until the
        // conversion call finishes.
        let packet = shared.holding.insert(packet);
        (*data).number_buffers = 1;
        (*data).buffers[0].data = packet.as_ptr() as *mut c_void;
        (*data).buffers[0].data_byte_size = packet.len() as u32;
        (*data).buffers[0].number_channels = 0;
        shared.feeding_desc = AudioStreamPacketDescription { start_offset: 0, variable_frames_in_packet: 0, data_byte_size: packet.len() as u32 };
        if !descs.is_null() {
            *descs = &mut shared.feeding_desc;
        }
        *packets = 1;
        0
    }
}

/// AudioToolbox's AAC decoder (AAC-LC, HE-AAC v1 and v2), fed ADTS frames.
pub struct AudioToolboxAac {
    shared: Box<Shared>,
    stream: AudioFileStreamID,
    converter: AudioConverterRef,
    out_format: AudioStreamBasicDescription,
    frames_per_packet: u32,
    discontinuity: bool,
}

// SAFETY: the AudioToolbox objects are only touched from whichever thread owns the decoder.
unsafe impl Send for AudioToolboxAac {}

impl AudioToolboxAac {
    pub fn new() -> AudioToolboxAac {
        let mut d = AudioToolboxAac {
            shared: Box::default(),
            stream: ptr::null_mut(),
            converter: ptr::null_mut(),
            out_format: AudioStreamBasicDescription::default(),
            frames_per_packet: 1024,
            discontinuity: false,
        };
        d.open_stream();
        d
    }

    fn open_stream(&mut self) {
        let client = &mut *self.shared as *mut Shared as *mut c_void;
        // SAFETY: the callbacks only use `client`, which lives as long as self.
        unsafe { AudioFileStreamOpen(client, on_property, on_packets, ADTS_FILE, &mut self.stream) };
    }

    fn close(&mut self) {
        // SAFETY: both are ours, or null.
        unsafe {
            if !self.converter.is_null() {
                AudioConverterDispose(self.converter);
                self.converter = ptr::null_mut();
            }
            if !self.stream.is_null() {
                AudioFileStreamClose(self.stream);
                self.stream = ptr::null_mut();
            }
        }
    }

    /// The richest format AudioFileStream found (HE-AAC v2 > v1 > the plain base layer).
    fn source_format(&self) -> Option<AudioStreamBasicDescription> {
        // SAFETY: sizes come from the API; buffers are sized to them.
        unsafe {
            let mut size = 0u32;
            let mut writable = 0u8;
            if AudioFileStreamGetPropertyInfo(self.stream, PROP_FORMAT_LIST, &mut size, &mut writable) == 0 && size as usize >= std::mem::size_of::<AudioFormatListItem>() {
                let n = size as usize / std::mem::size_of::<AudioFormatListItem>();
                let mut items = vec![AudioFormatListItem::default(); n];
                if AudioFileStreamGetProperty(self.stream, PROP_FORMAT_LIST, &mut size, items.as_mut_ptr() as *mut c_void) == 0 {
                    return items.first().map(|i| i.asbd);
                }
            }
            let mut asbd = AudioStreamBasicDescription::default();
            let mut size = std::mem::size_of::<AudioStreamBasicDescription>() as u32;
            if AudioFileStreamGetProperty(self.stream, PROP_DATA_FORMAT, &mut size, &mut asbd as *mut _ as *mut c_void) == 0 {
                return Some(asbd);
            }
            None
        }
    }

    fn make_converter(&mut self) -> Result<(), DecodeError> {
        let source = self.source_format().ok_or_else(|| DecodeError("audiotoolbox: no stream format".into()))?;
        let channels = source.channels_per_frame.max(1);
        let out = AudioStreamBasicDescription {
            sample_rate: source.sample_rate,
            format_id: LINEAR_PCM,
            format_flags: FLOAT_PACKED,
            bytes_per_packet: 4 * channels,
            frames_per_packet: 1,
            bytes_per_frame: 4 * channels,
            channels_per_frame: channels,
            bits_per_channel: 32,
            reserved: 0,
        };
        let mut converter: AudioConverterRef = ptr::null_mut();
        // SAFETY: valid descriptions in, out-pointer to a local.
        let status = unsafe { AudioConverterNew(&source, &out, &mut converter) };
        if status != 0 || converter.is_null() {
            return Err(DecodeError(format!("audiotoolbox: AudioConverterNew failed ({status})")));
        }
        // SAFETY: the cookie buffer is sized by the API.
        unsafe {
            let mut size = 0u32;
            let mut writable = 0u8;
            if AudioFileStreamGetPropertyInfo(self.stream, PROP_MAGIC_COOKIE, &mut size, &mut writable) == 0 && size > 0 {
                let mut cookie = vec![0u8; size as usize];
                if AudioFileStreamGetProperty(self.stream, PROP_MAGIC_COOKIE, &mut size, cookie.as_mut_ptr() as *mut c_void) == 0 {
                    AudioConverterSetProperty(converter, CONVERTER_COOKIE, size, cookie.as_ptr() as *const c_void);
                }
            }
        }
        self.converter = converter;
        self.out_format = out;
        self.frames_per_packet = if source.frames_per_packet > 0 { source.frames_per_packet } else { 2048 };
        Ok(())
    }

    fn convert(&mut self, packet: Vec<u8>) -> Result<Option<PcmBlock>, DecodeError> {
        let channels = self.out_format.channels_per_frame.max(1);
        let capacity = self.frames_per_packet.max(1024) * 2; // room for priming and SBR's doubling
        let mut samples = vec![0f32; (capacity * channels) as usize];
        let mut list = AudioBufferList {
            number_buffers: 1,
            buffers: [AudioBuffer { number_channels: channels, data_byte_size: (samples.len() * 4) as u32, data: samples.as_mut_ptr() as *mut c_void }],
        };
        let mut frames = capacity;
        self.shared.feeding = Some(packet);
        let user = &mut *self.shared as *mut Shared as *mut c_void;
        // SAFETY: the buffer list points at `samples`, which outlives the call.
        let status = unsafe { AudioConverterFillComplexBuffer(self.converter, feed_converter, user, &mut frames, &mut list, ptr::null_mut()) };
        self.shared.feeding = None;
        self.shared.holding = None;
        if status != 0 && status != NO_MORE_INPUT {
            return Err(DecodeError(format!("audiotoolbox: decode failed ({status})")));
        }
        if frames == 0 {
            return Ok(None);
        }
        samples.truncate((frames * channels) as usize);
        Ok(Some(PcmBlock { rate: self.out_format.sample_rate as u32, channels: channels as u16, samples }))
    }
}

impl Default for AudioToolboxAac {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioToolboxAac {
    fn drop(&mut self) {
        self.close();
    }
}

impl FrameDecoder for AudioToolboxAac {
    fn name(&self) -> &'static str {
        "audiotoolbox-aac"
    }

    fn decode(&mut self, frame: &[u8]) -> Result<Vec<PcmBlock>, DecodeError> {
        let flags = if std::mem::take(&mut self.discontinuity) { PARSE_DISCONTINUITY } else { 0 };
        // SAFETY: frame is a valid slice for the call.
        let status = unsafe { AudioFileStreamParseBytes(self.stream, frame.len() as u32, frame.as_ptr() as *const c_void, flags) };
        if status != 0 {
            return Err(DecodeError(format!("audiotoolbox: parse failed ({status})")));
        }
        if !self.shared.ready {
            return Ok(Vec::new()); // still learning the format; packets are kept
        }
        if self.converter.is_null() {
            self.make_converter()?;
        }
        let packets = std::mem::take(&mut self.shared.packets);
        let mut blocks = Vec::new();
        for p in packets {
            if let Some(b) = self.convert(p)? {
                blocks.push(b);
            }
        }
        Ok(blocks)
    }

    fn reset(&mut self) {
        // SAFETY: the converter is ours.
        if !self.converter.is_null() {
            unsafe { AudioConverterReset(self.converter) };
        }
        self.shared.packets.clear();
        self.discontinuity = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::FrameSplitter;
    use driftwave_core::icy::RecordableFormat;
    use std::process::Command;

    /// Writes a 6 s stereo test file (440 Hz left, 660 Hz right) as ADTS with afconvert.
    fn make_adts(codec: &str) -> Option<Vec<u8>> {
        let dir = std::env::temp_dir().join(format!("dw-at-{}-{codec}", std::process::id()));
        std::fs::create_dir_all(&dir).ok()?;
        let wav = dir.join("tone.wav");
        let aac = dir.join("tone.aac");
        let ok = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=6", "-f", "lavfi", "-i",
                "sine=frequency=660:duration=6", "-filter_complex", "[0][1]amerge=inputs=2", "-ar", "48000"])
            .arg(&wav)
            .status()
            .ok()?
            .success();
        if !ok {
            return None;
        }
        let ok = Command::new("afconvert").args(["-f", "adts", "-d", codec, "-b", "48000"]).arg(&wav).arg(&aac).status().ok()?.success();
        let data = if ok { std::fs::read(&aac).ok() } else { None };
        let _ = std::fs::remove_dir_all(&dir);
        data
    }

    /// Zero crossings per second / 2 of one channel: its frequency, for a pure tone.
    fn frequency(samples: &[f32], channels: usize, ch: usize, rate: u32) -> f64 {
        let s: Vec<f32> = samples.chunks(channels).map(|f| f[ch]).collect();
        let skip = s.len() / 4; // past the encoder's start-up
        let s = &s[skip..s.len() - skip];
        let crossings = s.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        crossings as f64 / 2.0 / (s.len() as f64 / rate as f64)
    }

    fn decode_all(adts: &[u8]) -> (u32, u16, Vec<f32>) {
        let mut split = FrameSplitter::new(RecordableFormat::Aac);
        let mut dec = AudioToolboxAac::new();
        let (mut rate, mut channels, mut all) = (0, 0, Vec::new());
        for chunk in adts.chunks(1500) {
            for frame in split.feed(chunk) {
                for b in dec.decode(&frame).expect("decode") {
                    rate = b.rate;
                    channels = b.channels;
                    all.extend(b.samples);
                }
            }
        }
        (rate, channels, all)
    }

    #[test]
    fn he_aac_v2_decodes_at_full_rate_in_stereo() {
        let Some(adts) = make_adts("aacp") else { return eprintln!("skipped: needs ffmpeg and afconvert") };
        let (rate, channels, samples) = decode_all(&adts);
        assert_eq!((rate, channels), (48000, 2), "HE-AAC v2 must not fall back to the 24 kHz mono base layer");
        let seconds = samples.len() as f64 / 2.0 / 48000.0;
        assert!((5.0..7.0).contains(&seconds), "decoded {seconds:.2} s");
        let (l, r) = (frequency(&samples, 2, 0, 48000), frequency(&samples, 2, 1, 48000));
        // Parametric stereo rebuilds the channels from a mono downmix; each side still leans its own way.
        assert!(l > 300.0 && r > 300.0, "tones gone: {l:.0} Hz / {r:.0} Hz");
    }

    #[test]
    fn he_aac_v1_and_lc_decode() {
        // HE-AAC v1 must come out at the full 48 kHz; for AAC-LC afconvert picks the rate itself.
        for (codec, want_rate) in [("aach", Some(48000)), ("aac ", None)] {
            let Some(adts) = make_adts(codec) else { return eprintln!("skipped: needs ffmpeg and afconvert") };
            let (rate, channels, samples) = decode_all(&adts);
            assert_eq!(channels, 2, "{codec}");
            if let Some(want) = want_rate {
                assert_eq!(rate, want, "{codec}");
            }
            let (l, r) = (frequency(&samples, 2, 0, rate), frequency(&samples, 2, 1, rate));
            assert!((l - 440.0).abs() < 15.0 && (r - 660.0).abs() < 15.0, "{codec}: {l:.0} Hz / {r:.0} Hz");
        }
    }
}
