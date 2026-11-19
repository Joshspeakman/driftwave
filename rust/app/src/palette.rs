//! Every theme, in the classic layout. The classic chrome has its own parts (bevelled frame, title bar, LCD,
//! marquee, buttons, sliders, the station list); this works out their colours from a theme's tokens, so all 26
//! themes and your own translate without any theme having to know the layout exists.
//!
//! Pure and tested: every built-in theme (and the Windows-look Default) gives a full palette whose text is readable
//! (WCAG contrast 4.5 for text, 3 for the LCD's large digits and for controls).

use driftwave_core::theme::{Color, Theme};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassicPalette {
    /// The frame's face, and its bevel's light and dark edges.
    pub face: Color,
    pub bevel_light: Color,
    pub bevel_dark: Color,
    /// The title bar: a gradient between two colours, its stripes and its text.
    pub title_a: Color,
    pub title_b: Color,
    pub title_text: Color,
    /// The LCD: background, lit segments and text, and unlit segments (a faint ghost of every digit).
    pub lcd_back: Color,
    pub lcd_lit: Color,
    pub lcd_ghost: Color,
    /// Buttons: face and glyph; the glyph of a button that's on.
    pub button: Color,
    pub glyph: Color,
    pub glyph_on: Color,
    /// Sliders: the groove, and the fill from low to high.
    pub groove: Color,
    pub fill_low: Color,
    pub fill_high: Color,
    /// The station list: background, text, the playing station, and the selection.
    pub list_back: Color,
    pub list_text: Color,
    pub list_playing: Color,
    pub list_selected_back: Color,
    pub list_selected_text: Color,
    /// Neon edges and glows (themes with a neon look).
    pub glow: Color,
    pub dark: bool,
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { a: 255, r, g, b }
}

