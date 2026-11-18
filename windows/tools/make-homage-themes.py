"""Writes the homage themes (themes/*.json): tributes to computers, phones and consoles people loved.

Each theme is original work in the spirit of its subject: its colours, type, shapes and a detailed "scene" (pixel
art, signage, skylines, status bars) with easter eggs. No logos, artwork or other assets are copied.
Usage: python windows/tools/make-homage-themes.py
"""
import json
import os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
OUT = os.path.join(ROOT, 'themes')

# MARK: pixel art (original drawings; "." is clear)

COMPUTER = ["..KKKKKKKKKK..", "..KWWWWWWWWK..", "..KWBBBBBBWK..", "..KWBCCCCBWK..", "..KWBCCCCBWK..",
            "..KWBBBBBBWK..", "..KWWWWWWWWK..", "..KKKKKKKKKK..", "....KGGGGK....", "KKKKKKKKKKKKKK",
            "KWWWWWWWWWWGGK", "KKKKKKKKKKKKKK"]
FOLDER = ["..KKKKK.......", ".KYYYYYK......", "KKKKKKKKKKKKKK", "KYYYYYYYYYYYYK", "KYWWWWWWWWWWYK",
          "KYYYYYYYYYYYYK", "KYYYYYYYYYYYYK", "KYYYYYYYYYYYYK", "KDDDDDDDDDDDDK", "KKKKKKKKKKKKKK"]
BIN = ["...KKKKKK...", "KKKKKKKKKKKK", "KWWWWWWWWWWK", ".KWGWGWGWGK.", ".KWGWGWGWGK.", ".KWGWGWGWGK.",
       ".KWGWGWGWGK.", ".KWGWGWGWGK.", ".KWWWWWWWWK.", "..KKKKKKKK.."]
FLOPPY = ["KKKKKKKKKKKK", "KBBWWWWWWBBK", "KBBWWWWWWBBK", "KBBWWWWWWBBK", "KBBBBBBBBBBK", "KBBBBBBBBBBK",
          "KBBGGGGGGBBK", "KBBGKKGGGBBK", "KBBGKKGGGBBK", "KKKKKKKKKKKK"]
HOURGLASS = ["KKKKKKKKK", ".KWWWWWK.", "..KYYYK..", "...KYK...", "....K....", "...KWK...", "..KWYWK..",
             ".KYYYYYK.", "KKKKKKKKK"]
NOTE = ["....KKKK", "....KYYK", "....K..K", "....K..K", "....K..K", "KKKKK..K", "KYYYK..K", "KYYYKKKK",
        "KKKKKYYK", "....KKKK"]
LLAMA = ["..KK..........", "..KWK.........", "..KWWK........", ".KWWWK........", "KWKWWK........",
         ".KWWWK........", "..KWWK........", "..KWWK........", "..KWWWKKKKKK..", "..KWWWWWWWWWK.",
         "..KWWWWWWWWWWK", "..KWWWWWWWWWK.", "..KWWKKKKWWK..", "..KWK....KWK..", "..KWK....KWK..",
         "..KKK....KKK.."]
CAT = ["K......K.....", "KK....KK.....", "KWKKKKWK.....", "KWWWWWWK...KK", "KWKWWKWK..KWK",
       "KWWWWWWKKKKWK", ".KWWWWWWWWWK.", ".KWWWWWWWWWK.", ".KWKKWKKKWK..", ".KK.KK..KK..."]
BOWL = ["....S..S..S...", "...S..S..S....", "....S..S..S...", "..............", "RRRRRRRRRRRRRR",
        "RWYYWYYWYYWYYR", ".RWWWWWWWWWWR.", "..RRRRRRRRRR..", "....RRRRRR...."]
LANTERN = [".KKKK.", "KRRRRK", "RRRRRR", "RRWWRR", "RRWWRR", "RRRRRR", "RRRRRR", "KRRRRK", ".KKKK.", "..YY.."]
PHONE = ["..KKKKKKKK..", ".KGGGGGGGGK.", ".KGLLLLLLGK.", ".KGLDDDDLGK.", ".KGLDDDDLGK.", ".KGLLLLLLGK.",
         ".KGGGGGGGGK.", ".KGKKGGKKGK.", ".KGGGGGGGGK.", ".KGKGKGKGGK.", ".KGKGKGKGGK.", ".KGKGKGKGGK.",
         ".KGGGGGGGGK.", "..KKKKKKKK.."]
JOYSTICK = [".....RR.....", "....RRRR....", "....RRRR....", ".....KK.....", ".....KK.....", ".....KK.....",
            "KKKKKKKKKKKK", "KWWWWWWWWWRK", "KWWWWWWWWWWK", "KKKKKKKKKKKK"]
CASSETTE = ["KKKKKKKKKKKKKKKK", "KWWWWWWWWWWWWWWK", "KWRRRRRRRRRRRRWK", "KWWWWWWWWWWWWWWK", "KWWKKKWWWWKKKWWK",
            "KWKWWWKWWKWWWKWK", "KWWKKKWWWWKKKWWK", "KWWWWWWWWWWWWWWK", "KWWWKKKKKKKKWWWK", "KKKKKKKKKKKKKKKK"]
SMILE_COMPUTER = ["KKKKKKKKKKKK", "KWWWWWWWWWWK", "KWKKKKKKKKWK", "KWKWWWWWWKWK", "KWKWKWWKWKWK",
                  "KWKWWWWWWKWK", "KWKWKWWKWKWK", "KWKWWKKWWKWK", "KWKWWWWWWKWK", "KWKKKKKKKKWK",
                  "KWWWWWWWWWWK", "KWWWWWKKKKWK", "KWWWWWWWWWWK", "KKKKKKKKKKKK"]
STAR = ["....Y....", "....Y....", "...YYY...", "YYYYYYYYY", ".YYYYYYY.", "..YYYYY..", ".YYY.YYY.", ".YY...YY.",
        "YY.....YY"]
HEART = [".RR.RR.", "RRRRRRR", "RRRRRRR", ".RRRRR.", "..RRR..", "...R..."]
BATTERY = ["KKKKKKKKKKKK.", "KGGGGGGGGGGKK", "KGGGGGGGGGGKK", "KGGGGGGGGGGKK", "KKKKKKKKKKKK."]
SIGNAL = ["......KK", "......KK", "....KKKK", "....KKKK", "..KKKKKK", "..KKKKKK", "KKKKKKKK", "KKKKKKKK"]

PAL = {"K": "#000000", "W": "#FFFFFF", "G": "#C0C0C0", "B": "#000080", "C": "#008080", "Y": "#FFD34E", "D": "#B08A2E",
       "R": "#D83A3A", "S": "#D8D8D8", "L": "#9BBC0F"}


def px(rows, x, y, scale=3, palette=None, **kw):
    pal = {k: v for k, v in PAL.items() if any(k in r for r in rows)}
    if palette:
        pal.update(palette)
    d = {"type": "pixels", "rows": rows, "palette": pal, "x": x, "y": y, "scale": scale}
    d.update(kw)
    return d


def text(t, x, y, size=13, color="#FFFFFF", **kw):
    d = {"type": "text", "text": t, "x": x, "y": y, "size": size, "color": color}
    d.update(kw)
    return d


def rect(x, y, w, h, fill=None, **kw):
    d = {"type": "rect", "x": x, "y": y, "w": w, "h": h}
    if fill is not None:
        d["fill"] = fill
    d.update(kw)
    return d


