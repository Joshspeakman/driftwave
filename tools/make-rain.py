"""The rain tiles: seamless 512 px sheets of motion-blurred streaks, slid down (and across) the window by the scenes.

Writes into assets/theme-art:
  rain-glass-far.png   Night Market: heavy, fine rain far beyond the glass, out of focus
  rain-glass-near.png  Night Market: nearer rain, fewer, longer, softer streaks
  rain-far.png         Neon Noir: fine rain over the city
  rain-near.png        Neon Noir: nearer, brighter streaks

Made for the scenes' 12 frames a second: a streak moves at most about half its own length between frames, so each
reads as one falling streak, not a flicker of dots. With the themes' loops (Night Market near 1.2 s and far 2.4 s
a tile, Neon Noir 1.6 s and 3.2 s), near streaks move 18 to 36 px a frame and are 90 to 170 px long; far ones move
9 to 18 px and are 34 to 64 px. Tails stay at 45% of the head, so the whole streak shows. Drawn at 4× and scaled
down, so no streak is stair-stepped. (Supersedes the rain in make-neon-glass.py and make-theme-art.py.)
"""

import math
import os
import random

from PIL import Image, ImageDraw, ImageFilter

ART = os.path.join(os.path.dirname(__file__), '..', 'assets', 'theme-art')


def rain(name, n, length, width, alpha, blur_px, seed, tints, size=512, angle=-9, ss=4):
    rnd = random.Random(seed)
    big = size * ss
    tile = Image.new('RGBA', (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(tile)
    dx = math.tan(math.radians(angle))
    for _ in range(n):
        x, y = rnd.uniform(0, big), rnd.uniform(0, big)
        ln = rnd.uniform(*length) * ss
        a = rnd.uniform(*alpha)
        wdt = max(1, round(rnd.uniform(*width) * ss))
        col = rnd.choice(tints)
        segs = 14
        for k in range(segs):
            t0, t1 = k / segs, (k + 1) / segs
            fill = col + (int(a * (0.45 + 0.55 * t1)),)   # a soft tail, a brighter head
            for oy in (-big, 0, big):
                for ox in (-big, 0, big):
                    d.line((x + ox + dx * ln * t0, y + oy + ln * t0, x + ox + dx * ln * t1, y + oy + ln * t1), fill=fill, width=wdt)
    # Blur with wrap-around, so the tile's edges stay seamless both ways.
    pad = int(blur_px * ss * 3) + 2
    wrapped = Image.new('RGBA', (big + 2 * pad, big + 2 * pad))
    for oy in (-1, 0, 1):
        for ox in (-1, 0, 1):
            wrapped.paste(tile, (pad + ox * big, pad + oy * big))
    wrapped = wrapped.filter(ImageFilter.GaussianBlur(blur_px * ss))
    tile = wrapped.crop((pad, pad, pad + big, pad + big)).resize((size, size), Image.LANCZOS)
    tile.save(os.path.join(ART, name), optimize=True)


glass = [(215, 228, 245)] * 10 + [(255, 150, 200), (130, 235, 255)]
city = [(210, 225, 245)]
rain('rain-glass-far.png', 700, (34, 64), (0.6, 1.1), (40, 80), 0.9, 7, glass)
rain('rain-glass-near.png', 120, (90, 170), (1.6, 2.6), (40, 80), 2.0, 8, glass)
rain('rain-far.png', 520, (34, 64), (0.8, 1.1), (40, 90), 0.5, 1, city)
rain('rain-near.png', 130, (90, 170), (1.0, 2.0), (60, 140), 0.9, 2, city)
print('ok')
