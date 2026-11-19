//! Every visualizer, fed the most provoking audio there is, must never flash on screen.
//!
//! The signals are chosen to make a visualizer pulse at the worst rates (WCAG counts more than three flashes a
//! second): noise gated on and off at 4, 7 and 10 Hz, kicks at 300 bpm, hard cuts between silence and full scale, and
//! an ordinary busy track. Frames are judged by `driftwave_core::flash` over the whole picture and over each of the
//! guard's regions (each about WCAG's small-area size).

use driftwave_core::flash;
use driftwave_vis::analysis::synth;
use driftwave_vis::guard::measure;
use driftwave_vis::{visualizers, VisHost};

const RATE: u32 = 44100;
const FPS: f64 = 30.0;

fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn signals() -> Vec<(&'static str, Vec<f32>)> {
    let tau = std::f64::consts::TAU;
    let mut out = Vec::new();
    for hz in [4.0, 7.0, 10.0] {
        let mut s = 7u32;
        out.push((
            match hz as u32 {
                4 => "noise gated at 4 Hz",
                7 => "noise gated at 7 Hz",
                _ => "noise gated at 10 Hz",
            },
            synth(RATE, 6.0, |t| if (t * hz).fract() < 0.5 { let v = noise(&mut s); (v, v) } else { (0.0, 0.0) }),
        ));
    }
    out.push((
        "kicks at 300 bpm",
        synth(RATE, 6.0, |t| {
            let p = t % 0.2;
            let v = if p < 0.09 { ((p * 55.0 * tau).sin() * (1.0 - p / 0.09)) as f32 } else { 0.0 };
            (v, v)
        }),
    ));
    let mut s = 11u32;
    out.push(("silence and full scale every 0.15 s", synth(RATE, 6.0, |t| if (t / 0.15) as u32 % 2 == 0 { (0.0, 0.0) } else { let v = noise(&mut s); (v, -v) })));
    let mut s = 3u32;
    out.push((
        "a busy track",
        synth(RATE, 6.0, |t| {
            let beat = t % (60.0 / 128.0);
            let kick = if beat < 0.1 { (beat * 60.0 * tau).sin() * (1.0 - beat / 0.1) } else { 0.0 };
            let chord = [220.0, 277.2, 329.6].iter().map(|f| (t * f * tau).sin()).sum::<f64>() * 0.12;
            let hat = if (t * 8.0).fract() < 0.03 { noise(&mut s) as f64 * 0.3 } else { 0.0 };
            let v = (kick * 0.8 + chord + hat) as f32;
            (v, v * 0.9)
        }),
    ));
    out
}

#[test]
fn no_visualizer_ever_flashes() {
    let (w, h) = (340usize, 256usize);
    let mut failures = Vec::new();
    for (name, audio) in signals() {
        let frames = (audio.len() / 2) as f64 / RATE as f64 * FPS;
        for v in visualizers::all() {
            let mut host = VisHost::new(v.id(), w, h, 1.0);
            let (gx, gy) = (2, 2);
            let mut whole = Vec::new();
            let mut cells: Vec<Vec<flash::Frame>> = vec![Vec::new(); gx * gy];
            for f in 0..frames as usize {
                let end = ((f as f64 / FPS) * RATE as f64) as usize;
                let start = end.saturating_sub(2048);
                let px = host.frame(&audio[start * 2..end * 2], 2, RATE, f as f64 / FPS, (1.0 / FPS) as f32);
                let (all, each) = measure(px, w, h, gx, gy, 1.0 / FPS);
                whole.push(all);
                for (c, e) in each.into_iter().enumerate() {
                    cells[c].push(e);
                }
            }
            if flash::is_flashing(&whole, false) {
                failures.push(format!("{} with {name}: the whole picture flashes", v.name()));
            }
            for (c, seq) in cells.iter().enumerate() {
                if flash::is_flashing(seq, false) {
                    failures.push(format!("{} with {name}: region {c} flashes", v.name()));
                }
            }
            if whole.iter().any(|f| f.red > 0.0) {
                failures.push(format!("{} with {name}: saturated red reached the screen", v.name()));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn every_visualizer_draws_something_and_has_a_unique_id() {
    let audio = synth(RATE, 3.0, |t| {
        let v = ((t * 330.0 * std::f64::consts::TAU).sin() * 0.5) as f32;
        (v, v)
    });
    let mut ids = std::collections::HashSet::new();
    for v in visualizers::all() {
        assert!(ids.insert(v.id()), "duplicate id {}", v.id());
        assert!(visualizers::by_id(v.id()).is_some());
        let mut host = VisHost::new(v.id(), 160, 90, 1.0);
        let mut lit = 0;
        for f in 0..45 {
            let end = (f + 30) * RATE as usize / 30;
            let px = host.frame(&audio[(end - 2048) * 2..end * 2], 2, RATE, f as f64 / 30.0, 1.0 / 30.0);
            lit = px.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30).count();
        }
        assert!(lit > 160 * 90 / 50, "{} drew almost nothing ({lit} lit pixels)", v.name());
    }
}
