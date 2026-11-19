//! The output device: a queue of interleaved samples at the device's rate, drained by cpal's callback.
//!
//! **Silent test runs:** `silent` is fixed when the output is made and applied as the callback's last
//! step, after volume and mute, so nothing the controls do can make a test run audible.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Frames kept for the meters, before volume and mute (what's playing, not what's heard).
pub const TAP_FRAMES: usize = 4096;

pub struct OutputShared {
    pub rate: u32,
    pub channels: usize,
    /// Fixed at creation: write silence whatever volume and mute say.
    pub silent: bool,
    queue: Mutex<VecDeque<f32>>,
    tap: Mutex<VecDeque<f32>>,
    /// Frames handed to the device since the start.
    played: AtomicU64,
    underruns: AtomicU64,
    /// Drain the queue (playing) or hold it (buffering).
    running: AtomicBool,
    volume: AtomicU32,
    muted: AtomicBool,
    stop: AtomicBool,
    pub device_name: Mutex<String>,
    /// Diagnostics: with DRIFTWAVE_OUTPUT_WAV=<file>, up to 30 s of what goes to the device (after volume, before mute
    /// and a test run's silence), written as a WAV when the output stops.
    capture: Option<(Mutex<Vec<f32>>, std::path::PathBuf)>,
    captured: AtomicBool,
}

impl OutputShared {
    pub fn new(rate: u32, channels: usize, silent: bool) -> Arc<OutputShared> {
        Arc::new(OutputShared {
            rate,
            channels,
            silent,
            queue: Mutex::new(VecDeque::new()),
            tap: Mutex::new(VecDeque::with_capacity(TAP_FRAMES * channels)),
            played: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            running: AtomicBool::new(false),
            volume: AtomicU32::new(1f32.to_bits()),
            muted: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            device_name: Mutex::new(String::new()),
            capture: std::env::var_os("DRIFTWAVE_OUTPUT_WAV").map(|p| (Mutex::new(Vec::new()), std::path::PathBuf::from(p))),
            captured: AtomicBool::new(false),
        })
    }

    /// Writes the diagnostic capture (32-bit float WAV).
    fn write_capture(&self) {
        let Some((buf, path)) = &self.capture else { return };
        // Once: stop can come twice (at quit, and as the engine is dropped).
        if self.captured.swap(true, Ordering::Relaxed) {
            return;
        }
        let data = std::mem::take(&mut *buf.lock().unwrap());
        let (ch, rate) = (self.channels as u32, self.rate);
        let bytes = (data.len() * 4) as u32;
        let mut out = Vec::with_capacity(44 + bytes as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + bytes).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
        out.extend_from_slice(&(ch as u16).to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * ch * 4).to_le_bytes());
        out.extend_from_slice(&((ch * 4) as u16).to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&bytes.to_le_bytes());
        for v in data {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let _ = std::fs::write(path, out);
    }

    pub fn push(&self, samples: &[f32]) {
        self.queue.lock().unwrap().extend(samples.iter().copied());
    }

    pub fn clear(&self) {
        self.queue.lock().unwrap().clear();
    }

    pub fn buffered_frames(&self) -> usize {
        self.queue.lock().unwrap().len() / self.channels
    }

    pub fn buffered_seconds(&self) -> f64 {
        self.buffered_frames() as f64 / self.rate as f64
    }

    pub fn played_frames(&self) -> u64 {
        self.played.load(Ordering::Relaxed)
    }

    pub fn underruns(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }

