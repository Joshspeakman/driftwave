#!/bin/bash
# Station import/export test: the built Nightly app imports fixtures/exchange/seed.json into an empty
# test-run library and exports it again, then checks a second import adds nothing, and that the
# Windows app's export (if committed) imports. Muted test runs; the real library is never touched.
# Usage: scripts/exchange-test.sh [--update-fixture]   (writes fixtures/exchange/mac-export.json)
set -uo pipefail
cd "$(dirname "$0")/.."
APP="${SHORTWAVE_APP:-$HOME/ProjectBuilds/shortwave-macos/app/Shortwave Nightly.app}"
OUT=$( (command -v scratchdir >/dev/null && scratchdir shortwave-exchange) || mktemp -d)
rm -rf "$OUT"; mkdir -p "$OUT"
TESTSUP="$HOME/Library/Application Support/ShortwaveTestRun"
fail=0
fresh() { rm -rf "$TESTSUP/library.json" "$TESTSUP/Covers"; }
run() { # name, args...
  local name=$1; shift
  SHORTWAVE_LOG=1 "$APP/Contents/MacOS/Shortwave" -muted -quitAfter 3 "$@" >"$OUT/$name.log" 2>&1
}
expect() { # name, pattern
  if grep -q "$2" "$OUT/$1.log"; then echo "PASS $1: $2"; else echo "FAIL $1: wanted '$2' (log $OUT/$1.log)"; fail=1; fi
}

fresh
run seed -importStations fixtures/exchange/seed.json -exportStations "$OUT/mac-export.json"
expect seed "imported added=3 present=0 unreadable=0 covers=1"
expect seed "exported 3 stations"
ls "$TESTSUP/Covers" 2>/dev/null | grep -qE '\.png$' && echo "PASS seed: cover written" || { echo "FAIL seed: cover not written"; fail=1; }

run again -importStations "$OUT/mac-export.json"
expect again "imported added=0 present=3 unreadable=0"

if [ -f fixtures/exchange/windows-export.json ]; then
  fresh
  run windows -importStations fixtures/exchange/windows-export.json
  expect windows "imported added=3 present=0 unreadable=0 covers=1"
else
  echo "SKIP windows: fixtures/exchange/windows-export.json not committed yet"
fi
fresh

if [ $fail = 0 ] && [ "${1:-}" = "--update-fixture" ]; then
  cp "$OUT/mac-export.json" fixtures/exchange/mac-export.json && echo "wrote fixtures/exchange/mac-export.json"
fi
[ $fail = 0 ] && echo "PASS exchange test" || echo "logs in $OUT"
exit $fail
