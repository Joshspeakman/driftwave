//! A float RGB canvas with antialiased drawing. Visualizers draw at the slot's real pixel size, and every shape is
//! splatted with sub-pixel weights, so lines and dots stay smooth at any size (nothing is ever stair-stepped).

pub type Rgb = [f32; 3];

#[derive(Debug, Clone)]
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<Rgb>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas { w: w.max(1), h: h.max(1), px: vec![[0.0; 3]; w.max(1) * h.max(1)] }
    }

    pub fn resize(&mut self, w: usize, h: usize) {
        if w.max(1) != self.w || h.max(1) != self.h {
            *self = Canvas::new(w, h);
        }
    }

    pub fn clear(&mut self, c: Rgb) {
        self.px.fill(c);
    }

    /// Multiplies everything by `k` (trails that fade).
    pub fn fade(&mut self, k: f32) {
        for p in &mut self.px {
            p[0] *= k;
            p[1] *= k;
            p[2] *= k;
        }
    }

    /// A vertical gradient from `top` to `bottom`.
    pub fn gradient(&mut self, top: Rgb, bottom: Rgb) {
        for y in 0..self.h {
            let t = y as f32 / (self.h - 1).max(1) as f32;
            let c = mix(top, bottom, t);
            self.px[y * self.w..(y + 1) * self.w].fill(c);
        }
    }

    #[inline]
    pub fn add_px(&mut self, x: i32, y: i32, c: Rgb, a: f32) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 {
            return;
        }
        let p = &mut self.px[y as usize * self.w + x as usize];
        p[0] += c[0] * a;
        p[1] += c[1] * a;
        p[2] += c[2] * a;
    }

    /// Adds a point at a sub-pixel position, shared between its four pixels.
    #[inline]
    pub fn splat(&mut self, x: f32, y: f32, c: Rgb, a: f32) {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i32, y0 as i32);
        self.add_px(x0, y0, c, a * (1.0 - tx) * (1.0 - ty));
        self.add_px(x0 + 1, y0, c, a * tx * (1.0 - ty));
        self.add_px(x0, y0 + 1, c, a * (1.0 - tx) * ty);
        self.add_px(x0 + 1, y0 + 1, c, a * tx * ty);
    }

    /// A soft round glow of radius `r`: a smooth bump, (1 − d²/(2r)²)², close to a Gaussian and much cheaper.
    pub fn glow(&mut self, x: f32, y: f32, r: f32, c: Rgb, a: f32) {
        if r <= 0.75 {
            return self.splat(x, y, c, a);
        }
        let reach = 2.0 * r;
        let inv = 1.0 / (reach * reach);
        let (x0, x1) = (((x - reach).floor() as i32).max(0), ((x + reach).ceil() as i32).min(self.w as i32 - 1));
        let (y0, y1) = (((y - reach).floor() as i32).max(0), ((y + reach).ceil() as i32).min(self.h as i32 - 1));
        for py in y0..=y1 {
            let ddy = py as f32 + 0.5 - y;
            let row = py as usize * self.w;
            for px in x0..=x1 {
                let ddx = px as f32 + 0.5 - x;
                let q = 1.0 - (ddx * ddx + ddy * ddy) * inv;
                if q > 0.0 {
                    let wgt = a * q * q;
                    let p = &mut self.px[row + px as usize];
                    p[0] += c[0] * wgt;
                    p[1] += c[1] * wgt;
                    p[2] += c[2] * wgt;
                }
            }
        }
    }

    /// An antialiased line. `width` over 1 adds a soft glow around a crisp core.
    #[allow(clippy::too_many_arguments)] // two points, colour, alpha and width: grouping them helps no caller
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb, a: f32, width: f32) {
        let len = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
        let steps = (len * 2.0).ceil().max(1.0) as usize;
        let a_core = a / 2.0; // two samples per pixel of length
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            self.splat(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, c, a_core);
        }
        if width > 1.0 {
            // Glow stamps every half radius (they overlap smoothly), weighted by the spacing.
            let spacing = (width * 0.5).max(1.0);
            let gsteps = (len / spacing).ceil().max(1.0) as usize;
            let ag = a * 0.35 / width * (len / gsteps as f32).max(0.5);
            for i in 0..=gsteps {
                let t = i as f32 / gsteps as f32;
                self.glow(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, width, c, ag);
            }
        }
    }

    /// A filled rectangle with antialiased edges (fractional coordinates), set rather than added.
    pub fn fill_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb) {
        let (ix0, iy0) = (x0.floor().max(0.0) as usize, y0.floor().max(0.0) as usize);
        let (ix1, iy1) = ((x1.ceil() as usize).min(self.w), (y1.ceil() as usize).min(self.h));
        for y in iy0..iy1 {
            let cov_y = (y1.min(y as f32 + 1.0) - y0.max(y as f32)).clamp(0.0, 1.0);
            for x in ix0..ix1 {
                let cov = cov_y * (x1.min(x as f32 + 1.0) - x0.max(x as f32)).clamp(0.0, 1.0);
                let p = &mut self.px[y * self.w + x];
                *p = mix(*p, c, cov);
            }
        }
    }

    /// Bilinear sample, clamped at the edges.
    #[inline]
    pub fn sample(&self, x: f32, y: f32) -> Rgb {
        let fx = (x - 0.5).clamp(0.0, (self.w - 1) as f32);
        let fy = (y - 0.5).clamp(0.0, (self.h - 1) as f32);
        let (x0, y0) = (fx as usize, fy as usize);
        let (x1, y1) = ((x0 + 1).min(self.w - 1), (y0 + 1).min(self.h - 1));
        let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
        let p = |x: usize, y: usize| self.px[y * self.w + x];
        let top = mix(p(x0, y0), p(x1, y0), tx);
        let bottom = mix(p(x0, y1), p(x1, y1), tx);
        mix(top, bottom, ty)
    }

    /// To 8-bit RGBA, with a soft shoulder so bright overlaps glow instead of clipping flat.
    pub fn to_rgba8(&self, out: &mut Vec<u8>) {
        static TABLE: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
        // 0..4 in steps of 1/1024.
        let table = TABLE.get_or_init(|| (0..4096).map(|i| (shoulder(i as f32 / 1024.0) * 255.0 + 0.5) as u8).collect());
        out.resize(self.w * self.h * 4, 255);
        for (o, p) in out.as_chunks_mut::<4>().0.iter_mut().zip(self.px.iter()) {
            o[0] = table[((p[0].max(0.0) * 1024.0) as usize).min(4095)];
            o[1] = table[((p[1].max(0.0) * 1024.0) as usize).min(4095)];
            o[2] = table[((p[2].max(0.0) * 1024.0) as usize).min(4095)];
            o[3] = 255;
        }
    }
}