def icon(rows, label, x, y, color="#FFFFFF", scale=3, font="Microsoft Sans Serif", region="content", **kw):
    """A desktop icon with its label (two layers)."""
    return [px(rows, x, y, scale, **{"in": region}, **kw),
            text(label, x - 6, y + len(rows) * scale + 4, 11, color, font=font, **{"in": region})]


def theme(id, name, description, base, backdrop, colors, fonts, corners, effects):
    return {"format": "shortwave-theme", "version": 1, "id": id, "name": name, "description": description, "base": base,
            "backdrop": backdrop, "colors": colors, "fonts": fonts, "corners": corners, "effects": effects}


def tokens(background, surface, card, card_stroke, control, control_stroke, subtle, divider, text, text2, text3, accent,
           accent_text, on_accent, dialog, pane=None):
    c = {"background": background, "surface": surface, "card": card, "cardStroke": card_stroke, "control": control,
         "controlStroke": control_stroke, "subtle": subtle, "divider": divider, "text": text, "textSecondary": text2,
         "textTertiary": text3, "accent": accent, "accentText": accent_text, "onAccent": on_accent, "dialog": dialog}
    if pane:
        c["pane"] = pane
    return c


THEMES = []

# MARK: Windows

THEMES.append(theme(
    "program-manager", "Program Manager",
    "A homage to Windows 3.1: a white workspace, navy title bars, chunky bevels and program groups waiting below.",
    "light", "none",
    tokens("#C0C0C0", "#E6FFFFFF", "#FFFFFF", "#000000", "#C0C0C0", "#000000", "#330000A8", "#808080", "#000000",
           "#202020", "#606060", "#000080", "#000080", "#FFFFFF", "#FFFFFF", pane="#FFFFFF"),
    {"body": "Microsoft Sans Serif", "title": "Microsoft Sans Serif", "titleSize": 22, "titleWeight": "Bold"},
    {"control": 0, "overlay": 0, "card": 0, "cover": 0},
    {"frame": {"color": "#000080", "thickness": 4},
     "scene": {
         "layers": [
             rect(0, 0, "100%", 48, "#000080", **{"in": "titlebar"}),
             {"type": "bevel", "in": "titlebar", "x": 130, "y": 12, "w": 24, "h": 24, "fill": "#C0C0C0", "edge": 2},
             rect(136, 22, 12, 3, "#000000", **{"in": "titlebar"}),
             text("Program Manager", "50%", 13, 15, "#FFFFFF", font="Microsoft Sans Serif", weight="bold", **{"in": "titlebar"}),
             # Minimised program groups along the bottom of the workspace
             *icon(FOLDER, "Main", 30, -96), *icon(FOLDER, "Accessories", 110, -96), *icon(FOLDER, "Games", 210, -96, id="games", clickable=True),
             *icon(FOLDER, "Startup", 290, -96), *icon(HOURGLASS, "Busy...", -70, -96, scale=4),
             {"type": "bevel", "in": "content", "x": 20, "y": -30, "w": "60%", "h": 2, "style": "sunken", "fill": "#C0C0C0"},
         ],
         "eggs": [
             {"trigger": "type:solitaire", "run": "cascade", "seconds": 9},
             {"trigger": "click:games", "say": "Games group: Solitaire, Minesweeper... and one very patient radio.", "seconds": 5},
             {"trigger": "type:win", "say": "C:\\> win  -- loading Driftwave for Workgroups...", "seconds": 5},
         ]}}))

THEMES.append(theme(
    "ninety-five", "Ninety-Five",
    "A homage to Windows 95: a teal desktop, grey 3D windows, a navy-to-blue title bar and icons on the desktop.",
    "light", "none",
    tokens("#008080", "#00008080", "#C0C0C0", "#000000", "#C0C0C0", "#000000", "#330000A8", "#808080", "#000000",
           "#222222", "#555555", "#000080", "#000080", "#FFFFFF", "#C0C0C0", pane="#C0C0C0"),
    {"body": "Microsoft Sans Serif", "title": "Microsoft Sans Serif", "titleSize": 22, "titleWeight": "Bold"},
    {"control": 0, "overlay": 0, "card": 0, "cover": 0},
    {"scene": {
        "layers": [
            {"type": "rect", "in": "titlebar", "x": 0, "y": 0, "w": "100%", "h": 48, "fill": [["#000080", 0], ["#1084D0", 1]], "angle": "h"},
            text("Driftwave - Ninety-Five", 130, 14, 14, "#FFFFFF", font="Microsoft Sans Serif", weight="bold", **{"in": "titlebar"}),
            # Desktop icons down the right of the desktop (the content area is the desktop)
            *icon(COMPUTER, "My Radio", -110, 300), *icon(BIN, "Recycled Tracks", -112, 380, id="bin", clickable=True),
            *icon(FLOPPY, "3\u00bd Floppy (A:)", -114, 460),
            # A taskbar along the bottom of the desktop, with a raised start-style button and a clock tray
            {"type": "bevel", "in": "content", "x": 0, "y": -30, "w": "100%", "h": 30, "fill": "#C0C0C0"},
            {"type": "bevel", "in": "content", "x": 4, "y": -26, "w": 64, "h": 22, "fill": "#C0C0C0", "id": "start", "clickable": True},
            px(["RRGG", "RRGG", "BBYY", "BBYY"], 10, -21, 3, {"R": "#FF0000", "G": "#00A000", "B": "#0000FF", "Y": "#FFD800"}, **{"in": "content"}),
            text("Start", 26, -25, 13, "#000000", font="Microsoft Sans Serif", weight="bold", **{"in": "content"}),
            {"type": "bevel", "in": "content", "x": -84, "y": -26, "w": 80, "h": 22, "fill": "#C0C0C0", "style": "sunken"},
            text("{time}", -70, -24, 12, "#000000", font="Microsoft Sans Serif", **{"in": "content"}),
            px(NOTE, -100, -24, 2, **{"in": "content"}),
            text("It's now safe to turn off your computer.", "18%", "40%", 26, "#FF8C1A", font="Microsoft Sans Serif",
                 weight="bold", id="safe", hidden=True, **{"in": "content"}),
        ],
        "eggs": [
            {"trigger": "click:start", "say": "Start \u25B8 Programs \u25B8 Accessories \u25B8 Entertainment \u25B8 Driftwave. You're already here.", "seconds": 6},
            {"trigger": "click:bin", "say": "Recycled Tracks is empty. Every song deserves a second listen.", "seconds": 5},
            {"trigger": "type:shutdown", "show": "safe", "seconds": 6},
            {"trigger": "konami", "run": "walk:walker", "seconds": 8},
        ]}}))
THEMES[-1]["effects"]["scene"]["layers"].append(px(CAT, -60, -60, 3, **{"in": "content", "id": "walker", "hidden": True}))

