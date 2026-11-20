//! Ember Field: the palette-plasma style. A single field of heat is pulled inwards in a slow spiral and softly
//! blurred each frame; the waveform is sprinkled across it as sparks, and kicks breathe warmth into the middle. The
//! heat is coloured through a palette that drifts between fire, aurora, moonlight and ocean every half minute
//! (in that order, so blue never blends into green).

use super::{mesh_warp, preset_mix, Visualizer};
use crate::analysis::Analysis;
use crate::canvas::{mix, safe, Canvas, Rgb};

type Palette = [(f32, Rgb); 5];

const PALETTES: [Palette; 4] = [
    // fire
    [(0.0, [0.0, 0.0, 0.0]), (0.25, [0.35, 0.03, 0.02]), (0.5, [0.85, 0.28, 0.04]), (0.75, [1.0, 0.7, 0.2]), (1.0, [1.0, 0.95, 0.75])],
    // aurora (green, clear of teal)
    [(0.0, [0.0, 0.01, 0.005]), (0.25, [0.0, 0.2, 0.08]), (0.5, [0.1, 0.65, 0.3]), (0.75, [0.55, 0.95, 0.6]), (1.0, [0.95, 1.0, 0.95])],
    // moonlight (silver, where orchid was: no purple or pink)
    [(0.0, [0.0, 0.0, 0.0]), (0.25, [0.08, 0.1, 0.14]), (0.5, [0.35, 0.4, 0.5]), (0.75, [0.75, 0.8, 0.88]), (1.0, [1.0, 1.0, 1.0])],
    // ocean (deep blue to ice; the colour rule keeps it clear of cyan)
    [(0.0, [0.0, 0.0, 0.01]), (0.25, [0.0, 0.07, 0.25]), (0.5, [0.02, 0.3, 0.7]), (0.75, [0.35, 0.65, 1.0]), (1.0, [0.85, 0.92, 1.0])],
];

fn lookup(p: &Palette, v: f32) -> Rgb {
    let v = v.clamp(0.0, 1.0);
    for k in 1..p.len() {
        if v <= p[k].0 {
            let t = (v - p[k - 1].0) / (p[k].0 - p[k - 1].0);
            return mix(p[k - 1].1, p[k].1, t);
        }
    }
    p[p.len() - 1].1
}

#[derive(Default)]
pub struct EmberField {
    heat: Vec<f32>,
    tmp: Vec<f32>,
    mesh: Vec<(f32, f32)>,
    w: usize,
    h: usize,
    lut: Vec<Rgb>,
}

impl EmberField {
    fn sample(&self, x: f32, y: f32) -> f32 {
        let fx = (x - 0.5).clamp(0.0, (self.w - 1) as f32);
        let fy = (y - 0.5).clamp(0.0, (self.h - 1) as f32);
        let (x0, y0) = (fx as usize, fy as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (tx, ty) = (fx.fract(), fy.fract());
        let p = |x: usize, y: usize| self.heat[y * self.w + x];
        let a = p(x0, y0) + (p(x1, y0) - p(x0, y0)) * tx;
        let b = p(x0, y1) + (p(x1, y1) - p(x0, y1)) * tx;
        a + (b - a) * ty
    }

    fn add(&mut self, x: f32, y: f32, v: f32, r: f32) {
        let reach = (r * 2.0).ceil() as i32;
        let (cx, cy) = (x as i32, y as i32);
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (px, py) = (cx + dx, cy + dy);
                if px < 0 || py < 0 || px >= self.w as i32 || py >= self.h as i32 {
                    continue;
                }
                let d2 = ((px as f32 + 0.5 - x).powi(2) + (py as f32 + 0.5 - y).powi(2)) / (r * r);
                let k = py as usize * self.w + px as usize;
                self.heat[k] = (self.heat[k] + v * (-d2).exp()).min(1.4);
            }
        }
    }
}

impl Visualizer for EmberField {
    fn id(&self) -> &'static str {
        "ember"
    }
    fn name(&self) -> &'static str {
        "Ember Field"
    }
    fn blurb(&self) -> &'static str {
        "Sparks of the waveform drawn into a slow spiral of heat, through drifting palettes"
    }
    fn resolution(&self) -> f32 {
        0.6
    }

    fn render(&mut self, a: &Analysis, c: &mut Canvas) {
        // The field runs at half resolution: it's all soft heat, and that's a quarter of the work.
        let (fw, fh) = (c.w, c.h);
        let (w, h) = (fw.div_ceil(2), fh.div_ceil(2));
        if (w, h) != (self.w, self.h) {
            self.w = w;
            self.h = h;
            self.heat = vec![0.0; w * h];
            self.tmp = vec![0.0; w * h];
        }
        let dt = a.dt.max(1e-3);
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let t = a.time as f32;
        // Spiral inwards: each pixel looks a little further out and round.
        let pull = 1.0 + 0.6 * dt;
        let (sn, cs) = ((0.35 * dt * (1.0 + 0.3 * (t * 0.1).sin())).sin(), (0.35 * dt).cos());
        let keep = 0.55f32.powf(dt);
        let source = |x: f32, y: f32| {
            let (dx, dy) = ((x - cx) * pull, (y - cy) * pull);
            (cx + dx * cs - dy * sn, cy + dx * sn + dy * cs)
        };
        // Resampling the field through the spiral every frame softens it on its own: the heat diffuses.
        let mut tmp = std::mem::take(&mut self.tmp);
        let mut mesh = std::mem::take(&mut self.mesh);
        mesh_warp(w, h, &mut mesh, source, |x, y, sx, sy| tmp[y * w + x] = self.sample(sx, sy) * keep);
        self.mesh = mesh;
        self.tmp = std::mem::replace(&mut self.heat, tmp);
        // Sparks along the waveform, and warmth in the middle on a kick.
        let n = a.wave.len();
        let size = w.min(h) as f32;
        let r = (size / 140.0).max(0.8);
        // The same sparks over fewer pixels would pile up into white: their heat follows the field's size.
        let k_heat = (size / 150.0).clamp(0.2, 1.0);
        for k in (0..n).step_by(3) {
            let f = k as f32 / (n - 1) as f32;
            let x = f * w as f32;
            let y = cy + a.wave[k] * size * 0.35;
            self.add(x, y, (0.22 + a.level * 0.4) * k_heat, r);
        }
        if a.beat > 0.05 {
            self.add(cx, cy, a.beat * 0.25 * k_heat, size * 0.08);
        }
        // Colour through the drifting palette.
        let (i, j, m) = preset_mix(a.time, PALETTES.len(), 24.0, 8.0);
        self.lut.clear();
        for s in 0..256 {
            let v = s as f32 / 255.0;
            self.lut.push(safe(mix(lookup(&PALETTES[i], v), lookup(&PALETTES[j], v), m)));
        }
        for y in 0..fh {
            let sy = (y as f32 + 0.5) * 0.5;
            for x in 0..fw {
                let v = self.sample((x as f32 + 0.5) * 0.5, sy);
                c.px[y * fw + x] = self.lut[(v.clamp(0.0, 1.0) * 255.0) as usize];
            }
        }
    }
}
