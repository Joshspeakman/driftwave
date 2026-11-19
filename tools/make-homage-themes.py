"""Writes the homage themes (themes/*.json): tributes to computers, phones and consoles people loved.

Design rules (Josh, 2026-11-18):
- Nothing is drawn over the window's own parts: themes restyle the real window title ("windowTitle") instead of
  drawing a second one, and title-bar decorations live only in the free stretch between the title and the
  caption buttons (scene region "titlebar"). No fake taskbars, docks, icons or gadgets in the content area.
- Realistic themes use real photographs (Wikimedia Commons, free licences; assets/theme-art/CREDITS.txt) or
  rendered art (windows/tools/make-theme-art.py), with realistic rain, behind smoky-glass surfaces.
- Only the pixel themes (DOS, Atari, Nokia, SNES, Genesis) use pixel art, and only in the title-bar chrome.
- Original work in the spirit of each subject: no logos, artwork or other assets from the originals.
Usage: python windows/tools/make-homage-themes.py
"""
import json
import os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
OUT = os.path.join(ROOT, 'themes')

# MARK: pixel art for the pixel themes' title-bar chrome (original drawings; "." is clear)
JOYSTICK = [".....RR.....", "....RRRR....", "....RRRR....", ".....KK.....", ".....KK.....", "KKKKKKKKKKKK", "KWWWWWWWWWRK", "KKKKKKKKKKKK"]
SIGNAL = ["......KK", "......KK", "....KKKK", "....KKKK", "..KKKKKK", "..KKKKKK", "KKKKKKKK", "KKKKKKKK"]
BATTERY = ["KKKKKKKKKKKK.", "KGGGGGGGGGGKK", "KGGGGGGGGGGKK", "KGGGGGGGGGGKK", "KKKKKKKKKKKK."]
HOURGLASS = ["KKKKKKKKK", ".KWWWWWK.", "..KYYYK..", "...KYK...", "....K....", "...KWK...", "..KWYWK..", ".KYYYYYK.", "KKKKKKKKK"]
STAR = ["....Y....", "...YYY...", "YYYYYYYYY", ".YYYYYYY.", "..YYYYY..", ".YYY.YYY.", "YY.....YY"]


def px(rows, x, y, scale, palette, **kw):
    d = {"type": "pixels", "rows": rows, "palette": palette, "x": x, "y": y, "scale": scale, "in": "titlebar"}
    d.update(kw)
    return d


def text(t, x, y, size, color, region="titlebar", **kw):
    d = {"type": "text", "text": t, "x": x, "y": y, "size": size, "color": color, "in": region}
    d.update(kw)
    return d


def photo(src, darken_top="#CC000000"):
    """A full-window photograph, darkened under the title bar so the title stays readable."""
    return [{"type": "image", "src": src, "in": "window", "x": 0, "y": 0},
            {"type": "rect", "in": "titlebar-full", "x": 0, "y": 0, "w": "100%", "h": 48, "fill": [darken_top, "#00000000"]}]


def rain(far_opacity=0.5, near_opacity=0.7):
    """Two sheets of motion-blurred rain at different speeds (parallax), behind the UI."""
    return [{"type": "image", "src": "rain-far.png", "in": "window", "x": 0, "y": 0, "loopY": 1.7, "tile": 512, "opacity": far_opacity},
            {"type": "image", "src": "rain-near.png", "in": "window", "x": 0, "y": 0, "loopY": 0.75, "tile": 512, "opacity": near_opacity}]


def tokens(background, surface, card, card_stroke, control, control_stroke, subtle, divider, text_, text2, text3, accent,
           accent_text, on_accent, dialog, pane=None):
    c = {"background": background, "surface": surface, "card": card, "cardStroke": card_stroke, "control": control,
         "controlStroke": control_stroke, "subtle": subtle, "divider": divider, "text": text_, "textSecondary": text2,
         "textTertiary": text3, "accent": accent, "accentText": accent_text, "onAccent": on_accent, "dialog": dialog}
    if pane:
        c["pane"] = pane
    return c


def theme(id, name, description, base, backdrop, colors, fonts, corners, effects):
    return {"format": "shortwave-theme", "version": 1, "id": id, "name": name, "description": description, "base": base,
            "backdrop": backdrop, "colors": colors, "fonts": fonts, "corners": corners, "effects": effects}


THEMES = []

# MARK: Windows

