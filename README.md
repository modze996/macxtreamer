# MacXtreamer

A lightweight macOS client (Rust, eframe/egui) for Xtream APIs. It preloads categories and items in the background, caches cover images, renders a fast table for Live/VOD/Series, and launches your external player (e.g., VLC) with sensible streaming defaults.

## Features
- Native macOS app using eframe/egui
- Live, VOD and Series with a fast, full-width/height table
- Draggable top/bottom panels with visible grab bars
- Search (Enter to start), background index
- Startup preload of categories/items/covers
- Disk cover cache with TTL and parallel downloads
- Recently played and Favorites persistence
- Light/Dark theme, increased default font size

## Prerequisites
- macOS
- Rust (stable), installed via rustup (https://rustup.rs)
- An external player, recommended: VLC
	- macOS CLI path (example): /Applications/VLC.app/Contents/MacOS/VLC
	- Alternatively, ensure `vlc` is in your PATH

## Build
```bash
# Dependencies are managed by Cargo
cargo build --release
```

Run (development):
```bash
cargo run -q
```

Run (release binary):
```bash
./target/release/MacXtreamer
```

Note: The produced binary name may vary with Cargo settings (case sensitivity). Use `cargo build --release` and check `target/release/`.

## Installation
Simplest: use the generated binary from `target/release/`. Take a look on chapter "Bypass signing checks for DMG/App (Gatekeeper)" as there is currently only a not signed version.

Optional (macOS App Bundle): Use `cargo-bundle` to create a `.app` bundle:
```bash
cargo install cargo-bundle
cargo bundle --release
```
The `.app` bundle will show up under `target/release/bundle/osx/`.

### App Icon
- Generate assets (once):
	- cargo run --bin genicon
	- cargo run --bin mkiconset
	- iconutil -c icns assets/macxtreamer.iconset -o assets/icon.icns
- The app also embeds a generated icon at runtime for the window/Dock.

### Create DMG (optional)
```bash
scripts/make_dmg.sh target/release/bundle/osx/MacXtreamer.app target/release/MacXtreamer.dmg
```

## Configuration
On first start, set the following in “Settings”:
- URL (server address)
- Username
- Password
- Player command (see below)
- Cover TTL (days), cover parallelism
- Theme (Dark/Light)

Paths (macOS):
- Config: `~/Library/Application Support/MacXtreamer/xtream_config.txt`
- Data (Recently/Favorites): `~/Library/Application Support/MacXtreamer/`
- Cache (JSON/Covers): `~/Library/Caches/MacXtreamer/cache/` and `.../images/`

### Player command and placeholder
The player command supports the placeholder `URL`. It will be replaced with the actual stream URL. If `URL` is missing, the URL will be appended at the end.

Recommended VLC defaults:
```
vlc --fullscreen --no-video-title-show --network-caching=2000 URL
```

If `vlc` is not in PATH, use the full path, e.g.:
```
/Applications/VLC.app/Contents/MacOS/VLC --fullscreen --no-video-title-show --network-caching=2000 URL
```

## Usage
- Top: Categories (Live/VOD/Series). Click to load items.
- Middle: Table with cover, details and actions. Series open an episode list.
- Bottom: Recently played and Favorites.
- Search: Type text and press Enter (or click “Search”).
- Preload: On startup, categories/items/covers are prefetched in the background.

## Cache & performance
- Categories (~6h), items (~3h), episodes (~12h) are cached as JSON.
- Cover cache (TTL configurable in Settings) with parallel downloads.

## Troubleshooting
- VLC doesn’t launch: Check `player_command` (path correct? `URL` placeholder present?)
- No content: Check `address`, `username`, `password` in Settings/Config.
- Font too big/small: The app uses an increased default scale; you can change `font_scale` in the config file.

### macOS: Bypass signing checks for DMG/App (Gatekeeper)
If you’re testing a locally built DMG or .app that isn’t signed/notarized yet, macOS Gatekeeper may block it. You can allow it temporarily from the command line:

Option A: Allow apps from anywhere (system-wide until changed)
```bash
sudo spctl --master-disable
```
You can later re-enable Gatekeeper with:
```bash
sudo spctl --master-enable
```

Option B: Allow just this app (recommended)
1) Remove the quarantine attribute from the app bundle (or DMG mount):
```bash
# If mounted at /Volumes/MacXtreamer/MacXtreamer.app
xattr -dr com.apple.quarantine "/Volumes/MacXtreamer/MacXtreamer.app"

# Or after copying to /Applications
xattr -dr com.apple.quarantine "/Applications/MacXtreamer.app"
```
2) Explicitly allow the app via spctl:
```bash
spctl --add --label "MacXtreamer" "/Applications/MacXtreamer.app"
spctl --enable --label "MacXtreamer"
```
3) First run may still prompt; open once via right-click → Open, or:
```bash
open "/Applications/MacXtreamer.app"
```

Notes:
- You may need admin rights for some commands.
- For distribution to other users, prefer proper signing and notarization.

## Android / Fire TV (Option A)
This repo contains an Android TV app (Compose + Media3) that reuses a Rust core via JNI.

### Prerequisites
- Android Studio (Giraffe or newer), Android SDK + NDK
- cargo-ndk installed: `cargo install cargo-ndk`
- Set `ANDROID_NDK_HOME` (or ensure Android Studio manages the NDK and cargo-ndk finds it)

### Build native Rust core for Android
```bash
scripts/build_android_core.sh
```
This will build `macxtreamer_core` for common ABIs and copy `.so` files into `mobile/android/app/src/main/jniLibs/`.

### Build and run the Android app
- Open `mobile/android` in Android Studio, sync Gradle, then Run on a device/emulator (Fire TV/Android TV recommended).

### Try it
1) Launch the app, go to the VOD tab
2) Enter address, username, password, tap “Save config”
3) Tap “Fetch VOD Cats”, choose a category
4) Tap an item to play it; a Player screen opens with ExoPlayer
5) Live and Series tabs provide basic category/item browsing as well

Notes:
- The JNI bridge currently returns JSON for categories/items. The UI maps minimal fields (id, name).
- Playback URL building uses the same logic as desktop (info: Movie/Live/SeriesEpisode). Container extensions default to mp4/m3u8.

## License
See `LICENSE`.
