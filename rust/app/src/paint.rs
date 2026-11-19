//! Drawing into RGBA8 pictures for theme scenes: anti-aliased rounded rectangles, ellipses, soft radial lights and
//! thick lines, gradient fills, 90s bevels and crisp pixel art. Pictures are premultiplied; over an opaque picture
//! (the backdrop) that's the same as straight alpha.

use image::RgbaImage;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// A colour (straight RGBA, 0–1).
pub type Rgba = [f32; 4];

/// A fill: one colour, or gradient stops along an angle (`v` down, `h` across, `d` corner to corner).
#[derive(Clone, Debug)]
pub enum Paint {
    Solid(Rgba),
    Stops(Vec<(f32, Rgba)>, u8),
}

impl Paint {
    /// The colour at (u, v) across the shape's box (0–1 each); radial lights sample by distance from the middle.
    pub fn at(&self, u: f32, v: f32) -> Rgba {
        match self {
            Paint::Solid(c) => *c,
            Paint::Stops(s, angle) => sample_stops(
                s,
                match angle {
                    b'h' => u,
                    b'd' => (u + v) / 2.0,
                    _ => v,
                },
            ),
        }
    }
}

pub fn sample_stops(s: &[(f32, Rgba)], t: f32) -> Rgba {
    if s.is_empty() {
        return [0.0; 4];
    }
    if t <= s[0].0 {
        return s[0].1;
    }
    for w in s.windows(2) {
        if t <= w[1].0 {
            let k = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
            let (a, b) = (w[0].1, w[1].1);
            return [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k, a[3] + (b[3] - a[3]) * k];
        }
    }
    s[s.len() - 1].1
}

/// Mixes two colours: `t` of the way from `a` to `b`.
pub fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

#[derive(Clone, Copy, Debug)]
pub enum Shape {
    /// With a corner radius (px).
    Rect(f32),
    Ellipse,
    /// A soft light: the paint's stops run from the middle (0) to the rim (1).
    Radial,
}

/// A small, steady random sequence (xorshift): scenes look the same every time.
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(seed.wrapping_mul(2_654_435_761).max(1))
    }

    /// 0 to 1.
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// How much of pixel `p` (one px wide) lies within `start`..`start + len`.
fn overlap(p: usize, start: f32, len: f32) -> f32 {
    ((p as f32 + 1.0).min(start + len) - (p as f32).max(start)).clamp(0.0, 1.0)
}

/// An RGBA8 picture, premultiplied.
pub struct Picture {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

impl Picture {
    pub fn new(w: usize, h: usize) -> Picture {
        Picture { w: w.max(1), h: h.max(1), px: vec![0; w.max(1) * h.max(1) * 4] }
    }

    /// An opaque picture of one colour.
    pub fn filled(w: usize, h: usize, c: [u8; 3]) -> Picture {
        let mut p = Picture::new(w, h);
        for px in p.px.as_chunks_mut::<4>().0 {
            *px = [c[0], c[1], c[2], 255];
        }
        p
    }

    /// Lays colour `c` (straight) at coverage `a` over pixel `i`.
    #[inline]
    pub fn blend(&mut self, i: usize, c: [f32; 3], a: f32) {
        if a <= 0.002 {
            return;
        }
        let a = a.min(1.0);
        let p = &mut self.px[i * 4..i * 4 + 4];
        for (d, s) in p.iter_mut().zip(c) {
            *d = (s * 255.0 * a + *d as f32 * (1.0 - a) + 0.5) as u8;
        }
        p[3] = (a * 255.0 + p[3] as f32 * (1.0 - a) + 0.5) as u8;
    }

    /// Lays a premultiplied RGBA8 pixel `s` at extra opacity `o` (0–256) over pixel `i`. Integer arithmetic: this is
    /// the motion layer's inner loop.
    #[inline]
    pub fn over_pre(&mut self, i: usize, s: &[u8], o: u32) {
        let a = (s[3] as u32 * o) >> 8;
        if a == 0 {
            return;
        }
        let inv = 255 - a;
        let p = &mut self.px[i * 4..i * 4 + 4];
        for (d, &v) in p.iter_mut().zip(&s[..3]) {
            *d = (((v as u32 * o) >> 8) + div255(*d as u32 * inv)).min(255) as u8;
        }
        p[3] = (a + div255(p[3] as u32 * inv)).min(255) as u8;
    }

