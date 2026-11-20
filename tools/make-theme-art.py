"""Renders the theme backdrops in assets/theme-art (original artwork, drawn by this script).

Realistic, not pixel art: layered skylines with atmospheric haze, neon with real glow, wet-street reflections,
motion-blurred rain sheets that loop seamlessly, clouds from fractal noise, light ribbons and blooms.
Pillow only. Usage: python windows/tools/make-theme-art.py [name ...]
"""
import math
import os

from safe_hues import safe_photo
import random
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

OUT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'assets', 'theme-art'))
W, H = 1920, 1080
FONTS = r'C:\Windows\Fonts'


def font(name, size):
    try:
        return ImageFont.truetype(os.path.join(FONTS, name), size)
    except OSError:
        return ImageFont.load_default()


def hexc(s, a=255):
    s = s.lstrip('#')
    return (int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16), a)


def vgrad(w, h, stops):
    """Vertical gradient through (offset, '#rrggbb') stops."""
    img = Image.new('RGB', (1, h))
    stops = [(o, hexc(c)) for o, c in stops]
    for y in range(h):
        t = y / max(1, h - 1)
        for (o1, c1), (o2, c2) in zip(stops, stops[1:]):
            if o1 <= t <= o2:
                f = (t - o1) / max(1e-6, o2 - o1)
                img.putpixel((0, y), tuple(round(c1[i] + (c2[i] - c1[i]) * f) for i in range(3)))
                break
    return img.resize((w, h))


def glow(layer, passes=((4, 1.0), (14, 0.9), (40, 0.7))):
    """A light layer (RGBA) with its bloom: blurred copies added under it."""
    out = Image.new('RGBA', layer.size, (0, 0, 0, 0))
    for radius, strength in passes:
        b = layer.filter(ImageFilter.GaussianBlur(radius))
        if strength != 1:
            b.putalpha(b.getchannel('A').point(lambda a: int(a * strength)))
        out = Image.alpha_composite(out, b)
    return Image.alpha_composite(out, layer)


def add(base, light):
    """Additive light over an RGB base (light is RGBA; its alpha scales it)."""
    rgb = Image.new('RGB', base.size, (0, 0, 0))
    rgb.paste(light.convert('RGB'), (0, 0), light)
    return ImageChops.add(base, rgb)


