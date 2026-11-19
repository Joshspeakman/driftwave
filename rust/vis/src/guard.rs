//! The flash guard: nothing a visualizer draws reaches the screen flashing.
//!
//! The frame is divided into regions about the size WCAG 2.3.1 measures flashes over (170 × 128 logical pixels,
//! half its 341 × 256 "small area", to be safe). In each region, average brightness (relative luminance, in linear
//! light) may change only so fast: [`LUMA_PER_SECOND`]. A region that changes faster is eased towards the new frame
//! instead.
//!
//! Red is handled more simply: no saturated red ever leaves the guard. A pixel at WCAG's saturated-red threshold is
//! nudged just under it (pure red becomes 255, 33, 32), so a red flash can't happen at all.
//!
//! At that rate a 10% swing takes at least 0.2 s, so an up-and-down pair takes 0.4 s: at most 2.5 flashes a second,
//! under the limit of 3. A kick drum becomes a swell, a cut to white becomes a fade, and switching visualizers becomes
//! a crossfade. Moving shapes are hardly touched: they change a region's average little.
//!
//! The guard eases per pixel with a weight blended between regions. Each region's weight is first lowered to the
//! smallest of its neighbours', so a blend never lets a region change faster than its own limit. When no region is
//! changing too fast (most frames), the frame passes straight through.

use driftwave_core::flash;

pub const LUMA_PER_SECOND: f32 = 0.5;
/// Region size in logical pixels (multiplied by the display scale).
pub const REGION: (f32, f32) = (170.0, 128.0);

const SRGB_STEPS: usize = 4096;

pub struct FlashGuard {
    w: usize,
    h: usize,
    gx: usize,
    gy: usize,
    /// What's on screen now, in linear light.
    shown: Vec<[f32; 3]>,
    /// Each region's summed luminance as shown.
    shown_luma: Vec<f32>,
    /// Pixels in each region.
    count: Vec<f32>,
    to_linear: [f32; 256],
    /// Luminance contributions of each 8-bit channel (linear × its coefficient).
    luma: [[f32; 256]; 3],
    to_srgb: Vec<u8>,
    col_cell: Vec<usize>,
    row_cell: Vec<usize>,
    /// For blending weights between region centres: (index, next index, fraction) per column and per row.
    col_mix: Vec<(usize, usize, f32)>,
    row_mix: Vec<(usize, usize, f32)>,
}

