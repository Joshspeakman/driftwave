//! Your own background: a picture or a looping GIF behind the whole window, in any theme (docs/THEMES.md, "Your own
//! background"). From a file or from GIPHY; every one is checked first, and a GIF that flashes more than three times a
//! second is refused (no flashing, ever). Files live in the data folder's Backgrounds folder; the `background`
//! setting names the one in use, and `backgroundDim` darkens it (0–0.8, 0.35 by default).
//!
//! It shows through every theme as smoky glass: the theme's photo steps aside (its rain, drops and neon stay). A
//! picture costs nothing once painted; a GIF plays only while the window is in front and scenes may move, at no more
//! than 12 frames a second, under the same valve as the scenes.

use driftwave_core::flash;
use driftwave_core::storage::AppPaths;
use driftwave_vis::guard::measure;
use image::{imageops, AnimationDecoder, RgbaImage};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Frames are kept no bigger than this on their longer side (a GIF's frames all stay in memory).
const MAX_SIDE: u32 = 960;
const MAX_FRAMES: usize = 400;

/// A loaded background: its frames (one for a picture) and how long each shows.
pub struct Picture {
    pub frames: Vec<Arc<RgbaImage>>,
    pub delays: Vec<f64>,
}

impl Picture {
    pub fn animated(&self) -> bool {
        self.frames.len() > 1
    }
}

pub fn folder(paths: &AppPaths) -> PathBuf {
    paths.support.join("Backgrounds")
}

fn shrink(img: RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let k = MAX_SIDE as f32 / w.max(h) as f32;
    if k >= 1.0 {
        return img;
    }
    imageops::resize(&img, ((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1), imageops::FilterType::Triangle)
}

/// Decodes a picture or a GIF (every frame composited, as it's shown).
pub fn load(path: &Path) -> Result<Picture, String> {
    let gif = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("gif"));
    if gif {
        let file = std::fs::File::open(path).map_err(|e| format!("Couldn't open it ({e})."))?;
        let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(file)).map_err(|_| "That GIF couldn't be read.".to_string())?;
        let mut frames = Vec::new();
        let mut delays = Vec::new();
        for f in decoder.into_frames().take(MAX_FRAMES) {
            let f = f.map_err(|_| "That GIF couldn't be read.".to_string())?;
            let (n, d) = f.delay().numer_denom_ms();
            // Browsers treat a delay under 20 ms as 100 ms; so do we.
            let ms = if d == 0 { 100.0 } else { n as f64 / d as f64 };
            delays.push(if ms < 20.0 { 0.1 } else { ms / 1000.0 });
            frames.push(Arc::new(shrink(f.into_buffer())));
        }
        if frames.is_empty() {
            return Err("That GIF has no pictures in it.".into());
        }
        return Ok(Picture { frames, delays });
    }
    let img = image::open(path).map_err(|_| "That picture couldn't be read.".to_string())?.to_rgba8();
    Ok(Picture { frames: vec![Arc::new(shrink(img))], delays: vec![1.0] })
}

/// Whether an animation flashes (WCAG 2.3.1, as the visualizers are judged): every frame, small, over the whole
/// picture and over a 3×2 grid of regions.
pub fn flashes(p: &Picture) -> bool {
    if !p.animated() {
        return false;
    }
    let (gx, gy) = (3, 2);
    let mut whole = Vec::new();
    let mut cells: Vec<Vec<flash::Frame>> = vec![Vec::new(); gx * gy];
    for (f, &d) in p.frames.iter().zip(&p.delays) {
        let w = 64u32;
        let h = ((f.height() as f32 / f.width().max(1) as f32) * 64.0).round().clamp(8.0, 128.0) as u32;
        let small = imageops::resize(&**f, w, h, imageops::FilterType::Triangle);
        let (all, each) = measure(small.as_raw(), w as usize, h as usize, gx, gy, d);
        whole.push(all);
        for (c, e) in cells.iter_mut().zip(each) {
            c.push(e);
        }
    }
    flash::is_flashing(&whole, true) || cells.iter().any(|c| flash::is_flashing(c, true))
}

