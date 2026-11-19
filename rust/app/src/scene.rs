//! A theme's scene behind the Full layout: its pictures, gradients and lights, and what moves among them (rain
//! sheets, a slow neon breathe, people's lights walking by, drops running down the glass). Ported from the retired
//! C# app's SceneEffect and GlassDrops (the format is docs/THEMES.md).
//!
//! Two pictures, for lightness:
//! - the **backdrop**: everything still, composited once per window size and theme (it costs nothing while it sits).
//!   With motion off it also holds a still moment of what would move (the rain, the drops on the glass);
//! - the **motion** layer: what moves, drawn at half resolution into a transparent picture a dozen times a second,
//!   only while the window is in front, and scaled up smoothly by the UI.
//!
//! Nothing here flashes: the breathe takes at least 4 s a cycle and passing lights fade in and out over seconds
//! (the scene_flash test runs Night Market's motion through the flash check).

use crate::effects::Effects;
use crate::paint::{fixed, premultiply, Paint, Rgba, Rng, Shape};
pub use crate::paint::{Picture, Rect};
use driftwave_core::theme::{Color, Theme};
use image::{imageops, RgbaImage};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// The Full layout's geometry (logical px), for a scene's regions.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Geometry {
    pub width: f32,
    pub height: f32,
    pub title: f32,
    pub pane: f32,
    pub player: f32,
    /// Where the window's own title ends in the title bar.
    pub title_end: f32,
}

impl Geometry {
    /// The page's card (it holds the player too) and the player's card, as the Full layout draws them.
    pub fn content_card(&self) -> Rect {
        Rect { x: self.pane, y: self.title, w: (self.width - self.pane).max(0.0), h: (self.height - self.title).max(0.0) }
    }

    pub fn player_card(&self) -> Option<Rect> {
        (self.player > 0.0).then(|| Rect { x: self.width - self.player + 4.0, y: self.title + 8.0, w: self.player - 12.0, h: (self.height - self.title - 16.0).max(0.0) })
    }

    /// A scene's area: `titlebar` (the free stretch between the window's title and its caption buttons),
    /// `titlebar-full`, `pane`, `content` (between the pane and the player), `player`, or the whole window.
    pub fn region(&self, name: Option<&str>) -> Rect {
        let (w, h) = (self.width, self.height);
        match name {
            Some("titlebar") => {
                let x = self.title_end + 16.0;
                Rect { x, y: 0.0, w: (w - 150.0 - x).max(0.0), h: self.title }
            }
            Some("titlebar-full") => Rect { x: 0.0, y: 0.0, w, h: self.title },
            Some("pane") => Rect { x: 0.0, y: self.title, w: self.pane, h: h - self.title },
            Some("content") => Rect { x: self.pane, y: self.title, w: (w - self.pane - self.player).max(0.0), h: h - self.title },
            Some("player") => Rect { x: (w - self.player).max(0.0), y: self.title, w: self.player, h: h - self.title },
            _ => Rect { x: 0.0, y: 0.0, w, h },
        }
    }
}

/// The folder of theme art (see `resources::find`).
pub fn art_dir() -> Option<PathBuf> {
    crate::resources::find("theme-art", "CREDITS.txt")
}

fn num(o: &Map<String, Value>, k: &str, d: f64) -> f64 {
    o.get(k).and_then(Value::as_f64).unwrap_or(d)
}

fn str_of<'a>(o: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    o.get(k).and_then(Value::as_str)
}

/// A coordinate: a number of px (negative: from the far edge), or "NN%" of `span`.
pub fn coord(v: Option<&Value>, span: f32, d: f32) -> f32 {
    match v {
        Some(Value::Number(n)) => {
            let n = n.as_f64().unwrap_or(0.0) as f32;
            if n < 0.0 {
                span + n
            } else {
                n
            }
        }
        Some(Value::String(s)) if s.ends_with('%') => s.trim_end_matches('%').parse::<f32>().unwrap_or(0.0) / 100.0 * span,
        Some(Value::String(s)) => s.parse::<f32>().unwrap_or(d),
        _ => d,
    }
}

fn colour(v: Option<&Value>) -> Option<Color> {
    v.and_then(Value::as_str).and_then(Color::parse)
}

fn rgba(c: Color) -> Rgba {
    [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0]
}

/// Gradient stops: ["#a", "#b"] evenly, or [["#a", 0], ["#b", 1]].
fn stops(v: Option<&Value>) -> Vec<(f32, Rgba)> {
    let Some(Value::Array(a)) = v else { return Vec::new() };
    let n = a.len();
    a.iter()
        .enumerate()
        .filter_map(|(i, s)| match s {
            Value::Array(p) => Some((p.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32, rgba(colour(p.first())?))),
            other => Some((if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 }, rgba(colour(Some(other))?))),
        })
        .collect()
}

/// A layer's fill (or stroke): a colour, or stops along its `angle`.
fn paint_of(l: &Map<String, Value>, key: &str) -> Option<Paint> {
    match l.get(key) {
        Some(Value::Array(_)) => Some(Paint::Stops(
            stops(l.get(key)),
            match str_of(l, "angle") {
                Some("h") => b'h',
                Some("d") => b'd',
                _ => b'v',
            },
        )),
        Some(Value::String(s)) => Color::parse(s).map(|c| Paint::Solid(rgba(c))),
        _ => None,
    }
}