impl FlashGuard {
    pub fn new(w: usize, h: usize, scale: f32) -> FlashGuard {
        let to_linear: [f32; 256] = std::array::from_fn(|i| {
            let s = i as f32 / 255.0;
            if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        });
        let coef = [0.2126f32, 0.7152, 0.0722];
        let luma = std::array::from_fn(|c| std::array::from_fn(|i| to_linear[i] * coef[c]));
        let to_srgb = (0..SRGB_STEPS)
            .map(|i| {
                let l = i as f32 / (SRGB_STEPS - 1) as f32;
                let s = if l <= 0.0031308 { l * 12.92 } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
                (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8
            })
            .collect();
        let (w, h) = (w.max(1), h.max(1));
        let (rw, rh) = (REGION.0 * scale.max(0.5), REGION.1 * scale.max(0.5));
        let gx = ((w as f32 / rw).round() as usize).max(1);
        let gy = ((h as f32 / rh).round() as usize).max(1);
        let mixes = |n: usize, g: usize| -> Vec<(usize, usize, f32)> {
            (0..n)
                .map(|p| {
                    let f = ((p as f32 + 0.5) / n as f32 * g as f32 - 0.5).clamp(0.0, (g - 1) as f32);
                    (f as usize, (f as usize + 1).min(g - 1), f.fract())
                })
                .collect()
        };
        let col_cell: Vec<usize> = (0..w).map(|x| (x * gx / w).min(gx - 1)).collect();
        let row_cell: Vec<usize> = (0..h).map(|y| (y * gy / h).min(gy - 1)).collect();
        let mut count = vec![0f32; gx * gy];
        for &r in &row_cell {
            for &c in &col_cell {
                count[r * gx + c] += 1.0;
            }
        }
        FlashGuard {
            w,
            h,
            gx,
            gy,
            shown: vec![[0.0; 3]; w * h],
            shown_luma: vec![0.0; gx * gy],
            count,
            to_linear,
            luma,
            to_srgb,
            col_cell,
            row_cell,
            col_mix: mixes(w, gx),
            row_mix: mixes(h, gy),
        }
    }

    pub fn regions(&self) -> (usize, usize) {
        (self.gx, self.gy)
    }

    /// Eases `rgba` (8-bit, from the visualizer) in place so it can't flash, `dt` seconds after the last frame.
    pub fn apply(&mut self, rgba: &mut [u8], dt: f32) {
        let (w, h, gx, gy) = (self.w, self.h, self.gx, self.gy);
        assert_eq!(rgba.len(), w * h * 4);
        let cells = gx * gy;
        // Each region's summed luminance as drawn.
        let mut drawn = vec![0f32; cells];
        for y in 0..h {
            let row = self.row_cell[y] * gx;
            let line = &rgba[y * w * 4..(y + 1) * w * 4];
            for x in 0..w {
                let p = &line[x * 4..x * 4 + 3];
                drawn[row + self.col_cell[x]] += self.luma[0][p[0] as usize] + self.luma[1][p[1] as usize] + self.luma[2][p[2] as usize];
            }
        }
        let max = LUMA_PER_SECOND * dt.clamp(0.0, 0.1);
        let mut k = vec![1f32; cells];
        for c in 0..cells {
            let d = ((drawn[c] - self.shown_luma[c]) / self.count[c].max(1.0)).abs();
            if d > max {
                k[c] = max / d;
            }
        }
        if k.iter().all(|&v| v >= 1.0) {
            // Nothing changes too fast: the frame goes straight to the screen (red still nudged).
            for (i, s) in self.shown.iter_mut().enumerate() {
                let p = &mut rgba[i * 4..i * 4 + 3];
                *s = [self.to_linear[p[0] as usize], self.to_linear[p[1] as usize], self.to_linear[p[2] as usize]];
                if p[0] > 128 {
                    let n = no_saturated_red([p[0], p[1], p[2]]);
                    p.copy_from_slice(&n);
                }
            }
            self.shown_luma = drawn;
            return;
        }
        // Lower each region's weight to the smallest of its neighbours', so blending can't exceed any limit.
        let mut ke = k.clone();
        for cy in 0..gy {
            for cx in 0..gx {
                let mut m = k[cy * gx + cx];
                for ny in cy.saturating_sub(1)..=(cy + 1).min(gy - 1) {
                    for nx in cx.saturating_sub(1)..=(cx + 1).min(gx - 1) {
                        m = m.min(k[ny * gx + nx]);
                    }
                }
                ke[cy * gx + cx] = m;
            }
        }
        let mut shown_luma = vec![0f32; cells];
        let q = (SRGB_STEPS - 1) as f32;
        for y in 0..h {
            let (y0, y1, ty) = self.row_mix[y];
            let row = self.row_cell[y] * gx;
            for x in 0..w {
                let (x0, x1, tx) = self.col_mix[x];
                let kk = (ke[y0 * gx + x0] * (1.0 - tx) + ke[y0 * gx + x1] * tx) * (1.0 - ty) + (ke[y1 * gx + x0] * (1.0 - tx) + ke[y1 * gx + x1] * tx) * ty;
                let i = y * w + x;
                let s = &mut self.shown[i];
                let p = &mut rgba[i * 4..i * 4 + 3];
                for ch in 0..3 {
                    s[ch] += (self.to_linear[p[ch] as usize] - s[ch]) * kk;
                }
                shown_luma[row + self.col_cell[x]] += 0.2126 * s[0] + 0.7152 * s[1] + 0.0722 * s[2];
                let out = [
                    self.to_srgb[(s[0].clamp(0.0, 1.0) * q) as usize],
                    self.to_srgb[(s[1].clamp(0.0, 1.0) * q) as usize],
                    self.to_srgb[(s[2].clamp(0.0, 1.0) * q) as usize],
                ];
                p.copy_from_slice(&no_saturated_red(out));
            }
        }
        self.shown_luma = shown_luma;
    }
}

/// Nudges a pixel just under WCAG's saturated-red threshold (red at least 80% of r + g + b), lifting green and blue
/// equally; anything else passes untouched.
#[inline]
pub fn no_saturated_red(c: [u8; 3]) -> [u8; 3] {
    if !flash::is_saturated_red(c[0], c[1], c[2]) {
        return c;
    }
    let r = c[0] as u32;
    // Need r / (r + g + b + 1) < 0.8, that is g + b > r / 4 - 1. Lift both a little past it.
    let need = (r / 4 + 2).saturating_sub(c[1] as u32 + c[2] as u32);
    let (g, b) = (c[1] as u32 + need - need / 2, c[2] as u32 + need / 2);
    [c[0], g.min(255) as u8, b.min(255) as u8]
}

/// Measures 8-bit RGBA frames for [`flash::is_flashing`]: the whole frame, and each region of a grid.
pub fn measure(rgba: &[u8], w: usize, h: usize, gx: usize, gy: usize, seconds: f64) -> (flash::Frame, Vec<flash::Frame>) {
    let cells = gx * gy;
    let (mut l, mut r, mut n) = (vec![0f64; cells], vec![0f64; cells], vec![0f64; cells]);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let c = (y * gy / h).min(gy - 1) * gx + (x * gx / w).min(gx - 1);
            l[c] += flash::luminance(rgba[i], rgba[i + 1], rgba[i + 2]);
            r[c] += flash::is_saturated_red(rgba[i], rgba[i + 1], rgba[i + 2]) as u8 as f64;
            n[c] += 1.0;
        }
    }
    let total: f64 = n.iter().sum();
    let whole = flash::Frame::new(l.iter().sum::<f64>() / total, r.iter().sum::<f64>() / total, seconds);
    let each = (0..cells).map(|c| flash::Frame::new(l[c] / n[c].max(1.0), r[c] / n[c].max(1.0), seconds)).collect();
    (whole, each)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_strobe_comes_out_steady() {
        let (w, h) = (340, 256);
        let mut g = FlashGuard::new(w, h, 1.0);
        let (gx, gy) = g.regions();
        let mut whole = Vec::new();
        let mut cells: Vec<Vec<flash::Frame>> = vec![Vec::new(); gx * gy];
        let mut raw = Vec::new();
        for i in 0..90 {
            // Black and white every frame, with a red square every other frame: the worst there is.
            let v = if i % 2 == 0 { 0 } else { 255 };
            let mut px = vec![v; w * h * 4];
            if i % 4 < 2 {
                for y in 20..120 {
                    for x in 20..150 {
                        let p = (y * w + x) * 4;
                        px[p..p + 3].copy_from_slice(&[255, 0, 0]);
                    }
                }
            }
            raw.push(measure(&px, w, h, gx, gy, 1.0 / 30.0).0);
            g.apply(&mut px, 1.0 / 30.0);
            let (f, each) = measure(&px, w, h, gx, gy, 1.0 / 30.0);
            whole.push(f);
            for (c, e) in each.into_iter().enumerate() {
                cells[c].push(e);
            }
        }
        assert!(flash::is_flashing(&raw, false), "the input should flash");
        assert!(whole.iter().all(|f| f.red == 0.0), "saturated red got through");
        assert!(!flash::is_flashing(&whole, false), "guarded output flashes");
        for (c, seq) in cells.iter().enumerate() {
            assert!(!flash::is_flashing(seq, false), "region {c} flashes");
        }
    }

    #[test]
    fn steady_pictures_pass_straight_through() {
        let (w, h) = (200, 100);
        let mut g = FlashGuard::new(w, h, 1.0);
        let frame: Vec<u8> = (0..w * h * 4).map(|i| if i % 4 == 3 { 255 } else { (i * 7 % 120) as u8 }).collect();
        let mut px = frame.clone();
        for _ in 0..120 {
            px.copy_from_slice(&frame);
            g.apply(&mut px, 1.0 / 30.0);
        }
        // After the fade-in, a still picture is shown exactly.
        assert_eq!(px, frame);
    }

    #[test]
    fn red_is_nudged_just_under_the_threshold() {
        for r in [129u8, 200, 255] {
            let c = no_saturated_red([r, 0, 0]);
            assert!(!flash::is_saturated_red(c[0], c[1], c[2]), "{c:?}");
            assert!(c[1] as u32 + c[2] as u32 <= r as u32 / 4 + 3, "too much: {c:?}");
        }
        assert_eq!(no_saturated_red([255, 0, 0]), [255, 33, 32]);
        assert_eq!(no_saturated_red([90, 0, 0]), [90, 0, 0]); // dark red isn't "saturated red"
        assert_eq!(no_saturated_red([10, 200, 30]), [10, 200, 30]);
    }
}