THEMES.append(theme(
    "program-manager", "Program Manager",
    "A homage to Windows 3.1: a white workspace, a solid navy title bar, crisp grey bevels.",
    "light", "none",
    tokens("#C0C0C0", "#FFFFFF", "#FFFFFF", "#000000", "#C0C0C0", "#000000", "#330000A8", "#808080", "#000000",
           "#202020", "#606060", "#000080", "#000080", "#FFFFFF", "#FFFFFF", pane="#FFFFFF"),
    {"body": "Microsoft Sans Serif", "title": "Microsoft Sans Serif", "titleSize": 22, "titleWeight": "Bold"},
    {"control": 0, "overlay": 0, "card": 0, "cover": 0},
    {"frame": {"color": "#000080", "thickness": 4},
     "windowTitle": {"font": "Microsoft Sans Serif", "size": 13, "weight": "bold", "color": "#FFFFFF"},
     "scene": {
         "layers": [
             {"type": "rect", "in": "titlebar-full", "x": 0, "y": 0, "w": "100%", "h": 48, "fill": "#000080"},
             {"type": "bevel", "in": "titlebar", "x": 0, "y": 12, "w": 26, "h": 24, "fill": "#C0C0C0", "edge": 2, "id": "menu", "clickable": True},
             {"type": "rect", "in": "titlebar", "x": 6, "y": 22, "w": 14, "h": 4, "fill": "#000000"},
         ],
         "eggs": [
             {"trigger": "type:solitaire", "run": "cascade", "seconds": 9},
             {"trigger": "click:menu", "say": "Restore, Move, Size, Minimise, Maximise, Close, Switch To... Driftwave.", "seconds": 5},
             {"trigger": "type:win", "say": "C:\\> win  -- starting Driftwave for Workgroups...", "seconds": 5},
         ]}}))

THEMES.append(theme(
    "ninety-five", "Ninety-Five",
    "A homage to Windows 95: a teal desktop, grey 3D windows, and a navy-to-blue title bar.",
    "light", "none",
    tokens("#008080", "#00008080", "#C0C0C0", "#000000", "#C0C0C0", "#000000", "#330000A8", "#808080", "#000000",
           "#222222", "#555555", "#000080", "#000080", "#FFFFFF", "#C0C0C0", pane="#C0C0C0"),
    {"body": "Microsoft Sans Serif", "title": "Microsoft Sans Serif", "titleSize": 22, "titleWeight": "Bold"},
    {"control": 0, "overlay": 0, "card": 0, "cover": 0},
    {"windowTitle": {"font": "Microsoft Sans Serif", "size": 13, "weight": "bold", "color": "#FFFFFF"},
     "scene": {
         "layers": [
             {"type": "rect", "in": "titlebar-full", "x": 0, "y": 0, "w": "100%", "h": 48, "fill": ["#000080", "#1084D0"], "angle": "h"},
             {"type": "bevel", "in": "titlebar", "x": 0, "y": 10, "w": 66, "h": 28, "fill": "#C0C0C0", "id": "start", "clickable": True},
             text("Start", 22, 14, 13, "#000000", font="Microsoft Sans Serif", weight="bold"),
             {"type": "rect", "in": "titlebar", "x": 8, "y": 17, "w": 6, "h": 6, "fill": "#FF0000"},
             {"type": "rect", "in": "titlebar", "x": 14, "y": 17, "w": 6, "h": 6, "fill": "#00A000"},
             {"type": "rect", "in": "titlebar", "x": 8, "y": 23, "w": 6, "h": 6, "fill": "#0000FF"},
             {"type": "rect", "in": "titlebar", "x": 14, "y": 23, "w": 6, "h": 6, "fill": "#FFD800"},
             {"type": "bevel", "in": "titlebar", "x": -84, "y": 10, "w": 82, "h": 28, "fill": "#C0C0C0", "style": "sunken"},
             text("{time}", -70, 14, 12, "#000000", font="Microsoft Sans Serif"),
             text("It's now safe to turn off your computer.", "18%", "40%", 26, "#FF8C1A", region="content", font="Microsoft Sans Serif",
                  weight="bold", id="safe", hidden=True),
         ],
         "eggs": [
             {"trigger": "click:start", "say": "Start \u25B8 Programs \u25B8 Accessories \u25B8 Entertainment \u25B8 Driftwave. You're already here.", "seconds": 6},
             {"trigger": "type:shutdown", "show": "safe", "seconds": 6},
             {"trigger": "type:clouds", "say": "Somewhere a blue sky full of clouds is waiting behind every window.", "seconds": 5},
         ]}}))