    /// `over_pre` along a run: the premultiplied RGBA8 pixels `src` laid over the pixels from `i` on, at extra
    /// opacity `o` (0–256), skipping source pixels fainter than `least`. Same arithmetic, without a bounds check
    /// or a wrap per pixel: the full-window image layers of the motion picture go through here.
    #[inline]
    pub fn over_pre_run(&mut self, i: usize, src: &[u8], o: u32, least: u8) {
        let n = src.len() / 4;
        let dst = &mut self.px[i * 4..(i + n) * 4];
        for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0) {
            if s[3] < least {
                continue;
            }
            let a = (s[3] as u32 * o) >> 8;
            if a == 0 {
                continue;
            }
            let inv = 255 - a;
            for (dv, &sv) in d[..3].iter_mut().zip(&s[..3]) {
                *dv = (((sv as u32 * o) >> 8) + div255(*dv as u32 * inv)).min(255) as u8;
            }
            d[3] = (a + div255(d[3] as u32 * inv)).min(255) as u8;
        }
    }

    /// Fills `shape` in the box (px), anti-aliased, with `paint` at `alpha`.
    pub fn fill(&mut self, b: Rect, shape: Shape, paint: &Paint, alpha: f32) {
        if b.w <= 0.0 || b.h <= 0.0 || alpha <= 0.002 {
            return;
        }
        let (x0, y0) = (b.x.floor().max(0.0) as usize, b.y.floor().max(0.0) as usize);
        let (x1, y1) = (((b.x + b.w).ceil().max(0.0) as usize).min(self.w), ((b.y + b.h).ceil().max(0.0) as usize).min(self.h));
        let (hw, hh) = (b.w / 2.0, b.h / 2.0);
        let (cx, cy) = (b.x + hw, b.y + hh);
        if let Shape::Radial = shape {
            // Soft round lights (the aurora's blobs, passing lights) are big: only the part of each row inside the
            // circle, and the colour from a table of the stops rather than sampled per pixel.
            const STEPS: usize = 1024;
            let table: Vec<Rgba> = (0..=STEPS).map(|i| paint.at(i as f32 / STEPS as f32, i as f32 / STEPS as f32)).collect();
            for py in y0..y1 {
                let ny = (py as f32 + 0.5 - cy) / hh;
                let rest = 1.0 - ny * ny;
                if rest < 0.0 {
                    continue;
                }
                let half = hw * rest.sqrt();
                let from = ((cx - half - 1.0).floor().max(x0 as f32)) as usize;
                let to = ((cx + half + 1.0).ceil().max(0.0) as usize).min(x1);
                for px in from..to {
                    let nx = (px as f32 + 0.5 - cx) / hw;
                    let k = (nx * nx + ny * ny).sqrt();
                    if k > 1.0 {
                        continue;
                    }
                    let c = table[(k * STEPS as f32 + 0.5) as usize];
                    self.blend(py * self.w + px, [c[0], c[1], c[2]], c[3] * alpha);
                }
            }
            return;
        }
        for py in y0..y1 {
            let y = py as f32 + 0.5;
            for px in x0..x1 {
                let x = px as f32 + 0.5;
                let (cov, c) = match shape {
                    Shape::Rect(r) if r <= 0.0 => (overlap(px, b.x, b.w) * overlap(py, b.y, b.h), paint.at((x - b.x) / b.w, (y - b.y) / b.h)),
                    Shape::Rect(r) => {
                        let r = r.min(hw).min(hh);
                        let (qx, qy) = ((x - cx).abs() - (hw - r), (y - cy).abs() - (hh - r));
                        let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
                        ((0.5 - d).clamp(0.0, 1.0), paint.at((x - b.x) / b.w, (y - b.y) / b.h))
                    }
                    Shape::Ellipse => {
                        let k = ((x - cx) / hw).hypot((y - cy) / hh);
                        (((1.0 - k) * hw.min(hh) + 0.5).clamp(0.0, 1.0), paint.at((x - b.x) / b.w, (y - b.y) / b.h))
                    }
                    Shape::Radial => unreachable!("drawn above"),
                };
                if cov > 0.0 {
                    self.blend(py * self.w + px, [c[0], c[1], c[2]], c[3] * alpha * cov);
                }
            }
        }
    }

