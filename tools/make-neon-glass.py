"""Night Market's rainy window: the street seen through dark tinted glass with neon bloom and water beads.

Writes into assets/theme-art:
  photo-night-market-glass.jpg  the street, blurred and darkened behind tinted glass, neon blooming through,
                                and still water beads on the glass (each a tiny upside-down lens of the street)
  night-market-neon.png         the neon glow alone (alpha), which the theme breathes very slowly
  drop-0..5.png                 sliding drops: lenses of the street with a dark rim and a highlight
  drop-trail.png                the wet trail a sliding drop leaves
  rain-glass-far.png            heavy rain beyond the glass: dense, fine, out of focus (a seamless tile)
  rain-glass-near.png           nearer rain beyond the glass: fewer, longer, softer streaks (a seamless tile)

Run with Pillow and NumPy:  python windows/tools/make-neon-glass.py
Source: photo-night-market.jpg (Basile Morin, CC BY-SA 4.0; see assets/theme-art/CREDITS.txt).
"""
import os
import random

import math

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

ART = os.path.join(os.path.dirname(__file__), '..', 'assets', 'theme-art')
W = 1920

photo = Image.open(os.path.join(ART, 'photo-night-market.jpg')).convert('RGB')
photo = photo.resize((W, round(photo.height * W / photo.width)), Image.LANCZOS)
H = photo.height
sharp = np.asarray(photo).astype(np.float32) / 255


def blur(arr, r):
    im = Image.fromarray((np.clip(arr, 0, 1) * 255).astype(np.uint8))
    return np.asarray(im.filter(ImageFilter.GaussianBlur(r))).astype(np.float32) / 255


def screen(a, b):
    return 1 - (1 - a) * (1 - b)


# The glass: the street out of focus, darkened and tinted a cold blue-green.
soft = blur(sharp, 11)
tint = np.array([0.03, 0.06, 0.09], np.float32)
glass = soft * 0.38 + tint * 0.6
# Lift the colour a little so the neon reads as colour, not grey.
grey = glass.mean(axis=2, keepdims=True)
glass = np.clip(grey + (glass - grey) * 1.35, 0, 1)

# Neon: the bright, saturated parts of the street, bloomed through the glass.
mx, mn = sharp.max(axis=2), sharp.min(axis=2)
sat = (mx - mn) / (mx + 1e-4)
mask = np.clip((mx - 0.55) / 0.35, 0, 1) * np.clip((sat - 0.25) / 0.4, 0, 1)
neon = sharp * mask[..., None]
neon = neon / np.maximum(neon.max(axis=2, keepdims=True), 1e-4) * mask[..., None]   # pure hue, full strength
near = blur(neon, 6) * 1.3
far = blur(neon, 26) * 1.6
wide = blur(neon, 70) * 1.2
glass = screen(glass, near * 0.55)
glass = screen(glass, far * 0.5)