THEMES.append(theme(
    "luna", "Luna",
    "A homage to Windows XP: green hills under a big blue sky (a real photograph), a glossy blue title bar.",
    "light", "none",
    tokens("#245EDC", "#66F4F8FF", "#D9EEF3FB", "#7A96DF", "#F4F7FD", "#7A96DF", "#333C7FE0", "#ACA899", "#000000",
           "#2A2A2A", "#5A5A5A", "#245EDC", "#0B3FB0", "#FFFFFF", "#ECE9D8", pane="#C6D3F5FF"),
    {"body": "Tahoma", "title": "Trebuchet MS", "titleSize": 28, "titleWeight": "Bold"},
    {"control": 3, "overlay": 6, "card": 8, "cover": 0.1},
    {"windowTitle": {"font": "Trebuchet MS", "size": 14, "weight": "bold", "color": "#FFFFFF"},
     "scene": {
         "surfaces": "translucent",
         "layers": photo("photo-hills.jpg", "#00000000") + [
             {"type": "rect", "in": "titlebar-full", "x": 0, "y": 0, "w": "100%", "h": 48,
              "fill": [["#0A58E2", 0], ["#3A93FF", 0.12], ["#0053E1", 0.5], ["#0044C8", 1]]},
             {"type": "rect", "in": "titlebar", "x": 0, "y": 9, "w": 90, "h": 30, "fill": [["#5FC24E", 0], ["#3C9A3C", 0.5], ["#2E7D2C", 1]],
              "radius": 6, "id": "go", "clickable": True},
             text("start", 18, 11, 17, "#FFFFFF", font="Trebuchet MS", weight="bold", style="italic"),
         ],
         "eggs": [
             {"trigger": "click:go", "say": "Start \u25B8 All Programs \u25B8 Driftwave. Good call.", "seconds": 5},
             {"trigger": "idle:90", "say": "New radio found. Click here to keep listening.", "seconds": 7},
             {"trigger": "type:bliss", "say": "Somewhere a hill is still this green.", "seconds": 5},
         ]}}))

THEMES.append(theme(
    "aurora-glass", "Aurora Glass",
    "A homage to Windows Vista: smoky glass over a real aurora, with a soft glow on everything.",
    "dark", "none",
    tokens("#020A12", "#66081620", "#8C0E1A28", "#5582C8FF", "#66203040", "#6682C8FF", "#3382C8FF", "#4482C8FF", "#FFFFFF",
           "#C8DCF0", "#8FA8C0", "#3FA0E8", "#7FC8FF", "#00121F", "#E6101C28", pane="#8C06101A"),
    {"body": "Segoe UI", "title": "Segoe UI Light", "titleSize": 32},
    {"control": 4, "overlay": 6, "card": 6, "cover": 0.08},
    {"windowTitle": {"font": "Segoe UI", "size": 13, "color": "#FFFFFF"},
     "scene": {
         "surfaces": "translucent",
         "layers": photo("photo-aurora.jpg", "#99000000") + [
             {"type": "radial", "in": "titlebar", "x": 0, "y": 6, "w": 36, "h": 36, "fill": [["#DDFFFFFF", 0], ["#883FA0E8", 0.5], ["#00000000", 1]],
              "id": "orb", "clickable": True},
         ],
         "eggs": [
             {"trigger": "click:orb", "say": "The wow starts... whenever you press play.", "seconds": 5},
             {"trigger": "type:wow", "say": "Wow.", "seconds": 3},
         ]}}))

THEMES.append(theme(
    "superbar", "Superbar",
    "A homage to Windows 7: clear glass over a deep blue swoosh of light.",
    "dark", "none",
    tokens("#0A2550", "#550A1E3A", "#801A3A60", "#5580B8E8", "#55183A5A", "#6680B8E8", "#3380B8E8", "#4480B8E8", "#FFFFFF",
           "#CFE2F5", "#93AFCB", "#2E8BE6", "#8CC7FF", "#00142A", "#E6142A44", pane="#80081A33"),
    {"body": "Segoe UI", "title": "Segoe UI", "titleSize": 28},
    {"control": 3, "overlay": 6, "card": 6, "cover": 0.1},
    {"windowTitle": {"font": "Segoe UI", "size": 13, "color": "#FFFFFF"},
     "scene": {
         "surfaces": "translucent",
         "layers": photo("superbar.jpg", "#66000000") + [
             {"type": "radial", "in": "titlebar", "x": 0, "y": 6, "w": 36, "h": 36, "fill": [["#FFFFFFFF", 0], ["#FF2E8BE6", 0.45], ["#FF0B3C7A", 1]],
              "id": "orb", "clickable": True},
         ],
         "eggs": [
             {"trigger": "click:orb", "say": "Pinned to the superbar: Driftwave. Of course.", "seconds": 5},
             {"trigger": "type:shake", "say": "You'd shake a window to clear the others away. Here, everything stays.", "seconds": 5},
         ]}}))

THEMES.append(theme(
    "fluent", "Fluent",
    "A homage to Windows 11: a soft blue bloom of light behind rounded, gently translucent surfaces.",
    "dark", "none",
    tokens("#071022", "#59101828", "#801C2438", "#22FFFFFF", "#33FFFFFF", "#33FFFFFF", "#1AFFFFFF", "#22FFFFFF", "#FFFFFF",
           "#D0D6E6", "#9AA3B8", "#4CC2FF", "#99DCFF", "#002033", "#EE1C2233", pane="#8C0B1426"),
    {"body": "Segoe UI Variable Text", "title": "Segoe UI Variable Display", "titleSize": 30, "titleWeight": "SemiBold"},
    {"control": 4, "overlay": 8, "card": 8, "cover": 0.16},
    {"windowTitle": {"font": "Segoe UI Variable Text", "size": 12, "color": "#FFFFFF"},
     "scene": {
         "surfaces": "translucent",
         "layers": photo("fluent-bloom.jpg", "#55000000"),
         "eggs": [{"trigger": "konami", "say": "Recommended: keep listening.", "seconds": 5}]}}))

