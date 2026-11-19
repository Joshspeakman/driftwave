//! Flowform: the flowing style of the era's most graceful visualizers. Hundreds of motes ride an invisible current
//! that slowly changes its mind, leaving long silk trails; the treble quickens them. A waveshape, the music's own
//! outline, floats over the current and turns.

use super::Visualizer;
use crate::analysis::Analysis;
use crate::canvas::{hsv, Canvas};

struct Mote {
    x: f32,
    y: f32,
    age: f32,
    life: f32,
    tint: f32,
}

pub struct Flowform {
    motes: Vec<Mote>,
    seed: u32,
    turn: f32,
}

impl Default for Flowform {
    fn default() -> Self {
        Flowform { motes: Vec::new(), seed: 0x9E37_79B9, turn: 0.0 }
    }
}

impl Flowform {
    fn rand(&mut self) -> f32 {
        // xorshift: cheap, steady, and the same every run.
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn spawn(&mut self, w: f32, h: f32) -> Mote {
        Mote { x: self.rand() * w, y: self.rand() * h, age: 0.0, life: 2.0 + self.rand() * 4.0, tint: self.rand() }
    }
}

impl Visualizer for Flowform {
    fn id(&self) -> &'static str {
        "flowform"
    }
    fn name(&self) -> &'static str {
        "Flowform"
    }
    fn blurb(&self) -> &'static str {
        "Silk trails riding a slowly changing current, under the turning outline of the music"
    }

    fn render(&mut self, a: &Analysis, c: &mut Canvas) {
        let (w, h) = (c.w as f32, c.h as f32);
        let dt = a.dt.max(1e-3);
        c.fade(0.25f32.powf(dt));
        let count = ((w * h / 260.0) as usize).clamp(200, 1600);
        while self.motes.len() < count {
            let m = self.spawn(w, h);
            self.motes.push(m);
        }
        self.motes.truncate(count);
        let t = a.time as f32;
        let size = w.min(h);
        let speed = size * (0.12 + 0.18 * a.treble.min(3.0));
        let hue = 0.58 + 0.1 * (t * 0.03).sin();
        let (f1, f2) = (3.0 / size, 2.3 / size);
        for i in 0..self.motes.len() {
            let (x, y) = (self.motes[i].x, self.motes[i].y);
            // The current: an angle field that drifts with time.
            let ang = (x * f1 + t * 0.21).sin() * 2.2 + (y * f2 - t * 0.17).cos() * 2.0 + (t * 0.05).sin();
            let (nx, ny) = (x + ang.cos() * speed * dt, y + ang.sin() * speed * dt);
            let m = &mut self.motes[i];
            m.age += dt;
            let life = (m.age / m.life).min(1.0);
            let alpha = (life * std::f32::consts::PI).sin() * (0.35 + 0.4 * a.mid.min(2.0) * 0.5);
            let col = hsv(hue + m.tint * 0.18, 0.55, 1.0);
            c.line(x, y, nx, ny, col, alpha, 1.0);
            let m = &mut self.motes[i];
            m.x = nx;
            m.y = ny;
            if m.age > m.life || nx < -2.0 || ny < -2.0 || nx > w + 2.0 || ny > h + 2.0 {
                self.motes[i] = self.spawn(w, h);
            }
        }
        // The waveshape: the music's outline, turning slowly over the current.
        self.turn += dt * 0.15;
        let (sn, cs) = self.turn.sin_cos();
        let (cx, cy) = (w * 0.5, h * 0.5);
        let n = a.wave.len();
        let mut prev = None;
        for k in (0..n).step_by(2) {
            let f = k as f32 / (n - 1) as f32 - 0.5;
            let (x, y) = (f * size * 1.1, a.wave[k] * size * 0.22);
            let (x, y) = (x * cs - y * sn, x * sn + y * cs);
            let p = (cx + x, cy + y);
            if let Some((px, py)) = prev {
                c.line(px, py, p.0, p.1, hsv(hue + 0.45, 0.55, 0.9), 0.22, 1.3);
            }
            prev = Some(p);
        }
    }
}
