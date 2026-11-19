# Driftwave: notes for Claude sessions

Josh's internet radio app, inspired by GNOME Shortwave (no code taken). **Driftwave is one app, written in Rust,
for Windows, macOS and Linux.** There is no other version: Josh retired the Swift (Mac) and C# (Windows) apps on
2026-11-18 ("There should be no other version at all. Period."). Their code is gone from this branch; git history
has it. Don't propose bridges, hybrids or a second program.

## The app

- **One window, two looks:**
  - **Full:** library, discover, search, settings and themes; the rebuild of Driftwave's familiar look.
  - **Classic:** an homage to the stacked Winamp players, which Josh treats like the mini player.
  - You swap in and out of Classic in the same window. The same engine keeps playing.
- `rust/` is the Cargo workspace. Run cargo through `rust/cargo.ps1` on the PC (the toolchain lives in
  `D:\DevCaches`); on the Mac run `cargo` directly.

| Crate | What it is |
|---|---|
| `rust/core` (driftwave-core) | Stations, library, exchange, ICY, playlists, radio-browser, ID3/M4A, recording, themes, the WCAG flash check |
| `rust/audio` (driftwave-audio) | The engine: HTTP/ICY/HLS, decoders (Media Foundation and AudioToolbox for AAC), output, recording, meters, scope tap; `driftwave-player` (headless, for tests) |
| `rust/vis` (driftwave-vis) | Visualizers: analysis, six originals, the flash guard at the output stage |
| `rust/app` (driftwave) | The app: Slint UI (software renderer), layouts, palettes |
| `rust/platform` (driftwave-platform) | OS integration: Now Playing and media keys (souvlaki), notifications, single instance, the menu bar / tray icon |
| `rust/vendor/softbuffer` | softbuffer 0.4.8 with a faster macOS backend (persistent canvas + sRGB IOSurfaces, and a see-through mode for the scene layers), via `[patch.crates-io]` |

- **Plan and progress:** `docs/RUST-SCOPE.md`. The rebuild order is: Full layout and the Classic swap; finding
  stations; settings and themes with their effects; OS integration (tray, media keys, notifications, single
  instance); the switch-over (installer, updater, migration).
- **Shared assets:**
  - `assets/theme-art/`: photos, credited in `CREDITS.txt`;
  - `assets/fonts/`: OFL fonts;
  - `assets/icons/`;
  - `themes/`: built-in themes;
  - `tools/`: Python generators for themes, art and the logo.

## Branches and sessions

- `rust` is the working branch. Two sessions work on it, one on the PC and one on the Mac. Fetch and merge
  `origin/rust` before every push, and stay in your own files where you can.
- They coordinate through `docs/MAC-WINDOWS-SYNC.md`. Read it first, take the newest round's asks for your
  side, and write your results there.
- `main` changes only when Josh says so.
- The GitHub repo is private. Push with the `Joshspeakman` account (on the Mac:
  `GH_TOKEN=$(gh auth token --user Joshspeakman)` per command).
- **The public repo `Joshspeakman/driftwave`** (README, site, wiki, releases) belongs to the public Driftwave.
  Deleting or changing what's published there needs Josh's say-so.

## Working with Josh

- Ask decisions in the question dialog, one question at a time, never as a wall of text.
- Finish the work end to end: fix what you find in the same pass, and don't leave "next steps".
- **Lightweight is a requirement, not a nicety.** It has to run "on a potato".
  - Measure CPU in every state (in front, behind, shaded, stopped) before calling anything light.
  - Cost must not grow with the window or the display: use pixel budgets, and let the UI scale them up.
- **Nothing ever flashes, flickers or strobes.** Visualizers go through the guard, and themes use still textures
  and smooth motion.
- **Nothing looks pixelated** except themes Josh asked to be pixel art. Draw in vectors, and render art
  supersampled.

## Testing rules

- **Test runs are muted and isolated.**
  - Any test flag (`-muted`, `-playURL`, `-library`, ...) or `SHORTWAVE_TEST=1` makes a test run.
  - A test run uses its own data folder (`DriftwaveRustTestRun` for the app, `DriftwaveTestRun` for the player),
    never saves settings, and stays silent unless `-audible`, whatever its controls do.
  - **Why:** a test once played a tone aloud, and another grabbed Josh's media keys while he was playing music.
- Never test media keys, Now Playing or the system media overlay while Josh is playing music. Ask him first.
- Never touch the real app's library, settings or track folder from a test.
- **Playback tests:**
  - On the Mac: `scripts/*.sh`. `scripts/package-mac.sh [--nightly]` builds `Driftwave.app` (ad-hoc signed) in
    `~/ProjectBuilds/shortwave-macos/app`.
  - **Launch tests** run the app with what only a real launch starts: `scripts/launch-test.sh` on the Mac (menu bar,
    scene layers; `--media` adds Now Playing, and refuses while anything holds the audio output),
    `scripts/launch-test.ps1` on the PC.
  - On the PC: `scripts/*.ps1` (smoke, behaviour, recording).
  - Both drive `driftwave-player`. Build it with
    `cargo build --release -p driftwave-audio --bin driftwave-player`.
- **App checks:**
  - `-screenshot <file.bmp>` snapshots the window.
  - `-alwaysActive` and `-inactive` force the focus states for measuring.
  - `DRIFTWAVE_LOG=1` prints per-stage timings.
