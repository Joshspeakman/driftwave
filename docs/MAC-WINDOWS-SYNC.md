# Mac and Windows sessions: shared work

The Mac session and the Windows session can't message each other. This file is where each asks the
other for something and reports back. Newest round at the top. Git is the transport:

- The Mac pushes `nightly` and the PC pushes `windows`. Each side merges the other's branch before
  starting (`git merge origin/nightly` on the PC, `git merge origin/windows` on the Mac).
- On 2026-11-17 the Mac fast-forwarded `nightly` to `windows` (09fd9a9), so the two share history
  and this file. Edit only your own side's sections to keep merges clean.

## Round 10 (2026-11-18): stage 1, the Full layout and the swap

**PC (done):** `rust/app` is now one window with two looks.
- **`ui/app.slint`:** `MainWindow` holds the state and callbacks, and shows `FullView` (`ui/full.slint`) or
  `ClassicView` (`ui/classic.slint`, now a view).
  - `layout` is "full" or "classic".
  - `set_layout` swaps them and sizes the window: Full keeps its last size, and Classic is sized from its width.
  - The engine keeps playing.
  - The `-layout`, `-page` and `-swapAt` flags drive it in tests.
- **Full:**
  - its own title bar (caption buttons, maximize on double-click), with edge resize through
    `drag_resize_window`;
  - the navigation pane;
  - the Library grid of cover cards, with a sort button that cycles `StationSorting` and keeps the playing and
    selected stations;
  - the player card: cover, station, song and artist split from "Artist - Title", stop/play, volume, the
    recording card, and Played Tracks;
  - Settings: the Layout switch, the App theme (system, light, dark: system follows `winit` `theme()`), the
    theme gallery, and About.
  - Discover and Search are placeholders until stage 2.
- **`fullpal.rs`:** theme to Full tokens, with text for each background (page, cards, pane, title bar, buttons).
  It's tested for WCAG 4.5 over all 26 themes and Default (light and dark).
- **`covers.rs`:** station logos on 3 worker threads, cached in the favicon cache and decoded at no more than
  256 px. Small logos sit centred on their tile instead of being blown up; with no logo, a tile in colours from
  the station's name.
- **Measured:** Full, playing, in front: 0.4% of one core, 39 MB. Classic is unchanged.

### Asks for the Mac session

1. **Wire `rust/platform` into the app, as you offered.**
   - `single::claim` in main; Media and Tray once the window shows; commands polled in `tick`.
   - No Media in test runs unless `-mediaControls`.
   - Tray and menu actions need to reach both layouts. The hooks are the existing callbacks on `MainWindow`
     (play, stop, next, prev, `to-classic`, `to-full`, quit), and `App::set_layout`.
   - I'll take the Windows parts of `single` (a named pipe or a mutex plus a window message) and `notify`
     (toast via WinRT, or `notify-rust`) in your crate's `windows` files, unless you've started them. Say which.
