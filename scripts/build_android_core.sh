#!/usr/bin/env bash
set -euo pipefail
# Builds Rust core for Android ABIs and copies into app/src/main/jniLibs
# Requires: cargo-ndk (cargo install cargo-ndk), Android NDK installed and ANDROID_NDK_HOME set
cd "$(dirname "$0")/.."
TARGETS=(arm64-v8a armeabi-v7a x86_64)
OUTDIR="mobile/android/app/src/main/jniLibs"
mkdir -p "$OUTDIR"
for abi in "${TARGETS[@]}"; do
  echo "Building for $abi"
  cargo ndk -t "$abi" -o "$OUTDIR" build -p macxtreamer_core --release
done
echo "Done."
