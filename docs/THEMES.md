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
  "description": "A midnight drive: a molten orange sun sinking into an electric-blue grid.",
  "base": "dark",
  "backdrop": "none",
  "colors": { "background": "#070B1F", "surface": "#B30C1433", "text": "#EEF2FF", "accent": "#FF6A2B", "...": "..." },
  "fonts": { "title": "Orbitron", "titleSize": 30, "titleWeight": "Bold", "titleUppercase": true, "titleSpacing": 80 },
  "corners": { "control": 2, "overlay": 4, "card": 6, "cover": 0.08 },
  "effects": { "synthwave": { "sunTop": "#FFE066" }, "titleShadow": { "color": "#2F7BFF", "blur": 16 } }
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

**No flashing, ever.** Nothing in a theme may flash, flicker, strobe or blink: no sudden or repeated
changes in brightness. Josh's rule (2026-11-18) protects people with photosensitive conditions.
Effects may only move steadily or change slowly: rain, the rolling grid, drifting colour, the slow
scanline band, the dial pointer, and the meters and bars.
- A `flicker` effect, a flickering neon title and a blinking cursor were removed.
- Grain is a still texture.
- Old theme files that still name `flicker` load fine; it is ignored.


Each effect is a key under `effects`, with an options object (`{}` for defaults). Effects follow the
`effects` switch and the `effectIntensity` setting. They pause while the window is hidden or minimised,
or while Windows' animation effects are off. They also freeze into one fixed frame in `-screenshot` runs.

| Effect | What it does | Options |
|---|---|---|
| `synthwave` | Striped setting sun and a perspective grid rolling towards you, behind the UI | `sunTop`, `sunBottom`, `grid`, `sky` |
| `rain` | Rain streaks and a hazy city glow, behind the UI (never across text or controls) | `color`, `drops`, `glowA`, `glowB` |
| `aurora` | Large soft colour blobs drifting behind the panes | `colors` (array), `opacity` |
| `wood` | Walnut veneer with grain, behind the UI (uses `background`) | |
| `concrete` | Board-marked concrete texture behind the UI | `opacity` |
| `grain` | Film grain over everything | `opacity` |
| `vignette` | Darkened edges | `opacity`, `color` |
| `scanlines` | CRT scanlines with a slow rolling band | `opacity` |
| `frame` | A thick screen border around the window | `color`, `thickness` |
| `titleShadow` | Glow (`blur` > 0) or hard shadow (`blur` 0, with offsets) behind page titles | `color`, `blur`, `offsetX`, `offsetY`, `opacity` |
| `cursor` | A solid block cursor after page titles | `color` |
| `spotlight` | Darkness except a soft light that follows the pointer | `radius`, `opacity` |
| `lcd` | Player pane: the track as a scrolling LCD marquee, with bitrate and state | `color`, `background` |
| `analyzer` | Player pane: spectrum bars with falling peaks | |
| `dial` | Player pane: a backlit FM tuning dial; each station gets its own spot, and the pointer swings there | `color` |
| `vu` | Player pane: left and right VU meters | |
| `scene` | The theme's own decoration and easter eggs (see "Scenes") | `layers`, `eggs` |
| `tiles` | Placeholder covers in the theme's own colours (one per station, by name) | `colors` (array), `flat`, `ink` (the antenna; else chosen by brightness), `border` |
| `visTint` | Classic's visualizers in two colours, like a one-colour screen (dark to bright follows the picture's brightness) | `back` (the darkest), `ink` (the brightest) |
| `chrome` | Window chrome of an era: caption buttons, 90s bevels on buttons, fields, cards and dialogs, the selection colour, a centred title, a hard card shadow | `captions` (`fluent`, `win95`, `win31`, `xp`, `mac`, `platinum`), `bevel`, `bevelLight`, `bevelHi`, `bevelShadow`, `bevelDark`, `selection`, `selectionText`, `center`, `plate`, `hardShadow` |

**Cost.** Moving effects run on one clock at 24 frames a second, setting composition properties
directly (no layout), so the window redraws only that often. They rest whenever the window isn't the
active one, and when it's minimised or hidden.

### Scenes

A `scene` is a list of `layers` (drawn behind the UI unless `"front": true`) and optional `eggs`.

With `"surfaces": "translucent"` the panels keep their colours' alpha over the scene, and the picture under them is
frosted (painted once); `"frost": false` keeps it sharp (Luna, Aqua).

**Placing a layer:**