# MARK: Mac

THEMES.append(theme(
    "platinum", "Platinum",
    "A homage to classic Mac OS: platinum grey, a pinstriped title bar, and calm, crisp black type.",
    "light", "none",
    tokens("#DDDDDD", "#F2EEEEEE", "#FFFFFF", "#000000", "#EEEEEE", "#555555", "#33666699", "#999999", "#000000",
           "#262626", "#666666", "#666699", "#333399", "#FFFFFF", "#EEEEEE", pane="#E4E4E4"),
    {"body": "Segoe UI", "title": "Segoe UI Semibold", "titleSize": 22},
    {"control": 3, "overlay": 2, "card": 2, "cover": 0.04},
    {"windowTitle": {"font": "Segoe UI Semibold", "size": 13, "color": "#000000"},
     "scene": {
         "layers": [
             {"type": "image", "src": "platinum.jpg", "in": "window", "x": 0, "y": 0, "opacity": 0.9},
             {"type": "line", "in": "titlebar", "x": 0, "y": 15, "x2": "100%", "y2": 0, "stroke": "#9A9A9A", "repeat": {"n": 7, "dy": 3}},
             text("Welcome to Driftwave.", "20%", "38%", 22, "#000000", region="content", font="Segoe UI Semibold", id="welcome", hidden=True),
         ],
         "eggs": [
             {"trigger": "konami", "show": "welcome", "seconds": 6},
             {"trigger": "type:sosumi", "say": "Sosumi. (Some sounds have stories.)", "seconds": 5},
         ]}}))

THEMES.append(theme(
    "aqua", "Aqua",
    "A homage to Mac OS X: water-blue light, pinstripes and glossy gel.",
    "light", "none",
    tokens("#0E5AC2", "#99F5F7FA", "#E6FFFFFF", "#9FB3CC", "#F3F6FA", "#8EA4C0", "#33479BE8", "#C5CED9", "#111111",
           "#3A3F47", "#6A717B", "#1F7CE8", "#1366CC", "#FFFFFF", "#F4F6F9", pane="#B3E8EDF3"),
    {"body": "Lucida Sans Unicode", "title": "Lucida Sans Unicode", "titleSize": 24, "titleWeight": "Bold"},
    {"control": 10, "overlay": 8, "card": 10, "cover": 0.18},
    {"windowTitle": {"font": "Lucida Sans Unicode", "size": 13, "weight": "bold", "color": "#FFFFFF"},
     "scene": {
         "surfaces": "translucent",
         "layers": photo("aqua.jpg", "#55000000") + [
             {"type": "line", "in": "window", "x": 0, "y": 0, "x2": "100%", "y2": 0, "stroke": "#10FFFFFF", "repeat": {"n": 400, "dy": 4}},
             {"type": "radial", "in": "titlebar", "x": 0, "y": 16, "w": 16, "h": 16, "fill": [["#FFC2BC", 0], ["#FF5F57", 0.5], ["#C1342C", 1]], "id": "red", "clickable": True},
             {"type": "radial", "in": "titlebar", "x": 22, "y": 16, "w": 16, "h": 16, "fill": [["#FFE7B3", 0], ["#FEBC2E", 0.5], ["#C08A12", 1]]},
             {"type": "radial", "in": "titlebar", "x": 44, "y": 16, "w": 16, "h": 16, "fill": [["#C9F7C2", 0], ["#28C840", 0.5], ["#178A2A", 1]]},
         ],
         "eggs": [
             {"trigger": "click:red", "say": "That one's just for looks. The real close button is over on the right.", "seconds": 5},
             {"trigger": "type:lick", "say": "Lickable. (Please don't.)", "seconds": 5},
         ]}}))

