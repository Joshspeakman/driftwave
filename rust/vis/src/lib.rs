//! Driftwave's visualizers: a toggleable slot in the classic layout, with six visualizers in the spirit of the late
//! 90s and early 2000s. Every one is original work: no code, presets or art from any other visualizer.
//!
//! The crate is pure. Samples, time and size go in; RGBA pixels come out. It knows nothing of the UI toolkit or the
//! audio engine, so the app, the tests and the preview tool all drive it the same way.
//!
//! - [`analysis`]: one FFT per frame, giving a waveform, 64 log-spaced bands, levels and a smooth beat envelope.
//! - [`canvas`]: a float RGB canvas with antialiased drawing (splats, glows, lines), drawn at the slot's real pixel
//!   size so nothing is ever stair-stepped.
//! - [`guard`]: the flash guard. It runs at the host's output stage, so every visualizer, including future ones,
//!   inherits it. Nothing it lets through can flash (WCAG 2.3.1, judged by `driftwave_core::flash`).
//! - [`host`]: ties it together for the app: [`host::VisHost::frame`].
//! - [`visualizers`]: the six, plus [`visualizers::all`] and [`visualizers::by_id`].

pub mod analysis;
pub mod canvas;
pub mod guard;
pub mod host;
pub mod visualizers;

pub use analysis::{Analysis, Analyzer};
pub use canvas::Canvas;
pub use host::VisHost;
pub use visualizers::Visualizer;