def noise(w, h, scale, seed):
    random.seed(seed)
    small = Image.effect_noise((max(2, w // scale), max(2, h // scale)), 64).filter(ImageFilter.GaussianBlur(0.8))
    return small.resize((w, h), Image.BICUBIC)


def fbm(w, h, seed, octaves=((256, 0.55), (96, 0.25), (32, 0.13), (10, 0.07))):
    acc = Image.new('L', (w, h), 0)
    for i, (scale, weight) in enumerate(octaves):
        n = noise(w, h, scale, seed + i * 17)
        acc = Image.blend(acc, n, weight / (weight + 0.0001 + sum(o[1] for o in octaves[:i])) if i else 1.0)
    return acc


def vignette(img, strength=0.55):
    w, h = img.size
    m = Image.new('L', (w, h), 0)
    ImageDraw.Draw(m).ellipse((-w * 0.25, -h * 0.35, w * 1.25, h * 1.35), fill=255)
    m = m.filter(ImageFilter.GaussianBlur(min(w, h) * 0.18))
    dark = Image.new('RGB', (w, h), (0, 0, 0))
    return Image.composite(img, Image.blend(img, dark, strength), m)


def save(img, name, quality=88):
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, name)
    # Driftwave's colour rule (no teal, purple or pink), last: blending light can make what no input had.
    img = safe_photo(img.convert('RGBA') if img.mode in ('RGBA', 'LA') else img.convert('RGB'))
    if name.endswith('.png'):
        img.save(path, optimize=True)
    else:
        img.convert('RGB').save(path, quality=quality, optimize=True, progressive=True)
    print(f'{name}: {os.path.getsize(path) // 1024} KB')


# MARK: the city at night

def city(seed, signs, palette, horizon=700, street=True, stall=True):
    """A rain-soaked city street at night: layered skyline in haze, lit windows, neon signs with real glow,
    a street-level row of shops (and a noodle stall), and a wet road reflecting it all."""
    rnd = random.Random(seed)
    img = vgrad(W, H, [(0, '#03050b'), (0.35, '#070b18'), (0.62, palette['haze']), (0.66, '#0a0d18'), (1, '#05060a')])
    # Haze lit from below by the city
    haze = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    hd = ImageDraw.Draw(haze)
    for _ in range(9):
        x = rnd.randint(-200, W)
        c = hexc(rnd.choice(palette['neon']), 70)
        hd.ellipse((x, horizon - 360, x + rnd.randint(400, 900), horizon + 120), fill=c)
    img = add(img, haze.filter(ImageFilter.GaussianBlur(120)))

    def skyline(layer_seed, base_y, min_h, max_h, color, window_alpha, lit, blur, wmin=60, wmax=160):
        r = random.Random(layer_seed)
        layer = Image.new('RGBA', (W, H), (0, 0, 0, 0))
        lights = Image.new('RGBA', (W, H), (0, 0, 0, 0))
        d, ld = ImageDraw.Draw(layer), ImageDraw.Draw(lights)
        x = -40
        while x < W + 40:
            bw = r.randint(wmin, wmax)
            bh = r.randint(min_h, max_h)
            top = base_y - bh
            d.rectangle((x, top, x + bw, base_y), fill=color)
            if r.random() < 0.3:   # stepped tops and rooftop boxes
                d.rectangle((x + bw * 0.2, top - r.randint(10, 40), x + bw * 0.6, top), fill=color)
            if r.random() < 0.25:  # antenna with a soft red light
                ax = x + bw * r.uniform(0.3, 0.7)
                d.line((ax, top - 60, ax, top), fill=color, width=2)
                ld.ellipse((ax - 3, top - 64, ax + 3, top - 58), fill=(255, 60, 60, 200))
            ww, wh = r.choice([(3, 5), (4, 6), (5, 7)])
            for wy in range(int(top) + 8, base_y - 6, wh + r.randint(4, 7)):
                for wx in range(x + 6, x + bw - 6, ww + r.randint(3, 6)):
                    if r.random() < lit:
                        c = hexc(r.choice(['#FFD9A0', '#FFE7C2', '#A8E4FF', '#FFC0D8', '#FFF4DE']), int(window_alpha * r.uniform(0.4, 1)))
                        ld.rectangle((wx, wy, wx + ww, wy + wh), fill=c)
            x += bw + r.randint(-10, 18)
        layer = layer.filter(ImageFilter.GaussianBlur(blur))
        lights = glow(lights.filter(ImageFilter.GaussianBlur(blur * 0.6)), ((2, 0.8), (6, 0.5)))
        return layer, lights

    for i, (base, mn, mx, col, wa, lit, bl, fog) in enumerate([
        (horizon - 40, 180, 420, (18, 24, 40, 255), 120, 0.18, 2.5, 0.35),
        (horizon + 10, 220, 560, (11, 15, 26, 255), 190, 0.24, 1.2, 0.18),
        (horizon + 60, 300, 680, (6, 8, 14, 255), 230, 0.3, 0.4, 0.0),
    ]):
        layer, lights = skyline(seed * 10 + i, base, mn, mx, col, wa, lit, bl)
        img = Image.alpha_composite(img.convert('RGBA'), layer).convert('RGB')
        img = add(img, lights)
        if fog:
            f = fbm(W, H, seed + i).point(lambda v: int(max(0, v - 90) * 1.6))
            fog_layer = Image.new('RGBA', (W, H), hexc(palette['fog'], 0))
            fog_layer.putalpha(f.point(lambda v: int(v * fog)))
            img = Image.alpha_composite(img.convert('RGBA'), fog_layer).convert('RGB')

    # Neon signs on the near buildings
    neon = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    boards = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    nd, bd = ImageDraw.Draw(neon), ImageDraw.Draw(boards)
    jp = font('YuGothB.ttc', 46)
    jp_small = font('YuGothB.ttc', 30)
    latin = font('bahnschrift.ttf', 40)
    for sign in signs:
        text, x, y, color, vertical = sign['text'], int(sign['x'] * W), int(sign['y'] * H), sign['color'], sign.get('vertical', True)
        f = jp if sign.get('big', True) else jp_small
        if any(ord(ch) < 128 for ch in text) and not vertical:
            f = latin
        if vertical:
            step = f.size + 6
            bd.rounded_rectangle((x - 14, y - 14, x + f.size + 14, y + step * len(text) + 8), 10, fill=(10, 8, 14, 235),
                                 outline=hexc(color, 90), width=3)
            for i, ch in enumerate(text):
                nd.text((x, y + i * step), ch, font=f, fill=hexc(color))
        else:
            tw = int(nd.textlength(text, font=f))
            bd.rounded_rectangle((x - 18, y - 12, x + tw + 18, y + f.size + 16), 10, fill=(10, 8, 14, 235), outline=hexc(color, 90), width=3)
            nd.text((x, y), text, font=f, fill=hexc(color))
        # Neon tubes are whiter at the core
        core = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    img = Image.alpha_composite(img.convert('RGBA'), boards).convert('RGB')
    lit_neon = glow(neon, ((2, 1.0), (8, 0.95), (22, 0.8), (60, 0.55)))
    img = add(img, lit_neon)
    hot = neon.point(lambda v: v).filter(ImageFilter.GaussianBlur(0.6))
    img = add(img, Image.blend(Image.new('RGBA', (W, H), (0, 0, 0, 0)), hot, 0.35))

    if street:
        sy = horizon + 60
        shops = Image.new('RGBA', (W, H), (0, 0, 0, 0))
        sd = ImageDraw.Draw(shops)
        x = -20
        while x < W:
            sw = rnd.randint(160, 300)
            warm = hexc(rnd.choice(['#FFB86B', '#FFD8A0', '#9FE0FF', '#FF9FC0']), rnd.randint(110, 190))
            sd.rectangle((x + 10, sy + 40, x + sw - 10, sy + 150), fill=warm)
            sd.rectangle((x, sy + 20, x + sw, sy + 40), fill=(12, 10, 16, 255))
            x += sw + rnd.randint(6, 24)
        img = add(img, glow(shops.filter(ImageFilter.GaussianBlur(3)), ((6, 0.8), (30, 0.6))))
        # Awnings and pillars over the shopfronts
        dark = Image.new('RGBA', (W, H), (0, 0, 0, 0))
        dd = ImageDraw.Draw(dark)
        for px in range(0, W, 230):
            dd.rectangle((px, sy + 30, px + 14, sy + 170), fill=(5, 5, 8, 255))
        img = Image.alpha_composite(img.convert('RGBA'), dark.filter(ImageFilter.GaussianBlur(1))).convert('RGB')
        if stall:
            # A noodle stall: red lanterns, noren curtains, a warm counter, steam
            cx = int(W * 0.42)
            stall_l = Image.new('RGBA', (W, H), (0, 0, 0, 0))
            st = ImageDraw.Draw(stall_l)
            st.rectangle((cx - 230, sy + 60, cx + 230, sy + 170), fill=(255, 178, 102, 210))
            for i in range(6):
                nx = cx - 220 + i * 74
                st.rectangle((nx, sy + 24, nx + 66, sy + 92), fill=(28, 34, 70, 245))
                st.text((nx + 18, sy + 36), 'らーめん'[i % 4], font=jp_small, fill=(240, 236, 226, 255))
            img = Image.alpha_composite(img.convert('RGBA'), stall_l.filter(ImageFilter.GaussianBlur(0.8))).convert('RGB')
            lanterns = Image.new('RGBA', (W, H), (0, 0, 0, 0))
            ldr = ImageDraw.Draw(lanterns)
            for i in range(5):
                lx = cx - 260 + i * 130
                ldr.ellipse((lx, sy - 40, lx + 46, sy + 16), fill=(255, 70, 40, 255))
                ldr.ellipse((lx + 12, sy - 30, lx + 34, sy + 6), fill=(255, 190, 120, 255))
            img = add(img, glow(lanterns, ((3, 1.0), (14, 0.9), (44, 0.7))))
            steam = Image.new('RGBA', (W, H), (0, 0, 0, 0))
            sdr = ImageDraw.Draw(steam)
            for i in range(14):
                r = rnd.randint(30, 80)
                sx, sy2 = cx + rnd.randint(-160, 160), sy + rnd.randint(-160, 40)
                sdr.ellipse((sx - r, sy2 - r, sx + r, sy2 + r), fill=(235, 235, 240, rnd.randint(20, 45)))
            img = Image.alpha_composite(img.convert('RGBA'), steam.filter(ImageFilter.GaussianBlur(26))).convert('RGB')
        # People with umbrellas, in silhouette
        people = Image.new('RGBA', (W, H), (0, 0, 0, 0))
        pd = ImageDraw.Draw(people)
        for _ in range(7):
            px = rnd.randint(80, W - 80)
            base_y = sy + 210 + rnd.randint(-10, 30)
            hgt = rnd.randint(150, 200)
            pd.rounded_rectangle((px - 14, base_y - hgt + 40, px + 14, base_y), 10, fill=(4, 4, 7, 255))
            pd.ellipse((px - 11, base_y - hgt + 18, px + 11, base_y - hgt + 42), fill=(4, 4, 7, 255))
            ur = rnd.randint(46, 62)
            pd.pieslice((px - ur, base_y - hgt - 18, px + ur, base_y - hgt + 46), 180, 360,
                        fill=(4, 4, 7, 255) if rnd.random() < 0.6 else hexc(rnd.choice(['#2A1E3A', '#1E2A3A', '#3A1E24'])))
        img = Image.alpha_composite(img.convert('RGBA'), people.filter(ImageFilter.GaussianBlur(1.2))).convert('RGB')

        # The wet road: the scene above it, flipped, smeared down and rippled, darker
        road_top = sy + 210
        above = img.crop((0, road_top - (H - road_top) * 2, W, road_top)).transpose(Image.FLIP_TOP_BOTTOM)
        above = above.resize((W, H - road_top))
        smear = above.resize((W // 2, (H - road_top) // 10), Image.BILINEAR).resize((W, H - road_top), Image.BICUBIC)
        refl = Image.blend(above.filter(ImageFilter.GaussianBlur(6)), smear, 0.55)
        ripple = Image.new('RGB', refl.size)
        for y in range(refl.height):
            off = int(math.sin(y * 0.35) * 3 + math.sin(y * 0.11) * 4)
            ripple.paste(refl.crop((0, y, W, y + 1)), (off, y))
        road = vgrad(W, H - road_top, [(0, '#0B0D14'), (1, '#030306')])
        mask = vgrad(W, H - road_top, [(0, '#8C8C8C'), (0.6, '#3C3C3C'), (1, '#141414')]).convert('L')
        road = Image.composite(ripple, road, mask)
        grit = noise(W, H - road_top, 2, seed + 99).point(lambda v: int(v * 0.12))
        road = ImageChops.add(road, Image.merge('RGB', (grit, grit, grit)))
        img.paste(road, (0, road_top))
        # Puddle ripples: thin bright ellipses
        rip = Image.new('RGBA', (W, H), (0, 0, 0, 0))
        rd = ImageDraw.Draw(rip)
        for _ in range(60):
            rx, ry = rnd.randint(0, W), rnd.randint(road_top + 20, H)
            rr = rnd.randint(6, 26)
            rd.ellipse((rx - rr, ry - rr * 0.25, rx + rr, ry + rr * 0.25), outline=(200, 210, 230, rnd.randint(25, 60)), width=1)
        img = Image.alpha_composite(img.convert('RGBA'), rip.filter(ImageFilter.GaussianBlur(0.6))).convert('RGB')
    return vignette(img, 0.5)


def rain_sheet(name, n, length, width, alpha, angle, blur, seed, size=512):
    """A seamless (top to bottom) tile of motion-blurred rain streaks, light at the head and fading at the tail."""
    rnd = random.Random(seed)
    tile = Image.new('RGBA', (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(tile)
    dx = math.tan(math.radians(angle))
    for _ in range(n):
        x, y = rnd.uniform(0, size), rnd.uniform(0, size)
        ln = rnd.uniform(*length)
        a = rnd.uniform(*alpha)
        wdt = rnd.choice(width)
        segs = 10
        for s in range(segs):
            t0, t1 = s / segs, (s + 1) / segs
            col = (210, 225, 245, int(a * (0.2 + 0.8 * t1)))
            for oy in (-size, 0, size):        # wrap so the tile loops seamlessly
                for ox in (-size, 0, size):
                    d.line((x + ox + dx * ln * t0, y + oy + ln * t0, x + ox + dx * ln * t1, y + oy + ln * t1), fill=col, width=wdt)
    tile = tile.filter(ImageFilter.GaussianBlur(blur))
    save(tile, name)


# MARK: wallpapers

def luna():
    """Rolling green hills under a deep blue sky with soft clouds (an original drawing)."""
    img = vgrad(W, H, [(0, '#1F5FC8'), (0.45, '#5FA2EE'), (0.7, '#B9DDFB'), (1, '#D9EEFF')])
    cl = fbm(W, H, 7).point(lambda v: int(max(0, v - 118) * 3.2))
    cl = cl.filter(ImageFilter.GaussianBlur(3))
    top_fade = vgrad(W, H, [(0, '#FFFFFF'), (0.55, '#9A9A9A'), (0.7, '#000000'), (1, '#000000')]).convert('L')
    cl = ImageChops.multiply(cl, top_fade)
    clouds = Image.new('RGBA', (W, H), (255, 255, 255, 0))
    clouds.putalpha(cl)
    img = Image.alpha_composite(img.convert('RGBA'), clouds).convert('RGB')

    def hill(y0, amp, freq, phase, colors, seed):
        mask = Image.new('L', (W, H), 0)
        pts = [(x, y0 - amp * math.sin(x / W * math.pi * freq + phase) - amp * 0.35 * math.sin(x / W * 7 + seed)) for x in range(0, W + 8, 8)]
        ImageDraw.Draw(mask).polygon(pts + [(W, H), (0, H)], fill=255)
        mask = mask.filter(ImageFilter.GaussianBlur(1.5))
        fill = vgrad(W, H, colors)
        grass = noise(W, H, 2, seed).point(lambda v: int((v - 128) * 0.25 + 128))
        fill = ImageChops.overlay(fill, Image.merge('RGB', (grass, grass, grass)))
        return fill, mask
    for y0, amp, freq, phase, cols, s, blur in [
        (720, 90, 1.3, 0.6, [(0, '#7DBB4B'), (0.75, '#3F8A1E'), (1, '#2A6A12')], 3, 3),
        (820, 120, 1.1, 2.2, [(0, '#8CCB52'), (0.8, '#3D8C1A'), (1, '#24600E')], 5, 0),
    ]:
        fill, mask = hill(y0, amp, freq, phase, cols, s)
        if blur:
            fill = fill.filter(ImageFilter.GaussianBlur(blur))
        img = Image.composite(fill, img, mask)
    sun = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(sun).ellipse((W * 0.62, -200, W * 1.1, 300), fill=(255, 250, 220, 90))
    img = add(img, sun.filter(ImageFilter.GaussianBlur(160)))
    save(img, 'luna-hills.jpg')


def ribbons(name, bg, colors, seed, count=6, thickness=(60, 180), blur=(30, 90), core=True, stars=False):
    rnd = random.Random(seed)
    img = vgrad(W, H, bg)
    if stars:
        st = Image.new('RGBA', (W, H), (0, 0, 0, 0))
        sd = ImageDraw.Draw(st)
        for _ in range(500):
            x, y = rnd.randint(0, W), rnd.randint(0, int(H * 0.7))
            r = rnd.choice([0.6, 0.8, 1, 1.4])
            sd.ellipse((x - r, y - r, x + r, y + r), fill=(255, 255, 255, rnd.randint(60, 200)))
        img = add(img, st.filter(ImageFilter.GaussianBlur(0.4)))
    for i in range(count):
        lay = Image.new('RGBA', (W, H), (0, 0, 0, 0))
        d = ImageDraw.Draw(lay)
        c = hexc(colors[i % len(colors)], rnd.randint(110, 200))
        y0, amp, f1, f2, ph = rnd.uniform(0.25, 0.75) * H, rnd.uniform(60, 220), rnd.uniform(0.6, 1.6), rnd.uniform(2, 4), rnd.uniform(0, 6)
        pts = [(x, y0 + amp * math.sin(x / W * math.pi * f1 + ph) + amp * 0.3 * math.sin(x / W * math.pi * f2 + ph * 2)) for x in range(-50, W + 60, 10)]
        d.line(pts, fill=c, width=int(rnd.uniform(*thickness)), joint='curve')
        lay = lay.filter(ImageFilter.GaussianBlur(rnd.uniform(*blur)))
        img = add(img, lay)
        if core:
            thin = Image.new('RGBA', (W, H), (0, 0, 0, 0))
            ImageDraw.Draw(thin).line(pts, fill=(255, 255, 255, 120), width=3, joint='curve')
            img = add(img, glow(thin.filter(ImageFilter.GaussianBlur(1.5)), ((4, 0.7), (14, 0.5))))
    save(vignette(img, 0.35), name)


def bloom(name, bg, petal_colors, seed):
    """Layered translucent petals around a centre, softly lit (an original take on a bloom wallpaper)."""
    rnd = random.Random(seed)
    img = vgrad(W, H, bg)
    cx, cy = W * 0.62, H * 0.58
    for layer_i in range(3):
        lay = Image.new('RGBA', (W, H), (0, 0, 0, 0))
        for k in range(7):
            petal = Image.new('RGBA', (900, 380), (0, 0, 0, 0))
            pd = ImageDraw.Draw(petal)
            c = hexc(petal_colors[(k + layer_i) % len(petal_colors)], 120 - layer_i * 25)
            pd.ellipse((40, 40, 860, 340), fill=c)
            hi = hexc('#FFFFFF', 60)
            pd.ellipse((120, 80, 700, 180), fill=hi)
            petal = petal.filter(ImageFilter.GaussianBlur(10 + layer_i * 6))
            ang = k * (360 / 7) + layer_i * 17 + rnd.uniform(-6, 6)
            rot = petal.rotate(ang, expand=True, resample=Image.BICUBIC)
            scale = 1 - layer_i * 0.2
            rot = rot.resize((int(rot.width * scale), int(rot.height * scale)), Image.BICUBIC)
            lay.alpha_composite(rot, (int(cx - rot.width / 2), int(cy - rot.height / 2)))
        img = add(img, lay)
    img = add(img, glow(Image.new('RGBA', (W, H), (0, 0, 0, 0)), ((1, 1),)))
    save(vignette(img, 0.3), name)


def blobs(name, bg, colors, seed, blur=140, count=7):
    rnd = random.Random(seed)
    img = vgrad(W, H, bg).convert('RGBA')
    lay = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(lay)
    for i in range(count):
        r = rnd.randint(260, 520)
        x, y = rnd.randint(-100, W + 100), rnd.randint(-100, H + 100)
        d.ellipse((x - r, y - r, x + r, y + r), fill=hexc(colors[i % len(colors)], rnd.randint(150, 230)))
    img = Image.alpha_composite(img, lay.filter(ImageFilter.GaussianBlur(blur))).convert('RGB')
    grain = noise(W, H, 1, seed).point(lambda v: int((v - 128) * 0.06 + 128))
    img = ImageChops.overlay(img, Image.merge('RGB', (grain, grain, grain)))
    return img


def bokeh(img, seed, n=40, colors=('#FFFFFF',)):
    rnd = random.Random(seed)
    lay = Image.new('RGBA', img.size, (0, 0, 0, 0))
    d = ImageDraw.Draw(lay)
    for _ in range(n):
        r = rnd.randint(10, 46)
        x, y = rnd.randint(0, img.width), rnd.randint(0, img.height)
        d.ellipse((x - r, y - r, x + r, y + r), fill=hexc(rnd.choice(colors), rnd.randint(25, 70)), outline=hexc('#FFFFFF', 40), width=2)
    return add(img, lay.filter(ImageFilter.GaussianBlur(3)))


def brushed(name, base, streak, size=(W, H), seed=5):
    n = Image.effect_noise((size[0] // 8, size[1]), 70).resize(size, Image.BICUBIC).filter(ImageFilter.GaussianBlur(0.6))
    n = n.point(lambda v: int((v - 128) * streak + 128))
    img = Image.new('RGB', size, hexc(base)[:3])
    img = ImageChops.overlay(img, Image.merge('RGB', (n, n, n)))
    return img


ART = {
    'night-street': lambda: save(city(11, [
        {'text': 'ラーメン', 'x': 0.08, 'y': 0.14, 'color': '#FF3D7F'},
        {'text': '居酒屋', 'x': 0.2, 'y': 0.08, 'color': '#2FE0FF'},
        {'text': 'カラオケ', 'x': 0.3, 'y': 0.2, 'color': '#B66BFF', 'big': False},
        {'text': '電脳', 'x': 0.62, 'y': 0.1, 'color': '#FFC23D'},
        {'text': 'ホテル', 'x': 0.74, 'y': 0.18, 'color': '#FF6BD6'},
        {'text': '薬', 'x': 0.86, 'y': 0.26, 'color': '#4DFF9D'},
        {'text': '寿司', 'x': 0.53, 'y': 0.3, 'color': '#FF5A3D', 'big': False},
        {'text': '24時間営業', 'x': 0.36, 'y': 0.44, 'color': '#2FE0FF', 'vertical': False, 'big': False},
        {'text': '夜市', 'x': 0.92, 'y': 0.12, 'color': '#FF3D7F'},
    ], {'haze': '#2A1838', 'fog': '#B8A0D8', 'neon': ['#FF3D7F', '#2FE0FF', '#B66BFF', '#FFC23D']}), 'night-street.jpg'),
    'noir-city': lambda: save(city(23, [
        {'text': 'HOTEL', 'x': 0.66, 'y': 0.16, 'color': '#FFB347', 'vertical': False},
        {'text': 'NOODLES', 'x': 0.14, 'y': 0.3, 'color': '#38E8FF', 'vertical': False, 'big': False},
        {'text': '電話', 'x': 0.82, 'y': 0.06, 'color': '#38E8FF'},
        {'text': 'BAR', 'x': 0.44, 'y': 0.38, 'color': '#FF7A3D', 'vertical': False, 'big': False},
    ], {'haze': '#1E2A36', 'fog': '#8FA8C0', 'neon': ['#FFB347', '#38E8FF']}, stall=False), 'noir-city.jpg'),
    # The rain tiles are made by make-rain.py (tuned for the scenes' 12 frames a second).
    'rain': lambda: None,
    'luna': luna,
    'aurora': lambda: ribbons('aurora-glass.jpg', [(0, '#020A12'), (0.6, '#04202A'), (1, '#062A2A')],
                              ['#2FFF9A', '#1BD6C6', '#3F8BFF', '#7DFFB8'], 3, count=7, thickness=(80, 220), blur=(40, 110), core=False, stars=True),
    'superbar': lambda: ribbons('superbar.jpg', [(0, '#0A2E5C'), (0.55, '#1868B4'), (1, '#0A2550')],
                                ['#9AD8FF', '#4FB2FF', '#FFFFFF'], 7, count=4, thickness=(40, 120), blur=(20, 60)),
    'aqua': lambda: ribbons('aqua.jpg', [(0, '#062A6E'), (0.5, '#0E56C2'), (1, '#1B7FE8')],
                            ['#8CC4FF', '#CFE4FF', '#5A9BFF'], 13, count=5, thickness=(50, 140), blur=(26, 70)),
    # Fluent's bloom is made by make-fluent-bloom.py (folded, shaded petals; needs numpy).
    'fluent': lambda: None,
    'big-glass': lambda: save(blobs('big-glass.jpg', [(0, '#F6E9F7'), (1, '#E4EEFB')], ['#FF8FB8', '#8F9BFF', '#7FE0D8', '#FFC07A', '#C59BFF'], 9), 'big-glass.jpg'),
    # A dawn wallpaper: deep blue night lifting into a warm amber glow (the colour rule: no teal, purple or pink).
    'home-screen': lambda: save(bokeh(blobs('home-screen.jpg', [(0, '#0B1A3A'), (0.55, '#2457A8'), (1, '#F2A25A')],
                                            ['#3D8BFF', '#FFB86B', '#FF8A3D', '#2A5BD7'], 21, blur=120), 4, 30, ('#FFFFFF', '#FFE2C2')), 'home-screen.jpg'),
    'platinum': lambda: save(brushed('platinum.jpg', '#DADADA', 0.18, seed=2), 'platinum.jpg'),
    'llama': lambda: (save(brushed('llama-metal.jpg', '#24242C', 0.35, seed=3), 'llama-metal.jpg'),
                      save(Image.composite(brushed('', '#D4AF37', 0.5, (W, 48), 4), Image.new('RGB', (W, 48), (60, 48, 14)),
                                           vgrad(W, 48, [(0, '#FFFFFF'), (0.5, '#C8C8C8'), (1, '#606060')]).convert('L')), 'llama-gold.jpg')),
}


if __name__ == '__main__':
    names = sys.argv[1:] or list(ART)
    for n in names:
        ART[n]()
