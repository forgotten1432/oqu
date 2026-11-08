#!/usr/bin/env bash
# Builds the Rust core for Android and regenerates the Kotlin bindings.
set -euo pipefail
cd "$(dirname "$0")/rust"
: "${ANDROID_HOME:=$HOME/Library/Android/sdk}"
: "${ANDROID_NDK_HOME:=$(ls -d "$ANDROID_HOME"/ndk/* | sort -V | tail -1)}"
export ANDROID_HOME ANDROID_NDK_HOME
JNI=../android/app/src/main/jniLibs
# PDFium (Chromium) prebuilt binaries; version must match the pdfium_8125 feature in Cargo.toml
PDFIUM_TAG="chromium%2F8125"
for pair in arm64:arm64-v8a arm:armeabi-v7a x64:x86_64; do
  arch=${pair%%:*}; abi=${pair##*:}
  if [ ! -f "$JNI/$abi/libpdfium.so" ]; then
    mkdir -p "$JNI/$abi" ../vendor/pdfium/$arch
    curl -fsSL "https://github.com/bblanchon/pdfium-binaries/releases/download/$PDFIUM_TAG/pdfium-android-$arch.tgz" | tar xz -C ../vendor/pdfium/$arch
    cp ../vendor/pdfium/$arch/lib/libpdfium.so "$JNI/$abi/"
  fi
done
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -P 26 -o "$JNI" build --release
# cargo-ndk also copies pdfium-render's own cdylib; the app only needs ours + libpdfium.so
find "$JNI" -name 'libpdfium_render-*.so' -delete
# Bindings come from an unstripped host build (release .so files are stripped)
cargo build --lib
LIB=target/debug/libreader_core.dylib; [ -f "$LIB" ] || LIB=target/debug/libreader_core.so
cargo run -q --features bindgen --bin uniffi-bindgen -- generate --library "$LIB" \
  --language kotlin --out-dir ../android/app/src/main/java --no-format
echo "Rust core built for Android."
