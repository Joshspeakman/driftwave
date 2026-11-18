# Shortwave design: the Windows app's values

What the renders in `docs/design/windows/` can't show: the exact values behind them. They come from
the Windows app's XAML (`windows/src/Shortwave/Views/`) and from WinUI's theme file
(`Microsoft.WindowsAppSDK.WinUI` 2.6.4, `Themes/generic.xaml`). Colours are `#AARRGGBB` when they
have alpha, `#RRGGBB` when they don't. All sizes are in effective pixels (epx), which equal points
on the Mac at 1x.

The renders were made on a PC whose Windows accent colour is **red**, so every accent in them is red.
WinUI always takes the accent from Windows; the default Windows accent is blue (below). The Mac
should use its own accent colour.

## Type

The font is Segoe UI Variable, the system font (`XamlAutoFontFamily`). Text uses the system's
natural line spacing; no line height is set.

| Style | Size | Weight | Used for |
|---|---|---|---|
| TitleTextBlockStyle | 28 | SemiBold (600) | Page titles (Library, Discover, Search, Settings) |
| SubtitleTextBlockStyle | 20 | SemiBold | Discover section titles, "No Playback", the player's station name, "Your Library Is Empty", dialog titles |
| BodyStrongTextBlockStyle | 14 | SemiBold | Card station names, the song title in the player, Settings group headers, "Played Tracks" |
| BodyTextBlockStyle | 14 | Normal (400) | Everything else; also the default control text size |
| CaptionTextBlockStyle | 12 | Normal | Card subtitles, Settings captions, the title bar text, recording explanations |

Secondary text (artist, subtitles, captions, empty-state explanations) is the same style in
TextFillColorSecondary. The page title is vertically centred in its header row.

## Colours

### Text

| Token | Light | Dark |
|---|---|---|
| TextFillColorPrimary | `#E4000000` | `#FFFFFF` |
| TextFillColorSecondary | `#9E000000` | `#C5FFFFFF` |
| TextFillColorTertiary (empty-state icons, chevrons) | `#72000000` | `#87FFFFFF` |
| TextFillColorDisabled | `#5C000000` | `#5DFFFFFF` |
| TextOnAccentFillColorPrimary (glyphs on accent buttons) | `#FFFFFF` | `#000000` |

In dark mode the play/stop glyph on the accent button is **black**, because the accent fill there is
a light shade.

### Accent

WinUI derives the shades from the Windows accent colour. The default blue's documented Fluent
values:

| Use | Token | Light | Dark |
|---|---|---|---|
| Filled accent (play button, toggle on, slider track, selection pill, default dialog button) | AccentFillColorDefault | SystemAccentColorDark1: `#005FB8` | SystemAccentColorLight2: `#60CDFF` |
| Accent hover | AccentFillColorSecondary | the same at 90% opacity | the same at 90% |
| Accent pressed | AccentFillColorTertiary | the same at 80% opacity | the same at 80% |
| Accent text (links, the playing icon on a card) | AccentTextFillColorPrimary | SystemAccentColorDark2: `#003E92` | SystemAccentColorLight3: `#99EBFF` |

The red on the render PC: Light3 `#FAA683`, Light2 `#F38064`, Light1 `#E73D2E`, accent `#E2241A`,
Dark1 `#C21C14`, Dark2 `#9A120D`.

### Backgrounds, fills and strokes

