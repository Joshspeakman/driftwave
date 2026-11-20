//! Station covers: each station's logo (or the picture someone chose), fetched on a few background threads, cached
//! on disk, decoded small (at most 256 px, enough for the largest place a cover is shown) and handed to the UI thread.
//! A station without one gets a soft two-colour tile, its colours taken from its name so it's always the same.

use driftwave_core::radio_browser::USER_AGENT;
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SIZE: u32 = 256;
const WORKERS: usize = 3;
const MAX_BYTES: u64 = 3 << 20;

/// What to fetch: the station's id, and a URL or a local file.
struct Job {
    key: String,
    source: String,
}

pub struct Covers {
    jobs: Sender<Job>,
    asked: HashSet<String>,
    pub images: HashMap<String, slint::Image>,
}

fn hash(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Two colours for a station's tile, from its name (soft, never saturated red).
/// What placeholder tiles take from a theme: its own colours (the `tiles` effect), or hues between its two
/// accents (Night Market's pink and cyan): deep and rich under a dark theme, soft pastels under a light one.
#[derive(Clone)]
pub struct Mood {
    pub from: f32,
    pub to: f32,
    pub light: bool,
    pub palette: Vec<[u8; 3]>,
    pub flat: bool,
}

thread_local! {
    static MOOD: std::cell::RefCell<Option<Mood>> = const { std::cell::RefCell::new(None) };
}

/// Sets the tiles' mood (None: Driftwave's own bright tiles).
pub fn set_mood(m: Option<Mood>) {
    MOOD.with(|c| *c.borrow_mut() = m);
}

/// A colour's hue (0–1).
pub fn hue_of(r: u8, g: u8, b: u8) -> f32 {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    if d < 1e-4 {
        return 0.0;
    }
    let h = if max == r { ((g - b) / d).rem_euclid(6.0) } else if max == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    h / 6.0
}

/// A tile's second colour's hue (turns): `dh` on from the first, but within the first's family of allowed colours.
/// The tile blends from one to the other, and green to azure would pass through teal.
fn partner(h: f32, dh: f32) -> f32 {
    use driftwave_core::hues::{allowed_arc, nearest_allowed};
    let h1 = nearest_allowed(h * 360.0, 1.0, 0.5).0;
    let (lo, hi) = allowed_arc(h1);
    let base = if h1 < lo { h1 + 360.0 } else { h1 };
    let d = dh * 360.0;
    let h2 = if base + d <= hi {
        base + d
    } else if base - d >= lo {
        base - d
    } else {
        base
    };
    (h2 / 360.0).rem_euclid(1.0)
}

pub fn tile_colours(name: &str) -> (slint::Color, slint::Color) {
    let mut h = DefaultHasher::new();
    name.hash(&mut h);
    let hue = (h.finish() % 360) as f32 / 360.0;
    // The colour rule: no teal, purple or pink, however the hue falls (a tile is one colour: it moves to the nearest
    // allowed hue at full strength).
    let rgb = |h: f32, s: f32, v: f32| {
        let (h, s) = driftwave_core::hues::nearest_allowed(h * 360.0, s, v);
        let h = h / 360.0;
        let i = (h * 6.0).floor();
        let f = h * 6.0 - i;
        let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
        let (r, g, b) = match i as i32 % 6 {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };
        slint::Color::from_rgb_u8((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
    };
    // A second, independent number from the name, for a little variety in brightness.
    let vary = ((h.finish() >> 20) % 1000) as f32 / 1000.0;
    let mood = MOOD.with(|c| c.borrow().clone());
    match mood {
        // The theme's own colours: one each, flat or lifted towards white at the bottom right.
        Some(m) if !m.palette.is_empty() => {
            let [r, g, b] = m.palette[(hue * m.palette.len() as f32) as usize % m.palette.len()];
            let base = slint::Color::from_rgb_u8(r, g, b);
            let lift = |v: u8| (v as f32 + (255.0 - v as f32) * 0.22) as u8;
            (base, if m.flat { base } else { slint::Color::from_rgb_u8(lift(r), lift(g), lift(b)) })
        }
        // Each station lands somewhere on the shorter way round from one accent's hue to the other (a little
        // either side of the accent when the two are close).
        Some(m) => {
            let mut arc = m.to - m.from;
            if arc > 0.5 {
                arc -= 1.0;
            }
            if arc < -0.5 {
                arc += 1.0;
            }
            let span = if arc.abs() < 0.08 { 0.24 } else { 0.0 };
            let h = if span > 0.0 {
                // A little either side of the accent, but only within its own family of allowed colours (a blue
                // theme's tiles stay blue, rather than spilling into teal and lavender).
                let from = m.from * 360.0;
                let (lo, hi) = driftwave_core::hues::allowed_arc(from);
                let from = if from < lo { from + 360.0 } else { from };
                let side = if hue < 0.5 { (from - lo).min(43.2) } else { (hi - from).min(43.2) };
                (from + (hue - 0.5) * 2.0 * side) / 360.0
            } else {
                m.from + arc * hue
            };
            let h = h.rem_euclid(1.0);
            if m.light {
                (rgb(h, 0.35, 0.80 + vary * 0.06), rgb(partner(h, 0.03), 0.30, 0.94))
            } else if (0.11..0.22).contains(&h) {
                // Deep yellows go olive: pull them to amber, and lighter.
                let h = 0.07 + (h - 0.11) * 0.25;
                (rgb(h, 0.8, 0.36 + vary * 0.08), rgb(partner(h, 0.02), 0.75, 0.72))
            } else {
                (rgb(h, 0.78, 0.22 + vary * 0.08), rgb(partner(h, 0.03), 0.72, 0.50 + vary * 0.1))
            }
        }
        None => {
            // Spread over the allowed colours (crimson to green, azure to blue), mostly clear of the banned bands.
            let hue = driftwave_core::hues::spread(hue * 360.0) / 360.0;
            let hue = if (0.13..0.22).contains(&hue) { 0.06 + (hue - 0.13) * 0.4 } else { hue };
            (rgb(hue, 0.5, 0.55), rgb(partner(hue, 0.09), 0.55, 0.78))
        }
    }
}

impl Covers {
    /// `deliver` runs on a worker thread; it should hand the picture to the UI thread (slint::invoke_from_event_loop).
    pub fn new(cache: PathBuf, deliver: impl Fn(String, Option<SharedPixelBuffer<Rgba8Pixel>>) + Send + Sync + 'static) -> Covers {
        let _ = std::fs::create_dir_all(&cache);
        let (tx, rx) = channel::<Job>();
        let rx: Arc<Mutex<Receiver<Job>>> = Arc::new(Mutex::new(rx));
        let deliver = Arc::new(deliver);
        for n in 0..WORKERS {
            let (rx, deliver, cache) = (rx.clone(), deliver.clone(), cache.clone());
            let _ = std::thread::Builder::new().name(format!("driftwave-covers-{n}")).spawn(move || loop {
                let job = match rx.lock().map(|r| r.recv()) {
                    Ok(Ok(job)) => job,
                    _ => return,
                };
                let picture = load(&cache, &job.source);
                deliver(job.key, picture);
            });
        }
        Covers { jobs: tx, asked: HashSet::new(), images: HashMap::new() }
    }

    /// Asks for a station's cover once; it arrives through `deliver`.
    pub fn want(&mut self, key: &str, source: Option<&str>) {
        let Some(source) = source.filter(|s| !s.is_empty()) else { return };
        if self.images.contains_key(key) || !self.asked.insert(key.to_string()) {
            return;
        }
        let _ = self.jobs.send(Job { key: key.to_string(), source: source.to_string() });
    }

    pub fn get(&self, key: &str) -> Option<slint::Image> {
        self.images.get(key).cloned()
    }
}

/// A logo too small to fill a cover without looking blocky.
pub fn is_small(img: &slint::Image) -> bool {
    let s = img.size();
    s.width < 128 && s.height < 128
}

/// Reads a cover from the cache, a local file or the web, and decodes it small.
fn load(cache: &std::path::Path, source: &str) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    let bytes = if source.starts_with("http://") || source.starts_with("https://") {
        let file = cache.join(hash(source));
        match std::fs::read(&file) {
            Ok(b) if !b.is_empty() => b,
            _ => {
                let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(12)).user_agent(USER_AGENT).build();
                let resp = agent.get(source).call().ok()?;
                let mut b = Vec::new();
                resp.into_reader().take(MAX_BYTES).read_to_end(&mut b).ok()?;
                let _ = std::fs::write(&file, &b);
                b
            }
        }
    } else {
        std::fs::read(source).ok()?
    };
    let img = image::load_from_memory(&bytes).ok()?;
    let img = if img.width() > SIZE || img.height() > SIZE { img.thumbnail(SIZE, SIZE) } else { img };
    let rgba = img.to_rgba8();
    Some(SharedPixelBuffer::clone_from_slice(rgba.as_raw(), rgba.width(), rgba.height()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use driftwave_core::hues::banned;

    /// Placeholder tiles keep the colour rule: both ends of every tile's gradient and the colour halfway between,
    /// with no theme and under light and dark themes' moods (one each side of every banned band).
    #[test]
    fn tiles_keep_the_colour_rule() {
        let moods = [
            None,
            Some(Mood { from: 215.0 / 360.0, to: 210.0 / 360.0, light: true, palette: Vec::new(), flat: false }),
            Some(Mood { from: 215.0 / 360.0, to: 205.0 / 360.0, light: false, palette: Vec::new(), flat: false }),
            Some(Mood { from: 10.0 / 360.0, to: 32.0 / 360.0, light: false, palette: Vec::new(), flat: false }),
            Some(Mood { from: 140.0 / 360.0, to: 30.0 / 360.0, light: true, palette: Vec::new(), flat: false }),
            Some(Mood { from: 20.0 / 360.0, to: 220.0 / 360.0, light: false, palette: Vec::new(), flat: false }),
        ];
        let mut bad = Vec::new();
        for (m, mood) in moods.iter().enumerate() {
            set_mood(mood.clone());
            for i in 0..2000 {
                let name = format!("Station {i} FM");
                let (a, b) = tile_colours(&name);
                let mid = (((a.red() as u16 + b.red() as u16) / 2) as u8, ((a.green() as u16 + b.green() as u16) / 2) as u8, ((a.blue() as u16 + b.blue() as u16) / 2) as u8);
                for (part, c) in [("top", (a.red(), a.green(), a.blue())), ("bottom", (b.red(), b.green(), b.blue())), ("middle", mid)] {
                    if let Some(k) = banned(c.0, c.1, c.2, 255) {
                        bad.push(format!("mood {m}, {name}, {part}: {c:?} ({k:?})"));
                    }
                }
            }
        }
        set_mood(None);
        assert!(bad.is_empty(), "{} banned tiles, e.g. {:?}", bad.len(), &bad[..bad.len().min(8)]);
    }
}
