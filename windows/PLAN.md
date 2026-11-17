# Shortwave for Windows: the plan

Josh's ask (2026-11-17): bring the Mac Shortwave app to Windows. A Windows Claude session pulls
this repo and does the port. This file is that session's starting point. Read `CLAUDE.md` at the
repo root first; its testing rules apply here too.

The Mac app is native SwiftUI and AVFoundation, so none of its UI or audio code runs on Windows.
The Windows app is a rebuild with the same features and behaviour. Its core logic is a 1:1
translation of `Sources/ShortwaveCore`, and its core tests are a 1:1 translation of the Swift tests.

## Where things go

- Branch `windows`, made from `nightly`. All Windows code goes under `windows/`. Leave the Swift
  code alone. If the Mac side changes, merge `nightly` into `windows`.
- `main` changes only when Josh says so.
- The repo is private (`github.com/Joshspeakman/shortwave-macos`). The PC needs git access through
  the Joshspeakman GitHub account.

## Baseline

The Mac app is at `nightly` from the 2026-11-17 prep commit, which added this file. The last change
to the Mac code itself is ce01f9e.

- All 19 core tests pass (`scripts/test.sh` on the Mac).
- `scripts/smoke-test.sh` and `scripts/recording-test.sh` passed on 2026-11-10.
- Josh uses the installed Nightly app.
- **Not yet confirmed by Josh on the Mac:** Now Playing and media keys, notifications, and AirPlay.
  Treat the Mac app as the reference for what these should do, but not as proof that it does them
  correctly.

## Decisions to make first (ask Josh, one question per dialog)

1. **Language and UI toolkit.** Josh chose to let this session pick, so ask him when you start,
   with your recommendation first. The candidates offered on the Mac were:
   - **C# + WinUI 3:** the native Windows 11 look; built-in media controls are easy to reach;
     C# is the same language as his Dead West Railway port.
   - **C# + WPF:** very mature and simple to build and package, but it looks dated unless styled.
   - **C# + Avalonia:** cross-platform, but less native on Windows and with weaker media-key
     support.
2. **Playback engine.** Don't decide this by preference or from memory. Phase 0 decides it by
   testing. Report the result to Josh before building on it.

## The Mac app, file by file

| Mac file | What it does | Windows counterpart |
|---|---|---|
| `ShortwaveCore/Station.swift` | Station model, radio-browser JSON decoding, library sorting | Core library, translated 1:1 |
| `ShortwaveCore/RadioBrowser.swift` | radio-browser.info client: server discovery and failover, search, popular, trending, random, by-UUID, facets, click counting | Core, 1:1 |
| `ShortwaveCore/ICY.swift` | ICY stream parser, `StreamTitle` extraction, artist/title split, recordable format from Content-Type | Core, 1:1 |
| `ShortwaveCore/Recording.swift` | Recording modes and states; `SegmentRecorder`, which cuts one file per track | Core, 1:1 |
| `ShortwaveCore/ID3.swift` | ID3v2.3 tag writer for saved MP3s | Core, 1:1 |
| `ShortwaveCore/Playlist.swift` | `.pls`, `.m3u` and `.xspf` resolving; safe file names; unique file names | Core, 1:1 (forbidden characters are already the Windows set) |
| `ShortwaveCore/Storage.swift` | Data paths; `library.json` load and save | Core, with Windows folders (below) |
| `Shortwave/Audio/IcyStream.swift` | Second HTTP connection that reads ICY titles and feeds the recorder | App: an HTTP client streaming the body on a background thread |
| `Shortwave/Audio/Player.swift` | Playback, retries, title timing, tracks, notifications, Now Playing, media keys | App: the heart of the port (behaviour below) |
| `Shortwave/Model/AppModel.swift` | Library, Discover, Search state, covers, library refresh, next/previous station | App view model |
| `Shortwave/Model/Track.swift` | One detected song; saving (MP3 with ID3, AAC remuxed to `.m4a`) | App |
| `Shortwave/Model/Preferences.swift` | Settings, persisted (keys below) | App settings store |
| `Shortwave/Model/ImageStore.swift` | Favicon download, disk cache (SHA-256 of URL), resize to 256 px; custom covers at 512 px | App |
| `Shortwave/Model/Diagnostics.swift` | Test flags and logging | App, same flags (below) |
| `Shortwave/Views/*` | Library, Discover, Search pages; station cards; player panel; mini player; sheets; settings; menu-bar menu | App UI |