    pub fn set_running(&self, on: bool) {
        self.running.store(on, Ordering::Relaxed);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn set_volume(&self, v: f32) {
        self.volume.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    pub fn set_muted(&self, m: bool) {
        self.muted.store(m, Ordering::Relaxed);
    }

    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// The last `frames` frames played (before volume and mute), interleaved.
    pub fn tap(&self, frames: usize) -> Vec<f32> {
        let t = self.tap.lock().unwrap();
        let n = (frames * self.channels).min(t.len());
        t.iter().skip(t.len() - n).copied().collect()
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.write_capture();
    }

    /// Fills one device buffer. Public so tests can check what reaches the device.
    pub fn fill(&self, out: &mut [f32]) {
        let mut wrote = 0;
        if self.is_running() {
            let mut q = self.queue.lock().unwrap();
            let n = out.len().min(q.len());
            for (o, s) in out.iter_mut().zip(q.drain(..n)) {
                *o = s;
            }
            wrote = n;
            if n < out.len() {
                self.underruns.fetch_add(1, Ordering::Relaxed);
            }
        }
        for o in &mut out[wrote..] {
            *o = 0.0;
        }
        self.played.fetch_add((wrote / self.channels) as u64, Ordering::Relaxed);
        if wrote > 0 {
            let mut t = self.tap.lock().unwrap();
            t.extend(out[..wrote].iter().copied());
            let max = TAP_FRAMES * self.channels;
            if t.len() > max {
                let extra = t.len() - max;
                t.drain(..extra);
            }
        }
        // The diagnostic capture: the signal as the volume leaves it (mute aside, so a muted run captures too).
        if let Some((buf, _)) = self.capture.as_ref().filter(|_| !self.captured.load(Ordering::Relaxed)) {
            let mut b = buf.lock().unwrap();
            if b.len() < 30 * self.rate as usize * self.channels {
                let v = self.volume();
                b.extend(out.iter().map(|o| o * v));
            }
        }
        let gain = if self.is_muted() { 0.0 } else { self.volume() };
        for o in out.iter_mut() {
            *o *= gain;
        }
        // Last of all: a silent (test) run writes nothing but zeros.
        if self.silent {
            out.fill(0.0);
        }
    }
}

/// Opens the default output device and starts draining `shared` into it; the device's rate and
/// channels decide `shared`'s. Without a device, a timer drains it in real time instead.
pub fn start(silent: bool) -> Arc<OutputShared> {
    let host = cpal::default_host();
    let device = host.default_output_device();
    let config = device.as_ref().and_then(|d| d.default_output_config().ok());
    let (rate, channels) = config.as_ref().map(|c| (c.sample_rate(), c.channels() as usize)).unwrap_or((48000, 2));
    let shared = OutputShared::new(rate, channels.max(1), silent);
    let (Some(device), Some(config)) = (device, config) else {
        *shared.device_name.lock().unwrap() = "none (timer)".into();
        spawn_timer(shared.clone());
        return shared;
    };
    let s = shared.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    // The cpal stream isn't Send on every platform, so it lives on its own thread.
    thread::Builder::new()
        .name("driftwave-output".into())
        .spawn(move || {
            let name = device.description().map(|d| d.to_string()).unwrap_or_else(|_| "default".into());
            *s.device_name.lock().unwrap() = name;
            let cb = s.clone();
            let stream = device.build_output_stream::<f32, _, _>(
                config.config(),
                move |data: &mut [f32], _| cb.fill(data),
                |e| eprintln!("[driftwave] output error: {e}"),
                None,
            );
            let stream = match stream {
                Ok(st) => st,
                Err(e) => {
                    eprintln!("[driftwave] output unavailable ({e}); using a timer");
                    let _ = ready_tx.send(false);
                    return;
                }
            };
            let _ = stream.play();
            let _ = ready_tx.send(true);
            while !s.stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(100));
            }
            drop(stream);
        })
        .expect("output thread");
    if ready_rx.recv_timeout(Duration::from_secs(5)) != Ok(true) {
        spawn_timer(shared.clone());
    }
    shared
}

fn spawn_timer(s: Arc<OutputShared>) {
    thread::spawn(move || {
        let chunk = (s.rate as usize / 100) * s.channels; // 10 ms
        let mut buf = vec![0f32; chunk];
        while !s.stop.load(Ordering::Relaxed) {
            s.fill(&mut buf);
            thread::sleep(Duration::from_millis(10));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_run_writes_zeros_at_full_volume_unmuted() {
        let s = OutputShared::new(48000, 2, true);
        s.set_volume(1.0);
        s.set_muted(false);
        s.push(&[0.5; 512]);
        s.set_running(true);
        let mut out = [1.0f32; 256];
        s.fill(&mut out);
        assert!(out.iter().all(|&v| v == 0.0), "a silent run must never reach the device");
        assert_eq!(s.played_frames(), 128, "but it still plays (time moves on)");
        assert_eq!(s.tap(128).len(), 256, "and the meters see the music");
        assert!(s.tap(128).iter().all(|&v| v == 0.5), "the tap holds the samples, not the silence");
        assert_eq!(s.tap(10_000).len(), 256, "asking for more than there is returns what there is");
    }

    #[test]
    fn audible_run_follows_volume_and_mute() {
        let s = OutputShared::new(48000, 2, false);
        s.push(&[0.5; 8]);
        s.set_running(true);
        s.set_volume(0.5);
        let mut out = [0f32; 4];
        s.fill(&mut out);
        assert_eq!(out, [0.25; 4]);
        s.set_muted(true);
        s.fill(&mut out);
        assert_eq!(out, [0.0; 4]);
    }

    #[test]
    fn buffering_holds_the_queue_and_counts_underruns() {
        let s = OutputShared::new(48000, 2, true);
        s.push(&[0.1; 4]);
        let mut out = [0f32; 8];
        s.fill(&mut out); // not running: nothing taken
        assert_eq!(s.buffered_frames(), 2);
        s.set_running(true);
        s.fill(&mut out); // takes 2 frames of the 4 asked for
        assert_eq!(s.underruns(), 1);
        assert_eq!(s.played_frames(), 2);
    }
}
