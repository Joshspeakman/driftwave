//! Scope Studio: the modular "superscope" style, where a few lines of maths per point became whole worlds. Three
//! scopes share a softly smearing stage: a 3D ring that the waveform pushes in and out as it tumbles (faster with the
//! bass), a starburst of the spectrum turning behind it, and a Lissajous of left against right drifting at the
//! centre.

use super::Visualizer;
use crate::analysis::{Analysis, BANDS};
use crate::canvas::{hsv, Canvas};

#[derive(Default)]
pub struct ScopeStudio {
    rx: f32,
    ry: f32,
    burst: f32,
    tmp: Vec<[f32; 3]>,
}

impl Visualizer for ScopeStudio {
    fn id(&self) -> &'static str {
        "scope-studio"
    }
    fn name(&self) -> &'static str {
        "Scope Studio"
    }
    fn blurb(&self) -> &'static str {
        "A tumbling 3D ring, a turning spectrum starburst and a stereo Lissajous on a smearing stage"
    }
    fn resolution(&self) -> f32 {
        0.6
    }

    fn render(&mut self, a: &Analysis, c: &mut Canvas) {
        let (w, h) = (c.w, c.h);
        let dt = a.dt.max(1e-3);
        // The stage smears: fade and soften towards the neighbours.
        let keep = 0.08f32.powf(dt);
        self.tmp.resize(w * h, [0.0; 3]);
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let l = c.px[y * w + x.saturating_sub(1)];
                let r = c.px[y * w + (x + 1).min(w - 1)];
                let u = c.px[y.saturating_sub(1) * w + x];
                let d = c.px[(y + 1).min(h - 1) * w + x];
                let s = c.px[i];
                self.tmp[i] = [
                    (s[0] * 0.6 + (l[0] + r[0] + u[0] + d[0]) * 0.1) * keep,
                    (s[1] * 0.6 + (l[1] + r[1] + u[1] + d[1]) * 0.1) * keep,
                    (s[2] * 0.6 + (l[2] + r[2] + u[2] + d[2]) * 0.1) * keep,
                ];
            }
        }
        std::mem::swap(&mut c.px, &mut self.tmp);

        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let size = w.min(h) as f32;
        let t = a.time as f32;
        let hue = t * 0.015;
        // Small pictures get lighter lines, or the scopes pile up into white.
        let lw = (size / 300.0).clamp(0.3, 1.0);

        // Scope 1: the spectrum starburst, turning slowly behind everything.
        self.burst += dt * (0.12 + 0.1 * a.mid.min(2.0));
        for (b, &band) in a.bands.iter().enumerate().take(BANDS) {
            let ang = b as f32 / BANDS as f32 * std::f32::consts::TAU + self.burst;
            let len = size * (0.12 + 0.42 * band);
            let col = hsv(hue + 0.5 + b as f32 / BANDS as f32 * 0.2, 0.7, 0.35);
            c.line(cx + ang.cos() * size * 0.1, cy + ang.sin() * size * 0.1, cx + ang.cos() * len, cy + ang.sin() * len, col, 0.6 * lw, (1.2 * lw).max(0.9));
        }

        // Scope 2: a ring in 3D, pushed by the waveform, tumbling.
        self.rx += dt * (0.25 + 0.5 * a.bass.min(3.0) * 0.3);
        self.ry += dt * 0.17;
        let (sx, cxr) = self.rx.sin_cos();
        let (sy, cyr) = self.ry.sin_cos();
        let n = 240;
        let mut first = None;
        let mut prev: Option<(f32, f32, f32)> = None;
        for k in 0..=n {
            let f = (k % n) as f32 / n as f32;
            let ang = f * std::f32::consts::TAU;
            let wv = a.wave[(f * (a.wave.len() - 1) as f32) as usize];
            let r = 1.0 + 0.45 * wv;
            let (x, y, z) = (ang.cos() * r, ang.sin() * r, (ang * 3.0 + t * 0.7).sin() * 0.25);
            // Turn about X, then about Y.
            let (y, z) = (y * cxr - z * sx, y * sx + z * cxr);
            let (x, z) = (x * cyr + z * sy, -x * sy + z * cyr);
            let d = 3.2 / (3.2 + z);
            let (px, py) = (cx + x * size * 0.3 * d, cy + y * size * 0.3 * d);
            let col = hsv(hue + f * 0.2, 0.65, 0.5 + 0.4 * d.min(1.2) - 0.2);
            if let Some((qx, qy, _)) = prev {
                c.line(qx, qy, px, py, col, 0.9 * lw, (1.8 * lw).max(1.0));
            }
            if first.is_none() {
                first = Some((px, py));
            }
            prev = Some((px, py, z));
        }

        // Scope 3: left against right, a drifting Lissajous of dots at the centre.
        let rot = t * 0.1;
        let (rs, rc) = rot.sin_cos();
        for k in (0..a.wave_l.len()).step_by(2) {
            let (l, r) = (a.wave_l[k], a.wave_r[k]);
            let (x, y) = ((l - r) * 0.7, (l + r) * 0.7);
            let (x, y) = (x * rc - y * rs, x * rs + y * rc);
            c.splat(cx + x * size * 0.3, cy - y * size * 0.3, hsv(hue + 0.15, 0.3, 1.0), 0.5 * lw);
        }
    }
}
