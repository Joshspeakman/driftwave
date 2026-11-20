//! A theme's look in the Full layout beyond its colours (ported from the C# app's ThemeManager, TitleDecor and
//! WindowEffects):
//! - the window's title and page titles: font, case, letter spacing, a neon glow or a hard shadow, a block cursor;
//! - neon tubes round the panels, a thick screen frame, the theme's signs (with the clock filled in);
//! - its scene (scene.rs) behind see-through panels, made just opaque enough for their text over the picture;
//! - what lies over everything (grain, vignette, scanlines), a scanline band rolling slowly down, synthwave's grid
//!   rolling towards you, and Stealth's spotlight following the pointer.
//!
//! The scene's backdrop and the overlay are painted once per window size (after a resize settles). What moves is
//! painted only in the Full layout, only while the window is in front; otherwise the last moment simply stays.

use super::{log, App, MainWindow, SceneText, Scenery, Th};
use crate::fullpal::FullPalette;
use crate::scene::{self, Geometry, Scene};
use driftwave_core::theme::{Color, Theme};
use serde_json::{Map, Value};
use driftwave_platform::layers::SceneLayers;
use slint::winit_030::{winit, WinitWindowAccessor};
use slint::{ComponentHandle, Image, ModelRc, Rgb8Pixel, Rgba8Pixel, SharedPixelBuffer, VecModel};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// A resize must rest this long before the backdrop is repainted at the new size (until then it's stretched).
const SETTLE: Duration = Duration::from_millis(160);
/// The most of one core the whole app may use while a scene moves (measured over a few seconds, the music
/// included). Above it, on a slower computer, the scene holds still instead. This PC: about 10%.
const MOTION_BUDGET: f64 = 0.16;
/// The Full layout's title bar and panes (full.slint).
const TITLE: f32 = 48.0;
const PANE: f32 = 220.0;
const PLAYER: f32 = 312.0;

/// See-through panels: each one's own colour and the text that has to read on it.
#[derive(Clone, Copy)]
struct Panels {
    surface: Color,
    card: Color,
    pane: Color,
    control: Color,
    text: Color,
    card_text: Color,
    pane_text: Color,
    control_text: Color,
}

/// Watches what the app costs while a scene moves.
#[derive(Default)]
struct Valve {
    since: Option<(Instant, Duration)>,
    tripped: bool,
}

/// This process's CPU time so far (all threads).
fn process_cpu() -> Option<Duration> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::FILETIME;
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
        let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
        let (mut c, mut e, mut k, mut u) = (zero, zero, zero, zero);
        // SAFETY: GetCurrentProcess is a pseudo-handle; the four FILETIMEs are ours to fill.
        let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
        if ok == 0 {
            return None;
        }
        let ticks = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
        Some(Duration::from_nanos((ticks(k) + ticks(u)) * 100))
    }
    #[cfg(unix)]
    {
        // SAFETY: getrusage fills the struct we hand it.
        let mut r: libc::rusage = unsafe { std::mem::zeroed() };
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut r) } != 0 {
            return None;
        }
        let t = |v: libc::timeval| Duration::from_secs(v.tv_sec as u64) + Duration::from_micros(v.tv_usec as u64);
        Some(t(r.ru_utime) + t(r.ru_stime))
    }
    #[cfg(not(any(windows, unix)))]
    None
}

#[derive(Default)]
pub struct Look {
    scene: Option<Scene>,
    panels: Option<Panels>,
    /// The size (px), stillness, player and title's end the backdrop was painted for.
    painted: Option<Key>,
    pending: Option<(Key, Instant)>,
    last_motion: Option<Instant>,
    started: Option<Instant>,
    /// Off: the scene holds still (a moment of its rain and drops), costing nothing.
    pub still: bool,
    valve: Valve,
    /// The theme's signs as written (with {time} and the like), and whether any shows the clock.
    texts: Vec<SceneText>,
    clock: bool,
    minute: i64,
    /// The UI's moving parts: synthwave's rows, the scanline band, the spotlight.
    synth: bool,
    band: bool,
    spot: bool,
    /// Where the pointer is over the window (logical px), and where the spotlight has got to.
    pub pointer: Option<(f32, f32)>,
    spot_at: (f32, f32),
    /// The Classic skin's painted size (px) and theme, and a size waiting to settle.
    skin_painted: Option<(u32, u32)>,
    skin_pending: Option<((u32, u32), Instant)>,
    /// How see-through the theme wants Classic's frames (palette.rs), before the picture's say.
    pub classic_see: f32,
    /// The theme's scene layers as written (for placing the eggs' clickable items).
    layers: Vec<Map<String, Value>>,
    /// The last motion frames' cost, for DRIFTWAVE_LOG.
    motion_ms: f64,
    motion_frames: u32,
    /// macOS: the scene's backdrop and motion as layers beneath the UI (see `driftwave_platform::layers`),
    /// once made; whether making them is settled; whether the UI is drawn see-through over them now;
    /// and a hide left for the next tick (after the UI has gone opaque again, so nothing shows through).
    native_layers: Option<SceneLayers>,
    layers_settled: bool,
    native_on: bool,
    hide_layers: bool,
    /// What the rows and band layers last showed (phase and size; the band's top), to skip repeats.
    native_rows: Option<(u32, u32, u32)>,
    native_band: Option<i32>,
}