THEMES.append(theme(
    "big-glass", "Big Glass",
    "A homage to today's Mac: frosted glass over a California coast at sunset (a real photograph).",
    "dark", "none",
    tokens("#1A1420", "#4D161220", "#73201828", "#33FFFFFF", "#33FFFFFF", "#44FFFFFF", "#22FFFFFF", "#33FFFFFF", "#FFFFFF",
           "#E6E1EC", "#B5ADC2", "#0A84FF", "#64B5FF", "#FFFFFF", "#EE1E1A26", pane="#661A1420"),
    {"body": "Segoe UI Variable Text", "title": "Segoe UI Variable Display", "titleSize": 34, "titleWeight": "Bold"},
    {"control": 10, "overlay": 14, "card": 18, "cover": 0.22},
    {"windowTitle": {"font": "Segoe UI Variable Text", "size": 13, "weight": "bold", "color": "#FFFFFF"},
     "scene": {
         "surfaces": "translucent",
         "layers": photo("photo-coast.jpg", "#88000000") + [
             text("File    Edit    View    Station    Window    Help", 4, 15, 13, "#F2FFFFFF", font="Segoe UI Variable Text", weight="semibold"),
             text("hello", "34%", "28%", 96, "#E6FFFFFF", region="content", font="Segoe Script", id="hello", hidden=True),
         ],
         "eggs": [
             {"trigger": "type:hello", "show": "hello", "seconds": 6},
             {"trigger": "konami", "say": "One more thing: it's still playing.", "seconds": 5},
         ]}}))

THEMES.append(theme(
    "home-screen", "Home Screen",
    "A homage to the phone in your pocket: a soft glowing wallpaper behind rounded glass.",
    "dark", "none",
    tokens("#2A1840", "#4D10101C", "#73FFFFFF", "#33FFFFFF", "#33FFFFFF", "#44FFFFFF", "#22FFFFFF", "#33FFFFFF", "#FFFFFF",
           "#F0ECF4", "#C8C0D2", "#0A84FF", "#64D2FF", "#FFFFFF", "#EE1C1C2A", pane="#59100A1C"),
    {"body": "Segoe UI Variable Text", "title": "Segoe UI Variable Display", "titleSize": 34, "titleWeight": "Bold"},
    {"control": 14, "overlay": 18, "card": 22, "cover": 0.26},
    {"windowTitle": {"font": "Segoe UI Variable Text", "size": 13, "weight": "bold", "color": "#FFFFFF"},
     "scene": {
         "surfaces": "translucent",
         "layers": photo("home-screen.jpg", "#55000000") + [
             {"type": "rect", "in": "titlebar", "x": "40%", "y": 9, "w": 120, "h": 30, "fill": "#000000", "radius": 15, "id": "island", "clickable": True},
             text("{time24}", -60, 14, 15, "#FFFFFF", font="Segoe UI Variable Text", weight="semibold"),
         ],
         "eggs": [
             {"trigger": "click:island", "say": "Now playing lives here too.", "seconds": 4},
             {"trigger": "type:slide", "say": "slide to unlock \u25B8\u25B8", "seconds": 5},
         ]}}))

# MARK: the street

def street_lights():
    """
    People in the street beyond the glass: soft out-of-focus lights (phones, lit umbrellas, a shop's glow on a coat)
    walking by with a slight bob, and now and then a car's headlights or tail lights. Each fades in and out.
    """
    def bokeh(colour, x, y, size, dx, seconds, pause, start, peak, bob=1.0, pair=None):
        c = colour.lstrip("#")
        item = {"type": "radial", "in": "window", "x": x, "y": y, "w": size, "h": size, "withPhoto": True,
                "fill": [["#E6" + c, 0], ["#80" + c, 0.35], ["#00" + c, 1]],
                "motion": {"kind": "pass", "x": dx, "bob": bob, "seconds": seconds, "pause": pause, "start": start,
                           "max": peak}}
        if pair:
            item["repeat"] = {"n": 2, "dx": pair}
            item["h"] = round(size * 0.65)
        return item
    return [
        bokeh("#FFF1DC", "18%", "74%", 26, 380, 34, 10, 0, 0.9, 1.2),
        bokeh("#FF6FA8", "62%", "71%", 34, -420, 40, 6, 13, 0.75),
        bokeh("#7FE8FF", "40%", "78%", 18, 300, 28, 16, 21, 0.85, 1.4),
        bokeh("#FFE2B0", "75%", "69%", 22, -300, 36, 12, 5, 0.8),
        bokeh("#FFC070", "8%", "80%", 30, 460, 44, 8, 30, 0.7, 1.1),
        bokeh("#DCE8FF", "50%", "66%", 14, -220, 30, 20, 17, 0.7, 0.6),
        bokeh("#FF8FD0", "30%", "70%", 20, 260, 38, 14, 9, 0.6, 0.9),
        # A car's headlights from the right, and later one's tail lights from the left.
        bokeh("#FFF6E8", "100%", "86%", 46, -1500, 14, 46, 8, 0.9, 0, pair=42),
        bokeh("#FF3B30", "-4%", "84%", 30, 1500, 17, 52, 35, 0.55, 0, pair=40),
    ]