THEMES.append(theme(
    "luna", "Luna",
    "A homage to Windows XP: rolling green hills under a big blue sky, a glossy blue title bar and a green go button.",
    "light", "none",
    tokens("#245EDC", "#33FFFFFF", "#EEF3FB", "#7A96DF", "#F4F7FD", "#7A96DF", "#333C7FE0", "#ACA899", "#000000",
           "#2A2A2A", "#6A6A6A", "#245EDC", "#0B3FB0", "#FFFFFF", "#ECE9D8", pane="#D6DFF7"),
    {"body": "Tahoma", "title": "Trebuchet MS", "titleSize": 28, "titleWeight": "Bold"},
    {"control": 3, "overlay": 6, "card": 8, "cover": 0.1},
    {"scene": {
        "layers": [
            {"type": "rect", "in": "titlebar", "x": 0, "y": 0, "w": "100%", "h": 48,
             "fill": [["#0A58E2", 0], ["#3A93FF", 0.12], ["#0053E1", 0.5], ["#0044C8", 1]]},
            text("Driftwave", 130, 12, 17, "#FFFFFF", font="Trebuchet MS", weight="bold", **{"in": "titlebar"}),
            # Sky and hills behind the content (original drawing: two soft hills and drifting clouds)
            {"type": "rect", "in": "content", "x": 0, "y": 0, "w": "100%", "h": "100%", "fill": [["#3F86E8", 0], ["#8EC3F7", 0.55], ["#BFE0FA", 0.62], ["#5DAA2E", 0.63], ["#2F7A12", 1]]},
            {"type": "radial", "in": "content", "x": "-20%", "y": "58%", "w": "90%", "h": "60%", "fill": [["#7CC24A", 0], ["#4E9A22", 0.7], ["#00000000", 1]]},
            {"type": "radial", "in": "content", "x": "40%", "y": "55%", "w": "80%", "h": "60%", "fill": [["#86CC52", 0], ["#3E8B1C", 0.75], ["#00000000", 1]]},
            {"type": "ellipse", "in": "content", "x": "10%", "y": "12%", "w": 140, "h": 40, "fill": "#E8F4FFFF", "opacity": 0.85,
             "motion": {"kind": "drift", "x": 60, "seconds": 40}},
            {"type": "ellipse", "in": "content", "x": "15%", "y": "9%", "w": 90, "h": 46, "fill": "#F2F9FFFF", "opacity": 0.85,
             "motion": {"kind": "drift", "x": 60, "seconds": 40}},
            {"type": "ellipse", "in": "content", "x": "55%", "y": "22%", "w": 120, "h": 34, "fill": "#EAF5FFFF", "opacity": 0.7,
             "motion": {"kind": "drift", "x": -50, "seconds": 52}},
            # Taskbar with a green go button and the tray
            {"type": "rect", "in": "content", "x": 0, "y": -30, "w": "100%", "h": 30, "fill": [["#3168D5", 0], ["#245EDC", 0.5], ["#1941A5", 1]]},
            {"type": "rect", "in": "content", "x": 0, "y": -30, "w": 96, "h": 30, "fill": [["#4FB33F", 0], ["#3C9A3C", 0.5], ["#2E7D2C", 1]], "radius": 0, "id": "go", "clickable": True},
            text("start", 22, -26, 17, "#FFFFFF", font="Trebuchet MS", weight="bold", style="italic", **{"in": "content"}),
            {"type": "rect", "in": "content", "x": -96, "y": -30, "w": 96, "h": 30, "fill": [["#0F8FEE", 0], ["#0A6CC7", 1]]},
            text("{time}", -80, -24, 12, "#FFFFFF", font="Tahoma", **{"in": "content"}),
            {"type": "rect", "in": "content", "x": -300, "y": -110, "w": 260, "h": 64, "fill": "#FFFFE1", "stroke": "#000000",
             "radius": 8, "id": "balloon", "hidden": True},
            text("New radio found\nClick here to keep listening.", -288, -100, 12, "#000000", font="Tahoma", id="balloon", hidden=True, **{"in": "content"}),
        ],
        "eggs": [
            {"trigger": "click:go", "say": "Start \u25B8 All Programs \u25B8 Driftwave. Good call.", "seconds": 5},
            {"trigger": "idle:90", "show": "balloon", "seconds": 7},
            {"trigger": "type:bliss", "say": "Somewhere a hill is still this green.", "seconds": 5},
        ]}}))

THEMES.append(theme(
    "aurora-glass", "Aurora Glass",
    "A homage to Windows Vista: smoky glass, an aurora of greens and blues, and a gadget sidebar with a real clock.",
    "dark", "acrylic",
    tokens("#33081620", "#33000000", "#55102030", "#5582C8FF", "#55203040", "#6682C8FF", "#3382C8FF", "#4482C8FF", "#FFFFFF",
           "#C8DCF0", "#8FA8C0", "#3FA0E8", "#7FC8FF", "#00121F", "#E6101C28"),
    {"body": "Segoe UI", "title": "Segoe UI Light", "titleSize": 32},
    {"control": 4, "overlay": 6, "card": 6, "cover": 0.08},
    {"aurora": {"colors": ["#2FBF71", "#2F7FD8", "#1BC6C6"]},
     "scene": {
         "layers": [
             # A gadget: an analog clock face showing the real time is too much motion; a calm digital one instead
             {"type": "rect", "in": "content", "x": -170, "y": 24, "w": 150, "h": 150, "fill": [["#AA1E3C5A", 0], ["#AA0B1626", 1]], "radius": 14, "stroke": "#5582C8FF"},
             text("{time}", -150, 70, 30, "#FFFFFF", font="Segoe UI Light", **{"in": "content"}),
             text("{date}", -148, 112, 13, "#B8D4F0", font="Segoe UI", **{"in": "content"}),
             {"type": "rect", "in": "content", "x": -170, "y": 190, "w": 150, "h": 90, "fill": [["#AA1E3C5A", 0], ["#AA0B1626", 1]], "radius": 14, "stroke": "#5582C8FF"},
             text("Now on air", -156, 202, 12, "#9CC8F0", font="Segoe UI", **{"in": "content"}),
             px(NOTE, -150, 228, 3, {"Y": "#7FC8FF"}, **{"in": "content"}),
             {"type": "rect", "in": "content", "x": -120, "y": 240, "w": 80, "h": 6, "fill": "#557FC8FF", "radius": 3},
             {"type": "rect", "in": "content", "x": -120, "y": 252, "w": 56, "h": 6, "fill": "#337FC8FF", "radius": 3},
             # A glass orb in the corner
             {"type": "radial", "in": "content", "x": 16, "y": -70, "w": 54, "h": 54, "fill": [["#AAFFFFFF", 0], ["#663FA0E8", 0.5], ["#22000000", 1]], "id": "orb", "clickable": True},
         ],
         "eggs": [
             {"trigger": "click:orb", "say": "The wow starts... whenever you press play.", "seconds": 5},
             {"trigger": "type:wow", "say": "Wow.", "seconds": 3},
         ]}}))

