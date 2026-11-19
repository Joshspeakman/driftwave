#!/bin/bash
# End-to-end recording test against a local ICY server: plays MP3 and AAC streams (muted)
# in "Save All Tracks" mode and checks that whole tracks are cut, saved and playable.
# Needs ffmpeg. Usage: scripts/recording-test.sh
set -uo pipefail
cd "$(dirname "$0")/.."
. "$(dirname "$0")/common.sh"
OUT=$( (command -v scratchdir >/dev/null && scratchdir shortwave-rectest) || mktemp -d)
rm -rf "$OUT"; mkdir -p "$OUT/media"
ffmpeg -loglevel error -f lavfi -i "sine=frequency=440:duration=30" -ac 2 -ar 44100 -b:a 128k "$OUT/media/tone.mp3"
ffmpeg -loglevel error -f lavfi -i "sine=frequency=660:duration=30" -ac 2 -ar 44100 -c:a aac -b:a 128k -f adts "$OUT/media/tone.aac"
PORT=8765; INTERVAL=15
python3 scripts/icy-test-server.py --port $PORT --interval $INTERVAL --media "$OUT/media" & SERVER=$!
trap 'kill $SERVER 2>/dev/null' EXIT
sleep 1
fail=0
for fmt in mp3 aac; do
  tracks="$OUT/tracks-$fmt"; mkdir -p "$tracks"
  "$BIN" -playURL "http://127.0.0.1:$PORT/$fmt" -muted \
    -recordingMode everything -recordingMinimumDuration 5 -trackDirectory "$tracks" >"$OUT/$fmt.log" 2>&1 &
  pid=$!
  sleep $((INTERVAL * 3 + 8))
  kill $pid 2>/dev/null; wait $pid 2>/dev/null
  saved=$(ls "$tracks" | wc -l | tr -d ' ')
  echo "--- $fmt: $saved saved"; ls -la "$tracks"
  [ "$saved" -ge 1 ] || { echo "FAIL $fmt: nothing saved (log $OUT/$fmt.log)"; fail=1; continue; }
  for f in "$tracks"/*; do
    dur=$(afinfo "$f" 2>/dev/null | sed -n 's/estimated duration: \([0-9.]*\).*/\1/p')
    echo "  $(basename "$f"): ${dur:-?} s"
    awk -v d="${dur:-0}" -v i=$INTERVAL 'BEGIN { exit !(d > i - 4 && d < i + 4) }' || { echo "FAIL $fmt: duration ${dur:-?} not ~${INTERVAL}s"; fail=1; }
  done
  grep -q 'idleIncomplete' "$OUT/$fmt.log" || { echo "FAIL $fmt: first track not marked incomplete"; fail=1; }
done
[ $fail = 0 ] && echo "PASS recording test" || echo "logs in $OUT"
exit $fail
