//! The Full layout's right-click menu (as Classic's: a native menu, see `classic_menu`).
//!
//! On a station card (Library, Discover, Search) the station's own actions come first: play or stop it, its details,
//! add it to or remove it from the library, its homepage. Then everything else: Classic, playback, where to go,
//! the player, moving scenes, the appearance and theme, Settings and Quit. In a text field it stays out of the way.

use super::{finding, log, played, App, Dir, MainWindow, Scenery};
use driftwave_platform::context_menu::{popup, Item};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

impl App {
    fn station_items(&self, source: &str, i: i32) -> Vec<Item> {
        let Some(s) = self.station_from(source, i) else { return Vec::new() };
        let key = format!("{source}:{i}");
        let playing = self.player.engine.is_some() && self.player.current.as_ref().is_some_and(|c| c.id == s.id);
        let in_library = self.stations.iter().any(|x| x.id == s.id);
        let mut items = vec![
            if playing { Item::action(format!("st:stop:{key}"), format!("Stop {}", s.name)) } else { Item::action(format!("st:play:{key}"), format!("Play {}", s.name)) },
            Item::action(format!("st:details:{key}"), "Station Details…"),
            if in_library { Item::action(format!("st:remove:{key}"), "Remove from Library") } else { Item::action(format!("st:add:{key}"), "Add to Library") },
        ];
        if s.homepage.as_deref().is_some_and(|h| h.starts_with("http")) {
            items.push(Item::action(format!("st:homepage:{key}"), "Open Homepage"));
        }
        items.push(Item::Separator);
        items
    }

    fn full_menu_items(&self, ui: &MainWindow, card: Option<(String, i32)>) -> Vec<Item> {
        let mut items = card.map(|(source, i)| self.station_items(&source, i)).unwrap_or_default();
        items.push(Item::action("classic", "Switch to Classic Layout"));
        items.push(Item::Separator);
        let playing = self.player.playing || self.player.loading;
        items.push(if playing { Item::action("stop", "Stop") } else { Item::action("play", "Play") });
        items.push(Item::action("prev", "Previous Station"));
        items.push(Item::action("next", "Next Station"));
        items.push(Item::check("record", "Record Songs", self.player.armed));
        items.push(Item::Separator);
        let page = ui.get_page().to_string();
        items.push(Item::sub(
            "Go To",
            [("library", "Library"), ("discover", "Discover"), ("search", "Search"), ("settings", "Settings")]
                .iter()
                .map(|(id, name)| Item::check(format!("page:{id}"), *name, page == *id))
                .collect(),
        ));
        items.push(Item::check("player", "Show Player", ui.get_show_player()));
        items.push(Item::check("motion", "Moving Scenes", ui.global::<Scenery>().get_motion_on()));
        let appearance = self.appearance.as_str();
        items.push(Item::sub(
            "Appearance",
            [("system", "Follow the System"), ("light", "Light"), ("dark", "Dark")].iter().map(|(id, name)| Item::check(format!("appear:{id}"), *name, appearance == *id)).collect(),
        ));
        let mut themes = vec![Item::check("theme:default", "Default", self.theme_id.is_none())];
        themes.extend(self.themes.iter().map(|t| Item::check(format!("theme:{}", t.id), t.name.clone(), self.theme_id.as_deref() == Some(t.id.as_str()))));
        items.push(Item::sub("Theme", themes));
        items.push(Item::Separator);
        items.push(Item::action("settings", "Settings…"));
        items.push(Item::action("quit", "Quit Driftwave"));
        items
    }

    /// Adds a station to the library or takes it out (without opening its details).
    fn toggle_library(&mut self, source: &str, i: i32) {
        let Some(s) = self.station_from(source, i) else { return };
        if let Some(k) = self.stations.iter().position(|x| x.id == s.id) {
            self.stations.remove(k);
            self.message(&format!("Removed {} from your library.", s.name));
        } else {
            self.stations.push(s.clone());
            self.message(&format!("Added {} to your library.", s.name));
        }
        self.save_library();
        self.resort();
    }
}

/// Opens the Full layout's menu (call from a timer: the native menu runs its own loop). `card` is the station card
/// right-clicked, if any.
pub(crate) fn open(app: &Rc<RefCell<App>>) {
    let (items, host, ui) = {
        let mut a = app.borrow_mut();
        let card = a.menu_card.take();
        let Some(ui) = a.ui.upgrade() else { return };
        // A text field has the right-click to itself; over a station's details, adding a station or the GIPHY
        // search, the menu (which can go to another page) stays shut.
        let dir = ui.global::<Dir>();
        if dir.get_typing() || dir.get_details_open() || dir.get_add_open() || ui.global::<super::Bg>().get_search_open() {
            return;
        }
        log(&format!("full menu: {card:?}"));
        (a.full_menu_items(&ui, card), crate::classic_menu::host(&ui), ui)
    };
    let chosen = match host {
        Some(h) => popup(h, &items).ok().flatten(),
        None => None,
    };
    if let Some(id) = chosen {
        run(app, &ui, &id);
    }
}

pub(crate) fn run(app: &Rc<RefCell<App>>, ui: &MainWindow, id: &str) {
    log(&format!("full menu: {id}"));
    match id {
        "classic" => ui.invoke_to_classic(),
        "player" => ui.set_show_player(!ui.get_show_player()),
        "motion" => {
            let on = !ui.global::<Scenery>().get_motion_on();
            ui.global::<Scenery>().invoke_motion_chosen(on);
        }
        _ => {
            if let Some(page) = id.strip_prefix("page:") {
                ui.set_page(page.into());
            } else if let Some(a) = id.strip_prefix("appear:") {
                ui.invoke_choose_appearance(a.into());
            } else if let Some(rest) = id.strip_prefix("st:") {
                let mut parts = rest.splitn(3, ':');
                let (Some(what), Some(source), Some(i)) = (parts.next(), parts.next(), parts.next().and_then(|i| i.parse::<i32>().ok())) else { return };
                match what {
                    "play" | "stop" => {
                        if source == "library" {
                            if what == "stop" {
                                ui.invoke_stop();
                            } else {
                                ui.invoke_play_station(i);
                            }
                        } else {
                            ui.global::<Dir>().invoke_play_now(source.into(), i);
                        }
                    }
                    "details" => ui.global::<Dir>().invoke_open(source.into(), i),
                    "add" | "remove" => app.borrow_mut().toggle_library(source, i),
                    "homepage" => {
                        let page = app.borrow().station_from(source, i).and_then(|s| s.homepage);
                        if let Some(h) = page {
                            finding::open_url(&h);
                        }
                    }
                    _ => {}
                }
            } else if id == "quit" {
                played::quit_app(app);
            } else {
                // Shared with Classic's menu: playback, record, theme, settings.
                crate::classic_menu::run(app, ui, id);
            }
        }
    }
}