THEMES.append(theme(
    "superbar", "Superbar",
    "A homage to Windows 7: clear glass, a blue swoosh of light, and a superbar with pinned favourites.",
    "dark", "acrylic",
    tokens("#331C3F66", "#22000000", "#44123355", "#5580B8E8", "#44183A5A", "#6680B8E8", "#3380B8E8", "#4480B8E8", "#FFFFFF",
           "#CFE2F5", "#93AFCB", "#2E8BE6", "#8CC7FF", "#00142A", "#E6142A44"),
    {"body": "Segoe UI", "title": "Segoe UI", "titleSize": 28},
    {"control": 3, "overlay": 6, "card": 6, "cover": 0.1},
    {"scene": {
        "layers": [
            {"type": "rect", "in": "content", "x": 0, "y": 0, "w": "100%", "h": "100%", "fill": [["#0E3F75", 0], ["#1A6FB8", 0.55], ["#0B2D57", 1]]},
            {"type": "radial", "in": "content", "x": "-10%", "y": "35%", "w": "120%", "h": "40%", "fill": [["#AA8FD4FF", 0], ["#334AA3E8", 0.45], ["#00000000", 1]],
             "motion": {"kind": "drift", "x": 30, "y": -10, "seconds": 30}},
            {"type": "radial", "in": "content", "x": "30%", "y": "50%", "w": "90%", "h": "25%", "fill": [["#88B7F0FF", 0], ["#00000000", 1]],
             "motion": {"kind": "drift", "x": -24, "y": 8, "seconds": 36}},
            # The superbar
            {"type": "rect", "in": "content", "x": 0, "y": -40, "w": "100%", "h": 40, "fill": [["#AA4F7FB0", 0], ["#CC123050", 1]], "stroke": "#5580B8E8"},
            {"type": "radial", "in": "content", "x": 8, "y": -38, "w": 36, "h": 36, "fill": [["#FFFFFFFF", 0], ["#FF2E8BE6", 0.45], ["#FF0B3C7A", 1]], "id": "orb", "clickable": True},
            {"type": "rect", "in": "content", "x": 56, "y": -38, "w": 54, "h": 36, "fill": "#334F9BE0", "radius": 3, "stroke": "#6680B8E8"},
            px(NOTE, 74, -32, 3, {"Y": "#FFFFFF"}, **{"in": "content"}),
            {"type": "rect", "in": "content", "x": 114, "y": -38, "w": 54, "h": 36, "fill": "#22FFFFFF", "radius": 3},
            px(FOLDER, 122, -34, 3, {"Y": "#F3C55B", "D": "#C28F1F"}, **{"in": "content"}),
            text("{time}\n{date}", -96, -38, 11, "#FFFFFF", font="Segoe UI", **{"in": "content"}),
        ],
        "eggs": [
            {"trigger": "click:orb", "say": "Pinned to the superbar: Driftwave. Of course.", "seconds": 5},
            {"trigger": "type:shake", "say": "You'd shake a window to clear the others away. Here, everything stays.", "seconds": 5},
        ]}}))

THEMES.append(theme(
    "fluent", "Fluent",
    "A homage to Windows 11: soft Mica, a blue bloom of petals, rounded corners and a centred taskbar.",
    "dark", "mica",
    tokens("#2A0B1A33", "#22000000", "#33FFFFFF", "#22FFFFFF", "#22FFFFFF", "#33FFFFFF", "#1AFFFFFF", "#22FFFFFF", "#FFFFFF",
           "#D0D6E6", "#9AA3B8", "#4CC2FF", "#99DCFF", "#002033", "#EE1C2233"),
    {"body": "Segoe UI Variable Text", "title": "Segoe UI Variable Display", "titleSize": 30, "titleWeight": "SemiBold"},
    {"control": 4, "overlay": 8, "card": 8, "cover": 0.16},
    {"scene": {
        "layers": [
            # An original bloom: overlapping translucent petals
            {"type": "radial", "in": "content", "x": "30%", "y": "20%", "w": "55%", "h": "70%", "fill": [["#AA9AD8FF", 0], ["#553A7BE8", 0.5], ["#00000000", 1]],
             "motion": {"kind": "drift", "x": 12, "y": -8, "seconds": 26}},
            {"type": "radial", "in": "content", "x": "45%", "y": "30%", "w": "45%", "h": "60%", "fill": [["#889FC9FF", 0], ["#442D5BD0", 0.55], ["#00000000", 1]],
             "motion": {"kind": "drift", "x": -14, "y": 10, "seconds": 31}},
            {"type": "radial", "in": "content", "x": "20%", "y": "38%", "w": "50%", "h": "50%", "fill": [["#666CE0FF", 0], ["#22000000", 1]],
             "motion": {"kind": "drift", "x": 10, "y": 12, "seconds": 37}},
            # Centred taskbar
            {"type": "rect", "in": "content", "x": 0, "y": -46, "w": "100%", "h": 46, "fill": "#CC1C2233"},
            {"type": "rect", "in": "content", "x": "44%", "y": -40, "w": 34, "h": 34, "fill": "#22FFFFFF", "radius": 6, "id": "start", "clickable": True},
            px(["BBB.BBB", "BBB.BBB", "BBB.BBB", ".......", "BBB.BBB", "BBB.BBB", "BBB.BBB"], "45%", -33, 3, {"B": "#4CC2FF"}, **{"in": "content"}),
            {"type": "rect", "in": "content", "x": "50%", "y": -40, "w": 34, "h": 34, "fill": "#33FFFFFF", "radius": 6},
            px(NOTE, "51%", -36, 3, {"Y": "#4CC2FF"}, **{"in": "content"}),
            {"type": "rect", "in": "content", "x": "51%", "y": -5, "w": 14, "h": 3, "fill": "#4CC2FF", "radius": 2},
            text("{time}", -64, -34, 12, "#FFFFFF", font="Segoe UI Variable Text", **{"in": "content"}),
        ],
        "eggs": [
            {"trigger": "click:start", "say": "Recommended: keep listening.", "seconds": 5},
            {"trigger": "konami", "run": "walk:walker", "seconds": 8},
        ]}}))
THEMES[-1]["effects"]["scene"]["layers"].append(px(CAT, -60, -90, 3, {"W": "#9AD8FF"}, **{"in": "content", "id": "walker", "hidden": True}))

# MARK: Mac

THEMES.append(theme(
    "platinum", "Platinum",
    "A homage to classic Mac OS: platinum grey, pinstriped title bars, a smiling computer and a desk full of folders.",
    "light", "none",
    tokens("#DDDDDD", "#F2EEEEEE", "#FFFFFF", "#000000", "#EEEEEE", "#555555", "#33666699", "#999999", "#000000",
           "#262626", "#666666", "#666699", "#333399", "#FFFFFF", "#EEEEEE", pane="#E4E4E4"),
    {"body": "Segoe UI", "title": "Segoe UI Semibold", "titleSize": 22},
    {"control": 3, "overlay": 2, "card": 2, "cover": 0.04},
    {"scene": {
        "layers": [
            # Pinstripes across the title bar, with a box for the title
            {"type": "line", "in": "titlebar", "x": 130, "y": 14, "x2": -330, "y2": 0, "stroke": "#9A9A9A", "repeat": {"n": 7, "dy": 3}},
            rect("44%", 10, 120, 26, "#DDDDDD", **{"in": "titlebar"}),
            text("Driftwave", "45%", 13, 14, "#000000", font="Segoe UI Semibold", **{"in": "titlebar"}),
            # Desktop items, top right like a classic desk
            *icon(SMILE_COMPUTER, "Macintosh HD", -110, 20, "#000000", font="Segoe UI", id="hd", clickable=True),
            *icon(FOLDER, "Radio Folder", -110, 100, "#000000", font="Segoe UI"),
            *icon(BIN, "Trash", -104, -80, "#000000", font="Segoe UI"),
            {"type": "rect", "in": "content", "x": "20%", "y": "30%", "w": 280, "h": 90, "fill": "#EEEEEE", "stroke": "#000000",
             "id": "welcome", "hidden": True},
            text("Welcome to Driftwave.", "23%", "36%", 20, "#000000", font="Segoe UI Semibold", id="welcome", hidden=True, **{"in": "content"}),
        ],
        "eggs": [
            {"trigger": "konami", "show": "welcome", "seconds": 6},
            {"trigger": "click:hd", "say": "About This Radio: built-in memory? Plenty. Largest unused block: all of it.", "seconds": 6},
            {"trigger": "type:sosumi", "say": "Sosumi. (Some sounds have stories.)", "seconds": 5},
        ]}}))