/// WCAG relative luminance.
pub fn luminance(c: Color) -> f64 {
    let lin = |v: u8| {
        let s = v as f64 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

pub fn contrast(a: Color, b: Color) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `fg`, moved towards white or black (whichever `bg` isn't) until it reads at `min` against `bg`.
pub fn readable(fg: Color, bg: Color, min: f64) -> Color {
    let fg = Color { a: 255, ..fg };
    if contrast(fg, bg) >= min {
        return fg;
    }
    let (white, black) = (rgb(255, 255, 255), rgb(0, 0, 0));
    let target = if contrast(white, bg) >= contrast(black, bg) { white } else { black };
    let mut t = 0.0;
    let mut c = fg;
    while t < 1.0 && contrast(c, bg) < min {
        t += 0.05;
        c = fg.mix(target, t);
    }
    c
}

/// Composites a translucent token over an opaque colour (themes often give glassy surfaces).
fn over(top: Color, under: Color) -> Color {
    let a = top.a as f64 / 255.0;
    Color { a: 255, ..under.mix(Color { a: 255, ..top }, a) }
}

/// The palette for `theme`; `None` is Default (the Windows look) in `dark` or light.
pub fn classic(theme: Option<&Theme>, dark: bool) -> ClassicPalette {
    let get = |k: &str| theme.and_then(|t| t.colors.get(k).copied());
    let dark = theme.map(|t| t.base != "light").unwrap_or(dark);
    // Default: a cool graphite frame, or a soft silver one in light mode, with a calm blue accent.
    let (def_back, def_text, def_accent) =
        if dark { (rgb(0x20, 0x22, 0x2A), rgb(0xEE, 0xF0, 0xF5), rgb(0x4C, 0xC2, 0xFF)) } else { (rgb(0xE8, 0xEA, 0xEF), rgb(0x1A, 0x1C, 0x22), rgb(0x00, 0x5F, 0xB8)) };
    let back = get("background").map(|c| Color { a: 255, ..c }).unwrap_or(def_back);
    let text = get("text").unwrap_or(def_text);
    let accent = Color { a: 255, ..get("accent").unwrap_or(def_accent) };
    let accent_text = Color { a: 255, ..get("accentText").unwrap_or(accent) };
    let on_accent = get("onAccent").unwrap_or(if luminance(accent) > 0.4 { rgb(0, 0, 0) } else { rgb(255, 255, 255) });
    let surface = get("surface").map(|s| over(s, back)).unwrap_or(back);
    let control = get("control").map(|c| over(c, back)).unwrap_or(surface);

    // The frame: the theme's surface, nudged off the background so the panel reads as an object.
    let face = if dark { surface.mix(rgb(255, 255, 255), 0.06) } else { surface.mix(rgb(0, 0, 0), 0.04) };
    let bevel_light = face.mix(rgb(255, 255, 255), if dark { 0.22 } else { 0.7 });
    let bevel_dark = face.mix(rgb(0, 0, 0), if dark { 0.55 } else { 0.35 });
    let bright = dark && luminance(accent) > 0.3;
    let title_a = accent.mix(face, if bright { 0.55 } else { 0.25 });
    let title_b = accent.mix(face, if bright { 0.85 } else { 0.75 });
    let title_text = readable(if bright { rgb(240, 248, 240) } else { on_accent }, title_a, 4.5);

    // The LCD is always dark glass, tinted by the theme; its segments glow in the theme's accent text colour.
    let lcd_back = back.mix(rgb(0, 0, 0), if dark { 0.75 } else { 0.88 });
    let lcd_lit = readable(accent_text, lcd_back, 4.5);
    let lcd_ghost = Color { a: 26, ..lcd_lit };

    let button = if dark { control.mix(rgb(255, 255, 255), 0.08) } else { control.mix(rgb(255, 255, 255), 0.5) };
    let glyph = readable(text, button, 4.5);
    let glyph_on = readable(accent_text, button, 3.0);
    let groove = face.mix(rgb(0, 0, 0), 0.45);
    let fill_low = readable(accent, groove, 3.0);
    let fill_high = readable(accent_text.mix(rgb(255, 255, 255), 0.25), groove, 3.0);

    let list_back = if dark { back.mix(rgb(0, 0, 0), 0.35) } else { back.mix(rgb(255, 255, 255), 0.5) };
    let list_text = readable(text, list_back, 4.5);
    let list_playing = readable(accent_text, list_back, 4.5);
    let list_selected_back = accent.mix(list_back, 0.35);
    let list_selected_text = readable(on_accent, list_selected_back, 4.5);
    let glow = theme
        .and_then(|t| t.effects.get("neonEdges"))
        .and_then(|o| o.get("content"))
        .and_then(|v| v.as_str())
        .and_then(Color::parse)
        .unwrap_or(Color { a: 0, ..accent });

    ClassicPalette {
        face,
        bevel_light,
        bevel_dark,
        title_a,
        title_b,
        title_text,
        lcd_back,
        lcd_lit,
        lcd_ghost,
        button,
        glyph,
        glyph_on,
        groove,
        fill_low,
        fill_high,
        list_back,
        list_text,
        list_playing,
        list_selected_back,
        list_selected_text,
        glow,
        dark,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use driftwave_core::theme::load_folder;
    use std::path::PathBuf;

    fn themes() -> Vec<Theme> {
        load_folder(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes"), true)
    }

    fn check(name: &str, p: &ClassicPalette) {
        let pairs = [
            ("title text", p.title_text, p.title_a, 4.5),
            ("LCD", p.lcd_lit, p.lcd_back, 4.5),
            ("button glyph", p.glyph, p.button, 4.5),
            ("button on", p.glyph_on, p.button, 3.0),
            ("slider", p.fill_low, p.groove, 3.0),
            ("list text", p.list_text, p.list_back, 4.5),
            ("playing station", p.list_playing, p.list_back, 4.5),
            ("selection", p.list_selected_text, p.list_selected_back, 4.5),
        ];
        for (part, fg, bg, min) in pairs {
            let c = contrast(fg, bg);
            assert!(c >= min - 0.01, "{name}: {part} contrast {c:.2} < {min}");
        }
        assert!(luminance(p.lcd_back) < 0.08, "{name}: the LCD should be dark glass");
    }

    #[test]
    fn every_theme_translates_readably() {
        let all = themes();
        assert!(all.len() >= 26, "found {} themes", all.len());
        for t in &all {
            check(&t.name, &classic(Some(t), true));
        }
        check("Default (dark)", &classic(None, true));
        check("Default (light)", &classic(None, false));
    }

    #[test]
    fn themes_look_different_from_each_other() {
        let all = themes();
        let mut seen = std::collections::HashSet::new();
        for t in &all {
            let p = classic(Some(t), true);
            seen.insert((p.face.r, p.face.g, p.face.b, p.lcd_lit.r, p.lcd_lit.g, p.lcd_lit.b));
        }
        assert!(seen.len() >= all.len() - 2, "only {} distinct palettes for {} themes", seen.len(), all.len());
    }

    #[test]
    fn readable_moves_away_from_the_background() {
        let bg = rgb(10, 10, 10);
        let c = readable(rgb(30, 30, 30), bg, 4.5);
        assert!(contrast(c, bg) >= 4.5 && luminance(c) > luminance(rgb(30, 30, 30)));
        let bg = rgb(240, 240, 240);
        let c = readable(rgb(200, 200, 200), bg, 4.5);
        assert!(contrast(c, bg) >= 4.5);
    }
}
