# Driftwave

Internet radio for Windows, macOS and Linux: find a station, hear what's on, and keep the songs you love. One app,
written in Rust. Inspired by GNOME's [Shortwave](https://gitlab.gnome.org/World/Shortwave), whose code it doesn't
contain. Station data comes from the community database at [radio-browser.info](https://www.radio-browser.info).

Driftwave is being rebuilt as a single Rust app, replacing the earlier Swift (Mac) and C# (Windows) apps, which are
retired. The plan and progress are in [docs/RUST-SCOPE.md](docs/RUST-SCOPE.md).

## One window, two looks

- **Full:**
  - library, Discover and Search;
  - station details;
  - the player with the songs it has played;
  - settings, 26 themes and your own GIF or picture backgrounds.
- **Classic:**
  - an homage to the stacked players of the Winamp years;
  - an LCD, a scrolling title and a visualizer slot with six original visualizers;
  - it scales by dragging its corner.
- **Swapping:** you move in and out of Classic like a mini player, and the music carries on.

## What it must do (parity with the retired apps)

| Area | Driftwave |
|---|---|
| Library | Saved stations with covers, sorted by name, language, country, state, codec, votes or bitrate; import and export (`docs/STATION-EXCHANGE.md`) |
| Discover | Popular stations in your country (with a country menu), trending worldwide, random |
| Search | By name, filtered by country, language and tag; sorted by votes, clicks, name, bitrate or random |
| Station details | Tags, location, codec, bitrate, stream URL, homepage; orphaned and local stations labelled |
| Local stations | Add your own stream; `.pls` and `.m3u` links are followed; change a station's cover |
| Now playing | ICY titles, HLS ID3 tags and Ogg comments, in time with what you hear |
| Recording | Save all, decide for each track, or record nothing; min/max duration; track folder. MP3 with ID3, AAC as tagged `.m4a`, copied exactly |
| Played tracks | The player's list of songs, each with save, cancel and details |
| Background playback | Tray / menu bar icon, notifications on a new song, one copy running |
| Media controls | The system's media overlay and Now Playing, and media keys |
| Themes | Default plus 26 built-ins and your own; photo scenes, rain, drips, neon and other effects, with easter eggs; never flashing |
| Backgrounds | Your own GIF or picture, from a file or GIPHY, flash-checked |
| Updates | In-app updates from GitHub Releases |

## Building

The workspace is `rust/`. Run cargo through `rust\cargo.ps1` on the PC, or plain `cargo` elsewhere.

```
cargo run --release -p driftwave-app          # the app (binary: driftwave)
cargo test --workspace                        # unit tests, including the visualizers' flash-safety test
cargo build --release -p driftwave-audio --bin driftwave-player   # the headless player for scripts/*
```

The playback tests drive the headless player against real streams and a local test server: `scripts/*.ps1` on
Windows, `scripts/*.sh` on the Mac. They need Python 3 and ffmpeg.

### A Windows release

`scripts\package-windows.ps1` builds the app and makes `dist\Driftwave-<version>-win-x64.zip` and its `.sha256`:
- the zip holds `Driftwave.exe` (with its icon and version, from the Windows SDK's `rc.exe`), the themes and their
  art, `Install.ps1`, `uninstall.ps1` and a README;
- `Install.ps1` installs for the current user (no admin rights) with a Start-menu shortcut carrying the app id
  `Joshspeakman.Driftwave` (which notifications need) and a Settings > Apps entry;
- published on a `v<version>` release of github.com/Joshspeakman/driftwave, the two files are what the in-app updater
  (Settings > Updates) finds, checks and installs.

Publishing a release is a separate, deliberate step. The first launch of the installed app brings the C# Driftwave's
settings, themes and background across from `%APPDATA%\Driftwave`, which it only reads.

## Layout of the repository

| Folder | What's in it |
|---|---|
| `rust/` | The app and its crates: `core`, `audio`, `vis`, `app` |
| `themes/` | The built-in themes (JSON) |
| `assets/` | Theme art (photos credited in `theme-art/CREDITS.txt`), fonts (SIL OFL), icons |
| `tools/` | Generators for the themes, their art and the logo (Python) |
| `fixtures/` | Shared test files (station exchange, design libraries) |
| `scripts/` | Playback tests and their local ICY test server |
| `docs/` | The scope and plan, design notes, the station exchange format, and the Mac–PC sync log |