THEMES.append(theme(
    "aqua", "Aqua",
    "A homage to Mac OS X: lickable gel buttons, pinstripes, brushed edges and water-blue everything.",
    "light", "none",
    tokens("#E8EBEF", "#E6F5F7FA", "#FFFFFF", "#9FB3CC", "#F3F6FA", "#8EA4C0", "#33479BE8", "#C5CED9", "#111111",
           "#3A3F47", "#7A818B", "#1F7CE8", "#1366CC", "#FFFFFF", "#F4F6F9", pane="#DFE5EC"),
    {"body": "Lucida Sans Unicode", "title": "Lucida Sans Unicode", "titleSize": 24, "titleWeight": "Bold"},
    {"control": 10, "overlay": 8, "card": 10, "cover": 0.18},
    {"scene": {
        "layers": [
            {"type": "line", "in": "content", "x": 0, "y": 0, "x2": "100%", "y2": 0, "stroke": "#14000000", "strokeWidth": 1, "repeat": {"n": 260, "dy": 4}},
            {"type": "rect", "in": "titlebar", "x": 0, "y": 0, "w": "100%", "h": 48, "fill": [["#F7F7F7", 0], ["#D6D6D6", 1]]},
            {"type": "line", "in": "titlebar", "x": 0, "y": 6, "x2": "100%", "y2": 0, "stroke": "#18000000", "repeat": {"n": 10, "dy": 4}},
            # Gel traffic lights beside the title (decorative; the real buttons are Windows')
            {"type": "radial", "in": "titlebar", "x": 128, "y": 16, "w": 16, "h": 16, "fill": [["#FFC2BC", 0], ["#FF5F57", 0.5], ["#C1342C", 1]], "id": "red", "clickable": True},
            {"type": "radial", "in": "titlebar", "x": 150, "y": 16, "w": 16, "h": 16, "fill": [["#FFE7B3", 0], ["#FEBC2E", 0.5], ["#C08A12", 1]]},
            {"type": "radial", "in": "titlebar", "x": 172, "y": 16, "w": 16, "h": 16, "fill": [["#C9F7C2", 0], ["#28C840", 0.5], ["#178A2A", 1]]},
            # A gel dock along the bottom of the content
            {"type": "rect", "in": "content", "x": "18%", "y": -64, "w": "64%", "h": 52, "fill": [["#AAFFFFFF", 0], ["#66BFD4EA", 1]], "radius": 10, "stroke": "#88FFFFFF"},
            px(SMILE_COMPUTER, "22%", -58, 3, **{"in": "content"}),
            px(NOTE, "34%", -58, 4, {"Y": "#1F7CE8"}, **{"in": "content"}),
            px(FOLDER, "44%", -54, 3, {"Y": "#8CC4FF", "D": "#4F8FD8"}, **{"in": "content"}),
            px(CASSETTE, "56%", -54, 3, **{"in": "content"}),
            px(BIN, "70%", -58, 4, **{"in": "content"}),
            {"type": "radial", "in": "content", "x": "45%", "y": "34%", "w": 120, "h": 120, "fill": [["#FFE6F2FF", 0], ["#FF63A4F0", 0.35], ["#FF1F5FCC", 0.8], ["#00000000", 1]],
             "id": "gel", "hidden": True},
        ],
        "eggs": [
            {"trigger": "click:red", "say": "That one's just for looks. The real close button is over on the right.", "seconds": 5},
            {"trigger": "type:lick", "show": "gel", "say": "Lickable. (Please don't.)", "seconds": 5},
            {"trigger": "konami", "say": "Think the radio was different. It was.", "seconds": 5},
        ]}}))

THEMES.append(theme(
    "big-glass", "Big Glass",
    "A homage to today's Mac: full frosted glass over a soft wallpaper, a menu bar, a floating dock and generous corners.",
    "light", "acrylic",
    tokens("#10F4F6FB", "#26FFFFFF", "#55FFFFFF", "#44FFFFFF", "#44FFFFFF", "#55FFFFFF", "#22FFFFFF", "#33000000", "#1D1D1F",
           "#3C3C43", "#6E6E73", "#0A84FF", "#0066CC", "#FFFFFF", "#EEF7F7FA"),
    {"body": "Segoe UI Variable Text", "title": "Segoe UI Variable Display", "titleSize": 34, "titleWeight": "Bold"},
    {"control": 10, "overlay": 14, "card": 18, "cover": 0.22},
    {"aurora": {"colors": ["#FF8FB8", "#8F9BFF", "#7FE0D8"]},
     "scene": {
         "layers": [
             # A menu bar in the title strip
             text("\u25CF  Driftwave    File    Edit    View    Station    Window    Help", 128, 15, 13, "#1D1D1F",
                  font="Segoe UI Variable Text", weight="semibold", **{"in": "titlebar"}),
             text("{date}  {time}", -330, 15, 13, "#1D1D1F", font="Segoe UI Variable Text", **{"in": "titlebar"}),
             # The floating dock
             {"type": "rect", "in": "content", "x": "22%", "y": -72, "w": "56%", "h": 60, "fill": "#55FFFFFF", "radius": 20, "stroke": "#88FFFFFF"},
             {"type": "rect", "in": "content", "x": "25%", "y": -64, "w": 44, "h": 44, "fill": [["#FF6CB6", 0], ["#A35CFF", 1]], "radius": 11},
             px(NOTE, "26.5%", -58, 3, {"Y": "#FFFFFF", "K": "#FFFFFF"}, **{"in": "content"}),
             {"type": "rect", "in": "content", "x": "36%", "y": -64, "w": 44, "h": 44, "fill": [["#5AC8FA", 0], ["#007AFF", 1]], "radius": 11},
             {"type": "rect", "in": "content", "x": "47%", "y": -64, "w": 44, "h": 44, "fill": [["#FFD60A", 0], ["#FF9F0A", 1]], "radius": 11},
             {"type": "rect", "in": "content", "x": "58%", "y": -64, "w": 44, "h": 44, "fill": [["#30D158", 0], ["#0E9F3A", 1]], "radius": 11},
             {"type": "rect", "in": "content", "x": "69%", "y": -64, "w": 44, "h": 44, "fill": [["#E5E5EA", 0], ["#AEAEB2", 1]], "radius": 11, "id": "trash", "clickable": True},
             px(BIN, "70.3%", -59, 3, **{"in": "content"}),
             {"type": "ellipse", "in": "content", "x": "48.7%", "y": -8, "w": 4, "h": 4, "fill": "#1D1D1F"},
             text("hello", "34%", "28%", 96, "#E6FFFFFF", font="Segoe Script", id="hello", hidden=True, **{"in": "content"}),
         ],
         "eggs": [
             {"trigger": "type:hello", "show": "hello", "seconds": 6},
             {"trigger": "click:trash", "say": "Nothing to empty. Driftwave never throws a song away unless you ask.", "seconds": 5},
             {"trigger": "konami", "say": "One more thing: it's still playing.", "seconds": 5},
         ]}}))

