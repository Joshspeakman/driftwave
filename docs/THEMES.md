# Shortwave themes

Josh's ask (2026-11-18): a customisable look, with built-in themes. Those are Retrowave, Blade Runner,
Liminal Space, Brutalist, Commodore 64, Winamp, Frosted Glass, Stealth (black on black) and Analog 70s
radio, some with creative effects.

The Windows app has them as of this file. The format below is plain JSON and isn't tied to Windows,
so the Mac app could read the same files. Whether the Mac follows is Josh's call.

## Where themes live

| | Windows |
|---|---|
| Built-in | `themes/*.json` in the repo, copied to `Themes\` next to the app |
| The user's own | `%APPDATA%\Shortwave\Themes\*.json` (test runs: `ShortwaveTestRun\Themes`) |
| Chosen theme | Setting `theme` (the theme's `id`, or `default`). `-theme <id>` overrides it for a run |
| Effects | Settings `effects` (true/false) and `effectIntensity` (0–1) |

`default` isn't a file. It is Windows' own look, light or dark following `appearance` (App Theme).
Every other theme brings its own light or dark base.

## The file

```json
{
  "format": "shortwave-theme",
  "version": 1,
  "id": "retrowave",
  "name": "Retrowave",
  "description": "Neon pink and purple over a synthwave sun and an endless grid.",
  "base": "dark",
  "backdrop": "none",
  "colors": { "background": "#12062B", "surface": "#B3180A33", "text": "#F3E8FF", "accent": "#FF2E97", "...": "..." },
  "fonts": { "title": "Orbitron", "titleSize": 30, "titleWeight": "Bold", "titleUppercase": true, "titleSpacing": 80 },
  "corners": { "control": 2, "overlay": 4, "card": 6, "cover": 0.08 },
  "effects": { "synthwave": { "sunTop": "#FFE45E" }, "titleShadow": { "color": "#FF2E97", "blur": 16 } }
}
```

- **`base`:** `dark` or `light`. It picks the system controls' variant that the colours then replace.
- **`backdrop`:** `none` (a solid `background`), `mica`, or `acrylic` (see-through; `background` may
  be translucent).
- **Colours:** `#RRGGBB` or `#AARRGGBB`. Any token left out keeps the system value.

| Token | Used for |
|---|---|
| `background` | Window background (and behind the navigation pane) |
| `surface` | The content area (pages, player pane). Translucent lets back effects show through |
| `card`, `cardStroke` | Cards (settings, details rows, the recording card) and their borders; flyout borders |
| `control`, `controlStroke` | Buttons, drop-downs, text boxes and their borders. Hover and pressed are derived |
| `subtle` | Hover on cards and nav items, the selected nav item, played-track rows |
| `divider` | Divider lines |
| `text`, `textSecondary`, `textTertiary` | Primary, secondary and faint text and icons (tertiary also draws toggle and radio outlines) |
| `accent`, `accentText`, `onAccent` | Accent fills (play button, toggles, slider, selection pill), accent text and links, and the glyph or text on accent |
| `dialog` | Dialogs, menus, drop-down lists and tooltips |

- **Fonts:** `body` (all text and controls; default Segoe UI Variable), `title` (page titles),
  `titleSize`, `titleWeight` (Light, Normal, SemiBold, Bold, Black), `titleUppercase`, `titleSpacing`
  (thousandths of an em), `lcd` (the LCD effect).
  - The app bundles Orbitron, Press Start 2P, VT323, Share Tech Mono, Space Mono and Shrikhand (SIL
    Open Font License; the licences ship beside them).
  - Any installed font can be named.
- **Corners:** `control` (buttons, fields), `overlay` (cards, dialogs, flyouts), `card` (the app's own
  cards and the station-card hover), and `cover` (cover radius as a fraction of its size; default
  0.18).

### Effects

Each effect is a key under `effects`, with an options object (`{}` for defaults). Effects follow the
`effects` switch and the `effectIntensity` setting. They pause while the window is hidden or minimised,
or while Windows' animation effects are off. They also freeze into one fixed frame in `-screenshot` runs.