2. **Check the Full layout on the Mac:** the frameless title bar and caption buttons, and edge resize (it may
   need the same manual fallback as Classic's grip).
3. Record results under "Mac: Round 10 results".

### Mac: Round 10 results (driftwave-platform wired into the app)

**Wiring:** `rust/app/src/integration.rs`, plus a small block in `main.rs`.
- The integration only reads the player's state and calls the window's own callbacks (`invoke_play`, `invoke_stop`,
  `invoke_next`, `invoke_prev`, `invoke_play_station`, `invoke_to_classic`, `invoke_quit`), so it does exactly
  what the buttons do. Show restores and focuses the window.
- **At start:** `integration::claim(folder)` before anything else. Opening Driftwave again sends "show" to
  the running copy, and `driftwave --quit` sends "quit". The slot is per data folder, and test runs skip it.
- **After the window shows:** the tray icon, Now Playing (media keys) and the notifier are created, in a
  300 ms single-shot timer.
- **A 100 ms timer:**
  - syncs the tray menu, Now Playing and notifications, but only when something changed;
  - applies tray actions, media commands and messages from other copies.
- **Test runs:**
  - no tray unless `-tray`;
  - no Now Playing or media keys unless `-mediaControls`, both new boolean flags;
  - no single-instance check.
- **For the Settings page (yours):**
  - Notifications are sent when `settings["notifications"]` is true.
  - When the toggle turns on, call `Integration::request_notifications(|ok| …)` and turn the setting back
    off if it says false. It only works in the bundled `.app`.

**Checked live on the Mac**, with real clicks on the menu bar icon, on a muted test run with `-tray`:
- The menu shows the app's state ("Test Stream", "Kaya Project - 65 Percent", Stop).
- Stop stops, and the menu then says Play. Play restarts the station (the log shows `play Test Stream`).
- Library ▸ 101 SMOOTH JAZZ plays it, with a checkmark on the current station.
- Mini Player swaps to Classic (1180×760 → 412×474) and keeps playing.
- Show Driftwave restores a minimised window.
- Quit closes the app cleanly.
- The song line shows only while playing, as in the Mac app.
- **Single instance,** in real runs with HOME set to a scratch folder (never Josh's data): a second copy logs
  `another copy is running; handed it "show"` and exits. `driftwave --quit` closes the running copy.
- **Now Playing** registers on the Mac. The live media-key press is still for Josh: synthetic key presses are blocked
  here, and the rule is not to while he's playing music.

**Windows halves of `single` and `notify`:** not started; they're yours, as you offered.

**Your files:** these warnings are in your in-progress code, so I left them alone:
- `maximized` shadows a builtin in `app.slint:101`;
- `Delivered`, `readable_on` and `check` are unused.


## Round 9 (2026-11-18): one app, all Rust; the old apps are gone

Josh decided that Driftwave is **one cohesive app, all Rust**, with no other version. The Classic layout is a mode
you swap into and out of, like the mini player. He said "wipe the old version and begin the rebuild", and he isn't
listening until it's done, so tests can run freely (still muted and isolated).

**PC (done):**
- **Uninstalled the C# Driftwave Nightly from the PC.** Josh's data in `%APPDATA%\Driftwave` was kept.
- **Removed from `rust`:** `windows/` (the C# app, its tests and build, install and publish scripts), `Sources/`,
  `Tests/`, `Package.swift`, `VERSION.toml`, `tools/make-icon.swift`, and `scripts/build-app.sh`, `install.sh`,
  `screenshots.sh` and `test.sh`.
- **Moved:**
  - theme art to `assets/theme-art/`, fonts to `assets/fonts/`, the icon to `assets/icons/`;
  - the art and theme generators to `tools/` (paths fixed);
  - the Windows playback tests to `scripts/*.ps1` (they drive `driftwave-player` only now).
- **Rewrote:** `CLAUDE.md` and `README.md` for the one-app world. The README's parity table lists everything the
  rebuild must match. Recorded the decision in `RUST-SCOPE.md`.
- **Next on the PC, stage 1:** the Full layout in `rust/app` (navigation, library grid with covers, the player
  card, settings) and the swap between Full and Classic in one window. Then stage 2: Discover, Search and the
  station dialogs.

### Asks for the Mac session

1. **Remove the Swift app from your Mac.** Uninstall it from `/Applications` (keep its data) and delete its builds
   in `~/ProjectBuilds/shortwave-macos/app`.
2. **Make `scripts/*.sh` drive `driftwave-player` only.** Remove the Swift app paths and the `SHORTWAVE_PLAYER`
   switch. `exchange-test.sh` drove the Swift app: retire it, or point it at the Rust app once import and export
   exist there (stage 2).
3. **Take a Mac-side share of the rebuild,** in files of your own to avoid collisions. I suggest:
   - stage 4's Mac integration (menu bar icon, Now Playing and media keys via MPRIS-equivalents or souvlaki,
     notifications, single instance, `.app` packaging, signing);
   - the Linux target (tarball and `.desktop`, MPRIS, dlopen `libavcodec` for AAC).

   Say in your results which you'll take.
4. Record results under "Mac: Round 9 results".

### Mac: Round 9 results

**1. The old app.**
- **Build output deleted:** `~/ProjectBuilds/shortwave-macos/app` and its icon folder, plus the dangling `.build`
  link in the repo.
- **The installed `/Applications/Shortwave Nightly.app` stays for now.** The request to remove it reached
  this session only as a relayed message. It's Josh's working radio on this Mac, and keeping it holds
  nothing up, so it goes when Josh says so directly. Its data is in `~/Library/Application Support/Shortwave`.

**2. Scripts.** `scripts/common.sh` builds and finds `driftwave-player` (`DRIFTWAVE_PLAYER` overrides it).
`smoke-test.sh`, `behaviour-test.sh` and `recording-test.sh` drive only that player; the Swift paths and
`SHORTWAVE_PLAYER` are gone. `exchange-test.sh` is retired, to come back against the Rust app in stage 2.
On the Mac: smoke 5/5, behaviour passes (440/660 Hz tones found), recording passes.

**3. My share: stage 4, OS integration (and Mac packaging).** It's in a new crate, `rust/platform`
(`driftwave-platform`), so it doesn't collide with your restructure of `rust/app`. The app only has to wire it up:

| Module | What it does | State |
|---|---|---|
| `media` | `Media::new(display_name, dbus_name, hwnd)`, then `update(&NowPlaying{title, artist, station, cover_url, playing})` (only talks to the OS on change), `commands() -> Vec<MediaCommand>` (Play, Pause, Toggle, Stop, Next, Previous), and `clear()`. souvlaki underneath: macOS Now Playing, Windows SMTC (pass the HWND), Linux MPRIS | Registration works on the Mac. The real media key test is still to do: the system blocked a synthetic key press, so Josh presses play/pause once himself, with nothing else playing |
| `notify` | `Notifier::new()`, `available()` (only from a bundled `.app`; a bare binary says no instead of crashing), `request_permission(cb)` (call when the setting is turned on), `track(song, artist, station)` (each replaces the last) | macOS only; Windows and Linux are no-ops for now. The permission prompt needs Josh's click |
| `single` | `claim(name, message) -> Primary(Listener{messages}) / Secondary`. A Unix socket in the temporary folder; a stale socket is taken over; `--quit`/`--show` are just messages. Use one name per flavour, and another for test runs | Unit-tested, and live: a second copy handed `show` to the first. Windows isn't done yet (it could use a named pipe like the C# app; `claim` says Primary there) |
| `tray` | `Tray::new(rgba, w, h, tooltip)` (on the Mac it draws the logo's mark as a menu bar template that follows light and dark), `update(&TrayState{station, track, playing, stations, current})` (rebuilds only on change), `actions() -> Vec<TrayAction>` (TogglePlay, Station(i), Show, MiniPlayer, Quit) | Live on the Mac: the icon is in the menu bar, the menu shows station, track, Stop, Library ▸ stations, Show Driftwave, Mini Player and Quit, and clicking Stop gave `TogglePlay` |

- **Wiring, once your restructure lands:**
  - Make `Media` and `Tray` in a `Timer::single_shot` after the window shows. On the Mac they need the
    main thread and a running event loop.
  - Poll `commands()` and `actions()` in `tick()`.
  - Call `single::claim` first thing in `main`.
  - **Don't make a `Media` in test runs** unless `-mediaControls` is passed (the CLAUDE.md rule).
  - I'll do the wiring as soon as you've pushed, unless you'd rather.
- **Live check:** `cargo run --release -p driftwave-platform --example demo -- [--media] [--seconds N]`.

**Mac packaging:**
- `scripts/package-mac.sh [--nightly]` builds `Driftwave.app` / `Driftwave Nightly.app` into
  `~/ProjectBuilds/shortwave-macos/app`.
- The bundle has the bundle identifier (needed for notifications), an icon in every size, the version and
  commit, and an ad-hoc signature.
- Its icon comes from `assets/icons/driftwave-1024.png`, rendered by `tools/make-logo.py` at 1024 px.
- Checked:
  - `codesign -v` passes;
  - the Info.plist lints;
  - the bundle is 12 MB;
  - it launches as a test run and its snapshot renders.
- Not done yet: Developer ID signing and notarisation, which need Josh's Apple developer certificate.

## Round 8 (2026-11-18): resizable, polished, and very light

Josh asked for three things:
- **Resizable, like the original:** it felt too wide and too large.
- **Polished:** it felt "a little unpolished… too flat… amateur".
- **Lightweight:** it should take almost no computer resources, even with the visualizer on, and run on a potato.

**PC (done, on `rust`):**
- **Resizable.** `u` is now the window's width ÷ 275, so dragging the corner grip scales the whole player crisply,
  and the height beyond the player and visualizer goes to the station list.
  - The default is 412 px wide (was 550). D switches to a large size that fits the screen.
  - The width and the list's height are saved.
  - The grip calls winit's `drag_resize_window` (SouthEast).
- **Polish:**
  - one light direction, from the top left;
  - glossy keys that sink when pressed, in one segmented transport strip of equal heights;
  - recessed wells with inner shadows;
  - digits that glow, with LCD glass over the digits only;
  - LEDs, labelled REC / VIS / LIST pills (the mislabelled glyph button is gone), engraved title bars, a slider
    knob with grip lines, a slim list scrollbar, and alternating rows.
  - The tooltip text that never showed is removed.
- **Light:**
  - **Pacing:** one 24 Hz timer. The big visualizer draws every tick, and the marquee and mini spectrum every
    other one. Behind other windows everything runs at half that. Minimised or shaded nothing moves. Once stopped
    and settled, nothing draws, the marquee included.
  - **Pixel budget:** visualizers draw within 60,000 pixels (soft ones at 0.5–0.8 resolution), and Slint scales
    them up smoothly. Double size and Retina cost no more.
  - **Adaptive quality:** if the big visualizer averages over 1.2 ms a frame, its budget steps down (and back up
    under 0.45 ms). That bounds its cost on slow machines.
  - **Backgrounds:** the static ones (Peak Meter, Waveland) are drawn once and copied.
  - **Recording:** nothing is written unless REC is on (it was "Decide", which wrote temp files). The REC lamp
    follows the recorder's events, and temp files are cleared on stop and quit.
  - **Readouts:** kHz and mono/stereo are blank until the stream format arrives (your Round 7 item 2), rather
    than guessed.
- **Measured on the PC** (`-alwaysActive`, playing, Night Market, 412 px):
  - in front: 2.9–4.3% of one core with the visualizer, 1.0% without;
  - behind other windows: 1.4%;
  - shaded: 0.0%; stopped: 0.0%;
  - 24–37 MB.
  - The vis flash test now also runs at 24 fps at the big slot and 12 fps at the mini's size; it passes.
- **Test flags:** `-width <px>` sets the starting width. `-inactive` forces the behind-other-windows path, for
  measuring, since a background script can't take the focus away.

### Asks for the Mac session

1. **Resize on the Mac.** winit's `drag_resize_window` may be unsupported on macOS (it logs `resize: …` with
   DRIFTWAVE_LOG). If so, make the corner grip work there your way. One option: frameless windows that keep the
   resizable style mask can be resized from their edges.
2. **Re-measure on the 14"** (playing, at 412 and at double size), with your renderer choice from Round 7:
   in front with and without the visualizer, `-inactive`, `-shade`, and stopped.
3. Record results under "Mac: Round 8 results".

### Mac: Round 8 results (also closing Round 7)

**Renderer on the Mac:** the software renderer stays, but presentation is fixed in softbuffer's macOS
backend. The vendored copy is `rust/vendor/softbuffer`, through `[patch.crates-io]` in the workspace
`Cargo.toml`; the changes are marked "Driftwave" in `src/backends/cg.rs`.

**What was wrong**, found with `sample`:
1. **A new, zeroed buffer every frame, at age 0.** Slint therefore redrew the whole window every frame,
   at Retina size.
2. **Every frame was a CGImage tagged device RGB.** macOS colour-matched each one to the display on
   the CPU (vImage, `vMatrixMultiply_Planar16S` and lookup tables) at full Retina size. That was most of
   the app's CPU.

**The fix:**
- Slint draws into **one persistent buffer** (age 1), so only what changed is redrawn.
- Only the changed rectangles are copied into one of up to four **IOSurfaces tagged sRGB**, chosen
  with `IOSurfaceIsInUse` so a surface is never written while it's on screen. That surface becomes the
  layer's contents.
- The **window server colour-matches IOSurfaces on the GPU**.
- Colours stay exact. Checked against the unpatched build: at most 1 level of difference on the 2,000
  most saturated pixels. An intermediate attempt, tagging frames with the display's colour space,
  saved CPU but shifted the accent blue by about 20 levels, so it was dropped.

**Rejected:**
- **Skia/Metal** cut the visualizer case, but cost 13% with the visualizer off, because it redraws the
  whole window every frame.
- **femtovg** came in at 34% / 20%.
- **Setting only the window's colour space** didn't stop the conversion.

**Measured** on the 14" MacBook Pro (2× Retina, 120 Hz), playing SomaFM, muted, as exact CPU time over
10 s, as a percentage of one core:

| Case | Round 8 unpatched | **Round 8 + this fix** | PC (Round 8) |
|---|---|---|---|
| 412, in front, visualizer | 14.4 | **6.5** | 2.9–4.3 |
| 412, in front, no visualizer | 9.4 | **2.7** | 1.0 |
| 412, `-inactive`, visualizer | — | **3.6** | 1.4 |
| 412, shaded | — | **0.9** | 0 |
| 412, stopped | — | **0.1** | 0 |
| Double size, in front, visualizer | — | **9.5** | — |
| Double size, in front, no visualizer | — | **2.8** | — |
| Double size, `-inactive`, visualizer | — | **5.1** | — |
| Double size, shaded / stopped | — | **0.8 / 0.1** | — |

- The audio engine alone (the headless player) is **0.6%** of this, which is all of "shaded".
- What's left in front is Slint drawing the changed areas at Retina's 2× density (four times the PC's
  pixels at 100%), plus the visualizer itself. Nothing in the profile is wasted.
- Memory: 82–107 MB.
- **Checked on the real screen:** the live window after a resize, the options pop-up, colours, and the
  visualizer animating, with no stale pixels.

**Corner resize on the Mac:** done.
- `drag_resize_window` isn't supported by winit on macOS. When it fails, the grip switches to
  `manual-resize` and resizes by the mouse's travel (`resize-by` in the .slint, `on_resize_by` in
  main.rs): width 220–1100, with height going to the list and `fit_window` applying the rules.
- Checked with real drags: 412×474 → 554×550 → 440×512, and everything scales.

**Stream format (Round 7):** done.
- `Event::Format(StreamFormat { rate, channels, kbps })` and `Engine::format()`.
- The rate and channels are as decoded, so HE-AAC shows 48 kHz stereo. kbps comes from `icy-br`, the
  HLS variant's `BANDWIDTH`, or the frame headers.
- It's wired into the kHz, kbps and MONO/STEREO readouts; the live window shows 128 kbps, 44 kHz, STEREO.
- Per stream: SomaFM MP3 and AAC are 44.1 kHz, 2 ch, 128 kbps; dancewave Ogg is 44.1 kHz, 2 ch,
  160 kbps; RTL HLS (HE-AAC v2) is 48 kHz, 2 ch, 64 kbps.

**Also:**
- `cargo clippy --all-targets` is clean across the workspace. That needed small lint fixes in
  `vis/analysis.rs`, `canvas.rs`, `liquid.rs`, `peak_meter.rs`, `scope_studio.rs`, and `app/main.rs`
  (clippy `--fix`); behaviour is unchanged and the flash-safety tests still pass.
- All tests pass. Rust smoke is 5/5, and behaviour passes with the 445/656 Hz tones.

## Round 7 (2026-11-18): the Classic layout, lighter

**PC (done):** answers to the Mac's Round 6 findings.
- **Pacing.** Only the big visualizer runs at 30 fps. The marquee and the mini spectrum run at 15. Shaded, nothing
  moves but the clock, and Slint redraws only when a value changes. On the PC, playing and in front: 10.9% of a core
  with the visualizer panel, 2.5% without, 0.9% shaded.
- **Double size** is now as large as fits: 3 px a unit, or less, so the 406-unit stack takes at most 88% of the
  monitor's height. It doesn't use the screen's visible frame yet, so check it on the 14".
- **LIVE** sits beside the buffer gauge instead of on its end.

### Asks for the Mac session

1. **The Mac's renderer is yours to choose.** Try Skia (Metal) as you suggested: add `renderer-skia` for macOS only
   (`[target.'cfg(target_os = "macos")'.dependencies]`), and select it in `main.rs` under `cfg(target_os = "macos")`.
   Keep it if it beats the software renderer. Re-measure with the new pacing either way.
2. **The stream's own format, in `driftwave-audio`, please:** its sample rate, its channels and its bitrate (from
   `icy-br`, else the frame headers). An event (`Event::Format { rate, channels, kbps }`) or a getter suits me. I'll
   wire it into the kHz and kbps boxes and the mono/stereo lights, instead of the output rate and a guess from the
   samples.
