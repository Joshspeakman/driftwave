//! The player: one connection carries audio and titles; decoded audio goes to the output queue, and
//! titles are pinned to the queue position where they occur, so a title changes when that audio is
//! heard (not when the metadata arrives, seconds early).
//!
//! State comes from the output buffer, not the socket: a dropped connection that reconnects before
//! the buffer runs dry never shows as "loading".

use crate::analysis::{Analyzer, Meters};
use crate::decode::{self, FrameDecoder, FrameSplitter, PcmBlock};
use crate::hls;
use crate::net::{self, Connection};
use crate::output::{self, OutputShared};
use crate::resample::Resampler;
use driftwave_core::icy::{self, IcyEvent, IcyParser, RecordableFormat};
use driftwave_core::ogg::OggCommentReader;
use driftwave_core::playlist;
use driftwave_core::recording::{RecorderConfig, RecorderEvent, RecordingMode, SegmentRecorder};
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use url::Url;

/// Start playing once this much audio is queued, and after an underrun.
const START_SECONDS: f64 = 1.5;
/// Stop reading ahead beyond this (only bursts and HLS get here; live streams pace themselves).
const MAX_BUFFER_SECONDS: f64 = 12.0;
/// Reconnect attempts in a row before giving up.
const RECONNECTS: u32 = 5;

#[derive(Debug, Clone, PartialEq)]
pub enum PlayState {
    Loading,
    Playing,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    State(PlayState),
    /// A connection is up. `titles`: the server sends ICY titles. `kind`: how it's decoded.
    Connected { titles: bool, format: Option<RecordableFormat>, kind: String },
    /// A title is now audible; `at` is the playback position in seconds.
    NowPlaying { title: String, at: f64 },
    Recorder(RecorderEvent),
    Saved(PathBuf),
    SaveFailed(String),
    Info(String),
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// A test run: the output stays silent whatever volume and mute say.
    pub silent: bool,
    pub user_agent: String,
    pub recorder: RecorderConfig,
    /// Temporary recordings.
    pub recording_dir: PathBuf,
    /// Where finished tracks are saved ("Save All Tracks").
    pub track_dir: PathBuf,
    pub station_name: String,
    /// Peek at links with unknown content types in case they're playlists (local stations).
    pub probe_playlists: bool,
}

struct Timeline {
    /// Frames pushed to the output so far.
    pushed: AtomicU64,
    /// (frame position, title), in order.
    markers: Mutex<VecDeque<(u64, String)>>,
    /// The stream thread has given up (or was stopped).
    ended: AtomicBool,
    failure: Mutex<Option<String>>,
}

pub struct Engine {
    output: Arc<OutputShared>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    analyzer: Mutex<Analyzer>,
}

impl Engine {
    /// Starts playing `url`. Events arrive on the returned receiver.
    pub fn play(url: &str, config: EngineConfig) -> (Engine, Receiver<Event>) {
        let (tx, rx) = mpsc::channel();
        let output = output::start(config.silent);
        let stop = Arc::new(AtomicBool::new(false));
        let timeline = Arc::new(Timeline {
            pushed: AtomicU64::new(0),
            markers: Mutex::new(VecDeque::new()),
            ended: AtomicBool::new(false),
            failure: Mutex::new(None),
        });
        let _ = tx.send(Event::Info(format!(
            "output {} at {} Hz, {} ch{}",
            output.device_name.lock().unwrap(),
            output.rate,
            output.channels,
            if config.silent { ", silent (test run)" } else { "" }
        )));
        let mut threads = Vec::new();
        {
            let (url, config, output, stop, timeline, tx) = (url.to_string(), config.clone(), output.clone(), stop.clone(), timeline.clone(), tx.clone());
            threads.push(thread::Builder::new().name("driftwave-stream".into()).spawn(move || {
                let mut s = Stream { config, output, stop, timeline: timeline.clone(), tx, tracks: HashMap::new() };
                if let Err(message) = s.run(&url) {
                    *timeline.failure.lock().unwrap() = Some(message);
                }
                timeline.ended.store(true, Ordering::Relaxed);
            }).expect("stream thread"));
        }
        {
            let (output, stop, timeline) = (output.clone(), stop.clone(), timeline.clone());
            threads.push(thread::Builder::new().name("driftwave-control".into()).spawn(move || control(output, stop, timeline, tx)).expect("control thread"));
        }
        (Engine { output, stop, threads, analyzer: Mutex::new(Analyzer::new()) }, rx)
    }

