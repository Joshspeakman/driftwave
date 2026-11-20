//! Driftwave's platform-independent core, ported 1:1 from the retired C# app's core (ShortwaveCore), which was itself a
//! translation of the Mac app's `Sources/ShortwaveCore`. Behaviour, file formats and tests carry over.

pub mod audio_frames;
pub mod exchange;
pub mod flash;
pub mod hues;
pub mod icy;
pub mod id3;
pub mod m4a;
pub mod migration;
pub mod ogg;
pub mod playlist;
pub mod radio_browser;
pub mod recording;
pub mod station;
pub mod storage;
pub mod theme;

pub use station::{Station, StationSorting};
pub use storage::AppPaths;