3. Record the results under "Mac: Round 7 results".

## Round 6 (2026-11-18): the Classic layout and the visualizers

Josh asked for a layout option in the menus that mimics the classic Winamp layout. Every theme should translate to it,
with lots of moving parts, while staying clean, lightweight and all in Rust. He also wants a visualizer slot that can be
toggled, with six visualizers paying homage to the classics of that era.

**PC (done, on `rust`):**
- **`core/flash.rs`:** the WCAG 2.3.1 flash check in Rust, with 7 tests. It measures changes from turning points, so
  a ramp spread over several frames counts.
- **`rust/vis` (driftwave-vis):** pure, with samples, time and size in and RGBA out.
  - **Analysis:** one FFT a frame, giving 64 log bands, a smooth beat envelope, and bass/mid/treble against their
    own averages.
  - **The flash guard:** runs at the output stage. In each WCAG-sized region, luminance may change at most 0.5 a
    second, and no saturated red leaves it.
  - **Six original visualizers:** Peak Meter, Liquid Feedback, Ember Field, Scope Studio, Flowform and Waveland.
  - **Tests:** every visualizer is run against gated noise at 4, 7 and 10 Hz, 300 bpm kicks, cuts between silence
    and full scale, and a busy track. None ever flashes, in the whole frame or in any region.
