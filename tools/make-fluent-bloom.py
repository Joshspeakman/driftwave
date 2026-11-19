"""Renders Fluent's backdrop, assets/theme-art/fluent-bloom.jpg: an original bloom of folded translucent petals.

Two rings of petals around a centre, each petal a teardrop shaded like a folded ribbon (lit on one side, in shadow on
the other, its rim catching the light) and running deep blue at its base to pale at its tip, the front ring casting a
soft shadow on the back one. A gentle glow and a soft shoulder keep the centre from blowing out to white. Drawn at
twice the size and scaled down, with a little dither against banding.
Needs Pillow and numpy. Usage: python tools/make-fluent-bloom.py [out.jpg]
"""
import math
import os
import sys

import numpy as np
from PIL import Image, ImageFilter

W, H = 1920, 1080
S = 2  # supersampling
OUT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'assets', 'theme-art', 'fluent-bloom.jpg'))


def rgb(h):
    h = h.lstrip('#')
    return np.array([int(h[i:i + 2], 16) / 255.0 for i in (0, 2, 4)], dtype=np.float32)


def smooth(e0, e1, x):
    t = np.clip((x - e0) / (e1 - e0), 0.0, 1.0)
    return t * t * (3 - 2 * t)


def ramp(t, stops):
    """Colour along t (0..1) through (offset, colour) stops."""
    out = np.zeros(t.shape + (3,), dtype=np.float32)
    for (o1, c1), (o2, c2) in zip(stops, stops[1:]):
        m = (t >= o1) & (t <= o2)
        f = ((t[m] - o1) / (o2 - o1))[:, None]
        out[m] = c1 * (1 - f) + c2 * f
    return out


def blur(a, r):
    """Gaussian blur of a float image (H, W) or (H, W, 3) in 0..1, through 16-bit-safe 8-bit layers."""
    if a.ndim == 2:
        img = Image.fromarray(np.clip(a * 255, 0, 255).astype(np.uint8), 'L')
        return np.asarray(img.filter(ImageFilter.GaussianBlur(r)), dtype=np.float32) / 255.0
    return np.stack([blur(a[..., i], r) for i in range(3)], axis=-1)


def petal(canvas, alpha, cx, cy, ang, length, width, stops, opacity, twist):
    """Composites one petal over `canvas` (H, W, 3) and adds its coverage to `alpha`."""
    w, h = canvas.shape[1], canvas.shape[0]
    ca, sa = math.cos(ang), math.sin(ang)
    # Its bounding box.
    corners = [(cx + ca * u - sa * v, cy + sa * u + ca * v) for u in (0, length) for v in (-width, width)]
    x0 = max(0, int(min(p[0] for p in corners)) - 4)
    x1 = min(w, int(max(p[0] for p in corners)) + 4)
    y0 = max(0, int(min(p[1] for p in corners)) - 4)
    y1 = min(h, int(max(p[1] for p in corners)) + 4)
    if x1 <= x0 or y1 <= y0:
        return
    ys, xs = np.mgrid[y0:y1, x0:x1].astype(np.float32)
    dx, dy = xs - cx, ys - cy
    u = dx * ca + dy * sa
    v = -dx * sa + dy * ca
    t = u / length
    # A teardrop: narrow at the base, widest a little before the middle, coming to a soft point.
    half = width * np.power(np.clip(np.sin(np.pi * np.clip(t, 0, 1)), 0, 1), 1.05) * (1.0 - 0.38 * np.clip(t, 0, 1))
    half = np.maximum(half, 1e-3)
    across = v / half  # -1..1 inside
    edge = 2.5 * S
    cover = smooth(0, 1, (half - np.abs(v)) / edge) * smooth(0, 1, u / edge) * smooth(0, 1, (length - u) / edge)
    if not cover.any():
        return
    col = ramp(np.clip(t, 0, 1), stops)
    # Folded: one side in light, the other in shadow (the fold's side alternates petal to petal).
    side = 0.5 + 0.5 * np.clip(across * twist, -1, 1)
    shade = 0.58 + 0.52 * side
    col = col * shade[..., None]
    # The rim catches the light; a faint crease runs down the middle.
    rim = smooth(0.72, 1.0, np.abs(across)) * smooth(0.05, 0.3, t) * 0.35
    col = col * (1 - rim[..., None]) + rgb('#D6F0FF') * rim[..., None]
    crease = np.exp(-(across * 6.0) ** 2) * smooth(0.1, 0.5, t) * 0.12
    col = col * (1 + crease[..., None])
    a = (cover * opacity)[..., None]
    region = canvas[y0:y1, x0:x1]
    canvas[y0:y1, x0:x1] = region * (1 - a) + col * a
    alpha[y0:y1, x0:x1] = np.maximum(alpha[y0:y1, x0:x1], cover * opacity)


