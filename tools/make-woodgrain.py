"""Woodgrain's materials, rendered (no photographs): the walnut veneer of a 1977 console's front, and the ribbed black
plastic of its top.

Writes into assets/theme-art:
  woodgrain-walnut.jpg   flat-cut walnut veneer, grain running across: figured rings, cathedral arches, open pores,
                         a satin sheen (1920 x 1280)
  woodgrain-ribs.jpg     ribbed black plastic, ridges running across, lit from above (1920 x 96)

Run with Pillow and NumPy:  python tools/make-woodgrain.py
"""
import os

import numpy as np
from PIL import Image, ImageFilter

from safe_hues import safe_photo

ART = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'assets', 'theme-art'))
rng = np.random.default_rng(1977)


def smooth_noise(h, w, cell, seed):
    """Value noise: a coarse random grid, upsampled smoothly."""
    r = np.random.default_rng(seed)
    gh, gw = h // cell + 3, w // cell + 3
    grid = r.random((gh, gw)).astype(np.float32)
    img = Image.fromarray((grid * 255).astype(np.uint8)).resize((gw * cell, gh * cell), Image.BICUBIC)
    return np.asarray(img).astype(np.float32)[:h, :w] / 255


def fbm(h, w, seed, octaves=((320, 0.5), (140, 0.25), (60, 0.15), (24, 0.1))):
    return sum(a * smooth_noise(h, w, c, seed + i) for i, (c, a) in enumerate(octaves))


def streaks(h, w, cx, cy, seed):
    """Anisotropic value noise: cells cx wide and cy tall, smoothly upsampled (long streaks along the grain)."""
    r = np.random.default_rng(seed)
    gh, gw = h // cy + 3, w // cx + 3
    grid = r.random((gh, gw)).astype(np.float32)
    img = Image.fromarray((grid * 255).astype(np.uint8)).resize((gw * cx, gh * cy), Image.BICUBIC)
    return np.asarray(img).astype(np.float32)[:h, :w] / 255


def walnut(w=1920, h=1280):
    ys, xs = np.mgrid[0:h, 0:w].astype(np.float32)
    # Long streaks of light and dark along the grain, at several widths.
    tone = (0.5 * streaks(h, w, 640, 26, 3) + 0.3 * streaks(h, w, 380, 9, 4) + 0.2 * streaks(h, w, 220, 3, 5))
    # Growth rings: fine dark lines, gently bowed, with broad cathedral arches where the cut crossed a ring's curve.
    warp = (streaks(h, w, 700, 160, 6) - 0.5) * 70
    arches = np.zeros_like(xs)
    for cx, cy, k in ((w * 0.32, h * 3.2, 1.0), (w * 0.8, -h * 2.8, -0.7)):
        d = np.sqrt(((xs - cx) * 0.42) ** 2 + (ys - cy) ** 2)
        arches += k * d * 0.09
    phase = (ys + warp + arches) / 9.0
    f = phase - np.floor(phase)
    # A line is dark and narrow; how dark wanders along it, so lines fade in and out like real latewood.
    line = np.clip(1 - np.abs(f - 0.5) / 0.16, 0, 1) ** 1.5
    strength = streaks(h, w, 300, 40, 8) ** 1.4
    tone = tone - 0.34 * line * (0.25 + 0.95 * strength)
    # Open pores: short dark flecks along the grain.
    pores = streaks(h, w, 14, 1, 9)
    tone -= np.clip((pores - 0.86) / 0.14, 0, 1) * 0.16
    tone = np.clip((tone - 0.12) / 0.75, 0, 1)
    # Walnut: deep chocolate to warm brown, not orange.
    stops = [(0.0, (26, 15, 9)), (0.3, (50, 31, 19)), (0.6, (82, 54, 34)), (0.85, (110, 75, 48)), (1.0, (134, 94, 62))]
    rgb = np.zeros((h, w, 3), np.float32)
    for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
        m = (tone >= t0) & (tone <= t1)
        u = ((tone - t0) / (t1 - t0))[m][:, None]
        rgb[m] = np.array(c0) * (1 - u) + np.array(c1) * u
    # A satin finish: a broad soft sheen from the upper left, darker towards the edges.
    sheen = np.exp(-(((xs - w * 0.3) / (w * 0.6)) ** 2 + ((ys - h * 0.25) / (h * 0.8)) ** 2))
    rgb *= (0.82 + 0.3 * sheen)[..., None]
    edge = np.minimum.reduce([xs / (w * 0.18), (w - xs) / (w * 0.18), ys / (h * 0.2), (h - ys) / (h * 0.2), np.ones_like(xs)])
    rgb *= (0.86 + 0.14 * np.clip(edge, 0, 1))[..., None]
    rgb += (rng.random((h, w, 1)).astype(np.float32) - 0.5) * 3
    return Image.fromarray(np.clip(rgb, 0, 255).astype(np.uint8))


def ribs(w=1920, h=96, pitch=6):
    ys, xs = np.mgrid[0:h, 0:w].astype(np.float32)
    p = (ys % pitch) / pitch
    # Each rib: a rounded ridge lit from above: a bright lip, a dark groove below.
    ridge = np.sin(p * np.pi) ** 0.8
    light = np.clip(1 - np.abs(p - 0.25) / 0.18, 0, 1) ** 2
    groove = np.clip(1 - np.abs(p - 0.98) / 0.12, 0, 1)
    v = 15 + 13 * ridge + 26 * light - 9 * groove
    # The plastic's faint mottling and a sheen across the middle.
    v = v + (smooth_noise(h, w, 48, 5) - 0.5) * 5
    v = v * (0.9 + 0.18 * np.exp(-((xs - w * 0.45) / (w * 0.5)) ** 2))
    rgb = np.stack([v * 1.02, v, v * 0.97], -1)
    return Image.fromarray(np.clip(rgb, 0, 255).astype(np.uint8))


if __name__ == '__main__':
    os.makedirs(ART, exist_ok=True)
    safe_photo(walnut()).save(os.path.join(ART, 'woodgrain-walnut.jpg'), quality=86, optimize=True, progressive=True)
    ribs().save(os.path.join(ART, 'woodgrain-ribs.jpg'), quality=90, optimize=True)
    for n in ('woodgrain-walnut.jpg', 'woodgrain-ribs.jpg'):
        print(n, os.path.getsize(os.path.join(ART, n)) // 1024, 'KB')