/// Pixel art's rows, each character its palette colour (anything not in the palette is clear).
fn pixel_rows(l: &Map<String, Value>) -> Vec<Vec<Option<Rgba>>> {
    let palette: HashMap<char, Rgba> = l
        .get("palette")
        .and_then(Value::as_object)
        .map(|p| {
            p.iter()
                .filter_map(|(k, v)| {
                    let mut chars = k.chars();
                    let ch = chars.next()?;
                    if chars.next().is_some() {
                        return None;
                    }
                    Some((ch, rgba(v.as_str().and_then(Color::parse)?)))
                })
                .collect()
        })
        .unwrap_or_default();
    l.get("rows")
        .and_then(Value::as_array)
        .map(|rows| rows.iter().map(|r| r.as_str().unwrap_or("").chars().map(|ch| palette.get(&ch).copied()).collect()).collect())
        .unwrap_or_default()
}

/// One copy of a layer (`repeat` makes a row of them, `scatter` strews them): how far it's moved, its colour if
/// scattered, how faded and stretched it is, and its motion's time shift.
struct Copy {
    ox: f32,
    oy: f32,
    color: Option<Rgba>,
    fade: f32,
    jw: f32,
    jh: f32,
    offset: f64,
}

fn copies(l: &Map<String, Value>, region: Rect) -> Vec<Copy> {
    let m = l.get("motion").and_then(Value::as_object);
    let (start, stagger, random) = m.map(|m| (num(m, "start", 0.0), num(m, "offset", 0.0), num(m, "randomOffset", 0.0))).unwrap_or((0.0, 0.0, 0.0));
    let jitter = l.get("jitter").and_then(Value::as_object);
    let stretch = |rng: &mut Rng| jitter.map(|j| (1.0 + (rng.next() - 0.5) * num(j, "w", 0.0) as f32, 1.0 + (rng.next() - 0.5) * num(j, "h", 0.0) as f32)).unwrap_or((1.0, 1.0));
    let mut out = Vec::new();
    if let Some(sc) = l.get("scatter").and_then(Value::as_object) {
        let n = (num(sc, "n", 10.0) as usize).clamp(1, 600);
        let mut rng = Rng::new(num(sc, "seed", 7.0) as u32);
        let (sw, sh) = (coord(sc.get("w"), region.w, region.w), coord(sc.get("h"), region.h, region.h));
        let palette: Vec<Rgba> = sc.get("colors").and_then(Value::as_array).map(|a| a.iter().filter_map(|c| colour(Some(c))).map(rgba).collect()).unwrap_or_default();
        let (skip, fade) = (num(l, "skip", 0.0) as f32, sc.get("fade").is_some());
        for i in 0..n {
            let (ox, oy) = (rng.next() * sw, rng.next() * sh);
            let color = (!palette.is_empty()).then(|| palette[(rng.next() * palette.len() as f32) as usize % palette.len()]);
            if rng.next() < skip {
                continue;
            }
            let (jw, jh) = stretch(&mut rng);
            let fade = if fade { 0.4 + 0.6 * rng.next() } else { 1.0 };
            out.push(Copy { ox, oy, color, fade, jw, jh, offset: start + stagger * i as f64 + rng.next() as f64 * random });
        }
        return out;
    }
    let (n, dx, dy) = l
        .get("repeat")
        .and_then(Value::as_object)
        .map(|r| ((num(r, "n", 1.0) as usize).clamp(1, 600), coord(r.get("dx"), region.w, 0.0), coord(r.get("dy"), region.h, 0.0)))
        .unwrap_or((1, 0.0, 0.0));
    for i in 0..n {
        let mut rng = Rng::new(i as u32 * 7919 + 13);
        let (jw, jh) = stretch(&mut rng);
        out.push(Copy { ox: i as f32 * dx, oy: i as f32 * dy, color: None, fade: 1.0, jw, jh, offset: start + stagger * i as f64 });
    }
    out
}

/// Where a moving copy is at time `t`: its offset (logical px) and how visible it is; None while it rests out of
/// sight. Breathing is an opacity, handled by the caller.
fn motion_at(m: &Map<String, Value>, t: f64) -> Option<(f32, f32, f32)> {
    let secs = num(m, "seconds", 12.0).max(2.0);
    let (x, y) = (num(m, "x", 0.0) as f32, num(m, "y", 0.0) as f32);
    match str_of(m, "kind") {
        Some("drift") => {
            let s = ((1.0 - (t / secs * std::f64::consts::TAU).cos()) / 2.0) as f32;
            Some((x * s, y * s, 1.0))
        }
        Some("scroll") => {
            let f = (t.rem_euclid(secs) / secs) as f32;
            Some((x * f, y * f, 1.0))
        }
        // Crosses once, fading in and out over the first and last 18%, with a walking bob, then rests out of sight.
        Some("pass") => {
            let period = secs + num(m, "pause", 0.0).max(0.0);
            let f = (t.rem_euclid(period) / secs) as f32;
            if f >= 1.0 {
                return None;
            }
            let edge = (f.min(1.0 - f) / 0.18).min(1.0);
            let a = num(m, "max", 1.0) as f32 * edge * edge * (3.0 - 2.0 * edge);
            let bob = num(m, "bob", 0.0) as f32 * (t as f32 * 1.8 * std::f32::consts::PI).sin().abs();
            let across = if m.contains_key("x") { x } else { 300.0 };
            Some((across * f, y * f - bob, a))
        }
        _ => Some((0.0, 0.0, 1.0)),
    }
}

