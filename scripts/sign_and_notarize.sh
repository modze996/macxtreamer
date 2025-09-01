#!/usr/bin/env bash
set -euo pipefail

# Fill these with your credentials / team info
TEAM_ID="YOUR_TEAM_ID"
IDENTITY="Developer ID Application: Your Name ($TEAM_ID)"
APP_PATH="${1:-target/release/bundle/osx/MacXtreamer.app}"
DMG_PATH="${2:-target/release/MacXtreamer.dmg}"
ENTITLEMENTS="${3:-}" # optional .entitlements plist

if [[ ! -d "$APP_PATH" ]]; then
  echo "App bundle not found at $APP_PATH"
  exit 1
fi

echo "Signing app..."
codesign --deep --force --options runtime --timestamp \
  ${ENTITLEMENTS:+--entitlements "$ENTITLEMENTS"} \
  -s "$IDENTITY" "$APP_PATH"

echo "Verifying signature..."
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

echo "Creating DMG for notarization..."
"$(dirname "$0")/make_dmg.sh" "$APP_PATH" "$DMG_PATH"

echo "Notarizing (you will be prompted for Apple ID creds if not configured)..."
xcrun notarytool submit "$DMG_PATH" --apple-id YOUR_APPLE_ID@example.com --team-id "$TEAM_ID" --keychain-profile AC_PASSWORD --wait

echo "Stapling ticket..."
xcrun stapler staple "$APP_PATH"
echo "Done."
