//! A theme's whole-window effects, painted by Rust (ported from the C# app's WindowEffects):
//! - behind the panels: walnut veneer (`wood`), board-marked `concrete`, the `synthwave` sky, sun and floor, and the
//!   `aurora`'s drifting colour;
//! - over everything: film `grain`, a `vignette` and CRT `scanlines`, as one still, mostly clear picture.
//!
//! Synthwave's rolling grid rows and the scanlines' slow band are drawn by the UI (thin moving strips, so only those
//! strips redraw).

use crate::paint::{mix, Paint, Picture, Rect, Rgba, Rng, Shape};
use driftwave_core::theme::{Color, Theme};
use serde_json::{Map, Value};

fn num(o: &Map<String, Value>, k: &str, d: f64) -> f64 {
    o.get(k).and_then(Value::as_f64).unwrap_or(d)
}

fn rgba(c: Color) -> Rgba {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0]
}

fn opt(o: &Map<String, Value>, k: &str, d: Rgba) -> Rgba {
    o.get(k).and_then(Value::as_str).and_then(Color::parse).map(rgba).unwrap_or(d)
}

const fn hex(r: u8, g: u8, b: u8) -> Rgba {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

/// The synthwave look's colours.
#[derive(Clone)]
pub struct Synthwave {
    pub grid: Rgba,
    sky: Rgba,
    sun_top: Rgba,
    sun_bottom: Rgba,
}

#[derive(Clone)]
pub struct Aurora {
    colors: Vec<Rgba>,
    opacity: f32,
}

/// The effects a theme asks for.
#[derive(Clone, Default)]
pub struct Effects {
    pub wood: bool,
    pub concrete: Option<f32>,
    pub synthwave: Option<Synthwave>,
    pub aurora: Option<Aurora>,
    pub grain: Option<f32>,
    pub vignette: Option<(Rgba, f32)>,
    pub scanlines: Option<f32>,
    back: Rgba,
    dark: bool,
}

impl Effects {
    /// No effects at all (a scene that's only your own background).
    pub fn none() -> Effects {
        Effects::default()
    }
}

/// When the aurora's still frame sits on its paths (C#: EffectsClock.StillTime).
const STILL_TIME: f64 = 30.0;

impl Effects {
    pub fn for_theme(t: &Theme) -> Effects {
        let e = |k: &str| t.effects.get(k);
        Effects {
            wood: e("wood").is_some(),
            concrete: e("concrete").map(|o| num(o, "opacity", 0.5) as f32),
            synthwave: e("synthwave").map(|o| Synthwave {
                grid: opt(o, "grid", hex(255, 46, 151)),
                sky: opt(o, "sky", hex(59, 15, 92)),
                sun_top: opt(o, "sunTop", hex(255, 228, 94)),
                sun_bottom: opt(o, "sunBottom", hex(255, 46, 151)),
            }),
            aurora: e("aurora").map(|o| Aurora {
                colors: o
                    .get("colors")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|c| c.as_str().and_then(Color::parse)).map(rgba).collect::<Vec<_>>())
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| vec![hex(125, 211, 252), hex(196, 181, 253), hex(249, 168, 212)]),
                opacity: num(o, "opacity", 0.55) as f32,
            }),
            grain: e("grain").map(|o| num(o, "opacity", 0.06) as f32),
            vignette: e("vignette").map(|o| (opt(o, "color", [0.0, 0.0, 0.0, 1.0]), num(o, "opacity", 0.4) as f32)),
            scanlines: e("scanlines").map(|o| num(o, "opacity", 0.12) as f32),
            back: t.colors.get("background").map(|c| rgba(Color { a: 255, ..*c })).unwrap_or(hex(18, 6, 43)),
            dark: t.base != "light",
        }
    }

    pub fn any_behind(&self) -> bool {
        self.wood || self.concrete.is_some() || self.synthwave.is_some() || self.aurora.is_some()
    }

    pub fn any_in_front(&self) -> bool {
        self.grain.is_some() || self.vignette.is_some() || self.scanlines.is_some()
    }

    /// The vignette, painted into a picture (the Classic skin bakes it in, so nothing over the visualizer has to be
    /// blended every frame).
    pub fn vignette_into(&self, p: &mut Picture) {
        if let Some((c, o)) = self.vignette {
            vignette(p, c, o);
        }
    }

    /// Paints the effects that sit behind the panels (the aurora only when it holds still: it moves otherwise).
    /// `sun_x`: where the synthwave sun stands (px), over the page's middle.
    pub fn behind(&self, p: &mut Picture, scale: f32, still: bool, sun_x: f32) {
        if self.wood {
            wood(p, scale, self.back);
        }
        if let Some(o) = self.concrete {
            concrete(p, scale, o, self.dark);
        }
        if let Some(s) = &self.synthwave {
            synthwave(p, scale, s, self.back, sun_x);
        }
        if let (Some(a), true) = (&self.aurora, still) {
            aurora(p, scale, a, 0.0);
        }
    }

    /// The moving part of the effects at time `t` (the aurora).
    pub fn moving(&self, p: &mut Picture, scale: f32, t: f64) {
        if let Some(a) = &self.aurora {
            aurora(p, scale, a, t);
        }
    }

    pub fn moves(&self) -> bool {
        self.aurora.is_some()
    }

    /// What lies over everything: grain, vignette (unless `vignette` is false) and scanlines, as one premultiplied
    /// picture (None: nothing).
    pub fn front(&self, w: usize, h: usize, scale: f32, vignette: bool) -> Option<Picture> {
        if !(self.grain.is_some() || self.scanlines.is_some() || (vignette && self.vignette.is_some())) {
            return None;
        }
        let mut p = Picture::new(w, h);
        if let Some(o) = self.grain {
            // White specks of random strength, about 2 px each: a still texture (moving grain reads as flicker).
            let mut rng = Rng::new(11);
            let cell = (2.0 * scale).round().max(1.0) as usize;
            for by in (0..h).step_by(cell) {
                for bx in (0..w).step_by(cell) {
                    let a = rng.next() * o;
                    for y in by..(by + cell).min(h) {
                        for x in bx..(bx + cell).min(w) {
                            p.blend(y * w + x, [1.0, 1.0, 1.0], a);
                        }
                    }
                }
            }
        }
        if let Some(o) = self.scanlines {
            // One dark line every 3 px.
            let pitch = 3.0 * scale;
            let mut y = 0.0;
            while y < h as f32 {
                let row = y.round() as usize;
                for line in row..(row + scale.round().max(1.0) as usize).min(h) {
                    for x in 0..w {
                        p.blend(line * w + x, [0.0, 0.0, 0.0], o);
                    }
                }
                y += pitch;
            }
        }
        if let (Some((c, o)), true) = (self.vignette, vignette) {
            self::vignette(&mut p, c, o);
        }
        Some(p)
    }
}

