#!/bin/bash
# Builds Ngambang.app and installs it to ~/Applications (or /Applications with --system).
set -euo pipefail
cd "$(dirname "$0")"

command -v cargo >/dev/null || { echo "Rust not found. Get it at https://rustup.rs" >&2; exit 1; }
command -v claude >/dev/null || echo "warning: 'claude' CLI not found in PATH. The app needs Claude Code to talk to." >&2
cargo tauri --version >/dev/null 2>&1 || cargo install tauri-cli --version "^2" --locked

DEST="$HOME/Applications"
[ "${1:-}" = "--system" ] && DEST="/Applications"
mkdir -p "$DEST"

cargo tauri build --bundles app
APP="target/release/bundle/macos/Ngambang.app"

pkill -x ngambang 2>/dev/null || true
rm -rf "$DEST/Ngambang.app"
cp -R "$APP" "$DEST/"
xattr -cr "$DEST/Ngambang.app"
# Keep Spotlight from listing the build copy as a second "Ngambang".
touch target/.metadata_never_index
rm -rf "$APP"
echo "Installed to $DEST/Ngambang.app"