    pub fn set_volume(&self, v: f32) {
        self.output.set_volume(v);
    }

    pub fn set_muted(&self, m: bool) {
        self.output.set_muted(m);
    }

    pub fn played_seconds(&self) -> f64 {
        self.output.played_frames() as f64 / self.output.rate as f64
    }

    pub fn buffered_seconds(&self) -> f64 {
        self.output.buffered_seconds()
    }

    pub fn underruns(&self) -> u64 {
        self.output.underruns()
    }

    /// Levels and spectrum of what's playing (before volume and mute).
    pub fn meters(&self, bands: usize) -> Meters {
        let tap = self.output.tap(crate::analysis::FFT_SIZE);
        self.analyzer.lock().unwrap().measure(&tap, self.output.channels, self.output.rate, bands)
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.output.stop();
        for t in self.threads.drain(..) {
            // The stream thread may be blocked in a read; it ends with its socket's timeout.
            let deadline = Instant::now() + Duration::from_millis(500);
            while !t.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            if t.is_finished() {
                let _ = t.join();
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Drives the state from the output buffer, and announces titles when playback reaches them.
fn control(output: Arc<OutputShared>, stop: Arc<AtomicBool>, timeline: Arc<Timeline>, tx: Sender<Event>) {
    let mut state = PlayState::Loading;
    let _ = tx.send(Event::State(state.clone()));
    let mut current: Option<String> = None;
    while !stop.load(Ordering::Relaxed) {
        let buffered = output.buffered_seconds();
        let ended = timeline.ended.load(Ordering::Relaxed);
        let next = match state {
            PlayState::Loading if buffered >= START_SECONDS || (ended && buffered > 0.0) => PlayState::Playing,
            PlayState::Playing if output.buffered_frames() == 0 && !ended => PlayState::Loading,
            ref s => s.clone(),
        };
        let next = if ended && output.buffered_frames() == 0 {
            match timeline.failure.lock().unwrap().clone() {
                Some(m) => PlayState::Failed(m),
                None => PlayState::Failed("The station stopped sending audio.".into()),
            }
        } else {
            next
        };
        if next != state {
            output.set_running(next == PlayState::Playing);
            let _ = tx.send(Event::State(next.clone()));
            let failed = matches!(next, PlayState::Failed(_));
            state = next;
            if failed {
                break;
            }
        }
        let played = output.played_frames();
        loop {
            let due = {
                let mut m = timeline.markers.lock().unwrap();
                match m.front() {
                    Some((at, _)) if *at <= played => m.pop_front(),
                    _ => None,
                }
            };
            let Some((at, title)) = due else { break };
            // A reconnect announces the same song again: keep the one we have.
            if current.as_deref() != Some(title.as_str()) {
                let _ = tx.send(Event::NowPlaying { title: title.clone(), at: at as f64 / output.rate as f64 });
                current = Some(title);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
}

struct Stream {
    config: EngineConfig,
    output: Arc<OutputShared>,
    stop: Arc<AtomicBool>,
    timeline: Arc<Timeline>,
    tx: Sender<Event>,
    /// Recorded tracks: id → (title, temporary file, format).
    tracks: HashMap<uuid::Uuid, (String, Option<PathBuf>)>,
}

/// What a connection carries.
enum Kind {
    Frames(RecordableFormat),
    Ogg,
    Hls,
}

/// Decodes whole frames to the output, converting rate and channels.
struct Sink {
    decoder: Box<dyn FrameDecoder>,
    resampler: Option<Resampler>,
    errors: u32,
}

impl Sink {
    fn new(decoder: Box<dyn FrameDecoder>) -> Sink {
        Sink { decoder, resampler: None, errors: 0 }
    }

    fn reset(&mut self) {
        self.decoder.reset();
        if let Some(r) = self.resampler.as_mut() {
            r.reset();
        }
    }
}

impl Stream {
    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn send(&self, e: Event) {
        let _ = self.tx.send(e);
    }

    fn push_block(&self, resampler: &mut Option<Resampler>, b: &PcmBlock) {
        s_push(self.output.clone(), self.timeline.clone(), self.stop.clone(), resampler, b);
    }

    fn decode_frames(&self, sink: &mut Sink, frames: Vec<Vec<u8>>) -> Result<(), String> {
        for f in frames {
            match sink.decoder.decode(&f) {
                Ok(blocks) => {
                    sink.errors = 0;
                    for b in blocks {
                        self.push_block(&mut sink.resampler, &b);
                    }
                }
                Err(e) => {
                    sink.errors += 1;
                    if sink.errors > 50 {
                        return Err(format!("The stream could not be decoded ({e})."));
                    }
                }
            }
        }
        Ok(())
    }

    fn mark_title(&self, title: &str) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        let at = self.timeline.pushed.load(Ordering::Relaxed);
        self.timeline.markers.lock().unwrap().push_back((at, title.to_string()));
    }

    fn recorder_events(&mut self, events: Vec<RecorderEvent>) {
        for e in events {
            match &e {
                RecorderEvent::Started { id, title, file, .. } => {
                    self.tracks.insert(*id, (title.clone(), file.clone()));
                }
                RecorderEvent::Finished { id, state, .. } => {
                    // "Save All Tracks": save, then drop the temporary file. (Deciding per track is the
                    // app's job; the recorder already deleted discarded ones.)
                    let track = self.tracks.remove(id);
                    let save_all = self.config.recorder.mode == RecordingMode::Everything;
                    if let (true, true, Some((title, Some(file)))) = (state.is_recorded(), save_all, track) {
                        match crate::save::save_track(&file, &title, &self.config.station_name, &self.config.track_dir) {
                            Ok(p) => {
                                let _ = std::fs::remove_file(&file);
                                self.send(Event::Saved(p));
                            }
                            Err(err) => self.send(Event::SaveFailed(err)),
                        }
                    }
                }
            }
            self.send(Event::Recorder(e));
        }
    }

    /// Follows playlist links (by extension, or by content type when probing) to the stream itself.
    fn resolve(&self, url: &str) -> Result<(Url, Option<Connection>), String> {
        let mut url = Url::parse(url.trim()).map_err(|_| "The stream address is not valid.".to_string())?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err("The stream address is not valid.".into());
        }
        for _ in 0..3 {
            if url.path().to_ascii_lowercase().ends_with(".m3u8") {
                return Ok((url, None));
            }
            let by_ext = playlist::looks_like_playlist(&url, None);
            if !by_ext && !self.config.probe_playlists {
                return Ok((url, None));
            }
            let conn = net::open(&url, &self.config.user_agent, true).map_err(|e| e.to_string())?;
            let ct = conn.content_type.clone();
            if is_hls_type(ct.as_deref()) {
                return Ok((conn.url.clone(), None));
            }
            if !playlist::looks_like_playlist(&conn.url, ct.as_deref()) {
                return Ok((conn.url.clone(), Some(conn))); // a plain stream: keep the connection
            }
            let mut body = Vec::new();
            let _ = conn.reader.take(65536).read_to_end(&mut body);
            let text = String::from_utf8_lossy(&body);
            match playlist::first_stream(&text, &conn.url) {
                Some(next) => url = next,
                None => return Ok((conn.url, None)),
            }
        }
        Ok((url, None))
    }

    fn run(&mut self, url: &str) -> Result<(), String> {
        let (url, mut first) = self.resolve(url)?;
        if url.path().to_ascii_lowercase().ends_with(".m3u8") {
            return hls::run(self_ref(self), &url);
        }
        let mut recorder: Option<SegmentRecorder> = None;
        let mut attempts = 0u32;
        let mut sink: Option<Sink> = None;
        let mut ogg: Option<OggPipe> = None;
        loop {
            if self.stopped() {
                return Ok(());
            }
            let conn = match first.take() {
                Some(c) => Ok(c),
                None => net::open(&url, &self.config.user_agent, true),
            };
            let conn = match conn {
                Ok(c) => c,
                Err(e) => {
                    attempts += 1;
                    if attempts > RECONNECTS {
                        return Err(e.to_string());
                    }
                    thread::sleep(Duration::from_millis(if attempts == 1 { 300 } else { 2000 }));
                    continue;
                }
            };
            if is_hls_type(conn.content_type.as_deref()) {
                return hls::run(self_ref(self), &conn.url);
            }
            let mut reader = conn.reader;
            let mut head = vec![0u8; 0];
            let kind = match kind_of(conn.content_type.as_deref()) {
                Some(k) => k,
                None => {
                    // Unknown type: sniff the first bytes.
                    head.resize(4096, 0);
                    let n = read_some(&mut reader, &mut head);
                    head.truncate(n);
                    sniff(&head).ok_or_else(|| "This station's stream format isn't supported.".to_string())?
                }
            };
            let format = match kind {
                Kind::Frames(f) => Some(f),
                _ => None,
            };
            let titles = conn.meta_int.is_some();
            let kind_name = match &kind {
                Kind::Frames(_) => sink.as_ref().map(|s| s.decoder.name()).unwrap_or_else(|| decode::for_format(format.unwrap()).name()).to_string(),
                Kind::Ogg => "symphonia-ogg".to_string(),
                Kind::Hls => "hls".to_string(),
            };
            self.send(Event::Connected { titles, format, kind: kind_name });
            if titles && recorder.is_none() {
                recorder = Some(SegmentRecorder::new(self.config.recorder, format, self.config.recording_dir.clone(), None));
            }
            let mut icy_parser = conn.meta_int.map(IcyParser::new);
            let mut splitter = format.map(FrameSplitter::new);
            match kind {
                Kind::Frames(f) => {
                    if sink.is_none() {
                        sink = Some(Sink::new(decode::for_format(f)));
                    } else if let Some(s) = sink.as_mut() {
                        s.reset();
                    }
                }
                Kind::Ogg => {
                    if ogg.is_none() {
                        ogg = Some(OggPipe::start(self.output.clone(), self.timeline.clone(), self.stop.clone()));
                    }
                }
                Kind::Hls => unreachable!(),
            }
            let mut comments = OggCommentReader::new();
            let mut good_bytes = 0usize;
            let mut buf = vec![0u8; 16384];
            let mut pending = head;
            let result: Result<(), String> = loop {
                if self.stopped() {
                    break Ok(());
                }
                let data: Vec<u8> = if !pending.is_empty() {
                    std::mem::take(&mut pending)
                } else {
                    match reader.read(&mut buf) {
                        Ok(0) => break Ok(()),
                        Ok(n) => buf[..n].to_vec(),
                        Err(_) => break Ok(()),
                    }
                };
                good_bytes += data.len();
                if good_bytes > 256 * 1024 {
                    attempts = 0; // this connection worked
                }
                let events = match icy_parser.as_mut() {
                    Some(p) => p.feed(&data),
                    None => vec![IcyEvent::Audio(data)],
                };
                let mut failure: Option<String> = None;
                for e in events {
                    match e {
                        IcyEvent::Audio(a) => {
                            if let Some(r) = recorder.as_mut() {
                                let ev = r.handle_audio(&a);
                                self.recorder_events(ev);
                            }
                            if let Some(o) = ogg.as_ref() {
                                for (title, _) in comments.feed(&a) {
                                    self.mark_title(&title);
                                }
                                o.feed(a);
                            } else if let (Some(sp), Some(sk)) = (splitter.as_mut(), sink.as_mut()) {
                                let frames = sp.feed(&a);
                                if let Err(m) = self.decode_frames(sk, frames) {
                                    failure = Some(m);
                                    break;
                                }
                            }
                        }
                        IcyEvent::Metadata(m) => {
                            if let Some(title) = icy::stream_title(&m) {
                                if let Some(r) = recorder.as_mut() {
                                    let ev = r.handle_title(&title);
                                    self.recorder_events(ev);
                                }
                                self.mark_title(&title);
                            }
                        }
                    }
                }
                if let Some(m) = failure {
                    break Err(m);
                }
            };
            result?;
            if let Some(r) = recorder.as_mut() {
                let ev = r.stream_ended();
                self.recorder_events(ev);
            }
            if self.stopped() {
                return Ok(());
            }
            attempts += 1;
            if attempts > RECONNECTS {
                return Err("The stream was interrupted.".into());
            }
            self.send(Event::Info(format!("reconnecting ({attempts})")));
            thread::sleep(Duration::from_millis(if attempts == 1 { 200 } else { 1500 }));
        }
    }
}

/// HLS needs the stream's helpers; this hands them over without exposing `Stream`.
fn self_ref(s: &mut Stream) -> hls::Host<'_> {
    hls::Host {
        user_agent: &s.config.user_agent,
        stop: &s.stop,
        tx: &s.tx,
        push: Box::new(|resampler, block| s_push(s.output.clone(), s.timeline.clone(), s.stop.clone(), resampler, block)),
        mark: Box::new({
            let tl = s.timeline.clone();
            move |title| {
                let title = title.trim();
                if !title.is_empty() {
                    let at = tl.pushed.load(Ordering::Relaxed);
                    tl.markers.lock().unwrap().push_back((at, title.to_string()));
                }
            }
        }),
        buffered: Box::new({
            let o = s.output.clone();
            move || o.buffered_seconds()
        }),
    }
}

fn s_push(output: Arc<OutputShared>, timeline: Arc<Timeline>, stop: Arc<AtomicBool>, resampler: &mut Option<Resampler>, b: &PcmBlock) {
    let (rate, ch) = (output.rate, output.channels);
    if !resampler.as_ref().is_some_and(|r| r.matches(b.rate, b.channels as usize)) {
        *resampler = Some(Resampler::new(b.rate, b.channels as usize, rate, ch));
    }
    let out = resampler.as_mut().unwrap().process(&b.samples);
    output.push(&out);
    timeline.pushed.fetch_add((out.len() / ch) as u64, Ordering::Relaxed);
    while output.buffered_seconds() > MAX_BUFFER_SECONDS && !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(50));
    }
}

fn read_some(r: &mut dyn Read, buf: &mut [u8]) -> usize {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => n += k,
        }
    }
    n
}

