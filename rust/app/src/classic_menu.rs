//! Classic's right-click menu: right-click anywhere in Classic (or its menu buttons) for the options.
//!
//! A native menu (Windows', macOS's), like Winamp's own: it reaches past the window, which in windowshade mode is a
//! thin strip, and brings submenus, checks and the keyboard. What's under the pointer comes first: the song title,
//! the display, the station list or a title bar offer their text size, the visualizer its visualizers. Then
//! everything else: back to the Full layout, playback, the window, the visualizer, the theme, every text size,
//! Settings and Quit. Text sizes are kept in the settings (`classicText`). Where there is no native menu, the
//! in-window options open instead.

use super::{log, played, visualizers, App, MainWindow, Pal, DEFAULT_WIDTH};
use driftwave_platform::context_menu::{popup, Host, Item};
use serde_json::{json, Value};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

/// The part of Classic under the pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Part {
    Player,
    Marquee,
    Display,
    List,
    Titles,
    Visualizer,
}

/// The text sizes on offer.
const SIZES: [(&str, f32); 4] = [("Small", 0.85), ("Normal", 1.0), ("Large", 1.2), ("Larger", 1.4)];

/// The parts whose text can be sized: settings key and menu name.
const TEXTS: [(&str, &str); 4] = [("marquee", "Song Title"), ("display", "Display"), ("list", "Station List"), ("titles", "Title Bar")];

/// What's at (`x`, `y`) (logical pixels) in a Classic window `w` wide. Mirrors classic.slint's layout: 275 units
/// wide; the player 102 units tall (13 shaded), the visualizer 104, then the list; each panel's title bar its top 12.
pub(crate) fn part_at(w: f32, x: f32, y: f32, shaded: bool, show_vis: bool) -> Part {
    let u = w / 275.0;
    let main = if shaded { 13.0 } else { 102.0 } * u;
    let vis = if show_vis { 104.0 * u } else { 0.0 };
    let inside = |x0: f32, y0: f32, x1: f32, y1: f32| x >= x0 * u && x < x1 * u && y >= y0 * u && y < y1 * u;
    if y < main {
        if shaded {
            return Part::Marquee;
        }
        if y < 12.0 * u {
            return Part::Titles;
        }
        if inside(101.0, 16.0, 272.0, 29.0) {
            return Part::Marquee;
        }
        if inside(12.0, 16.0, 98.0, 27.0) || inside(101.0, 30.0, 272.0, 56.0) || inside(12.0, 63.0, 272.0, 73.0) {
            return Part::Display;
        }
        return Part::Player;
    }
    if y < main + vis {
        return if y - main < 12.0 * u { Part::Titles } else { Part::Visualizer };
    }
    if y - main - vis < 12.0 * u {
        Part::Titles
    } else {
        Part::List
    }
}

fn key_of(part: Part) -> Option<&'static str> {
    match part {
        Part::Marquee => Some("marquee"),
        Part::Display => Some("display"),
        Part::List => Some("list"),
        Part::Titles => Some("titles"),
        _ => None,
    }
}

/// The window's native handle, for its menu.
fn host(ui: &MainWindow) -> Option<Host> {
    use slint::winit_030::{winit, WinitWindowAccessor};
    ui.window()
        .with_winit_window(|w: &winit::window::Window| {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            match w.window_handle().ok()?.as_raw() {
                RawWindowHandle::Win32(h) => Some(Host::Win32(h.hwnd.get())),
                RawWindowHandle::AppKit(h) => Some(Host::AppKit(h.ns_view.as_ptr())),
                _ => None,
            }
        })
        .flatten()
}

impl App {
    /// A part's text size (1 is the standard).
    pub(crate) fn classic_text(&self, key: &str) -> f32 {
        self.settings.get("classicText").and_then(|t| t.get(key)).and_then(Value::as_f64).map(|v| (v as f32).clamp(0.7, 1.6)).unwrap_or(1.0)
    }