/// Darkened edges: clear in the middle 60%, deepening to `o` at the rim of an ellipse 1.5× the picture.
fn vignette(p: &mut Picture, c: Rgba, o: f32) {
    let (w, h) = (p.w, p.h);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    for y in 0..h {
        let dy = (y as f32 + 0.5 - cy) / (0.75 * h as f32);
        for x in 0..w {
            let dx = (x as f32 + 0.5 - cx) / (0.75 * w as f32);
            let a = ((dx.hypot(dy) - 0.6) / 0.4).clamp(0.0, 1.0) * o;
            p.blend(y * w + x, [c[0], c[1], c[2]], a);
        }
    }
}

/// Walnut veneer: a lit gradient with wavy grain lines.
fn wood(p: &mut Picture, scale: f32, back: Rgba) {
    let (w, h) = (p.w as f32, p.h as f32);
    let back = if 0.2126 * back[0] + 0.7152 * back[1] + 0.0722 * back[2] < 0.1 { hex(70, 42, 24) } else { back };
    let black = [0.0, 0.0, 0.0, 1.0];
    let stops = vec![(0.0, mix(back, black, 0.25)), (0.45, back), (0.7, mix(back, hex(120, 72, 40), 0.25)), (1.0, mix(back, black, 0.3))];
    // Along (0,0) → (1, 0.3) of the window.
    let (ex, ey) = (w, 0.3 * h);
    let len2 = ex * ex + ey * ey;
    for y in 0..p.h {
        for x in 0..p.w {
            let t = ((x as f32 * ex + y as f32 * ey) / len2).clamp(0.0, 1.0);
            let c = crate::paint::sample_stops(&stops, t);
            p.blend(y * p.w + x, [c[0], c[1], c[2]], 1.0);
        }
    }
    let mut rng = Rng::new(1970);
    let lh = h / scale;
    let count = (lh / 9.0) as usize;
    let (dark, light) = (hex(30, 16, 8), hex(140, 86, 50));
    for k in 0..count {
        let y0 = k as f32 * lh / count as f32 + rng.next() * 6.0;
        let amp = 2.0 + rng.next() * 7.0;
        let (f1, f2, ph) = (0.004 + rng.next() * 0.006, 0.017 + rng.next() * 0.02, rng.next() * std::f32::consts::TAU);
        let is_dark = rng.next() < 0.65;
        let (c, a) = if is_dark { (dark, 0.35) } else { (light, 0.18) };
        let thick = (0.8 + rng.next() * if is_dark { 2.6 } else { 1.2 }) * scale;
        let at = |x: f32| y0 + amp * (x * f1 + ph).sin() + amp * 0.35 * (x * f2 + ph * 2.0).sin();
        let mut x = -20.0;
        while x <= w / scale + 40.0 {
            let nx = x + 24.0;
            p.line(x * scale, at(x) * scale, nx * scale, at(nx) * scale, thick, c, a);
            x = nx;
        }
    }
}

