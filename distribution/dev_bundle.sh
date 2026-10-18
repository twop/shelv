#!/bin/bash
# Builds a debug Shelv.app with its document types registered, for local testing.
# (files opened via LaunchServices are only delivered to bundled apps, not to `cargo run`)
#
# Usage:
#   distribution/dev_bundle.sh
#   open -a target/debug/bundle/osx/Shelv.app path/to/file.md
#
# Note: launched via LaunchServices the app's stdout is lost, use
#   open -a target/debug/bundle/osx/Shelv.app --stdout /tmp/shelv.log --stderr /tmp/shelv.log file.md

set -euo pipefail

APP="target/debug/bundle/osx/Shelv.app"
PLIST_FILE="$APP/Contents/Info.plist"
LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"

# document types come from `info_plist_exts` in Cargo.toml (requires cargo-bundle >= 0.12.0)
cargo bundle --format osx --release

if ! /usr/libexec/PlistBuddy -c "Print :CFBundleDocumentTypes" "$PLIST_FILE" >/dev/null 2>&1; then
  echo "CFBundleDocumentTypes missing from $PLIST_FILE, is cargo-bundle older than 0.12.0?"
  exit 1
fi

if ! /usr/libexec/PlistBuddy -c "Print :LSUIElement" "$PLIST_FILE" >/dev/null 2>&1; then
  echo "LSUIElement missing from $PLIST_FILE, is distribution/Info.ext.plist merged?"
  exit 1
fi

# Make LaunchServices pick up the registration from this exact bundle
"$LSREGISTER" -f "$APP"
echo "Registered $APP"