#[derive(Clone, Copy, Default)]
struct Drop {
    x: f32,
    y: f32,
    r: f32,
    v: f32,
    wait: f32,
    slide: f32,
    trail_top: f32,
    trail: f32,
    alpha: f32,
    phase: f32,
    sprite: usize,
    gone: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Not moving: drawn as it is.
    Still,
    /// A breathe's low point (in the backdrop, under its motion).
    Low,
    /// A breathe's middle (the still scene, motion off).
    Middle,
    /// Moving, at time t.
    Moving,
}

/// Neon tubes' light on the glass round the panels ("neonEdges"); the tubes themselves are drawn by the UI.
struct Neon {
    content: Option<Color>,
    player: Option<Color>,
    reach: f32,
    strength: f32,
    radius: f32,
}

/// A theme's scene, ready to draw.
pub struct Scene {
    layers: Vec<Map<String, Value>>,
    neon: Option<Neon>,
    drips: Option<f32>,
    effects: Effects,
    back: Color,
    art: HashMap<String, Arc<RgbaImage>>,
    /// Pictures scaled (and premultiplied) for a size, by file and fit.
    scaled: HashMap<(String, u32, u32), Arc<RgbaImage>>,
    /// Drop sprites scaled (filtered, premultiplied) to each size a drop has been.
    drop_art: HashMap<(usize, u32, u32), Arc<RgbaImage>>,
    drops: Vec<Drop>,
    drops_size: (usize, usize),
    seed: u32,
    pub has_motion: bool,
    /// How often what moves is repainted, and at what share of the screen's resolution (rain and lights at half;
    /// the aurora's huge soft blobs at a quarter).
    pub motion_fps: f64,
    pub motion_scale: f32,
}

fn moving(l: &Map<String, Value>) -> bool {
    l.get("motion").is_some() || l.get("loopX").is_some() || l.get("loopY").is_some()
}

fn motion_kind(l: &Map<String, Value>) -> Option<&str> {
    l.get("motion").and_then(Value::as_object).and_then(|m| str_of(m, "kind"))
}

impl Scene {
    /// The scene for a theme, or None when it has nothing to draw.
    pub fn for_theme(theme: Option<&Theme>) -> Option<Scene> {
        let t = theme?;
        let layers: Vec<Map<String, Value>> = t
            .effects
            .get("scene")
            .and_then(|s| s.get("layers"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|l| l.as_object().cloned()).filter(|l| str_of(l, "type") != Some("text")).collect())
            .unwrap_or_default();
        let drips = t.effects.get("drips").map(|o| num(o, "density", 2.4) as f32);
        let neon = t.effects.get("neonEdges").map(|o| Neon {
            content: colour(o.get("content")),
            player: colour(o.get("player")),
            reach: num(o, "glow", 16.0) as f32,
            strength: num(o, "opacity", 0.8) as f32,
            radius: t.corners.card.unwrap_or(8.0) as f32,
        });
        let effects = Effects::for_theme(t);
        if layers.is_empty() && drips.is_none() && neon.is_none() && !effects.any_behind() && !effects.any_in_front() {
            return None;
        }
        let layer_motion = layers.iter().any(moving) || drips.is_some();
        Some(Scene {
            has_motion: layer_motion || effects.moves(),
            motion_fps: if layer_motion { 12.0 } else { 6.0 },
            motion_scale: if layer_motion { 0.5 } else { 0.25 },
            layers,
            neon,
            drips,
            effects,
            back: t.colors.get("background").copied().map(|c| Color { a: 255, ..c }).unwrap_or(Color { a: 255, r: 0, g: 0, b: 0 }),
            art: HashMap::new(),
            scaled: HashMap::new(),
            drop_art: HashMap::new(),
            drops: Vec::new(),
            drops_size: (0, 0),
            seed: 0x2545_F491,
        })
    }

    /// Whether anything lies behind the panels (then see-through panels show it).
    pub fn has_backdrop(&self) -> bool {
        !self.layers.is_empty() || self.drips.is_some() || self.neon.is_some() || self.effects.any_behind()
    }

    fn picture(&mut self, name: &str) -> Option<Arc<RgbaImage>> {
        if let Some(p) = self.art.get(name) {
            return Some(p.clone());
        }
        let path = art_dir()?.join(name);
        let img = Arc::new(image::open(&path).ok()?.to_rgba8());
        self.art.insert(name.to_string(), img.clone());
        Some(img)
    }

