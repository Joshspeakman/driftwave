//! Waveland: the 3D spectrum terrain. The spectrum becomes a landscape scrolling towards you: each moment a ridge
//! line on the horizon that rolls closer and grows, in neon wireframe over a dusk sky, with the camera gently
//! swaying.

use super::{band_at, Visualizer};
use crate::analysis::Analysis;
use crate::canvas::{hsv, scale, Canvas};

const ROWS: usize = 42;
const COLS: usize = 56;
/// New ridges a second.
const RATE: f32 = 14.0;

#[derive(Default)]
pub struct Waveland {
    rows: Vec<Vec<f32>>,
    since: f32,
    /// The dusk sky, drawn once per size and copied each frame.
    sky: Option<Canvas>,
}

impl Visualizer for Waveland {
    fn id(&self) -> &'static str {
        "waveland"
    }
    fn name(&self) -> &'static str {
        "Waveland"
    }
    fn blurb(&self) -> &'static str {
        "The spectrum as a neon wireframe landscape rolling in from the horizon"
    }
    fn resolution(&self) -> f32 {
        0.8
    }

    fn render(&mut self, a: &Analysis, c: &mut Canvas) {
        let (w, h) = (c.w as f32, c.h as f32);
        let dt = a.dt.max(1e-3);
        if self.rows.is_empty() {
            self.rows = vec![vec![0.0; COLS]; ROWS];
        }
        // A new ridge every 1/RATE s; between, everything slides smoothly by the fraction.
        self.since += dt * RATE;
        while self.since >= 1.0 {
            self.since -= 1.0;
            let row: Vec<f32> = (0..COLS)
                .map(|k| {
                    // Mirror the spectrum so the bass rises in the middle of the land.
                    let x = (k as f32 / (COLS - 1) as f32 - 0.5).abs() * 2.0;
                    band_at(a, x).powf(1.4)
                })
                .collect();
            self.rows.pop();
            self.rows.insert(0, row);
        }
        let t = a.time as f32;
        let horizon = h * (0.34 + 0.015 * (t * 0.3).sin());
        if self.sky.as_ref().map(|s| (s.w, s.h)) != Some((c.w, c.h)) {
            let mut s = Canvas::new(c.w, c.h);
            s.gradient([0.02, 0.01, 0.06], [0.05, 0.02, 0.09]);
            // Glow along the (resting) horizon.
            let rest = h * 0.35;
            for y in 0..(rest as usize).min(c.h) {
                let k = (y as f32 / rest).powi(3) * 0.25;
                for x in 0..c.w {
                    let p = &mut s.px[y * c.w + x];
                    p[0] += 0.5 * k;
                    p[1] += 0.12 * k;
                    p[2] += 0.45 * k;
                }
            }
            self.sky = Some(s);
        }
        c.px.copy_from_slice(&self.sky.as_ref().expect("sky").px);
        let sway = (t * 0.2).sin() * w * 0.03;
        let project = |col: usize, depth: f32, v: f32| -> (f32, f32) {
            let s = 1.0 / depth;
            let x = w * 0.5 + sway * s + (col as f32 / (COLS - 1) as f32 - 0.5) * w * 1.8 * s;
            let y = horizon + (h - horizon) * s * 0.9 - v * h * 0.55 * s;
            (x, y)
        };
        // Far to near, so near ridges draw over far ones.
        for r in (0..ROWS).rev() {
            let depth = 1.0 + (r as f32 + self.since) * 0.22;
            let near = 1.0 - (r as f32 + self.since) / ROWS as f32;
            let fade = near.clamp(0.0, 1.0).powf(1.2);
            let mut prev: Option<(f32, f32)> = None;
            for k in 0..COLS {
                let v = self.rows[r][k];
                let p = project(k, depth, v);
                let col = scale(hsv(0.52 + v * 0.38 - r as f32 * 0.002, 0.8, 1.0), 0.15 + 0.75 * fade);
                if let Some(q) = prev {
                    c.line(q.0, q.1, p.0, p.1, col, 0.9, if r < 6 { 1.6 } else { 1.0 });
                }
                // Cross lines to the next ridge back, every few columns: the wireframe.
                if k % 4 == 0 && r + 1 < ROWS {
                    let back = project(k, depth + 0.22, self.rows[r + 1][k]);
                    c.line(p.0, p.1, back.0, back.1, scale(col, 0.45), 0.7, 1.0);
                }
                prev = Some(p);
            }
        }
    }
}
