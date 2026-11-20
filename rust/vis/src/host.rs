//! The visualizer slot's engine: the app hands it the samples playing now, and gets back pixels that can't flash.
//!
//! Cost is set by a pixel budget, not by the window: the picture is drawn at most [`PIXEL_BUDGET`] pixels (less for
//! soft visualizers that declare a lower [`Visualizer::resolution`]) and the UI scales it up smoothly. So double
//! size, a big window or a Retina screen cost no more than the default.

use crate::analysis::Analyzer;
use crate::canvas::Canvas;
use crate::guard::FlashGuard;
use crate::visualizers::{self, Visualizer};

/// The most pixels a frame is drawn at (about 400 × 150).
pub const PIXEL_BUDGET: usize = 60_000;

pub struct VisHost {
    analyzer: Analyzer,
    vis: Box<dyn Visualizer>,
    canvas: Canvas,
    guard: FlashGuard,
    out: Vec<u8>,
    /// The slot on screen, in physical pixels, and the display scale.
    slot: (usize, usize),
    scale: f32,
    budget: usize,
}

impl VisHost {
    /// A slot of `w` × `h` physical pixels on a display of `scale`.
    pub fn new(id: &str, w: usize, h: usize, scale: f32) -> VisHost {
        Self::with_budget(id, w, h, scale, PIXEL_BUDGET)
    }

    pub fn with_budget(id: &str, w: usize, h: usize, scale: f32, budget: usize) -> VisHost {
        let vis = visualizers::by_id(id).unwrap_or_else(|| visualizers::all().remove(0));
        let mut host = VisHost {
            analyzer: Analyzer::new(),
            vis,
            canvas: Canvas::new(1, 1),
            guard: FlashGuard::new(1, 1, 1.0),
            out: Vec::new(),
            slot: (0, 0),
            scale,
            budget: budget.max(1000),
        };
        host.set_slot(w, h, scale);
        host
    }

    pub fn id(&self) -> &'static str {
        self.vis.id()
    }

    /// The slot on screen, in physical pixels.
    pub fn slot_size(&self) -> (usize, usize) {
        self.slot
    }

    /// The size the picture is drawn at (the UI scales it to the slot).
    pub fn size(&self) -> (usize, usize) {
        (self.canvas.w, self.canvas.h)
    }

    /// The drawing size for a slot: the visualizer's resolution, then the budget, keeping the slot's shape.
    fn drawing_size(&self, w: usize, h: usize) -> (usize, usize) {
        let r = self.vis.resolution().clamp(0.25, 1.0);
        let (mut dw, mut dh) = ((w as f32 * r).max(1.0), (h as f32 * r).max(1.0));
        let area = dw * dh;
        if area > self.budget as f32 {
            let k = (self.budget as f32 / area).sqrt();
            dw *= k;
            dh *= k;
        }
        (dw.round().max(1.0) as usize, dh.round().max(1.0) as usize)
    }

    /// Sets the slot (physical pixels). Rebuilds only when the drawing size changes.
    pub fn set_slot(&mut self, w: usize, h: usize, scale: f32) {
        let (dw, dh) = self.drawing_size(w.max(1), h.max(1));
        self.slot = (w.max(1), h.max(1));
        if (dw, dh) != (self.canvas.w, self.canvas.h) || scale != self.scale {
            self.canvas = Canvas::new(dw, dh);
            // The guard's regions are a fixed size on screen: in drawing pixels, that's the display scale times how
            // much smaller the drawing is than the slot.
            self.guard = FlashGuard::new(dw, dh, scale * dw as f32 / self.slot.0 as f32);
            self.scale = scale;
        }
    }

    /// Switches visualizer. When the drawing size stays the same the guard keeps what's on screen, so it crossfades.
    pub fn set_visualizer(&mut self, id: &str) {
        if let Some(v) = visualizers::by_id(id) {
            self.vis = v;
            let (w, h) = self.slot;
            let size = self.drawing_size(w, h);
            if size != (self.canvas.w, self.canvas.h) {
                self.canvas = Canvas::new(size.0, size.1);
                self.guard = FlashGuard::new(size.0, size.1, self.scale * size.0 as f32 / w as f32);
            } else {
                self.canvas = Canvas::new(size.0, size.1);
            }
        }
    }

    /// Changes the pixel budget (adaptive quality); the picture is redrawn at the new size.
    pub fn set_budget(&mut self, budget: usize) {
        self.budget = budget.max(1000);
        let (w, h, scale) = (self.slot.0, self.slot.1, self.scale);
        self.set_slot(w, h, scale);
    }

    /// Back-compatible: a slot of `w` × `h` physical pixels.
    pub fn resize(&mut self, w: usize, h: usize, scale: f32) {
        self.set_slot(w, h, scale);
    }

    /// One frame: `samples` interleaved (most recent last), `time` and `dt` in seconds. Returns RGBA8 at [`size`].
    pub fn frame(&mut self, samples: &[f32], channels: usize, rate: u32, time: f64, dt: f32) -> &[u8] {
        let a = self.analyzer.analyse(samples, channels, rate, time, dt);
        self.vis.render(a, &mut self.canvas);
        self.canvas.to_rgba8(&mut self.out);
        crate::canvas::keep_the_rule(&mut self.out);
        self.guard.apply(&mut self.out, dt);
        &self.out
    }
}
