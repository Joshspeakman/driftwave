//! The player card's decorations (ported from the C# app's PlayerDecor): an LCD track marquee with bitrate and
//! state, spectrum bars with falling peaks (Classic Skin, Llama Whip), a backlit FM tuning dial whose pointer swings
//! to each station's own spot, and left and right VU meters (Analog 70s).
//!
//! Unlike the C# app's, the bars and meters follow the music: they're driven by the engine's own audio. They move
//! only while a station plays and the window is in front; otherwise they rest and cost nothing.

use super::{App, Decor as DecorUi, MainWindow};
use driftwave_core::theme::{Color, Theme};
use driftwave_vis::analysis::{Analyzer, BANDS};
use serde_json::Value;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::rc::Rc;
use std::time::Instant;

const BARS: usize = 19;

pub struct Decor {
    lcd: bool,
    analyzer: bool,
    dial: bool,
    vu: bool,
    analysis: Analyzer,
    bars: Rc<VecModel<f32>>,
    peaks: Rc<VecModel<f32>>,
    level: [f32; BARS],
    peak: [f32; BARS],
    needle: f32,
    needle_v: f32,
    vu_angle: [f32; 2],
    vu_v: [f32; 2],
    text: (String, String),
    started: Instant,
}

impl Default for Decor {
    fn default() -> Self {
        Decor {
            lcd: false,
            analyzer: false,
            dial: false,
            vu: false,
            analysis: Analyzer::new(),
            bars: Rc::new(VecModel::from(vec![0.0; BARS])),
            peaks: Rc::new(VecModel::from(vec![0.0; BARS])),
            level: [0.0; BARS],
            peak: [0.0; BARS],
            needle: 0.02,
            needle_v: 0.0,
            vu_angle: [-48.0; 2],
            vu_v: [0.0; 2],
            text: (String::new(), String::new()),
            started: Instant::now(),
        }
    }
}

fn slint_colour(c: Color) -> slint::Color {
    slint::Color::from_argb_u8(c.a, c.r, c.g, c.b)
}

/// Where a station sits on the dial (0–1): a stable hash of its id, so it's the same spot every time.
fn dial_spot(id: &str) -> f32 {
    let mut h: u32 = 2_166_136_261;
    for c in id.encode_utf16() {
        h = (h ^ c as u32).wrapping_mul(16_777_619);
    }
    (h % 10_000) as f32 / 10_000.0
}

/// A channel's RMS level as a VU needle angle: −30 dBFS at the left stop (−48°), +3 dBFS at the right (48°).
fn vu_angle(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -48.0;
    }
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    let db = 20.0 * (rms.max(1e-6) * std::f32::consts::SQRT_2).log10();
    (-48.0 + (db + 30.0) / 33.0 * 96.0).clamp(-48.0, 50.0)
}

impl App {
    /// The theme's player decorations.
    pub(crate) fn apply_decor(&mut self, theme: Option<&Theme>) {
        let Some(ui) = self.ui.upgrade() else { return };
        let d = ui.global::<DecorUi>();
        let fx = |k: &str| theme.and_then(|t| t.effects.get(k));
        let colour = |o: Option<&serde_json::Map<String, Value>>, k: &str, dflt: Color| o.and_then(|o| o.get(k)).and_then(Value::as_str).and_then(Color::parse).unwrap_or(dflt);
        let dec = &mut self.decor;
        dec.lcd = fx("lcd").is_some();
        dec.analyzer = fx("analyzer").is_some();
        dec.dial = fx("dial").is_some();
        dec.vu = fx("vu").is_some();
        let lcd = fx("lcd");
        d.set_lcd(dec.lcd);
        d.set_lcd_ink(slint_colour(colour(lcd, "color", Color { a: 255, r: 0, g: 224, b: 0 })));
        d.set_lcd_back(slint_colour(colour(lcd, "background", Color { a: 255, r: 5, g: 8, b: 5 })));
        d.set_lcd_stroke(slint_colour(theme.and_then(|t| t.colors.get("controlStroke").copied()).unwrap_or(Color { a: 255, r: 128, g: 128, b: 128 })));
        let fallback = if cfg!(target_os = "macos") { "Menlo" } else { "Consolas" };
        d.set_lcd_font(theme.and_then(|t| t.fonts.lcd.clone()).unwrap_or(fallback.into()).into());
        d.set_analyzer(dec.analyzer);
        d.set_bars(ModelRc::from(dec.bars.clone()));
        d.set_peaks(ModelRc::from(dec.peaks.clone()));
        d.set_dial(dec.dial);
        d.set_dial_ink(slint_colour(colour(fx("dial"), "color", Color { a: 255, r: 242, g: 166, b: 90 })));
        d.set_vu(dec.vu);
        dec.text = (String::new(), String::new());
    }