/// Board-marked concrete: soft blotches and specks, with formwork lines every 120 px.
fn concrete(p: &mut Picture, scale: f32, opacity: f32, dark_base: bool) {
    let mut rng = Rng::new(1952);
    let cell = 48.0 * scale * 1.6;
    let (gw, gh) = ((p.w as f32 / cell) as usize + 2, (p.h as f32 / cell) as usize + 2);
    let grid: Vec<f32> = (0..gw * gh).map(|_| rng.next()).collect();
    let _ = dark_base;
    for y in 0..p.h {
        let gy = y as f32 / cell;
        let (iy, fy) = (gy as usize, gy.fract());
        for x in 0..p.w {
            let gx = x as f32 / cell;
            let (ix, fx) = (gx as usize, gx.fract());
            let (a, b, c, d) = (grid[iy * gw + ix], grid[iy * gw + ix + 1], grid[(iy + 1) * gw + ix], grid[(iy + 1) * gw + ix + 1]);
            let v = (a * (1.0 - fx) + b * fx) * (1.0 - fy) + (c * (1.0 - fx) + d * fx) * fy;
            let n = rng.next() * 0.45 + v * 0.55;
            let shade = if n > 0.5 { 1.0 } else { 0.0 };
            p.blend(y * p.w + x, [shade, shade, shade], 70.0 / 255.0 * n * opacity);
        }
    }
    let mut y = 120.0;
    while y * scale < p.h as f32 {
        p.line(0.0, y * scale, p.w as f32, y * scale, scale, [0.0, 0.0, 0.0, 1.0], 0.12);
        y += 120.0;
    }
}

/// Retrowave: the sky, the floor and its vanishing lines. The sun is painted on its own (sun_picture), over the page's
/// glass; the rows rolling towards you are the UI's. The horizon is at 68% of the height.
fn synthwave(p: &mut Picture, scale: f32, s: &Synthwave, back: Rgba, sun_x: f32) {
    let (w, h) = (p.w as f32, p.h as f32);
    let horizon = h * 0.68;
    let sky = Paint::Stops(vec![(0.0, back), (0.7, s.sky), (1.0, mix(s.sky, s.sun_bottom, 0.45))], b'v');
    p.fill(Rect { x: 0.0, y: 0.0, w, h: horizon }, Shape::Rect(0.0), &sky, 1.0);
    let floor = Paint::Stops(vec![(0.0, mix(back, s.grid, 0.18)), (1.0, back)], b'v');
    p.fill(Rect { x: 0.0, y: horizon, w, h: h - horizon }, Shape::Rect(0.0), &floor, 1.0);
    for i in -14..=14 {
        let i = i as f32;
        p.line(sun_x + i * w * 0.012, horizon, sun_x + i * w * 0.11, h, 1.2 * scale, s.grid, 0.55);
    }
    let glow = mix(s.grid, [1.0, 1.0, 1.0, 1.0], 0.4);
    p.fill(Rect { x: 0.0, y: horizon - scale, w, h: 2.0 * scale }, Shape::Rect(0.0), &Paint::Solid(glow), 1.0);
}

