#!/bin/bash
# Launches the app (muted, as a test run) with the parts only a real launch otherwise starts, and fails if one of them
# doesn't start or the app panics or quits early. (Windows' 1.0.0 crashed at launch in its media controls; see
# scripts/launch-test.ps1.)
#
#   scripts/launch-test.sh           the menu bar icon (-tray), the Dock reopen hook, the scene layers
#   scripts/launch-test.sh --media   and Now Playing / the media keys (-mediaControls)
#
# --media makes this copy the Mac's Now Playing app for a few seconds, which takes the media keys. CLAUDE.md: never
# while Josh is playing music. So it refuses while anything else holds the audio output (a pmset audio-out
# assertion); DRIFTWAVE_MEDIA_OK=1 overrides that once he's said it's fine.
# The app: DRIFTWAVE_APP, else the release build (built if needed).
set -uo pipefail
REPO="$(cd "$(dirname "$0")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/ProjectBuilds/shortwave-macos/rust-target}"

MEDIA=0
for a in "$@"; do
  case "$a" in
    --media) MEDIA=1 ;;
    *) echo "unknown option $a" >&2; exit 2 ;;
  esac
done

if [ $MEDIA = 1 ] && [ "${DRIFTWAVE_MEDIA_OK:-}" != 1 ]; then
  # Read it whole first: `pmset | grep -q` under pipefail fails on the SIGPIPE and would never refuse.
  ASSERTIONS=$(pmset -g assertions)
  if grep -q "Resources:.*audio-out" <<<"$ASSERTIONS"; then
    echo "REFUSED --media: something is playing audio (pmset audio-out). Run it when nothing is playing, or with DRIFTWAVE_MEDIA_OK=1 once that's been agreed."
    exit 3
  fi
fi

APP="${DRIFTWAVE_APP:-$CARGO_TARGET_DIR/release/driftwave}"
if [ -z "${DRIFTWAVE_APP:-}" ]; then
  CARGO=$(command -v cargo || echo "$HOME/.cargo/bin/cargo")
  (cd "$REPO/rust" && "$CARGO" build --release -p driftwave-app -q) || { echo "build failed"; exit 1; }
fi

OUT=$(mktemp -d "${TMPDIR:-/tmp}/driftwave-launchtest.XXXXXX")
LOG="$OUT/launch.log"
ARGS=(-muted -tray -library "$REPO/fixtures/design/library.json" -playURL http://ice1.somafm.com/groovesalad-128-mp3
      -layout full -theme night-market -quitAfter 12)
[ $MEDIA = 1 ] && ARGS+=(-mediaControls)

DRIFTWAVE_LOG=1 RUST_BACKTRACE=1 "$APP" "${ARGS[@]}" >"$OUT/stdout.log" 2>"$LOG" &
PID=$!
sleep 8
EARLY=0
kill -0 $PID 2>/dev/null || EARLY=1
# -quitAfter 12; give it a little longer, then make sure it's gone.
for _ in $(seq 1 30); do kill -0 $PID 2>/dev/null || break; sleep 0.5; done
kill $PID 2>/dev/null
wait $PID 2>/dev/null
CODE=$?

FAIL=0
[ $EARLY = 1 ] && { echo "FAIL quit within 8 s (log $LOG)"; FAIL=1; }
if grep -q "panicked" "$LOG"; then echo "FAIL panicked:"; grep -A2 "panicked" "$LOG"; FAIL=1; fi
[ $CODE -ne 0 ] && [ $CODE -ne 143 ] && { echo "FAIL exit code $CODE (log $LOG)"; FAIL=1; }
WANT="integration: tray=true media=$([ $MEDIA = 1 ] && echo true || echo false)"
grep -q "$WANT" "$LOG" || { echo "FAIL expected \"$WANT\" (log $LOG)"; grep "integration:" "$LOG"; FAIL=1; }
grep -q "scene layers: native" "$LOG" || { echo "FAIL the scene layers didn't start (log $LOG)"; FAIL=1; }

# Test runs keep their own data folder and never save; tidy it anyway.
rm -rf "$HOME/Library/Application Support/DriftwaveRustTestRun" "$HOME/Library/Caches/DriftwaveRustTestRun"
if [ $FAIL = 0 ]; then
  echo "PASS launch test ($([ $MEDIA = 1 ] && echo "menu bar, media controls" || echo "menu bar"), scene layers)"
  rm -rf "$OUT"
else
  exit 1
fi