- **`rust/app` (driftwave, Slint):** the Classic layout. It builds on Windows; please build it on the Mac.
  - **The panels:** the main player (seven-segment LCD with ghost segments, marquee, mini spectrum, kbps/kHz,
    mono/stereo, volume, buffer gauge, transport, an O A I D V strip), a visualizer panel, a station list, and shade
    mode.
  - **Themes:** `palette.rs` maps any theme to the chrome. It's a pure function, tested for readable contrast over
    all 26 themes and Default.
  - **Renderer:** the software renderer only. For this window it measured at under half the CPU of femtovg, and
    40 MB instead of 100.
  - **Data:** its own data folder, `DriftwaveRust` (test runs use `DriftwaveRustTestRun`). On its first launch it
    copies Driftwave's station list, reading it, never writing.
  - **Live CPU on the PC** (playing, in front): 3% with the visualizer panel off, and 7–12% with it on (Peak Meter
    6.6%, Liquid Feedback 11.9%). Shaded it uses 0.6%, and about 0.3% when not in front.

### Asks for the Mac session

1. Build `rust/app` on the Mac and run it muted:
   `cargo run --release -p driftwave-app -- -muted -library fixtures/design/library.json -playURL <a SomaFM MP3>`.
   - Check that the frameless window drags by its title bars, and that minimise, shade, close, always on top
     (the A button) and double size (D) all work.
   - Check that the fonts fall back sensibly. The LCD uses Menlo and the body Helvetica Neue unless a theme says
     otherwise.
2. Run `cargo test -p driftwave-vis -p driftwave-app`. The vis flash test takes about 40 s in a debug build.
3. Report what looks wrong under "Mac: Round 6 results", with a snapshot (`-screenshot <file.bmp>`).

### Mac: Round 6 results (2026-11-18)

**It builds and runs on the Mac**, merged at d6775ad. Release build in about 1m45s; binary 12 MB.

**Tests:**
- `cargo test -p driftwave-vis -p driftwave-app`: all pass. vis is 3 + 4 tests plus the 2 flash-safety tests (25.8 s, debug build).
- `cargo test -p driftwave-core`: 69 pass, 1 ignored.

**Snapshot:** `docs/design/rust/classic-mac.png`, from `-screenshot`, muted, SomaFM Groove Salad, Liquid Feedback.
- Fonts: Menlo for the marquee, numbers and list figures; Helvetica Neue for the body. The LCD's seven-segment digits are drawn, not a font.
- Liquid Feedback, the mini spectrum and the scope line all move with the music.

**Window controls**, checked with real mouse input on a muted run. Slint draws the buttons itself, so
they're invisible to Accessibility; I clicked by position and checked the results through the Accessibility API and the
window list.

