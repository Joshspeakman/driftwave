//! Driftwave, in the Rust app's first layout: Classic, an homage to the stacked players of the Winamp years.
//!
//! Until the switch-over (docs/RUST-SCOPE.md, phase 5) this app keeps its own data folder, `DriftwaveRust`, apart
//! from the Driftwave app people use today. On its first launch it copies that app's station list (reading it, never
//! writing). Test runs (any flag below, or SHORTWAVE_TEST=1) use `DriftwaveRustTestRun` and stay silent unless
//! `-audible`.
//!
//! Flags: -playURL <url>, -muted, -audible, -theme <id>, -vis <id>, -library <file>, -double, -shade, -noVis,
//! -noList, -screenshot <file.bmp>, -screenshotDelay <s>, -quitAfter <s>.

mod palette;

use driftwave_audio::{Engine, EngineConfig, Event, PlayState};
use driftwave_core::radio_browser::USER_AGENT;
use driftwave_core::recording::{RecorderConfig, RecordingMode};
use driftwave_core::station::Station;
use driftwave_core::storage::{library_file, AppPaths};
use driftwave_core::theme::{load_folder, Color, Theme};
use driftwave_vis::{visualizers, VisHost};
use serde_json::{json, Value};
use slint::winit_030::{winit, WinitWindowAccessor};
use slint::{ComponentHandle, ModelRc, SharedPixelBuffer, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

slint::include_modules!();

const FLAGS: [&str; 9] = ["muted", "audible", "double", "shade", "noVis", "noList", "logLevels", "help", "alwaysActive"];

fn log(s: &str) {
    if std::env::var_os("SHORTWAVE_LOG").is_some() || std::env::var_os("DRIFTWAVE_LOG").is_some() {
        eprintln!("[driftwave] {s}");
    }
}

struct Args {
    values: HashMap<String, String>,
    flags: Vec<String>,
}

impl Args {
    fn parse() -> Args {
        let raw: Vec<String> = std::env::args().skip(1).collect();
        let (mut values, mut flags) = (HashMap::new(), Vec::new());
        let mut i = 0;
        while i < raw.len() {
            let k = raw[i].trim_start_matches('-').to_string();
            if FLAGS.contains(&k.as_str()) {
                flags.push(k);
                i += 1;
            } else if i + 1 < raw.len() {
                values.insert(k, raw[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
        }
        Args { values, flags }
    }
    fn flag(&self, k: &str) -> bool {
        self.flags.iter().any(|f| f == k)
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.values.get(k).map(|s| s.as_str())
    }
    fn num(&self, k: &str) -> Option<f64> {
        self.get(k).and_then(|v| v.parse().ok())
    }
    fn test_run(&self) -> bool {
        !self.values.is_empty() || !self.flags.is_empty() || std::env::var_os("SHORTWAVE_TEST").is_some()
    }
}

fn to_slint(c: Color) -> slint::Color {
    slint::Color::from_argb_u8(c.a, c.r, c.g, c.b)
}

/// The built-in themes: next to the program, or (while developing) the repository's `themes` folder.
fn built_in_themes() -> Vec<Theme> {
    let exe = std::env::current_exe().unwrap_or_default();
    let mut dir = exe.parent().map(Path::to_path_buf);
    while let Some(d) = dir {
        let candidate = d.join("themes");
        if candidate.join("night-market.json").exists() {
            let mut t = load_folder(&candidate, true);
            t.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            return t;
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    Vec::new()
}

struct Player {
    engine: Option<(Engine, Receiver<Event>)>,
    station: Option<usize>,
    started: Option<Instant>,
    title: String,
    state: String,
    playing: bool,
    loading: bool,
    recording: bool,
}

struct App {
    ui: slint::Weak<ClassicWindow>,
    args: Args,
    paths: AppPaths,
    settings: Value,
    stations: Vec<Station>,
    themes: Vec<Theme>,
    player: Player,
    mini: VisHost,
    big: VisHost,
    vis_ids: Vec<&'static str>,
    last: Instant,
    started: Instant,
    marquee: String,
    /// With DRIFTWAVE_LOG: time spent in each part of the tick, logged every 150 ticks.
    profile: [f64; 4],
    ticks: u32,
}

impl App {
    fn setting_str(&self, k: &str) -> Option<String> {
        self.settings.get(k).and_then(|v| v.as_str()).map(String::from)
    }

    fn save_settings(&self) {
        // Test runs start from their flags every time: they never save (one run's -shade mustn't shade the next).
        if self.args.test_run() {
            return;
        }
        if let Some(ui) = self.ui.upgrade() {
            let mut s = self.settings.clone();
            s["layout"] = json!("classic");
            s["visualizer"] = json!(self.big.id());
            s["showVisualizer"] = json!(ui.get_show_vis());
            s["showList"] = json!(ui.get_show_list());
            s["double"] = json!(ui.get_u() > 2.0);
            s["shaded"] = json!(ui.get_shaded());
            s["volume"] = json!(ui.get_volume());
            s["onTop"] = json!(ui.get_on_top());
            let _ = std::fs::create_dir_all(&self.paths.support);
            let _ = std::fs::write(self.paths.support.join("settings.json"), serde_json::to_vec_pretty(&s).unwrap_or_default());
        }
    }

    fn apply_theme(&mut self, id: Option<String>) {
        let Some(ui) = self.ui.upgrade() else { return };
        let theme = id.as_ref().and_then(|id| self.themes.iter().find(|t| &t.id == id));
        let p = palette::classic(theme, true);
        let g = ui.global::<Pal>();
        g.set_face(to_slint(p.face));
        g.set_bevel_light(to_slint(p.bevel_light));
        g.set_bevel_dark(to_slint(p.bevel_dark));
        g.set_title_a(to_slint(p.title_a));
        g.set_title_b(to_slint(p.title_b));
        g.set_title_text(to_slint(p.title_text));
        g.set_lcd_back(to_slint(p.lcd_back));
        g.set_lcd_lit(to_slint(p.lcd_lit));
        g.set_lcd_ghost(to_slint(p.lcd_ghost));
        g.set_button(to_slint(p.button));
        g.set_glyph(to_slint(p.glyph));
        g.set_glyph_on(to_slint(p.glyph_on));
        g.set_groove(to_slint(p.groove));
        g.set_fill_low(to_slint(p.fill_low));
        g.set_fill_high(to_slint(p.fill_high));
        g.set_list_back(to_slint(p.list_back));
        g.set_list_text(to_slint(p.list_text));
        g.set_list_playing(to_slint(p.list_playing));
        g.set_list_selected_back(to_slint(p.list_selected_back));
        g.set_list_selected_text(to_slint(p.list_selected_text));
        g.set_glow(to_slint(p.glow));
        let fallback_body = if cfg!(target_os = "macos") { "Helvetica Neue" } else { "Segoe UI" };
        let fallback_lcd = if cfg!(target_os = "macos") { "Menlo" } else { "Consolas" };
        g.set_body_font(theme.and_then(|t| t.fonts.body.clone()).unwrap_or(fallback_body.into()).into());
        g.set_lcd_font(theme.and_then(|t| t.fonts.lcd.clone()).unwrap_or(fallback_lcd.into()).into());
        let mut choices = vec![Choice { id: "default".into(), name: "Default".into(), on: theme.is_none() }];
        choices.extend(self.themes.iter().map(|t| Choice { id: t.id.clone().into(), name: t.name.clone().into(), on: Some(&t.id) == id.as_ref() }));
        ui.set_themes(ModelRc::new(VecModel::from(choices)));
        self.settings["theme"] = json!(id.clone().unwrap_or_else(|| "default".into()));
        log(&format!("theme {}", id.unwrap_or_else(|| "default".into())));
    }

    fn refresh_list(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let rows: Vec<StationRow> = self
            .stations
            .iter()
            .enumerate()
            .map(|(i, s)| StationRow {
                name: s.name.clone().into(),
                detail: {
                    let codec = s.codec.to_uppercase();
                    match (codec.is_empty(), s.bitrate > 0) {
                        (false, true) => format!("{codec} {}", s.bitrate),
                        (false, false) => codec,
                        (true, true) => format!("{} kbps", s.bitrate),
                        _ => String::new(),
                    }
                }
                .into(),
                playing: self.player.station == Some(i) && self.player.engine.is_some(),
            })
            .collect();
        ui.set_list_summary(format!("{} STATION{}", self.stations.len(), if self.stations.len() == 1 { "" } else { "S" }).into());
        ui.set_stations(ModelRc::new(VecModel::from(rows)));
    }

    fn play(&mut self, index: usize) {
        let Some(station) = self.stations.get(index).cloned() else { return };
        self.stop();
        let silent = self.args.test_run() && !self.args.flag("audible");
        let recording_dir = self.paths.recording().join(std::process::id().to_string());
        let config = EngineConfig {
            silent,
            user_agent: USER_AGENT.to_string(),
            recorder: RecorderConfig {
                mode: if self.player.recording { RecordingMode::Everything } else { RecordingMode::Decide },
                minimum_duration: 30.0,
                maximum_duration: 900.0,
            },
            recording_dir,
            track_dir: self.paths.support.join("Tracks"),
            station_name: station.name.clone(),
            probe_playlists: station.is_local,
        };
        log(&format!("play {} silent={silent}", station.name));
        let (engine, events) = Engine::play(&station.stream_url, config);
        if let Some(ui) = self.ui.upgrade() {
            engine.set_volume(ui.get_volume());
            ui.set_kbps(station.bitrate.max(0));
            ui.set_station(station.name.clone().into());
            ui.set_selected(index as i32);
        }
        engine.set_muted(self.args.flag("muted"));
        self.player.engine = Some((engine, events));
        self.player.station = Some(index);
        self.player.started = None;
        self.player.title.clear();
        self.player.loading = true;
        self.player.playing = false;
        self.player.state = "CONNECTING".into();
        self.settings["lastStation"] = json!(station.id);
        self.update_marquee();
        self.refresh_list();
    }

    fn stop(&mut self) {
        if let Some((engine, _)) = self.player.engine.take() {
            engine.stop();
        }
        self.player.playing = false;
        self.player.loading = false;
        self.player.started = None;
        self.player.state = "STOPPED".into();
        self.update_marquee();
        self.refresh_list();
    }

    fn update_marquee(&mut self) {
        let station = self.player.station.and_then(|i| self.stations.get(i)).map(|s| s.name.clone());
        self.marquee = match (station, self.player.title.is_empty()) {
            (Some(s), false) => format!("{}  ***  {}", self.player.title, s),
            (Some(s), true) => format!("{s}  ***  {}", self.player.state),
            (None, _) => "DRIFTWAVE  ***  DOUBLE-CLICK A STATION TO PLAY".into(),
        };
        if let Some(ui) = self.ui.upgrade() {
            ui.set_marquee(self.marquee.clone().into());
        }
    }

    /// One tick of the clock: engine events, the time, the marquee, the meters and the visualizers.
    fn tick(&mut self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.25);
        self.last = now;
        // Engine events.
        let mut events = Vec::new();
        if let Some((_, rx)) = &self.player.engine {
            while let Ok(e) = rx.try_recv() {
                events.push(e);
            }
        }
        let mut marquee_changed = false;
        for e in events {
            match e {
                Event::State(PlayState::Loading) => {
                    self.player.loading = true;
                    self.player.state = "BUFFERING".into();
                    marquee_changed = true;
                }
                Event::State(PlayState::Playing) => {
                    self.player.playing = true;
                    self.player.loading = false;
                    self.player.state = "PLAYING".into();
                    self.player.started.get_or_insert(now);
                    marquee_changed = true;
                    log("state playing");
                }
                Event::State(PlayState::Failed(why)) => {
                    self.player.playing = false;
                    self.player.loading = false;
                    self.player.state = "FAILED".into();
                    self.player.title = why;
                    marquee_changed = true;
                }
                Event::NowPlaying { title, .. } => {
                    log(&format!("now playing \"{title}\""));
                    self.player.title = title;
                    marquee_changed = true;
                }
                Event::Recorder(_) | Event::Saved(_) | Event::SaveFailed(_) | Event::Connected { .. } | Event::Info(_) => {}
            }
        }
        if marquee_changed {
            self.update_marquee();
        }
        // Time: listening time, or the clock.
        if ui.get_clock_mode() {
            use chrono::Timelike;
            let local = chrono::Local::now();
            ui.set_minutes(local.hour() as i32);
            ui.set_seconds(local.minute() as i32);
        } else {
            let t = self.player.started.map(|s| (now - s).as_secs()).unwrap_or(0);
            ui.set_minutes(((t / 60) % 100) as i32);
            ui.set_seconds((t % 60) as i32);
        }
        ui.set_playing(self.player.playing);
        ui.set_loading(self.player.loading);
        ui.set_status(self.player.state.clone().into());
        ui.set_recording(self.player.recording && self.player.playing);

        // Moving parts rest while the window is minimised or not in front.
        let minimized = ui.window().is_minimized();
        let focused = self.args.flag("alwaysActive") || ui.window().with_winit_window(|w: &winit::window::Window| w.has_focus()).unwrap_or(true);
        ui.set_active(focused);
        if minimized || !focused {
            return;
        }
        let t0 = Instant::now();
        // The marquee scrolls at a steady pace and wraps seamlessly.
        let u = ui.get_u();
        let span = ui.get_marquee_width() + 24.0 * u;
        if span > 24.0 * u + 1.0 {
            let off = (ui.get_marquee_offset() + 16.0 * u * dt) % span;
            ui.set_marquee_offset(off);
        }
        // Real audio for the meters and the visualizers.
        let (samples, channels, rate) = match &self.player.engine {
            Some((engine, _)) if self.player.playing => engine.scope(2048),
            _ => (vec![0.0; 4096], 2, 44100),
        };
        if self.player.playing && channels >= 2 {
            ui.set_khz((rate / 1000) as i32);
            // Stereo: the channels differ.
            let diff: f32 = samples.chunks(channels).map(|f| (f[0] - f[1]).abs()).sum::<f32>();
            let total: f32 = samples.chunks(channels).map(|f| f[0].abs() + f[1].abs()).sum::<f32>();
            ui.set_stereo(total > 1e-3 && diff / total > 0.02);
        }
        // The buffer gauge eases towards the engine's figure, in steps fine enough to look continuous.
        let target = self.player.engine.as_ref().map(|(e, _)| (e.buffered_seconds() / 12.0).clamp(0.0, 1.0) as f32).unwrap_or(0.0);
        let shown = ui.get_buffer();
        let next = shown + (target - shown) * driftwave_vis::analysis::ease(dt, 0.3);
        if (next - shown).abs() > 0.002 {
            ui.set_buffer(next);
        }
        let t1 = Instant::now();
        let scale = ui.window().scale_factor();
        let t = (now - self.started).as_secs_f64();
        if !ui.get_shaded() {
            let (w, h) = ((ui.get_mini_width() * scale) as usize, (ui.get_mini_height() * scale) as usize);
            self.mini.resize(w, h, scale);
            let px = self.mini.frame(&samples, channels, rate, t, dt);
            ui.set_vis_mini(slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(px, w as u32, h as u32)));
        }
        let t2 = Instant::now();
        if ui.get_show_vis() {
            let (w, h) = ((ui.get_big_width() * scale) as usize, (ui.get_big_height() * scale) as usize);
            self.big.resize(w, h, scale);
            let px = self.big.frame(&samples, channels, rate, t, dt);
            ui.set_vis_big(slint::Image::from_rgba8(SharedPixelBuffer::clone_from_slice(px, w as u32, h as u32)));
        }
        let t3 = Instant::now();
        self.profile[0] += (t1 - t0).as_secs_f64();
        self.profile[1] += (t2 - t1).as_secs_f64();
        self.profile[2] += (t3 - t2).as_secs_f64();
        self.profile[3] += dt as f64;
        self.ticks += 1;
        if self.ticks == 150 {
            let ms = |v: f64| v * 1000.0 / 150.0;
            log(&format!(
                "tick ms: marquee+meters {:.2}, mini vis {:.2}, big vis {:.2}; {:.1} ticks a second",
                ms(self.profile[0]), ms(self.profile[1]), ms(self.profile[2]), 150.0 / self.profile[3].max(1e-3)
            ));
            self.profile = [0.0; 4];
            self.ticks = 0;
        }
    }

    fn cycle_vis(&mut self, step: i32) {
        let n = self.vis_ids.len() as i32;
        let cur = self.vis_ids.iter().position(|&v| v == self.big.id()).unwrap_or(0) as i32;
        let next = self.vis_ids[((cur + step).rem_euclid(n)) as usize];
        self.big.set_visualizer(next);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_vis_name(visualizers::by_id(next).map(|v| v.name()).unwrap_or("").into());
        }
        log(&format!("visualizer {next}"));
    }
}

fn save_bmp(path: &str, buf: &SharedPixelBuffer<slint::Rgba8Pixel>) -> std::io::Result<()> {
    let (w, h) = (buf.width() as usize, buf.height() as usize);
    let mut out = Vec::with_capacity(54 + w * h * 4);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((54 + w * h * 4) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(-(h as i32)).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    for p in buf.as_slice() {
        out.extend_from_slice(&[p.b, p.g, p.r, 255]);
    }
    std::fs::write(path, out)
}

fn main() -> Result<(), slint::PlatformError> {
    // The software renderer: measured at under half the CPU of the GPU one for this window (it redraws only what
    // changed) and 40 MB instead of 100. SLINT_BACKEND still overrides it.
    if std::env::var_os("SLINT_BACKEND").is_none() {
        let _ = slint::BackendSelector::new().backend_name("winit".into()).renderer_name("software".into()).select();
    }
    let args = Args::parse();
    let test = args.test_run();
    let paths = AppPaths::for_app(if test { "DriftwaveRustTestRun" } else { "DriftwaveRust" });
    let _ = std::fs::create_dir_all(&paths.support);
    log(&format!("launch testRun={test} folder={}", paths.support.display()));
    let settings: Value = std::fs::read(paths.support.join("settings.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| json!({}));

    // Stations: a test run's from -library (read only) or a single -playURL; otherwise our own list, first copied
    // from the Driftwave app's (read, never written).
    let mut stations = if let Some(lib) = args.get("library") {
        library_file::load(Path::new(lib))
    } else if test {
        Vec::new()
    } else {
        if !paths.library().exists() {
            let theirs = AppPaths::for_app("Driftwave").library();
            if theirs.exists() {
                let _ = std::fs::copy(&theirs, paths.library());
                log("copied the station list from Driftwave");
            }
        }
        library_file::load(&paths.library())
    };
    if let Some(url) = args.get("playURL") {
        stations.insert(0, Station::new("test-url", "Test Stream", url));
    }

    let ui = ClassicWindow::new()?;
    let vis_ids: Vec<&'static str> = visualizers::all().iter().map(|v| v.id()).collect();
    let vis_id = args.get("vis").map(String::from).or_else(|| settings.get("visualizer").and_then(|v| v.as_str()).map(String::from)).unwrap_or_else(|| "liquid".into());
    let mut themes = built_in_themes();
    themes.extend(load_folder(&paths.themes(), false));
    let app = Rc::new(RefCell::new(App {
        ui: ui.as_weak(),
        mini: VisHost::new("peak-meter", 152, 32, 1.0),
        big: VisHost::new(&vis_id, 526, 200, 1.0),
        args,
        paths,
        settings,
        stations,
        themes,
        player: Player { engine: None, station: None, started: None, title: String::new(), state: "STOPPED".into(), playing: false, loading: false, recording: false },
        vis_ids,
        last: Instant::now(),
        started: Instant::now(),
        marquee: String::new(),
        profile: [0.0; 4],
        ticks: 0,
    }));

    {
        let a = app.borrow();
        let s = &a.settings;
        ui.set_u(if a.args.flag("double") || s.get("double").and_then(Value::as_bool).unwrap_or(false) { 3.0 } else { 2.0 });
        ui.set_shaded(a.args.flag("shade") || s.get("shaded").and_then(Value::as_bool).unwrap_or(false));
        ui.set_show_vis(!a.args.flag("noVis") && s.get("showVisualizer").and_then(Value::as_bool).unwrap_or(true));
        ui.set_show_list(!a.args.flag("noList") && s.get("showList").and_then(Value::as_bool).unwrap_or(true));
        ui.set_volume(s.get("volume").and_then(Value::as_f64).unwrap_or(0.8) as f32);
        ui.set_vis_name(visualizers::by_id(a.big.id()).map(|v| v.name()).unwrap_or("").into());
        ui.set_version(env!("CARGO_PKG_VERSION").into());
    }
    let theme_id = { let a = app.borrow(); a.args.get("theme").map(String::from).or_else(|| a.setting_str("theme")) };
    app.borrow_mut().apply_theme(theme_id.filter(|t| t != "default"));
    app.borrow_mut().refresh_list();
    app.borrow_mut().update_marquee();

    // Callbacks.
    let weak = ui.as_weak();
    ui.on_drag(move || {
        if let Some(ui) = weak.upgrade() {
            ui.window().with_winit_window(|w: &winit::window::Window| {
                let _ = w.drag_window();
            });
        }
    });
    let weak = ui.as_weak();
    ui.on_minimize(move || {
        if let Some(ui) = weak.upgrade() {
            ui.window().set_minimized(true);
        }
    });
    let a = app.clone();
    ui.on_quit(move || {
        a.borrow().save_settings();
        a.borrow_mut().stop();
        let _ = slint::quit_event_loop();
    });
    let a = app.clone();
    ui.on_play(move || {
        let mut app = a.borrow_mut();
        let pick = app.ui.upgrade().map(|u| u.get_selected()).filter(|&i| i >= 0).map(|i| i as usize).or(app.player.station).or(if app.stations.is_empty() { None } else { Some(0) });
        if let Some(i) = pick {
            if !(app.player.station == Some(i) && app.player.engine.is_some()) {
                app.play(i);
            }
        }
    });
    let a = app.clone();
    ui.on_pause(move || a.borrow_mut().stop()); // live radio: pausing stops; play picks up live again
    let a = app.clone();
    ui.on_stop(move || a.borrow_mut().stop());
    let a = app.clone();
    ui.on_next(move || {
        let mut app = a.borrow_mut();
        let n = app.stations.len();
        if n > 0 {
            let i = app.player.station.map(|i| (i + 1) % n).unwrap_or(0);
            app.play(i);
        }
    });
    let a = app.clone();
    ui.on_prev(move || {
        let mut app = a.borrow_mut();
        let n = app.stations.len();
        if n > 0 {
            let i = app.player.station.map(|i| (i + n - 1) % n).unwrap_or(0);
            app.play(i);
        }
    });
    let a = app.clone();
    ui.on_play_station(move |i| a.borrow_mut().play(i as usize));
    let a = app.clone();
    ui.on_volume_changed(move |v| {
        if let Some((engine, _)) = &a.borrow().player.engine {
            engine.set_volume(v);
        }
    });
    let a = app.clone();
    ui.on_toggle_record(move || {
        let mut app = a.borrow_mut();
        app.player.recording = !app.player.recording;
        // Recording applies from the start of a stream: restart the station that's playing.
        if let Some(i) = app.player.station.filter(|_| app.player.engine.is_some()) {
            app.play(i);
        }
    });
    let weak = ui.as_weak();
    ui.on_toggle_clock(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_clock_mode(!ui.get_clock_mode());
        }
    });
    let a = app.clone();
    ui.on_next_vis(move || a.borrow_mut().cycle_vis(1));
    let a = app.clone();
    ui.on_prev_vis(move || a.borrow_mut().cycle_vis(-1));
    let weak = ui.as_weak();
    ui.on_toggle_on_top(move || {
        if let Some(ui) = weak.upgrade() {
            let on = !ui.get_on_top();
            ui.set_on_top(on);
            ui.window().with_winit_window(|w: &winit::window::Window| {
                w.set_window_level(if on { winit::window::WindowLevel::AlwaysOnTop } else { winit::window::WindowLevel::Normal });
            });
        }
    });
    let weak = ui.as_weak();
    ui.on_toggle_double(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_u(if ui.get_u() > 2.0 { 2.0 } else { 3.0 });
        }
    });
    let a = app.clone();
    ui.on_choose_theme(move |id| {
        let id = id.to_string();
        a.borrow_mut().apply_theme(if id == "default" { None } else { Some(id) });
        a.borrow().save_settings();
    });

    // The clock: 30 frames a second while in front; the tick itself skips the moving parts otherwise.
    let timer = slint::Timer::default();
    let a = app.clone();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(33), move || a.borrow_mut().tick());

    // Test runs: play at once, take a snapshot, quit after a while.
    if app.borrow().args.get("playURL").is_some() {
        app.borrow_mut().play(0);
    }
    if let Some(path) = app.borrow().args.get("screenshot").map(String::from) {
        let delay = app.borrow().args.num("screenshotDelay").unwrap_or(4.0);
        let weak = ui.as_weak();
        slint::Timer::single_shot(Duration::from_secs_f64(delay), move || {
            if let Some(ui) = weak.upgrade() {
                match ui.window().take_snapshot() {
                    Ok(buf) => {
                        let _ = save_bmp(&path, &buf);
                        log(&format!("screenshot {}x{} {path}", buf.width(), buf.height()));
                    }
                    Err(e) => log(&format!("screenshot failed: {e}")),
                }
            }
            let _ = slint::quit_event_loop();
        });
    }
    if let Some(s) = app.borrow().args.num("quitAfter") {
        slint::Timer::single_shot(Duration::from_secs_f64(s), || {
            let _ = slint::quit_event_loop();
        });
    }
    ui.run()?;
    app.borrow().save_settings();
    app.borrow_mut().stop();
    Ok(())
}
