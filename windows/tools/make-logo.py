"""Draws the Driftwave logo: a sun settling over two drifting waves on a dusk gradient.

Writes the app icon (multi-size .ico and a 256 px PNG), the mark as SVG and large PNG, and the wordmark
(mark + "Driftwave" in Shrikhand, which ships with the app under the OFL) for light and dark backgrounds.

Usage: python windows/tools/make-logo.py [--assets <dir>] [--brand <dir>]
Needs Pillow. Everything is drawn at 4x and scaled down, so edges are smooth.
"""
import argparse
import math
import os

from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..'))
FONT = os.path.join(ROOT, 'src', 'Shortwave', 'Assets', 'Fonts', 'Shrikhand-Regular.ttf')

# Dusk: orange at the top, through rose, to night indigo.
TOP, MID, BOTTOM = (253, 140, 77), (204, 64, 107), (46, 38, 105)
SUN = (255, 227, 176)
WAVE = (255, 255, 255)
SS = 4  # supersampling


def lerp(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def gradient(size):
    img = Image.new('RGB', (1, size))
    for y in range(size):
        t = y / max(size - 1, 1)
        img.putpixel((0, y), lerp(TOP, MID, t / 0.5) if t < 0.5 else lerp(MID, BOTTOM, (t - 0.5) / 0.5))
    return img.resize((size, size))


def wave_points(x0, x1, y, amp, periods, phase, scale):
    pts = []
    n = 200
    for i in range(n + 1):
        x = x0 + (x1 - x0) * i / n
        a = phase + periods * 2 * math.pi * i / n
        pts.append((x * scale, (y - amp * math.sin(a)) * scale))
    return pts


def stroke(draw, pts, width, fill):
    """A thick line as one filled outline (both sides offset along the normal) with round caps."""
    r = width / 2
    left, right = [], []
    for i, (x, y) in enumerate(pts):
        (ax, ay), (bx, by) = pts[max(i - 1, 0)], pts[min(i + 1, len(pts) - 1)]
        dx, dy = bx - ax, by - ay
        n = math.hypot(dx, dy) or 1
        nx, ny = -dy / n * r, dx / n * r
        left.append((x + nx, y + ny))
        right.append((x - nx, y - ny))
    draw.polygon(left + right[::-1], fill=fill)
    for (x, y) in (pts[0], pts[-1]):
        draw.ellipse((x - r, y - r, x + r, y + r), fill=fill)


def mark(px, small=None):
    """The square icon at px pixels. Small sizes (<= 32 px) use a simpler, bolder drawing."""
    if small is None:
        small = px <= 32
    size = px * SS
    s = size / 1024.0
    inset = 24 if small else 64
    radius = 190 if small else 220
    bg = gradient(size)
    mask = Image.new('L', (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle((inset * s, inset * s, (1024 - inset) * s, (1024 - inset) * s), radius * s, fill=255)
    img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    img.paste(bg, (0, 0), mask)

    art = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(art)
    # The sun, low in the sky, with a soft halo.
    cx, cy, r = 512, (470 if small else 500), (200 if small else 185)
    if not small:
        for k, a in ((1.55, 40), (1.3, 60)):
            d.ellipse(((cx - r * k) * s, (cy - r * k) * s, (cx + r * k) * s, (cy + r * k) * s), fill=SUN + (a,))
    d.ellipse(((cx - r) * s, (cy - r) * s, (cx + r) * s, (cy + r) * s), fill=SUN + (255,))
    # Water: the waves cover the bottom of the sun.
    if small:
        water_top = 600
    else:
        water_top = 610
    water = [(inset * s, size)] + wave_points(inset, 1024 - inset, water_top, 34, 1.5, 0.3, s) + [((1024 - inset) * s, size)]
    cut = Image.new('L', (size, size), 0)
    ImageDraw.Draw(cut).polygon(water, fill=255)
    art.putalpha(Image.composite(Image.new('L', (size, size), 0), art.getchannel('A'), cut))
    d = ImageDraw.Draw(art)
    if small:
        stroke(d, wave_points(170, 854, 640, 56, 1.25, 0.3, s), 110 * s, WAVE + (255,))
    else:
        stroke(d, wave_points(190, 834, 640, 52, 1.5, 0.3, s), 64 * s, WAVE + (255,))
        stroke(d, wave_points(270, 754, 770, 36, 1.5, 0.9, s), 40 * s, WAVE + (150,))
    img.alpha_composite(Image.composite(art, Image.new('RGBA', (size, size), (0, 0, 0, 0)), mask))
    return img.resize((px, px), Image.LANCZOS)


def mark_svg():
    def path(x0, x1, y, amp, periods, phase):
        pts = wave_points(x0, x1, y, amp, periods, phase, 1)[::5]
        return 'M' + ' L'.join(f'{x:.1f} {y:.1f}' for x, y in pts)
    water = path(64, 960, 610, 34, 1.5, 0.3)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" role="img" aria-label="Driftwave">
  <defs>
    <linearGradient id="dusk" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="rgb{TOP}"/><stop offset="0.5" stop-color="rgb{MID}"/><stop offset="1" stop-color="rgb{BOTTOM}"/>
    </linearGradient>
    <clipPath id="tile"><rect x="64" y="64" width="896" height="896" rx="220"/></clipPath>
    <clipPath id="sky"><path d="M64 0 L960 0 L960 {610} {water.replace('M', 'L', 1)[1:] and ''}L960 610 {water[1:].replace(' L', ' L')} Z"/></clipPath>
  </defs>
  <g clip-path="url(#tile)">
    <rect width="1024" height="1024" fill="url(#dusk)"/>
    <g clip-path="url(#skyclip)">
      <circle cx="512" cy="500" r="287" fill="rgb{SUN}" fill-opacity="0.16"/>
      <circle cx="512" cy="500" r="241" fill="rgb{SUN}" fill-opacity="0.24"/>
      <circle cx="512" cy="500" r="185" fill="rgb{SUN}"/>
    </g>
    <path d="{path(190, 834, 640, 52, 1.5, 0.3)}" fill="none" stroke="#fff" stroke-width="64" stroke-linecap="round" stroke-linejoin="round"/>
    <path d="{path(270, 754, 770, 36, 1.5, 0.9)}" fill="none" stroke="#fff" stroke-opacity="0.59" stroke-width="40" stroke-linecap="round" stroke-linejoin="round"/>
  </g>
</svg>
'''


def sky_clip_svg():
    pts = wave_points(64, 960, 610, 34, 1.5, 0.3, 1)[::5]
    return 'M0 0 L1024 0 L1024 ' + f'{pts[-1][1]:.1f} ' + ' '.join(f'L{x:.1f} {y:.1f}' for x, y in reversed(pts)) + ' L0 ' + f'{pts[0][1]:.1f} Z'


def wordmark(height, text_color):
    m = mark(height, small=False)
    font = ImageFont.truetype(FONT, round(height * 0.62))
    box = font.getbbox('Driftwave')
    w = height + round(height * 0.22) + (box[2] - box[0]) + round(height * 0.1)
    img = Image.new('RGBA', (w, height), (0, 0, 0, 0))
    img.alpha_composite(m, (0, 0))
    d = ImageDraw.Draw(img)
    tx = height + round(height * 0.22) - box[0]
    ty = (height - (box[3] - box[1])) // 2 - box[1]
    d.text((tx, ty), 'Driftwave', font=font, fill=text_color)
    return img


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--assets', default=os.path.join(ROOT, 'src', 'Shortwave', 'Assets'))
    ap.add_argument('--brand', default=None, help='also write the mark (SVG/PNG) and wordmarks here')
    a = ap.parse_args()
    sizes = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
    frames = [mark(px) for px in sizes]
    frames[-1].save(os.path.join(a.assets, 'Driftwave.png'))
    frames[-1].save(os.path.join(a.assets, 'Driftwave.ico'), format='ICO', sizes=[(p, p) for p in sizes], append_images=frames[:-1])
    print('icon:', a.assets)
    if a.brand:
        os.makedirs(a.brand, exist_ok=True)
        mark(1024).save(os.path.join(a.brand, 'logo-mark.png'))
        mark(512).save(os.path.join(a.brand, 'logo-mark-512.png'))
        mark(32).save(os.path.join(a.brand, 'favicon-32.png'))
        svg = mark_svg().replace('url(#skyclip)', 'url(#skyclip)')
        svg = svg[:svg.index('<clipPath id="sky">')] + f'<clipPath id="skyclip"><path d="{sky_clip_svg()}"/></clipPath>' + svg[svg.index('</clipPath>', svg.index('<clipPath id="sky">')) + len('</clipPath>'):]
        with open(os.path.join(a.brand, 'logo-mark.svg'), 'w', encoding='utf-8') as f:
            f.write(svg)
        wordmark(160, (29, 26, 36, 255)).save(os.path.join(a.brand, 'logo-wordmark-light.png'))
        wordmark(160, (241, 237, 246, 255)).save(os.path.join(a.brand, 'logo-wordmark-dark.png'))
        print('brand:', a.brand)


if __name__ == '__main__':
    main()
