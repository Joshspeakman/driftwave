//! The visualizer slot's engine: the app hands it the samples playing now, and gets back pixels that can't flash.

use crate::analysis::Analyzer;
use crate::canvas::Canvas;
use crate::guard::FlashGuard;
use crate::visualizers::{self, Visualizer};

pub struct VisHost {
    analyzer: Analyzer,
    vis: Box<dyn Visualizer>,
    canvas: Canvas,
    guard: FlashGuard,
    out: Vec<u8>,
    scale: f32,
}

impl VisHost {
    /// `w` × `h` in physical pixels (the slot's real size, so nothing is scaled up); `scale` is the display scale.
    pub fn new(id: &str, w: usize, h: usize, scale: f32) -> VisHost {
        let vis = visualizers::by_id(id).unwrap_or_else(|| visualizers::all().remove(0));
        VisHost { analyzer: Analyzer::new(), vis, canvas: Canvas::new(w, h), guard: FlashGuard::new(w, h, scale), out: Vec::new(), scale }
    }

    pub fn id(&self) -> &'static str {
        self.vis.id()
    }

    pub fn size(&self) -> (usize, usize) {
        (self.canvas.w, self.canvas.h)
    }

    /// Switches visualizer. The guard keeps what's on screen, so the change crossfades.
    pub fn set_visualizer(&mut self, id: &str) {
        if let Some(v) = visualizers::by_id(id) {
            self.vis = v;
            let (w, h) = (self.canvas.w, self.canvas.h);
            self.canvas = Canvas::new(w, h);
        }
    }

    pub fn resize(&mut self, w: usize, h: usize, scale: f32) {
        if (w, h) != (self.canvas.w, self.canvas.h) || scale != self.scale {
            self.canvas = Canvas::new(w, h);
            self.guard = FlashGuard::new(w, h, scale);
            self.scale = scale;
        }
    }

    /// One frame: `samples` interleaved (most recent last), `time` and `dt` in seconds. Returns RGBA8, `w` × `h`.
    pub fn frame(&mut self, samples: &[f32], channels: usize, rate: u32, time: f64, dt: f32) -> &[u8] {
        let a = self.analyzer.analyse(samples, channels, rate, time, dt);
        self.vis.render(a, &mut self.canvas);
        self.canvas.to_rgba8(&mut self.out);
        self.guard.apply(&mut self.out, dt);
        &self.out
    }
}
