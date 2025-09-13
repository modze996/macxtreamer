# MacXtreamer Android / Fire TV (Option A)

Dieser Ordner enthält ein Android-Projekt (Jetpack Compose) und ein Rust-Core-Crate.

## Struktur
- `core/`: Rust Core (API, Models, URL-Builder). Baut `rlib` und `cdylib` für JNI.
- `mobile/android/`: Gradle-Projekt mit Compose-UI, ExoPlayer-Vorbereitung.

## Bauen (lokal)
1. Rust-Teil bauen (für Android-ABIs):
   - Installieren: `rustup target add aarch64-linux-android` (ggf. weitere ABIs)
   - Bauen der cdylib (Beispiel):
     - Verwende cargo-ndk oder Gradle NDK-Integration; die JNI-Brücke wird im nächsten Schritt ergänzt.

2. Android-App bauen:
   - Öffne `mobile/android` in Android Studio (mit NDK konfiguriert)
   - Sync & Build/Run auf Fire TV oder Emulator/Device

## Nächste Schritte
- JNI/UniFFI-Bindings implementieren:
  - In `core/` JNI-Exports bereitstellen (set_config, fetch_categories/items, fetch_series_episodes, build_stream_url)
  - In `android` `System.loadLibrary("macxtreamer_core")` und `external fun ...` deklarieren
- UI anbinden:
  - Tabs oben (Live/VOD/Series), Listen aus Rust-Daten befüllen
  - ExoPlayer für Playback
  - WorkManager für Downloads
