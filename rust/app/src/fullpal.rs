//! Every theme, in the Full layout: the theme's own tokens where it has them, Driftwave's Default (light or dark)
//! where it doesn't. Pure and tested: every built-in theme reads at WCAG contrast 4.5 for text and 3 for the accent.

use crate::palette::{contrast, readable};
use driftwave_core::theme::{Color, Theme};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FullPalette {
    pub background: Color,
    pub pane: Color,
    pub surface: Color,
    pub card: Color,
    pub card_stroke: Color,
    pub control: Color,
    pub control_stroke: Color,
    pub subtle: Color,
    pub divider: Color,
    /// Text on the page (the content surface).
    pub text: Color,
    pub text2: Color,
    pub text3: Color,
    /// Text on cards (the player, settings), in the navigation pane and title bar, and on buttons.
    pub card_text: Color,
    pub card_text2: Color,
    pub pane_text: Color,
    pub title_text: Color,
    pub control_text: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub on_accent: Color,
    pub dialog: Color,
}

const fn c(a: u8, r: u8, g: u8, b: u8) -> Color {
    Color { a, r, g, b }
}

/// Driftwave's own look, in dark or light.
pub fn default(dark: bool) -> FullPalette {
    if dark {
        FullPalette {
            background: c(255, 0x20, 0x20, 0x20),
            pane: c(255, 0x20, 0x20, 0x20),
            surface: c(255, 0x27, 0x27, 0x27),
            card: c(255, 0x2D, 0x2D, 0x2D),
            card_stroke: c(255, 0x3A, 0x3A, 0x3A),
            control: c(255, 0x37, 0x37, 0x37),
            control_stroke: c(255, 0x45, 0x45, 0x45),
            subtle: c(0x14, 255, 255, 255),
            divider: c(0x18, 255, 255, 255),
            text: c(255, 0xFF, 0xFF, 0xFF),
            text2: c(255, 0xC8, 0xC8, 0xC8),
            text3: c(255, 0x9A, 0x9A, 0x9A),
            card_text: c(255, 0xFF, 0xFF, 0xFF),
            card_text2: c(255, 0xC8, 0xC8, 0xC8),
            pane_text: c(255, 0xFF, 0xFF, 0xFF),
            title_text: c(255, 0xFF, 0xFF, 0xFF),
            control_text: c(255, 0xFF, 0xFF, 0xFF),
            accent: c(255, 0x4C, 0xC2, 0xFF),
            accent_text: c(255, 0x99, 0xEB, 0xFF),
            on_accent: c(255, 0, 0, 0),
            dialog: c(255, 0x2B, 0x2B, 0x2B),
        }
    } else {
        FullPalette {
            background: c(255, 0xF3, 0xF3, 0xF3),
            pane: c(255, 0xF3, 0xF3, 0xF3),
            surface: c(255, 0xF9, 0xF9, 0xF9),
            card: c(255, 0xFF, 0xFF, 0xFF),
            card_stroke: c(255, 0xE5, 0xE5, 0xE5),
            control: c(255, 0xFB, 0xFB, 0xFB),
            control_stroke: c(255, 0xD6, 0xD6, 0xD6),
            subtle: c(0x0C, 0, 0, 0),
            divider: c(0x14, 0, 0, 0),
            text: c(255, 0x1A, 0x1A, 0x1A),
            text2: c(255, 0x5C, 0x5C, 0x5C),
            text3: c(255, 0x6E, 0x6E, 0x6E),
            card_text: c(255, 0x1A, 0x1A, 0x1A),
            card_text2: c(255, 0x5C, 0x5C, 0x5C),
            pane_text: c(255, 0x1A, 0x1A, 0x1A),
            title_text: c(255, 0x1A, 0x1A, 0x1A),
            control_text: c(255, 0x1A, 0x1A, 0x1A),
            accent: c(255, 0x00, 0x5F, 0xB8),
            accent_text: c(255, 0x00, 0x3E, 0x92),
            on_accent: c(255, 0xFF, 0xFF, 0xFF),
            dialog: c(255, 0xF9, 0xF9, 0xF9),
        }
    }
}

