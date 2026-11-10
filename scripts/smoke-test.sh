#!/bin/bash
# Plays real streams (muted) in the built Nightly app and checks that playback starts and titles arrive.
# Usage: scripts/smoke-test.sh [seconds-per-stream]
set -uo pipefail
cd "$(dirname "$0")/.."
APP="${SHORTWAVE_APP:-$HOME/ProjectBuilds/shortwave-macos/app/Shortwave Nightly.app}"
SECS=${1:-20}
OUT=$( (command -v scratchdir >/dev/null && scratchdir shortwave-smoke) || mktemp -d)
mkdir -p "$OUT"
STREAMS=(
  "mp3-icy|http://ice1.somafm.com/groovesalad-128-mp3|titles=true format=mp3"
  "aac-icy|https://ice1.somafm.com/groovesalad-128-aac|titles=true format=aac"
  "hls|https://live.m6radio.quortex.io/webM89Hc99XApzgfhXNX8ASN5/grouprtl/national/short/audio-64000/index.m3u8|"
  "ogg-vorbis|https://dancewave.online/dance.ogg|"
  "pls-playlist|https://somafm.com/groovesalad.pls|titles=true"
)
fail=0
for entry in "${STREAMS[@]}"; do
  IFS='|' read -r name url expect <<<"$entry"
  log="$OUT/$name.log"
  SHORTWAVE_LOG=1 "$APP/Contents/MacOS/Shortwave" -playURL "$url" -muted >"$log" 2>&1 &
  pid=$!
  sleep "$SECS"
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
  ok=1
  grep -q "state playing" "$log" || ok=0
  [ -n "$expect" ] && { grep -q "icy connected $expect" "$log" || ok=0; }
  if [ "$name" != hls ] && [ "$name" != ogg-vorbis ]; then grep -q "now playing" "$log" || ok=0; fi
  if [ $ok = 1 ]; then echo "PASS $name: $(grep -m1 'now playing' "$log" | sed 's/.*now playing //')"
  else echo "FAIL $name (log: $log)"; fail=1; fi
done
exit $fail
