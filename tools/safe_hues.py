"""Driftwave's colour rule for pictures: no teal, no purple, no pink (Josh, 2026-11-20).

`banned` mirrors rust/core/src/hues.rs. `safe_photo` moves a picture's colours out of the banned bands while
keeping their strength: teal's lower half turns green and its upper half azure, purple folds into blue, and pink and
magenta fold into crimson. Each fold has a narrow fade to grey at its seam, so gradients that cross it stay smooth.

    python tools/safe_hues.py audit             (every theme picture's share of banned colour)
    python tools/safe_hues.py fix a.jpg b.jpg   (rewrites those pictures in assets/theme-art)
"""
import glob
import os
import sys

import numpy as np
from PIL import Image

ART = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'assets', 'theme-art'))

# (hue in, hue out), piecewise linear; a seam is two points at the same hue in. Pinks and magentas land in crimson,
# purples in blue, cyans in azure, teal-greens in green.
MAP = [(0, 0), (120, 120), (178, 156), (178, 200), (230, 226), (285, 231), (285, 341), (360, 360)]
SEAMS = [(178, 6), (285, 6)]
# For pictures seen through blue-tinted glass (Aurora Glass): greens pulled well clear of teal, since a cool tint
# over them moves them towards it.
MAP_DEEP_GREEN = [(0, 0), (90, 90), (178, 128), (178, 200), (230, 226), (285, 231), (285, 341), (360, 360)]


def hsl(rgb):
    """Hue (degrees), saturation and lightness (HSL) of an (..., 3) float array in 0–1."""
    mx, mn = rgb.max(-1), rgb.min(-1)
    l = (mx + mn) / 2
    d = mx - mn
    s = np.where(d < 1e-6, 0, d / np.maximum(1e-6, np.where(l > 0.5, 2 - mx - mn, mx + mn)))
    r, g, b = rgb[..., 0], rgb[..., 1], rgb[..., 2]
    dd = np.maximum(d, 1e-6)
    h = np.where(mx == r, ((g - b) / dd) % 6, np.where(mx == g, (b - r) / dd + 2, (r - g) / dd + 4)) * 60
    return np.where(d < 1e-6, 0, h), s, l


def from_hsl(h, s, l):
    c = (1 - np.abs(2 * l - 1)) * s
    hp = (h % 360) / 60
    x = c * (1 - np.abs(hp % 2 - 1))
    z = np.zeros_like(h)
    i = np.floor(hp).astype(int) % 6
    r = np.choose(i, [c, x, z, z, x, c])
    g = np.choose(i, [x, c, c, x, z, z])
    b = np.choose(i, [z, z, x, c, c, x])
    m = l - c / 2
    return np.stack([r + m, g + m, b + m], -1)


def banned(h, s, l, a=None):
    """Teal, purple or pink, per pixel (as hues.rs)."""
    # As hues.rs: a white with the faintest tint (chroma under 16/255) shows no hue.
    seen = (s >= 0.25) & (l >= 0.08) & (l <= 0.95) & (s * (1 - np.abs(2 * l - 1)) >= 16 / 255)
    if a is not None:
        seen &= a >= 40 / 255
    teal = (h >= 160) & (h < 197)
    purple = ((h >= 245) & (h < 290)) | ((h >= 232) & (h < 245) & (l > 0.6))
    pink = ((h >= 290) & (h < 340)) | (((h >= 340) | (h < 8)) & (l > 0.72) & (s > 0.45))
    return seen & (teal | purple | pink)


def remap(h, points=None):
    points = points or MAP
    out = np.copy(h)
    for (h0, o0), (h1, o1) in zip(points, points[1:]):
        if h1 == h0:
            continue
        m = (h >= h0) & (h < h1)
        out[m] = o0 + (o1 - o0) * (h[m] - h0) / (h1 - h0)
    return out


def safe_rgb(rgb, points=None):
    """An (..., 3) float RGB array (0–1), its colours moved out of the banned bands."""
    h, s, l = hsl(rgb)
    s2 = np.copy(s)
    for at, half in SEAMS:
        d = np.abs(h - at)
        s2 = np.where(d < half, s2 * d / half, s2)
    h2 = remap(h, points)
    # Pale crimson reads as pink: keep it soft.
    pale = ((h2 >= 340) | (h2 < 8)) & (l > 0.72)
    s2 = np.where(pale, np.minimum(s2, 0.44), s2)
    # Pale blue at the violet end reads as lavender.
    lav = (h2 >= 228) & (h2 < 245) & (l > 0.6)
    h2 = np.where(lav, np.minimum(h2, 228), h2)
    return np.clip(from_hsl(h2, s2, l), 0, 1)


def safe_photo(img, points=None):
    """A PIL image (RGB or RGBA) with its colours moved out of the banned bands."""
    a = None
    if img.mode == 'RGBA':
        a = img.getchannel('A')
    rgb = np.asarray(img.convert('RGB')).astype(np.float32) / 255
    out = Image.fromarray((safe_rgb(rgb, points) * 255 + 0.5).astype(np.uint8))
    if a is not None:
        out.putalpha(a)
    return out


def share(img):
    im = img.convert('RGBA').resize((320, 180))
    arr = np.asarray(im).astype(np.float32) / 255
    h, s, l = hsl(arr[..., :3])
    seen = arr[..., 3] >= 60 / 255
    bad = banned(h, s, l) & seen
    return 100 * bad.sum() / max(1, seen.sum())


if __name__ == '__main__':
    if sys.argv[1:2] == ['audit']:
        for f in sorted(glob.glob(os.path.join(ART, '*.*'))):
            if f.lower().endswith(('.jpg', '.png')):
                print(f'{os.path.basename(f):32} {share(Image.open(f)):5.1f}% banned')
    elif sys.argv[1:2] == ['fix']:
        # fix [--deep-green] [--from original.jpg] name...
        args = sys.argv[2:]
        points = MAP_DEEP_GREEN if '--deep-green' in args else None
        source = args[args.index('--from') + 1] if '--from' in args else None
        names = [a for i, a in enumerate(args) if not a.startswith('--') and (i == 0 or args[i - 1] != '--from')]
        for name in names:
            path = os.path.join(ART, name)
            img = Image.open(source or path)
            before = share(img)
            out = safe_photo(img.convert('RGBA') if img.mode in ('RGBA', 'LA', 'P') else img.convert('RGB'), points)
            if name.lower().endswith('.png'):
                out.save(path, optimize=True)
            else:
                out.save(path, quality=90, optimize=True)
            print(f'{name}: {before:.1f}% -> {share(out):.1f}% banned')
