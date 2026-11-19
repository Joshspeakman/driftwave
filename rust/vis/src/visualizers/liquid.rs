//! Liquid Feedback: the feedback-warp style that defined the era. Every frame the last picture is zoomed, turned and
//! rippled a little, faded a touch, and the music is drawn over it again, so shapes melt into flowing tunnels. The
//! warp drifts slowly between five original moods; kicks lean on the zoom.

use super::{mesh_warp, preset_mix, Visualizer};
use crate::analysis::Analysis;
use crate::canvas::{hsv, Canvas};

#[derive(Clone, Copy)]
struct Mood {
    zoom: f32,   // per second; above 1 pulls the picture outwards
    turn: f32,   // radians per second
    ripple: f32, // ripple strength (fraction of the size)
    rings: f32,  // ripple frequency
    fade: f32,   // per second, 0..1 kept
    drift: (f32, f32),
    shape: u8,   // 0 ring, 1 line, 2 twin spirals
    hue: f32,
}

const MOODS: [Mood; 5] = [
    Mood { zoom: 1.30, turn: 0.25, ripple: 0.004, rings: 18.0, fade: 0.30, drift: (0.0, 0.0), shape: 0, hue: 0.55 },
    Mood { zoom: 1.12, turn: -0.6, ripple: 0.010, rings: 9.0, fade: 0.40, drift: (0.01, 0.0), shape: 2, hue: 0.85 },
    Mood { zoom: 0.92, turn: 0.15, ripple: 0.006, rings: 26.0, fade: 0.35, drift: (0.0, -0.02), shape: 1, hue: 0.08 },
    Mood { zoom: 1.45, turn: 0.9, ripple: 0.002, rings: 12.0, fade: 0.25, drift: (0.0, 0.0), shape: 0, hue: 0.32 },
    Mood { zoom: 1.20, turn: -0.2, ripple: 0.014, rings: 6.0, fade: 0.45, drift: (-0.01, 0.01), shape: 2, hue: 0.68 },
];

#[derive(Default)]
pub struct LiquidFeedback {
    tmp: Vec<[f32; 3]>,
    mesh: Vec<(f32, f32)>,
    spin: f32,
}

impl Visualizer for LiquidFeedback {
    fn id(&self) -> &'static str {
        "liquid"
    }
    fn name(&self) -> &'static str {
        "Liquid Feedback"
    }
    fn blurb(&self) -> &'static str {
        "The music melting into itself: a slow zooming, turning, rippling feedback warp"
    }

    fn render(&mut self, a: &Analysis, c: &mut Canvas) {
        let (i, j, m) = preset_mix(a.time, MOODS.len(), 18.0, 6.0);
        let (p, q) = (MOODS[i], MOODS[j]);
        let l = |x: f32, y: f32| x + (y - x) * m;
        let dt = a.dt.max(1e-3);
        let zoom = l(p.zoom, q.zoom).powf(dt) * (1.0 + 0.03 * a.beat * dt * 30.0 / 30.0);
        let turn = l(p.turn, q.turn) * dt * (0.7 + 0.3 * a.mid.min(2.0));
        let keep = l(p.fade, q.fade).powf(dt);
        let ripple = l(p.ripple, q.ripple);
        let rings = l(p.rings, q.rings);
        let drift = (l(p.drift.0, q.drift.0) * dt, l(p.drift.1, q.drift.1) * dt);
        let (w, h) = (c.w, c.h);
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let size = w.min(h) as f32;
        let (sn, cs) = turn.sin_cos();
        let t = a.time as f32;
        self.tmp.resize(w * h, [0.0; 3]);
        let inv_zoom = 1.0 / zoom;
        // The warp: each pixel looks back to where its colour came from (zoomed, turned and rippled).
        let source = |x: f32, y: f32| {
            let (dx, dy) = ((x - cx) * inv_zoom, (y - cy) * inv_zoom);
            let (rx, ry) = (dx * cs - dy * sn, dx * sn + dy * cs);
            let d = (rx * rx + ry * ry).sqrt();
            let wob = ripple * size * ((d / size) * rings - t * 1.3).sin();
            let (nx, ny) = if d > 1e-3 { (rx / d, ry / d) } else { (0.0, 0.0) };
            (cx + rx + nx * wob - drift.0 * w as f32, cy + ry + ny * wob - drift.1 * h as f32)
        };
        let tmp = &mut self.tmp;
        let canvas = &*c;
        mesh_warp(w, h, &mut self.mesh, source, |x, y, sx, sy| {
            let s = canvas.sample(sx, sy);
            tmp[y * w + x] = [s[0] * keep, s[1] * keep, s[2] * keep];
        });
        std::mem::swap(&mut c.px, &mut self.tmp);
        // The music, drawn fresh over the melt.
        self.spin += dt * (0.2 + a.bass.min(3.0) * 0.15);
        let hue = l(p.hue, q.hue) + t * 0.01;
        let shape = if m < 0.5 { p.shape } else { q.shape };
        let n = a.wave.len();
        let amp = 0.6 + a.level * 2.0;
        let mut prev: Option<(f32, f32)> = None;
        for k in 0..n {
            let f = k as f32 / (n - 1) as f32;
            let v = a.wave[k] * amp;
            let (x, y) = match shape {
                0 => {
                    let ang = f * std::f32::consts::TAU + self.spin;
                    let r = size * (0.18 + 0.1 * v);
                    (cx + ang.cos() * r, cy + ang.sin() * r)
                }
                1 => (f * w as f32, cy + v * size * 0.25),
                _ => {
                    let ang = f * std::f32::consts::TAU * 2.0 + self.spin;
                    let r = size * (0.05 + 0.3 * f) * (1.0 + 0.3 * v);
                    (cx + ang.cos() * r, cy + ang.sin() * r)
                }
            };
            // Colour runs round the shape and drifts with time, so the melt carries rainbows of its own history.
            let col = hsv(hue + f * 0.6 + t * 0.04, 0.85, 0.75);
            if let Some((px, py)) = prev {
                if ((x - px).powi(2) + (y - py).powi(2)) < (size * 0.2).powi(2) {
                    c.line(px, py, x, y, col, 0.8, 1.5);
                }
            }
            prev = Some((x, y));
        }
    }
}
