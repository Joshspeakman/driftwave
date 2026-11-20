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
    /// Text on the frames themselves (labels, status): readable on the face.
    pub label: Color,
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
    /// How much of the theme's picture shows through the panels' faces (0.4 unless the theme says).
    pub see: f32,
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

/// A derived colour kept to the colour rule (no teal, purple or pink). Violet-leaning blues (a pure navy) are blue
/// as they are, but lavender once the UI lightens them for a highlight or a glow, so they lean to a plainer blue now.
pub fn ruled(c: Color) -> Color {
    use driftwave_core::hues;
    let (r, g, b) = hues::rule_rgb(c.r, c.g, c.b);
    let (h, _, _) = hues::hsl(r, g, b);
    let chroma = r.max(g).max(b) - r.min(g).min(b);
    let (r, g, b) = if chroma >= 16 && (229.0..245.0).contains(&h) { hues::with_hue(r, g, b, 226.0) } else { (r, g, b) };
    Color { a: c.a, r, g, b }
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
    // The frame is what Full's panels are made of: the theme's surface, unless it leaves that clear (Ninety-Five's
    // desktop shows through it), then its cards'.
    let panel = match get("surface") {
        Some(s) if s.a >= 40 => Some(s),
        _ => get("card").or_else(|| get("pane")),
    };
    let surface = panel.map(|s| over(s, back)).unwrap_or(back);
    let control = get("control").map(|c| over(c, back)).unwrap_or(surface);

    // The frame: the theme's surface, nudged off the background so the panel reads as an object.
    let face = if dark { surface.mix(rgb(255, 255, 255), 0.06) } else { surface.mix(rgb(0, 0, 0), 0.04) };
    let bevel_light = face.mix(rgb(255, 255, 255), if dark { 0.22 } else { 0.7 });
    let bevel_dark = face.mix(rgb(0, 0, 0), if dark { 0.55 } else { 0.35 });
    let bright = dark && luminance(accent) > 0.3;
    // Windows 95, 3.1 and XP title bars were solid colour with a white title: their themes' bars stay nearly flat.
    let era = theme
        .and_then(|t| t.effects.get("chrome"))
        .and_then(|c| c.get("captions"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|c| matches!(c, "win95" | "win31" | "xp"));
    let title_a = accent.mix(face, if bright { 0.55 } else if era { 0.08 } else { 0.25 });
    let title_b = accent.mix(face, if bright { 0.85 } else if era { 0.3 } else { 0.75 });
    // The title sits in the middle of the bar: read against the middle of its gradient.
    let title_text = readable(if bright { rgb(240, 248, 240) } else { on_accent }, title_a.mix(title_b, 0.5), 4.5);

    // The LCD is always dark glass, tinted by the theme; its segments glow in the theme's accent text colour.
    // A theme whose screen is one colour (visTint) has its LCD in those colours instead.
    let tint = theme.and_then(|t| t.effects.get("visTint")).map(|o| {
        let c = |k: &str| o.get(k).and_then(serde_json::Value::as_str).and_then(Color::parse);
        (c("back").unwrap_or(rgb(0, 0, 0)), c("ink").unwrap_or(rgb(255, 255, 255)))
    });
    let lcd_back = match tint {
        Some((b, _)) => Color { a: 255, ..b },
        None => back.mix(rgb(0, 0, 0), if dark { 0.75 } else { 0.88 }),
    };
    let lcd_lit = readable(tint.map(|(_, ink)| ink).unwrap_or(accent_text), lcd_back, 4.5);
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

    let p = ClassicPalette {
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
        label: readable(glyph, face, 4.5),
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
        see: see_through(theme, panel),
    };
    let p = match theme.and_then(|t| t.effects.get("classic")) {
        Some(o) => overridden(p, o),
        None => p,
    };
    // The rule (for Driftwave's own themes: one you make keeps the colours you chose), then text made readable again
    // on what the rule changed, then the rule on that text.
    if theme.is_some_and(|t| !t.built_in) {
        return p;
    }
    all_ruled(settled(all_ruled(p)))
}

/// Every pair of text and what it sits on, readable again (after a theme's overrides, or the colour rule).
fn settled(mut p: ClassicPalette) -> ClassicPalette {
    p.title_text = readable(p.title_text, p.title_a.mix(p.title_b, 0.5), 4.5);
    p.lcd_lit = readable(p.lcd_lit, p.lcd_back, 4.5);
    p.lcd_ghost = Color { a: 26, ..p.lcd_lit };
    p.glyph = readable(p.glyph, p.button, 4.5);
    p.glyph_on = readable(p.glyph_on, p.button, 3.0);
    p.label = readable(p.label, p.face, 4.5);
    p.fill_low = readable(p.fill_low, p.groove, 3.0);
    p.fill_high = readable(p.fill_high, p.groove, 3.0);
    p.list_text = readable(p.list_text, p.list_back, 4.5);
    p.list_playing = readable(p.list_playing, p.list_back, 4.5);
    p.list_selected_text = readable(p.list_selected_text, p.list_selected_back, 4.5);
    p
}

fn all_ruled(p: ClassicPalette) -> ClassicPalette {
    ClassicPalette {
        face: ruled(p.face),
        bevel_light: ruled(p.bevel_light),
        bevel_dark: ruled(p.bevel_dark),
        title_a: ruled(p.title_a),
        title_b: ruled(p.title_b),
        title_text: ruled(p.title_text),
        lcd_back: ruled(p.lcd_back),
        lcd_lit: ruled(p.lcd_lit),
        lcd_ghost: ruled(p.lcd_ghost),
        button: ruled(p.button),
        glyph: ruled(p.glyph),
        glyph_on: ruled(p.glyph_on),
        label: ruled(p.label),
        groove: ruled(p.groove),
        fill_low: ruled(p.fill_low),
        fill_high: ruled(p.fill_high),
        list_back: ruled(p.list_back),
        list_text: ruled(p.list_text),
        list_playing: ruled(p.list_playing),
        list_selected_back: ruled(p.list_selected_back),
        list_selected_text: ruled(p.list_selected_text),
        glow: ruled(p.glow),
        ..p
    }
}

/// How much of the theme's picture shows through Classic's frames, as Full shows it through its panels: glass
/// themes as see-through as their panels, materials (walnut, concrete) mostly the material, the rest as their
/// panels are (solid panels, solid frames).
/// (Where the picture would drown the frames' text, `look` thickens them again.)
fn see_through(theme: Option<&Theme>, panel: Option<Color>) -> f32 {
    let Some(t) = theme else { return 0.4 };
    let glass = t.effects.get("scene").and_then(|s| s.get("surfaces")).and_then(serde_json::Value::as_str) == Some("translucent")
        || t.effects.contains_key("aurora");
    if glass {
        let a = panel.map(|c| c.a as f32 / 255.0).unwrap_or(0.6);
        return (1.0 - a).clamp(0.4, 0.75);
    }
    if t.effects.contains_key("wood") || t.effects.contains_key("concrete") {
        return 0.7;
    }
    // Otherwise as see-through as Full's panels, up to a hint: Ninety-Five's grey windows are solid.
    let a = panel.map(|c| c.a as f32 / 255.0).unwrap_or(1.0);
    (1.0 - a).clamp(0.0, 0.4)
}

/// A theme's own choices for Classic (the `classic` effect: any part's colour, and `see`), for materials the
/// derivation can't know (Woodgrain's frames are the walnut itself). Whatever it sets, text stays readable: the
/// derived pairs are checked again.
fn overridden(mut p: ClassicPalette, o: &serde_json::Map<String, serde_json::Value>) -> ClassicPalette {
    let c = |k: &str| o.get(k).and_then(serde_json::Value::as_str).and_then(Color::parse).map(|c| Color { a: 255, ..c });
    if let Some(face) = c("face") {
        p.face = face;
        p.bevel_light = face.mix(rgb(255, 255, 255), if p.dark { 0.22 } else { 0.7 });
        p.bevel_dark = face.mix(rgb(0, 0, 0), if p.dark { 0.55 } else { 0.35 });
        p.groove = face.mix(rgb(0, 0, 0), 0.45);
    }
    let fields: [(&str, &mut Color); 17] = [
        ("bevelLight", &mut p.bevel_light),
        ("bevelDark", &mut p.bevel_dark),
        ("titleA", &mut p.title_a),
        ("titleB", &mut p.title_b),
        ("titleText", &mut p.title_text),
        ("lcdBack", &mut p.lcd_back),
        ("lcdLit", &mut p.lcd_lit),
        ("button", &mut p.button),
        ("glyph", &mut p.glyph),
        ("glyphOn", &mut p.glyph_on),
        ("groove", &mut p.groove),
        ("fillLow", &mut p.fill_low),
        ("fillHigh", &mut p.fill_high),
        ("listBack", &mut p.list_back),
        ("listText", &mut p.list_text),
        ("listPlaying", &mut p.list_playing),
        ("listSelectedBack", &mut p.list_selected_back),
    ];
    for (k, field) in fields {
        if let Some(v) = c(k) {
            *field = v;
        }
    }
    if let Some(v) = c("listSelectedText") {
        p.list_selected_text = v;
    }
    p.label = c("label").unwrap_or(p.label);
    if let Some(see) = o.get("see").and_then(serde_json::Value::as_f64) {
        p.see = see.clamp(0.0, 1.0) as f32;
    }
    p.lcd_ghost = Color { a: 26, ..p.lcd_lit };
    settled(p)
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
            ("title text", p.title_text, p.title_a.mix(p.title_b, 0.5), 4.5),
            ("LCD", p.lcd_lit, p.lcd_back, 4.5),
            ("button glyph", p.glyph, p.button, 4.5),
            ("button on", p.glyph_on, p.button, 3.0),
            ("label", p.label, p.face, 4.5),
            ("slider", p.fill_low, p.groove, 3.0),
            ("list text", p.list_text, p.list_back, 4.5),
            ("playing station", p.list_playing, p.list_back, 4.5),
            ("selection", p.list_selected_text, p.list_selected_back, 4.5),
        ];
        for (part, fg, bg, min) in pairs {
            let c = contrast(fg, bg);
            assert!(c >= min - 0.01, "{name}: {part} contrast {c:.2} < {min}");
        }
    }

    fn dark_glass(name: &str, p: &ClassicPalette) {
        assert!(luminance(p.lcd_back) < 0.08, "{name}: the LCD should be dark glass");
    }

    #[test]
    fn every_theme_translates_readably() {
        let all = themes();
        assert!(all.len() >= 26, "found {} themes", all.len());
        for t in &all {
            let p = classic(Some(t), true);
            check(&t.name, &p);
            // The LCD is dark glass, unless the theme's screen is one colour of its own.
            if !t.effects.contains_key("visTint") {
                dark_glass(&t.name, &p);
            }
        }
        for (name, p) in [("Default (dark)", classic(None, true)), ("Default (light)", classic(None, false))] {
            check(name, &p);
            dark_glass(name, &p);
        }
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