# The neon glow alone, for the slow breathing layer (half size: it's all soft).
glow = np.clip(screen(far * 0.6, wide * 0.7), 0, 1)
alpha = glow.max(axis=2)
rgb = glow / np.maximum(alpha[..., None], 1e-4)
glow_img = Image.fromarray((np.dstack([np.clip(rgb, 0, 1), alpha]) * 255).astype(np.uint8), 'RGBA')
glow_img.resize((W // 2, H // 2), Image.LANCZOS).save(os.path.join(ART, 'night-market-neon.png'), optimize=True)


def lens(cx, cy, r, src, gain=1.25):
    """A water bead of radius r at (cx, cy): the street through it, magnified and upside down."""
    size = int(r * 2 + 3)
    ys, xs = np.mgrid[0:size, 0:size].astype(np.float32)
    dx, dy = (xs - size / 2) / r, (ys - size / 2) / r
    d = np.sqrt(dx * dx + dy * dy)
    inside = d < 1
    # A lens: looks at a wider patch of the street, flipped, a little way above (where the light comes from).
    reach = r * 11
    sx = np.clip(cx - dx * reach, 0, W - 1).astype(int)
    sy = np.clip(cy - r * 6 - dy * reach, 0, H - 1).astype(int)
    view = src[sy, sx] * gain
    # Darker towards the rim, a dark crescent at the bottom, and a bright spot at the top left.
    rim = np.clip((d - 0.8) / 0.2, 0, 1)
    view = view * (1 - 0.6 * rim[..., None])
    crescent = np.clip((dy - 0.45) / 0.55, 0, 1) * rim
    view = view * (1 - 0.4 * crescent[..., None])
    spot = np.exp(-(((dx + 0.35) ** 2 + (dy + 0.4) ** 2) / 0.03))
    view = screen(view, (spot * 0.85)[..., None] * np.ones(3))
    edge = np.clip((1 - d) * r, 0, 1)   # antialiased edge
    return view, edge * inside, size


# What drops see: the street with its neon bloom (drops gather light).
lit = blur(np.clip(screen(sharp, far * 0.8), 0, 1), 4)
lit_soft = blur(lit, 7)

# Still beads on the glass, mostly tiny, a few larger; more towards the bottom, where water gathers.
rng = random.Random(42)
out = glass.copy()
centres = []
for _ in range(1400):
    r = rng.choice([1.2, 1.4, 1.7, 2.0, 2.4, 2.8, 3.4, 4.2, 5.5, 7, 9]) * (1 + rng.random() * 0.3)
    if rng.random() < 0.5 and centres:   # beads cluster
        bx, by = rng.choice(centres)
        cx, cy = bx + rng.gauss(0, 40), by + rng.gauss(0, 40)
    else:
        cx = rng.random() * W
        cy = H * (1 - rng.random() ** 1.4)
        centres.append((cx, cy))
    view, a, size = lens(cx, cy, r, lit, gain=1.15)
    x0, y0 = int(cx - size / 2), int(cy - size / 2)
    x1, y1 = max(x0, 0), max(y0, 0)
    x2, y2 = min(x0 + size, W), min(y0 + size, H)
    if x2 <= x1 or y2 <= y1:
        continue
    va = view[y1 - y0:y2 - y0, x1 - x0:x2 - x0]
    aa = a[y1 - y0:y2 - y0, x1 - x0:x2 - x0][..., None] * 0.85
    out[y1:y2, x1:x2] = out[y1:y2, x1:x2] * (1 - aa) + va * aa

# A few old trails where drops have run before: the glass slightly clearer along them.
clear = blur(sharp, 5) * 0.5 + tint * 0.4
for _ in range(9):
    x = rng.random() * W
    top = rng.uniform(0.15, 0.6) * H
    width = rng.uniform(2.5, 5)
    phase, wander, period = rng.random() * 6.28, rng.uniform(3, 9), rng.choice([80, 110])
    for y in range(int(top), H):
        xx = x + wander * np.sin(y / period + phase)
        k = min(1, (y - top) / 160) * 0.7   # fades in below where the drop started
        x1, x2 = max(int(xx - width * 2), 0), min(int(xx + width * 2) + 1, W)
        if x2 > x1:
            cols = np.arange(x1, x2)
            wgt = (np.exp(-((cols - xx) / width) ** 2) * k)[:, None]
            out[y, x1:x2] = out[y, x1:x2] * (1 - wgt) + clear[y, x1:x2] * wgt

Image.fromarray((np.clip(out, 0, 1) * 255).astype(np.uint8)).save(
    os.path.join(ART, 'photo-night-market-glass.jpg'), quality=88, optimize=True)

# Sliding drops: big lenses of the street, slightly taller than wide (they sag as they run).
for i in range(6):
    r = 22
    cx, cy = rng.uniform(200, W - 200), rng.uniform(H * 0.3, H * 0.8)
    view, a, size = lens(cx, cy, r, lit_soft, gain=1.5)
    ys, xs = np.mgrid[0:size, 0:size].astype(np.float32)
    d = np.sqrt(((xs - size / 2) / r) ** 2 + ((ys - size / 2) / r) ** 2)
    a = a * (0.4 + 0.5 * np.clip((d - 0.45) / 0.55, 0, 1))
    img = Image.fromarray((np.dstack([np.clip(view, 0, 1), a]) * 255).astype(np.uint8), 'RGBA')
    img = img.resize((size, int(size * 1.18)), Image.LANCZOS)
    img.save(os.path.join(ART, f'drop-{i}.png'), optimize=True)

# The trail: a faint wet streak with brighter edges, fading at the top.
tw, th = 16, 256
ys, xs = np.mgrid[0:th, 0:tw].astype(np.float32)
u = np.abs(xs - tw / 2 + 0.5) / (tw / 2)
core = np.clip(1 - u, 0, 1) ** 2 * 0.10
edges = np.exp(-((u - 0.75) ** 2) / 0.01) * 0.22
fade = np.clip(ys / th * 1.4, 0, 1)
a = (core + edges) * fade
Image.fromarray((np.dstack([np.full((th, tw, 3), 0.85), a]) * 255).astype(np.uint8), 'RGBA').save(
    os.path.join(ART, 'drop-trail.png'), optimize=True)


def rain_tile(name, n, length, width, alpha, blur_px, seed, size=512, angle=-9, ss=4):
    """
    Rain beyond the glass, as a tile that repeats seamlessly both ways. Drawn at ss times the size (so every streak
    is smooth, never stair-stepped), motion-blurred along its slant, softened to the street's focus, then scaled
    down. Most streaks are cold white; some catch the pink or cyan of the signs.
    """
    rnd = random.Random(seed)
    big = size * ss
    tile = Image.new('RGBA', (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(tile)
    dx = math.tan(math.radians(angle))
    tints = [(215, 228, 245)] * 10 + [(255, 150, 200), (130, 235, 255)]
    for _ in range(n):
        x, y = rnd.uniform(0, big), rnd.uniform(0, big)
        ln = rnd.uniform(*length) * ss
        a = rnd.uniform(*alpha)
        wdt = max(1, round(rnd.uniform(*width) * ss))
        col = rnd.choice(tints)
        segs = 12
        for k in range(segs):
            t0, t1 = k / segs, (k + 1) / segs
            fill = col + (int(a * (0.15 + 0.85 * t1)),)   # faint tail, brighter head
            for oy in (-big, 0, big):
                for ox in (-big, 0, big):
                    d.line((x + ox + dx * ln * t0, y + oy + ln * t0, x + ox + dx * ln * t1, y + oy + ln * t1), fill=fill, width=wdt)
    # Blur with wrap-around, so the tile's edges stay seamless.
    pad = int(blur_px * ss * 3) + 2
    wrapped = Image.new('RGBA', (big + 2 * pad, big + 2 * pad))
    for oy in (-1, 0, 1):
        for ox in (-1, 0, 1):
            wrapped.paste(tile, (pad + ox * big, pad + oy * big))
    wrapped = wrapped.filter(ImageFilter.GaussianBlur(blur_px * ss))
    tile = wrapped.crop((pad, pad, pad + big, pad + big)).resize((size, size), Image.LANCZOS)
    tile.save(os.path.join(ART, name), optimize=True)


# The rain tiles are made by make-rain.py (tuned for the scenes' 12 frames a second).
print('ok', W, H)