    /// Classic's text sizes, from the settings.
    pub(crate) fn show_classic_text(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let p = ui.global::<Pal>();
        p.set_text_marquee(self.classic_text("marquee"));
        p.set_text_display(self.classic_text("display"));
        p.set_text_list(self.classic_text("list"));
        p.set_text_titles(self.classic_text("titles"));
    }

    fn set_classic_text(&mut self, key: &str, v: f32) {
        if !self.settings.get("classicText").is_some_and(Value::is_object) {
            self.settings["classicText"] = json!({});
        }
        self.settings["classicText"][key] = json!(v);
        self.save_settings();
        self.show_classic_text();
    }

    fn set_vis(&mut self, id: &str) {
        let Some(&id) = self.vis_ids.iter().find(|&&v| v == id) else { return };
        self.big.host.set_visualizer(id);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_vis_name(visualizers::by_id(id).map(|v| v.name()).unwrap_or("").into());
        }
    }

    fn sizes(&self, key: &str) -> Vec<Item> {
        let now = self.classic_text(key);
        SIZES.iter().map(|(name, v)| Item::check(format!("text:{key}:{v}"), *name, (now - v).abs() < 0.01)).collect()
    }

    /// The menu for a right-click on `part`.
    fn classic_menu_items(&self, ui: &MainWindow, part: Part) -> Vec<Item> {
        let mut items = Vec::new();
        // What's under the pointer first.
        if let Some(key) = key_of(part) {
            let name = TEXTS.iter().find(|(k, _)| *k == key).map(|(_, n)| *n).unwrap_or("");
            items.push(Item::sub(format!("{name} Text Size"), self.sizes(key)));
            items.push(Item::Separator);
        }
        let current_vis = self.big.host.id();
        let vis_items: Vec<Item> = self
            .vis_ids
            .iter()
            .map(|id| Item::check(format!("vis:{id}"), visualizers::by_id(id).map(|v| v.name()).unwrap_or(*id), *id == current_vis))
            .collect();
        if part == Part::Visualizer {
            items.extend(vis_items.iter().map(|i| match i {
                Item::Check { id, label, checked } => Item::check(id.clone(), label.clone(), *checked),
                _ => Item::Separator,
            }));
            items.push(Item::Separator);
        }
        items.push(Item::action("full", "Switch to Full Layout"));
        items.push(Item::Separator);
        let playing = self.player.playing || self.player.loading;
        items.push(if playing { Item::action("stop", "Stop") } else { Item::action("play", "Play") });
        items.push(Item::action("prev", "Previous Station"));
        items.push(Item::action("next", "Next Station"));
        items.push(Item::check("record", "Record Songs", self.player.armed));
        items.push(Item::Separator);
        let size = ui.window().size().to_logical(ui.window().scale_factor());
        items.push(Item::check("shade", "Windowshade Mode", ui.get_shaded()));
        items.push(Item::check("ontop", "Always on Top", ui.get_on_top()));
        items.push(Item::check("showvis", "Show Visualizer", ui.get_show_vis()));
        items.push(Item::check("showlist", "Show Station List", ui.get_show_list()));
        items.push(Item::check("double", "Double Size", size.width > DEFAULT_WIDTH * 1.2));
        items.push(Item::Separator);
        items.push(Item::sub("Visualizer", vis_items));
        let mut themes = vec![Item::check("theme:default", "Default", self.theme_id.is_none())];
        themes.extend(self.themes.iter().map(|t| Item::check(format!("theme:{}", t.id), t.name.clone(), self.theme_id.as_deref() == Some(t.id.as_str()))));
        items.push(Item::sub("Theme", themes));
        items.push(Item::sub("Text Size", TEXTS.iter().map(|(key, name)| Item::sub(*name, self.sizes(key))).collect()));
        items.push(Item::Separator);
        items.push(Item::action("settings", "Settings…"));
        items.push(Item::action("quit", "Quit Driftwave"));
        items
    }
}

