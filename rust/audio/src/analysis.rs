//! Real meters: left/right levels and a spectrum, from the samples being played (before volume
//! and mute, so the meters move in a muted test run and match the music, not the volume knob).

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

pub const FFT_SIZE: usize = 2048;

#[derive(Debug, Clone, PartialEq)]
pub struct Meters {
    /// RMS level per channel, 0–1.
    pub levels: Vec<f32>,
    /// Peak (absolute) per channel, 0–1.
    pub peaks: Vec<f32>,
    /// Magnitudes of `bands` log-spaced bands from 40 Hz to 16 kHz, 0–1.
    pub bands: Vec<f32>,
    /// Frequency of the strongest FFT bin, in Hz.
    pub dominant_hz: f32,
}

pub struct Analyzer {
    planner: FftPlanner<f32>,
    window: Vec<f32>,
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer {
    pub fn new() -> Analyzer {
        let window = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos())
            .collect();
        Analyzer { planner: FftPlanner::new(), window }
    }

    /// `samples` interleaved with `channels`, the most recent last.
    pub fn measure(&mut self, samples: &[f32], channels: usize, rate: u32, bands: usize) -> Meters {
        let channels = channels.max(1);
        let frames = samples.len() / channels;
        let mut levels = vec![0f32; channels];
        let mut peaks = vec![0f32; channels];
        for f in samples.chunks(channels) {
            for (c, &v) in f.iter().enumerate() {
                levels[c] += v * v;
                peaks[c] = peaks[c].max(v.abs());
            }
        }
        for l in &mut levels {
            *l = if frames > 0 { (*l / frames as f32).sqrt() } else { 0.0 };
        }
        // Spectrum of the mono mix of the last FFT_SIZE frames.
        let n = FFT_SIZE.min(frames);
        let mut buf: Vec<Complex<f32>> = vec![Complex::new(0.0, 0.0); FFT_SIZE];
        let start = frames - n;
        for i in 0..n {
            let f = &samples[(start + i) * channels..(start + i + 1) * channels];
            let mono = f.iter().sum::<f32>() / channels as f32;
            buf[i] = Complex::new(mono * self.window[i], 0.0);
        }
        self.planner.plan_fft_forward(FFT_SIZE).process(&mut buf);
        let mags: Vec<f32> = buf[..FFT_SIZE / 2].iter().map(|c| c.norm() / (FFT_SIZE as f32 / 4.0)).collect();
        let bin_hz = rate as f32 / FFT_SIZE as f32;
        let (best, _) = mags.iter().enumerate().skip(1).fold((0, 0f32), |acc, (i, &m)| if m > acc.1 { (i, m) } else { acc });
        let mut out_bands = Vec::with_capacity(bands);
        let (lo, hi) = (40f32, 16000f32.min(rate as f32 / 2.0));
        for b in 0..bands {
            let f0 = lo * (hi / lo).powf(b as f32 / bands as f32);
            let f1 = lo * (hi / lo).powf((b + 1) as f32 / bands as f32);
            let (i0, i1) = ((f0 / bin_hz) as usize, ((f1 / bin_hz) as usize).max((f0 / bin_hz) as usize + 1));
            let m = mags[i0.min(mags.len() - 1)..i1.min(mags.len())].iter().copied().fold(0f32, f32::max);
            out_bands.push(m.min(1.0));
        }
        Meters { levels, peaks, bands: out_bands, dominant_hz: best as f32 * bin_hz }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_tone_and_the_level() {
        let rate = 48000;
        let samples: Vec<f32> = (0..FFT_SIZE * 2)
            .flat_map(|i| {
                let t = i as f32 / rate as f32;
                [0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin(), 0.0]
            })
            .collect();
        let m = Analyzer::new().measure(&samples, 2, rate, 16);
        assert!((m.dominant_hz - 440.0).abs() < 30.0, "{}", m.dominant_hz);
        assert!((m.levels[0] - 0.354).abs() < 0.02, "rms {}", m.levels[0]);
        assert_eq!(m.levels[1], 0.0);
        assert_eq!(m.bands.len(), 16);
    }
}
