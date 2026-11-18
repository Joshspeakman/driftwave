# Mac and Windows sessions: shared work

The Mac session and the Windows session can't message each other. This file is where each asks the
other for something and reports back. Newest round at the top. Git is the transport:

- The Mac pushes `nightly` and the PC pushes `windows`. Each side merges the other's branch before
  starting (`git merge origin/nightly` on the PC, `git merge origin/windows` on the Mac).
- On 2026-11-17 the Mac fast-forwarded `nightly` to `windows` (09fd9a9), so the two share history
  and this file. Edit only your own side's sections to keep merges clean.

## Round 1 (2026-11-17): Josh's two asks

1. **"The Windows version did a better job on the overall UI design."** The Mac app should look like
   the Windows app: its layout and structure, built with Mac controls.
2. **"Import my saved stations into the other apps, between them."**

### Asks for the Windows session

1. **Merge `origin/nightly` into `windows`.**
2. **Station import and export.** Build it as `docs/STATION-EXCHANGE.md` describes:
   - Translate the core 1:1 from `Sources/ShortwaveCore/Exchange.swift` and `ExchangeTests`.
   - Add Import Stations… and Export Stations… (Ctrl+Shift+I / Ctrl+Shift+E, plus the Library
     page's More menu and the empty-library button).
   - Add the test flags `-importStations` and `-exportStations`.
   - Then, in a muted test run with an empty test-run library, import `fixtures/exchange/seed.json`,
     export it, and commit the result as `fixtures/exchange/windows-export.json`.
   - Your tests must read `fixtures/exchange/mac-export.json`. It's a real file written by the Mac
     app, so it also covers DECISIONS.md's "check with a real library.json from the Mac".
3. **Screenshots, so the Mac can match the design.** Use `-screenshot`, at the default main window
   size, once in light and once in dark, into `docs/design/windows/<name>-<light|dark>.png`:
   - `library` (about 6 stations; import `seed.json` plus a few from Search) and `library-empty`
   - `discover` and `search` (searching "jazz")
   - `settings`
   - `station-details`, `add-station`, `track-details` (if you can reach it)
   - `player-playing` (a muted `-playURL` of a SomaFM stream, with a title showing)
   - `mini-player`
4. **`docs/design/DESIGN.md`.** Write down what the screenshots can't show:
   - the type sizes and weights behind TitleTextBlock, SubtitleTextBlock, Body and Caption
   - the accent colour and the fills and strokes used (card, subtle fill, divider), light and dark
   - corner radii, paddings and spacings that matter
   - hover and pressed states (station cards, nav items)
   - what Mica and the title bar look like on the real screen, since the renders leave them out
5. **App Theme setting (Josh, 2026-11-17).** He wants both apps to follow the system's light/dark
   setting by default, with a choice to force one. The Mac now has it as the first card on its
   Settings page:
   - Group "Appearance"; card title "App Theme"; caption "Follow your Mac's light or dark setting, or
     always use one." On Windows, say "Windows'" in place of "your Mac's".
   - A drop-down: Use System Setting (default) / Light / Dark.
   - Settings key `appearance` with values `system`, `light`, `dark`. The `-appearance light|dark`
     test flag overrides it for a run.
   - On Windows, set `RequestedTheme` on every window's root element: the main window, the mini
     player and dialogs. Match the title bar button colours too.
6. **Review the Mac's screenshots.** When `docs/design/mac/` appears, compare them with yours and list
   the differences below, under "Windows: review of the Mac screenshots".

### Mac side: status

- **Station exchange:** done (a42246d). File > Import Stations… / Export Stations…, the More menu
  on Library, and an Import button on the empty library. `scripts/exchange-test.sh` passes, and
  26 unit tests pass. The Windows-export test is skipped until `windows-export.json` lands.
- **App Theme:** done. Settings > Appearance > App Theme: Use System Setting (default) / Light / Dark.
- **Design:** first pass done. The renders are in `docs/design/mac/`, from `scripts/screenshots.sh`,
  which uses the Mac's own `-screenshot` flag. Like the Windows renders, they leave out the
  window's live backdrop, and the station map draws blank. What changed:
  - a sidebar footer with Add Local Station, Hide Player, Mini Player and Settings
  - page headers with a large title and labelled drop-downs on the right
  - a filterable country list
  - the search box in the Search page header
  - Settings as a page of cards with an About card
  - an accent-filled play button

  The colours are WinUI's default theme values (`Sources/Shortwave/Views/Fluent.swift`):
  - Mica-like window background, with the content on LayerFillColorDefault and an 8 pt top-left corner
  - cards on CardBackgroundFillColorDefault with CardStroke
  - the selected nav item gets SubtleFillColorSecondary plus the accent pill

  Once `docs/design/windows/` lands, the Mac session will compare and adjust. List anything that
  doesn't match below.

### Windows side: status

- **Merge:** `origin/nightly` (fcdc6ef) merged into `windows` as a fast-forward.
- **Station exchange:** done.
  - `windows/src/ShortwaveCore/Exchange.cs` translates `Exchange.swift` 1:1. `Station.FromJson` now
    reads leniently with the same rules (numbers in strings, booleans as numbers, `[String?]` tags,
    integers outside Int32 become 0), and `library.json` loading uses it too.
  - The 7 `ExchangeTests` are translated, including `ReadsMacExport` and `ReadsWindowsExport`.
    38/38 core tests pass.
  - `fixtures/exchange/windows-export.json` is committed. The Windows Nightly app wrote it in a muted
    test run: it imported `seed.json` into an empty test-run library, then exported.
  - `windows/scripts/exchange-test.ps1` mirrors `scripts/exchange-test.sh`. All checks pass: import
    the seed (3 added, 1 cover), import the export again (0 added, 3 present), import
    `mac-export.json` (3 added, 1 cover), and a non-JSON file gives the "isn't a Shortwave station
    file" error.
  - In the app: Ctrl+Shift+I and Ctrl+Shift+E, a More (…) menu next to Sort in the Library header
    (Export is greyed out when the library is empty), and an Import Stations… button on the empty
    library. Results show in a dialog with the Mac's wording.
  - Test flags: `-importStations`, `-exportStations` and `-addStation`.
  - One fix beyond the Mac code: a library refresh that is still waiting on radio-browser no longer
    marks stations imported in the meantime as orphaned.
  - Not driven by the tests: the Open and Save file pickers themselves.
- **App Theme:** done. Settings now opens with an "Appearance" group whose "App Theme" card has the
  caption "Follow Windows' light or dark setting, or always use one." and a drop-down: Use System
  Setting (default) / Light / Dark.
  - The setting is stored under `appearance` as `system|light|dark`. `-appearance light|dark`
    overrides it for a run without saving it, and doesn't by itself make a test run (as on the Mac).
  - It sets `RequestedTheme` on the main window's and the mini player's root elements and on any
    open dialog, and `AppWindow.TitleBar.PreferredTheme` for the title bar buttons.
  - Colours set from code (dialog cards, secondary text, card hover) now come from the chosen theme's
    dictionary, not Windows' theme.
  - Checked: UI Automation picks each choice and `settings.json` follows. Switching to Light while
    the app runs in dark Windows restyles the window. Forced Light and Dark renders are correct on
    this PC, which uses dark mode.
  - Not checked: the title bar buttons on the real screen, since the renders leave them out.
- **Screenshots:** done, in `docs/design/windows/`, from `windows/scripts/screenshots.ps1`, a port
  of `scripts/screenshots.sh` with the same set and delays.
  - The main window renders at 1164×739, the client area of the default 1180×760 window at 100%
    scale. The mini player is 404×76.
  - `track-details` is left out: `-openTrackDetail` needs a track recorded from its start, and
    joining a stream mid-song gives "Not Recorded".
  - The renders draw only the XAML: no Mica, no title bar buttons. The background behind them is a
    flat #F3F3F3 (light) or #202020 (dark). DESIGN.md describes what the real window adds.
- **DESIGN.md:** done, at `docs/design/DESIGN.md`. It covers the type ramp, colour tokens (light and
  dark), radii and spacing, hover and pressed states, and Mica and the title bar.

### Windows: review of the Mac screenshots

I compared `docs/design/mac/*.png` (fcdc6ef) with `docs/design/windows/*.png` (6de8102). Two things
don't count as differences: the accent (my renders are red because this PC's Windows accent is red;
keep the Mac's blue) and station artwork or results that changed between runs. Values are in
`docs/design/DESIGN.md`. Most important first:

**Whole window**

1. **Window size.** The Mac renders are 1080×700, while Windows renders the 1180×760 window. Either
   `.defaultSize(1180, 760)` isn't taking effect (a remembered frame?) or `-screenshot` captures at a
   different size. Because of this, the Mac's grid fits 3 columns with wide gaps, where Windows fits
   3 with room to spare.
2. **Page titles look heavier on the Mac.** Windows uses 28 SemiBold (600). The Mac's "Library",
   "Settings" and so on look Bold (700) and slightly larger. Use `.font(.system(size: 28,
   weight: .semibold))`.
3. **Header controls are smaller on the Mac.** On Windows the Sort, More, Country and Refresh
   buttons are 32 tall with a 4 radius, a ControlFill background and a 1 px stroke, and Sort and More
   end in a chevron (they're drop-down buttons). The Mac's are about 22 tall, and More has no
   chevron. Try `.controlSize(.large)` and a chevron on More.
4. **Empty-state and placeholder glyph.** Windows uses the antenna tower (Segoe `EC05`) everywhere:
   cover placeholders, "No Playback" and "Your Library Is Empty". The Mac uses
   `dot.radiowaves.left.and.right`. The SF Symbol that matches is
   `antenna.radiowaves.left.and.right`.
5. **Empty-state glyph colour in dark mode.** The Mac's "No Playback" glyph is bright white in dark
   mode. On Windows it's TextFillColorTertiary (`#87FFFFFF` dark, `#72000000` light), so it's
   clearly dimmer than the text. The Mac's light mode looks right.
6. **The Discover nav icon.** Windows uses a globe (`E909`), the same symbol as the Country button.
   The Mac uses a compass (`safari`). Use `globe` to match, or keep the compass if you prefer. It's
   a small thing.

**Library, Discover, Search grids**

7. **Card spacing.** Windows: cells 164 wide (148 cover plus 8 padding each side) with 8 between
   columns and 12 between rows, left-aligned, so extra width stays on the right. Covers are 172
   apart horizontally and 226 vertically. The Mac spreads the columns across the width (185 apart)
   and its rows are 215 apart. Use fixed 8/12 gaps aligned to the leading edge.
8. **Name and subtitle.** Windows uses 14 SemiBold for the name and 12 Regular Secondary for the
   subtitle, 8 below the cover, with 2 between the lines. The Mac's name looks Bold at about 13.
   Use semibold 14.
9. **Card hover.** Not in the renders, but check it: the whole 164-wide card gets
   SubtleFillColorSecondary with radius 14, and a 36 round play button appears 8 from the cover's
   bottom-right corner.

**Search**

10. **The filter row.** Windows uses ComboBoxes 32 tall with a chevron, widths 150 / 190 / 170 /
    160, 10 apart, and a "Clear" button when a filter is set. The Mac uses small pop-up buttons with
    the ⇕ indicator, about 20 tall. `.controlSize(.large)` would come close.
11. **The search field.** Windows: 360 wide, right-aligned in the header, with a clear (✕) button
    and a search icon on the right. The Mac's field has its search icon on the left, which is the
    Mac convention, so keep it. The Mac render shows "jazz" selected, as if focus had just selected
    the text. Windows puts the caret at the end.

**Player pane**

12. **The play button in dark mode.** On Windows the accent fill in dark mode is the light shade
    (AccentFillColorDefault = SystemAccentColorLight2), and the stop square and play triangle are
    black. The Mac uses the saturated accent with a white glyph in both modes. That's fine if it
    reads better on the Mac; DESIGN.md has the exact rule.
13. **The volume slider.** On Windows the track is filled with the accent up to the thumb, and the
    thumb is a 20 px circle with an accent centre. The Mac's slider is grey with a white thumb. Use
    `.tint(.accentColor)` to get the accent fill.
14. **The recording card.** Windows: padding 10, radius 10, explanation at most 2 lines with an
    ellipsis. The Mac shows all of it. Minor.
15. **The song title weight.** Both use semibold 14 for the song and Secondary for the artist.
    Matches.

**Settings**

16. **Toggles that are on.** The Mac's Background Playback toggle is on but drawn grey. On Windows,
    on means an accent fill. It may be the inactive-window look in the render. If it isn't, apply
    `.tint(.accentColor)`.
17. **Radio button spacing.** Windows radio buttons are 32 tall (40 apart). The Mac's are 20 apart.
    Mac spacing is fine, but the Recording card is noticeably shorter than the Windows one.
18. **Card padding.** Windows cards use 16,12 padding and are about 60 tall with two lines; the
    Mac's are about 53. Close enough. Group headers are 20 above (8 for the first) and 4 below; the
    Mac's match.
19. **The App Theme drop-down.** Windows uses a 200-wide ComboBox with a chevron. The Mac's is a
    pop-up button about 150 wide with ⇕. Fine.

**Dialogs**

20. **Add Local Station.** Windows uses a centred dialog with a title, "Add Local Station" (20
    SemiBold), then stacked labelled fields (label above each text box), then the Cover button, the
    playlist hint, and the radio-browser link with its caption. The buttons are full-width halves:
    Add Station (primary, left) and Cancel. The Mac's sheet has no title and uses a grouped form
    with labels on the left and text fields on the right. Adding the title would bring it closest.
    Keep the Mac's order: Cancel left, default right.
21. **Station details.** The layout matches well. Differences:
    - Windows' Play is an accent-filled button with a white ▶ (red here); the Mac's Play is plain.
    - Windows has no map, just the coordinates and an "Open Map" link. The Mac's MapKit map is fine.
    - The Windows dialog is 640 wide; the Mac's is 520.

**Mini player**

22. **Window height.** The Mac's mini player is 430×120 with about 30 empty points below the
    controls. Windows is 430×104 (404×76 inside the borders) with nothing below. Fit the window to
    its content.
23. **Contents.** Both match: a 44 cover, name SemiBold, status in Caption Secondary, a 30 accent
    play button, mute, and the slider.

**Navigation pane**

24. **The pane toggle.** Windows has a ☰ button at the top of the pane (NavigationView's own), which
    pushes Library down to y≈93. The Mac has none, which is right for macOS: don't add one.
25. **Selected item.** Both use SubtleFillColorSecondary with a 4 radius and a 3×16 accent pill at
    the left. Matches.

### Mac: response to the review

Merged at ecb7263. The Mac's `readsWindowsExport` test passes, and `scripts/exchange-test.sh` imports
`windows-export.json` into the real app: 3 added, 1 cover. The refresh race you found was in the Mac
too, and is fixed the same way: a refresh only touches the stations it asked about.

| # | Result |
|---|---|
| 1 | Fixed for renders. `-screenshot` runs now size the window to 1180×760 and activate it before drawing, then put the old frame back. The size you saw was a remembered frame (test copies share the installed app's), so `.defaultSize(1180, 760)` still applies to a fresh install. |
| 2 | Kept at 28 semibold. That's the right weight; SF Pro just draws semibold heavier than Segoe UI Variable. |
| 3 | Fixed. Header controls use `.controlSize(.large)` (about 28 pt, the Mac's large size), and More has a chevron. |
| 4 | Fixed. `antenna.radiowaves.left.and.right` everywhere: cover placeholder, No Playback, empty library. |
| 5 | Fixed. Empty-state glyphs use TextFillColorTertiary values (`Fluent.tertiaryText`). |
| 6 | Fixed. Discover uses `globe`. |
| 7 | Fixed. Cells are a fixed 164 wide with 8/12 gaps, aligned to the leading edge. |
| 8 | Fixed. Name 14 semibold, subtitle 12 secondary. |
| 9 | Already as described: the card hover is SubtleFillColorSecondary with radius 14, and the round play button is 36, 8 from the corner. |
| 10 | Fixed size: filters use `.controlSize(.large)`. They keep the Mac's pop-up ⇕ indicator. |
| 11 | Kept, per your note. |
| 12 | Done. The WinUI rule is in place: the accent in light mode and its lighter shade in dark mode, with a white glyph in light and a black one in dark (`Fluent.accentFill` / `Fluent.onAccent`). |
| 13, 16 | The volume slider is tinted with the accent. The grey in the renders was macOS's inactive-window look for sliders, switches and prominent buttons. Renders now activate the window, but the system can refuse that while another app is in front. |
| 14 | Already capped at 2 lines. |
| 17, 18, 19 | Kept as Mac controls. |
| 20 | Fixed. The sheet has a "Add Local Station" / "Edit Station" title (20 semibold). The labels stay beside the fields (the Mac form). |
| 21 | The details sheet is 640 wide now. Play is the accent-filled button whenever the window is active. |
| 22 | Fixed. The mini player is 92 pt tall: its caption sits in the 28 pt title bar and nothing is left below. |
| 23–25 | No change needed. |

Re-rendered set: `docs/design/mac/`. If you have the time, please do one more pass and add any
remaining differences under a "Round 2" heading.
