//! What the visualizers see of the music, worked out once per frame.

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

/// Samples per channel in the waveform (the length the classic players handed their plug-ins).
pub const WAVE_LEN: usize = 576;
/// Log-spaced bands from 40 Hz to 16 kHz.
pub const BANDS: usize = 64;
const FFT_LEN: usize = 1024;

#[derive(Debug, Clone, Default)]
pub struct Analysis {
    /// The last [`WAVE_LEN`] samples of each channel and their mix, −1 to 1.
    pub wave_l: Vec<f32>,
    pub wave_r: Vec<f32>,
    pub wave: Vec<f32>,
    /// [`BANDS`] bands, 0 to 1: quick to rise, slower to fall.
    pub bands: Vec<f32>,
    /// Band energy relative to its own recent average (1 = as usual): what's loud for this song, not in general.
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    /// RMS level, 0 to 1.
    pub level: f32,
    /// A smooth beat envelope, 0 to 1: rises over about 60 ms on a kick and falls over about 400 ms.
    pub beat: f32,
    /// Seconds since the host started, and since the last frame.
    pub time: f64,
    pub dt: f32,
}

pub struct Analyzer {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    buf: Vec<Complex<f32>>,
    a: Analysis,
    avg: [f32; 3],
    prev_bass: f32,
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Exponential smoothing factor for a time constant `tau` over `dt`.
pub fn ease(dt: f32, tau: f32) -> f32 {
    1.0 - (-dt / tau.max(1e-4)).exp()
}

impl Analyzer {
    pub fn new() -> Analyzer {
        let fft = FftPlanner::new().plan_fft_forward(FFT_LEN);
        let window = (0..FFT_LEN).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (FFT_LEN - 1) as f32).cos()).collect();
        let a = Analysis {
            wave_l: vec![0.0; WAVE_LEN],
            wave_r: vec![0.0; WAVE_LEN],
            wave: vec![0.0; WAVE_LEN],
            bands: vec![0.0; BANDS],
            bass: 1.0,
            mid: 1.0,
            treble: 1.0,
            ..Default::default()
        };
        Analyzer { fft, window, buf: vec![Complex::new(0.0, 0.0); FFT_LEN], a, avg: [0.05; 3], prev_bass: 0.0 }
    }

    pub fn last(&self) -> &Analysis {
        &self.a
    }

    /// `samples`: interleaved, the most recent last (what the engine's scope tap returns).
    pub fn analyse(&mut self, samples: &[f32], channels: usize, rate: u32, time: f64, dt: f32) -> &Analysis {
        let ch = channels.max(1);
        let frames = samples.len() / ch;
        let dt = dt.clamp(0.0, 0.25);
        let a = &mut self.a;
        a.time = time;
        a.dt = dt;
        let at = |f: usize, c: usize| -> f32 { samples[f * ch + c.min(ch - 1)] };

        // Waveform: the last WAVE_LEN frames.
        let start = frames.saturating_sub(WAVE_LEN);
        let mut sq = 0.0f32;
        for i in 0..WAVE_LEN {
            let f = start + i;
            let (l, r) = if f < frames { (at(f, 0), at(f, 1)) } else { (0.0, 0.0) };
            a.wave_l[i] = l.clamp(-1.0, 1.0);
            a.wave_r[i] = r.clamp(-1.0, 1.0);
            a.wave[i] = ((l + r) * 0.5).clamp(-1.0, 1.0);
            sq += a.wave[i] * a.wave[i];
        }
        a.level = (sq / WAVE_LEN as f32).sqrt().min(1.0);

        // Spectrum of the mix.
        let start = frames.saturating_sub(FFT_LEN);
        for i in 0..FFT_LEN {
            let f = start + i;
            let v = if f < frames { (at(f, 0) + at(f, 1)) * 0.5 } else { 0.0 };
            self.buf[i] = Complex::new(v * self.window[i], 0.0);
        }
        self.fft.process(&mut self.buf);
        let rate = rate.max(8000) as f32;
        let bin_hz = rate / FFT_LEN as f32;
        let (lo, hi) = (40.0f32, 16000.0f32.min(rate / 2.0));
        let (up, down) = (ease(dt, 0.025), ease(dt, 0.2));
        let mut sums = [0.0f32; 3];
        let mut counts = [0usize; 3];
        for (b, band) in a.bands.iter_mut().enumerate().take(BANDS) {
            let f0 = lo * (hi / lo).powf(b as f32 / BANDS as f32);
            let f1 = lo * (hi / lo).powf((b + 1) as f32 / BANDS as f32);
            let (k0, k1) = ((f0 / bin_hz) as usize, ((f1 / bin_hz) as usize).max((f0 / bin_hz) as usize + 1));
            let m = self.buf.get(k0..k1.min(FFT_LEN / 2)).unwrap_or(&[]).iter().fold(0.0f32, |m, c| m.max(c.norm()));
            // Magnitude to 0..1 over a 60 dB range.
            let db = 20.0 * (m / (FFT_LEN as f32 * 0.25) + 1e-9).log10();
            let v = ((db + 66.0) / 60.0).clamp(0.0, 1.0);
            let k = if v > *band { up } else { down };
            *band += (v - *band) * k;
            let group = if f1 <= 250.0 { 0 } else if f1 <= 2500.0 { 1 } else { 2 };
            sums[group] += v * v;
            counts[group] += 1;
        }
        // Each range against its own slow average: 1 is "as usual for this song".
        let mut rel = [0.0f32; 3];
        for g in 0..3 {
            let e = sums[g] / counts[g].max(1) as f32;
            self.avg[g] += (e - self.avg[g]) * ease(dt, 4.0);
            rel[g] = (e / self.avg[g].max(0.002)).min(4.0);
        }
        let smooth = ease(dt, 0.08);
        a.bass += (rel[0] - a.bass) * smooth;
        a.mid += (rel[1] - a.mid) * smooth;
        a.treble += (rel[2] - a.treble) * smooth;
        // Beat: bass well above its average and rising. The envelope only ever eases; it never jumps.
        let target = if rel[0] > 1.35 && rel[0] > self.prev_bass { ((rel[0] - 1.35) * 1.5).min(1.0) } else { 0.0 };
        self.prev_bass = rel[0];
        let k = if target > a.beat { ease(dt, 0.06) } else { ease(dt, 0.4) };
        a.beat += (target - a.beat) * k;
        &self.a
    }
}

