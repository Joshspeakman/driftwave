# Rebuilding Shortwave in Rust: scope

Josh asked for this on 2026-11-18. His goals:
1. **One codebase** for Mac and Windows, replacing the Swift and C# apps.
2. **A smaller, faster app.**
3. **Real audio data**, so the spectrum bars and VU meters follow the music.

This is a scope, not a build. The two riskiest parts, audio and the GUI toolkit, were tried in
throwaway spikes on Windows. The code is in `I:\Development\scratch\rust-spikes\` (`audio`,
`gui-slint`, `gui-tauri`). They were built with Rust 1.100. The machine's toolchain is in
`D:\DevCaches` (set `RUSTUP_HOME` and `CARGO_HOME` per command).

## Recommendation

**Go ahead, in phases, with a gate after the audio engine:**
- [Slint](https://slint.dev) for the UI;
- a pure-Rust audio pipeline: symphonia, cpal, our own ICY and HLS handling;
- FDK-AAC, or the operating system's own decoder, for HE-AAC;
- small crates for the system integration.

Keep the Swift and C# apps until the Rust app passes the same tests they do. Expect about **11–16
weeks** of focused work for one developer; the phases are below.

Tauri (a web UI) would make the themes easiest, but it fails goal 2. It used 2.3× the memory and twice
the CPU in the spike.

## What exists today

Line counts from the `windows` branch, 2026-11-18.

| Part | Windows (C#) | Mac (Swift) |
|---|---|---|
| Core logic: ICY, recorder, ID3, playlists, stations, library, exchange, radio-browser, M4A muxer, Ogg comments, frame clock | 1,744 | 942 |
| App: player, UI, integrations | 6,893 (+ 639 XAML), of which themes 2,050 | 3,021 |
| Unit tests | 702 (38 tests) | 320 |
| Test scripts | 745 PowerShell + 21 Python | 296 shell + 94 Python |

**What carries over unchanged:**
- the core logic's behaviour, its 38 tests and the fixtures;
- the `library.json`, station exchange and theme formats (`docs/STATION-EXCHANGE.md`,
  `docs/THEMES.md`);
- the test flags and the log lines.

If the Rust app keeps the same flags and log lines, the existing scripts can drive it: smoke,
recording, behaviour, exchange, shortcuts, covers and screenshots. That is the cheapest way to prove
parity.

## Spike 1: audio, all five smoke streams (muted)

A 300-line Rust program streamed each stream for 15 s. It used `ureq` over HTTP, its own ICY reader,
`.pls` resolution, and HLS with its own small MPEG-TS demuxer and timed-ID3 reader. It decoded with
`symphonia` 0.6 or `fdk-aac`, and ran a `cpal` output stream fed silence. It computed real left and
right levels from the samples.

| Stream | symphonia (pure Rust) | FDK-AAC |
|---|---|---|
| MP3 with ICY titles | ✅ 44.1 kHz stereo, titles, levels | — |
| AAC-LC with ICY | ✅ | ✅ |
| Ogg Vorbis (https→http redirect) | ✅ | — |
| `.pls` | ✅ | — |
| HLS: MPEG-TS, **HE-AAC v2**, ID3 titles | ⚠️ decodes as 24 kHz mono (base layer only; sounds dull) | ✅ 48 kHz stereo |

**Findings:**
- **HE-AAC needs more than pure Rust.** symphonia decodes only AAC-LC. Many stations listed as
  "AAC+" are HE-AAC, so we need FDK-AAC (C, built by cargo here without trouble), or the OS decoder:
  Media Foundation on Windows, AudioToolbox on the Mac.
  - FDK's licence allows shipping, but grants no patent licence.
  - The OS decoders avoid that, at the cost of about 200 lines of platform code each.
  - **This is Josh's call.**
- **Opus** wasn't tested; symphonia doesn't decode it. Opus stations would need `libopus` (C, small).
- **Real audio data comes for free.** We decode the samples, so the meters and spectrum bars can be
  real (an FFT with `rustfft`).
  - Title timing gets simpler too: the app knows exactly which sample is playing, so the frame-clock
    estimate (`AudioClock`) goes away.
- **Two things to build that the spike only sketched:**
  - pacing, buffering and resampling to the device rate (`rubato`);
  - HLS playlist refresh by media sequence (the spike re-fetched old segments).
- **Older servers:** `ICY 200 OK` responses need a raw-socket fallback, as the C# app has. `ureq`
  won't parse that status line.
- **Size:** the whole spike, TLS and decoders included, is a 6 MB exe.

## Spike 2: GUI toolkit (Retrowave look)

| | WinUI today | **Slint 1.19** | Tauri 2 |
|---|---|---|---|
| Binary | 158 MB folder (.NET + WinUI) | **11.7 MB** exe | 8.1 MB exe + the system WebView |
| Memory (working set) | 155–166 MB | **~153 MB** | 372 MB over 7 processes (26 MB of it ours) |
| CPU, Retrowave grid animating | 20% of a core | **14%** | 31% |
| Theme effects | XAML shapes, composition | Gradients, drop shadows, animations, paths and custom fonts all worked. No backdrop blur, and no text shadow (glow is done with copies, as in WinUI today) | Easiest: CSS gives `text-shadow` neon and real `backdrop-filter` blur for Frosted Glass |
| Snapshot for tests | `-screenshot` (RenderTargetBitmap) | `window.take_snapshot()` worked | needs a separate headless renderer |

**Slint fits the goals best:**
- one native codebase;
- small, with CPU no worse than today;
- the Retrowave window was reproduced in about 120 lines of `.slint`, bundled Orbitron font included.

What it lacks is real blur, so Frosted Glass would use translucency and drifting colour without a blur
behind it, or a custom blur shader.

**Slint's licence:** its royalty-free licence covers desktop apps, if the app shows a "Made with
Slint" attribution (for example in About). GPLv3 and commercial licences are the alternatives.

Not chosen:
- **egui:** immediate mode; the app wouldn't feel native.
- **iced:** still young for this much UI.
- **gtk4-rs:** heavy on Windows and the Mac. GNOME Shortwave itself is Rust and GTK4, but it's GPL,
  and this app deliberately contains none of its code.

## System integration (both platforms)

| Need | Crate | Notes |
|---|---|---|
| Tray / menu bar icon | `tray-icon`, `muda` (menus) | Used by Tauri; maintained |
| Media keys, Now Playing / media overlay | `souvlaki` | SMTC on Windows, MPNowPlayingInfoCenter on the Mac |
| Notifications | `winrt-notification` / `objc2-user-notifications` | Or `notify-rust` |
| File pickers | `rfd` | |
| Single instance, `--quit` | `single-instance` + a small IPC | |
| HTTP, TLS | `ureq` (blocking) or `reqwest` | The spike used `ureq`; it follows https→http redirects |
| Packaging | Windows: Inno Setup or WiX (`cargo-wix`). Mac: `.app` bundle, signing and notarisation | Same per-user install as today |

## Phases

| Phase | Work | Weeks | Done when |
|---|---|---|---|
| 0 | Spikes | done | This document |
| 1 | **Core in Rust:** port ShortwaveCore 1:1 with its 38 tests, plus stations, library, exchange and theme files | 1.5–2 | Same tests pass; reads `mac-export.json`, `windows-export.json` and the Mac `library.json` |
| 2 | **Audio engine plus a headless player:** HTTP, ICY, redirects, raw-socket ICY, HLS, decoders, resampling, output, volume and mute (test runs silent), output device, reconnect, recording (MP3 + ID3, AAC → M4A), title timing, levels/FFT | 3–4 | **Gate:** the existing smoke, recording and behaviour scripts pass against the Rust player, on Windows and the Mac |
| 3 | **Slint UI:** pages, dialogs, player pane, mini player, shortcuts, Default theme, theme engine (tokens → Slint globals), accessibility | 4–6 | Screenshot and shortcut scripts pass; parity checklist from `windows/DECISIONS.md` |
| 4 | **Integration and themes:** tray, media keys, notifications, pickers, single instance, the nine themes and their effects, real meters | 2–3 | Cover, exchange and theme checks; on-screen check by Josh |
| 5 | **Packaging and switch-over:** installers, Mac signing, Nightly/Release flavours, migrating settings and library | 1 | Josh uses it daily; the old apps retire |

**Total: about 11–16 weeks.** If phase 2 fails its gate on either platform, stop and keep today's apps.
Little would be lost beyond phases 1 and 2.

## Risks

| Risk | Effect | Mitigation |
|---|---|---|
| HE-AAC decoding (licence or patents) | Dull audio on many AAC+ stations | Josh decides FDK vs OS decoders before phase 2 |
| Our own audio engine: buffering, live-stream jitter, device changes | The part the OS did for us; most of the remaining risk | Phase 2 gate on real streams; reuse the behaviour test server |
| Slint maturity (accessibility, IME, platform look) | Rough edges in text input or screen readers | Check with Narrator and VoiceOver in phase 3 |
| No backdrop blur in Slint | Frosted Glass becomes "translucent glass" | Accept, or write a blur shader later |
| Two old apps to keep working meanwhile | Double maintenance for about 3 months | Freeze their features; fixes only |
| Mac work happens on the Mac | This PC can't build or test the Mac target | The Mac session runs the phase 2 and 4 checks |

## Decisions for Josh

1. **HE-AAC:** FDK-AAC (simplest, one codebase; no patent licence) or the OS decoders (about 200
   lines per platform).
2. **Slint licence:** royalty-free with a "Made with Slint" attribution in About? Or GPLv3?
3. **Linux:** a third target costs little once the app is in Rust and Slint (cpal, souvlaki and
   tray-icon all support it). Worth including?
4. **The old apps:** retire the Swift and C# apps after phase 5, or keep one as a fallback?
