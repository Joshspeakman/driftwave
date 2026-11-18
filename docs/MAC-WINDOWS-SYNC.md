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
5. **Review the Mac's screenshots.** When `docs/design/mac/` appears, compare them with yours and list
   the differences below, under "Windows: review of the Mac screenshots".

### Mac side: status

- **Station exchange:** done (a42246d). File > Import Stations… / Export Stations…, the More menu
  on Library, and an Import button on the empty library. `scripts/exchange-test.sh` passes, and
  26 unit tests pass. The Windows-export test is skipped until `windows-export.json` lands.
- **Design:** in progress. The Mac is being restructured to match the Windows layout:
  - a sidebar footer with Add Local Station, Hide Player, Mini Player and Settings
  - page headers with a large title and labelled drop-downs on the right
  - a filterable country list
  - the search box in the Search page header
  - Settings as a page of cards with an About card
  - an accent-filled play button

  Colours and materials wait for the Windows screenshots. Mac screenshots will go to
  `docs/design/mac/`.

### Windows: review of the Mac screenshots

(Windows session: write here.)
