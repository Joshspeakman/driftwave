#!/bin/bash
# Two player behaviours against the local test server (muted test runs; needs ffmpeg):
#  1. reconnect: the server closes the title (ICY) connection every 20 s while the audio keeps
#     playing. The app must reconnect it, keep following titles, and not list a song twice.
#  2. playlist probe: a local station whose link is a .pls with no file extension (served as
#     audio/x-scpls) must be followed to the stream.
# Usage: scripts/behaviour-test.sh
set -uo pipefail
cd "$(dirname "$0")/.."
APP="${SHORTWAVE_APP:-$HOME/ProjectBuilds/shortwave-macos/app/Shortwave Nightly.app}"
OUT=$( (command -v scratchdir >/dev/null && scratchdir shortwave-behaviour) || mktemp -d)
rm -rf "$OUT"; mkdir -p "$OUT/media"
ffmpeg -loglevel error -f lavfi -i "sine=frequency=440:duration=30" -ac 2 -ar 44100 -b:a 128k "$OUT/media/tone.mp3"
ffmpeg -loglevel error -f lavfi -i "sine=frequency=660:duration=30" -ac 2 -ar 44100 -c:a aac -b:a 128k -f adts "$OUT/media/tone.aac"
PORT=8766
python3 scripts/icy-test-server.py --port $PORT --interval 15 --drop-icy-after 20 --media "$OUT/media" & SERVER=$!
trap 'kill $SERVER 2>/dev/null' EXIT
sleep 1
fail=0
check() { if eval "$2"; then echo "PASS $1"; else echo "FAIL $1 (logs in $OUT)"; fail=1; fi; }

SHORTWAVE_LOG=1 "$APP/Contents/MacOS/Shortwave" -muted -quitAfter 52 -playURL "http://127.0.0.1:$PORT/mp3" >"$OUT/reconnect.log" 2>&1
L="$OUT/reconnect.log"
connects=$(grep -c "icy connected titles=true" "$L")
check "reconnect: title connection came back ($connects connections)" '[ "$connects" -ge 2 ]'
check "reconnect: playback never stopped, failed or rebuffered" '! grep -v "stop (station change)" "$L" | grep -qE "stop \(|state failed" && [ "$(grep -c "state loading" "$L")" = 1 ]'
check "reconnect: titles kept coming (Song 3 shown)" 'grep -q "now playing \"Test Artist - Song 3\"" "$L"'
dups=$(grep "now playing" "$L" | sort | uniq -d | wc -l | tr -d " ")
check "reconnect: no song shown twice" '[ "$dups" = 0 ]'

SHORTWAVE_LOG=1 "$APP/Contents/MacOS/Shortwave" -muted -quitAfter 12 -playURL "http://127.0.0.1:$PORT/playlist" >"$OUT/playlist.log" 2>&1
check "playlist probe: extensionless .pls link played" 'grep -q "state playing" "$OUT/playlist.log" && grep -q "icy connected titles=true" "$OUT/playlist.log"'

[ $fail = 0 ] && echo "PASS behaviour test"
exit $fail