    /// `name` scaled to `w`×`h` (cover-fit: filled and cropped to the middle; else stretched), premultiplied.
    fn scaled(&mut self, name: &str, w: u32, h: u32, cover: bool) -> Option<Arc<RgbaImage>> {
        let (w, h) = (w.max(1), h.max(1));
        let id = format!("{name}{}", if cover { "#cover" } else { "" });
        if let Some(p) = self.scaled.get(&(id.clone(), w, h)) {
            return Some(p.clone());
        }
        let src = self.picture(name)?;
        let mut out = if cover {
            let k = (w as f32 / src.width() as f32).max(h as f32 / src.height() as f32);
            let (sw, sh) = (((src.width() as f32) * k).ceil() as u32, ((src.height() as f32) * k).ceil() as u32);
            let big = imageops::resize(&*src, sw.max(w), sh.max(h), imageops::FilterType::Triangle);
            imageops::crop_imm(&big, (big.width() - w) / 2, (big.height() - h) / 2, w, h).to_image()
        } else {
            imageops::resize(&*src, w, h, imageops::FilterType::Triangle)
        };
        premultiply(&mut out);
        // Only the newest size of each picture is kept.
        self.scaled.retain(|(n, _, _), _| n != &id);
        let out = Arc::new(out);
        self.scaled.insert((id, w, h), out.clone());
        Some(out)
    }

    /// Lets go of scaled pictures bigger than `max_px` pixels: the backdrop's full-window ones (tens of MB on a
    /// dense screen) are needed again only if the same size is painted again, which rescales them once.
    pub fn forget_large(&mut self, max_px: u64) {
        self.scaled.retain(|_, p| (p.width() as u64) * (p.height() as u64) <= max_px);
    }

    /// The still part, at `scale` px per logical px. With `still` (motion off), a moment of what would move too.
    pub fn backdrop(&mut self, g: &Geometry, scale: f32, still: bool) -> Picture {
        let (w, h) = ((g.width * scale).round() as usize, (g.height * scale).round() as usize);
        let b = self.back;
        let mut p = Picture::filled(w, h, [b.r, b.g, b.b]);
        self.effects.behind(&mut p, scale, still);
        for l in self.layers.clone().iter() {
            if !moving(l) {
                self.draw(&mut p, l, g, scale, 0.0, Mode::Still);
            } else if motion_kind(l) == Some("breathe") {
                // The breathe's low point lives here; the motion layer swells it.
                self.draw(&mut p, l, g, scale, 0.0, if still { Mode::Middle } else { Mode::Low });
            } else if still && motion_kind(l) != Some("pass") {
                self.draw(&mut p, l, g, scale, 0.0, Mode::Still);
            }
        }
        if still {
            if let Some(density) = self.drips {
                self.drips(&mut p, scale, density, 0.0);
            }
        }
        if let Some(n) = &self.neon {
            let lit = [(n.content, Some(g.content_card())), (n.player, g.player_card())];
            for (c, rect) in lit {
                if let (Some(c), Some(rect)) = (c, rect) {
                    neon_glow(&mut p, rect, n.radius, c, n.reach, n.strength, scale);
                }
            }
        }
        p
    }

    /// The moving part at time `t` (seconds), at `scale` px per logical px; `dt` advances the drops.
    pub fn motion(&mut self, g: &Geometry, scale: f32, t: f64, dt: f32) -> Picture {
        let (w, h) = ((g.width * scale).round() as usize, (g.height * scale).round() as usize);
        let mut p = Picture::new(w, h);
        self.effects.moving(&mut p, scale, t);
        for l in self.layers.clone().iter().filter(|l| moving(l)) {
            self.draw(&mut p, l, g, scale, t, Mode::Moving);
        }
        if let Some(density) = self.drips {
            self.drips(&mut p, scale, density, dt);
        }
        p
    }

    /// The Classic layout's skin: the theme's whole-window picture and effects (not the Full layout's title-bar and
    /// panel art), still, at `w`×`h` logical px. None when the theme has no picture of its own.
    pub fn skin(&mut self, w: f32, h: f32, scale: f32) -> Option<Picture> {
        let whole = |l: &Map<String, Value>| matches!(str_of(l, "in"), None | Some("window")) && str_of(l, "type") == Some("image");
        if !self.layers.iter().any(whole) && !self.effects.any_behind() && self.drips.is_none() && self.effects.vignette.is_none() {
            return None;
        }
        let g = Geometry { width: w, height: h, title: 0.0, pane: 0.0, player: 0.0, title_end: 0.0 };
        let b = self.back;
        let mut p = Picture::filled((w * scale).round() as usize, (h * scale).round() as usize, [b.r, b.g, b.b]);
        self.effects.behind(&mut p, scale, true);
        for l in self.layers.clone().iter().filter(|l| matches!(str_of(l, "in"), None | Some("window"))) {
            if motion_kind(l) == Some("pass") {
                continue;
            }
            let mode = if motion_kind(l) == Some("breathe") { Mode::Middle } else { Mode::Still };
            self.draw(&mut p, l, &g, scale, 0.0, mode);
        }
        if let Some(density) = self.drips {
            // A fresh, lived-in moment of the drops at this size.
            self.drops_size = (0, 0);
            self.drips(&mut p, scale, density, 0.0);
            self.drops_size = (0, 0);
        }
        self.effects.vignette_into(&mut p);
        Some(p)
    }

