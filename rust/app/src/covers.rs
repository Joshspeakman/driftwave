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

pub fn tile_colours(name: &str) -> (slint::Color, slint::Color) {
    let mut h = DefaultHasher::new();
    name.hash(&mut h);
    let hue = (h.finish() % 360) as f32 / 360.0;
    let rgb = |h: f32, s: f32, v: f32| {
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
            let h = if span > 0.0 { m.from - 0.12 + hue * span } else { m.from + arc * hue };
            let h = h.rem_euclid(1.0);
            if m.light {
                (rgb(h, 0.35, 0.80 + vary * 0.06), rgb((h + 0.03).rem_euclid(1.0), 0.30, 0.94))
            } else if (0.11..0.22).contains(&h) {
                // Deep yellows go olive: pull them to amber, and lighter.
                let h = 0.07 + (h - 0.11) * 0.25;
                (rgb(h, 0.8, 0.36 + vary * 0.08), rgb(h + 0.02, 0.75, 0.72))
            } else {
                (rgb(h, 0.78, 0.22 + vary * 0.08), rgb((h + 0.03).rem_euclid(1.0), 0.72, 0.50 + vary * 0.1))
            }
        }
        None => {
            let hue = if (0.13..0.22).contains(&hue) { 0.06 + (hue - 0.13) * 0.4 } else { hue };
            (rgb(hue, 0.5, 0.55), rgb((hue + 0.09) % 1.0, 0.55, 0.78))
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
