//! Customize Theme (from the C# app's ThemeSettings): your own themes, edited live. Built-in themes stay as they
//! are: customizing one makes a copy. A copy's name and base, its colour tokens, type (fonts, title size, weight,
//! letter spacing, capitals), shape (corners, cover roundness) and effects can be changed; it can be exported,
//! deleted, and themes can be imported. Every change is saved to the user themes folder at once and applied.

use super::{fullpal, App, EffectRow, Ed, TokenRow};
use driftwave_core::theme::{new_custom_id, Color, Theme, EFFECT_KINDS, TOKENS};
use serde_json::Map;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::rc::Rc;

fn slint_colour(c: Color) -> slint::Color {
    slint::Color::from_argb_u8(c.a, c.r, c.g, c.b)
}

impl App {
    /// The theme being edited: the current one, if it's yours.
    fn editing(&mut self) -> Option<&mut Theme> {
        let id = self.theme_id.clone()?;
        self.themes.iter_mut().find(|t| t.id == id && !t.built_in)
    }

    /// Saves the theme being edited and shows the change.
    fn edited(&mut self) {
        let dir = self.paths.themes();
        // A test run's themes live only as long as it does (it never leaves copies behind).
        let on_disk = !self.args.test_run();
        if let Some(t) = self.editing().filter(|_| on_disk) {
            let path = t.source_path.clone().unwrap_or_else(|| dir.join(format!("{}.json", t.id)));
            t.source_path = Some(path.clone());
            let json = t.to_json();
            let _ = std::fs::create_dir_all(&dir);
            if let Err(e) = std::fs::write(&path, json) {
                self.message(&format!("Couldn't save the theme: {e}"));
            }
        }
        let id = self.theme_id.clone();
        self.apply_theme(id);
        self.show_editor();
    }

    /// Fills the editor from the current theme (it's only editable when it's yours).
    pub(crate) fn show_editor(&mut self) {
        let dark = self.default_dark();
        let Some(ui) = self.ui.upgrade() else { return };
        let ed = ui.global::<Ed>();
        let id = self.theme_id.clone();
        let Some(t) = id.and_then(|id| self.themes.iter().find(|t| t.id == id && !t.built_in).cloned()) else {
            ed.set_editable(false);
            return;
        };
        let f = fullpal::full(Some(&t), dark);
        ed.set_editable(true);
        ed.set_name(t.name.clone().into());
        ed.set_base(t.base.clone().into());
        // Each token: the theme's own colour, or what it falls back to (shown, so it can be changed).
        let fallback = |k: &str| -> Color {
            match k {
                "background" => f.background,
                "surface" => f.surface,
                "card" => f.card,
                "cardStroke" => f.card_stroke,
                "control" => f.control,
                "controlStroke" => f.control_stroke,
                "subtle" => f.subtle,
                "divider" => f.divider,
                "text" => f.text,
                "textSecondary" => f.text2,
                "textTertiary" => f.text3,
                "accent" => f.accent,
                "accentText" => f.accent_text,
                "onAccent" => f.on_accent,
                "dialog" => f.dialog,
                _ => f.pane,
            }
        };
        let tokens: Vec<TokenRow> = TOKENS
            .iter()
            .map(|(k, label)| {
                let c = t.colors.get(*k).copied().unwrap_or_else(|| fallback(k));
                TokenRow { key: (*k).into(), label: (*label).into(), hex: c.text().into(), color: slint_colour(c) }
            })
            .collect();
        let model = ed.get_tokens();
        match model.as_any().downcast_ref::<VecModel<TokenRow>>().filter(|m| m.row_count() == tokens.len()) {
            Some(m) => {
                for (i, row) in tokens.into_iter().enumerate() {
                    if m.row_data(i).as_ref() != Some(&row) {
                        m.set_row_data(i, row);
                    }
                }
            }
            None => ed.set_tokens(ModelRc::from(Rc::new(VecModel::from(tokens)))),
        }
        ed.set_body_font(t.fonts.body.clone().unwrap_or_default().into());
        ed.set_title_font(t.fonts.title.clone().unwrap_or_default().into());
        ed.set_title_size(t.fonts.title_size.unwrap_or(28.0).round() as i32);
        ed.set_weight(match t.fonts.title_weight.as_deref().map(str::to_lowercase).as_deref() {
            Some("normal") | Some("regular") => 0,
            Some("bold") => 2,
            Some("black") => 3,
            _ => 1,
        });
        ed.set_spacing(t.fonts.title_spacing.unwrap_or(0));
        ed.set_upper(t.fonts.title_uppercase);
        ed.set_corners((t.corners.card.unwrap_or(8.0) / 20.0).clamp(0.0, 1.0) as f32);
        ed.set_cover((t.corners.cover.unwrap_or(0.07) / 0.5).clamp(0.0, 1.0) as f32);
        // Four groups: behind the panels, over everything, the titles, and the player and window.
        let group = |k: &str| match k {
            "grain" | "vignette" | "scanlines" | "frame" | "spotlight" => 1,
            "titleShadow" | "cursor" | "windowTitle" => 2,
            "lcd" | "analyzer" | "dial" | "vu" | "chrome" | "tiles" | "visTint" => 3,
            _ => 0,
        };
        let mut slots = [0; 4];
        let effects: Vec<EffectRow> = EFFECT_KINDS
            .iter()
            .map(|(k, label)| {
                let g = group(k);
                slots[g] += 1;
                EffectRow { key: (*k).into(), label: (*label).into(), on: t.effects.contains_key(*k), group: g as i32, slot: slots[g] - 1 }
            })
            .collect();
        ed.set_effect_counts(ModelRc::from(Rc::new(VecModel::from(slots.to_vec()))));
        ed.set_effects(ModelRc::from(Rc::new(VecModel::from(effects))));
        ed.set_confirm_delete(false);
    }

