//! A native context menu (Classic's right-click menu), and the one place native menu choices are read.
//!
//! Windows' and macOS's own popup menus, as Winamp's right-click menu was: they can reach past the window (Classic
//! can be a thin strip), and bring submenus, checkmarks and keyboard use. Elsewhere there is none (`popup` says so).
//!
//! The tray menu and this one report choices through the same channel, so both read it through [`take`]: each takes
//! only its own, and leaves the other's for it.

use std::sync::Mutex;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

/// A menu entry. Ids are the caller's own; [`popup`] returns the chosen one.
pub enum Item {
    Action { id: String, label: String, enabled: bool },
    Check { id: String, label: String, checked: bool },
    Separator,
    Sub { label: String, items: Vec<Item> },
}

impl Item {
    pub fn action(id: impl Into<String>, label: impl Into<String>) -> Item {
        Item::Action { id: id.into(), label: label.into(), enabled: true }
    }
    pub fn check(id: impl Into<String>, label: impl Into<String>, checked: bool) -> Item {
        Item::Check { id: id.into(), label: label.into(), checked }
    }
    pub fn sub(label: impl Into<String>, items: Vec<Item>) -> Item {
        Item::Sub { label: label.into(), items }
    }
}

/// The window a menu belongs to.
pub enum Host {
    Win32(isize),
    AppKit(*mut std::ffi::c_void),
}

const PREFIX: &str = "ctx:";

/// A context menu's choice (the tray leaves these for it).
pub fn is_context_menu_id(id: &str) -> bool {
    id.starts_with(PREFIX)
}

static PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Native menu choices (their ids) that `mine` claims, leaving the rest for their own menu's next look.
pub fn take(mine: impl Fn(&str) -> bool) -> Vec<String> {
    let mut pending = PENDING.lock().unwrap();
    pending.extend(MenuEvent::receiver().try_iter().map(|e| e.id.0));
    let (taken, rest): (Vec<String>, Vec<String>) = pending.drain(..).partition(|id| mine(id));
    *pending = rest;
    taken
}

fn build_into(add: &dyn Fn(&dyn tray_icon::menu::IsMenuItem), items: &[Item]) {
    for item in items {
        match item {
            Item::Action { id, label, enabled } => add(&MenuItem::with_id(format!("{PREFIX}{id}"), label, *enabled, None)),
            Item::Check { id, label, checked } => add(&CheckMenuItem::with_id(format!("{PREFIX}{id}"), label, true, *checked, None)),
            Item::Separator => add(&PredefinedMenuItem::separator()),
            Item::Sub { label, items } => {
                let sub = Submenu::new(label, true);
                build_into(&|i| drop(sub.append(i)), items);
                add(&sub);
            }
        }
    }
}

/// There are no native menus here (the app shows its own instead).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoNativeMenus;

/// Shows `items` at the pointer and waits for a choice (or none). Returns the chosen item's id; `Err` where there are
/// no native menus.
pub fn popup(host: Host, items: &[Item]) -> Result<Option<String>, NoNativeMenus> {
    let menu = Menu::new();
    build_into(&|i| drop(menu.append(i)), items);
    #[allow(unused_variables)]
    let shown = match host {
        #[cfg(windows)]
        Host::Win32(hwnd) => {
            use tray_icon::menu::ContextMenu;
            crate::winapp::dark_menus(hwnd);
            // SAFETY: the handle is the app's own live window.
            unsafe { menu.show_context_menu_for_hwnd(hwnd, None) };
            true
        }
        #[cfg(target_os = "macos")]
        Host::AppKit(view) => {
            use tray_icon::menu::ContextMenu;
            // SAFETY: the view is the app's own live window's.
            unsafe { menu.show_context_menu_for_nsview(view, None) };
            true
        }
        #[allow(unreachable_patterns)]
        _ => false,
    };
    if !shown {
        return Err(NoNativeMenus);
    }
    Ok(take(|id| id.starts_with(PREFIX)).into_iter().next().map(|id| id[PREFIX.len()..].to_string()))
}