/// Copies a picture or GIF into the Backgrounds folder after checking it; the new file's name, or why not.
pub fn import(paths: &AppPaths, source: &Path) -> Result<String, String> {
    let ext = source.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !matches!(ext.as_str(), "gif" | "png" | "jpg" | "jpeg" | "webp" | "bmp") {
        return Err("Choose a GIF, PNG, JPEG, WebP or BMP.".into());
    }
    let picture = load(source)?;
    if flashes(&picture) {
        return Err("That GIF flashes, so Driftwave won't use it (nothing in Driftwave flashes).".into());
    }
    let dir = folder(paths);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the Backgrounds folder ({e})."))?;
    let name = format!("{}.{ext}", uuid::Uuid::new_v4());
    std::fs::copy(source, dir.join(&name)).map_err(|e| format!("Couldn't copy it ({e})."))?;
    Ok(name)
}

/// A GIF found on GIPHY: its title, a small still preview, and the GIF to use.
#[derive(Clone, Debug)]
pub struct Found {
    pub title: String,
    pub preview: String,
    pub full: String,
}

/// The GIPHY key: your own (Settings), else the one built into release builds.
pub fn giphy_key(own: &str) -> Option<String> {
    let own = own.trim();
    if !own.is_empty() {
        return Some(own.to_string());
    }
    option_env!("DRIFTWAVE_GIPHY_KEY").map(String::from).filter(|k| !k.is_empty())
}