    /// Makes the current theme yours to change: a copy of a built-in (or Default), selected.
    pub(crate) fn customize_theme(&mut self) {
        if self.editing().is_some() {
            self.show_editor();
            return;
        }
        let dark = self.default_dark();
        let current = self.theme_id.clone().and_then(|id| self.themes.iter().find(|t| t.id == id).cloned());
        let mut t = current.clone().unwrap_or_else(|| {
            // Default, written out: its own colours (in the appearance in use), so there's something to change.
            let d = fullpal::default(dark);
            let mut t = Theme { base: if dark { "dark".into() } else { "light".into() }, backdrop: "none".into(), ..Theme::default() };
            for (k, c) in [
                ("background", d.background),
                ("surface", d.surface),
                ("card", d.card),
                ("cardStroke", d.card_stroke),
                ("control", d.control),
                ("controlStroke", d.control_stroke),
                ("subtle", d.subtle),
                ("divider", d.divider),
                ("text", d.text),
                ("textSecondary", d.text2),
                ("textTertiary", d.text3),
                ("accent", d.accent),
                ("accentText", d.accent_text),
                ("onAccent", d.on_accent),
                ("dialog", d.dialog),
                ("pane", d.pane),
            ] {
                t.colors.insert(k.into(), c);
            }
            t
        });
        // Your copy of this theme already exists: that's the one to change.
        let base = current.as_ref().map(|c| c.name.as_str()).unwrap_or("Default");
        let mine = format!("{base} (Mine)");
        if let Some(id) = self.themes.iter().find(|x| !x.built_in && x.name == mine).map(|x| x.id.clone()) {
            self.theme_id = Some(id.clone());
            self.apply_theme(Some(id));
            self.save_settings();
            return;
        }
        t.id = new_custom_id();
        t.name = mine;
        t.built_in = false;
        t.source_path = Some(self.paths.themes().join(format!("{}.json", t.id)));
        let id = t.id.clone();
        self.themes.push(t);
        self.theme_id = Some(id);
        self.edited();
        self.save_settings();
    }

    pub(crate) fn edit_name(&mut self, name: &str) {
        if let Some(t) = self.editing() {
            let n = name.trim();
            if !n.is_empty() {
                t.name = n.to_string();
            }
        }
        self.edited();
    }

    pub(crate) fn edit_base(&mut self, base: &str) {
        if let Some(t) = self.editing() {
            t.base = if base == "light" { "light".into() } else { "dark".into() };
        }
        self.edited();
    }

    pub(crate) fn edit_token(&mut self, key: &str, hex: &str) {
        let Some(c) = Color::parse(hex.trim()) else {
            self.message("A colour is a hex code, like #1E90FF (or #CC1E90FF with opacity).");
            self.show_editor();
            return;
        };
        if let Some(t) = self.editing() {
            t.colors.insert(key.to_string(), c);
        }
        self.edited();
    }

    pub(crate) fn edit_font(&mut self, which: &str, value: &str) {
        let v = value.trim();
        let v = (!v.is_empty()).then(|| v.to_string());
        if let Some(t) = self.editing() {
            match which {
                "body" => t.fonts.body = v,
                _ => t.fonts.title = v,
            }
        }
        self.edited();
    }

