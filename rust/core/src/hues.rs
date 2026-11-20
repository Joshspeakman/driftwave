//! Driftwave's colour rule (Josh, 2026-11-20): no teal, no purple and no pink, in any theme.
//!
//! `banned` says whether a colour is one of them (the built-in themes are tested against it). `safe` steers a
//! generated colour (a visualizer's rainbow, a placeholder tile) out of them without a visible jump: inside a banned
//! band the colour fades towards grey, and the hue turns to the nearer allowed side where the grey is complete, so
//! the change is continuous in RGB. `spread` squeezes a full rainbow's banned bands narrow first, so a hue sweep
//! (red, orange, gold, green, blue) only dips briefly through silver.

/// Teal, purple or pink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Banned {
    Teal,
    Purple,
    Pink,
}

/// Hue (degrees), saturation and lightness (HSL, 0–1) of an RGB colour (0–255).
pub fn hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-5 {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

/// Whether a colour reads as teal, purple or pink. Greys, near-blacks, near-whites, whites with the faintest tint and
/// nearly clear colours don't count: nobody sees a hue in them. Pale blue at the violet end reads as lavender, and
/// pale red as pink.
pub fn banned(r: u8, g: u8, b: u8, alpha: u8) -> Option<Banned> {
    let (h, s, l) = hsl(r, g, b);
    let chroma = r.max(g).max(b) - r.min(g).min(b);
    if s < 0.25 || chroma < 16 || l < 0.08 || l > 0.95 || alpha < 40 {
        return None;
    }
    if (160.0..197.0).contains(&h) {
        return Some(Banned::Teal);
    }
    if (245.0..290.0).contains(&h) || ((232.0..245.0).contains(&h) && l > 0.6) {
        return Some(Banned::Purple);
    }
    if (290.0..340.0).contains(&h) || ((h >= 340.0 || h < 8.0) && l > 0.72 && s > 0.45) {
        return Some(Banned::Pink);
    }
    None
}

// The bands `safe` steers around (degrees): where each starts, where its grey is complete, and where it ends. Below
// the grey the hue holds at the start, above it at the end, so nothing jumps. Teal: green to azure. Purple and pink:
// blue to crimson. Wider than `banned`'s, so what lands at an edge is plainly green, azure, blue or crimson (a pale
// colour right at the edge would still read aqua or periwinkle).
const BANDS: [(f32, f32, f32); 2] = [(150.0, 178.0, 205.0), (225.0, 268.0, 350.0)];

/// The allowed stretch of hues (degrees) that `hue` is in, or the nearest one: crimson to green (wrapping past 360:
/// the end is over 360), or azure to blue.
pub fn allowed_arc(hue: f32) -> (f32, f32) {
    let h = hue.rem_euclid(360.0);
    let (blue, warm) = ((BANDS[0].2, BANDS[1].0), (BANDS[1].2, BANDS[0].0 + 360.0));
    if h >= blue.0 && h <= blue.1 {
        return blue;
    }
    if h >= warm.0 || h <= BANDS[0].0 {
        return warm;
    }
    // In a band: the side it's nearer.
    let edge = nearest_allowed(h, 1.0, 0.5).0;
    if edge >= blue.0 && edge <= blue.1 { blue } else { warm }
}

/// A hue (degrees) and saturation (0–1) steered out of the banned bands: (hue, saturation).
pub fn safe(hue: f32, sat: f32) -> (f32, f32) {
    let h = hue.rem_euclid(360.0);
    for (lo, mid, hi) in BANDS {
        if h >= lo && h < hi {
            return if h < mid { (lo, sat * (mid - h) / (mid - lo)) } else { (hi, sat * (h - mid) / (hi - mid)) };
        }
    }
    (h, sat)
}

/// For colours chosen one at a time (a placeholder tile), where nothing blends: a hue in a banned band moves to the
/// nearer allowed edge at full strength, rather than fading through grey. Pale crimson warms to peach, as in
/// `safe_hsv`.
pub fn nearest_allowed(hue: f32, sat: f32, val: f32) -> (f32, f32) {
    let h = hue.rem_euclid(360.0);
    let h = BANDS.iter().find(|(lo, _, hi)| h >= *lo && h < *hi).map(|(lo, _, hi)| if h - lo < hi - h { *lo } else { *hi }).unwrap_or(h);
    safe_hsv(h, sat, val)
}

/// An 8-bit colour kept to the rule: a banned one moves to the nearest allowed hue, keeping its strength and
/// brightness (for colours a palette derives by mixing, such as a navy lightened to a lavender selection).
pub fn rule_rgb(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    if banned(r, g, b, 255).is_none() {
        return (r, g, b);
    }
    let (max, min) = (r.max(g).max(b) as f32 / 255.0, r.min(g).min(b) as f32 / 255.0);
    let (h, _, _) = hsl(r, g, b);
    let (s, v) = (if max > 0.0 { (max - min) / max } else { 0.0 }, max);
    let (h2, s2) = nearest_allowed(h, s, v);
    let hh = h2.rem_euclid(360.0) / 60.0;
    let i = hh.floor();
    let f = hh - i;
    let (p, q, t) = (v * (1.0 - s2), v * (1.0 - s2 * f), v * (1.0 - s2 * (1.0 - f)));
    let (r2, g2, b2) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    ((r2 * 255.0).round() as u8, (g2 * 255.0).round() as u8, (b2 * 255.0).round() as u8)
}

/// The same colour with another hue (degrees), keeping its HSV saturation and value.
pub fn with_hue(r: u8, g: u8, b: u8, hue: f32) -> (u8, u8, u8) {
    let (max, min) = (r.max(g).max(b) as f32 / 255.0, r.min(g).min(b) as f32 / 255.0);
    let (s, v) = (if max > 0.0 { (max - min) / max } else { 0.0 }, max);
    let hh = hue.rem_euclid(360.0) / 60.0;
    let i = hh.floor();
    let f = hh - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r2, g2, b2) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    ((r2 * 255.0).round() as u8, (g2 * 255.0).round() as u8, (b2 * 255.0).round() as u8)
}

