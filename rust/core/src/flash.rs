//! Does an animation flash? Driftwave never shows anything that flashes, flickers or strobes, and this is the
//! judge: background GIFs are checked with it, and every visualizer is tested against it.
//!
//! It follows WCAG 2.3.1 (three flashes): a flash is a pair of opposing changes in average brightness, each of at
//! least [`MIN_CHANGE`] relative luminance with the darker state below [`DARK_LIMIT`]. More than
//! [`MAX_FLASHES_PER_SECOND`] flashes in any one second fails. Saturated red counts on its own, at a change of
//! [`MIN_RED_CHANGE`] in the share of saturated-red pixels, as red is the most provoking colour.
//!
//! Stricter than the C# app's GIF check (`FlashCheck.cs`), which compares neighbouring frames only: here a change is
//! measured from the last turning point, so a fast ramp spread over several small steps still counts.

pub const MIN_CHANGE: f64 = 0.10;
pub const DARK_LIMIT: f64 = 0.80;
pub const MIN_RED_CHANGE: f64 = 0.20;
pub const MAX_FLASHES_PER_SECOND: usize = 3;

/// One frame (or one region of a frame): its average relative luminance (0–1), its share of saturated-red pixels
/// (0–1), and how long it shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub luminance: f64,
    pub red: f64,
    pub seconds: f64,
}

impl Frame {
    pub fn new(luminance: f64, red: f64, seconds: f64) -> Frame {
        Frame { luminance, red, seconds }
    }
}

/// Times (seconds from the start) of the changes in one measure, each opposite to the one before.
fn changes(values: impl Iterator<Item = (f64, f64)>, min: f64, dark_limit: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut anchor: Option<f64> = None;
    let mut dir = 0.0f64;
    for (t, v) in values {
        let Some(a) = anchor else {
            anchor = Some(v);
            continue;
        };
        if dir == 0.0 {
            if (v - a).abs() >= min && a.min(v) < dark_limit {
                dir = (v - a).signum();
                out.push(t);
                anchor = Some(v);
            }
        } else if (v - a) * dir > 0.0 {
            anchor = Some(v); // still going the same way: the turning point moves with it
        } else if (a - v).abs() >= min && a.min(v) < dark_limit {
            dir = -dir;
            out.push(t);
            anchor = Some(v);
        }
    }
    out
}

/// More than three flashes (pairs of opposing changes) in any one-second window?
fn too_many(changes: &[f64]) -> bool {
    let mut flashes = Vec::new();
    let mut i = 1;
    while i < changes.len() {
        flashes.push(changes[i]);
        i += 2;
    }
    (0..flashes.len()).any(|i| flashes[i..].iter().take_while(|&&f| f - flashes[i] < 1.0).count() > MAX_FLASHES_PER_SECOND)
}

/// Whether `frames` flash. `looping`: the animation repeats (a GIF), so a flash across the loop point counts too.
pub fn is_flashing(frames: &[Frame], looping: bool) -> bool {
    if frames.len() < 3 {
        return false;
    }
    let seq: Vec<Frame> = if looping { frames.iter().chain(frames.iter()).copied().collect() } else { frames.to_vec() };
    let mut t = 0.0;
    let timed: Vec<(f64, &Frame)> = seq
        .iter()
        .map(|f| {
            let at = t;
            t += f.seconds.max(0.01);
            (at, f)
        })
        .collect();
    let bright = changes(timed.iter().map(|(t, f)| (*t, f.luminance)), MIN_CHANGE, DARK_LIMIT);
    let red = changes(timed.iter().map(|(t, f)| (*t, f.red)), MIN_RED_CHANGE, f64::INFINITY);
    too_many(&bright) || too_many(&red)
}

/// Relative luminance of an sRGB colour (0–255 channels).
pub fn luminance(r: u8, g: u8, b: u8) -> f64 {
    fn lin(c: u8) -> f64 {
        let s = c as f64 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    }
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// Whether a pixel counts as saturated red (WCAG's red-flash definition, simplified).
pub fn is_saturated_red(r: u8, g: u8, b: u8) -> bool {
    r > 128 && r as f64 / (r as f64 + g as f64 + b as f64 + 1.0) >= 0.8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(l: f64, s: f64) -> Frame {
        Frame::new(l, 0.0, s)
    }

    #[test]
    fn strobe_is_flashing() {
        let frames: Vec<Frame> = (0..20).map(|i| f(if i % 2 == 0 { 0.0 } else { 1.0 }, 0.05)).collect();
        assert!(is_flashing(&frames, true));
        assert!(is_flashing(&frames, false));
    }

    #[test]
    fn slow_pulse_is_fine() {
        // Dark to bright and back once a second: one flash a second.
        let frames: Vec<Frame> = (0..40).map(|i| f(0.5 + 0.4 * (i as f64 / 20.0 * std::f64::consts::TAU).sin(), 0.05)).collect();
        assert!(!is_flashing(&frames, true));
    }

    #[test]
    fn gentle_motion_is_fine() {
        let frames: Vec<Frame> = (0..60).map(|i| f(0.4 + 0.03 * ((i * 7919) % 13) as f64 / 13.0, 1.0 / 30.0)).collect();
        assert!(!is_flashing(&frames, false));
    }

    #[test]
    fn bright_only_changes_dont_count() {
        // Both states brighter than the dark limit.
        let frames: Vec<Frame> = (0..20).map(|i| f(if i % 2 == 0 { 0.85 } else { 1.0 }, 0.05)).collect();
        assert!(!is_flashing(&frames, true));
    }

    #[test]
    fn red_flash_is_flashing() {
        let frames: Vec<Frame> = (0..20).map(|i| Frame::new(0.2, if i % 2 == 0 { 0.0 } else { 0.6 }, 0.05)).collect();
        assert!(is_flashing(&frames, true));
    }

    #[test]
    fn still_images_and_luminance() {
        assert!(!is_flashing(&[f(0.5, 1.0)], true));
        assert!((luminance(0, 0, 0)).abs() < 1e-6);
        assert!((luminance(255, 255, 255) - 1.0).abs() < 1e-6);
        assert!(is_saturated_red(240, 10, 10));
        assert!(!is_saturated_red(240, 200, 200));
    }

    #[test]
    fn a_ramp_in_small_steps_still_counts() {
        // 0 → 0.4 → 0 in steps of 0.08, five times a second: each step is under 10%, the swing is not.
        let mut frames = Vec::new();
        for _ in 0..10 {
            for k in 0..6 {
                frames.push(f(k as f64 * 0.08, 1.0 / 60.0));
            }
            for k in (0..6).rev() {
                frames.push(f(k as f64 * 0.08, 1.0 / 60.0));
            }
        }
        assert!(is_flashing(&frames, false));
    }
}
