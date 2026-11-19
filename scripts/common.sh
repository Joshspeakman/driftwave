# Shared by the .sh test scripts: the Rust headless player (rust/audio, driftwave-player), built if needed.
# DRIFTWAVE_PLAYER overrides the binary; CARGO_TARGET_DIR is honoured (default: ~/ProjectBuilds/shortwave-macos/rust-target).
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/ProjectBuilds/shortwave-macos/rust-target}"
BIN="${DRIFTWAVE_PLAYER:-$CARGO_TARGET_DIR/release/driftwave-player}"
if [ -z "${DRIFTWAVE_PLAYER:-}" ]; then
  CARGO=$(command -v cargo || echo "$HOME/.cargo/bin/cargo")
  (cd "$REPO/rust" && "$CARGO" build --release -p driftwave-audio --bin driftwave-player -q) || { echo "build failed"; exit 1; }
fi