- `in`: the area its coordinates are relative to: `titlebar` (the 48 px strip), `pane` (the navigation
  pane), `content` (between the pane and the player), `player`, or the whole `window` (default).
- `x` and `y`: a number of pixels, a negative number (counted from the far edge) or a percentage
  (`"40%"`).
- `anchor: "bottom"` makes `y` the item's bottom edge.
- `w` and `h`: the same kinds of values.
- `opacity`, `rotate`.
- `id`, `hidden: true` (shown by an egg), `clickable: true` (an egg can react to clicks on it; the UI
  underneath still gets the click).

**Kinds (`type`):**

| type | What it draws | Options |
|---|---|---|
| `rect`, `ellipse` | A shape | `fill` (a colour, or an array of colours or `[colour, offset]` stops), `angle` (`v`, `h`, `d`), `stroke`, `strokeWidth`, `radius` |
| `radial` | A soft radial gradient | `fill` (stops) |
| `bevel` | A 3D edge, as on 90s desktops | `fill`, `light`, `dark`, `edge`, `style: "sunken"`, `outline` |
| `line` | A line | `x2`, `y2` (relative to the start), `stroke`, `strokeWidth` |
| `path` | Path geometry | `data`, `fill`, `stroke`, `scale` |
| `text` | Text | `text` (`{time}`, `{time24}` and `{date}` are filled in), `font`, `size`, `weight`, `style`, `color`, `spacing`, `vertical: true` (stacked, for signage), `glow` and `glowRadius` (a neon halo) |
| `pixels` | Pixel art | `rows` (strings), `palette` (one character to a colour; `.` is clear), `scale` |

**Many copies:**

- `repeat: {n, dx, dy}` makes copies in a row or column.
- `scatter: {n, seed, w, h, colors, fade}` makes random copies in an area. The pattern is the same
  every time.
- `jitter: {w, h}` varies their sizes, and `skip` drops a share of them.

**Motion (`motion: {kind, seconds, …}`)** is always slow and smooth:

- `drift` (there and back, by `x` and `y`);
- `scroll` (a loop by `x` and `y`);
- `breathe` (opacity between `min` and `max`, never faster than 4 s a cycle);
- `sway` (rotation by `angle`);
- `spin`;
- `pass` (crosses once by `x` and `y`, fading in and out over the first and last 18%, with a walking `bob` in px,
  then rests for `pause` seconds; `max` is its brightest opacity). Night Market's people in the street use it.

`start` shifts one item in time, `offset` staggers repeated items, and `randomOffset` scatters them.

`withPhoto: true` marks a layer that belongs to the photograph's world, such as the people's lights in the street.
It leaves with the photo when you set your own background.

**Rain on the window (`drips`):** drops cling to the glass, then run down in fits and starts, leaving wet trails
that dry slowly. Each drop is a little lens of the street.
- `density` sets the drops per 100,000 px² (2.4 by default).
- Clinging drops cost nothing: the window redraws only while drops run.

**Neon tubes (`neonEdges`):** a bright tube with a white-hot core round the page (`content`), the player card
(`player`) and the bottom bar (`bar`), with a soft glow (`glow`, its reach in px) spilling round it.
- It's plain XAML, so screenshots show it.
- It's steady: neon in Driftwave hums, it never flickers.

**Night Market's art** comes from `windows/tools/make-neon-glass.py`, made from the Kabukichō photograph:
- the street blurred behind dark tinted glass, with neon bloom and still water beads (each a tiny upside-down lens);
- a glow layer that breathes over 9 s;
- the sliding drop sprites;
- two out-of-focus rain tiles. These are drawn at 4× size and scaled down, so no streak is ever stair-stepped.

**Eggs (`eggs: [{trigger, …}]`):**

- **Triggers:**
  - `type:word`: the letters typed anywhere outside a text field;
  - `konami`: up, up, down, down, left, right, left, right, B, A;
  - `click:<id>`;
  - `idle:<seconds>`.
- **What happens:**
  - `show: <id>`: fades hidden layers in, then out;
  - `say: "text"`: a bubble near the click or `near`;
  - `run`: `snake`, `pong`, `cascade` (bouncing cards) or `walk:<id>` (a hidden layer walks across);
  - `seconds` sets how long it lasts.

**The spectrum bars and VU meters are decorative.** They move while a station plays, but they don't
follow the sound. The Windows app has no access to the decoded audio, because Media Foundation plays
it.