/// Frosted Glass: big soft colour blobs, each wandering slowly through a few points and back (60 s and more a loop).
fn aurora(p: &mut Picture, _scale: f32, a: &Aurora, t: f64) {
    let (w, h) = (p.w as f32, p.h as f32);
    let d = w.max(h) * 0.75;
    for (i, c) in a.colors.iter().enumerate() {
        let fi = i as f64;
        // A slow Lissajous figure: steady speed, no corners.
        let tau = std::f64::consts::TAU;
        let (t1, t2) = (60.0 + fi * 17.0, 77.0 + fi * 17.0);
        let t = t + STILL_TIME;
        let x = w * (0.5 + 0.38 * (tau * t / t1 + fi * 2.1).sin() as f32) - d / 2.0;
        let y = h * (0.5 + 0.34 * (tau * t / t2 + fi * 1.7).cos() as f32) - d / 2.0;
        let light = Paint::Stops(vec![(0.0, [c[0], c[1], c[2], a.opacity]), (1.0, [c[0], c[1], c[2], 0.0])], b'v');
        p.fill(Rect { x, y, w: d, h: d }, Shape::Radial, &light, 1.0);
    }
}

impl Effects {
    /// The synthwave sun on its own (transparent around it): a smooth disc cut into slices by gaps that widen towards
    /// the horizon, at `alpha`. With where its top left goes (px). The horizon is at 68% of `h`.
    pub fn sun_picture(&self, w: f32, h: f32, sun_x: f32, scale: f32, alpha: f32) -> Option<(Picture, f32, f32)> {
        let s = self.synthwave.as_ref()?;
        let horizon = h * 0.68;
        let r = w.min(h) * 0.2;
        let (cx, cy) = (sun_x, horizon - r * 0.3);
        let (x0, y0) = ((cx - r - 1.0).floor().max(0.0), (cy - r - 1.0).floor().max(0.0));
        let (pw, ph) = ((2.0 * r + 3.0).ceil() as usize, (horizon - y0).ceil().max(1.0) as usize);
        let mut p = Picture::new(pw, ph);
        let step = 6.0 * scale;
        for py in 0..ph {
            let y = y0 + py as f32 + 0.5;
            let frac = (y - (cy - r)) / (2.0 * r);
            let gap = if frac < 0.42 { 0.0 } else { ((frac - 0.42) * 14.0).min(4.5) * scale };
            if (y - (cy - r)).rem_euclid(step) >= step - gap {
                continue;
            }
            let c = mix(s.sun_top, s.sun_bottom, frac.clamp(0.0, 1.0));
            for px in 0..pw {
                let d = (x0 + px as f32 + 0.5 - cx).hypot(y - cy);
                let cov = (r + 0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    p.blend(py * pw + px, [c[0], c[1], c[2]], cov * alpha);
                }
            }
        }
        Some((p, x0, y0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use driftwave_core::theme::load_folder;
    use std::path::PathBuf;

    #[test]
    fn effects_paint_without_panicking_at_odd_sizes() {
        let themes = load_folder(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes"), true);
        for t in &themes {
            let e = Effects::for_theme(t);
            for (w, h, s) in [(1usize, 1usize, 1.0f32), (37, 211, 1.0), (300, 200, 1.5)] {
                let mut p = Picture::filled(w, h, [0, 0, 0]);
                e.behind(&mut p, s, true, w as f32 / 2.0);
                e.moving(&mut p, s, 12.0);
                let _ = e.front(w, h, s, true);
            }
        }
    }
}