THEMES.append(theme(
    "night-market", "Night Market",
    "A rainy night in Tokyo, seen through dark tinted glass: neon blooming through, rain running down the window "
    "(a real photograph of Kabukichō), and every panel lit like a neon tube.",
    "dark", "none",
    tokens("#04050A", "#8C06080F", "#9906080F", "#CC37E0FF", "#B30A0F1A", "#CCFF3D88", "#2637E0FF", "#4D37E0FF", "#F4F6FF",
           "#C8D2EA", "#8892AA", "#FF3D88", "#5CF2FF", "#0A0410", "#F2070910", pane="#B305070D"),
    {"body": "Yu Gothic UI", "title": "Yu Gothic UI", "titleSize": 30, "titleWeight": "Bold", "titleSpacing": 60},
    {"control": 2, "overlay": 4, "card": 4, "cover": 0.06},
    {"windowTitle": {"font": "Yu Gothic UI", "size": 13, "weight": "bold", "color": "#9FF6FF"},
     "vignette": {"opacity": 0.45},
     "titleShadow": {"color": "#FF3D88", "blur": 18},
     # The street through the glass, its neon glowing (breathing very slowly: a hum, never a flicker).
     "scene": {
         "surfaces": "translucent",
         "layers": photo("photo-night-market-glass.jpg", "#99000000") + [
             {"type": "image", "src": "night-market-neon.png", "in": "window", "x": 0, "y": 0, "opacity": 0.7,
              "motion": {"kind": "breathe", "seconds": 9, "min": 0.5, "max": 0.85}},
         ] + street_lights() + [
             # Heavy rain beyond the glass, out of focus like the street: a dense fine sheet and a nearer, softer one,
             # each moving along its own slant.
             {"type": "image", "src": "rain-glass-far.png", "in": "window", "x": 0, "y": 0, "loopY": 0.9, "loopX": 5.7,
              "tile": 512, "opacity": 0.55},
             {"type": "image", "src": "rain-glass-near.png", "in": "window", "x": 0, "y": 0, "loopY": 0.5, "loopX": 3.2,
              "tile": 512, "opacity": 0.6},
             text("夜市 ・ 雨", 4, 12, 16, "#FF8FB8", font="Yu Gothic UI", weight="bold", glow="#FF3D88", glowRadius=2),
             text("二つで十分ですよ！", "30%", "44%", 26, "#FFFFFF", region="content", font="Yu Gothic UI",
                  weight="bold", glow="#FF3D88", id="two", hidden=True),
         ],
         "eggs": [
             {"trigger": "type:noodles", "show": "two", "say": "Four bowls? Two is enough.", "seconds": 6},
             {"trigger": "idle:120", "say": "The rain never stops here. Neither does the radio.", "seconds": 6},
         ]},
     # Rain on the outside of the window: drops cling, then run down and leave wet trails.
     "drips": {"density": 2.4},
     # Neon tubes round the page (cyan) and the player (pink).
     "neonEdges": {"content": "#37E0FF", "player": "#FF3D88", "bar": "#FF3D88", "glow": 16, "opacity": 0.8}}))

# MARK: pixel themes (pixel art only in the title-bar chrome; plain textures behind the content)

THEMES.append(theme(
    "dos-prompt", "C:\\ Prompt",
    "A homage to DOS: grey text on black, a prompt in the title bar and box-drawn borders.",
    "dark", "none",
    tokens("#000000", "#000000", "#0000AA", "#AAAAAA", "#0000AA", "#AAAAAA", "#335555FF", "#555555", "#AAAAAA",
           "#AAAAAA", "#555555", "#55FFFF", "#55FFFF", "#000000", "#0000AA", pane="#000000"),
    {"body": "Consolas", "title": "VT323", "titleSize": 34, "titleUppercase": True},
    {"control": 0, "overlay": 0, "card": 0, "cover": 0},
    {"cursor": {"color": "#AAAAAA"},
     "scanlines": {"opacity": 0.08},
     "windowTitle": {"font": "VT323", "size": 20, "color": "#AAAAAA", "uppercase": True},
     "scene": {
         "layers": [
             text("C:\\>_", 6, 12, 22, "#AAAAAA", font="VT323"),
             px(HOURGLASS, 64, 12, 3, {"K": "#AAAAAA", "W": "#555555", "Y": "#55FFFF"}),
         ],
         "eggs": [
             {"trigger": "type:dir", "say": " Volume in drive C is DRIFTWAVE\n LIBRARY  <DIR>\n COVERS   <DIR>\n SONGS    <DIR>\n        3 dir(s)  plenty free", "seconds": 7},
             {"trigger": "type:ver", "say": "Driftwave Version 1.0 Beta 1", "seconds": 4},
             {"trigger": "type:help", "say": "DIR  VER  HELP  and one secret involving arrows.", "seconds": 5},
             {"trigger": "type:format", "say": "Nice try.", "seconds": 3},
             {"trigger": "konami", "run": "pong", "seconds": 12},
         ]}}))