## The colour rule

No theme uses teal, purple or pink (Josh, 2026-11-20). `rust/core/src/hues.rs` says what counts (`banned`) and its
tests hold every built-in theme to it, translucent tokens included as they show over their backgrounds. Colours the
app makes itself keep to it too: derived palettes (`palette::ruled`), placeholder tiles (`hues::nearest_allowed`),
and the visualizers (their rainbows through `hues::spread` and `hues::safe`, and every frame through
`canvas::keep_the_rule` before the flash guard). Pictures are moved out of those hues by `tools/safe_hues.py`, which
the art scripts run last. The one exception is Driftwave's own sunset logo.

## Built-in themes

| id | Name | After | Base | Highlights |
|---|---|---|---|---|
| `retrowave` | Retrowave | 80s synthwave | dark | Synthwave sun and grid, neon title glow, scanlines, Orbitron |
| `neon-noir` | Neon Noir | Blade Runner | dark | Rain, amber and steel-blue city glow, blue neon title, grain, vignette, Share Tech Mono |
| `liminal` | Liminal Space | the Backrooms | light | Yellowed walls, grain, vignette, light titles |
| `brutalist` | Brutalist | brutalist web design | light | Concrete, black outlines, square corners, Space Mono, orange hard shadow |
| `retro-64` | Retro 64 | the Commodore 64 | dark | Blue screen, light blue border, block cursor, scanlines, Press Start 2P |
| `classic-skin` | Classic Skin | 90s Winamp skins | dark | Gunmetal, green LCD marquee, spectrum bars, VT323 |
| `frosted-glass` | Frosted Glass | Windows' acrylic | light | See-through acrylic window, translucent cards, drifting colour |
| `stealth` | Stealth | | dark | Black on black, dim grey type, a spotlight that follows the pointer |
| `analog-70s` | Analog 70s | 70s hi-fi receivers | dark | Walnut, cream and orange, backlit tuning dial, VU meters, Shrikhand |

### Your own background

**Settings > Appearance > Background** puts a GIF or picture behind the whole window, in any theme.

- **Choose File…** takes a GIF, PNG, JPEG, WebP or BMP.
- **Search GIPHY…** searches GIF results rated G.
  - Results show "Powered by GIPHY".
  - Searching needs a GIPHY API key: either the one built into release builds (`DRIFTWAVE_GIPHY_KEY` at build
    time, never committed), or your own, pasted under *GIPHY key*.
- **Dim** darkens the picture (0–80%, 35% by default).
- **How it shows:** the theme's surfaces become smoky glass, and a theme's own still pictures step aside, while
  moving layers such as rain stay.
- **When GIFs move:** they loop only while the window is shown and active, and when Windows' animation effects are
  on.
- **The flash check:** every GIF is checked before it's used. Each frame is composited at 64 px and measured for
  brightness and saturated red (`ShortwaveCore/FlashCheck.cs`, with unit tests). Anything that flashes more than
  three times a second is refused, following WCAG 2.3.1.
- **Where the files go:** pictures are copied into the data folder's `Backgrounds` folder.
- **Testing:** in test runs, `-background <file>` does the same import.

### Homage themes