    /// What lies over everything (grain, vignette, scanlines), or None.
    pub fn front(&self, g: &Geometry, scale: f32) -> Option<Picture> {
        self.effects.front((g.width * scale).round() as usize, (g.height * scale).round() as usize, scale, true)
    }

    /// The Classic layout's overlay: grain and scanlines only (its skin holds the vignette).
    pub fn classic_front(&self, w: f32, h: f32, scale: f32) -> Option<Picture> {
        self.effects.front((w * scale).round() as usize, (h * scale).round() as usize, scale, false)
    }

    fn draw(&mut self, p: &mut Picture, l: &Map<String, Value>, g: &Geometry, scale: f32, t: f64, mode: Mode) {
        // Hidden layers wait for an easter egg.
        if l.get("hidden").and_then(Value::as_bool) == Some(true) {
            return;
        }
        let region = g.region(str_of(l, "in"));
        let mut opacity = num(l, "opacity", 1.0) as f32;
        let m = l.get("motion").and_then(Value::as_object);
        // Breathing: a slow swell of opacity, never under 4 s a cycle. The backdrop holds the low point; the motion
        // layer lays the same thing again on top, so together they reach the breathe's value.
        if let Some(m) = m.filter(|m| str_of(m, "kind") == Some("breathe")) {
            let secs = num(m, "seconds", 12.0).max(4.0);
            let offset = num(m, "start", 0.0) + num(m, "offset", 0.0);
            let (lo, hi) = (num(m, "min", 0.6) as f32, num(m, "max", 1.0) as f32);
            opacity *= match mode {
                Mode::Low | Mode::Still => lo,
                Mode::Middle => (lo + hi) / 2.0,
                Mode::Moving => {
                    let b = lo + (hi - lo) * (1.0 - ((t + offset) / secs * std::f64::consts::TAU).cos() as f32) / 2.0;
                    ((b - lo) / (1.0 - lo).max(0.01)).clamp(0.0, 1.0)
                }
            };
        }
        let kind = str_of(l, "type").unwrap_or("rect");
        if kind == "image" {
            self.draw_image(p, l, region, scale, t, opacity);
            return;
        }
        let fill = paint_of(l, "fill");
        for c in copies(l, region) {
            let (mut dx, mut dy, mut a) = (0.0, 0.0, opacity * c.fade);
            if let Some(m) = m.filter(|m| str_of(m, "kind") != Some("breathe")) {
                match motion_at(m, t + c.offset) {
                    Some((mx, my, ma)) => {
                        dx = mx;
                        dy = my;
                        a *= ma;
                    }
                    None => continue,
                }
            }
            let (w, h) = (coord(l.get("w"), region.w, 10.0) * c.jw, coord(l.get("h"), region.h, 10.0) * c.jh);
            let x = region.x + coord(l.get("x"), region.w, 0.0) + c.ox + dx;
            let mut y = region.y + coord(l.get("y"), region.h, 0.0) + c.oy + dy;
            if str_of(l, "anchor") == Some("bottom") {
                y -= h;
            }
            let b = Rect { x: x * scale, y: y * scale, w: w * scale, h: h * scale };
            let paint = c.color.map(Paint::Solid).or_else(|| fill.clone());
            match kind {
                "rect" | "ellipse" | "radial" => {
                    let Some(paint) = paint else { continue };
                    let shape = match kind {
                        "ellipse" => Shape::Ellipse,
                        "radial" => Shape::Radial,
                        _ => Shape::Rect(num(l, "radius", 0.0) as f32 * scale),
                    };
                    p.fill(b, shape, &paint, a);
                }
                "bevel" => {
                    let light = colour(l.get("light")).map(rgba).unwrap_or([1.0; 4]);
                    let dark = colour(l.get("dark")).map(rgba).unwrap_or([0.5, 0.5, 0.5, 1.0]);
                    let edge = (num(l, "edge", 2.0) as f32 * scale).round().max(1.0) as usize;
                    let outline = colour(l.get("outline")).map(rgba);
                    p.bevel(b, paint.map(|p| p.at(0.5, 0.5)), light, dark, edge, str_of(l, "style") == Some("sunken"), outline, a);
                }
                "line" => {
                    let (x2, y2) = (coord(l.get("x2"), region.w, 0.0), coord(l.get("y2"), region.h, 0.0));
                    let stroke = c.color.or_else(|| paint_of(l, "stroke").map(|p| p.at(0.5, 0.5))).unwrap_or([1.0; 4]);
                    p.line(b.x, b.y, b.x + x2 * scale, b.y + y2 * scale, num(l, "strokeWidth", 1.0) as f32 * scale, stroke, a);
                }
                "pixels" => p.pixels(b.x, b.y, &pixel_rows(l), num(l, "scale", 4.0) as f32 * scale, a),
                _ => {}
            }
        }
    }

