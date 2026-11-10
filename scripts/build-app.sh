#!/bin/bash
# Builds Shortwave.app (or "Shortwave Nightly.app" with --nightly) into ~/ProjectBuilds/shortwave-macos/app.
# Usage: scripts/build-app.sh [--nightly] [--debug]
set -euo pipefail
cd "$(dirname "$0")/.."
REPO=$(pwd)

NIGHTLY=0; CONFIG=release
for a in "$@"; do
  case "$a" in
    --nightly) NIGHTLY=1 ;;
    --debug) CONFIG=debug ;;
    *) echo "unknown option $a" >&2; exit 2 ;;
  esac
done

BUILD_ROOT="${SHORTWAVE_BUILD_ROOT:-$HOME/ProjectBuilds/shortwave-macos}"
mkdir -p "$BUILD_ROOT/.build" "$BUILD_ROOT/app"
# Build output lives on the Mac, not next to the source.
[ -L .build ] || { [ -e .build ] && rm -rf .build; ln -s "$BUILD_ROOT/.build" .build; }

if [ $NIGHTLY = 1 ]; then NAME="Shortwave Nightly"; BUNDLE_ID="io.github.joshspeakman.Shortwave.Nightly"
else NAME="Shortwave"; BUNDLE_ID="io.github.joshspeakman.Shortwave"; fi

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' VERSION.toml 2>/dev/null || true); VERSION=${VERSION:-1.0}
BUILD_NO=$(git rev-list --count HEAD 2>/dev/null || echo 1)
COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo dev)
BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")

swift build -c "$CONFIG" --product Shortwave
BIN="$(swift build -c "$CONFIG" --show-bin-path)/Shortwave"

APP="$BUILD_ROOT/app/$NAME.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/Shortwave"

# Icon
ICONWORK="$BUILD_ROOT/icon"
rm -rf "$ICONWORK"; mkdir -p "$ICONWORK/AppIcon.iconset"
swift tools/make-icon.swift "$ICONWORK/icon.png"
for s in 16 32 128 256 512; do
  sips -z $s $s "$ICONWORK/icon.png" --out "$ICONWORK/AppIcon.iconset/icon_${s}x${s}.png" >/dev/null
  sips -z $((s*2)) $((s*2)) "$ICONWORK/icon.png" --out "$ICONWORK/AppIcon.iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONWORK/AppIcon.iconset" -o "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>Shortwave</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$BUILD_NO</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>LSMinimumSystemVersion</key><string>15.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.music</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticTermination</key><false/>
  <key>NSHumanReadableCopyright</key><string>Internet radio for macOS, after GNOME Shortwave. Station data: radio-browser.info</string>
  <key>ShortwaveCommit</key><string>$COMMIT</string>
  <key>ShortwaveBranch</key><string>$BRANCH</string>
  <key>ShortwaveRepo</key><string>$REPO</string>
  <key>NSAppTransportSecurity</key>
  <dict>
    <!-- Many radio streams and station icons are plain http. -->
    <key>NSAllowsArbitraryLoads</key><true/>
  </dict>
</dict>
</plist>
PLIST

codesign --force --sign - --timestamp=none "$APP"
echo "$APP"