These are tributes to computers, phones and consoles people loved. `tools/make-homage-themes.py` first wrote
them; the files in `themes/` have been edited by hand since, and are the source now (don't run it again). Each is original work in the spirit of its subject: colours, type, shapes and a detailed
scene with easter eggs. No logos, artwork, fonts or other assets were copied, and the names describe the
look rather than the brands.

| id | Name | A homage to | Some details (and where to look for eggs) |
|---|---|---|---|
| `program-manager` | Program Manager | Windows 3.1 | Navy title bar, program groups, hourglass. Type *solitaire*; click Games |
| `ninety-five` | Ninety-Five | Windows 95 | Blue desktop with icons, a taskbar with a clock. Click Start and Recycled Tracks; type *shutdown*; Konami |
| `luna` | Luna | Windows XP | Hills and clouds, a glossy title bar, green start button. Click start; wait; type *bliss* |
| `aurora-glass` | Aurora Glass | Windows Vista | Smoky glass, aurora, clock and now-on-air gadgets, a glass orb. Click the orb; type *wow* |
| `superbar` | Superbar | Windows 7 | A light swoosh, the superbar with pinned favourites. Click the orb; type *shake* |
| `fluent` | Fluent | Windows 11 | Mica, a blue bloom, a centred taskbar. Click start; Konami |
| `platinum` | Platinum | classic Mac OS | Pinstripes, a smiling computer, folders. Konami; click the disk; type *sosumi* |
| `aqua` | Aqua | Mac OS X | Pinstripes, gel traffic lights, a gel dock. Click red; type *lick*; Konami |
| `big-glass` | Big Glass | today's Mac | Full frosted glass, a menu bar, a floating dock. Type *hello*; click the trash; Konami |
| `home-screen` | Home Screen | the iPhone | Status bar with a pill, a gradient wallpaper, a dock. Click the battery and the pill; type *slide* |
| `dos-prompt` | C:\ Prompt | DOS | Text mode, box-drawn frames, a prompt. Type *dir*, *ver*, *help*, *format*; Konami |
| `night-market` | Night Market | a rainy Japanese street at night | Skyline, vertical neon signs, a noodle bar with lanterns and steam, wet reflections. Click the bowl; type *noodles*; wait |
| `woodgrain` | Woodgrain | the Atari 2600 | Rendered walnut veneer, a ribbed black plastic top, satin black panels, chrome-trimmed controls, rainbow stripes, a joystick; in Classic the frames are the walnut. Click the joystick; Konami; type *adventure* |
| `brick-phone` | Brick Phone | Nokia phones of 1999 | Green LCD, signal and battery, a phone. Type *snake*; click the phone; Konami |
| `super-16` | Super 16 | the Super Nintendo | Soft greys, four coloured buttons, a pad. Konami; type *blow* |
| `sixteen-blast` | Sixteen Blast | the Sega Genesis | Glossy black, red and gold, speed lines, PRESS START. Type *blast*; Konami |
| `llama-whip` | Llama Whip | Winamp | Black and gold, LCD marquee, spectrum bars, a llama. Click the llama; type *llama*; Konami |

The names describe the look rather than the brands. No artwork, logos or fonts from the originals
are used.

## Checked (Windows, 2026-11-18)

- **Renders:** every theme across Library, Settings, a dialog, the playing player, Discover and the
  mini player (`windows/scripts/screenshots.ps1 -Theme <id>`). The committed ones are in
  `docs/design/windows/themes/`.
  - The render hook can't draw acrylic, so open menus look see-through in renders, Default included.
- **On the real screen** (Josh's go-ahead, 2026-11-18): Retrowave, Neon Noir and Frosted Glass were
  captured. The rain falls, the synthwave scene draws, and Frosted Glass is see-through acrylic
  (`docs/design/windows/screen-frosted-glass.png`).
  - The captures caught a bug: the title glows (then a composition shadow masked by the text) showed
    as a blob under the title.
  - The glows are now plain XAML: a halo and copies of the text. So the renders show them exactly as
    the screen does. The same fix corrected the hard shadows and the Retro 64 cursor, which were
    24 px right and 12 px down.
- **Live runs:** each theme ran 20 s with a stream playing, with no effect errors. CPU while visible,
  in percent of one core:

  | Theme | CPU |
  |---|---|
  | Default | 2% |
  | Brutalist | 1% |
  | Stealth | 2% |
  | Liminal Space | 6% |
  | Classic Skin | 7% |
  | Analog 70s | 8% |
  | Retro 64 | ≤12% |
  | Retrowave | 15% |
  | Frosted Glass | 15% |
  | Neon Noir | 19% |

  - Minimised, it's about 5%, and the effects clock stops and restarts as it should.
  - Rain, aurora and the scanline band run as composition animations, and the rest
    is capped at 30 fps.
- **The UI:** picking a theme, the restart banner and Restart Now (resuming a station that was
  playing) were all checked through UI Automation. So were making a copy, renaming it (saved to the
  same file), deleting it, and a theme file with a hostile id (`..\library`), which loads under a
  safe id and can only touch its own file.
- **Not checked yet:**
  - the acrylic menus on the real screen;
  - the restart's wait for the old copy to exit, in a normal (non-test) run.

## Applying a theme (Windows)

- **Everything switches live:** colours, fonts, corners, effects and scenes, in both layouts, with no
  restart.
- **Customising:** built-in themes can't be edited. Settings > Theme > Customize Theme makes a copy
  (of Default too), which is saved under the user's themes as you go. In the copy you can change:
  - every colour
  - the fonts, title size, weight, case and spacing
  - corners and cover roundness
  - the light or dark base
  - which effects are on

  Themes can be exported, imported (as files) and deleted.
