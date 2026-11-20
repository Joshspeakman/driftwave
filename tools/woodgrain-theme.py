"""Woodgrain, redesigned (2026-11-20): the 1977 console itself. Walnut veneer is the window; the ribbed black plastic
of its top runs across the title bar with a bevelled lip; the panels are satin black plastic; controls are trimmed in
brushed chrome like its switches; and everything you press glows the console's orange. Pixel art stays in the title
bar's free stretch (the joystick, racing stripes), per the homage rules.

Writes themes/woodgrain.json (the art comes from make-woodgrain.py).
"""
import json
import os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'themes'))

theme = {
    "format": "shortwave-theme",
    "version": 1,
    "id": "woodgrain",
    "name": "Woodgrain",
    "description": "A homage to the woodgrain consoles of 1977: walnut veneer, ribbed black plastic, chrome switches, "
                   "chunky pixels and a joystick.",
    "base": "dark",
    "backdrop": "none",
    "colors": {
        "background": "#2A1A10",
        "pane": "#3A0E0905",
        "surface": "#F2131010",
        "card": "#1D1916",
        "cardStroke": "#2EF4E9D8",
        "control": "#2B2622",
        "controlStroke": "#A8A39A",
        "subtle": "#22F5A35C",
        "divider": "#2EF4E9D8",
        "text": "#F4E9D8",
        "textSecondary": "#CDB79A",
        "textTertiary": "#97836C",
        "accent": "#E8752A",
        "accentText": "#F5A35C",
        "onAccent": "#1A0F08",
        "dialog": "#1A1512",
    },
    "fonts": {"body": "Segoe UI", "title": "Press Start 2P", "titleSize": 20, "titleUppercase": True},
    "corners": {"control": 2, "overlay": 4, "card": 2, "cover": 0.02},
    "effects": {
        "windowTitle": {"font": "Press Start 2P", "size": 10, "color": "#F5A35C", "uppercase": True},
        # Classic: the frames are the walnut itself, the buttons chrome (see palette.rs).
        "classic": {"see": 0.88, "face": "#3A2516", "button": "#BDB8AF", "glyph": "#1B1714", "glyphOn": "#C4561A", "label": "#EADBC4",
                    "titleA": "#17130F", "titleB": "#0E0C0A", "titleText": "#F5A35C"},
        "scene": {
            "layers": [
                {"type": "image", "src": "woodgrain-walnut.jpg", "in": "window", "x": 0, "y": 0},
                {"type": "image", "src": "woodgrain-ribs.jpg", "in": "titlebar-full", "x": 0, "y": 0, "h": 48, "fit": "fill"},
                # The top's lip: a lit edge, then its shadow on the veneer.
                {"type": "rect", "in": "titlebar-full", "x": 0, "y": 46, "w": "100%", "h": 1, "fill": "#40FFFFFF"},
                {"type": "rect", "in": "titlebar-full", "x": 0, "y": 47, "w": "100%", "h": 1, "fill": "#000000"},
                {"type": "rect", "in": "pane", "x": 0, "y": 0, "w": "100%", "h": 14, "fill": ["#80000000", "#00000000"]},
                # Racing stripes: upright and spaced out, in one warm family (rust to gold, the console's own
                # oranges). Never stacked bars of different colours: that reads as a flag (Josh, 2026-11-20).
                {"type": "rect", "in": "titlebar", "x": -132, "y": 11, "w": 7, "h": 26, "fill": "#C4561A"},
                {"type": "rect", "in": "titlebar", "x": -120, "y": 11, "w": 7, "h": 26, "fill": "#E8752A"},
                {"type": "rect", "in": "titlebar", "x": -108, "y": 11, "w": 3, "h": 26, "fill": "#F2A65A"},
                {"type": "rect", "in": "titlebar", "x": -100, "y": 11, "w": 3, "h": 26, "fill": "#F5C77A"},
                {"type": "pixels",
                 "rows": [".....RR.....", "....RRRR....", "....RRRR....", ".....KK.....", ".....KK.....", "KKKKKKKKKKKK",
                          "KWWWWWWWWWRK", "KKKKKKKKKKKK"],
                 "palette": {"R": "#D8342C", "K": "#3A3A3A", "W": "#E0E0E0"},
                 "x": 4, "y": 12, "scale": 3, "in": "titlebar", "id": "stick", "clickable": True},
            ],
            "eggs": [
                {"trigger": "click:stick", "run": "pong", "seconds": 12},
                {"trigger": "konami", "run": "pong", "seconds": 12},
                {"trigger": "type:adventure", "say": "You found the secret room. The radio is in here.", "seconds": 5},
            ],
        },
    },
}

with open(os.path.join(ROOT, 'woodgrain.json'), 'w', encoding='utf-8', newline='\n') as f:
    json.dump(theme, f, ensure_ascii=False, indent=2)
    f.write('\n')
print('woodgrain written')
