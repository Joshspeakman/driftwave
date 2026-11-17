# Shortwave for macOS

A native macOS internet radio player modelled on GNOME's
[Shortwave](https://gitlab.gnome.org/World/Shortwave). It's a new SwiftUI and AVFoundation app, not a port of the GTK code.
Station data comes from the community database at [radio-browser.info](https://www.radio-browser.info),
which lists over 50,000 stations.

## Features

| Shortwave (GNOME) | This app |
|---|---|
| Library of saved stations, with sorting by name, language, country, state, codec, votes or bitrate | Same. Toolbar sort menu and View > Sort Library By |
| Browse page: popular stations near you and random stations | Discover page: popular in your country (country menu to change it), trending worldwide, random |
| Search | Search by name, filtered by country, language and tag, and sorted by votes, clicks, name, bitrate or random |
| Station details: tags, location map, codec, bitrate, stream URL, homepage | Same, with a MapKit map. Orphaned and local stations are labelled |
| Add local station, change station cover | Same (File > Add Local Station, ⌘N). `.pls` and `.m3u` links are followed |
| Song titles from stream metadata | ICY titles (Shoutcast and Icecast), HLS ID3 tags and Ogg comments |
| Track recording: Save All / Decide for Each Track / Record Nothing; min/max duration; track folder | Same. MP3 is saved with ID3 tags, AAC is saved as tagged `.m4a` (no re-encoding) |
| Played tracks list with save buttons | Same, in the player sidebar. Click a track for details, save, cancel recording or play |
| Background playback, notifications on new track | Same |
| Gadget mode (compact player) | Mini Player window (⇧⌘M) that floats above other windows, plus a menu bar player |
| MPRIS media controls | macOS Now Playing and media keys. Next/previous keys step through the library |
| Google Cast | AirPlay picker in the player sidebar |

### Mac-specific features

- A menu bar extra to play/stop and switch stations
- Keyboard shortcuts:

  | Keys | Action |
  |---|---|
  | ⌘↩ | Play/stop |
  | ⌘↑ / ⌘↓ | Volume up/down |
  | ⌥⌘M | Mute |
  | ⌘← / ⌘→ | Previous/next library station |
  | ⌘1 / ⌘2 | Library / Discover |
  | ⌘F | Search |
  | ⌥⌘P | Show/hide the player |

### Limits

- Only MP3 and AAC (Shoutcast/Icecast) streams can be recorded. HLS and Ogg/Opus streams play and show titles, but they aren't recorded.
- Recording, like Shortwave's, relies on the station's metadata. The first song after tuning in isn't recorded because it started before you joined.

## How it works

AVPlayer plays every stream format, including Ogg/Opus on current macOS. However, AVPlayer doesn't report Icecast titles on macOS. So for Shoutcast and Icecast streams the app opens a second connection with `Icy-MetaData: 1` (`Audio/IcyStream.swift`). That connection reads the titles and cuts the raw audio into one file per track at the exact byte where the title changes (`ShortwaveCore/Recording.swift`). The title shown on screen waits for AVPlayer's buffer, so it changes when you actually hear the new song.

## Build and install

You only need the Xcode Command Line Tools (Swift 6), not Xcode.

| Command | What it does |
|---|---|
| `scripts/test.sh` | Runs the core unit tests (`swift test`, plus the flag that finds the test macros with only the Command Line Tools) |
| `scripts/build-app.sh --nightly` | Builds `~/ProjectBuilds/shortwave-macos/app/Shortwave Nightly.app` |
| `scripts/install.sh --nightly` | Builds the app and installs it to `/Applications` |
| `scripts/smoke-test.sh` | Plays real MP3, AAC, HLS, Ogg and `.pls` streams muted and checks that playback and titles work |
| `scripts/recording-test.sh` | Runs a local Icecast-style server (`scripts/icy-test-server.py`, needs ffmpeg) and checks that MP3 and AAC tracks are cut at each title change, saved, tagged and playable |

Build output goes to `~/ProjectBuilds/shortwave-macos`, and `.build` is a symlink to it.

Test runs use their own data folder (`ShortwaveTestRun`) and their own preferences. They also stay off Now Playing and the media keys, so they never disturb the installed app. A run counts as a test run when it gets any launch flag from `Sources/Shortwave/Model/Diagnostics.swift`, such as `-muted` or `-playURL`.

### Toolchain notes

- In the macOS 27 SDK, SwiftUI's `@State` is a macro whose compiler plugin ships only with Xcode. The code therefore uses `@ViewState`, an alias for the same property-wrapper type.
- With the Swift 6.4 Command Line Tools, a bare `swift test` can't find the swift-testing macro plugin. `scripts/test.sh` passes its path.

## Windows

The Windows version (C# and WinUI 3, on the `windows` branch) is in [`windows/`](windows/README.md):
same features, same `library.json`, same test flags.

## Data

| Data | Location |
|---|---|
| Library | `~/Library/Application Support/Shortwave/library.json` |
| Custom covers | `~/Library/Application Support/Shortwave/Covers/` |
| Favicon cache | `~/Library/Caches/Shortwave/Favicons/` |
| Temporary recordings | `~/Library/Caches/Shortwave/Recording/` (cleared at launch) |
| Saved tracks (default) | `~/Music/Shortwave` |

## Credits

Inspired by Shortwave by Felix Häcker and contributors (GPL-3.0-or-later). This app doesn't contain Shortwave's code or artwork.
