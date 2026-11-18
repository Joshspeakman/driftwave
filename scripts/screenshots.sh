#!/bin/bash
# Draws every main screen of the built Nightly app, in light and dark, into docs/design/mac/
# (muted test runs with their own library; the real app's data is never touched). Pair with the
# Windows app's -screenshot renders in docs/design/windows/.
# Usage: scripts/screenshots.sh [name ...]   (default: all)
set -uo pipefail
cd "$(dirname "$0")/.."
APP="${SHORTWAVE_APP:-$HOME/ProjectBuilds/shortwave-macos/app/Shortwave Nightly.app}"
OUT=docs/design/mac
LOGS=$( (command -v scratchdir >/dev/null && scratchdir shortwave-shots) || mktemp -d)
mkdir -p "$OUT" "$LOGS"
TESTSUP="$HOME/Library/Application Support/ShortwaveTestRun"
STREAM=http://ice1.somafm.com/groovesalad-128-mp3

shot() { # name delay args...
  local name=$1 delay=$2; shift 2
  for look in light dark; do
    local png="$PWD/$OUT/$name-$look.png"
    rm -f "$png"
    SHORTWAVE_LOG=1 "$APP/Contents/MacOS/Shortwave" -muted -appearance $look -screenshotDelay "$delay" -screenshot "$png" "$@" \
      >"$LOGS/$name-$look.log" 2>&1
    if [ -f "$png" ]; then
      sips -Z "$(( $(sips -g pixelWidth "$png" | awk '/pixelWidth/ {print $2}') / 2 ))" "$png" >/dev/null   # 1x, like the Windows renders
      echo "ok   $name-$look"
    else
      echo "FAIL $name-$look (log $LOGS/$name-$look.log)"
    fi
  done
}
want() { [ -z "$WANT" ] || [[ " $WANT " == *" $1 "* ]]; }
WANT="$*"

rm -rf "$TESTSUP/library.json" "$TESTSUP/Covers"
want library-empty && shot library-empty 5 -page library
SHORTWAVE_LOG=1 "$APP/Contents/MacOS/Shortwave" -muted -quitAfter 2 -importStations fixtures/exchange/seed.json >/dev/null 2>&1
SHORTWAVE_LOG=1 "$APP/Contents/MacOS/Shortwave" -muted -quitAfter 2 -importStations fixtures/design/library.json >/dev/null 2>&1
want library && shot library 6 -page library
want discover && shot discover 10 -page discover
want search && shot search 10 -page search -searchText jazz
want settings && shot settings 5 -page settings
want station-details && shot station-details 10 -page search -searchText "groove salad" -openFirstResult
want add-station && shot add-station 5 -page library -addStation
want player-playing && shot player-playing 16 -page library -playURL "$STREAM"
want mini-player && shot mini-player 16 -miniPlayer -playURL "$STREAM"
rm -rf "$TESTSUP/library.json" "$TESTSUP/Covers"
