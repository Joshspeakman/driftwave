//! Driftwave's system integration (stage 4 of the rebuild), kept out of `rust/app` so the app only
//! wires it up:
//! - [`layers`]: a theme's scene as Core Animation layers beneath the UI (macOS);
//! - [`media`]: Now Playing and media keys (all three platforms, through souvlaki);
//! - [`notify`]: new-track notifications (macOS, and an installed copy on Windows);
//! - [`reopen`]: a Dock click while the window is hidden shows it again (macOS);
//! - [`system`]: the person's own short-time format, and another app's saved settings (macOS);
//! - [`single`]: one running copy per flavour, and `--quit` / `--show` handed to it (macOS, Linux);
//! - [`tray`]: the menu bar / notification-area icon and its menu;
//! - [`winapp`]: Windows' app id and the Start-menu shortcut that carries it.

pub mod layers;
pub mod media;
pub mod notify;
pub mod reopen;
pub mod single;
pub mod system;
pub mod tray;
#[cfg(windows)]
pub mod winapp;
