//! The colour rule (Josh, 2026-11-20): no teal, no purple and no pink, in the visualizers too. Each one plays a busy
//! track for long enough to drift through its moods and palettes; what reaches the screen is judged by
//! `driftwave_core::hues::banned`.

use driftwave_core::hues::banned;
use driftwave_vis::analysis::synth;
use driftwave_vis::{visualizers, VisHost};

const RATE: u32 = 44100;

#[test]
fn no_visualizer_shows_teal_purple_or_pink() {
    let tau = std::f64::consts::TAU;
    let mut seed = 5u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let secs = 75.0;
    let audio = synth(RATE, secs, |t| {
        let beat = t % (60.0 / 124.0);
        let kick = if beat < 0.1 { (beat * 60.0 * tau).sin() * (1.0 - beat / 0.1) } else { 0.0 };
        // Chords that move, so every band gets its turn.
        let root = [220.0, 196.0, 247.0, 165.0][((t / 4.0) as usize) % 4];
        let chord = [1.0, 1.26, 1.5, 2.0].iter().map(|k| (t * root * k * tau).sin()).sum::<f64>() * 0.1;
        let hat = if (t * 8.0).fract() < 0.03 { noise() as f64 * 0.3 } else { 0.0 };
        let v = (kick * 0.8 + chord + hat) as f32;
        (v, v * 0.92)
    });
    let fps = 24.0;
    let mut failures = Vec::new();
    for v in visualizers::all() {
        let mut host = VisHost::new(v.id(), 200, 150, 1.0);
        let (mut lit, mut bad) = (0u64, 0u64);
        for f in 0..(secs * fps) as usize {
            let end = ((f as f64 / fps) * RATE as f64) as usize;
            let start = end.saturating_sub(2048);
            let px = host.frame(&audio[start * 2..end * 2], 2, RATE, f as f64 / fps, (1.0 / fps) as f32);
            // Every sixth frame is plenty to see the palettes drift.
            if f % 6 != 0 {
                continue;
            }
            for p in px.chunks(4) {
                if p[0] as u32 + p[1] as u32 + p[2] as u32 > 45 {
                    lit += 1;
                    if banned(p[0], p[1], p[2], 255).is_some() {
                        bad += 1;
                    }
                }
            }
        }
        let share = 100.0 * bad as f64 / lit.max(1) as f64;
        // Blending and antialiasing leave a trace at the edges between colours; anything visible is a failure.
        if share > 1.0 {
            failures.push(format!("{}: {share:.2}% of lit pixels are teal, purple or pink", v.name()));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// What the rule costs at the output: a worst case (every pixel coloured, a third of them in the banned bands) on the
/// big slot, against the visualizers' budget of a couple of milliseconds a frame.
#[test]
fn the_rule_is_cheap() {
    let (w, h) = (340usize, 256usize);
    let mut frame = vec![0u8; w * h * 4];
    for (i, p) in frame.chunks_mut(4).enumerate() {
        let hue = (i % 360) as f32;
        let c = driftwave_vis::canvas::hsv_unruled(hue / 360.0, 0.8, 0.9);
        p.copy_from_slice(&[(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, 255]);
    }
    let t = std::time::Instant::now();
    let runs = 50;
    for _ in 0..runs {
        let mut f = frame.clone();
        driftwave_vis::canvas::keep_the_rule(&mut f);
        std::hint::black_box(&f);
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / runs as f64;
    println!("the rule: {ms:.3} ms a frame ({w}x{h}, worst case)");
    // And a frame of allowed colours only (the usual case: the visualizers draw allowed colours; blends stray).
    let mut ok = vec![0u8; w * h * 4];
    for (i, p) in ok.chunks_mut(4).enumerate() {
        let hue = [10.0f32, 40.0, 100.0, 215.0][i % 4];
        let c = driftwave_vis::canvas::hsv_unruled(hue / 360.0, 0.8, 0.9);
        p.copy_from_slice(&[(c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8, 255]);
    }
    let t = std::time::Instant::now();
    for _ in 0..runs {
        let mut f = ok.clone();
        driftwave_vis::canvas::keep_the_rule(&mut f);
        std::hint::black_box(&f);
    }
    let t2 = std::time::Instant::now();
    for _ in 0..runs {
        let f = ok.clone();
        std::hint::black_box(&f);
    }
    let copy = t2.elapsed().as_secs_f64() * 1000.0 / runs as f64;
    println!("the rule: {:.3} ms a frame of allowed colours; copying a frame {copy:.3} ms", t.elapsed().as_secs_f64() * 1000.0 / runs as f64 - copy);
    assert!(ms < 2.0, "{ms:.3} ms a frame");
}