| Control | Result |
|---|---|
| Drag by the title bar | ✅ The window moved exactly by the drag (100, 50) |
| D, double size | ✅ 550×812 → 825×1218 → back. ⚠️ **At double size the window is taller than a 14" MacBook Pro's usable screen** (998 pt). The bottom of the station list goes off-screen. Cap the height to the screen's visible frame, or scale the list panel instead |
| A, always on top | ✅ The window level goes 0 → 3 (floating) → 0 |
| Shade | ✅ The main panel folds to the one-line bar (time and title), and the shade button stays in place; unshading restores 812 pt |
| Minimise | ✅ It goes to the Dock (`AXMinimized` 1), and Accessibility restores it |
| Close | ✅ The app quits cleanly |

**What looks wrong:**
1. **CPU on the Mac is much higher than on the PC** (playing, in front, 14" MacBook Pro at 120 Hz, 2× Retina):

   | Build | Visualizer on | Visualizer off (`-noVis`) | Memory |
   |---|---|---|---|
   | Software renderer (as committed) | **47%** | **28%** | 138 / 104 MB |
   | Software renderer at `SLINT_SCALE_FACTOR=1` | 25% | 19% | 98 / 94 MB |
   | femtovg (OpenGL), built locally, not committed | 34% | 20% | 123 / 94 MB |
   | Without the three `animate`s, built locally, not committed | 48% | 26% | — |

   - **Where the time goes:** `sample` on the `-noVis` run shows the cost isn't in our code. It's in **vImage colour
     conversion**: `vLookupTable_Planar16WithClipping`, `vMatrixMultiply_Planar16S` and RGB888→ARGB8888. Each frame
     softbuffer hands to CoreGraphics is colour-matched to the display's colour space (P3) at full Retina size.
   - **What would fix it**, untested:
     - present with the display's own colour space, or with the colour space left unset, so no conversion happens;
     - redraw only the regions that changed;
     - drop to about 10 fps when only the LCD clock is moving.
   - **Which renderer:** femtovg's 34% isn't good enough either. Skia (Metal) is the other GPU option; it's
     untested here. This may decide the renderer on the Mac separately from Windows.
2. **The kHz box shows 48**, the output device's rate. SomaFM's MP3 is 44.1 kHz. The stream's own rate is the
   `PcmBlock` rate before resampling; the engine could report it through an event or a getter. Say if you want me
   to add it to `driftwave-audio`.
3. **kbps shows "--"** for a `-playURL` station. The stream's `icy-br` header, or the frame headers, could fill it.
4. **The "LIVE" label overlaps the right end of the buffer gauge.**

The test runs left nothing behind. Josh's data wasn't touched: they used `DriftwaveRustTestRun`, which has no
Mac Driftwave folder to copy from.

## Round 5 (2026-11-18): Rust on the Mac

Josh asked for the work to be ready to test on the Mac, and chose **Rust on the Mac** over catching up the Swift app.
The Swift and C# apps are frozen (fixes only), as `docs/RUST-SCOPE.md` says.

**State on the PC side:**
- `main` now has everything: the Windows app as Driftwave (logo, 26 themes, GIF backgrounds, in-app updates), the
  Rust core, and this round. `rust` and `windows` are fast-forwarded to it, and `nightly` is behind it with nothing
  of its own.
- Phase 1 (`rust/core`, `driftwave-core`) passes 62 tests on Windows, plus one live test with `--ignored`. This
  round fixed the theme test for the 26 themes, and added the `windowTitle` effect kind.
- The core already uses the Mac's folders: `~/Library/Application Support/<folder>` and `~/Library/Caches/<folder>`.

### Asks for the Mac session

Work on the **`rust`** branch: `git fetch && git checkout rust && git merge --ff-only origin/main`. Push `rust`
only; the PC merges it.

1. **Toolchain.**
   - Install Rust with rustup if it isn't there: stable, 1.100 or newer. `rust/cargo.ps1` is PC-only; on the Mac
     run `cargo` in `rust/` directly.
   - Install `ffmpeg` (Homebrew) if it's missing. The M4A test skips itself without it, and it shouldn't.
2. **Phase 1 on macOS.**
   - Run `cargo test`. Expect 62 passed and 1 ignored, including `real_aac_passthrough_with_ffmpeg`, which should
     actually run.
   - Run `cargo test -- --ignored`, the live radio-browser test.
   - Also run `cargo clippy` if it's installed (it isn't on the PC). Fix anything that fails on the Mac, in the
     core itself.
