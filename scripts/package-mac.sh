#!/bin/bash
# Builds Driftwave.app (or "Driftwave Nightly.app" with --nightly) from rust/app into
# ~/ProjectBuilds/shortwave-macos/app, ad-hoc signed. The bundle gives macOS what a bare binary lacks:
# a bundle identifier (notifications), an icon, and a name in the Dock and menu bar.
# Usage: scripts/package-mac.sh [--nightly] [--debug]
set -euo pipefail
cd "$(dirname "$0")/.."
REPO=$(pwd)

NIGHTLY=0; PROFILE=release
for a in "$@"; do
  case "$a" in
    --nightly) NIGHTLY=1 ;;
    --debug) PROFILE=dev ;;
    *) echo "unknown option $a" >&2; exit 2 ;;
  esac
done
if [ $NIGHTLY = 1 ]; then NAME="Driftwave Nightly"; BUNDLE_ID="io.github.joshspeakman.Driftwave.Nightly"
else NAME="Driftwave"; BUNDLE_ID="io.github.joshspeakman.Driftwave"; fi

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/ProjectBuilds/shortwave-macos/rust-target}"
CARGO=$(command -v cargo || echo "$HOME/.cargo/bin/cargo")
(cd rust && "$CARGO" build --profile "$PROFILE" -p driftwave-app)
BIN="$CARGO_TARGET_DIR/$([ "$PROFILE" = dev ] && echo debug || echo release)/driftwave"

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' rust/Cargo.toml | head -1); VERSION=${VERSION:-1.0.0}
BUILD_NO=$(git rev-list --count HEAD 2>/dev/null || echo 1)
COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo dev)
BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")

OUT="$HOME/ProjectBuilds/shortwave-macos/app"
APP="$OUT/$NAME.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/Driftwave"

# Icon: every size macOS asks for, from the 1024 px render of the logo.
WORK=$(mktemp -d); trap 'rm -rf "$WORK"' EXIT
mkdir "$WORK/AppIcon.iconset"
SRC=assets/icons/driftwave-1024.png
for s in 16 32 128 256 512; do
  sips -z $s $s "$SRC" --out "$WORK/AppIcon.iconset/icon_${s}x${s}.png" >/dev/null
  sips -z $((s*2)) $((s*2)) "$SRC" --out "$WORK/AppIcon.iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$WORK/AppIcon.iconset" -o "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>Driftwave</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$BUILD_NO</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.music</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticTermination</key><false/>
  <key>NSHumanReadableCopyright</key><string>Internet radio. Station data: radio-browser.info. Made with Slint.</string>
  <key>DriftwaveCommit</key><string>$COMMIT</string>
  <key>DriftwaveBranch</key><string>$BRANCH</string>
</dict>
</plist>
PLIST

codesign --force --sign - --timestamp=none "$APP"
echo "$APP"