## Behaviour to keep (easy to miss when reading the Swift)

**Titles and recording**
- The ICY connection sends `Icy-MetaData: 1` and `User-Agent`. Without an `icy-metaint` header
  there are no titles: close that connection; playback carries on.
- `StreamTitle` values can contain apostrophes (`Guns N' Roses - Don't Cry`). The value ends at
  the first `';` that is followed by a letter, by NULs or spaces only, or by the end of the block.
- Metadata text is UTF-8, falling back to Latin-1. Trailing NULs are stripped.
- The **first title after tuning in is never recorded** (`idleIncomplete`), because that song began
  before you joined. A repeated identical title is not a new track.
- A cut happens at the exact byte where the title changed. Minimum duration (default 30 s) and
  maximum duration (default 900 s, stops recording but keeps the file) apply.
- If the ICY connection drops while audio still plays, it reconnects up to 5 times, 3 s apart.
  After a reconnect the same song is announced again as `idleIncomplete`. Don't add it twice.
- **Title timing:** the ICY connection runs ahead of what you hear by the player's buffer. The Mac
  app delays showing a new title by the buffered lead (buffered end minus playback position,
  clamped 0 to 20 s). Without this, titles change early. Find the equivalent in the chosen engine.
  If you instead feed the player from the same connection, timing comes from the player's position.
- HLS streams (`isHLS`, or URLs ending `.m3u8`) get no ICY connection. Their titles, and Ogg
  comments, come from the player's own metadata (ID3 `TIT2`/`TPE1`, title/artist keys). Those
  tracks show as "not recorded" (`idleUnsupported`, or `idleDisabled` when recording is off).
- Only MP3 and AAC (by Content-Type) can be recorded. Ogg, Opus and HLS play but aren't recorded.