THEMES.append(theme(
    "home-screen", "Home Screen",
    "A homage to the phone in your pocket: a status bar, a soft gradient wallpaper, rounded glass and a dock.",
    "dark", "acrylic",
    tokens("#18101030", "#22000000", "#44FFFFFF", "#33FFFFFF", "#33FFFFFF", "#44FFFFFF", "#22FFFFFF", "#33FFFFFF", "#FFFFFF",
           "#E0E0EA", "#A8A8B8", "#0A84FF", "#64D2FF", "#FFFFFF", "#EE1C1C2A"),
    {"body": "Segoe UI Variable Text", "title": "Segoe UI Variable Display", "titleSize": 34, "titleWeight": "Bold"},
    {"control": 14, "overlay": 18, "card": 22, "cover": 0.26},
    {"scene": {
        "layers": [
            {"type": "rect", "in": "content", "x": 0, "y": 0, "w": "100%", "h": "100%", "fill": [["#3A1C71", 0], ["#D76D77", 0.6], ["#FFAF7B", 1]]},
            {"type": "radial", "in": "content", "x": "-10%", "y": "-10%", "w": "70%", "h": "70%", "fill": [["#66FFFFFF", 0], ["#00000000", 1]],
             "motion": {"kind": "drift", "x": 20, "y": 14, "seconds": 28}},
            # Status bar in the title strip
            text("{time24}", 136, 14, 15, "#FFFFFF", font="Segoe UI Variable Text", weight="semibold", **{"in": "titlebar"}),
            px(SIGNAL, -250, 16, 2, {"K": "#FFFFFF"}, **{"in": "titlebar"}),
            px(BATTERY, -226, 18, 2, {"K": "#FFFFFF", "G": "#34C759"}, **{"in": "titlebar", "id": "battery", "clickable": True}),
            {"type": "rect", "in": "titlebar", "x": "44%", "y": 8, "w": 120, "h": 30, "fill": "#000000", "radius": 15, "id": "island", "clickable": True},
            # The dock
            {"type": "rect", "in": "content", "x": "20%", "y": -84, "w": "60%", "h": 72, "fill": "#44FFFFFF", "radius": 28},
            *[{"type": "rect", "in": "content", "x": f"{24 + i * 14}%", "y": -76, "w": 56, "h": 56, "fill": [[a, 0], [b, 1]], "radius": 14}
              for i, (a, b) in enumerate([("#34C759", "#1E9C3F"), ("#0A84FF", "#0060DF"), ("#FF375F", "#C71E45"), ("#FFD60A", "#FF9F0A")])],
            px(NOTE, "62.5%", -68, 4, {"Y": "#FFFFFF", "K": "#FFFFFF"}, **{"in": "content"}),
            {"type": "rect", "in": "content", "x": "42%", "y": -6, "w": "16%", "h": 5, "fill": "#CCFFFFFF", "radius": 3},
        ],
        "eggs": [
            {"trigger": "click:battery", "say": "Low Power Mode isn't needed: Driftwave barely sips.", "seconds": 5},
            {"trigger": "click:island", "say": "Now playing lives here too.", "seconds": 4},
            {"trigger": "type:slide", "say": "slide to unlock \u25B8\u25B8", "seconds": 5},
        ]}}))

# MARK: DOS and the street

THEMES.append(theme(
    "dos-prompt", "C:\\ Prompt",
    "A homage to DOS: grey text on black, box-drawn frames, a blue setup screen and a prompt that answers back.",
    "dark", "none",
    tokens("#000000", "#000000", "#0000AA", "#AAAAAA", "#0000AA", "#AAAAAA", "#335555FF", "#555555", "#AAAAAA",
           "#AAAAAA", "#555555", "#55FFFF", "#55FFFF", "#000000", "#0000AA", pane="#000000"),
    {"body": "Consolas", "title": "VT323", "titleSize": 34, "titleUppercase": True},
    {"control": 0, "overlay": 0, "card": 0, "cover": 0},
    {"cursor": {"color": "#AAAAAA"},
     "scanlines": {"opacity": 0.08},
     "scene": {
         "layers": [
             text("C:\\DRIFTWAVE>", 130, 12, 22, "#AAAAAA", font="VT323", **{"in": "titlebar"}),
             text("\u2554" + "\u2550" * 58 + "\u2557", 8, -118, 16, "#55FFFF", font="Consolas", **{"in": "content"}),
             text("\u2551  DRIFTWAVE.EXE    640K should be enough for anybody's  \u2551", 8, -100, 16, "#55FFFF", font="Consolas", **{"in": "content"}),
             text("\u2551  radio. Type DIR, VER or HELP anywhere outside a box.  \u2551", 8, -82, 16, "#55FFFF", font="Consolas", **{"in": "content"}),
             text("\u255A" + "\u2550" * 58 + "\u255D", 8, -64, 16, "#55FFFF", font="Consolas", **{"in": "content"}),
             text("C:\\>_", 8, -36, 18, "#AAAAAA", font="VT323", **{"in": "content"}),
             px(HOURGLASS, -40, -48, 3, {"W": "#AAAAAA", "Y": "#55FFFF"}, **{"in": "content"}),
         ],
         "eggs": [
             {"trigger": "type:dir", "say": " Volume in drive C is DRIFTWAVE\n LIBRARY  <DIR>\n COVERS   <DIR>\n SONGS    <DIR>\n        3 dir(s)  plenty free", "seconds": 7},
             {"trigger": "type:ver", "say": "Driftwave Version 1.0 Beta 1", "seconds": 4},
             {"trigger": "type:help", "say": "DIR  VER  HELP  CLS  and one secret involving arrows.", "seconds": 5},
             {"trigger": "type:format", "say": "Nice try.", "seconds": 3},
             {"trigger": "konami", "run": "pong", "seconds": 12},
         ]}}))