/// Searches GIPHY (rated G).
pub fn search(key: &str, query: &str) -> Result<Vec<Found>, String> {
    let resp = ureq::get("https://api.giphy.com/v1/gifs/search")
        .set("User-Agent", driftwave_core::radio_browser::USER_AGENT)
        .query("api_key", key)
        .query("q", query)
        .query("limit", "24")
        .query("rating", "g")
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(401 | 403, _) => "GIPHY didn't accept the key.".to_string(),
            ureq::Error::Status(429, _) => "GIPHY is busy; try again in a minute.".to_string(),
            _ => "Couldn't reach GIPHY.".to_string(),
        })?;
    let v: serde_json::Value = resp.into_json().map_err(|_| "GIPHY's answer couldn't be read.".to_string())?;
    let url = |g: &serde_json::Value, k: &str| g["images"][k]["url"].as_str().map(String::from);
    Ok(v["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|g| {
                    Some(Found {
                        title: g["title"].as_str().unwrap_or("").to_string(),
                        preview: url(g, "fixed_width_small_still").or_else(|| url(g, "fixed_width_still"))?,
                        full: url(g, "downsized_medium").or_else(|| url(g, "original"))?,
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

/// Fetches a URL's bytes (GIPHY's previews and GIFs), up to 12 MB.
pub fn fetch(url: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    if !url.starts_with("https://") {
        return Err("Not a GIPHY address.".into());
    }
    let resp = ureq::get(url)
        .set("User-Agent", driftwave_core::radio_browser::USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|_| "Couldn't download it.".to_string())?;
    let mut bytes = Vec::new();
    resp.into_reader().take(12 * 1024 * 1024).read_to_end(&mut bytes).map_err(|_| "Couldn't download it.".to_string())?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, Rgba};

    fn gif(path: &Path, frames: &[[u8; 3]], ms: u32) {
        let file = std::fs::File::create(path).unwrap();
        let mut enc = GifEncoder::new(file);
        enc.set_repeat(Repeat::Infinite).unwrap();
        for c in frames {
            let img = RgbaImage::from_pixel(32, 24, Rgba([c[0], c[1], c[2], 255]));
            enc.encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(ms, 1))).unwrap();
        }
    }

    #[test]
    fn a_flashing_gif_is_refused_and_a_gentle_one_kept() {
        let dir = std::env::temp_dir().join(format!("driftwave-bg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Black and white, ten times a second: a strobe.
        let strobe = dir.join("strobe.gif");
        gif(&strobe, &[[0, 0, 0], [255, 255, 255]].repeat(10), 100);
        assert!(flashes(&load(&strobe).unwrap()));
        // A slow fade between two blues: fine.
        let calm = dir.join("calm.gif");
        let fade: Vec<[u8; 3]> = (0..20).map(|i| [20, 40, 100 + (i as u8) * 3]).collect();
        gif(&calm, &fade, 150);
        let p = load(&calm).unwrap();
        assert!(p.animated());
        assert!(!flashes(&p));
        let paths = AppPaths::new(dir.join("support"), dir.join("caches"));
        assert!(import(&paths, &strobe).is_err());
        let name = import(&paths, &calm).unwrap();
        assert!(folder(&paths).join(name).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

// ── The app's side: settings, the playing GIF, and the GIPHY dialog.

use super::{log, App, Bg as BgUi, GiphyItem, MainWindow, Scenery};
use slint::{ComponentHandle, Model, ModelRc, SharedPixelBuffer, VecModel};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct State {
    images: Vec<slint::Image>,
    frame: usize,
    next: Option<Instant>,
    playing: bool,
    found: Vec<Found>,
    items: Rc<VecModel<GiphyItem>>,
    search_seq: u64,
}

fn slint_image(img: &RgbaImage) -> slint::Image {
    slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(img.as_raw(), img.width(), img.height()))
}

impl App {
    pub(crate) fn bg_dim(&self) -> f32 {
        self.settings.get("backgroundDim").and_then(serde_json::Value::as_f64).unwrap_or(0.35) as f32
    }

    /// Loads the background the settings name (on a thread: a long GIF takes a moment), then shows it.
    pub(crate) fn load_background(&mut self) {
        let name = self.settings.get("background").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        if name.is_empty() {
            if self.bg.is_some() {
                self.show_background(None);
            }
            return;
        }
        let path = folder(&self.paths).join(&name);
        crate::finding::background(move || load(&path), move |app, result| match result {
            Ok(p) => app.show_background(Some(p)),
            Err(e) => {
                log(&format!("background: {e}"));
                app.show_background(None);
            }
        });
    }

    fn show_background(&mut self, p: Option<Picture>) {
        self.bg_state.images = p.as_ref().map(|p| p.frames.iter().map(|f| slint_image(f)).collect()).unwrap_or_default();
        self.bg_state.frame = 0;
        self.bg_state.next = None;
        self.bg_state.playing = false;
        self.bg = p;
        if let Some(ui) = self.ui.upgrade() {
            let b = ui.global::<BgUi>();
            b.set_has(self.bg.is_some());
            b.set_dim(self.bg_dim());
            let s = ui.global::<Scenery>();
            s.set_user_anim(false);
            s.set_user_dim(self.bg_dim());
        }
        // The look is rebuilt round it (smoky glass, the theme's photo aside).
        let id = self.theme_id.clone();
        self.apply_theme(id);
    }

    /// Uses a picture or GIF: checked, copied in, saved as the setting.
    pub(crate) fn use_background_file(&mut self, source: std::path::PathBuf) {
        self.bg_message("Checking it…");
        let paths = self.paths.clone();
        crate::finding::background(move || import(&paths, &source), move |app, result| match result {
            Ok(name) => {
                app.settings["background"] = serde_json::json!(name);
                app.save_settings();
                app.bg_message("");
                app.message("Your background is in place.");
                app.load_background();
            }
            Err(e) => {
                app.bg_message("");
                app.message(&e);
            }
        });
    }

    pub(crate) fn choose_background(&mut self) {
        let picked = rfd::FileDialog::new().add_filter("Pictures and GIFs", &["gif", "png", "jpg", "jpeg", "webp", "bmp"]).pick_file();
        if let Some(path) = picked {
            self.use_background_file(path);
        }
    }

    pub(crate) fn remove_background(&mut self) {
        self.settings["background"] = serde_json::json!("");
        self.save_settings();
        self.show_background(None);
    }

    pub(crate) fn set_background_dim(&mut self, d: f32) {
        self.settings["backgroundDim"] = serde_json::json!((d.clamp(0.0, 0.8) * 100.0).round() / 100.0);
        self.save_settings();
        if self.bg.is_some() {
            if let Some(ui) = self.ui.upgrade() {
                ui.global::<Scenery>().set_user_dim(self.bg_dim());
                ui.global::<BgUi>().set_dim(self.bg_dim());
            }
            let id = self.theme_id.clone();
            self.apply_theme(id);
        }
    }

    fn bg_message(&self, text: &str) {
        if let Some(ui) = self.ui.upgrade() {
            ui.global::<BgUi>().set_status(text.into());
        }
    }

    /// A GIF background plays over its own first frame (the backdrop), while `moving`; otherwise it rests there.
    pub(crate) fn tick_background(&mut self, ui: &MainWindow, now: Instant, moving: bool) {
        let animated = self.bg.as_ref().is_some_and(Picture::animated);
        let st = &mut self.bg_state;
        if !animated || !moving {
            if st.playing {
                st.playing = false;
                ui.global::<Scenery>().set_user_anim(false);
            }
            return;
        }
        let Some(p) = self.bg.as_ref() else { return };
        if st.next.is_some_and(|n| now < n) || st.images.is_empty() {
            return;
        }
        st.frame = (st.frame + 1) % p.frames.len();
        // No faster than 12 frames a second (the scenes' rate): a GIF's own pace, or slower.
        st.next = Some(now + Duration::from_secs_f64(p.delays[st.frame].max(1.0 / 12.0)));
        let s = ui.global::<Scenery>();
        s.set_user_image(st.images[st.frame].clone());
        if !st.playing {
            st.playing = true;
            s.set_user_anim(true);
        }
    }

    // GIPHY.

    pub(crate) fn open_giphy(&mut self) {
        if let Some(ui) = self.ui.upgrade() {
            let b = ui.global::<BgUi>();
            b.set_results(ModelRc::from(self.bg_state.items.clone()));
            b.set_search_open(true);
            let key = self.settings.get("giphyKey").and_then(serde_json::Value::as_str).unwrap_or("");
            b.set_status(if giphy_key(key).is_some() { "".into() } else { "Searching GIPHY needs a key: paste yours under GIPHY key in Settings.".into() });
        }
    }

    pub(crate) fn giphy_search(&mut self, query: &str) {
        let own = self.settings.get("giphyKey").and_then(serde_json::Value::as_str).unwrap_or("").to_string();
        let Some(key) = giphy_key(&own) else {
            self.bg_message("Searching GIPHY needs a key: paste yours under GIPHY key in Settings.");
            return;
        };
        let q = query.trim().to_string();
        if q.is_empty() {
            return;
        }
        self.bg_state.search_seq += 1;
        let seq = self.bg_state.search_seq;
        self.bg_message("Searching…");
        crate::finding::background(move || search(&key, &q), move |app, result| {
            if seq != app.bg_state.search_seq {
                return;
            }
            match result {
                Ok(found) => {
                    app.bg_message(if found.is_empty() { "Nothing found." } else { "" });
                    app.bg_state.items.set_vec(found.iter().map(|f| GiphyItem { title: f.title.clone().into(), preview: Default::default(), ready: false }).collect::<Vec<_>>());
                    // The previews arrive one by one.
                    for (i, f) in found.iter().enumerate() {
                        let url = f.preview.clone();
                        crate::finding::background(
                            move || fetch(&url).ok().and_then(|b| image::load_from_memory(&b).ok()).map(|i| i.to_rgba8()),
                            move |app, img| {
                                if seq != app.bg_state.search_seq {
                                    return;
                                }
                                if let (Some(img), Some(mut row)) = (img, app.bg_state.items.row_data(i)) {
                                    row.preview = slint_image(&img);
                                    row.ready = true;
                                    app.bg_state.items.set_row_data(i, row);
                                }
                            },
                        );
                    }
                    app.bg_state.found = found;
                }
                Err(e) => app.bg_message(&e),
            }
        });
    }

    /// Uses a GIPHY result: downloaded, checked for flashing, copied in.
    pub(crate) fn giphy_pick(&mut self, i: usize) {
        let Some(f) = self.bg_state.found.get(i).cloned() else { return };
        self.bg_message("Downloading and checking it…");
        let dir = folder(&self.paths);
        crate::finding::background(
            move || -> Result<std::path::PathBuf, String> {
                let bytes = fetch(&f.full)?;
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let tmp = dir.join(format!("giphy-{}.gif", uuid::Uuid::new_v4()));
                std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
                Ok(tmp)
            },
            move |app, result| match result {
                Ok(tmp) => {
                    if let Some(ui) = app.ui.upgrade() {
                        ui.global::<BgUi>().set_search_open(false);
                    }
                    let paths = app.paths.clone();
                    // Checked and copied under a new name; the download goes either way.
                    crate::finding::background(
                        move || {
                            let r = import(&paths, &tmp);
                            let _ = std::fs::remove_file(&tmp);
                            r
                        },
                        |app, result| match result {
                            Ok(name) => {
                                app.settings["background"] = serde_json::json!(name);
                                app.save_settings();
                                app.message("Your background is in place.");
                                app.load_background();
                            }
                            Err(e) => app.message(&e),
                        },
                    );
                }
                Err(e) => app.bg_message(&e),
            },
        );
    }

    pub(crate) fn set_giphy_key(&mut self, key: &str) {
        self.settings["giphyKey"] = serde_json::json!(key.trim());
        self.save_settings();
        self.message(if key.trim().is_empty() { "Your GIPHY key is cleared." } else { "Your GIPHY key is saved." });
    }
}
