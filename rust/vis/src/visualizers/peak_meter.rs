//! Peak Meter: the built-in spectrum of the classic players, grown up. Segmented LED bars from green through amber
//! to hot orange, peak caps that hang a moment then fall under gravity, a faint reflection on the glass floor, and an
//! oscilloscope trace glowing underneath.

use super::{band_at, Visualizer};
use crate::analysis::Analysis;
use crate::canvas::{mix, scale, Canvas, Rgb};

#[derive(Default)]
pub struct PeakMeter {
    peaks: Vec<f32>,
    hold: Vec<f32>,
    fall: Vec<f32>,
    /// The still background (gradient and scale lines), drawn once per size and copied each frame.
    backdrop: Option<Canvas>,
}

const GREEN: Rgb = [0.12, 0.85, 0.35];
const AMBER: Rgb = [0.98, 0.78, 0.12];
const HOT: Rgb = [1.0, 0.38, 0.12];

impl Visualizer for PeakMeter {
    fn id(&self) -> &'static str {
        "peak-meter"
    }
    fn name(&self) -> &'static str {
        "Peak Meter"
    }
    fn blurb(&self) -> &'static str {
        "LED spectrum bars with falling peak caps over a glowing oscilloscope"
    }

    fn render(&mut self, a: &Analysis, c: &mut Canvas) {
        let (w, h) = (c.w as f32, c.h as f32);
        let u = (h / 120.0).max(0.6); // one "LED unit" scales with the slot
        let top = h * 0.06;
        let floor = h * 0.66;
        let span = floor - top;
        if self.backdrop.as_ref().map(|b| (b.w, b.h)) != Some((c.w, c.h)) {
            let mut b = Canvas::new(c.w, c.h);
            b.gradient([0.012, 0.016, 0.03], [0.02, 0.03, 0.05]);
            for k in 1..4 {
                let y = top + span * k as f32 / 4.0;
                b.fill_rect(0.0, y, w, y + u * 0.5, [0.05, 0.07, 0.1]);
            }
            self.backdrop = Some(b);
        }
        c.px.copy_from_slice(&self.backdrop.as_ref().expect("backdrop").px);
        let bars = ((w / (7.0 * u)) as usize).clamp(12, 64);
        if self.peaks.len() != bars {
            self.peaks = vec![0.0; bars];
            self.hold = vec![0.0; bars];
            self.fall = vec![0.0; bars];
        }
        let slot = w / bars as f32;
        let bar_w = (slot * 0.72).max(1.0);
        let seg = 2.0 * u;
        let gap = 1.0 * u;
        let dt = a.dt.max(1e-3);
        for b in 0..bars {
            // Each bar covers its share of the bands; it shows the loudest.
            let x0 = b as f32 / bars as f32;
            let x1 = (b + 1) as f32 / bars as f32;
            let v = band_at(a, x0).max(band_at(a, (x0 + x1) * 0.5)).max(band_at(a, x1)).powf(1.3);
            let x = b as f32 * slot + (slot - bar_w) * 0.5;
            let height = v * span;
            let segs = (height / (seg + gap)).floor() as usize;
            for s in 0..segs {
                let y1 = floor - s as f32 * (seg + gap);
                let f = (s as f32 * (seg + gap)) / span;
                let col = if f < 0.55 { mix(GREEN, AMBER, f / 0.55) } else { mix(AMBER, HOT, ((f - 0.55) / 0.45).min(1.0)) };
                c.fill_rect(x, y1 - seg, x + bar_w, y1, col);
                // A soft reflection on the floor.
                let ry = floor + 2.0 * u + s as f32 * (seg + gap);
                if ry < h * 0.74 {
                    c.fill_rect(x, ry, x + bar_w, ry + seg, scale(col, 0.12 * (1.0 - s as f32 / 6.0).max(0.0)));
                }
            }
            // Peak caps: hang for a moment, then fall with gravity.
            if v >= self.peaks[b] {
                self.peaks[b] = v;
                self.hold[b] = 0.35;
                self.fall[b] = 0.0;
            } else if self.hold[b] > 0.0 {
                self.hold[b] -= dt;
            } else {
                self.fall[b] += 2.2 * dt;
                self.peaks[b] = (self.peaks[b] - self.fall[b] * dt).max(0.0);
            }
            let py = floor - self.peaks[b] * span - u;
            c.fill_rect(x, py - u * 1.2, x + bar_w, py, [0.85, 0.92, 1.0]);
        }
        // The oscilloscope: the mix, with a glow, under the floor.
        let mid = h * 0.86;
        let amp = h * 0.11;
        let n = a.wave.len();
        let mut prev = (0.0, mid);
        let col = [0.4, 0.68, 1.0];
        for (i, &sample) in a.wave.iter().enumerate().take(n) {
            let x = i as f32 / (n - 1) as f32 * w;
            let y = mid - sample * amp;
            if i > 0 {
                c.line(prev.0, prev.1, x, y, col, 0.9, 1.6 * u);
            }
            prev = (x, y);
        }
    }
}