type Key = (u32, u32, bool, bool, i32);

/// Synthwave's 16 rows as the Slint ones draw them (full.slint, "Synthwave's rows"), for the native rows
/// layer: one pixel wide (each row is one colour across the window), from the horizon down at `scale` px a
/// point. Returns the premultiplied pixels, their height, and the strip's top and height in points.
fn synth_rows(lh: f32, scale: f32, phase: f32, grid: slint::Color) -> (Vec<u8>, usize, f32, f32) {
    let horizon = lh * 0.68;
    let y0 = horizon.floor();
    let hpt = (lh - y0).max(1.0);
    let h = (hpt * scale).ceil() as usize;
    let mut px = vec![0f32; h * 4];
    let (r, g, b, ca) = (grid.red() as f32 / 255.0, grid.green() as f32 / 255.0, grid.blue() as f32 / 255.0, grid.alpha() as f32 / 255.0);
    for i in 0..16 {
        let depth = i as f32 + 1.0 - phase;
        let at = horizon + (lh - horizon) / (1.0 + 0.55 * depth);
        let a = ((at - horizon) / (lh - horizon) * 3.0).clamp(0.0, 1.0) * ((16.0 - depth) / 2.0).clamp(0.0, 1.0) * 0.7;
        let top = at.floor();
        let frac = at - top;
        // Split across two point rows by its fraction, so it glides.
        for (row, alpha) in [(top, a * (1.0 - frac * 0.5)), (top + 1.0, a * (0.5 + frac * 0.5))] {
            let src = ca * alpha;
            let from = (((row - y0) * scale).round().max(0.0) as usize).min(h);
            let to = (((row + 1.0 - y0) * scale).round().max(0.0) as usize).min(h);
            for p in px[from * 4..to * 4].as_chunks_mut::<4>().0 {
                p[0] = r * src + p[0] * (1.0 - src);
                p[1] = g * src + p[1] * (1.0 - src);
                p[2] = b * src + p[2] * (1.0 - src);
                p[3] = src + p[3] * (1.0 - src);
            }
        }
    }
    (px.iter().map(|v| (v * 255.0).round() as u8).collect(), h, y0, hpt)
}

fn slint_colour(c: Color) -> slint::Color {
    slint::Color::from_argb_u8(c.a, c.r, c.g, c.b)
}

fn from_slint(c: slint::Color) -> Color {
    Color { a: c.alpha(), r: c.red(), g: c.green(), b: c.blue() }
}

fn num(o: &Map<String, Value>, k: &str) -> Option<f64> {
    o.get(k).and_then(Value::as_f64)
}

fn colour(o: &Map<String, Value>, k: &str) -> Option<Color> {
    o.get(k).and_then(Value::as_str).and_then(Color::parse)
}

fn weight(w: Option<&str>) -> i32 {
    match w.map(str::to_lowercase).as_deref() {
        Some("bold") => 700,
        Some("black") | Some("heavy") => 900,
        Some("semibold") => 600,
        Some("light") => 300,
        _ => 400,
    }
}

/// A coordinate as a share of its region plus px (a negative number of px is from the far edge).
fn place(v: Option<&Value>) -> (f32, f32) {
    match v {
        Some(Value::String(s)) if s.ends_with('%') => (s.trim_end_matches('%').parse::<f32>().unwrap_or(0.0) / 100.0, 0.0),
        Some(Value::Number(n)) => {
            let n = n.as_f64().unwrap_or(0.0) as f32;
            if n < 0.0 {
                (1.0, n)
            } else {
                (0.0, n)
            }
        }
        _ => (0.0, 0.0),
    }
}

/// A sign's text with the clock filled in: {time} (12-hour), {time24}, {date}.
fn with_clock(text: &str) -> String {
    if !text.contains('{') {
        return text.to_string();
    }
    let now = chrono::Local::now();
    text.replace("{time24}", &now.format("%H:%M").to_string())
        .replace("{time}", &now.format("%-I:%M %p").to_string())
        .replace("{date}", &now.format("%a %-d %b").to_string())
}

