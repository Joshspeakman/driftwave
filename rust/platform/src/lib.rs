//! Driftwave's system integration (stage 4 of the rebuild), kept out of `rust/app` so the app only
//! wires it up:
//! - [`layers`]: a theme's scene as Core Animation layers beneath the UI (macOS);
//! - [`media`]: Now Playing and media keys (all three platforms, through souvlaki);
//! - [`notify`]: new-track notifications (macOS, and an installed copy on Windows);
//! - [`single`]: one running copy per flavour, and `--quit` / `--show` handed to it (macOS, Linux);
//! - [`tray`]: the menu bar / notification-area icon and its menu;
//! - [`winapp`]: Windows' app id and the Start-menu shortcut that carries it.

pub mod layers;
pub mod media;
pub mod notify;
pub mod single;
pub mod tray;
#[cfg(windows)]
pub mod winapp;
