# Windows port: decisions and evidence

Josh asked for this port to be built autonomously while he was away (2026-11-17), so the "ask Josh"
gates in `PLAN.md` were decided here instead. Each decision is reversible. Overrule any of them and
the next session will change it.

## 1. Language and UI toolkit: C# + WinUI 3 (Windows App SDK 2.6.4, unpackaged)

The plan listed this first. Before committing to it I checked, in a time-boxed hello-world, that it
works without Visual Studio:

- `dotnet build` alone builds it (.NET SDK 10.0.403, no workloads).
- It runs **unpackaged** (`WindowsPackageType=None`, `WindowsAppSDKSelfContained=true`), so it needs no
  MSIX install or certificate.
- A GUI exe's `Console.Error` reaches a redirected stderr, so the test scripts can grep the
  `SHORTWAVE_LOG` lines just as they do on the Mac.

## 2. Playback engine: Media Foundation through `Windows.Media.Playback.MediaPlayer`

This is built into Windows: no native DLLs to ship. Phase 0 tested it in a console spike, muted
(`IsMuted`, volume 0) and with `CommandManager.IsEnabled = false` so that it stayed off the system media
session. Before the run, a read-only query of the system media sessions found nothing playing. During
the run, the spike didn't show up as a session either.

| Smoke stream | Plays (position advancing) | ICY connection | Titles |
|---|---|---|---|
| mp3-icy `ice1.somafm.com/groovesalad-128-mp3` | yes | `audio/mpeg`, metaint 45000 | yes |
| aac-icy `ice1.somafm.com/groovesalad-128-aac` | yes | `audio/aac`, metaint 45000 | yes |
| hls (RTL `.m3u8`) | yes | n/a | yes: ID3 `TIT2`/`TPE1` arrive as timed-metadata data cues |
| ogg-vorbis `dancewave.online/dance.ogg` | yes | none (Ogg) | the app reads Vorbis/Opus comments itself |
| pls-playlist `somafm.com/groovesalad.pls` | yes (after resolving) | metaint 45000 | yes |

Findings that shaped the code:

- **Ogg depends on a Windows component.** Media Foundation plays Ogg Vorbis through the *Web Media
  Extensions* package (`Microsoft.WebMediaExtensions` 2.1.51.0 here). It ships with Windows 11 but can
  be removed. Without it, Ogg stations fail with the normal "stream could not be played" error.
- **There is no buffer lead.** `GetBufferedRanges()` is always empty for live streams, so the Mac's
  "buffered end minus position" trick isn't available. Instead the ICY connection counts audio time by
  walking the MP3/ADTS frames (`AudioClock`). A title found *x* seconds into that connection's audio is
  shown when the player reaches the same point:
  `delay = x + (icyConnectedAt - playerConnectedAt) - playerPosition`, clamped 0 to 20 s, like the Mac.
  SomaFM sends about 16 s of audio as a burst on connect, so this delay really matters there.
- **https→http redirects.** `dance.ogg` redirects from https to http. .NET's `HttpClient` refuses to follow
  that downgrade (Media Foundation follows it), so the app follows redirects itself (up to 5).
- **Media Foundation never reports Ogg comments**, so for Ogg streams the second connection parses the
  Ogg pages for Vorbis/Opus comment headers (`OggCommentReader`). Icecast starts a new chained stream at
  each song change.

## 3. AAC to `.m4a` without re-encoding: a small managed MP4 muxer (`M4A.cs`)

Media Foundation's transcoder can't be told "passthrough" reliably, and shipping ffmpeg would add
about 100 MB. ADTS-to-MP4 is a plain container rewrite: every AAC frame is copied byte for byte, and
an `esds` box built from the ADTS header is added, plus iTunes tags (`©nam`, `©ART`, `©alb`). Recordings
are cut at arbitrary bytes, so the muxer locks on to two consecutive frames and drops a partial last frame.

Verified by `M4ATests.RealAacPassthroughWithFfmpeg`, which runs whenever ffmpeg is installed:

1. ffmpeg encodes real AAC.
2. The test cuts it at odd byte offsets and muxes it.
3. ffprobe reads codec, rate, channels and tags.
4. ffmpeg decodes the file with `-xerror`.
5. ffmpeg copies the frames back out with `-c copy -f adts`, and every frame payload is compared byte
   for byte with the source.

If muxing fails, the raw `.aac` is saved instead, as on the Mac.

## 4. Smaller calls (the plan said "ask Josh")

| Question | Choice | Why |
|---|---|---|
| Map in station details | Coordinates plus an "Open Map" link to OpenStreetMap | WinUI's MapControl needs an Azure Maps key. The plan allows "any map or a map link" |
| AirPlay counterpart | Output device picker (Windows audio endpoints), next to the volume | Cheap with `MediaPlayer.AudioDevice`. Default is "System default" |
| Install method | Per-user install to `%LOCALAPPDATA%\Programs\<name>` with a Start menu shortcut and an uninstall script; no admin, no MSIX, no certificate | Easiest to undo. Release (`Shortwave`) and Nightly (`Shortwave Nightly`) install side by side |
| Settings window | A Settings page in the main window's navigation (Windows convention) | |
| Library.json fixture | Written by hand in the Mac's exact format | The repo has no captured Mac file. **Still worth checking with a real one from the Mac** |