| Token | Light | Dark | Used for |
|---|---|---|---|
| SolidBackgroundFillColorBase | `#F3F3F3` | `#202020` | Window base (Mica's fallback), dialog background, the render background |
| LayerFillColorDefault | `#80FFFFFF` | `#4C3A3A3A` | The content area right of the navigation pane (over Mica) |
| LayerFillColorAlt | `#FFFFFF` | `#0DFFFFFF` | Dialog content area (above the button row); the mini player bar at the bottom when the player is hidden |
| CardBackgroundFillColorDefault | `#B3FFFFFF` | `#0DFFFFFF` | Settings cards, the details dialog's grouped rows, the recording card in the player |
| CardStrokeColorDefault | `#0F000000` | `#19000000` | Card borders (1 px), the content area's top and left edge, cover outlines (0.5 px) |
| DividerStrokeColorDefault | `#0F000000` | `#15FFFFFF` | Between the content and the player pane (1 px), rows inside a card |
| SubtleFillColorSecondary | `#09000000` | `#0FFFFFFF` | Hover on nav items and station cards, the selected nav item, played-track rows |
| SubtleFillColorTertiary | `#06000000` | `#0AFFFFFF` | Pressed nav item; selected nav item on hover |
| ControlFillColorDefault | `#B3FFFFFF` | `#0FFFFFFF` | Buttons, drop-downs, text boxes at rest |
| ControlFillColorSecondary | `#80F9F9F9` | `#15FFFFFF` | Button hover |
| ControlFillColorTertiary | `#4DF9F9F9` | `#08FFFFFF` | Button pressed |
| ControlStrokeColorDefault / Secondary | `#0F000000` / `#29000000` | `#12FFFFFF` / `#18FFFFFF` | Button border: Default all round, Secondary along the bottom 1 px (a slight "lip") |
| ControlStrongStrokeColorDefault | `#72000000` | `#8BFFFFFF` | Toggle and radio outlines when off |
| SmokeFillColorDefault | `#4D000000` | `#4D000000` | The dimming behind a dialog |
| SystemFillColorCritical | `#C42B1C` | `#FF99A4` | Recording dot, "Cancel Recording" |
| SystemFillColorCaution | `#9D5D00` | `#FCE100` | Playback errors |
| SystemFillColorSuccess | `#0F7B0F` | `#6CCB5F` | Saved-track check mark |
| AcrylicInAppFillColorDefault | about `#F9F9F9` | about `#2C2C2C` | The round play button that appears on a station card |

Station covers without an image use a diagonal gradient from the station name's hue: top left
HSB(h, 0.45, 0.85), bottom right HSB(h, 0.65, 0.55). The glyph on them is `#E6FFFFFF`. The hue is the
same as the Mac's.

## Shapes and spacing

| Thing | Value |
|---|---|
| Control corner radius (buttons, drop-downs, text boxes, nav items) | 4 |
| Overlay corner radius (cards, dialogs, flyouts, the content area's top-left corner) | 8 |
| Station cover | `0.18 × size` (148 → 26.6, 200 → 36, 44 → 7.9), 0.5 px CardStroke outline |
| Station card | 164 wide (148 cover plus 8 padding each side), corner radius 14 for the hover fill, 8 between cover and text, 2 between name and subtitle |
| Station grid | Uniform grid of 164 × 214 cells, at least 8 between columns and 12 between rows, left-aligned (extra width stays on the right) |
| Page header | Padding 24 left, 12 top, 16 right, 8 bottom. Header controls are on the right with 4 between them, 32 tall |
| Page content | Grid inset 16 left and right, 4 top. Discover sections are 28 apart, with the section title 8 above its grid |
| Navigation pane | 200 wide when open. Items 36 tall with a 4,2 margin (so 40 apart) and 4 radius. 16 px icon, 14 px label. The footer holds Add Local Station, Hide Player, Mini Player and Settings |
| Selection pill | 3 × 16, radius 2, accent fill, vertically centred at the item's left edge |
| Title bar | 40 tall: a 16 px app icon at 14 from the left, then 10 gap, then the app name in Caption (12) |
| Player pane | 300 wide, padding 16, 16 between blocks. Cover 200; name, song and artist 4 apart and centred |
| Play button | Accent circle, 48 in the player pane (20 px glyph, 16 px stop square with radius 2), 30 elsewhere. Volume slider 120 (player) / 90 (mini), 12 / 6 between controls |
| Recording card | A button with padding 10 and radius 10: 16 px state glyph, title in BodyStrong, explanation in Caption Secondary (at most 2 lines), chevron |
| Played-track rows | Padding 6,5, radius 8, SubtleFillColorSecondary, 4 apart |
| Settings | Column up to 760 wide, padding 24. Cards: padding 16,12, radius 8, 4 between cards. Group headers BodyStrong, 20 above (8 for the first) and 4 below. Drop-downs 200 wide, number boxes 120 |
| Dialogs | 24 padding, 320 to 548 wide (station details 640, track details 560). The button row is SolidBackgroundFillColorBase; the content above it is LayerFillColorAlt. Primary button first (left), with the accent style when it's the default |
| Mini player | 430 × 104 window, always on top. 10 padding, 10 between cover (44), text and controls |
| Default window | 1180 × 760, at least 760 × 480 |

## Hover and pressed states

| Element | Rest | Hover | Pressed | Selected |
|---|---|---|---|---|
| Nav item | transparent | SubtleFillColorSecondary | SubtleFillColorTertiary | SubtleFillColorSecondary + accent pill; on hover SubtleFillColorTertiary |
| Station card | transparent | SubtleFillColorSecondary over the whole card (radius 14), and the round play button (36, AcrylicInAppFill, CardStroke border, 8 from the cover's bottom-right corner) appears | No separate state. A single click opens the details, a double click plays | The playing station keeps its stop button visible and shows a small speaker (12 px, accent text) before its name |
| Button | ControlFillColorDefault | ControlFillColorSecondary | ControlFillColorTertiary, text TextFillColorSecondary | |
| Accent button | AccentFillColorDefault | 90% | 80%, text TextOnAccentFillColorSecondary | |
| Toggle | Off: ControlStrongStroke outline with a dark knob | Slightly larger knob | Knob stretches | On: accent fill, knob TextOnAccent |
| Played-track row | SubtleFillColorSecondary | (the row is a button: subtle hover) | | |

Hover changes have no animation beyond WinUI's default (about 83 ms colour fades). Pressed buttons
don't scale.

## Mica and the title bar on the real screen

The renders leave both out: they draw the XAML over a flat `#F3F3F3` or `#202020`. On a real
screen:

- **Mica** fills the whole window behind the navigation pane and the title bar. It is the window
  base colour (`#F3F3F3` light, `#202020` dark), very lightly tinted by the desktop wallpaper
  (blurred so heavily that only its average hue shows). An inactive window drops to the plain base
  colour. So the navigation pane and title bar look like the renders, with at most a faint hint of
  wallpaper colour.
- **The content area** (pages and the player pane) sits on LayerFillColorDefault over Mica. It is
  lighter than the pane in light mode (`#F9F9F9`) and lighter in dark mode too (about `#272727`).
  Its top-left corner is rounded 8, with a 1 px CardStroke line along its top and left edges. The
  renders show this correctly.
- **The title bar** is the app's own 40 px row, transparent so Mica shows through. On the right,
  Windows draws the minimise, maximise and close buttons, each 46 wide and the full 40 high, with
  no backgrounds. Hover fills them with SubtleFillColorSecondary; the close button turns `#C42B1C`
  with a white glyph. The glyphs follow App Theme (`AppWindow.TitleBar.PreferredTheme`). There is
  no line between the title bar and the content.
- **The mini player** has the same Mica and title bar buttons. Its own title row is the text
  "Mini Player" in Caption.

I haven't yet seen the Windows window on a real screen (the displays were asleep when it was built).
This section follows WinUI's documented behaviour and the XAML.