impl App {
    /// The theme's treatments and scene (after its colours are set).
    pub(crate) fn apply_look(&mut self, theme: Option<&Theme>, f: &FullPalette, ui_font: &str) {
        let Some(ui) = self.ui.upgrade() else { return };
        let th = ui.global::<Th>();
        let fx = |k: &str| theme.and_then(|t| t.effects.get(k));

        // The window's own title.
        let wt = fx("windowTitle");
        th.set_window_title_font(wt.and_then(|o| o.get("font")).and_then(Value::as_str).unwrap_or(ui_font).into());
        th.set_window_title_size(wt.and_then(|o| num(o, "size")).unwrap_or(12.0) as f32);
        th.set_window_title_weight(weight(wt.and_then(|o| o.get("weight")).and_then(Value::as_str)));
        th.set_window_title_color(slint_colour(wt.and_then(|o| colour(o, "color")).unwrap_or(f.title_text)));
        th.set_window_title_upper(wt.and_then(|o| o.get("uppercase")).and_then(Value::as_bool) == Some(true));

        // Page titles: case, letter spacing (thousandths of an em), a glow or a hard shadow, a block cursor.
        let size = theme.and_then(|t| t.fonts.title_size).unwrap_or(28.0);
        th.set_title_upper(theme.is_some_and(|t| t.fonts.title_uppercase));
        th.set_title_spacing((theme.and_then(|t| t.fonts.title_spacing).unwrap_or(0) as f64 / 1000.0 * size) as f32);
        let shadow = fx("titleShadow");
        let blur = shadow.and_then(|o| num(o, "blur")).unwrap_or(12.0);
        let shadow_colour = shadow.and_then(|o| colour(o, "color")).unwrap_or(f.accent);
        let shadow_colour = Color { a: (shadow_colour.a as f64 * shadow.and_then(|o| num(o, "opacity")).unwrap_or(1.0)).round() as u8, ..shadow_colour };
        th.set_title_glows(shadow.is_some() && blur > 0.0);
        th.set_title_glow(slint_colour(shadow_colour));
        th.set_title_glow_reach(blur.max(1.0) as f32);
        th.set_title_hard(shadow.is_some() && blur <= 0.0);
        th.set_title_hard_x(shadow.and_then(|o| num(o, "offsetX")).unwrap_or(3.0) as f32);
        th.set_title_hard_y(shadow.and_then(|o| num(o, "offsetY")).unwrap_or(3.0) as f32);
        let cursor = fx("cursor");
        th.set_title_cursor(cursor.is_some());
        th.set_title_cursor_color(slint_colour(cursor.and_then(|o| colour(o, "color")).unwrap_or(f.text)));

        // Window chrome of an era: caption buttons (win95, win31, xp, mac, platinum), bevels, selection, a centred title.
        let chrome = fx("chrome");
        let captions = chrome.and_then(|o| o.get("captions")).and_then(Value::as_str).unwrap_or("fluent");
        th.set_captions(captions.into());
        th.set_title_center(chrome.and_then(|o| o.get("center")).and_then(Value::as_bool) == Some(true));
        th.set_title_plate(slint_colour(chrome.and_then(|o| colour(o, "plate")).unwrap_or(Color { a: 0, r: 0, g: 0, b: 0 })));
        th.set_bevel(chrome.and_then(|o| o.get("bevel")).and_then(Value::as_bool) == Some(true));
        let bc = |k: &str, d: Color| slint_colour(chrome.and_then(|o| colour(o, k)).unwrap_or(d));
        th.set_bevel_light(bc("bevelLight", Color { a: 255, r: 255, g: 255, b: 255 }));
        th.set_bevel_hi(bc("bevelHi", Color { a: 255, r: 0xDF, g: 0xDF, b: 0xDF }));
        th.set_bevel_shadow(bc("bevelShadow", Color { a: 255, r: 0x80, g: 0x80, b: 0x80 }));
        th.set_bevel_dark(bc("bevelDark", Color { a: 255, r: 0, g: 0, b: 0 }));
        th.set_selection(bc("selection", Color { a: 0, r: 0, g: 0, b: 0 }));
        th.set_selection_text(bc("selectionText", Color { a: 255, r: 255, g: 255, b: 255 }));
        th.set_hard_shadow(bc("hardShadow", Color { a: 0, r: 0, g: 0, b: 0 }));

        // Neon tubes, and a thick screen frame.
        let neon = fx("neonEdges");
        th.set_neon_content_on(neon.is_some_and(|o| o.get("content").is_some()));
        th.set_neon_content(slint_colour(neon.and_then(|o| colour(o, "content")).unwrap_or(f.accent)));
        th.set_neon_player_on(neon.is_some_and(|o| o.get("player").is_some()));
        th.set_neon_player(slint_colour(neon.and_then(|o| colour(o, "player")).unwrap_or(f.accent)));
        th.set_neon_reach(neon.and_then(|o| num(o, "glow")).unwrap_or(16.0) as f32);
        th.set_neon_opacity(neon.and_then(|o| num(o, "opacity")).unwrap_or(0.8) as f32);
        let frame = fx("frame");
        th.set_frame_on(frame.is_some());
        th.set_frame_color(slint_colour(frame.and_then(|o| colour(o, "color")).unwrap_or(f.accent)));
        th.set_frame_thickness(frame.and_then(|o| num(o, "thickness")).unwrap_or(4.0) as f32);

        // The scene's text signs (the easter eggs' hidden ones are the eggs' to show).
        let layers: Vec<Map<String, Value>> = fx("scene")
            .and_then(|s| s.get("layers"))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|l| l.as_object().cloned()).collect())
            .unwrap_or_default();
        let texts: Vec<SceneText> = layers
            .iter()
            .filter(|l| l.get("type").and_then(Value::as_str) == Some("text"))
            .map(|l| {
                let (xf, x) = place(l.get("x"));
                let (yf, y) = place(l.get("y"));
                let ink = colour(l, "color").unwrap_or(f.title_text);
                SceneText {
                    text: l.get("text").and_then(Value::as_str).unwrap_or("").into(),
                    region: l.get("in").and_then(Value::as_str).unwrap_or("window").into(),
                    xf,
                    x,
                    yf,
                    // The C# app placed text by its top, and WinUI's line box sat a little lower than Slint's.
                    y: y + 1.0,
                    size: num(l, "size").unwrap_or(14.0) as f32,
                    ink: slint_colour(ink),
                    family: l.get("font").and_then(Value::as_str).unwrap_or(ui_font).into(),
                    weight: weight(l.get("weight").and_then(Value::as_str)),
                    italic: l.get("style").and_then(Value::as_str) == Some("italic"),
                    glows: l.get("glow").is_some(),
                    glow: slint_colour(colour(l, "glow").unwrap_or(ink)),
                    reach: num(l, "glowRadius").map(|r| r * 3.0).unwrap_or(6.0) as f32,
                    id: l.get("id").and_then(Value::as_str).unwrap_or("").into(),
                    opacity: if l.get("hidden").and_then(Value::as_bool) == Some(true) { 0.0 } else { 1.0 },
                }
            })
            .collect();
        let scenery = ui.global::<Scenery>();
        self.look.clock = texts.iter().any(|t| t.text.contains('{'));
        let shown: Vec<SceneText> = texts.iter().map(|t| SceneText { text: with_clock(&t.text).into(), ..t.clone() }).collect();
        self.look.texts = texts;
        self.look.minute = chrono::Local::now().timestamp() / 60;
        scenery.set_texts(ModelRc::from(Rc::new(VecModel::from(shown))));

        // Placeholder tiles: the theme's own colours, or tones between its accents (deep under a dark theme, soft
        // under a light one); Driftwave's bright ones for Default. Their antenna picks its own ink unless the theme
        // gives one, and a theme can outline them.
        let hue = |c: Color| crate::covers::hue_of(c.r, c.g, c.b);
        let tiles = fx("tiles");
        crate::covers::set_mood(theme.map(|t| crate::covers::Mood {
            from: hue(f.accent),
            to: hue(f.accent_text),
            light: t.base == "light",
            palette: tiles
                .and_then(|o| o.get("colors"))
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|c| c.as_str().and_then(Color::parse)).map(|c| [c.r, c.g, c.b]).collect())
                .unwrap_or_default(),
            flat: tiles.and_then(|o| o.get("flat")).and_then(Value::as_bool) == Some(true),
        }));
        th.set_tile_ink(slint_colour(tiles.and_then(|o| colour(o, "ink")).unwrap_or(Color { a: 0, r: 0, g: 0, b: 0 })));
        th.set_tile_border(slint_colour(tiles.and_then(|o| colour(o, "border")).unwrap_or(Color { a: 0, r: 0, g: 0, b: 0 })));

        // The UI's moving parts.
        let synth = fx("synthwave");
        self.look.synth = synth.is_some();
        scenery.set_synth_on(self.look.synth);
        scenery.set_synth_grid(slint_colour(synth.and_then(|o| colour(o, "grid")).unwrap_or(Color { a: 255, r: 255, g: 46, b: 151 })));
        self.look.band = fx("scanlines").is_some();
        scenery.set_band_on(self.look.band);
        scenery.set_band_y(-400.0);
        let spot = fx("spotlight");
        self.look.spot = spot.is_some();
        scenery.set_spot_on(self.look.spot);
        scenery.set_spot_radius(spot.and_then(|o| num(o, "radius")).unwrap_or(360.0) as f32);
        scenery.set_spot_opacity(spot.and_then(|o| num(o, "opacity")).unwrap_or(0.6) as f32);

        // The scene, and the panels that let it show through: a theme's see-through colours are see-through
        // whenever there's something behind them.
        let user = self.bg.as_ref().map(|p| p.frames[0].clone());
        self.look.scene = match user {
            Some(img) => Some(Scene::with_user(theme, img, self.bg_dim(), f.background)),
            None => Scene::for_theme(theme),
        };
        let behind = self.look.scene.as_ref().is_some_and(Scene::has_backdrop);
        let user = self.bg.is_some();
        self.look.panels = behind.then(|| {
            let own = |k: &str, solid: Color| theme.and_then(|t| t.colors.get(k).copied()).unwrap_or(solid);
            // Your background shows through every theme: smoky glass in the theme's colours (as the C# app did).
            let glass = |c: Color, a: f64| if user { Color { a: (a * 255.0) as u8, ..c } } else { c };
            Panels {
                surface: glass(own("surface", f.surface), 0.72),
                card: glass(own("card", f.card), 0.72),
                pane: glass(own("pane", f.pane), 0.62),
                control: glass(own("control", f.control), 0.8),
                text: f.text2,
                card_text: f.card_text2,
                pane_text: f.pane_text,
                control_text: f.control_text,
            }
        });
        scenery.set_on(self.look.scene.is_some());
        scenery.set_moving(false);
        scenery.set_front_on(false);
        // The Classic layout: neon on the panels' edges; its skin is painted on its next tick.
        let pal = ui.global::<super::Pal>();
        pal.set_edge_on(neon.is_some());
        pal.set_edge(slint_colour(neon.and_then(|o| colour(o, "player").or_else(|| colour(o, "content"))).unwrap_or(f.accent)));
        pal.set_skin_on(false);
        pal.set_skin_front_on(false);
        self.look.skin_painted = None;
        self.look.skin_pending = None;
        self.look.painted = None;
        self.look.pending = None;
        self.look.last_motion = None;
        self.look.started.get_or_insert_with(Instant::now);
        self.look.valve = Valve::default();
        self.apply_decor(theme);
        self.apply_eggs(theme, f.text, f.surface);
        self.look.layers = layers;
        self.refresh_list();
        self.refresh_found();
        self.show_now_playing_cover();
    }

    /// Moving or still, as the person chose in Settings (saved).
    pub(crate) fn choose_motion(&mut self, on: bool) {
        self.look.still = !on;
        self.look.valve = Valve::default();
        self.look.painted = None;
        self.settings["sceneMotion"] = serde_json::json!(on);
        if let Some(ui) = self.ui.upgrade() {
            ui.global::<Scenery>().set_motion_on(on);
        }
        self.save_settings();
    }

    /// Paints the scene when it needs it, and what moves when it's due. Called every tick in the Full layout.
    /// The scene's layers, made the first time they're needed. None off the Mac, for `-screenshot` (which
    /// captures only what Slint draws), with DRIFTWAVE_SLINT_SCENE set, or if they couldn't be made.
    fn scene_layers(&mut self, ui: &MainWindow) -> Option<&mut SceneLayers> {
        if !self.look.layers_settled {
            let wanted = cfg!(target_os = "macos") && self.args.get("screenshot").is_none() && std::env::var_os("DRIFTWAVE_SLINT_SCENE").is_none();
            // `None` while winit has no window yet: try again on the next backdrop.
            let made = if wanted {
                ui.window().with_winit_window(|w: &winit::window::Window| {
                    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
                    match w.window_handle().map(|h| h.as_raw()) {
                        Ok(RawWindowHandle::AppKit(h)) => SceneLayers::new(h.ns_view.as_ptr()),
                        _ => None,
                    }
                })
            } else {
                Some(None)
            };
            if let Some(layers) = made {
                self.look.layers_settled = true;
                self.look.native_layers = layers;
                if wanted {
                    log(&format!("scene layers: {}", if self.look.native_layers.is_some() { "native" } else { "unavailable, drawn by the UI" }));
                }
            }
        }
        self.look.native_layers.as_mut()
    }

    /// Draws the UI see-through over the scene's layers (on), or opaque again (off).
    fn set_native(&mut self, ui: &MainWindow, on: bool) {
        if self.look.native_on == on {
            return;
        }
        self.look.native_on = on;
        self.look.native_rows = None;
        self.look.native_band = None;
        ui.global::<Scenery>().set_native(on);
        #[cfg(target_os = "macos")]
        softbuffer::set_see_through(on);
        if on {
            self.look.hide_layers = false;
            if let Some(l) = self.look.native_layers.as_mut() {
                l.set_visible(true);
            }
        } else {
            self.look.hide_layers = true;
        }
    }

    /// No scene showing (Classic, or a theme without one): the UI goes opaque, then the layers hide.
    pub(crate) fn park_look(&mut self, ui: &MainWindow) {
        if self.look.native_on {
            self.set_native(ui, false);
        } else if self.look.hide_layers {
            self.look.hide_layers = false;
            if let Some(l) = self.look.native_layers.as_mut() {
                l.set_visible(false);
            }
        }
    }

    pub(crate) fn tick_look(&mut self, ui: &MainWindow, now: Instant, dt: f32, focused: bool) {
        let scenery = ui.global::<Scenery>();
        // Signs that show the time change with the minute.
        if self.look.clock {
            let minute = chrono::Local::now().timestamp() / 60;
            if minute != self.look.minute {
                self.look.minute = minute;
                let shown: Vec<SceneText> = self.look.texts.iter().map(|t| SceneText { text: with_clock(&t.text).into(), ..t.clone() }).collect();
                scenery.set_texts(ModelRc::from(Rc::new(VecModel::from(shown))));
            }
        }
        let scale = ui.window().scale_factor();
        let size = ui.window().size();
        // Nothing to paint for until the window is on screen (its size isn't final before then).
        if size.width < 8 || size.height < 8 || !ui.window().is_visible() {
            return;
        }
        let (lw, lh) = (size.width as f32 / scale, size.height as f32 / scale);
        let t = (now - *self.look.started.get_or_insert(now)).as_secs_f64();
        let ambient = !self.look.still && !self.look.valve.tripped;

        // The UI's moving parts: thin strips (or the spotlight), so only they redraw.
        if self.look.synth && focused && ambient {
            scenery.set_synth_phase((t * 0.45 % 1.0) as f32);
        }
        if self.look.band {
            let y = if focused && ambient { ((t % 7.0) / 7.0) as f32 * (lh + 280.0) - 140.0 } else { -400.0 };
            if (scenery.get_band_y() - y).abs() > 0.4 {
                scenery.set_band_y(y);
            }
        }
        // macOS with the scene's layers: the rows and the band are layers too, so the UI isn't redrawn under
        // them every tick (full.slint hides its own while Scenery.native).
        if self.look.native_on {
            let phase = scenery.get_synth_phase();
            let grid = scenery.get_synth_grid();
            let band_y = scenery.get_band_y();
            let (synth, band) = (self.look.synth, self.look.band);
            let key = (phase.to_bits(), size.width, size.height);
            if let Some(layers) = self.look.native_layers.as_mut() {
                if !synth {
                    if self.look.native_rows.take().is_some() {
                        layers.set_rows(None, (0.0, 0.0, 0.0, 0.0));
                    }
                } else if self.look.native_rows != Some(key) {
                    let (px, h, y0, hpt) = synth_rows(lh, scale, phase, grid);
                    layers.set_rows(Some((&px, 1, h)), (0.0, y0 as f64, lw as f64, hpt as f64));
                    self.look.native_rows = Some(key);
                }
                let at = band.then(|| (band_y * 4.0).round() as i32);
                if at != self.look.native_band || (band && self.look.native_band.is_none()) {
                    layers.set_band(band.then_some((0.0, band_y as f64, lw as f64, 140.0)));
                    self.look.native_band = at;
                }
            }
        }
        if self.look.spot {
            let target = self.look.pointer.unwrap_or((lw * 0.42, lh * 0.35));
            let fresh = self.look.spot_at == (0.0, 0.0);
            let k = if fresh { 1.0 } else { 1.0 - (-dt * 10.0).exp() };
            let (x, y) = (self.look.spot_at.0 + (target.0 - self.look.spot_at.0) * k, self.look.spot_at.1 + (target.1 - self.look.spot_at.1) * k);
            if fresh || (x - self.look.spot_at.0).abs() > 0.3 || (y - self.look.spot_at.1).abs() > 0.3 {
                self.look.spot_at = (x, y);
                scenery.set_spot_x(x);
                scenery.set_spot_y(y);
            }
        }

        self.tick_background(ui, now, focused && ambient);
        if self.look.scene.is_none() {
            self.park_look(ui);
            return;
        }
        // Back in the Full layout with the backdrop already on its layer: show it again.
        if self.look.painted.is_some() && self.look.native_layers.as_ref().is_some_and(|l| l.ready()) {
            self.set_native(ui, true);
        }
        let player = ui.get_show_player();
        let title_end = ui.get_title_end();
        let g = Geometry { width: lw, height: lh, title: TITLE, pane: PANE, player: if player { PLAYER } else { 0.0 }, title_end };
        let has_motion = self.look.scene.as_ref().is_some_and(|s| s.has_motion);
        let still = !ambient || !has_motion;
        let key = (size.width, size.height, still, player, title_end.round() as i32);
        if self.look.painted != Some(key) {
            // A drag-resize sends many sizes: wait for it to rest (the old picture stretches meanwhile).
            match self.look.pending {
                Some((k, since)) if k == key => {
                    if self.look.painted.is_some() && now - since < SETTLE {
                        return;
                    }
                }
                _ => {
                    self.look.pending = Some((key, now));
                    if self.look.painted.is_some() {
                        return;
                    }
                }
            }
            self.paint_backdrop(ui, &g, scale, still);
            // The valve's next reading starts after the painting (it isn't what moving costs).
            self.look.valve.since = None;
            let layers = std::mem::take(&mut self.look.layers);
            self.place_eggs(&layers, &g);
            self.look.layers = layers;
            self.look.painted = Some(key);
            self.look.pending = None;
        }
        if still || !focused {
            self.look.valve.since = None;
            return;
        }
        // The valve: every 6 s of motion in front, what the app cost; over budget, the scene holds still.
        match (self.look.valve.since, process_cpu()) {
            (None, Some(cpu)) => self.look.valve.since = Some((now, cpu)),
            (Some((at, was)), Some(cpu)) if now - at >= Duration::from_secs(6) => {
                let share = (cpu.saturating_sub(was)).as_secs_f64() / (now - at).as_secs_f64();
                log(&format!("scene motion: the app used {:.1}% of one core", share * 100.0));
                if share > MOTION_BUDGET && !self.args.flag("noValve") {
                    log("scene motion over budget: holding still");
                    self.look.valve.tripped = true;
                    return;
                }
                self.look.valve.since = Some((now, cpu));
            }
            _ => {}
        }
        let Some(scene) = self.look.scene.as_mut() else { return };
        let due = self.look.last_motion.is_none_or(|l| (now - l).as_secs_f64() >= 1.0 / scene.motion_fps - 0.004);
        if !due {
            return;
        }
        let mdt = self.look.last_motion.map(|l| (now - l).as_secs_f32()).unwrap_or(0.0);
        self.look.last_motion = Some(now);
        let t0 = Instant::now();
        // Soft things (rain, light, colour) scale up smoothly: no finer than 0.6 px a point on dense screens.
        let m = scene.motion(&g, (scale * scene.motion_scale).min(0.6), t, mdt);
        if self.look.native_on {
            // The GPU stretches and blends it: nothing in the UI is redrawn.
            if let Some(l) = self.look.native_layers.as_mut() {
                l.set_motion(Some((&m.px, m.w, m.h)));
            }
        } else {
            let buf = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&m.px, m.w as u32, m.h as u32);
            scenery.set_motion(Image::from_rgba8_premultiplied(buf));
            scenery.set_moving(true);
        }
        self.look.motion_ms += t0.elapsed().as_secs_f64() * 1000.0;
        self.look.motion_frames += 1;
        if self.look.motion_frames == 120 {
            log(&format!("scene motion {:.2} ms a frame ({}×{})", self.look.motion_ms / 120.0, m.w, m.h));
            self.look.motion_ms = 0.0;
            self.look.motion_frames = 0;
        }
    }

    /// The Classic layout's skin, painted once per size (after a resize settles): still, so Classic stays as light
    /// as it was.
    pub(crate) fn tick_classic_look(&mut self, ui: &MainWindow, now: Instant) {
        let Some(scene) = self.look.scene.as_mut() else { return };
        let size = ui.window().size();
        if size.width < 8 || size.height < 8 {
            return;
        }
        let key = (size.width, size.height);
        if self.look.skin_painted == Some(key) {
            return;
        }
        match self.look.skin_pending {
            Some((k, since)) if k == key => {
                if self.look.skin_painted.is_some() && now - since < SETTLE {
                    return;
                }
            }
            _ => {
                self.look.skin_pending = Some((key, now));
                if self.look.skin_painted.is_some() {
                    return;
                }
            }
        }
        let scale = ui.window().scale_factor();
        let (w, h) = (size.width as f32 / scale, size.height as f32 / scale);
        let t0 = Instant::now();
        let pal = ui.global::<super::Pal>();
        match scene.skin(w, h, scale) {
            Some(mut p) => {
                // Glass themes: frosted, as Full frosts the picture under its panels.
                if scene.glass() {
                    let (pw, ph) = (p.w, p.h);
                    crate::paint::frost(&mut p, 0, 0, pw, ph, (15.0 * scale).round() as usize);
                }
                driftwave_vis::canvas::keep_the_rule(&mut p.px);
                // As see-through as the theme asks, unless the picture would drown the frames' labels. They're all on
                // the player's frame, at the top (136 units of a 412-unit-wide window).
                let face = from_slint(pal.get_face());
                let want = self.look.classic_see;
                let top = (136.0 * w / 412.0 * scale).min(p.h as f32);
                let alpha = scene::readable_alpha(
                    &p,
                    scene::Rect { x: 0.0, y: 0.0, w: p.w as f32, h: top },
                    Color { a: ((1.0 - want) * 255.0).round() as u8, ..face },
                    from_slint(pal.get_label()),
                    4.5,
                );
                pal.set_see(want.min(1.0 - alpha as f32 / 255.0));
                let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(p.w as u32, p.h as u32);
                for (d, s) in buf.make_mut_bytes().as_chunks_mut::<3>().0.iter_mut().zip(p.px.as_chunks::<4>().0) {
                    d.copy_from_slice(&s[..3]);
                }
                pal.set_skin(Image::from_rgb8(buf));
                pal.set_skin_on(true);
            }
            None => pal.set_skin_on(false),
        }
        match scene.classic_front(w, h, scale) {
            Some(f) => {
                pal.set_skin_front(Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&f.px, f.w as u32, f.h as u32)));
                pal.set_skin_front_on(true);
            }
            None => pal.set_skin_front_on(false),
        }
        self.look.skin_painted = Some(key);
        self.look.skin_pending = None;
        log(&format!("classic skin {}×{} in {:.1} ms", size.width, size.height, t0.elapsed().as_secs_f64() * 1000.0));
    }

    fn paint_backdrop(&mut self, ui: &MainWindow, g: &Geometry, scale: f32, still: bool) {
        let Some(scene) = self.look.scene.as_mut() else { return };
        let t0 = Instant::now();
        let mut back = scene.backdrop(g, scale, still);
        let mut front = scene.front(g, scale);
        // Glass: under the panels (everything below the title bar), the picture is frosted. Painted once.
        if scene.glass() {
            let r = (15.0 * scale).round() as usize;
            let (bw, bh) = (back.w, back.h);
            crate::paint::frost(&mut back, 0, (g.title * scale).round() as usize, bw, bh, r);
        }
        // The colour rule over the whole picture, once it's put together: light meeting light (red neon over blue)
        // can make what no layer had. Painted once per size, so this costs a millisecond or two, once.
        driftwave_vis::canvas::keep_the_rule(&mut back.px);
        // The full-window scaled art isn't needed until the next backdrop (motion's pictures are smaller).
        // Retrowave's sun: its own picture over the page's glass, in its real colours.
        {
            let scenery = ui.global::<Scenery>();
            let page = g.region(Some("content"));
            match scene.effects().sun_picture(g.width * scale, g.height * scale, (page.x + page.w / 2.0) * scale, scale, 0.7) {
                Some((p, x, y)) => {
                    scenery.set_sun(Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&p.px, p.w as u32, p.h as u32)));
                    scenery.set_sun_x(x / scale);
                    scenery.set_sun_y(y / scale);
                    scenery.set_sun_w(p.w as f32 / scale);
                    scenery.set_sun_h(p.h as f32 / scale);
                    scenery.set_sun_on(true);
                }
                None => scenery.set_sun_on(false),
            }
        }
        scene.forget_large((back.w * back.h / 2) as u64);
        // See-through panels: as see-through as the theme asks, unless the picture behind would drown their text.
        // With motion, a little more (lights pass behind them).
        if let Some(p) = self.look.panels {
            let target = if still { 4.5 } else { 5.0 };
            let px = |r: scene::Rect| scene::Rect { x: r.x * scale, y: r.y * scale, w: r.w * scale, h: r.h * scale };
            let content = px(g.region(Some("content")));
            let lift = |c: Color, a: u8| slint_colour(Color { a: a.max(c.a), ..c });
            let th = ui.global::<Th>();
            let surface = scene::readable_alpha(&back, content, p.surface, p.text, target);
            th.set_surface(lift(p.surface, surface));
            th.set_control(lift(p.control, scene::readable_alpha(&back, content, p.control, p.control_text, target)));
            if g.player > 0.0 {
                th.set_card(lift(p.card, scene::readable_alpha(&back, px(g.region(Some("player"))), p.card, p.card_text, target)));
            }
            th.set_pane(lift(p.pane, scene::readable_alpha(&back, px(g.region(Some("pane"))), p.pane, p.pane_text, target)));
        }
        let native = if let Some(layers) = self.scene_layers(ui) {
            // macOS: onto its layer beneath the UI, which then draws see-through over it.
            layers.set_backdrop(&back.px, back.w, back.h);
            if still {
                layers.set_motion(None);
            }
            true
        } else {
            // Opaque, so it goes to the UI as RGB: drawn by copying, not blending.
            let mut buf = SharedPixelBuffer::<Rgb8Pixel>::new(back.w as u32, back.h as u32);
            for (d, s) in buf.make_mut_bytes().as_chunks_mut::<3>().0.iter_mut().zip(back.px.as_chunks::<4>().0) {
                d.copy_from_slice(&s[..3]);
            }
            ui.global::<Scenery>().set_back(Image::from_rgb8(buf));
            false
        };
        if native {
            self.set_native(ui, true);
        }
        let scenery = ui.global::<Scenery>();
        // macOS with the layers: the front picture is a layer over the UI (full.slint skips its own then). The UI
        // draws the window frame over the front picture: an opaque frame hides it, so it's cleared there; under
        // a see-through frame it stays in the UI.
        let th = ui.global::<Th>();
        let frame = th.get_frame_on().then(|| ((th.get_frame_thickness() * scale).round().max(0.0) as usize, th.get_frame_color().alpha()));
        let front_native = native && frame.is_none_or(|(_, alpha)| alpha == 255);
        ui.global::<Scenery>().set_front_native(front_native);
        if front_native {
            if let (Some(f), Some((t, _))) = (front.as_mut(), frame) {
                for y in 0..f.h {
                    let edge = y < t || y + t >= f.h;
                    for x in 0..f.w {
                        if edge || x < t || x + t >= f.w {
                            f.px[(y * f.w + x) * 4..][..4].fill(0);
                        }
                    }
                }
            }
        }
        if native {
            if let Some(layers) = self.look.native_layers.as_mut() {
                layers.set_front(front.as_ref().filter(|_| front_native).map(|f| (&f.px[..], f.w, f.h)));
            }
        }
        match front {
            Some(f) if !front_native => {
                scenery.set_front(Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&f.px, f.w as u32, f.h as u32)));
                scenery.set_front_on(true);
            }
            Some(_) => scenery.set_front_on(true),
            None => scenery.set_front_on(false),
        }
        if still {
            scenery.set_moving(false);
        }
        self.look.last_motion = None;
        log(&format!("scene backdrop {}×{} {}in {:.1} ms{}", back.w, back.h, if still { "(still) " } else { "" }, t0.elapsed().as_secs_f64() * 1000.0, if native { " (layer)" } else { "" }));
    }
}
