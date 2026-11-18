//! Driftwave's platform-independent core, ported 1:1 from `windows/src/ShortwaveCore` (C#), which was itself a
//! translation of the Mac app's `Sources/ShortwaveCore`. Behaviour, file formats and tests carry over.

pub mod exchange;
pub mod migration;
pub mod station;
pub mod storage;

pub use station::{Station, StationSorting};
pub use storage::AppPaths;
