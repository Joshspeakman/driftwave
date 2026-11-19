//! Renders the visualizers to image files, to look at them without the app, and times them.
//!
//! cargo run --release -p driftwave-vis --example preview -- <out dir> [audio.f32] [width height] [seconds]
//!
//! `audio.f32` is raw interleaved stereo float at 44.1 kHz, for example from a stream:
//! ffmpeg -i <url> -t 20 -f f32le -ac 2 -ar 44100 audio.f32
//! Without it, a synthetic track plays. Writes <id>-<frame>.ppm every half second, and prints ms per frame.

use driftwave_vis::analysis::synth;
use driftwave_vis::{visualizers, VisHost};
use std::io::Write;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = std::path::PathBuf::from(args.first().cloned().unwrap_or_else(|| "vis-preview".into()));
    std::fs::create_dir_all(&out).unwrap();
    let audio_file = args.get(1).filter(|a| a.ends_with(".f32"));
    let rest: Vec<&String> = args.iter().skip(if audio_file.is_some() { 2 } else { 1 }).collect();
    let w: usize = rest.first().and_then(|s| s.parse().ok()).unwrap_or(640);
    let h: usize = rest.get(1).and_then(|s| s.parse().ok()).unwrap_or(360);
    let seconds: f64 = rest.get(2).and_then(|s| s.parse().ok()).unwrap_or(12.0);
    let rate = 44100u32;
    let audio: Vec<f32> = match audio_file {
        Some(p) => std::fs::read(p).unwrap().chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect(),
        None => synth(rate, seconds + 1.0, |t| {
            let tau = std::f64::consts::TAU;
            let beat = t % (60.0 / 124.0);
            let kick = if beat < 0.12 { (beat * 58.0 * tau).sin() * (1.0 - beat / 0.12) } else { 0.0 };
            let bassline = (t * [55.0, 55.0, 65.4, 49.0][(t / 1.94) as usize % 4] * tau).sin() * 0.25;
            let pad = [261.6, 329.6, 392.0, 493.9].iter().map(|f| (t * f * tau).sin()).sum::<f64>() * 0.06 * (1.0 + (t * 0.25).sin());
            let lead = (t * 880.0 * tau * (1.0 + 0.002 * (t * 5.0).sin())).sin() * 0.1 * ((t * 2.0).fract() < 0.5) as u8 as f64;
            let v = (kick * 0.7 + bassline + pad + lead) as f32;
            (v, v * 0.92 + (t * 3.0).sin() as f32 * 0.02)
        }),
    };
    let frames = ((audio.len() / 2) as f64 / rate as f64 * 30.0).min(seconds * 30.0) as usize;
    for v in visualizers::all() {
        let mut host = VisHost::new(v.id(), w, h, 1.0);
        let started = Instant::now();
        for f in 0..frames {
            let end = (f as f64 / 30.0 * rate as f64) as usize;
            let start = end.saturating_sub(2048);
            let (dw, dh) = host.size();
            let px = host.frame(&audio[start * 2..end * 2], 2, rate, f as f64 / 30.0, 1.0 / 30.0);
            if f % 15 == 14 {
                // The drawing size (the UI scales it up to the slot).
                let mut file = std::fs::File::create(out.join(format!("{}-{:04}.ppm", v.id(), f))).unwrap();
                write!(file, "P6\n{dw} {dh}\n255\n").unwrap();
                let rgb: Vec<u8> = px.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
                file.write_all(&rgb).unwrap();
            }
        }
        let ms = started.elapsed().as_secs_f64() * 1000.0 / frames.max(1) as f64;
        println!("{:16} {:6.2} ms a frame at {w}x{h} ({:.1}% of one core at 30 fps)", v.name(), ms, ms * 30.0 / 10.0);
    }
}