fn is_hls_type(ct: Option<&str>) -> bool {
    let ct = ct.unwrap_or("").to_ascii_lowercase();
    ct.contains("vnd.apple.mpegurl") || ct.contains("application/x-mpegurl")
}

fn kind_of(ct: Option<&str>) -> Option<Kind> {
    if let Some(f) = RecordableFormat::from_content_type(ct) {
        return Some(Kind::Frames(f));
    }
    let ct = ct.unwrap_or("").to_ascii_lowercase();
    if ct.contains("ogg") || ct.contains("vorbis") {
        return Some(Kind::Ogg);
    }
    if is_hls_type(Some(&ct)) {
        return Some(Kind::Hls);
    }
    None
}

fn sniff(b: &[u8]) -> Option<Kind> {
    if b.starts_with(b"OggS") {
        return Some(Kind::Ogg);
    }
    if b.starts_with(b"ID3") {
        return Some(Kind::Frames(RecordableFormat::Mp3));
    }
    for i in 0..b.len().saturating_sub(1) {
        if b[i] == 0xFF {
            if b[i + 1] & 0xF6 == 0xF0 {
                return Some(Kind::Frames(RecordableFormat::Aac));
            }
            if b[i + 1] & 0xE0 == 0xE0 {
                return Some(Kind::Frames(RecordableFormat::Mp3));
            }
        }
    }
    None
}

