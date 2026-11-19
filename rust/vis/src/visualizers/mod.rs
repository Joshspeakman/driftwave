//! The six visualizers. Each is an homage to the feel of a classic from the Winamp years, written from scratch
//! with its own name: no code, presets or artwork from the originals.
//!
//! A visualizer draws into a canvas that it keeps between frames (feedback effects read last frame's picture).
//! It never needs to worry about flashing: the host's guard takes care of that for every one.

use crate::analysis::Analysis;
use crate::canvas::Canvas;

mod ember;
mod flowform;
mod liquid;
mod peak_meter;
mod scope_studio;
mod waveland;

pub trait Visualizer: Send {
    /// Stable id, saved in settings.
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    /// One line for the picker.
    fn blurb(&self) -> &'static str;
    fn render(&mut self, a: &Analysis, c: &mut Canvas);
}

/// All six, in picker order.
pub fn all() -> Vec<Box<dyn Visualizer>> {
    vec![
        Box::new(peak_meter::PeakMeter::default()),
        Box::new(liquid::LiquidFeedback::default()),
        Box::new(ember::EmberField::default()),
        Box::new(scope_studio::ScopeStudio::default()),
        Box::new(flowform::Flowform::default()),
        Box::new(waveland::Waveland::default()),
    ]
}

pub fn by_id(id: &str) -> Option<Box<dyn Visualizer>> {
    all().into_iter().find(|v| v.id() == id)
}

/// A warp worked out on a coarse mesh (the way the classic feedback visualizers did it): `source(x, y)` gives where
/// the pixel at (x, y) takes its colour from, for each mesh point; `each(x, y, sx, sy)` is then called for every
/// pixel with its source interpolated smoothly between the points.
pub(crate) fn mesh_warp(w: usize, h: usize, mesh: &mut Vec<(f32, f32)>, source: impl Fn(f32, f32) -> (f32, f32), mut each: impl FnMut(usize, usize, f32, f32)) {
    const GW: usize = 40;
    const GH: usize = 24;
    mesh.clear();
    for gy in 0..=GH {
        for gx in 0..=GW {
            mesh.push(source(gx as f32 / GW as f32 * w as f32, gy as f32 / GH as f32 * h as f32));
        }
    }
    let mut row = vec![(0f32, 0f32); GW + 1];
    for y in 0..h {
        let fy = (y as f32 + 0.5) / h as f32 * GH as f32;
        let j = (fy as usize).min(GH - 1);
        let ty = fy - j as f32;
        for (i, r) in row.iter_mut().enumerate() {
            let (a, b) = (mesh[j * (GW + 1) + i], mesh[(j + 1) * (GW + 1) + i]);
            *r = (a.0 + (b.0 - a.0) * ty, a.1 + (b.1 - a.1) * ty);
        }
        for x in 0..w {
            let fx = (x as f32 + 0.5) / w as f32 * GW as f32;
            let i = (fx as usize).min(GW - 1);
            let tx = fx - i as f32;
            let (a, b) = (row[i], row[i + 1]);
            each(x, y, a.0 + (b.0 - a.0) * tx, a.1 + (b.1 - a.1) * tx);
        }
    }
}

/// Picks a value from the bands at fraction `x` (0..1), interpolated.
pub(crate) fn band_at(a: &Analysis, x: f32) -> f32 {
    let n = a.bands.len();
    let f = (x.clamp(0.0, 1.0) * (n - 1) as f32).max(0.0);
    let i = f as usize;
    let j = (i + 1).min(n - 1);
    a.bands[i] + (a.bands[j] - a.bands[i]) * f.fract()
}

/// A slow drift through a list of presets: (current mix of preset `i` into `i + 1`), moving every `hold` seconds
/// with a `blend`-second crossfade.
pub(crate) fn preset_mix(time: f64, count: usize, hold: f64, blend: f64) -> (usize, usize, f32) {
    let period = hold + blend;
    let k = (time / period).floor() as usize;
    let t = time - k as f64 * period;
    let m = if t < hold { 0.0 } else { ((t - hold) / blend) as f32 };
    let m = m * m * (3.0 - 2.0 * m);
    (k % count, (k + 1) % count, m)
}
