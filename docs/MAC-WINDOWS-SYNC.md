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

### Windows: review of the Mac screenshots

(Windows session: write here.)
