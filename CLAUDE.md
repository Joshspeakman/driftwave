# Shortwave: notes for Claude sessions

Josh's personal internet radio app, modelled on GNOME Shortwave. The macOS app is native SwiftUI
(`Sources/`). A Windows version is being built from `windows/PLAN.md`; read that first if you are
on Windows.

## Branches

- `nightly` is the default and working branch for the Mac app.
- `windows` is the working branch for the Windows port. Its code goes under `windows/`, and it
  merges `nightly` when the Mac side changes.
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
- Never touch the real app's library, settings or track folder from a test.

## Mac build

Only the Command Line Tools are needed, not Xcode.

| Command | What it does |
|---|---|
| `scripts/test.sh` | Core unit tests (19). Use it instead of a bare `swift test`, which can't find the test macros with only the Swift 6.4 Command Line Tools |
| `scripts/build-app.sh --nightly` | Builds `~/ProjectBuilds/shortwave-macos/app/Shortwave Nightly.app` |
| `scripts/install.sh --nightly` | Builds and installs `/Applications/Shortwave Nightly.app` |
| `scripts/smoke-test.sh` | Real streams, muted |
| `scripts/recording-test.sh` | Local ICY server plus ffmpeg: checks that tracks are cut and saved |