THEMES.append(theme(
    "woodgrain", "Woodgrain",
    "A homage to the woodgrain consoles of 1977: walnut veneer, ribbed black plastic, chunky pixels and a joystick.",
    "dark", "none",
    tokens("#1A120B", "#E01C1410", "#EE141010", "#55E07A1F", "#E0201812", "#66E07A1F", "#33E07A1F", "#44E07A1F", "#F4E6CF",
           "#D4B48C", "#9C7E5C", "#E07A1F", "#F2A65A", "#1A120B", "#20160E", pane="#100B07"),
    {"body": "Segoe UI", "title": "Press Start 2P", "titleSize": 20, "titleUppercase": True},
    {"control": 0, "overlay": 2, "card": 0, "cover": 0},
    {"wood": {},
     "windowTitle": {"font": "Press Start 2P", "size": 10, "color": "#F2A65A", "uppercase": True},
     "scene": {
         "layers": [
             {"type": "rect", "in": "titlebar-full", "x": 0, "y": 0, "w": "100%", "h": 48, "fill": "#0E0E0E"},
             {"type": "line", "in": "titlebar-full", "x": 0, "y": 4, "x2": "100%", "y2": 0, "stroke": "#202020", "repeat": {"n": 10, "dy": 4}},
             *[{"type": "rect", "in": "titlebar", "x": -96 + i * 18, "y": 0, "w": 12, "h": 48, "fill": c} for i, c in enumerate(["#D8342C", "#E8782A", "#F2C230", "#4A9A3A"])],
             px(JOYSTICK, 4, 12, 3, {"R": "#D8342C", "K": "#3A3A3A", "W": "#E0E0E0"}, id="stick", clickable=True),
         ],
         "eggs": [
             {"trigger": "click:stick", "run": "pong", "seconds": 12},
             {"trigger": "konami", "run": "pong", "seconds": 12},
             {"trigger": "type:adventure", "say": "You found the secret room. The radio is in here.", "seconds": 5},
         ]}}))

THEMES.append(theme(
    "brick-phone", "Brick Phone",
    "A homage to the indestructible phones of 1999: a green backlit LCD, chunky pixels, signal bars and a snake.",
    "light", "none",
    tokens("#8FA87A", "#E6C7F0A8", "#D8B6DE8E", "#43523D", "#C7F0A8", "#43523D", "#3343523D", "#43523D", "#2B3627",
           "#43523D", "#5E7356", "#43523D", "#2B3627", "#C7F0A8", "#C7F0A8", pane="#9DB689"),
    {"body": "Consolas", "title": "VT323", "titleSize": 36},
    {"control": 0, "overlay": 2, "card": 2, "cover": 0},
    {"windowTitle": {"font": "VT323", "size": 20, "color": "#2B3627"},
     "scene": {
         "layers": [
             {"type": "line", "in": "window", "x": 0, "y": 0, "x2": "100%", "y2": 0, "stroke": "#1443523D", "repeat": {"n": 400, "dy": 3}},
             px(SIGNAL, 4, 15, 2, {"K": "#2B3627"}, id="phone", clickable=True),
             px(BATTERY, -34, 18, 2, {"K": "#2B3627", "G": "#43523D"}),
         ],
         "eggs": [
             {"trigger": "type:snake", "run": "snake", "seconds": 20},
             {"trigger": "click:phone", "say": "1 missed call: your favourite station.", "seconds": 5},
             {"trigger": "konami", "run": "snake", "seconds": 20},
         ]}}))

THEMES.append(theme(
    "super-16", "Super 16",
    "A homage to the 16-bit era's grey-and-purple console: soft greys, purple accents, four coloured buttons and a star.",
    "light", "none",
    tokens("#C9C9D4", "#E6E4E4EC", "#F2F2F7", "#7C76B8", "#E8E8F0", "#7C76B8", "#334F43AE", "#A9A9BC", "#26243A",
           "#4A4766", "#7A7896", "#4F43AE", "#4F43AE", "#FFFFFF", "#ECECF3", pane="#B8B8C6"),
    {"body": "Segoe UI", "title": "Press Start 2P", "titleSize": 18},
    {"control": 10, "overlay": 12, "card": 14, "cover": 0.12},
    {"windowTitle": {"font": "Press Start 2P", "size": 10, "color": "#4F43AE"},
     "scene": {
         "layers": [
             {"type": "ellipse", "in": "titlebar", "x": -58, "y": 4, "w": 16, "h": 16, "fill": "#2E9E4F"},
             {"type": "ellipse", "in": "titlebar", "x": -76, "y": 16, "w": 16, "h": 16, "fill": "#3D5FD9"},
             {"type": "ellipse", "in": "titlebar", "x": -40, "y": 16, "w": 16, "h": 16, "fill": "#D93D3D"},
             {"type": "ellipse", "in": "titlebar", "x": -58, "y": 28, "w": 16, "h": 16, "fill": "#E8C43A"},
             px(STAR, 4, 14, 3, {"Y": "#E8C43A"}, id="star", clickable=True),
             text("30 LIVES", "40%", "40%", 20, "#4F43AE", region="content", font="Press Start 2P", id="lives", hidden=True),
         ],
         "eggs": [
             {"trigger": "konami", "show": "lives", "seconds": 6},
             {"trigger": "click:star", "say": "Invincible for exactly as long as this song lasts.", "seconds": 5},
             {"trigger": "type:blow", "say": "Blowing on the cartridge never really helped. It still feels right.", "seconds": 5},
         ]}}))