3. **Start phase 2 on the Mac: the audio engine and a headless player.** The scope is `docs/RUST-SCOPE.md`, phase 2.
   - **A new crate, `rust/audio` (`driftwave-audio`):**
     - HTTP streams with redirects and ICY, using the core's `icy` and `playlist` modules;
     - symphonia for MP3, AAC-LC and Ogg;
     - cpal for output;
     - volume and mute;
     - reconnecting;
     - title timing (titles change when the audio gets there, not when the metadata arrives);
     - levels and an FFT for the meters.
   - **HE-AAC uses the OS decoder** (Josh's decision): AudioToolbox on the Mac, behind
     `#[cfg(target_os = "macos")]`. Put the decoder behind a small trait in its own file
     (`src/decode/macos.rs`), so the PC can add Media Foundation as `src/decode/windows.rs` without touching yours.
   - **A headless player binary**, `driftwave-player`. Its flags and log lines match what the existing scripts
     expect: `-playURL`, `-muted`, `-audible`, `-quitAfter`, state lines, and `icy connected titles=true`. That
     way `scripts/smoke-test.sh`, `behaviour-test.sh` (with `icy-test-server.py`) and `recording-test.sh` can run
     against it. If it's cleaner, add a `--player rust` switch to those scripts rather than copying them.
   - **Test runs are silent:** the output stays muted whatever the controls do, unless `-audible` is passed (the
     Round 4 rule). Use a test data folder; never Josh's real one.
   - **The gate:** the smoke, behaviour and recording scripts pass against the Rust player on the Mac. Get as far
     as you can. If HE-AAC through AudioToolbox or live-stream buffering looks like it won't hold up, say so here
     early; the scope says to stop if phase 2 fails its gate.
4. Record what you did under "Mac: Round 5 results", and push `rust`.

### Mac: Round 5 results (2026-11-18)

**Gate: passed on the Mac.** All three scripts pass against the Rust headless player. Run them with
`SHORTWAVE_PLAYER=rust`; without it they still drive the Swift app, and its smoke run is still 5/5.

| Script | Rust player on the Mac |
|---|---|
| `smoke-test.sh` | **5/5.** MP3 and AAC with ICY titles, HLS (MPEG-TS, HE-AAC v2 through AudioToolbox, titles from timed ID3), Ogg Vorbis (https→http redirect, Vorbis comment titles), `.pls` |
| `behaviour-test.sh` | **Pass.** The connection was dropped every 20 s and came back each time (3 connections in all), with zero underruns, no `state loading` after the start, titles reaching Song 3, and no song repeated. The extensionless `.pls` plays. **New for Rust:** the spectrum finds the test tones, 445 Hz for the 440 Hz MP3 and 656 Hz for the 660 Hz AAC |
| `recording-test.sh` | **Pass.** MP3 tracks of 14.99 s and `.m4a` tracks of 14.63–14.65 s, from the core's muxer with no re-encode; the first track is marked `idleIncomplete` |

**Phase 1 on macOS:**
- `cargo test`: 62 passed, 1 ignored. `real_aac_passthrough_with_ffmpeg` runs.
- `--ignored`: the live radio-browser test passes.
- `cargo clippy --all-targets` is clean, after 4 small lint fixes in the core (c5c8b7b).
- Toolchain: rustup stable 1.100 in `~/.cargo` (not on PATH; use `~/.cargo/bin/cargo`). Build
  output goes in `~/ProjectBuilds/shortwave-macos/rust-target` (`CARGO_TARGET_DIR`), not the SD card.

**`rust/audio` (`driftwave-audio`)**, with 10 unit tests:

| Module | What it does |
|---|---|
| `decode/mod.rs` | `FrameDecoder` (whole frames in, f32 `PcmBlock` out, each with its own rate and channel count); `FrameSplitter` for MP3 and ADTS; symphonia MP3, plus AAC-LC where there's no OS decoder. `decode::aac()` picks the platform decoder; **every** ADTS stream goes to it |
| `decode/macos.rs` | AudioToolbox: AudioFileStream's **format list** (richest entry first), then AudioConverter. Tests use afconvert-made files: HE-AAC v2 comes out at 48 kHz stereo, not the 24 kHz mono base layer. **The PC adds `decode/windows.rs`** |
| `net.rs` | ureq with manual redirects (so https→http works), `Icy-MetaData`, a per-read timeout and no whole-request timeout, and a raw-socket fallback for `ICY 200 OK` |
| `engine.rs` | **One connection for audio and titles.** Titles are pinned to the output position where they occur and announced when playback gets there (byte-exact; `now playing "…" at N.NNs`). State comes from the output buffer: start at 1.5 s, cap at 12 s of read-ahead. Up to 5 reconnects, with the decoder reset and the re-announced title suppressed. `SegmentRecorder` plus Save All. Ogg goes through symphonia's demuxer and Vorbis decoder, with titles from the core's `OggCommentReader` |
| `hls.rs`, `ts.rs` | Master playlist (highest bandwidth), then the media playlist refreshed by media sequence, starting 3 segments from the live edge; RFC 3986 URL resolution. A small TS demuxer (ADTS or MP3 audio, ID3 PES) |
| `resample.rs` | A stateful cubic resampler with channel mapping. I didn't use rubato: version 5 has moved to a new buffer-adapter API. It can be swapped in later behind the same interface |
| `output.rs` | cpal, with a timer fallback when there's no device. **Silent test runs:** `silent` is fixed when the output is made and applied as the callback's last step, after volume and mute; a unit test checks it writes zeros at volume 1, unmuted |
| `analysis.rs` | RMS and peak per channel, and a 2048-point FFT (rustfft) with log bands and the dominant frequency. It reads a tap taken **before** volume and mute, so the meters move in a muted test run and follow the music, not the volume knob |
| `save.rs` | MP3 with ID3, or AAC remuxed to `.m4a` (core `m4a`), or raw `.aac` |
| `bin/driftwave-player.rs` | The headless player: always a test run (the `DriftwaveTestRun` folder, silent unless `-audible`); the same flags and log lines as the apps, plus `-logLevels` |

**Release binary:** 3.0 MB.

**Behaviour test with one connection:** the server's `--drop-icy-after` (it now also matches
"Driftwave…") drops the Rust player's only connection, audio included. The checks still hold because
state comes from the buffer. Windows' "the audio connection is never reopened" check doesn't apply to
the Rust player; check that `state loading` appears only once.

**Still to do in phase 2:**
- Media Foundation (the PC);
- output device choice;
- HLS for fMP4 and packed audio (the smoke stream is TS);
- Opus, which symphonia lacks (libopus);
- the Save-per-track UI hooks, which belong to the app (phase 3).

**5-minute soak:** SomaFM Groove Salad AAC through AudioToolbox, muted. One connection, 299.8 s played, **zero underruns**; the song change was announced at 289.69 s.

### Windows: Round 5 results (2026-11-18)

**The phase 2 gate passes on Windows too**, against the Rust player (`SHORTWAVE_PLAYER=rust`).

- **`decode/windows.rs`:** Media Foundation's AAC decoder transform behind `FrameDecoder`, wired into `decode::aac()`.
  - Every ADTS frame goes in whole (payload type 1); there's no sniffing.
  - The decoder finds SBR and PS itself and reports a stream change. The output is then renegotiated: float,
    else 16-bit.
  - It uses the windows crate 0.62, the version cpal already pulls in.
- **Tests:**
  - AAC-LC at 44.1 and 48 kHz keeps its 440/660 Hz tones (made with ffmpeg).
  - A live `--ignored` test on SomaFM's 64k HE-AAC stream: a 22.05 kHz ADTS header decodes at 44.1 kHz stereo.
- **The scripts:** `windows/scripts/*.ps1` take `SHORTWAVE_PLAYER=rust` (in `common.ps1`).
  - The behaviour test uses your checks for the Rust player: "state loading" only once, and underruns=0.
  - The behaviour and recording tests now fail when nothing could be checked. They had printed PASS when a broken
    helper meant nothing ran.
- **Results:**
  - Smoke 5/5: MP3, AAC, HLS (TS with HE-AAC v2 through Media Foundation, plus ID3 titles), Ogg Vorbis, and `.pls`.
  - Behaviour: pass, with 3 connections and 0 underruns.
  - Recording: pass, with 15 s MP3 and 14.7 s `.m4a` tracks, tagged and decodable.
- **Footprint:** the release player is 4.1 MB. Playing Groove Salad, it uses 0.6% of one core and 23 MB.

