#!/bin/bash
# Runs the core unit tests. Use this instead of a bare `swift test`.
# With only the Command Line Tools (Swift 6.4), SwiftPM no longer finds the swift-testing macro
# plugin, so @Test/@Suite fail to expand. The plugin ships in the CLT under host/plugins/testing;
# point the compiler at it when it is there.
set -euo pipefail
cd "$(dirname "$0")/.."
PLUGINS=/Library/Developer/CommandLineTools/usr/lib/swift/host/plugins/testing
EXTRA=()
[ -d "$PLUGINS" ] && EXTRA=(-Xswiftc -plugin-path -Xswiftc "$PLUGINS")
exec swift test ${EXTRA[@]+"${EXTRA[@]}"} "$@"
