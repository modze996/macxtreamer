#!/usr/bin/env bash
set -euo pipefail

APP_PATH="${1:-target/release/bundle/osx/MacXtreamer.app}"
DMG_PATH="${2:-target/release/MacXtreamer.dmg}"
VOLNAME="${3:-MacXtreamer}"

if [[ ! -d "$APP_PATH" ]]; then
  echo "App bundle not found at $APP_PATH"
  echo "Build it first: cargo bundle --release"
  exit 1
fi

mkdir -p "$(dirname "$DMG_PATH")"
rm -f "$DMG_PATH"

echo "Creating DMG: $DMG_PATH"
hdiutil create -volname "$VOLNAME" -srcfolder "$APP_PATH" -ov -format UDZO "$DMG_PATH"
echo "DMG created: $DMG_PATH"