    /// Rendered art: cover-fit to its box (the region, or `w`/`h`), or with `loopY`/`loopX` (seconds) a seamless
    /// 512 px tile slid along forever (rain sheets).
    fn draw_image(&mut self, p: &mut Picture, l: &Map<String, Value>, region: Rect, scale: f32, t: f64, opacity: f32) {
        let Some(src) = str_of(l, "src").map(String::from) else { return };
        let o = fixed(opacity);
        if o == 0 {
            return;
        }
        let (lx, ly) = (num(l, "loopX", 0.0), num(l, "loopY", 0.0));
        if lx > 0.0 || ly > 0.0 {
            let tile = ((num(l, "tile", 512.0) as f32) * scale).round().max(8.0) as usize;
            let Some(img) = self.scaled(&src, tile as u32, tile as u32, false) else { return };
            let ts = tile as f32;
            let oy = if ly > 0.0 { ((t % ly) / ly) as f32 * ts } else { 0.0 };
            let ox = if lx > 0.0 { ((t % lx) / lx) as f32 * ts } else { 0.0 };
            let (oy, ox) = (oy as usize % tile, ox as usize % tile);
            let raw = img.as_raw();
            // Pixels too faint to change anything at this opacity are skipped.
            let least = (3 * 256 / o.max(1)).min(255) as u8;
            let (x0, y0) = ((region.x * scale).max(0.0) as usize, (region.y * scale).max(0.0) as usize);
            let (x1, y1) = ((((region.x + region.w) * scale) as usize).min(p.w), (((region.y + region.h) * scale) as usize).min(p.h));
            for y in y0..y1 {
                let row = &raw[((y + tile - oy) % tile) * tile * 4..][..tile * 4];
                let mut tx = (x0 + ox) % tile;
                let base = y * p.w;
                for x in x0..x1 {
                    let s = &row[tx * 4..tx * 4 + 4];
                    if s[3] >= least {
                        p.over_pre(base + x, s, o);
                    }
                    tx += 1;
                    if tx == tile {
                        tx = 0;
                    }
                }
            }
            return;
        }
        let w = l.get("w").map(|v| coord(Some(v), region.w, region.w)).unwrap_or(region.w);
        let h = l.get("h").map(|v| coord(Some(v), region.h, region.h)).unwrap_or(region.h);
        let (x, y) = (region.x + coord(l.get("x"), region.w, 0.0), region.y + coord(l.get("y"), region.h, 0.0));
        let (iw, ih) = ((w * scale).round().max(1.0) as u32, (h * scale).round().max(1.0) as u32);
        let Some(img) = self.scaled(&src, iw, ih, str_of(l, "fit") != Some("fill")) else { return };
        let raw = img.as_raw();
        let (x0, y0) = ((x * scale).round() as i64, (y * scale).round() as i64);
        for yy in 0..ih as i64 {
            let py = y0 + yy;
            if py < 0 || py >= p.h as i64 {
                continue;
            }
            for xx in 0..iw as i64 {
                let px = x0 + xx;
                if px < 0 || px >= p.w as i64 {
                    continue;
                }
                let si = ((yy as u32 * iw + xx as u32) * 4) as usize;
                p.over_pre(py as usize * p.w + px as usize, &raw[si..si + 4], o);
            }
        }
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn spawn(&mut self, w: f32, h: f32, anywhere: bool) -> Drop {
        // Mostly small drops, the odd big one.
        let r = 4.0 + self.rand().powf(2.2) * 9.0;
        let y = if anywhere { self.rand() * h } else { self.rand() * h * 0.45 };
        Drop {
            x: self.rand() * w,
            y,
            r,
            wait: 0.5 + self.rand() * 6.0,
            trail_top: y,
            alpha: if anywhere { 1.0 } else { 0.0 },
            phase: self.rand() * 100.0,
            sprite: (self.rand() * 6.0) as usize % 6,
            ..Drop::default()
        }
    }

    /// Rain on the glass: drops cling, then run down in fits and starts, leaving trails that dry slowly.
    fn drips(&mut self, p: &mut Picture, scale: f32, density: f32, dt: f32) {
        let (w, h) = (p.w as f32 / scale, p.h as f32 / scale);
        if self.drops_size != (p.w, p.h) {
            let count = ((w * h / 100_000.0 * density) as usize).clamp(4, 70);
            self.drops = (0..count).map(|_| self.spawn(w, h, true)).collect();
            self.drops_size = (p.w, p.h);
            // Settle into a lived-in moment: some running, some trails drying.
            for _ in 0..1500 {
                self.step_drops(0.04, w, h);
            }
        }
        if dt > 0.0 {
            self.step_drops(dt.min(0.25), w, h);
        }
        let drops = self.drops.clone();
        for d in &drops {
            // The wet trail: a faint streak with brighter edges, fading towards its top.
            let len = (d.y - d.trail_top).max(0.0);
            if d.trail > 0.01 && len > 1.0 {
                let tw = (d.r * 0.5 * scale).max(1.0);
                let cx = d.x * scale;
                let (x0, x1) = ((cx - tw).max(0.0) as usize, ((cx + tw).min(p.w as f32 - 1.0)).max(0.0) as usize);
                let (y0, y1) = ((d.trail_top * scale).max(0.0) as usize, ((d.y * scale).min(p.h as f32 - 1.0)).max(0.0) as usize);
                for y in y0..y1 {
                    let fade = ((y - y0) as f32 / (len * scale).max(1.0) * 1.4).min(1.0);
                    for x in x0..=x1 {
                        let u = ((x as f32 + 0.5 - cx) / tw).abs();
                        if u > 1.0 {
                            continue;
                        }
                        let a = ((1.0 - u).powi(2) * 0.08 + (-(u - 0.8).powi(2) / 0.01).exp() * 0.16) * fade * d.trail;
                        p.blend(y * p.w + x, [0.85, 0.9, 1.0], a);
                    }
                }
            }
            if d.gone {
                continue;
            }
            let (sw, sh) = (((d.r * 2.0 + 3.0) * scale).round().max(2.0) as u32, ((d.r * 2.0 + 3.0) * 1.18 * scale).round().max(2.0) as u32);
            let Some(sprite) = self.drop_sprite(d.sprite, sw, sh) else { continue };
            let (x0, y0) = ((d.x * scale - sw as f32 / 2.0).round() as i64, (d.y * scale - sh as f32 / 2.0).round() as i64);
            let raw = sprite.as_raw();
            for yy in 0..sh as i64 {
                let y = y0 + yy;
                if y < 0 || y >= p.h as i64 {
                    continue;
                }
                for xx in 0..sw as i64 {
                    let x = x0 + xx;
                    if x < 0 || x >= p.w as i64 {
                        continue;
                    }
                    let si = ((yy as u32 * sw + xx as u32) * 4) as usize;
                    p.over_pre(y as usize * p.w + x as usize, &raw[si..si + 4], fixed(d.alpha));
                }
            }
        }
    }

    /// Drop sprite `i` at `w`×`h`, filtered down from its art (cached: drops only come in a few dozen sizes).
    fn drop_sprite(&mut self, i: usize, w: u32, h: u32) -> Option<Arc<RgbaImage>> {
        if let Some(s) = self.drop_art.get(&(i, w, h)) {
            return Some(s.clone());
        }
        let art = self.picture(&format!("drop-{i}.png"))?;
        let mut s = imageops::resize(&*art, w, h, imageops::FilterType::CatmullRom);
        premultiply(&mut s);
        if self.drop_art.len() > 400 {
            self.drop_art.clear();
        }
        let s = Arc::new(s);
        self.drop_art.insert((i, w, h), s.clone());
        Some(s)
    }

    fn step_drops(&mut self, dt: f32, w: f32, h: f32) {
        for i in 0..self.drops.len() {
            let mut d = self.drops[i];
            if d.gone {
                d.trail -= dt * 0.08;
                self.drops[i] = if d.trail <= 0.0 { self.spawn(w, h, false) } else { d };
                continue;
            }
            d.alpha = (d.alpha + dt * 0.6).min(1.0);
            if d.wait > 0.0 {
                d.wait -= dt;
                d.trail = (d.trail - dt * 0.035).max(0.0);
                if d.trail <= 0.0 {
                    d.trail_top = d.y;
                }
                self.drops[i] = d;
                continue;
            }
            if d.slide <= 0.0 {
                d.slide = (25.0 + self.rand() * 240.0) * (d.r / 8.0);
            }
            let vmax = 30.0 + d.r * 16.0;
            d.v = (d.v + vmax * 2.5 * dt).min(vmax);
            let dy = d.v * dt;
            d.y += dy;
            d.slide -= dy;
            d.x += (d.y / 27.0 + d.phase).sin() * 0.18;
            d.trail = (d.trail + dt * 1.5).min(0.9);
            if d.slide <= 0.0 {
                d.v = 0.0;
                d.wait = 0.3 + self.rand() * 4.5;
                d.r = (d.r * 0.96).max(3.5);
            }
            if d.y > h + 40.0 {
                d.gone = true;
            }
            self.drops[i] = d;
        }
    }
}

/// The light round a neon tube on a rounded rectangle's edge: falling away outward like light does, and a little
/// inward from the 2 px tube (the light on the glass).
fn neon_glow(p: &mut Picture, r: Rect, radius: f32, c: Color, reach: f32, strength: f32, scale: f32) {
    let col = [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0];
    let (hw, hh) = (r.w / 2.0, r.h / 2.0);
    let (cx, cy) = (r.x + hw, r.y + hh);
    let rad = radius.min(hw).min(hh).max(0.0);
    let spread = reach * 0.55;
    let (tail, far) = (reach * 1.2, reach * 2.6);
    let (x0, x1) = (((r.x - far) * scale).max(0.0) as usize, (((r.x + r.w + far) * scale) as usize).min(p.w));
    let (y0, y1) = (((r.y - far) * scale).max(0.0) as usize, (((r.y + r.h + far) * scale) as usize).min(p.h));
    for py in y0..y1 {
        let y = (py as f32 + 0.5) / scale;
        for px in x0..x1 {
            let x = (px as f32 + 0.5) / scale;
            // Signed distance to the rounded rectangle's edge (outside positive), in logical px.
            let (qx, qy) = ((x - cx).abs() - (hw - rad), (y - cy).abs() - (hh - rad));
            let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad;
            let a = if d >= 0.0 {
                if d > far {
                    continue;
                }
                0.62 * (-(d / spread).powi(2)).exp() + 0.16 * (-(d / tail).powi(2)).exp()
            } else {
                let inside = -d - 2.0;
                if !(0.0..=5.0).contains(&inside) {
                    continue;
                }
                0.3 * (-(inside / spread).powi(2)).exp()
            };
            p.blend(py * p.w + px, col, a * strength);
        }
    }
}

/// The least opacity a see-through `surface` needs over the picture in `rect` (px) for `text` to read at `target`
/// contrast over nearly all of it (the brightest or darkest 1% may fall short). `surface`'s own alpha when it
/// already reads.
pub fn readable_alpha(back: &Picture, rect: Rect, surface: Color, text: Color, target: f64) -> u8 {
    use crate::palette::contrast;
    let (x0, y0) = (rect.x.max(0.0) as usize, rect.y.max(0.0) as usize);
    let (x1, y1) = (((rect.x + rect.w) as usize).min(back.w), ((rect.y + rect.h) as usize).min(back.h));
    let mut samples: Vec<Color> = Vec::new();
    for y in (y0..y1).step_by(4) {
        for x in (x0..x1).step_by(4) {
            let i = (y * back.w + x) * 4;
            samples.push(Color { a: 255, r: back.px[i], g: back.px[i + 1], b: back.px[i + 2] });
        }
    }
    // Thickening only helps text that contrasts with the panel itself (light glass under white text never would).
    if samples.is_empty() || contrast(text, Color { a: 255, ..surface }) < target {
        return surface.a;
    }
    let mut alpha = surface.a as u32;
    loop {
        let a = alpha as f64 / 255.0;
        let failing = samples.iter().filter(|&&under| contrast(text, Color { a: 255, ..under.mix(Color { a: 255, ..surface }, a) }) < target).count();
        if failing * 100 <= samples.len() || alpha >= 255 {
            return alpha.min(255) as u8;
        }
        alpha = (alpha + 8).min(255);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use driftwave_core::flash;
    use driftwave_core::theme::load_folder;
    use driftwave_vis::guard::measure;
    use std::path::PathBuf;

    fn themes() -> Vec<Theme> {
        load_folder(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../themes"), true)
    }

    /// Every moving scene, watched for 24 s as the window shows it (backdrop and motion together), judged over the
    /// whole window and over regions about WCAG's small-area size: no flashing, anywhere.
    #[test]
    fn no_moving_scene_ever_flashes() {
        let fps = 12.0;
        let scale = 0.4;
        let g = Geometry { width: 1180.0, height: 760.0, title: 48.0, pane: 220.0, player: 312.0, title_end: 110.0 };
        let mut moving = 0;
        for t in themes() {
            let Some(mut scene) = Scene::for_theme(Some(&t)) else { continue };
            if !scene.has_motion {
                continue;
            }
            moving += 1;
            let back = scene.backdrop(&g, scale, false);
            let (gx, gy) = (4, 3);
            let mut whole = Vec::new();
            let mut cells: Vec<Vec<flash::Frame>> = vec![Vec::new(); gx * gy];
            for f in 0..(24.0 * fps) as usize {
                let m = scene.motion(&g, scale, f as f64 / fps, (1.0 / fps) as f32);
                assert_eq!((m.w, m.h), (back.w, back.h));
                let mut seen = Picture { w: back.w, h: back.h, px: back.px.clone() };
                for i in 0..m.w * m.h {
                    seen.over_pre(i, &m.px[i * 4..i * 4 + 4], 256);
                }
                let (all, each) = measure(&seen.px, seen.w, seen.h, gx, gy, 1.0 / fps);
                whole.push(all);
                for (c, e) in cells.iter_mut().zip(each) {
                    c.push(e);
                }
            }
            assert!(!flash::is_flashing(&whole, true), "{}: the window flashes", t.name);
            for (i, c) in cells.iter().enumerate() {
                assert!(!flash::is_flashing(c, true), "{}: region {i} flashes", t.name);
            }
        }
        assert!(moving >= 1, "Night Market at least moves");
    }

    /// A see-through panel is made just opaque enough for its text over a bright picture, and left alone over a
    /// dark one.
    #[test]
    fn panels_thicken_only_where_the_picture_needs_it() {
        let white = Color { a: 255, r: 255, g: 255, b: 255 };
        let panel = Color { a: 100, r: 8, g: 8, b: 16 };
        let mut bright = Picture::new(64, 64);
        bright.px.fill(235);
        let mut dark = Picture::new(64, 64);
        for p in dark.px.chunks_exact_mut(4) {
            p.copy_from_slice(&[10, 10, 20, 255]);
        }
        let all = Rect { x: 0.0, y: 0.0, w: 64.0, h: 64.0 };
        let thick = readable_alpha(&bright, all, panel, white, 4.5);
        assert!(thick > 100);
        let seen = Color { a: 255, r: 235, g: 235, b: 235 }.mix(Color { a: 255, ..panel }, thick as f64 / 255.0);
        assert!(crate::palette::contrast(white, seen) >= 4.5);
        assert_eq!(readable_alpha(&dark, all, panel, white, 4.5), 100);
    }
}