**Saving tracks**
- MP3: the ID3v2.3 tag (title, artist, album = station name, comment "Recorded with Shortwave
  from <station>") followed by the raw stream bytes.
- AAC: the raw ADTS stream is remuxed **without re-encoding** into a tagged `.m4a`. If that fails,
  the raw `.aac` is copied. Find a passthrough remux on Windows and verify it. Re-encoding is not
  acceptable.
- File name is the sanitised track title, with ` (2)`, ` (3)` ... if the name is taken.
- "Save All Tracks" saves automatically. "Decide for Each Track" keeps tracks in the temporary
  folder until saved from the played-tracks list, or saves one track when it finishes if its
  "Save this track when recording is complete" box is ticked. "Record Nothing" doesn't record.
- The temporary recording folder is cleared at launch (test runs use a per-process subfolder).
- Played tracks list: newest first, length = setting (default 10, range 1 to 50). A track dropped
  off the list loses its temporary file unless it was saved or is still recording.

**Playback**
- A generation counter discards events from a station you already left.
- Stream failure: retry twice, 1.5 s apart, then show the error.
- Station links ending `.pls`, `.m3u`, `.xspf` (or with a playlist Content-Type) are followed, up to
  3 hops, reading at most 64 KB. Unknown content types are probed (first 4 KB) only for local
  stations; database stations are trusted to be streams.
- Each play of a database station calls radio-browser's click counter (`/json/url/<uuid>`).
- Volume steps are 0.05. Volume is saved. Mute isn't saved.
- Next and previous step through the library in its current sort order, wrapping around.
- With "Background playback" off, closing the main window stops playback. The app keeps running
  after its last window closes (it has a menu bar icon; on Windows, a notification-area icon).

**Station database**
- Discover servers from `https://all.api.radio-browser.info/json/servers`. Fall back to `de1`,
  `de2`, `fi1`. Move a server that answers to the front. Send a descriptive User-Agent (the Mac
  sends `ShortwaveMac/1.0`; use `ShortwaveWindows/1.0`).
- Popular: the country's top 100 by votes, drop anything over 50,000 votes (botted), shuffle, keep 12.
  Trending: `clicktrend`, top 18. Random: 18. Searches use `hidebroken=true`, limit 150, and wait
  350 ms after typing stops.
- Library refresh at launch: re-fetch saved database stations by UUID (50 per request). Mark ones
  the database no longer knows as orphaned. Keep the user's custom cover.

## Data

Keep `library.json` compatible with the Mac: same keys and types, so a library copied from one
machine loads on the other. Exact bytes needn't match: the Mac writes `\/` for `/` and `" : "`
between keys and values, and any JSON reader accepts both. Format: `{"stations": [...], "version": 1}`, pretty-printed with sorted keys. Station
keys are `id, name, streamURL, homepage, faviconURL, tags, country, countryCode, state, language,
codec, bitrate, votes, isHLS, latitude, longitude, isLocal, isOrphaned, customCover`. Optional
values are left out rather than written as null. Local stations have ids `local-<lowercase uuid>`.

| Data | Mac | Windows (suggested) |
|---|---|---|
| Library | `~/Library/Application Support/Shortwave/library.json` | `%APPDATA%\Shortwave\library.json` |
| Custom covers | `.../Application Support/Shortwave/Covers/` | `%APPDATA%\Shortwave\Covers\` |
| Favicon cache | `~/Library/Caches/Shortwave/Favicons/` | `%LOCALAPPDATA%\Shortwave\Favicons\` |
| Temporary recordings | `~/Library/Caches/Shortwave/Recording/` | `%LOCALAPPDATA%\Shortwave\Recording\` |
| Saved tracks | `~/Music/Shortwave` | `Music\Shortwave` (the Music known folder) |
| Test runs | `ShortwaveTestRun` folders + separate preferences | Same folder name, separate settings file |

**Settings** (key, default): `recordingMode` decide · `trackDirectory` Music\Shortwave ·
`recordingMinimumDuration` 30 · `recordingMaximumDuration` 900 · `backgroundPlayback` true ·
`notifications` false · `pastTracksCount` 10 · `librarySorting` name · `libraryAscending` true ·
`volume` 1.0 · `lastStation` (none) · `discoverCountry` (the system region, else US).
At launch the last station is shown, not played.

## Test flags

The Windows app must accept the same flags. A test run uses the isolation rules in `CLAUDE.md`.

| Flag | Effect |
|---|---|
| `SHORTWAVE_LOG=1` (environment) | Log player events to stderr (the scripts grep these lines, so keep the wording: `state playing`, `icy connected titles=true format=mp3`, `now playing "..."`, `track started ... idleIncomplete`, `saved ...`) |
| `SHORTWAVE_TEST=1` (environment) | Test run with no other effect |
| `-playURL <url>` | Play this stream at launch as a temporary local station |
| `-muted` / `-audible` | Muted (default in test runs) / allow sound in a test run |
| `-page library\|discover\|search`, `-searchText <text>` | Start on a page or with a search |
| `-openFirstResult`, `-openTrackDetail` | Open the first result's details / the first recording track's details |
| `-recordingMode`, `-recordingMinimumDuration`, `-trackDirectory` (each takes a value) | Override that setting for this run |

On the Mac, `-key value` pairs override settings through the system's launch-argument defaults,
so no code parses them. On Windows, parse them explicitly.

## Mac features and what each needs on Windows

Each candidate API below is unverified; check it before relying on it.

| Mac | Capability | Acceptance check on Windows |
|---|---|---|
| AVPlayer | Plays MP3, AAC, HLS, Ogg Vorbis/Opus, and `.pls` links resolved to those | All five smoke-test streams play (phase 0) |
| AVPlayer buffered ranges | Buffer lead for title timing | A new song's title appears when you hear it, not seconds early |
| AVPlayerItemMetadataOutput | HLS and Ogg titles | The HLS and Ogg smoke streams show titles, if they send any |
| URLSession data task | ICY connection, streamed body | MP3 and AAC smoke streams log `icy connected titles=true` |
| AVAssetExportSession passthrough | AAC to tagged `.m4a` without re-encoding | Saved `.m4a` plays, shows tags, same bitrate as the stream |
| MPNowPlayingInfoCenter + MPRemoteCommandCenter | Media overlay; play/pause/next/previous keys | Keyboard media keys and the Windows media overlay control the app; overlay shows song, artist, station, cover (candidate: System Media Transport Controls). **Test only when Josh isn't playing music** |
| UNUserNotificationCenter | Notification on a new song (off by default) | Toast with song, artist, station |
| MenuBarExtra | Play/stop and station switching from the menu bar | Notification-area icon with the same menu |
| Floating Mini Player window | Compact player that stays on top | Always-on-top compact window |
| Settings scene | General and Recording settings | Settings page or window with the same options |
| MapKit map in station details | Station location | Any map or a map link; ask Josh if unsure |
| AVRoutePickerView (AirPlay) | Pick an output device | Output device picker, or leave out (ask Josh) |
| NSOpenPanel | Choose track folder, choose cover image | File and folder pickers |
| `NSWorkspace.open` | Play a saved track, open homepage | Open with the default app or browser |
| Menu shortcuts (⌘) | Below | Same with Ctrl |

Shortcuts: play/stop ⌘↩ · volume ⌘↑/⌘↓ · mute ⌥⌘M · previous/next station ⌘←/⌘→ · Library ⌘1 ·
Discover ⌘2 · Search ⌘F · show/hide player ⌥⌘P · add local station ⌘N · Mini Player ⇧⌘M.
Space is deliberately not play/stop (it would fire while typing a search).

## Phases

0. **Playback spike (console app, no UI).** Prove the engine before anything else. Play each
   stream in `scripts/smoke-test.sh` muted, run the ICY connection beside it, and log the same
   lines the script greps for. Pass: all five play, and MP3, AAC and the `.pls` link show titles.
   Also check buffer lead and passthrough AAC remux. Report results to Josh and get his OK on
   the engine.
1. **Core library and tests.** Translate `ShortwaveCore` and all 19 tests in
   `Tests/ShortwaveTests/CoreTests.swift` 1:1: ICY parsing across chunk sizes, title parsing,
   Latin-1, artist/title, formats, PLS/M3U/relative/detection, file names, ID3 header, the five
   recorder cases, API decoding, sorting, library round trip. Add a test that loads a
   `library.json` written by the Mac app.
2. **Player and recording.** Player behaviour above, track saving, test flags, logging.
   Port `scripts/recording-test.sh` to PowerShell. `scripts/icy-test-server.py` runs as is
   (needs Python and ffmpeg). Use `ffprobe` in place of the Mac's `afinfo`. Pass: MP3 and AAC
   tracks cut at about the 15 s interval, saved, tagged and playable; first track marked incomplete.
3. **UI.** Library (sort menu, empty state), Discover (country menu, popular, trending, random,
   retry), Search (name plus country, language, tag and order filters), station details, add or edit
   local station, change cover, player panel with played tracks, track details, settings.
4. **Windows integration.** Media keys and overlay, notifications, notification-area icon, mini
   player, shortcuts, background playback. Port `scripts/smoke-test.sh` to PowerShell.
5. **Build and install.** Release and Nightly builds that install side by side
   (`Shortwave` and `Shortwave Nightly`), an icon (`tools/make-icon.swift` draws
   the Mac one), and a build-and-install script. Ask Josh how he wants it installed.

## Done

Every phase's checks pass. The muted smoke and recording tests pass. A `library.json` from the
Mac loads. Josh has played a station, saved a track, and tried the media keys himself.