def render():
    w, h = W * S, H * S
    ys, xs = np.mgrid[0:h, 0:w].astype(np.float32)
    # The night: deep navy, a little lighter behind the bloom.
    top, bottom = rgb('#08142E'), rgb('#040916')
    img = top * (1 - ys / h)[..., None] + bottom * (ys / h)[..., None]
    cx, cy = w * 0.63, h * 0.56
    d = np.hypot(xs - cx, ys - cy) / (h * 0.75)
    img += (rgb('#173A8A') * 0.42) * np.exp(-d * d * 2.4)[..., None]
    del xs, ys, d

    back_stops = [(0.0, rgb('#08215E')), (0.45, rgb('#1A52B8')), (0.85, rgb('#3F8BE6')), (1.0, rgb('#7DB9FA'))]
    front_stops = [(0.0, rgb('#0E2F80')), (0.4, rgb('#2468D8')), (0.8, rgb('#5BA8F8')), (1.0, rgb('#B4DDFF'))]
    n = 7
    # The back ring: larger, deeper, softened a little (it's further away).
    back = img.copy()
    back_a = np.zeros((h, w), dtype=np.float32)
    for k in range(n):
        ang = math.radians(k * 360 / n - 8)
        petal(back, back_a, cx, cy, ang, h * 0.43, h * 0.135, back_stops, 0.82, 1 if k % 2 else -1)
    back = blur(back, 1.6 * S)
    # The front ring's shadow on it.
    front = back.copy()
    front_a = np.zeros((h, w), dtype=np.float32)
    for k in range(n):
        ang = math.radians(k * 360 / n + 360 / n / 2 - 8)
        petal(front, front_a, cx, cy, ang, h * 0.33, h * 0.11, front_stops, 0.78, -1 if k % 2 else 1)
    shadow = blur(front_a, 14 * S) * 0.45
    under = back * (1 - shadow[..., None])
    img = under * (1 - front_a[..., None]) + front * front_a[..., None]
    # The heart of the bloom: a soft light, not a white-out.
    ys, xs = np.mgrid[0:h, 0:w].astype(np.float32)
    d = np.hypot(xs - cx, ys - cy) / (h * 0.1)
    img += (rgb('#9ED4FF') * 0.22) * np.exp(-d * d)[..., None]
    del xs, ys, d
    # Glow from the bright parts, then a soft shoulder (nothing clips flat).
    bright = np.clip(img - 0.6, 0, None)
    img = img + blur(bright, 40 * S) * 0.3
    # A knee: everything below 0.72 as it is, the highlights rolled off.
    k = 0.72
    over = np.clip(img - k, 0, None)
    img = np.minimum(img, k) + (1 - k) * (1 - np.exp(-over / (1 - k)))
    return np.clip(img, 0, 1)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else OUT
    img = render()
    pic = Image.fromarray((img * 255).astype(np.uint8), 'RGB').resize((W, H), Image.LANCZOS)
    a = np.asarray(pic, dtype=np.float32)
    # A gentle vignette and dither (smooth gradients band in 8 bits).
    ys, xs = np.mgrid[0:H, 0:W].astype(np.float32)
    v = 1 - 0.28 * np.clip(np.hypot((xs - W / 2) / (W / 2), (ys - H / 2) / (H / 2)) - 0.35, 0, None)
    rng = np.random.default_rng(5)
    a = a * v[..., None] + rng.uniform(-1.2, 1.2, a.shape)
    Image.fromarray(np.clip(a, 0, 255).astype(np.uint8), 'RGB').save(out, quality=92, optimize=True)
    print(out)


if __name__ == '__main__':
    main()
