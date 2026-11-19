//! Driftwave's audio engine (phase 2 of docs/RUST-SCOPE.md): streams over HTTP with ICY titles and
//! reconnects, HLS, decoding (symphonia; the OS decoder for AAC), resampling, output, recording,
//! and real levels and spectrum.

pub mod analysis;
pub mod decode;
pub mod engine;
pub mod hls;
pub mod net;
pub mod output;
pub mod resample;
pub mod save;
pub mod ts;

pub use engine::{Engine, EngineConfig, Event, PlayState};