// MARK: Ogg (Vorbis) through symphonia's demuxer, fed through a pipe

struct OggPipe {
    tx: Sender<Vec<u8>>,
}

struct PipeReader {
    /// In a Mutex only because symphonia's sources must be Sync.
    rx: Mutex<Receiver<Vec<u8>>>,
    cur: Vec<u8>,
    pos: usize,
}

impl Read for PipeReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        while self.pos >= self.cur.len() {
            match self.rx.lock().unwrap().recv() {
                Ok(v) => {
                    self.cur = v;
                    self.pos = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let n = out.len().min(self.cur.len() - self.pos);
        out[..n].copy_from_slice(&self.cur[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

impl OggPipe {
    fn start(output: Arc<OutputShared>, timeline: Arc<Timeline>, stop: Arc<AtomicBool>) -> OggPipe {
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        thread::Builder::new()
            .name("driftwave-ogg".into())
            .spawn(move || {
                use symphonia::core::codecs::audio::AudioDecoderOptions;
                use symphonia::core::codecs::CodecParameters;
                use symphonia::core::formats::{FormatOptions, FormatReader};
                use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions, ReadOnlySource};
                let reader = PipeReader { rx: Mutex::new(rx), cur: Vec::new(), pos: 0 };
                let mss = MediaSourceStream::new(Box::new(ReadOnlySource::new(reader)), MediaSourceStreamOptions::default());
                let mut resampler: Option<Resampler> = None;
                let Ok(mut format) = symphonia::default::formats::OggReader::try_new(mss, FormatOptions::default()) else { return };
                let mut decoder: Option<(u32, symphonia::default::codecs::VorbisDecoder)> = None;
                while !stop.load(Ordering::Relaxed) {
                    let packet = match format.next_packet() {
                        Ok(Some(p)) => p,
                        Ok(None) | Err(_) => break,
                    };
                    if decoder.as_ref().map(|d| d.0) != Some(packet.track_id) {
                        let params = format.tracks().iter().find(|t| t.id == packet.track_id).and_then(|t| t.codec_params.clone());
                        let Some(CodecParameters::Audio(p)) = params else { continue };
                        match symphonia::default::codecs::VorbisDecoder::try_new(&p, &AudioDecoderOptions::default()) {
                            Ok(d) => decoder = Some((packet.track_id, d)),
                            Err(_) => continue,
                        }
                    }
                    let (_, d) = decoder.as_mut().unwrap();
                    use symphonia::core::codecs::audio::AudioDecoder;
                    if d.decode(&packet).is_err() {
                        continue;
                    }
                    let buf = d.last_decoded();
                    if buf.frames() == 0 {
                        continue;
                    }
                    let mut samples = Vec::new();
                    buf.copy_to_vec_interleaved(&mut samples);
                    let block = PcmBlock { rate: buf.spec().rate(), channels: buf.spec().channels().count() as u16, samples };
                    s_push(output.clone(), timeline.clone(), stop.clone(), &mut resampler, &block);
                }
            })
            .expect("ogg thread");
        OggPipe { tx }
    }

    fn feed(&self, data: Vec<u8>) {
        let _ = self.tx.send(data);
    }
}