    pub(crate) fn edit_title_size(&mut self, n: i32) {
        if let Some(t) = self.editing() {
            t.fonts.title_size = Some((n as f64).clamp(14.0, 72.0));
        }
        self.edited();
    }

    pub(crate) fn edit_weight(&mut self, i: i32) {
        if let Some(t) = self.editing() {
            t.fonts.title_weight = Some(["Normal", "SemiBold", "Bold", "Black"][i.clamp(0, 3) as usize].into());
        }
        self.edited();
    }

    pub(crate) fn edit_spacing(&mut self, n: i32) {
        if let Some(t) = self.editing() {
            t.fonts.title_spacing = Some(n.clamp(-100, 800));
        }
        self.edited();
    }

    /// From the colour picker.
    pub(crate) fn edit_token_colour(&mut self, key: &str, c: slint::Color) {
        if let Some(t) = self.editing() {
            t.colors.insert(key.to_string(), Color { a: c.alpha(), r: c.red(), g: c.green(), b: c.blue() });
        }
        self.edited();
    }

    pub(crate) fn edit_upper(&mut self, on: bool) {
        if let Some(t) = self.editing() {
            t.fonts.title_uppercase = on;
        }
        self.edited();
    }

    pub(crate) fn edit_corners(&mut self, f: f32) {
        if let Some(t) = self.editing() {
            let r = (f as f64 * 20.0).round();
            t.corners.card = Some(r);
            t.corners.overlay = Some(r);
            t.corners.control = Some((r * 0.5).round());
        }
        self.edited();
    }

    pub(crate) fn edit_cover(&mut self, f: f32) {
        if let Some(t) = self.editing() {
            t.corners.cover = Some(((f as f64) * 0.5 * 100.0).round() / 100.0);
        }
        self.edited();
    }

    pub(crate) fn edit_effect(&mut self, key: &str, on: bool) {
        if let Some(t) = self.editing() {
            if on {
                t.effects.entry(key.to_string()).or_insert_with(Map::new);
            } else {
                t.effects.remove(key);
            }
        }
        self.edited();
    }

    pub(crate) fn export_theme(&mut self) {
        let Some(t) = self.theme_id.clone().and_then(|id| self.themes.iter().find(|t| t.id == id).cloned()) else {
            self.message("Default isn't a theme file; customize it first to export it.");
            return;
        };
        let file = format!("{}.json", t.name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-"));
        let Some(path) = rfd::FileDialog::new().add_filter("Driftwave theme", &["json"]).set_file_name(&file).save_file() else { return };
        match std::fs::write(&path, t.to_json()) {
            Ok(()) => self.message(&format!("Exported {}.", t.name)),
            Err(e) => self.message(&format!("Couldn't save the theme file: {e}")),
        }
    }

    pub(crate) fn import_theme(&mut self) {
        let Some(path) = rfd::FileDialog::new().add_filter("Driftwave theme", &["json"]).pick_file() else { return };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => return self.message(&format!("Couldn't read that file: {e}")),
        };
        let mut t = match Theme::from_json(&text, false) {
            Ok(t) => t,
            Err(e) => return self.message(&format!("That isn't a theme Driftwave can read ({}).", e.0)),
        };
        // Its own id, so it never replaces a built-in or another of yours.
        if self.themes.iter().any(|x| x.id == t.id) {
            t.id = new_custom_id();
        }
        t.built_in = false;
        t.source_path = Some(self.paths.themes().join(format!("{}.json", t.id)));
        // Named like one you have: numbered, so the gallery can tell them apart.
        let stem = t.name.clone();
        let mut n = 2;
        while self.themes.iter().any(|x| x.name == t.name) {
            t.name = format!("{stem} {n}");
            n += 1;
        }
        let id = t.id.clone();
        let name = t.name.clone();
        self.themes.push(t);
        self.theme_id = Some(id);
        self.edited();
        self.save_settings();
        self.message(&format!("Imported {name}."));
    }

    pub(crate) fn delete_theme(&mut self) {
        let Some(id) = self.theme_id.clone() else { return };
        let Some(i) = self.themes.iter().position(|t| t.id == id && !t.built_in) else { return };
        let t = self.themes.remove(i);
        if let Some(p) = t.source_path.as_ref().filter(|_| !self.args.test_run()) {
            let _ = std::fs::remove_file(p);
        }
        self.apply_theme(None);
        self.save_settings();
        self.show_editor();
        self.message(&format!("Deleted {}.", t.name));
    }
}