THEMES.append(theme(
    "night-market", "Night Market",
    "Rain over a neon-soaked Japanese street at night: noodle-bar lanterns, vertical signs, steam and wet reflections.",
    "dark", "none",
    tokens("#05070C", "#8C0A0E16", "#AA101826", "#3337E0FF", "#CC111A28", "#4437E0FF", "#2237E0FF", "#2237E0FF", "#E8F0FF",
           "#A8B8D0", "#6A7A92", "#FF4F8B", "#37E0FF", "#05070C", "#0B111C", pane="#070A12"),
    {"body": "Yu Gothic UI", "title": "Yu Gothic UI", "titleSize": 30, "titleWeight": "Bold", "titleSpacing": 60},
    {"control": 2, "overlay": 4, "card": 4, "cover": 0.06},
    {"rain": {},
     "grain": {"opacity": 0.03},
     "vignette": {"opacity": 0.45},
     "scene": {
         "layers": [
             text("\u591c\u5e02  NIGHT MARKET", 132, 12, 16, "#FF4F8B", font="Yu Gothic UI", weight="bold", glow="#FF4F8B", **{"in": "titlebar"}),
             # Skyline: dark towers with scattered lit windows
             {"type": "rect", "in": "content", "x": 0, "y": "80%", "anchor": "bottom", "w": 60, "h": "62%", "fill": "#0A0F1A", "repeat": {"n": 16, "dx": 62},
              "jitter": {"h": 0.7, "w": 0.3}},
             {"type": "rect", "in": "content", "x": 30, "y": "80%", "anchor": "bottom", "w": 44, "h": "40%", "fill": "#111A2A", "repeat": {"n": 16, "dx": 62},
              "jitter": {"h": 0.8}},
             {"type": "rect", "in": "content", "x": 6, "y": "12%", "w": 3, "h": 4, "scatter": {"n": 260, "seed": 11, "w": "100%", "h": "60%",
              "colors": ["#FFD27A", "#7AE3FF", "#FF8FB0", "#FFF2C2"], "fade": True}, "skip": 0.35},
             # Vertical neon signs
             text("\u30e9\u30fc\u30e1\u30f3", "6%", "14%", 28, "#FF4F8B", font="Yu Gothic UI", weight="bold", vertical=True, glow="#FF4F8B", glowRadius=3,
                  motion={"kind": "breathe", "min": 0.75, "max": 1, "seconds": 7}),
             text("\u5c45\u9152\u5c4b", "18%", "8%", 26, "#37E0FF", font="Yu Gothic UI", weight="bold", vertical=True, glow="#37E0FF", glowRadius=3,
                  motion={"kind": "breathe", "min": 0.8, "max": 1, "seconds": 9}),
             text("\u96fb\u8133", "58%", "10%", 30, "#FFD23F", font="Yu Gothic UI", weight="bold", vertical=True, glow="#FFB000", glowRadius=3),
             text("\u30db\u30c6\u30eb", "72%", "16%", 22, "#B57BFF", font="Yu Gothic UI", weight="bold", vertical=True, glow="#B57BFF", glowRadius=2,
                  motion={"kind": "breathe", "min": 0.7, "max": 1, "seconds": 11}),
             text("\u85ac", "84%", "26%", 34, "#5CFF9D", font="Yu Gothic UI", weight="bold", glow="#5CFF9D", glowRadius=3),
             text("24\u6642\u9593", "40%", "20%", 18, "#37E0FF", font="Yu Gothic UI", weight="bold", glow="#37E0FF"),
             # A noodle-bar counter under a canopy with red lanterns
             {"type": "rect", "in": "content", "x": "28%", "y": "66%", "w": "40%", "h": 14, "fill": [["#7A1F2B", 0], ["#3A0D14", 1]]},
             {"type": "rect", "in": "content", "x": "28%", "y": "70%", "w": "40%", "h": 4, "fill": "#FF4F8B", "opacity": 0.5},
             *[px(LANTERN, f"{30 + i * 9}%", "58%", 3, {"R": "#E8402E", "W": "#FFB37A", "Y": "#FFD27A"},
                  **{"in": "content", "motion": {"kind": "sway", "angle": 4, "seconds": 5 + i, "offset": i}}) for i in range(4)],
             text("\u3089\u30fc\u3081\u3093", "40%", "62%", 18, "#FFE3B0", font="Yu Gothic UI", weight="bold", glow="#FF7A3A"),
             {"type": "rect", "in": "content", "x": "28%", "y": "72%", "w": "40%", "h": 34, "fill": "#140A0E"},
             px(BOWL, "38%", "73%", 3, {"S": "#E8E8E8", "R": "#B23A2E", "W": "#F2E6C8", "Y": "#E8B24A"}, **{"in": "content", "id": "bowl", "clickable": True}),
             # Steam rising
             {"type": "ellipse", "in": "content", "x": "40%", "y": "64%", "w": 30, "h": 30, "fill": "#33FFFFFF",
              "repeat": {"n": 4, "dx": 18}, "motion": {"kind": "drift", "y": -60, "x": 8, "seconds": 6, "offset": 1.5}},
             # Wet street: reflections of the signs
             {"type": "rect", "in": "content", "x": 0, "y": "80%", "w": "100%", "h": "20%", "fill": [["#0B1424", 0], ["#05070C", 1]]},
             {"type": "rect", "in": "content", "x": "6%", "y": "82%", "w": 20, "h": 60, "fill": [["#88FF4F8B", 0], ["#00FF4F8B", 1]]},
             {"type": "rect", "in": "content", "x": "18%", "y": "82%", "w": 18, "h": 70, "fill": [["#7737E0FF", 0], ["#0037E0FF", 1]]},
             {"type": "rect", "in": "content", "x": "58%", "y": "82%", "w": 22, "h": 56, "fill": [["#77FFD23F", 0], ["#00FFD23F", 1]]},
             {"type": "rect", "in": "content", "x": "72%", "y": "82%", "w": 16, "h": 64, "fill": [["#77B57BFF", 0], ["#00B57BFF", 1]]},
             text("\u4e8c\u3064\u3067\u5341\u5206\u3067\u3059\u3088\uff01", "36%", "50%", 22, "#FFFFFF", font="Yu Gothic UI", weight="bold",
                  glow="#FF4F8B", id="two", hidden=True, **{"in": "content"}),
         ],
         "eggs": [
             {"trigger": "click:bowl", "show": "two", "say": "Four bowls? Two is enough.", "seconds": 6},
             {"trigger": "type:noodles", "show": "two", "seconds": 6},
             {"trigger": "idle:120", "say": "The rain never stops here. Neither does the radio.", "seconds": 6},
         ]}}))

# MARK: consoles, phones and players