    /// A line `width` px thick from (x0, y0) to (x1, y1), anti-aliased. Level and upright thin lines sit on whole
    /// pixels, so they're crisp.
    #[allow(clippy::too_many_arguments)]
    pub fn line(&mut self, mut x0: f32, mut y0: f32, mut x1: f32, mut y1: f32, width: f32, c: Rgba, alpha: f32) {
        if alpha <= 0.002 || width <= 0.0 {
            return;
        }
        if (y1 - y0).abs() < 0.01 && width <= 1.5 {
            y0 = y0.floor() + 0.5;
            y1 = y0;
        } else if (x1 - x0).abs() < 0.01 && width <= 1.5 {
            x0 = x0.floor() + 0.5;
            x1 = x0;
        }
        let hw = width / 2.0;
        let pad = hw + 1.0;
        let (bx0, by0) = ((x0.min(x1) - pad).floor().max(0.0) as usize, (y0.min(y1) - pad).floor().max(0.0) as usize);
        let (bx1, by1) = (((x0.max(x1) + pad).ceil().max(0.0) as usize).min(self.w), ((y0.max(y1) + pad).ceil().max(0.0) as usize).min(self.h));
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len2 = (dx * dx + dy * dy).max(1e-6);
        for py in by0..by1 {
            let y = py as f32 + 0.5;
            for px in bx0..bx1 {
                let x = px as f32 + 0.5;
                let t = (((x - x0) * dx + (y - y0) * dy) / len2).clamp(0.0, 1.0);
                let d = (x - (x0 + dx * t)).hypot(y - (y0 + dy * t));
                let cov = (hw + 0.5 - d).clamp(0.0, 1.0).min(width.max(0.0));
                if cov > 0.0 {
                    self.blend(py * self.w + px, [c[0], c[1], c[2]], c[3] * alpha * cov);
                }
            }
        }
    }

    /// A 90s 3D edge: light top and left, dark bottom and right (sunken: the reverse), `t` px wide, on whole pixels.
    #[allow(clippy::too_many_arguments)]
    pub fn bevel(&mut self, b: Rect, fill: Option<Rgba>, light: Rgba, dark: Rgba, t: usize, sunken: bool, outline: Option<Rgba>, alpha: f32) {
        let (x0, y0) = (b.x.round() as i64, b.y.round() as i64);
        let (w, h) = (b.w.round().max(1.0) as i64, b.h.round().max(1.0) as i64);
        let (lo, hi) = if sunken { (dark, light) } else { (light, dark) };
        let t = t as i64;
        for v in 0..h {
            let py = y0 + v;
            if py < 0 || py >= self.h as i64 {
                continue;
            }
            for u in 0..w {
                let px = x0 + u;
                if px < 0 || px >= self.w as i64 {
                    continue;
                }
                let edge = u == 0 || v == 0 || u == w - 1 || v == h - 1;
                let c = match outline {
                    Some(o) if edge => Some(o),
                    _ if u >= w - t || v >= h - t => Some(hi),
                    _ if u < t || v < t => Some(lo),
                    _ => fill,
                };
                if let Some(c) = c {
                    self.blend(py as usize * self.w + px as usize, [c[0], c[1], c[2]], c[3] * alpha);
                }
            }
        }
    }