/// Opens Classic's menu for a right-click at `at` (logical pixels; None: from a menu button). Call from a timer, not
/// from inside an input event: the native menu runs its own loop until it closes.
pub(crate) fn open(app: &Rc<RefCell<App>>, at: Option<(f32, f32)>) {
    let (items, host, ui) = {
        let a = app.borrow();
        let Some(ui) = a.ui.upgrade() else { return };
        let w = ui.window().size().to_logical(ui.window().scale_factor()).width;
        let part = at.map(|(x, y)| part_at(w, x, y, ui.get_shaded(), ui.get_show_vis())).unwrap_or(Part::Player);
        log(&format!("classic menu: {part:?}"));
        (a.classic_menu_items(&ui, part), host(&ui), ui)
    };
    let chosen = match host {
        Some(h) => popup(h, &items),
        None => Err(driftwave_platform::context_menu::NoNativeMenus),
    };
    match chosen {
        Ok(Some(id)) => run(app, &ui, &id),
        Ok(None) => {}
        // No native menu here: the in-window options.
        Err(_) => ui.set_options_open(true),
    }
}

/// Carries out a menu choice. Choices that go through the UI's own callbacks run with the app not borrowed.
pub(crate) fn run(app: &Rc<RefCell<App>>, ui: &MainWindow, id: &str) {
    log(&format!("classic menu: {id}"));
    match id {
        "full" => app.borrow_mut().set_layout("full"),
        "settings" => {
            app.borrow_mut().set_layout("full");
            ui.set_page("settings".into());
        }
        "play" => ui.invoke_play(),
        "stop" => ui.invoke_stop(),
        "prev" => ui.invoke_prev(),
        "next" => ui.invoke_next(),
        "record" => ui.invoke_toggle_record(),
        "shade" => {
            ui.set_shaded(!ui.get_shaded());
            ui.invoke_panels_changed();
        }
        "ontop" => ui.invoke_toggle_on_top(),
        "showvis" => {
            ui.set_show_vis(!ui.get_show_vis());
            ui.invoke_panels_changed();
        }
        "showlist" => {
            ui.set_show_list(!ui.get_show_list());
            ui.invoke_panels_changed();
        }
        "double" => ui.invoke_toggle_double(),
        "quit" => played::quit_app(app),
        _ => {
            if let Some(v) = id.strip_prefix("vis:") {
                app.borrow_mut().set_vis(v);
            } else if let Some(t) = id.strip_prefix("theme:") {
                ui.invoke_choose_theme(t.into());
            } else if let Some(rest) = id.strip_prefix("text:") {
                if let Some((key, v)) = rest.split_once(':') {
                    if let Ok(v) = v.parse::<f32>() {
                        app.borrow_mut().set_classic_text(key, v);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_clicks_find_their_part() {
        // A 275-unit window at 1.5 px a unit (412.5 px wide).
        let (w, u) = (412.5, 1.5);
        let at = |x: f32, y: f32| part_at(w, x * u, y * u, false, true);
        assert_eq!(at(150.0, 5.0), Part::Titles);
        assert_eq!(at(150.0, 20.0), Part::Marquee);
        assert_eq!(at(140.0, 35.0), Part::Display);
        assert_eq!(at(30.0, 85.0), Part::Player);
        assert_eq!(at(100.0, 107.0), Part::Titles);
        assert_eq!(at(100.0, 150.0), Part::Visualizer);
        assert_eq!(at(100.0, 210.0), Part::Titles);
        assert_eq!(at(100.0, 250.0), Part::List);
        // Without the visualizer, the list follows the player; shaded, it's all the song title.
        assert_eq!(part_at(w, 150.0 * u, 150.0 * u, false, false), Part::List);
        assert_eq!(part_at(w, 150.0, 10.0, true, true), Part::Marquee);
    }
}