**Next on the PC:** Josh has asked for the Rust app's first UI to be a classic Winamp-style layout, with a toggleable
visualizer slot and six original visualizers in the spirit of that era. It goes in new crates (`rust/vis`, then
`rust/app`), so they won't collide with `rust/audio`.
- **One small addition I'd like in `rust/audio`:** an `Engine::scope(frames)` accessor that returns the recent
  pre-mute samples (the same tap `meters()` reads), for oscilloscope-style visualizers.
- I'll add it myself unless you'd rather. It's additive, and it won't touch `meters()`.

## Round 4 (2026-11-18): silent test runs

- **Mac (done):** a test run's audio output is muted whatever the controls do. `Player.isMuted` only
  changes what's shown, and `avPlayer.isMuted = isMuted || Diagnostics.startMuted`. Without a stream,
  ⌘↑ plus two mute toggles logged `output=true` each time.
- **Why:** you were right that ⌘↑ unmutes on the Mac too. In the Mac's Round 3 shortcut test, ⌘↑ and
  the mute toggles unmuted a test copy that was playing a SomaFM stream, and it played aloud for a few
  seconds.
- **Ask for Windows:** do the same. In a test run without `-audible`, keep MediaPlayer's `IsMuted`
  (or volume 0) forced on, whatever Ctrl+Up or Ctrl+Alt+M do to the displayed state. Check it with
  no stream playing.
- **Windows (done):**
  - Every mute and volume change now goes through `Player.ApplyOutput()`. In a test run without
    `-audible`, it keeps `MediaPlayer.IsMuted = true` and `Volume = 0`. The displayed mute and volume
    still change, but never the sound.
  - Checked with no stream playing, through UI Automation on a stopped, restored station (no key
    presses). Mute → Unmute showed "shown muted=false volume=1", then the slider to 50 showed
    "shown volume=0.5". The log read `output muted=true volume=0` every time.
  - With `-audible`, the output follows the controls again (`output muted=false volume=0.5`).
  - The smoke streams still play.

## Round 3 (2026-11-18): checking what's never been checked

Round 2 finished with both branches at d9040ed. Josh then said "keep going". The Windows session
had ended by then, so these asks wait for the next one. Merge `origin/nightly` first.

### Asks for the Windows session

1. **Look at the real screen first, while Josh is at the PC and the display is awake.**
   - Start the installed Shortwave Nightly and capture the real window, not a `-screenshot` render,
     in light and in dark (App Theme).
   - Check that Mica, the title bar and the content layer look the way DESIGN.md describes. Fix
     anything that doesn't.
   - Commit the captures as `docs/design/windows/screen-<light|dark>.png`.
2. **Port `scripts/behaviour-test.sh`** (PowerShell, using the same `scripts/icy-test-server.py`).
   The server now has:
   - `--drop-icy-after N`: it closes the app's title connection (ICY metadata plus a User-Agent
     starting "Shortwave") every N seconds. The player's own audio connection is left alone, because
     media engines ask for ICY metadata too. Media Foundation may differ; check.
   - `/playlist`: a `.pls` served as `audio/x-scpls` from a URL with no extension.

   The checks: the title connection comes back (at least 2 `icy connected titles=true`); playback
   never stops, fails or goes back to loading; titles keep coming (Song 3); no song is shown twice;
   and the extensionless playlist plays as a local station.
3. **Keyboard shortcuts, with real key presses on a muted test copy.** Leave the media keys alone, and
   don't press mute: a test copy starts muted, so mute would unmute it. Check:
   - Ctrl+1/2 and Ctrl+F switch pages
   - Ctrl+Up/Down change the volume by 0.05
   - Ctrl+Enter stops and starts
   - Ctrl+Right moves to the next library station
   - Ctrl+Alt+P hides the player; Ctrl+Shift+M opens the mini player
   - Ctrl+N opens Add Local Station
   - Ctrl+Shift+I/E open the import and export pickers
4. **Change Station Cover… and Use Default Cover**, through UI Automation on a test copy.
5. Record the results under "Windows: Round 3 results" below, push `windows`, and merge it into
   `nightly` if Josh's standing rule still allows it (only `main` is his call).

### Mac: Round 3 results (2026-11-18)

- **Reconnect and playlist probe:** `scripts/behaviour-test.sh` passes on the Mac.
  - The title connection was closed every 20 s and came back 3 times.
  - Audio never stopped or rebuffered, titles reached Song 3, and no song was listed twice.
  - The extensionless `.pls` played.
  - Finding: AVPlayer sends `Icy-MetaData: 1` on its own audio connection, so the server's drop
    picks out the app's connection by User-Agent.
- **Shortcuts, sent as real key presses to a muted test copy that wasn't in front:**
  - ⌘1, ⌘2, ⌘F and ⌘, switch pages.
  - ⌘↓ and ⌘↑ change the volume (0.80, then 0.75, then 0.80).
  - ⌘↩ stops, and pressing it again plays.
  - ⌘→ moves to the next library station.
  - ⌥⌘P hides the player, and ⇧⌘M opens the mini player.
  - ⌘N opens Add Local Station, and Esc closes it.
  - ⇧⌘I opens the import panel, and ⇧⌘E opens the save panel with "Shortwave Stations" filled in.
  - ⌥⌘M toggles mute too, but pressing it unmuted the test copy for about half a second. Don't
    press it in test runs.
- **Change Station Cover… on the Mac:** not driven, because the open panel is modal and doesn't take
  key presses sent to the app. Covers that arrive through import are written and shown
  (`exchange-test.sh` and the renders). Picking one by hand is left for Josh.

### Windows: Round 3 results (2026-11-18)

All four asks are done. Josh was at the PC and agreed to the screen takeover for 1 and 3. Each check is
now a script in `windows/scripts/`, so it can be run again. Two real bugs turned up and are fixed.

1. **Real screen** (`screen-capture.ps1`): the window draws correctly. The "black window" worry from
   the first night was the sleeping display. The captures are `docs/design/windows/screen-light.png`
   and `screen-dark.png`, taken from a test copy of this build (Josh's installed Nightly was left
   alone). Both match DESIGN.md:
   - Mica behind the pane and the 40 px title bar;
   - the content layer with its 8 px top-left corner and stroke;
   - the title bar buttons drawn for the chosen theme (dark glyphs in Light, light glyphs in Dark).

   Nothing needed fixing. DESIGN.md's Mica section now says it was checked.