    /// Pixel art: each character of `rows` a `cell`-px block in its palette colour (None is clear). Crisp: blocks
    /// fall on whole pixels.
    pub fn pixels(&mut self, x: f32, y: f32, rows: &[Vec<Option<Rgba>>], cell: f32, alpha: f32) {
        for (ry, row) in rows.iter().enumerate() {
            let (top, bottom) = ((y + ry as f32 * cell).round() as i64, (y + (ry + 1) as f32 * cell).round() as i64);
            for (rx, c) in row.iter().enumerate() {
                let Some(c) = c else { continue };
                let (left, right) = ((x + rx as f32 * cell).round() as i64, (x + (rx + 1) as f32 * cell).round() as i64);
                for py in top.max(0)..bottom.min(self.h as i64) {
                    for px in left.max(0)..right.min(self.w as i64) {
                        self.blend(py as usize * self.w + px as usize, [c[0], c[1], c[2]], c[3] * alpha);
                    }
                }
            }
        }
    }
}

/// Frosts a band of rows (`y0`..`y1`, columns `x0`..`x1`): three box blurs of radius `r` each way (about a
/// Gaussian of σ ≈ r), the colour only. Running sums, so it's quick even for the whole window.
pub fn frost(p: &mut Picture, x0: usize, y0: usize, x1: usize, y1: usize, r: usize) {
    let (x1, y1) = (x1.min(p.w), y1.min(p.h));
    if r == 0 || x1 <= x0 + 1 || y1 <= y0 + 1 {
        return;
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let mut buf: Vec<[f32; 3]> = Vec::with_capacity(w * h);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = (y * p.w + x) * 4;
            buf.push([p.px[i] as f32, p.px[i + 1] as f32, p.px[i + 2] as f32]);
        }
    }
    let pass = |line: &mut [[f32; 3]], tmp: &mut Vec<[f32; 3]>| {
        let n = line.len();
        tmp.clear();
        tmp.extend_from_slice(line);
        let at = |i: isize| tmp[i.clamp(0, n as isize - 1) as usize];
        let mut sum = [0.0f32; 3];
        let ri = r as isize;
        for i in -ri..=ri {
            let c = at(i);
            for k in 0..3 {
                sum[k] += c[k];
            }
        }
        let div = (2 * r + 1) as f32;
        for (i, out) in line.iter_mut().enumerate() {
            *out = [sum[0] / div, sum[1] / div, sum[2] / div];
            let (add, sub) = (at(i as isize + ri + 1), at(i as isize - ri));
            for k in 0..3 {
                sum[k] += add[k] - sub[k];
            }
        }
    };
    let mut tmp = Vec::new();
    let mut col = vec![[0.0f32; 3]; h];
    for _ in 0..3 {
        for row in buf.chunks_exact_mut(w) {
            pass(row, &mut tmp);
        }
        for x in 0..w {
            for y in 0..h {
                col[y] = buf[y * w + x];
            }
            pass(&mut col, &mut tmp);
            for y in 0..h {
                buf[y * w + x] = col[y];
            }
        }
    }
    for y in y0..y1 {
        for x in x0..x1 {
            let i = (y * p.w + x) * 4;
            let c = buf[(y - y0) * w + (x - x0)];
            p.px[i] = c[0].round().clamp(0.0, 255.0) as u8;
            p.px[i + 1] = c[1].round().clamp(0.0, 255.0) as u8;
            p.px[i + 2] = c[2].round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// x / 255, rounded, for x up to 255 × 255.
#[inline]
pub fn div255(x: u32) -> u32 {
    (x + 128 + ((x + 128) >> 8)) >> 8
}

/// An opacity as a fraction of 256.
pub fn fixed(o: f32) -> u32 {
    (o.clamp(0.0, 1.0) * 256.0).round() as u32
}

/// Premultiplies a picture's colours by its alpha (once, when it's scaled).
pub fn premultiply(img: &mut RgbaImage) {
    for p in img.pixels_mut() {
        let a = p.0[3] as u16;
        for v in &mut p.0[..3] {
            *v = ((*v as u16 * a + 127) / 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn radial_fill_matches_sampling_each_pixel() {
        let paint = Paint::Stops(vec![(0.0, [0.9, 0.5, 0.2, 0.8]), (0.35, [0.9, 0.5, 0.2, 0.4]), (1.0, [0.9, 0.5, 0.2, 0.0])], b'v');
        for b in [Rect { x: 3.3, y: -4.0, w: 61.0, h: 47.5 }, Rect { x: -20.0, y: 10.2, w: 140.0, h: 140.0 }] {
            let mut fast = Picture::filled(90, 70, [20, 30, 40]);
            fast.fill(b, Shape::Radial, &paint, 0.9);
            // The old way: every pixel of the box, the stops sampled at its distance.
            let mut slow = Picture::filled(90, 70, [20, 30, 40]);
            let (hw, hh) = (b.w / 2.0, b.h / 2.0);
            let (cx, cy) = (b.x + hw, b.y + hh);
            for py in 0..70 {
                for px in 0..90 {
                    let k = ((px as f32 + 0.5 - cx) / hw).hypot((py as f32 + 0.5 - cy) / hh);
                    if k <= 1.0 {
                        let c = paint.at(k, k);
                        slow.blend(py * 90 + px, [c[0], c[1], c[2]], c[3] * 0.9);
                    }
                }
            }
            let worst = fast.px.iter().zip(&slow.px).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            assert!(worst <= 1, "differs by {worst}");
        }
    }

    #[test]
    fn over_pre_run_matches_over_pre() {
        // Pseudo-random premultiplied pixels over a pseudo-random picture, at a few opacities and thresholds.
        let mut seed = 0x1234_5678u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for (o, least) in [(256, 0u8), (180, 0), (77, 9), (256, 3)] {
            let (w, h) = (37, 5);
            let mut a = Picture::new(w, h);
            for v in a.px.iter_mut() {
                *v = next() as u8;
            }
            let mut b = Picture { w, h, px: a.px.clone() };
            let src: Vec<u8> = (0..w * h)
                .flat_map(|_| {
                    let al = next() as u8;
                    let c = |x: u32| ((x as u8 as u32 * al as u32) / 255) as u8;
                    [c(next()), c(next()), c(next()), al]
                })
                .collect();
            for i in 0..w * h {
                if src[i * 4 + 3] >= least {
                    a.over_pre(i, &src[i * 4..i * 4 + 4], o);
                }
            }
            b.over_pre_run(0, &src, o, least);
            assert_eq!(a.px, b.px, "o {o} least {least}");
        }
    }

    use super::*;

    #[test]
    fn a_thin_level_line_is_crisp() {
        let mut p = Picture::filled(20, 10, [0, 0, 0]);
        p.line(0.0, 4.0, 20.0, 4.0, 1.0, [1.0, 1.0, 1.0, 1.0], 1.0);
        let row = |y: usize| p.px[(y * 20 + 10) * 4];
        assert_eq!(row(4), 255);
        assert_eq!(row(3), 0);
        assert_eq!(row(5), 0);
    }

    #[test]
    fn shapes_are_smooth_at_the_edge_and_solid_inside() {
        let mut p = Picture::filled(40, 40, [0, 0, 0]);
        p.fill(Rect { x: 4.3, y: 4.3, w: 31.4, h: 31.4 }, Shape::Ellipse, &Paint::Solid([1.0, 1.0, 1.0, 1.0]), 1.0);
        let at = |x: usize, y: usize| p.px[(y * 40 + x) * 4];
        assert_eq!(at(20, 20), 255);
        assert_eq!(at(1, 1), 0);
        let edge: Vec<u8> = (0..40).map(|x| at(x, 20)).collect();
        assert!(edge.iter().any(|&v| v > 20 && v < 235), "an anti-aliased rim: {edge:?}");
    }

    #[test]
    fn a_bevel_is_light_above_and_dark_below() {
        let mut p = Picture::filled(10, 10, [0, 0, 0]);
        let (white, grey) = ([1.0, 1.0, 1.0, 1.0], [0.5, 0.5, 0.5, 1.0]);
        p.bevel(Rect { x: 0.0, y: 0.0, w: 10.0, h: 10.0 }, Some([0.75, 0.75, 0.75, 1.0]), white, grey, 2, false, None, 1.0);
        let at = |x: usize, y: usize| p.px[(y * 10 + x) * 4];
        assert_eq!(at(5, 0), 255);
        assert_eq!(at(5, 9), 128);
        assert_eq!(at(5, 5), 191);
    }
}
