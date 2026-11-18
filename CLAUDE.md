# Shortwave: notes for Claude sessions

Josh's personal internet radio app, modelled on GNOME Shortwave. The macOS app is native SwiftUI
(`Sources/`). The Windows version (C# + WinUI 3, under `windows/`) was built from `windows/PLAN.md`;
on Windows read `windows/README.md` and `windows/DECISIONS.md` first.

## Branches

- `nightly` is the default and working branch for the Mac app.
- `windows` is the working branch for the Windows port. Its code goes under `windows/`.
- The Mac and Windows sessions can't message each other. They coordinate through
  `docs/MAC-WINDOWS-SYNC.md`: read it first, and take the newest round's asks for your side. Each
  side pushes only its own branch and merges the other's before starting (`git merge
  origin/nightly` on the PC, `git merge origin/windows` on the Mac).
- Shared specs: `docs/STATION-EXCHANGE.md` (moving stations between the apps). Shared test files:
  `fixtures/`.
- `main` changes only when Josh says so.
- The GitHub repo is private. Push with the `Joshspeakman` GitHub account. On the Mac the active
  `gh` account has no access, so use `GH_TOKEN=$(gh auth token --user Joshspeakman)` per command.

## Working with Josh

- Ask decisions in the question dialog, one question at a time, never as a wall of text.
- Finish the work end to end: fix what you find in the same pass, and don't leave "next steps".

## Testing rules (both platforms)

- **Test runs are muted and isolated.** Any test flag (`-muted`, `-playURL`, `-page`, ...) or
  `SHORTWAVE_TEST=1` makes a test run. A test run uses its own data folder (`ShortwaveTestRun`)
  and its own preferences, stays muted unless `-audible` is passed, and stays off Now Playing and
  the media keys. The Windows app must work the same way.
  **Why:** a test once played a tone aloud, and a muted test grabbed Josh's media keys while he
  was playing YouTube Music. A media key press then stopped a test recording.
- Never test media keys, Now Playing or the system media overlay while Josh is playing music.
  Ask him first.
- Don't press mute (⌥⌘M / Ctrl+Alt+M) in a test run: test copies start muted, so it unmutes them.
- Never touch the real app's library, settings or track folder from a test.
- Real-click tests on the Mac: AppleScript's `process whose unix id is …` matches by **name**, and
  Josh's installed app is also called "Shortwave", so it reaches his real app. Target the test copy
  by PID instead (Accessibility API plus CGEvent). Check first that he isn't using the machine.

## Mac build

Only the Command Line Tools are needed, not Xcode.

| Command | What it does |
|---|---|
| `scripts/test.sh` | Core unit tests (26). Use it instead of a bare `swift test`, which can't find the test macros with only the Swift 6.4 Command Line Tools |
| `scripts/build-app.sh --nightly` | Builds `~/ProjectBuilds/shortwave-macos/app/Shortwave Nightly.app` |
| `scripts/install.sh --nightly` | Builds and installs `/Applications/Shortwave Nightly.app` |
| `scripts/smoke-test.sh` | Real streams, muted |
| `scripts/recording-test.sh` | Local ICY server plus ffmpeg: checks that tracks are cut and saved |
| `scripts/exchange-test.sh` | Station import/export in the real app (test runs); `--update-fixture` rewrites `fixtures/exchange/mac-export.json` |
| `scripts/screenshots.sh` | Every screen, light and dark, into `docs/design/mac/` |
| `scripts/behaviour-test.sh` | Local server: the title connection drops and must reconnect without repeats; an extensionless `.pls` must play |

## Windows build

Needs the .NET 10 SDK; the tests also need Python 3 and ffmpeg. Run from the repo root in PowerShell.

| Command | What it does |
|---|---|
| `windows/scripts/test.ps1` | Core unit tests (31: the 19 Swift ones translated, plus Windows-only ones) |
| `windows/scripts/build.ps1 -Flavor Nightly` | Builds `windows/out/Nightly/Shortwave.exe` |
| `windows/scripts/install.ps1 -Flavor Nightly` | Builds and installs `Shortwave Nightly` for the current user (Start menu, Settings > Apps) |
| `windows/scripts/smoke-test.ps1` | Real streams, muted |
| `windows/scripts/recording-test.ps1` | Local ICY server plus ffmpeg: checks that tracks are cut, saved and tagged |

- With the displays asleep, newly started GPU-drawn windows (WinUI and WPF alike) came out black in
  screen captures. `-screenshot <png>` renders the app's XAML instead, dialogs included; use it to
  check the UI.
- Test runs save tracks under `ShortwaveTestRun\Tracks` unless given `-trackDirectory`. Never point
  them at `Music\Shortwave`.
- Keep `EnableMsixTooling` in `Shortwave.csproj`. Without it, published builds have no `Shortwave.pri`
  and crash on the first XAML page.
- Keep a reference to every `DispatcherQueueTimer` until it fires. An unreferenced one can be
  garbage-collected and never tick.
