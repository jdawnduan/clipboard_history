#!/bin/bash
# Build the macOS .app bundle for clipboard-history.
# Run after `cargo build --release`.
#
# Usage: ./scripts/build_app_bundle.sh
#
# The bundle wraps the binary with a stable CFBundleIdentifier so macOS
# accessibility permissions survive upgrades. When installed via Homebrew,
# the formula should invoke this script and install the resulting .app.

set -euo pipefail

BINARY="target/release/clipboard-history"
APP_NAME="Clipboard History.app"
APP_DIR="$APP_NAME/Contents"
MACOS_DIR="$APP_DIR/MacOS"

if [ ! -f "$BINARY" ]; then
    echo "Error: Release binary not found. Run 'cargo build --release' first."
    echo "  Expected: $BINARY"
    exit 1
fi

mkdir -p "$MACOS_DIR"
cp "$BINARY" "$MACOS_DIR/clipboard-history"
cp macos/Info.plist "$APP_DIR/Info.plist"

# Make the binary executable
chmod +x "$MACOS_DIR/clipboard-history"

echo "✅ Bundle created: $APP_NAME"
echo "   Binary inside: $MACOS_DIR/clipboard-history"
echo ""
echo "To test: open '$APP_NAME'"
echo "To install system-wide: cp -R '$APP_NAME' /Applications/"
