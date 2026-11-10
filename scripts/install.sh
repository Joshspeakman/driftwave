#!/bin/bash
# Builds and installs the app into /Applications, replacing an older copy.
# Usage: scripts/install.sh [--nightly]
set -euo pipefail
cd "$(dirname "$0")/.."
APP=$(scripts/build-app.sh "$@" | tail -1)
NAME=$(basename "$APP")
DEST="/Applications/$NAME"
# Quit a running copy first.
osascript -e "tell application id \"$(defaults read "$APP/Contents/Info" CFBundleIdentifier)\" to quit" 2>/dev/null || true
for _ in 1 2 3 4 5 6 7 8 9 10; do pgrep -f "$DEST/Contents/MacOS/Shortwave" >/dev/null || break; sleep 0.5; done
rm -rf "$DEST"
ditto "$APP" "$DEST"
echo "Installed $DEST"