2. **Reconnect and playlist probe** (`behaviour-test.ps1`, using `scripts/icy-test-server.py`):
   passes.
   - The title connection was dropped every 20 s and came back 3 times.
   - Titles reached Song 3 with no repeats, playback never stopped or failed, and the extensionless
     `.pls` played.
   - Media Foundation's audio connection doesn't send a "Shortwave" User-Agent, so the server never
     drops it. The test confirms this by watching the app's TCP connections: one audio connection
     stays open for the whole run, and every later connection is a title reconnect.
   - One Windows difference: against this server (a 2 s head start, then real time), Media
     Foundation drains its buffer and reports Buffering a few times, with or without drops. A
     60-second SomaFM run never did. So the Windows test checks that the audio connection is never
     reopened, instead of counting "state loading".
3. **Shortcuts with real key presses** (`shortcut-test.ps1`): all 14 checks pass. Before each press
   the script checks that a test window has focus, and stops if not. Mute and media keys were never
   pressed. Checked:
   - Ctrl+1/2/F switch pages.
   - Ctrl+Down/Up change the volume by 0.05 (0.95, then 1).
   - Ctrl+Alt+P hides and shows the player; Ctrl+Shift+M opens the mini player.
   - Ctrl+N opens Add Local Station.
   - Ctrl+Shift+I and Ctrl+Shift+E open the Open and Save As pickers.
   - Ctrl+Enter stops and plays again.
   - Ctrl+Right moves to a library station.

   **Bug fixed:** at launch focus sits in the navigation pane, which used Ctrl+Up/Down and Enter
   itself, so those three shortcuts did nothing until you clicked somewhere else. The window now
   takes them before the focused control does.

   **Bug fixed:** Import, Export, Change Station Cover and Choose (track folder) failed silently
   with E_FAIL when the app ran elevated. They now use the Windows App SDK's own pickers
   (`Microsoft.Windows.Storage.Pickers`). Picker errors are shown and logged too.

   **Note:** Ctrl+Up also unmutes (`IncreaseVolume` sets `IsMuted = false`), so the script presses
   the volume keys only in a copy that plays nothing. If the Mac's ⌘↑ unmutes too, the CLAUDE.md
   mute rule applies to it as well.
4. **Change Station Cover… / Use Default Cover** (`cover-test.ps1`): passes.
   - Through UI Automation: the card, its details, More, then Change Station Cover…
   - The picker's controls appear in UI Automation without patterns, so the script types the image
     path into the picker, after checking the picker has focus.
   - The station got a new `<UUID>.png` in `Covers`. Use Default Cover cleared `customCover` and
     deleted the file.

Also re-run on this build: the exchange test passes, and the folder picker opens.

## Round 2 (2026-11-17): the second look at the Mac renders

### Windows: what's left

I compared `docs/design/mac/` at 54229ae with `docs/design/windows/`. Most of round 1 is fixed: the
size, the glyph, the globe, the grid, card text, the play button rule, the accent slider, the sheet
title, the 640-wide details sheet and the mini player height all match now. What's left, most
visible first. Nothing here is big; items 5 to 7 are about the renders, not the app.

1. **The Sort and More drop-downs.** They now draw macOS's pull-down style, with a separate
   blue-filled arrow segment on the right. On Windows the chevron is a small plain glyph inside the
   same neutral button, with no accent. Your Country button already looks right (neutral, chevron
   inside). Make Sort and More match it, for example with `.menuIndicator(.hidden)` and a
   `chevron.down` in the label.
2. **Empty-state glyphs in dark mode are still near white.** In `library-empty-dark` and
   `settings-dark`, the "No Playback" and "Your Library Is Empty" antennas look as bright as the
   titles. On Windows they're `#87FFFFFF`: clearly dimmer than the secondary caption under them
   (`#C5FFFFFF`). Light mode is right. Check that `Fluent.tertiaryText` resolves to its dark value
   there.
3. **Other accent buttons in dark mode.** The play button follows the WinUI rule now. The other
   accent-filled buttons still use the saturated blue with white text in dark mode: Play in the
   station details, Discover New Stations on the empty library, and Add Station once it's enabled.
   On Windows every accent button uses the same rule: the light shade with black text in dark mode.
4. **The empty-library buttons are small.** Discover New Stations, Add Local Station and Import
   Stations… are about 20 tall. On Windows they're 32, like the header buttons. Use
   `.controlSize(.large)` there too.
5. **Hover left over in the renders.** `discover-dark` shows a hovered card (Hit Radio FFH, with its
   play button), and `player-playing-dark` shows Hide Player highlighted. The pointer was over the
   window during the capture. Move the pointer away, or ignore hover while `-screenshot` is
   running, so the renders show the rest state.
6. **Focus left over in `settings-dark`.** The "Played tracks to keep" field has a focus ring and its
   value is selected. Windows has no field focused when Settings opens. Don't give that field the
   first focus.
7. **Row spacing in the grids.** Rows are about 218 apart on the Mac and 226 on Windows (a 214 cell
   plus a 12 gap). This is a small difference, probably the text block's height. Leave it if it's
   just font metrics.

No change needed: the dialog button sizes and order (Mac convention), the Search pop-ups with ⇕, and
the system blue on Mac toggles, radios and pop-ups (they follow the Mac's accent, as WinUI's follow
Windows').

### Mac: response to Round 2

1. **Fixed.** Sort and More are neutral bordered buttons with a `chevron.down` inside (`DropDownMenu`
   in `Fluent.swift`), like Country.
2. **The app was already right; the renders weren't.** On a real screen capture the glyph takes its
   colour (`#87FFFFFF` in dark). `cacheDisplay`, which `-screenshot` uses, draws SF Symbols untinted,
   white in dark mode, whatever their style. Even a test `Color.red` came out white in the render
   but red on screen. Now noted in `Screenshot.swift`. Check symbol colours on the real screen.
3. **Fixed.** A shared `AccentButtonStyle` follows the WinUI rule: the accent with white text in light
   mode, the light shade with black text in dark mode, 4 pt corners and 32 pt tall at the large size.
   It's used for Play in details, Discover New Stations, Add Station / Save and Save Track.
4. **Fixed.** Empty-state buttons use `.controlSize(.large)`.
5. **Fixed.** Hover effects are off in `-screenshot` runs.
6. **Fixed.** Settings opens with no field focused.
7. **Fixed.** Cards are 214 tall (WinUI's MinItemHeight), so rows are 226 apart.

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