| Effect | What it does | Options |
|---|---|---|
| `synthwave` | Striped setting sun and a perspective grid rolling towards you, behind the UI | `sunTop`, `sunBottom`, `grid`, `sky` |
| `rain` | Rain streaks over the UI and a hazy city glow behind it | `color`, `drops`, `glowA`, `glowB` |
| `aurora` | Large soft colour blobs drifting behind the panes | `colors` (array), `opacity` |
| `wood` | Walnut veneer with grain, behind the UI (uses `background`) | |
| `concrete` | Board-marked concrete texture behind the UI | `opacity` |
| `grain` | Film grain over everything | `opacity` |
| `vignette` | Darkened edges | `opacity`, `color` |
| `flicker` | Faint fluorescent hum with occasional stutters | `opacity` |
| `scanlines` | CRT scanlines with a slow rolling band | `opacity` |
| `frame` | A thick screen border around the window | `color`, `thickness` |
| `titleShadow` | Glow (`blur` > 0) or hard shadow (`blur` 0, with offsets) behind page titles | `color`, `blur`, `offsetX`, `offsetY`, `opacity`, `flicker` |
| `cursor` | A blinking block cursor after page titles | `color` |
| `spotlight` | Darkness except a soft light that follows the pointer | `radius`, `opacity` |
| `lcd` | Player pane: the track as a scrolling LCD marquee, with bitrate and state | `color`, `background` |
| `analyzer` | Player pane: spectrum bars with falling peaks | |
| `dial` | Player pane: a backlit FM tuning dial; each station gets its own spot, and the pointer swings there | `color` |
| `vu` | Player pane: left and right VU meters | |

**The spectrum bars and VU meters are decorative.** They move while a station plays, but they don't
follow the sound. The Windows app has no access to the decoded audio, because Media Foundation plays
it.

## Built-in themes

| id | Name | After | Base | Highlights |
|---|---|---|---|---|
| `retrowave` | Retrowave | 80s synthwave | dark | Synthwave sun and grid, neon title glow, scanlines, Orbitron |
| `neon-noir` | Neon Noir | Blade Runner | dark | Rain, amber and cyan city glow, flickering neon title, grain, vignette, Share Tech Mono |
| `liminal` | Liminal Space | the Backrooms | light | Yellowed walls, grain, vignette, fluorescent flicker, light titles |
| `brutalist` | Brutalist | brutalist web design | light | Concrete, black outlines, square corners, Space Mono, orange hard shadow |
| `retro-64` | Retro 64 | the Commodore 64 | dark | Blue screen, light blue border, blinking cursor, scanlines, Press Start 2P |
| `classic-skin` | Classic Skin | 90s Winamp skins | dark | Gunmetal, green LCD marquee, spectrum bars, VT323 |
| `frosted-glass` | Frosted Glass | Windows' acrylic | light | See-through acrylic window, translucent cards, drifting colour |
| `stealth` | Stealth | | dark | Black on black, dim grey type, a spotlight that follows the pointer |
| `analog-70s` | Analog 70s | 70s hi-fi receivers | dark | Walnut, cream and orange, backlit tuning dial, VU meters, Shrikhand |

The names describe the look rather than the brands. No artwork, logos or fonts from the originals
are used.

## Applying a theme (Windows)

- **Colours switch live.** Picking a theme recolours WinUI's shared theme brushes, so every control,
  dialog and popup follows.
- **Some things need a restart.** WinUI reads fonts, control corner radii and the primary text colour
  only when the app starts. Switching to a theme that differs in those shows "Restart to finish",
  with a Restart Now button that brings back the station that was playing.
- **High Contrast always uses Default.**
- **Customising:** built-in themes can't be edited. Settings > Appearance > Customize Theme makes a
  copy, which is saved under the user's themes. In the copy you can change:
  - every colour
  - the fonts, title size, weight, case and spacing
  - corners and cover roundness
  - the backdrop and the light or dark base
  - which effects are on

  Themes can be exported and imported as files.