/// Whether an 8-bit colour sits in (or at the edge of) a banned band: a colour with a visible hue between teal's and
/// purple-to-pink's edges, or a pale red.
#[inline]
fn near_banned(r: i32, g: i32, b: i32) -> bool {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let c = max - min;
    if c < 12 {
        return false;
    }
    let h = if max == r {
        (60 * (g - b) / c).rem_euclid(360)
    } else if max == g {
        60 * (b - r) / c + 120
    } else {
        60 * (r - g) / c + 240
    };
    (148..=207).contains(&h) || (223..=352).contains(&h) || ((h >= 334 || h < 12) && max + min > 300)
}

/// The colour rule at the output (RGBA8, in place): blends, glows and additive light can make teal, purple or pink
/// from allowed colours (blue over amber is lavender), so every frame's pixels pass through `hues::safe_hsv` last.
/// Quick: a table of 32 × 32 × 32 colour cells says which cells come near a banned band, so nearly every pixel costs
/// one lookup.
pub fn keep_the_rule(px: &mut [u8]) {
    static CELLS: std::sync::OnceLock<Vec<bool>> = std::sync::OnceLock::new();
    let cells = CELLS.get_or_init(|| {
        let mut t = vec![false; 32 * 32 * 32];
        for (i, cell) in t.iter_mut().enumerate() {
            let (r0, g0, b0) = (((i >> 10) & 31) as i32 * 8, ((i >> 5) & 31) as i32 * 8, (i & 31) as i32 * 8);
            // The corners and the middle of the cell.
            *cell = [(0, 0, 0), (7, 0, 0), (0, 7, 0), (0, 0, 7), (7, 7, 0), (7, 0, 7), (0, 7, 7), (7, 7, 7), (4, 4, 4)]
                .iter()
                .any(|&(dr, dg, db)| near_banned(r0 + dr, g0 + dg, b0 + db));
        }
        t
    });
    for p in px.as_chunks_mut::<4>().0 {
        if !cells[((p[0] as usize >> 3) << 10) | ((p[1] as usize >> 3) << 5) | (p[2] as usize >> 3)] {
            continue;
        }
        let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
        if !near_banned(r, g, b) {
            continue;
        }
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        let c = (max - min) as f32 / 255.0;
        let (rf, gf, bf) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
        let v = max as f32 / 255.0;
        let hf = if max == r {
            ((gf - bf) / c).rem_euclid(6.0)
        } else if max == g {
            (bf - rf) / c + 2.0
        } else {
            (rf - gf) / c + 4.0
        } * 60.0;
        let (h2, s2) = driftwave_core::hues::safe_hsv(hf, c / v, v);
        let o = hsv_raw(h2 / 360.0, s2, v);
        p[0] = (o[0] * 255.0 + 0.5) as u8;
        p[1] = (o[1] * 255.0 + 0.5) as u8;
        p[2] = (o[2] * 255.0 + 0.5) as u8;
    }
}

#[inline]
fn shoulder(v: f32) -> f32 {
    let v = v.max(0.0);
    if v < 0.8 { v } else { 0.8 + 0.2 * (1.0 - (-(v - 0.8) / 0.2).exp()) }
}

#[inline]
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// A colour moved out of teal, purple and pink (the colour rule), keeping its brightness: for blends between
/// palettes, where two allowed colours can meet in a banned one (blue and green make teal).
pub fn safe(c: Rgb) -> Rgb {
    let (max, min) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let d = max - min;
    if max <= 1e-5 || d <= 1e-5 {
        return c;
    }
    let h = if max == c[0] {
        ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if max == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    } * 60.0;
    let (h2, s2) = driftwave_core::hues::safe_hsv(h, d / max, max);
    if h2 == h && s2 == d / max {
        return c;
    }
    hsv_raw(h2 / 360.0, s2, max)
}

/// Hue (0–1, wrapping), saturation and value to RGB. The colour rule (no teal, purple or pink) applies: a sweep
/// round the hues spends its time on crimson to green and azure to blue, dipping briefly through silver between.
pub fn hsv(h: f32, s: f32, v: f32) -> Rgb {
    let (h, s) = driftwave_core::hues::safe_hsv(driftwave_core::hues::spread(h.rem_euclid(1.0) * 360.0), s, v);
    hsv_raw(h / 360.0, s, v)
}

/// Hue, saturation and value to RGB without the colour rule (tests make banned colours with it on purpose).
pub fn hsv_unruled(h: f32, s: f32, v: f32) -> Rgb {
    hsv_raw(h, s, v)
}

fn hsv_raw(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i as i32 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// Scales a colour.
#[inline]
pub fn scale(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}