/// Composites a translucent token over an opaque colour.
fn over(top: Color, under: Color) -> Color {
    let a = top.a as f64 / 255.0;
    Color { a: 255, ..under.mix(Color { a: 255, ..top }, a) }
}

/// The palette for `theme`; `None` is Default in `dark` or light.
pub fn full(theme: Option<&Theme>, dark: bool) -> FullPalette {
    let Some(t) = theme else { return default(dark) };
    let base = default(t.base != "light");
    let get = |k: &str| t.colors.get(k).copied();
    let background = get("background").map(|c| Color { a: 255, ..c }).unwrap_or(base.background);
    // Translucent tokens are for themes with pictures behind them; until those scenes are drawn here, composite
    // them over the background so text is judged against what's really there.
    let solid = |k: &str, fallback: Color| get(k).map(|c| over(c, background)).unwrap_or(fallback);
    let surface = solid("surface", base.surface);
    let card = solid("card", base.card);
    let pane = solid("pane", background);
    let control = solid("control", base.control);
    let accent = get("accent").map(|c| Color { a: 255, ..c }).unwrap_or(base.accent);
    // Each kind of background gets text that reads on it: themes can pair a light card with a dark page.
    let want_text = get("text").unwrap_or(base.text);
    let want_text2 = get("textSecondary").unwrap_or(base.text2);
    let text = readable(want_text, surface, 4.5);
    let text2 = readable(want_text2, surface, 4.5);
    let text3 = readable(get("textTertiary").unwrap_or(base.text3), surface, 3.0);
    let card_text = readable(want_text, card, 4.5);
    let card_text2 = readable(want_text2, card, 4.5);
    let pane_text = readable(want_text, pane, 4.5);
    let title_text = readable(want_text, background, 4.5);
    let control_text = readable(want_text, control, 4.5);
    let on_accent = readable(get("onAccent").unwrap_or(base.on_accent), accent, 4.5);
    let accent_text = readable(get("accentText").unwrap_or(accent), card, 4.5);
    FullPalette {
        background,
        pane,
        surface,
        card,
        card_stroke: get("cardStroke").unwrap_or(base.card_stroke),
        control,
        control_stroke: get("controlStroke").unwrap_or(base.control_stroke),
        subtle: get("subtle").unwrap_or(base.subtle),
        divider: get("divider").unwrap_or(base.divider),
        text,
        text2,
        text3,
        card_text,
        card_text2,
        pane_text,
        title_text,
        control_text,
        accent,
        accent_text,
        on_accent,
        dialog: solid("dialog", base.dialog),
    }
}

/// Whether a palette reads well: text on its cards and surfaces, and the accent's own text.
#[cfg_attr(not(test), allow(dead_code))]
pub fn check(p: &FullPalette) -> Vec<String> {
    let mut problems = Vec::new();
    for (part, fg, bg, min) in [
        ("text on the page", p.text, p.surface, 4.5),
        ("secondary text on the page", p.text2, p.surface, 4.5),
        ("text on cards", p.card_text, p.card, 4.5),
        ("secondary text on cards", p.card_text2, p.card, 4.5),
        ("text in the pane", p.pane_text, p.pane, 4.5),
        ("text in the title bar", p.title_text, p.background, 4.5),
        ("text on buttons", p.control_text, p.control, 4.5),
        ("text on accent", p.on_accent, p.accent, 4.5),
        ("accent text", p.accent_text, p.card, 4.5),
    ] {
        let ratio = contrast(fg, bg);
        if ratio < min - 0.01 {
            problems.push(format!("{part} {ratio:.2} < {min}"));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use driftwave_core::theme::load_folder;
    use std::path::PathBuf;

    #[test]
    fn every_theme_and_default_reads_well() {
        let themes = load_folder(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes"), true);
        assert!(themes.len() >= 26);
        let mut failures = Vec::new();
        for (name, p) in themes.iter().map(|t| (t.name.clone(), full(Some(t), true))).chain([("Default dark".to_string(), default(true)), ("Default light".to_string(), default(false))]) {
            for problem in check(&p) {
                failures.push(format!("{name}: {problem}"));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