    /// Moves the decorations along: called every tick in the Full layout.
    pub(crate) fn tick_decor(&mut self, ui: &MainWindow, dt: f32, focused: bool) {
        let dec = &self.decor;
        if !(dec.lcd || dec.analyzer || dec.dial || dec.vu) {
            return;
        }
        let d = ui.global::<DecorUi>();
        let playing = self.player.playing && self.player.engine.is_some();
        let station = self.player.current.as_ref();

        // The LCD: the song (or the station), in capitals, and what's playing at what rate.
        if self.decor.lcd {
            let line = match (self.player.loading, playing, self.player.title.is_empty()) {
                (true, _, _) => "BUFFERING...".to_string(),
                (_, true, false) => self.player.title.clone(),
                _ => station.map(|s| s.name.clone()).unwrap_or_default(),
            }
            .to_uppercase();
            let mut info = Vec::new();
            if let Some(s) = station {
                if s.bitrate > 0 {
                    info.push(format!("{} KBPS", s.bitrate));
                }
                if !s.codec.is_empty() {
                    info.push(s.codec.to_uppercase());
                }
            }
            info.push(if playing { "PLAY" } else if self.player.loading { "LOAD" } else { "STOP" }.into());
            let text = (format!("{line}  ***  "), info.join("   "));
            if text != self.decor.text {
                d.set_lcd_text(text.0.clone().into());
                d.set_lcd_info(text.1.clone().into());
                self.decor.text = text;
            }
            if focused {
                d.set_lcd_time(self.decor.started.elapsed().as_secs_f32());
            }
        }
        if !focused {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);

        // The music, for the bars and the meters.
        let wants_audio = self.decor.analyzer || self.decor.vu;
        let audio = if wants_audio && playing { self.player.engine.as_ref().map(|(e, _)| e.scope(2048)) } else { None };

        if self.decor.analyzer {
            let mut targets = [0.0f32; BARS];
            if let Some((samples, channels, rate)) = &audio {
                let a = self.decor.analysis.analyse(samples, *channels, *rate, 0.0, dt);
                for (i, t) in targets.iter_mut().enumerate() {
                    let (lo, hi) = (i * BANDS / BARS, ((i + 1) * BANDS / BARS).max(i * BANDS / BARS + 1));
                    let v = a.bands[lo..hi.min(BANDS)].iter().copied().fold(0.0, f32::max).clamp(0.0, 1.0);
                    // Typical music sits high in the analyzer's range: spread it so it moves through the middle.
                    *t = ((v - 0.25) / 0.75).clamp(0.0, 1.0).powf(1.35);
                }
            }
            let dec = &mut self.decor;
            let mut moved = false;
            for i in 0..BARS {
                let rise = targets[i] > dec.level[i];
                let next = dec.level[i] + (targets[i] - dec.level[i]) * (dt * if rise { 30.0 } else { 9.0 }).min(1.0);
                let peak = next.max(dec.peak[i] - dt * 0.35).max(0.0);
                if (next - dec.level[i]).abs() > 0.002 || (peak - dec.peak[i]).abs() > 0.002 {
                    moved = true;
                }
                dec.level[i] = next;
                dec.peak[i] = peak;
            }
            if moved {
                for i in 0..BARS {
                    dec.bars.set_row_data(i, if dec.level[i] < 0.005 { 0.0 } else { dec.level[i] });
                    dec.peaks.set_row_data(i, if dec.peak[i] < 0.02 { 0.0 } else { dec.peak[i] });
                }
            }
        }

        if self.decor.dial {
            // A slightly springy pointer, like a dial cord.
            let target = station.map(|s| dial_spot(&s.id)).unwrap_or(0.02);
            let dec = &mut self.decor;
            let a = (target - dec.needle) * 40.0 - dec.needle_v * 11.0;
            dec.needle_v += a * dt;
            dec.needle += dec.needle_v * dt;
            let shown = dec.needle + if playing { (dec.started.elapsed().as_secs_f32() * 3.1).sin() * 0.0002 } else { 0.0 };
            if (d.get_needle() - shown).abs() > 0.0001 {
                d.set_needle(shown);
            }
        }

        if self.decor.vu {
            let mut targets = [-48.0f32; 2];
            if let Some((samples, channels, _)) = &audio {
                let ch = (*channels).max(1);
                for (c, t) in targets.iter_mut().enumerate() {
                    let side: Vec<f32> = samples.iter().skip(c.min(ch - 1)).step_by(ch).copied().collect();
                    *t = vu_angle(&side[side.len().saturating_sub(1024)..]);
                }
            }
            let dec = &mut self.decor;
            for c in 0..2 {
                let a = (targets[c] - dec.vu_angle[c]) * 60.0 - dec.vu_v[c] * 12.0;
                dec.vu_v[c] += a * dt;
                dec.vu_angle[c] = (dec.vu_angle[c] + dec.vu_v[c] * dt).clamp(-50.0, 50.0);
            }
            if (d.get_vu_left() - dec.vu_angle[0]).abs() > 0.05 {
                d.set_vu_left(dec.vu_angle[0]);
            }
            if (d.get_vu_right() - dec.vu_angle[1]).abs() > 0.05 {
                d.set_vu_right(dec.vu_angle[1]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stations_keep_their_spot_on_the_dial() {
        let a = dial_spot("9617a958-0601-11e8-ae97-52543be04c81");
        assert_eq!(a, dial_spot("9617a958-0601-11e8-ae97-52543be04c81"));
        assert!((0.0..1.0).contains(&a));
        assert_ne!(a, dial_spot("another station"));
    }

    #[test]
    fn the_vu_needle_rests_on_silence_and_climbs_with_level() {
        assert_eq!(vu_angle(&[0.0; 512]), -48.0);
        let quiet: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.1).sin() * 0.05).collect();
        let loud: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.1).sin() * 0.7).collect();
        assert!(vu_angle(&quiet) < vu_angle(&loud));
        assert!(vu_angle(&loud) <= 50.0);
    }
}
