#!/bin/bash

# Exit immediately if a command exits with a non-zero status
set -e

# Variables
APP_NAME="macxtreamer"
APP_BUNDLE="target/release/bundle/osx/$APP_NAME.app"
DMG_NAME="$APP_NAME.dmg"
DMG_DIR="dmg_temp"
VOLUME_NAME="$APP_NAME"

cargo clean
cargo bundle --bin $APP_NAME --release

# Check if the .app bundle exists
if [ ! -d "$APP_BUNDLE" ]; then
  echo "Error: $APP_BUNDLE does not exist. Build the app first."
  exit 1
fi

# Create a temporary directory for the DMG
mkdir -p "$DMG_DIR"
cp -R "$APP_BUNDLE" "$DMG_DIR/"

# Check and remove the existing symbolic link if it exists
if [ -L "$DMG_DIR/Applications" ]; then
  rm "$DMG_DIR/Applications"
fi

# Add a symbolic link to the Applications folder
ln -s /Applications "$DMG_DIR/Applications"

# Ensure the icon is correctly embedded
ICON_PATH="assets/icon.icns"
if [ ! -f "$ICON_PATH" ]; then
  echo "Error: $ICON_PATH does not exist. Please provide a valid icon file."
  exit 1
fi

# Stelle sicher, dass der Resources-Ordner existiert
RESOURCES_DIR="$DMG_DIR/$APP_NAME.app/Contents/Resources"
mkdir -p "$RESOURCES_DIR"

# Kopiere das Icon in den Resources-Ordner
cp "$ICON_PATH" "$RESOURCES_DIR/icon.icns"

# Aktualisiere die Info.plist, um sicherzustellen, dass das Icon korrekt referenziert wird
PLIST_FILE="$DMG_DIR/$APP_NAME.app/Contents/Info.plist"

# Debugging: Überprüfe den Pfad und den Inhalt der Info.plist-Datei vor der Bearbeitung
echo "Überprüfe Info.plist unter: $PLIST_FILE"
if [ -f "$PLIST_FILE" ]; then
  echo "Info.plist gefunden. Inhalt vor der Bearbeitung:"
  cat "$PLIST_FILE"
else
  echo "Info.plist nicht gefunden unter: $PLIST_FILE"
  exit 1
fi

# Füge den CFBundleIconFile-Schlüssel hinzu oder setze ihn sicher
if /usr/libexec/PlistBuddy -c "Print :CFBundleIconFile" "$PLIST_FILE" > /dev/null 2>&1; then
  echo "CFBundleIconFile exists. Updating its value."
  /usr/libexec/PlistBuddy -c "Set :CFBundleIconFile icon.icns" "$PLIST_FILE"
else
  echo "CFBundleIconFile does not exist. Adding it to Info.plist."
  /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string icon.icns" "$PLIST_FILE"
fi

# Debugging: Überprüfe den Inhalt der Info.plist-Datei nach der Bearbeitung
echo "Inhalt der Info.plist nach der Bearbeitung:"
cat "$PLIST_FILE"

# Create the DMG file
hdiutil create "$DMG_NAME" \
  -volname "$VOLUME_NAME" \
  -srcfolder "$DMG_DIR" \
  -ov \
  -format UDZO

# Mount the DMG to customize the layout
MOUNT_DIR=$(hdiutil attach "$DMG_NAME" | grep Volumes | awk '{print $3}')

# Set the Finder window properties
osascript <<EOD
   tell application "Finder"
       tell disk "$VOLUME_NAME"
           open
           set current view of container window to icon view
           set toolbar visible of container window to false
           set statusbar visible of container window to false
           set the bounds of container window to {100, 100, 600, 400}
           set theViewOptions to the icon view options of container window
           set arrangement of theViewOptions to not arranged
           set icon size of theViewOptions to 128

           -- Position the application icon
           set position of item "$APP_NAME.app" of container window to {100, 150}

           -- Position the Applications folder
           set position of item "Applications" of container window to {400, 150}

           close
           open
           update without registering applications
       end tell
   end tell
EOD

# Detach the DMG
hdiutil detach "$MOUNT_DIR"

# Clean up the temporary directory
rm -rf "$DMG_DIR"

echo "DMG created: $DMG_NAME"
