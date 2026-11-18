# Shortwave for Windows

The Windows version of the Mac app: same features, same `library.json`, same test flags. It's a
C# / WinUI 3 rebuild (see [`DECISIONS.md`](DECISIONS.md) for why, and [`PLAN.md`](PLAN.md) for the
port plan). The core logic (`src/ShortwaveCore`) is a 1:1 translation of `Sources/ShortwaveCore`.

## Requirements

- Windows 10 1809 or later (built and tested on Windows 11), x64
- .NET 10 SDK to build. The installed app needs only the .NET 10 runtime.
- Ogg stations need the *Web Media Extensions* package, which ships with Windows 11
- For the tests: Python 3 and ffmpeg (`winget install Python.Python.3.13 Gyan.FFmpeg`)

## Build, test, install

Run from the repo root in PowerShell (`powershell -ExecutionPolicy Bypass -File ...` if scripts are blocked).

| Command | What it does |
|---|---|
| `windows\scripts\test.ps1` | Core unit tests: the 19 translated from Swift, plus Windows-only ones, and the station exchange tests that read both apps' exports (38 in total) |
| `windows\scripts\build.ps1 [-Flavor Nightly\|Release]` | Builds `windows\out\<Flavor>\Shortwave.exe` |
| `windows\scripts\install.ps1 [-Flavor Nightly\|Release]` | Builds and installs to `%LOCALAPPDATA%\Programs\Shortwave Nightly` (or `Shortwave`), adds a Start menu shortcut and a Settings > Apps entry. No admin needed. Nightly and Release install side by side |
| `uninstall.ps1` in the install folder, or Settings > Apps | Removes the program. Your library, settings and tracks stay |
| `windows\scripts\smoke-test.ps1` | Plays the five real smoke streams (MP3, AAC, HLS, Ogg, `.pls`) muted and checks playback and titles |
| `windows\scripts\recording-test.ps1 [-StatusLine icy]` | Local ICY server plus ffmpeg: checks that MP3 and AAC tracks are cut at each title change, saved, tagged and playable. `-StatusLine icy` serves `ICY 200 OK` like an old Shoutcast server |
| `windows\scripts\exchange-test.ps1 [-UpdateFixture]` | Imports `fixtures\exchange\seed.json` into an empty test-run library and exports it, imports that export again (nothing added), and imports the Mac's export. `-UpdateFixture` writes `fixtures\exchange\windows-export.json` |

The test scripts run `windows\out\Nightly\Shortwave.exe` unless `SHORTWAVE_APP` points elsewhere. They
write logs to `%TEMP%`, or to `SHORTWAVE_SCRATCH` if set.

## Test runs

The rules in the root `CLAUDE.md` apply. Any test flag or `SHORTWAVE_TEST=1` makes a test run. A test
run:

- uses `%APPDATA%\ShortwaveTestRun` and `%LOCALAPPDATA%\ShortwaveTestRun`, with their own settings;
- stays muted unless you pass `-audible`;
- stays off the media overlay and media keys;
- skips the single-instance check, so test runs can run side by side.
- saves tracks to `%APPDATA%\ShortwaveTestRun\Tracks` unless `-trackDirectory` says otherwise.

| Flag | Effect |
|---|---|
| `SHORTWAVE_LOG=1` (environment) | Log player events to stderr (`state playing`, `icy connected titles=true format=mp3`, `now playing "..."`, `track started ... idleIncomplete`, `saved ...`) |
| `SHORTWAVE_TEST=1` (environment) | Test run with no other effect |
| `-playURL <url>` | Play this stream at launch as a temporary local station |
| `-muted` / `-audible` | Muted (default in test runs) / allow sound in a test run |
| `-page library\|discover\|search\|settings`, `-searchText <text>` | Start on a page or with a search |
| `-openFirstResult`, `-openTrackDetail` | Open the first result's details / the first recording track's details |
| `-<settingKey> <value>` | Override a setting for this run, e.g. `-recordingMode everything`, `-recordingMinimumDuration 5`, `-trackDirectory <dir>` |
| `-miniPlayer`, `-addStation` | Open the mini player / the Add Local Station dialog |
| `-importStations <file>`, `-exportStations <file>` | Import a station file at launch (logs `imported added=N present=M unreadable=K covers=C`), then export the library (`exported N stations to <file>`) |
| `-appearance light\|dark` | Use that look for this run (not saved) |
| `-screenshot <png> [-screenshotDelay <s>]` | After the delay (default 4 s), render the main window and any open dialog to PNG, then quit |
| `-quitAfter <s>` | Quit after that many seconds |
| `-mediaControls` | Use the media overlay even in a test run. Only for checking the media session when nothing else is playing |

`Shortwave.exe --quit` asks the running copy of that flavor to quit. The install script uses it.

## Data

| Data | Location |
|---|---|
| Library (same format as the Mac's) | `%APPDATA%\Shortwave\library.json` |
| Settings | `%APPDATA%\Shortwave\settings.json` (Release), `settings-nightly.json` (Nightly) |
| Custom covers | `%APPDATA%\Shortwave\Covers\` |
| Favicon cache | `%LOCALAPPDATA%\Shortwave\Favicons\` |
| Temporary recordings | `%LOCALAPPDATA%\Shortwave\Recording\` (cleared at launch) |
| Saved tracks (default) | `Music\Shortwave` |

To move the library from the Mac, copy `~/Library/Application Support/Shortwave/library.json` (and
the `Covers` folder) into `%APPDATA%\Shortwave\`. Like the Mac's Release and Nightly builds, the two
Windows flavors share the library but keep separate settings.

## Mac features on Windows

| Mac | Windows |
|---|---|
| AVPlayer | Media Foundation (`Windows.Media.Playback.MediaPlayer`) |
| ICY titles on a second connection | Same. The connection also reads Ogg Vorbis/Opus comments, which Media Foundation doesn't report |
| AVPlayer buffered ranges for title timing | Media Foundation reports none for live streams. The title connection counts MP3/ADTS frames to know where in the stream a title changed, then shows it when the player gets there |
| HLS ID3 titles (AVPlayerItemMetadataOutput) | Timed-metadata data cues, ID3 `TIT2`/`TPE1` |
| AVAssetExportSession passthrough to `.m4a` | A small built-in MP4 muxer that copies the AAC frames unchanged |
| Now Playing and media keys | System Media Transport Controls (the volume/media overlay and keyboard media keys) |
| Notifications | Windows toast (Windows App SDK app notifications) |
| Menu bar extra | Notification-area icon with the same menu (left click opens the window) |
| Mini Player window | Always-on-top mini player |
| Settings window | Settings page in the navigation pane |
| MapKit map | Coordinates plus an "Open Map" link to OpenStreetMap |
| AirPlay picker | Audio output picker next to the volume |
| Closing the window | The app keeps running in the notification area. With Background Playback off, closing stops playback. Quit from the tray menu or with Ctrl+Q |

Shortcuts: play/stop Ctrl+Enter · volume Ctrl+Up/Down · mute Ctrl+Alt+M · previous/next library
station Ctrl+Left/Right · Library Ctrl+1 · Discover Ctrl+2 · Search Ctrl+F · show/hide player
Ctrl+Alt+P · add local station Ctrl+N · import stations Ctrl+Shift+I · export stations Ctrl+Shift+E · mini player Ctrl+Shift+M · quit Ctrl+Q. Space doesn't
play or stop playback.