/// `safe` for an HSV colour (all 0–1 but the hue, in degrees), which also knows how light it is: a pale crimson
/// reads as pink, so as it pales it warms to peach.
pub fn safe_hsv(hue: f32, sat: f32, val: f32) -> (f32, f32) {
    let (h, s) = safe(hue, sat);
    let l = val * (1.0 - s / 2.0);
    if (h >= 336.0 || h < 10.0) && l > 0.6 {
        let t = ((l - 0.6) / 0.1).clamp(0.0, 1.0);
        // Forward round the turn to 12 degrees (orange-red), never back through magenta.
        let from = if h >= 336.0 { h } else { h + 360.0 };
        return ((from + (372.0 - from) * t).rem_euclid(360.0), s);
    }
    (h, s)
}

/// A rainbow's hue (degrees) with the banned bands squeezed narrow (each to about a tenth of a turn's worth of
/// silver), so a sweep spends its time on the allowed colours. Feed the result to `safe`.
pub fn spread(hue: f32) -> f32 {
    // (hue, where along the turn it lands): crimson to green gets half the turn, the squeezed teal band a
    // fifteenth, azure to blue over a quarter, the squeezed purple and pink band a tenth, and crimson the rest.
    const MAP: [(f32, f32); 6] = [(0.0, 0.0), (150.0, 180.0), (205.0, 204.0), (225.0, 300.0), (350.0, 336.0), (360.0, 360.0)];
    let h = hue.rem_euclid(360.0);
    for w in MAP.windows(2) {
        let ((h0, t0), (h1, t1)) = (w[0], w[1]);
        if h >= t0 && h <= t1 {
            return h0 + (h1 - h0) * (h - t0) / (t1 - t0).max(1e-3);
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
        let h = h.rem_euclid(360.0) / 60.0;
        let i = h.floor();
        let f = h - i;
        let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
        let (r, g, b) = match i as i32 {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };
        ((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)
    }

    #[test]
    fn knows_the_banned_colours() {
        assert_eq!(banned(0x00, 0x80, 0x80, 255), Some(Banned::Teal));
        assert_eq!(banned(0x55, 0xFF, 0xFF, 255), Some(Banned::Teal));
        assert_eq!(banned(0x6B, 0x4E, 0xA0, 255), Some(Banned::Purple));
        assert_eq!(banned(0xCC, 0xCC, 0xFF, 255), Some(Banned::Purple));
        assert_eq!(banned(0xFF, 0x2E, 0x97, 255), Some(Banned::Pink));
        assert_eq!(banned(0xFF, 0xC0, 0xCB, 255), Some(Banned::Pink));
        for ok in [(0x2E, 0x7B, 0xFF), (0xFF, 0x6A, 0x2B), (0x4C, 0xD9, 0x64), (0xFF, 0xD2, 0x3F), (0x80, 0x80, 0x80), (0x4C, 0xC2, 0xFF)] {
            assert_eq!(banned(ok.0, ok.1, ok.2, 255), None, "{ok:?}");
        }
    }

    #[test]
    fn safe_never_lands_on_a_banned_colour_and_never_jumps() {
        for (sat, val) in [(0.9, 0.95), (0.55, 1.0), (0.3, 1.0), (0.8, 0.6), (1.0, 0.35)] {
            let mut last: Option<(u8, u8, u8)> = None;
            for k in 0..=3600 {
                let h = k as f32 / 10.0;
                let (h2, s2) = safe_hsv(h, sat, val);
                let c = rgb(h2, s2, val);
                assert_eq!(banned(c.0, c.1, c.2, 255), None, "hue {h} (s {sat}, v {val}) → {c:?}");
                if let Some(p) = last {
                    let d = (p.0 as i32 - c.0 as i32).abs().max((p.1 as i32 - c.1 as i32).abs()).max((p.2 as i32 - c.2 as i32).abs());
                    assert!(d <= 40, "jump at {h} (s {sat}, v {val}): {p:?} → {c:?}");
                }
                last = Some(c);
            }
        }
    }

    #[test]
    fn arcs_hold_their_families() {
        assert_eq!(allowed_arc(215.0), (205.0, 225.0));
        assert_eq!(allowed_arc(30.0), (350.0, 510.0));
        assert_eq!(allowed_arc(355.0), (350.0, 510.0));
        assert_eq!(allowed_arc(260.0), (205.0, 225.0));
        assert_eq!(allowed_arc(300.0), (350.0, 510.0));
    }

    #[test]
    fn single_colours_move_whole() {
        for k in 0..360 {
            let (h, s) = nearest_allowed(k as f32, 0.8, 0.7);
            let c = rgb(h, s, 0.7);
            assert_eq!(banned(c.0, c.1, c.2, 255), None, "hue {k} → {c:?}");
            assert!(s > 0.79, "hue {k} lost its colour");
        }
    }

    /// Every colour a built-in theme names (tokens, effects, scene layers, tiles) keeps the rule.
    #[test]
    fn built_in_themes_keep_the_rule() {
        fn walk(v: &serde_json::Value, path: &str, bad: &mut Vec<String>) {
            match v {
                serde_json::Value::Object(m) => m.iter().for_each(|(k, v)| walk(v, &format!("{path}.{k}"), bad)),
                serde_json::Value::Array(a) => a.iter().enumerate().for_each(|(i, v)| walk(v, &format!("{path}[{i}]"), bad)),
                serde_json::Value::String(s) if s.starts_with('#') => {
                    if let Some(c) = crate::theme::Color::parse(s) {
                        // Even a faint tint shows (a fifth of navy over white is lavender).
                        if let Some(k) = banned(c.r, c.g, c.b, if c.a >= 16 { 255 } else { 0 }) {
                            bad.push(format!("{path} = {s} ({k:?})"));
                        }
                    }
                }
                _ => {}
            }
        }
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes");
        let mut bad = Vec::new();
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = e.path();
            if path.extension().is_some_and(|x| x == "json") {
                let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
                walk(&v, &path.file_stem().unwrap().to_string_lossy(), &mut bad);
            }
        }
        // And the tokens as they're seen: each translucent one over its theme's background (and over white or black,
        // the light or dark base).
        for t in crate::theme::load_folder(&dir, true) {
            let back = t.colors.get("background").copied().unwrap_or(crate::theme::Color { a: 255, r: 0, g: 0, b: 0 });
            let base = if t.base == "light" { crate::theme::Color { a: 255, r: 255, g: 255, b: 255 } } else { crate::theme::Color { a: 255, r: 0, g: 0, b: 0 } };
            for (k, c) in &t.colors {
                if c.a == 255 {
                    continue;
                }
                for under in [back, base] {
                    let m = under.mix(crate::theme::Color { a: 255, ..*c }, c.a as f64 / 255.0);
                    if let Some(kind) = banned(m.r, m.g, m.b, 255) {
                        bad.push(format!("{}.{k} over {} ({kind:?})", t.id, under.text()));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{} banned colours:
{}", bad.len(), bad.join("
"));
    }

    #[test]
    fn a_spread_rainbow_is_mostly_colour() {
        let grey = (0..360).filter(|&k| safe(spread(k as f32), 1.0).1 < 0.5).count();
        assert!(grey < 40, "{grey} degrees of silver");
        // Allowed hues come through where they were (near enough).
        for h in [0.0f32, 30.0, 60.0, 120.0] {
            assert!((spread(h) - h).abs() < 35.0, "{h} → {}", spread(h));
        }
    }
}