/// A test signal: interleaved stereo at `rate`, `seconds` long, made by `f(t) -> (l, r)`.
pub fn synth(rate: u32, seconds: f64, mut f: impl FnMut(f64) -> (f32, f32)) -> Vec<f32> {
    let n = (rate as f64 * seconds) as usize;
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let (l, r) = f(i as f64 / rate as f64);
        out.push(l);
        out.push(r);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_lands_in_its_band_and_kicks_make_beats() {
        let rate = 44100;
        let tone = synth(rate, 0.2, |t| ((t * 1000.0 * std::f64::consts::TAU).sin() as f32 * 0.5, 0.0));
        let mut an = Analyzer::new();
        for _ in 0..10 {
            an.analyse(&tone, 2, rate, 0.0, 1.0 / 30.0);
        }
        let a = an.last();
        let peak = (0..BANDS).max_by(|&x, &y| a.bands[x].total_cmp(&a.bands[y])).unwrap();
        let f0 = 40.0 * (16000.0f32 / 40.0).powf(peak as f32 / BANDS as f32);
        let f1 = 40.0 * (16000.0f32 / 40.0).powf((peak + 1) as f32 / BANDS as f32);
        assert!(f0 <= 1100.0 && f1 >= 900.0, "1 kHz in band {peak} ({f0}-{f1} Hz)");
        assert!(a.level > 0.1);

        // Quiet hum, then kicks every half second: the beat envelope must rise.
        let song = synth(rate, 6.0, |t| {
            let kick = if t % 0.5 < 0.08 { ((t % 0.5) * 60.0 * std::f64::consts::TAU).sin() * (1.0 - (t % 0.5) / 0.08) } else { 0.0 };
            let v = (kick * 0.9 + (t * 3000.0 * std::f64::consts::TAU).sin() * 0.05) as f32;
            (v, v)
        });
        let mut an = Analyzer::new();
        let mut max_beat = 0.0f32;
        let step = rate as usize / 30;
        let mut f = 2048;
        while f * 2 <= song.len() {
            let a = an.analyse(&song[(f - 2048) * 2..f * 2], 2, rate, f as f64 / rate as f64, 1.0 / 30.0);
            max_beat = max_beat.max(a.beat);
            f += step;
        }
        assert!(max_beat > 0.3, "beat {max_beat}");
    }
}
