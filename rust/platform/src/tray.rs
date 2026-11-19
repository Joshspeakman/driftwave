//! The menu bar icon (macOS), notification-area icon (Windows) or StatusNotifier item (Linux), with
//! the same menu as the Mac app's: what's playing, Play/Stop, the library, Show Driftwave, Mini Player,
//! Quit. Built on tray-icon; create it on the main thread once the event loop runs.

use std::collections::HashMap;
use tray_icon::menu::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    TogglePlay,
    Station(usize),
    Show,
    MiniPlayer,
    Quit,
}

/// What the menu shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrayState {
    pub station: Option<String>,
    pub track: Option<String>,
    pub playing: bool,
    /// The library, in the order the app shows it.
    pub stations: Vec<String>,
    pub current: Option<usize>,
}

pub struct Tray {
    icon: TrayIcon,
    actions: HashMap<MenuId, TrayAction>,
    state: Option<TrayState>,
}

impl Tray {
    /// The Driftwave mark as a menu bar template on macOS (it follows the menu bar's light and dark
    /// look); the colour icon `rgba` elsewhere.
    pub fn new(rgba: Vec<u8>, width: u32, height: u32, tooltip: &str) -> Result<Tray, String> {
        let (icon, template) = if cfg!(target_os = "macos") {
            let (px, s) = menu_bar_template(44);
            (Icon::from_rgba(px, s, s), true)
        } else {
            (Icon::from_rgba(rgba, width, height), false)
        };
        let icon = icon.map_err(|e| format!("tray icon: {e}"))?;
        let builder = TrayIconBuilder::new();
        // `with_icon_templated` exists only on macOS: elsewhere the colour icon goes in as it is.
        #[cfg(target_os = "macos")]
        let builder = if template { builder.with_icon_templated(icon) } else { builder.with_icon(icon) };
        #[cfg(not(target_os = "macos"))]
        let builder = {
            let _ = template;
            builder.with_icon(icon)
        };
        let tray = builder
            .with_tooltip(tooltip)
            .with_menu(Box::new(Menu::new()))
            .build()
            .map_err(|e| format!("tray icon: {e}"))?;
        let mut t = Tray { icon: tray, actions: HashMap::new(), state: None };
        t.update(&TrayState::default());
        Ok(t)
    }

    /// Rebuilds the menu when the state changed.
    pub fn update(&mut self, state: &TrayState) {
        if self.state.as_ref() == Some(state) {
            return;
        }
        let menu = Menu::new();
        let mut actions = HashMap::new();
        let mut action = |title: &str, a: TrayAction| {
            let item = MenuItem::new(title, true, None);
            actions.insert(item.id().clone(), a);
            item
        };
        let label = |title: &str| MenuItem::new(title, false, None);
        match &state.station {
            Some(station) => {
                let _ = menu.append(&label(station));
                if let Some(track) = &state.track {
                    let _ = menu.append(&label(track));
                }
                let _ = menu.append(&action(if state.playing { "Stop" } else { "Play" }, TrayAction::TogglePlay));
            }
            None => {
                let _ = menu.append(&label("No Playback"));
            }
        }
        let mut station_ids = Vec::new();
        if !state.stations.is_empty() {
            let _ = menu.append(&PredefinedMenuItem::separator());
            let library = Submenu::new("Library", true);
            for (i, name) in state.stations.iter().enumerate() {
                let item = CheckMenuItem::new(name, true, state.current == Some(i), None);
                station_ids.push((item.id().clone(), TrayAction::Station(i)));
                let _ = library.append(&item);
            }
            let _ = menu.append(&library);
        }
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&action("Show Driftwave", TrayAction::Show));
        let _ = menu.append(&action("Mini Player", TrayAction::MiniPlayer));
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&action("Quit Driftwave", TrayAction::Quit));
        actions.extend(station_ids);
        self.icon.set_menu(Some(Box::new(menu)));
        self.actions = actions;
        self.state = Some(state.clone());
    }

    /// Menu choices since the last call.
    pub fn actions(&self) -> Vec<TrayAction> {
        // Through the shared reader: a context menu's choices are left for it; every other choice is the tray's, and
        // one it doesn't know (from a menu it has since rebuilt) is dropped rather than left waiting.
        crate::context_menu::take(|id| !crate::context_menu::is_context_menu_id(id))
            .into_iter()
            .filter_map(|id| self.actions.get(&MenuId::new(&id)).copied())
            .collect()
    }
}

/// The mark as a black-on-transparent template, `size` pixels square (44 = 22 pt at 2×): the sun as a
/// disc behind two waves, as in the app icon.
pub fn menu_bar_template(size: u32) -> (Vec<u8>, u32) {
    let s = size as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    // Coverage of a point by the mark, 0–1, from distances (antialiased over about a pixel).
    let aa = |d: f32| (0.5 - d).clamp(0.0, 1.0);
    let curve = |x: f32, base: f32, amp: f32| s * base + s * amp * (x / s * std::f32::consts::TAU * 1.75 - 0.4).sin();
    // A wave stroke with rounded ends between x0 and x1 (fractions of the size).
    let wave = |x: f32, y: f32, base: f32, amp: f32, half: f32, x0: f32, x1: f32| {
        let cx = x.clamp(s * x0, s * x1);
        let d = ((x - cx).powi(2) + (y - curve(cx, base, amp)).powi(2)).sqrt();
        aa(d - s * half)
    };
    let cover = |x: f32, y: f32| -> f32 {
        let upper = wave(x, y, 0.60, 0.05, 0.065, 0.13, 0.87);
        // The lower wave is lighter in the icon; a template shows that as partial alpha.
        let lower = wave(x, y, 0.79, 0.045, 0.05, 0.24, 0.76) * 0.6;
        let (cx, cy, r) = (s * 0.5, s * 0.40, s * 0.23);
        let sun = aa(((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - r);
        // A clear band between the sun and the wave over it, as in the icon.
        let band = wave(x, y, 0.60, 0.05, 0.12, 0.0, 1.0);
        let sun = if y > curve(x, 0.60, 0.05) { 0.0 } else { sun * (1.0 - band) };
        upper.max(lower).max(sun)
    };
    for y in 0..size {
        for x in 0..size {
            // 4× supersampling for smooth curves at menu bar size.
            let mut a = 0.0;
            for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                a += cover(x as f32 + dx, y as f32 + dy);
            }
            let i = ((y * size + x) * 4) as usize;
            px[i + 3] = (a / 4.0 * 255.0).round() as u8; // black, alpha = coverage
        }
    }
    (px, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_has_a_mark_and_clear_edges() {
        let (px, s) = menu_bar_template(44);
        let alpha: Vec<u8> = px.chunks(4).map(|p| p[3]).collect();
        let covered = alpha.iter().filter(|&&a| a > 128).count();
        assert!(covered > 200 && covered < 44 * 44 / 2, "covered {covered}");
        assert!(px.chunks(4).all(|p| p[0] == 0 && p[1] == 0 && p[2] == 0), "templates are black");
        assert_eq!(alpha[0], 0);
        assert_eq!(alpha[(s * s - 1) as usize], 0);
    }
}