THEMES.append(theme(
    "woodgrain", "Woodgrain",
    "A homage to the woodgrain consoles of 1977: walnut veneer, ribbed black plastic, chunky pixels and a joystick.",
    "dark", "none",
    tokens("#1A120B", "#E01C1410", "#EE141010", "#55E07A1F", "#E0201812", "#66E07A1F", "#33E07A1F", "#44E07A1F", "#F4E6CF",
           "#D4B48C", "#9C7E5C", "#E07A1F", "#F2A65A", "#1A120B", "#20160E", pane="#100B07"),
    {"body": "Segoe UI", "title": "Press Start 2P", "titleSize": 20, "titleUppercase": True},
    {"control": 0, "overlay": 2, "card": 0, "cover": 0},
    {"wood": {},
     "scene": {
         "layers": [
             # Ribbed black panel across the title strip with rainbow stripes
             {"type": "rect", "in": "titlebar", "x": 0, "y": 0, "w": "100%", "h": 48, "fill": "#0E0E0E"},
             {"type": "line", "in": "titlebar", "x": 0, "y": 4, "x2": "100%", "y2": 0, "stroke": "#262626", "repeat": {"n": 10, "dy": 4}},
             *[rect(-260 + i * 18, 0, 12, 48, c, **{"in": "titlebar"}) for i, c in enumerate(["#D8342C", "#E8782A", "#F2C230", "#4A9A3A"])],
             text("DRIFTWAVE", 132, 16, 12, "#F2A65A", font="Press Start 2P", **{"in": "titlebar"}),
             px(JOYSTICK, -80, -80, 4, {"R": "#D8342C"}, **{"in": "content", "id": "stick", "clickable": True}),
             px(CASSETTE, -170, -64, 3, {"R": "#E07A1F"}, **{"in": "content"}),
             text("GAME 1\n1 PLAYER", 16, -60, 10, "#F2A65A", font="Press Start 2P", **{"in": "content"}),
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
    {"scene": {
        "layers": [
            # LCD pixel grid over the content
            {"type": "line", "in": "content", "x": 0, "y": 0, "x2": "100%", "y2": 0, "stroke": "#1443523D", "repeat": {"n": 300, "dy": 3}},
            px(SIGNAL, 132, 14, 2, {"K": "#2B3627"}, **{"in": "titlebar"}),
            text("DRIFTWAVE", 160, 10, 24, "#2B3627", font="VT323", **{"in": "titlebar"}),
            px(BATTERY, -232, 18, 2, {"K": "#2B3627", "G": "#43523D"}, **{"in": "titlebar"}),
            px(PHONE, -90, -110, 5, {"G": "#43523D", "L": "#C7F0A8", "D": "#8FA87A", "K": "#2B3627"}, **{"in": "content", "id": "phone", "clickable": True}),
            text("Menu", 20, -34, 22, "#2B3627", font="VT323", **{"in": "content"}),
            text("Names", -170, -34, 22, "#2B3627", font="VT323", **{"in": "content"}),
            {"type": "rect", "in": "content", "x": 0, "y": 0, "w": 380, "h": 230, "id": "board", "fill": "#00000000"},
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
    {"scene": {
        "layers": [
            text("SUPER 16", 132, 16, 12, "#4F43AE", font="Press Start 2P", **{"in": "titlebar"}),
            # Four coloured buttons, as on a pad
            {"type": "ellipse", "in": "titlebar", "x": -232, "y": 4, "w": 16, "h": 16, "fill": "#2E9E4F"},
            {"type": "ellipse", "in": "titlebar", "x": -250, "y": 16, "w": 16, "h": 16, "fill": "#3D5FD9"},
            {"type": "ellipse", "in": "titlebar", "x": -214, "y": 16, "w": 16, "h": 16, "fill": "#D93D3D"},
            {"type": "ellipse", "in": "titlebar", "x": -232, "y": 28, "w": 16, "h": 16, "fill": "#E8C43A"},
            # A d-pad and a pad shape at the bottom of the content
            {"type": "rect", "in": "content", "x": 40, "y": -110, "w": 220, "h": 86, "fill": "#DADAE4", "radius": 43, "stroke": "#9A9AB0"},
            {"type": "rect", "in": "content", "x": 70, "y": -82, "w": 42, "h": 14, "fill": "#3A3A44", "radius": 2},
            {"type": "rect", "in": "content", "x": 84, "y": -96, "w": 14, "h": 42, "fill": "#3A3A44", "radius": 2},
            {"type": "ellipse", "in": "content", "x": 196, "y": -96, "w": 18, "h": 18, "fill": "#2E9E4F"},
            {"type": "ellipse", "in": "content", "x": 178, "y": -80, "w": 18, "h": 18, "fill": "#3D5FD9"},
            {"type": "ellipse", "in": "content", "x": 214, "y": -80, "w": 18, "h": 18, "fill": "#D93D3D"},
            {"type": "ellipse", "in": "content", "x": 196, "y": -64, "w": 18, "h": 18, "fill": "#E8C43A"},
            {"type": "rect", "in": "content", "x": 128, "y": -66, "w": 18, "h": 6, "fill": "#7A7896", "radius": 3, "rotate": -25},
            {"type": "rect", "in": "content", "x": 150, "y": -66, "w": 18, "h": 6, "fill": "#7A7896", "radius": 3, "rotate": -25},
            px(STAR, -60, -60, 4, **{"in": "content", "id": "walker", "hidden": True}),
            text("30 LIVES", "40%", "40%", 20, "#4F43AE", font="Press Start 2P", id="lives", hidden=True, **{"in": "content"}),
        ],
        "eggs": [
            {"trigger": "konami", "show": "lives", "run": "walk:walker", "seconds": 8},
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
     "scene": {
         "layers": [
             {"type": "rect", "in": "titlebar", "x": 0, "y": 0, "w": "100%", "h": 48, "fill": [["#1E1E24", 0], ["#050506", 1]]},
             text("16-BIT", 132, 10, 22, "#FFD24A", font="Orbitron", weight="black", **{"in": "titlebar"}),
             {"type": "line", "in": "titlebar", "x": 132, "y": 40, "x2": 120, "y2": 0, "stroke": "#E8262B", "strokeWidth": 3},
             # Speed lines streaking across the content
             {"type": "rect", "in": "content", "x": "-30%", "y": 0, "w": 160, "h": 2, "fill": [["#00FFFFFF", 0], ["#88FFFFFF", 1]], "angle": "h",
              "scatter": {"n": 26, "seed": 3, "w": "100%", "h": "100%"}, "motion": {"kind": "scroll", "x": 1600, "seconds": 3.5, "randomOffset": 3.5}},
             {"type": "ellipse", "in": "content", "x": -120, "y": -120, "w": 60, "h": 60, "stroke": "#FFD24A", "strokeWidth": 8,
              "repeat": {"n": 3, "dx": 30}, "motion": {"kind": "drift", "y": -8, "seconds": 3, "offset": 0.5}},
             text("PRESS START", "40%", -40, 18, "#FFFFFF", font="Orbitron", weight="bold", motion={"kind": "breathe", "min": 0.35, "max": 1, "seconds": 4}),
             text("BLAST PROCESSING ENGAGED", "26%", "42%", 22, "#FFD24A", font="Orbitron", weight="black", id="blast", hidden=True, **{"in": "content"}),
         ],
         "eggs": [
             {"trigger": "type:blast", "show": "blast", "seconds": 5},
             {"trigger": "konami", "show": "blast", "say": "Level select unlocked: every station is level 1.", "seconds": 6},
         ]}}))

THEMES.append(theme(
    "llama-whip", "Llama Whip",
    "A homage to the classic MP3 player of 1997: a black-and-gold skin, an LCD readout, spectrum bars and a llama.",
    "dark", "none",
    tokens("#1C1C24", "#E623232B", "#EE2A2A33", "#55D4AF37", "#E6303038", "#66D4AF37", "#3300E000", "#44D4AF37", "#D8D8E0",
           "#A8A8B4", "#76768A", "#00E000", "#D4AF37", "#000000", "#24242C", pane="#15151B"),
    {"body": "Segoe UI", "title": "VT323", "titleSize": 34, "titleUppercase": True, "lcd": "VT323"},
    {"control": 0, "overlay": 2, "card": 2, "cover": 0},
    {"lcd": {"color": "#00E000", "background": "#050805"},
     "analyzer": {},
     "scene": {
         "layers": [
             # A gold title stripe with ridges, as on a classic skin
             {"type": "rect", "in": "titlebar", "x": 128, "y": 18, "w": -380, "h": 12, "fill": [["#6E5A1E", 0], ["#D4AF37", 0.5], ["#6E5A1E", 1]]},
             {"type": "line", "in": "titlebar", "x": 128, "y": 20, "x2": -380, "y2": 0, "stroke": "#3A2E0C", "repeat": {"n": 4, "dy": 2}},
             rect("40%", 12, 150, 24, "#1C1C24", **{"in": "titlebar"}),
             text("DRIFTWAVE 2.0", "41%", 13, 20, "#D4AF37", font="VT323", **{"in": "titlebar"}),
             px(LLAMA, -90, -90, 4, **{"in": "content", "id": "llama", "clickable": True}),
             px(CASSETTE, 20, -60, 3, {"R": "#D4AF37"}, **{"in": "content"}),
             text("128 kbps   44 kHz   stereo", 80, -50, 16, "#00E000", font="VT323", **{"in": "content"}),
         ],
         "eggs": [
             {"trigger": "click:llama", "say": "It really whips the llama's... well. You know the rest.", "seconds": 6},
             {"trigger": "type:llama", "run": "walk:walker", "seconds": 8},
             {"trigger": "konami", "run": "walk:walker", "seconds": 8},
         ]}}))
THEMES[-1]["effects"]["scene"]["layers"].append(px(LLAMA, -60, -80, 3, **{"in": "content", "id": "walker", "hidden": True}))


def main():
    os.makedirs(OUT, exist_ok=True)
    for t in THEMES:
        path = os.path.join(OUT, t["id"] + ".json")
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            json.dump(t, f, ensure_ascii=False, indent=2)
            f.write("\n")
    print(f"{len(THEMES)} themes written to {OUT}")


if __name__ == "__main__":
    main()