THEMES.append(theme(
    "sixteen-blast", "Sixteen Blast",
    "A homage to the black 16-bit console that did what the others didn't: glossy black, red and gold, speed lines.",
    "dark", "none",
    tokens("#0A0A0C", "#E6101014", "#EE16161C", "#55E8262B", "#E01A1A20", "#66E8262B", "#33E8262B", "#44E8262B", "#F2F2F2",
           "#BDBDC6", "#80808C", "#E8262B", "#FF5A5E", "#FFFFFF", "#141418", pane="#050506"),
    {"body": "Segoe UI", "title": "Orbitron", "titleSize": 28, "titleWeight": "Black", "titleUppercase": True, "titleSpacing": 60},
    {"control": 2, "overlay": 4, "card": 4, "cover": 0.06},
    {"titleShadow": {"color": "#E8262B", "blur": 0, "offsetX": 2, "offsetY": 2},
     "windowTitle": {"font": "Orbitron", "size": 12, "weight": "bold", "color": "#FFD24A", "uppercase": True},
     "scene": {
         "layers": [
             {"type": "rect", "in": "titlebar-full", "x": 0, "y": 0, "w": "100%", "h": 48, "fill": ["#1E1E24", "#050506"]},
             text("16-BIT", 4, 12, 18, "#E8262B", font="Orbitron", weight="black", id="badge", clickable=True),
             {"type": "rect", "in": "window", "x": "-30%", "y": 0, "w": 160, "h": 2, "fill": ["#00FFFFFF", "#55FFFFFF"], "angle": "h",
              "scatter": {"n": 22, "seed": 3, "w": "100%", "h": "100%"}, "motion": {"kind": "scroll", "x": 1600, "seconds": 3.5, "randomOffset": 3.5}},
             text("BLAST PROCESSING ENGAGED", "26%", "42%", 22, "#FFD24A", region="content", font="Orbitron", weight="black", id="blast", hidden=True),
         ],
         "eggs": [
             {"trigger": "type:blast", "show": "blast", "seconds": 5},
             {"trigger": "click:badge", "show": "blast", "seconds": 5},
             {"trigger": "konami", "say": "Level select unlocked: every station is level 1.", "seconds": 6},
         ]}}))

THEMES.append(theme(
    "llama-whip", "Llama Whip",
    "A homage to the classic MP3 player of 1997: brushed gunmetal, a gold title strip, an LCD readout and spectrum bars.",
    "dark", "none",
    tokens("#1C1C24", "#CC23232B", "#E62A2A33", "#55D4AF37", "#E6303038", "#66D4AF37", "#3300E000", "#44D4AF37", "#D8D8E0",
           "#A8A8B4", "#76768A", "#00E000", "#D4AF37", "#000000", "#24242C", pane="#E615151B"),
    {"body": "Segoe UI", "title": "VT323", "titleSize": 34, "titleUppercase": True, "lcd": "VT323"},
    {"control": 0, "overlay": 2, "card": 2, "cover": 0},
    {"lcd": {"color": "#00E000", "background": "#050805"},
     "analyzer": {},
     "windowTitle": {"font": "Segoe UI", "size": 12, "weight": "bold", "color": "#D4AF37", "uppercase": True},
     "scene": {
         "layers": [
             {"type": "image", "src": "llama-metal.jpg", "in": "window", "x": 0, "y": 0},
             {"type": "image", "src": "llama-gold.jpg", "in": "titlebar", "x": 0, "y": 19, "h": 10, "fit": "fill", "id": "strip", "clickable": True},
         ],
         "eggs": [
             {"trigger": "click:strip", "say": "It really whips the llama's... well. You know the rest.", "seconds": 6},
             {"trigger": "type:llama", "say": "It really whips the llama's... well. You know the rest.", "seconds": 6},
         ]}}))


def main():
    os.makedirs(OUT, exist_ok=True)
    for t in THEMES:
        with open(os.path.join(OUT, t["id"] + ".json"), "w", encoding="utf-8", newline="\n") as f:
            json.dump(t, f, ensure_ascii=False, indent=2)
            f.write("\n")
    print(f"{len(THEMES)} themes written to {OUT}")


if __name__ == "__main__":
    main()
