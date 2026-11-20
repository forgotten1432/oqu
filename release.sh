#!/usr/bin/env bash
# Builds, signs and publishes a GitHub release in one go:
#   ./release.sh            # version taken from android/app/build.gradle.kts
#   ./release.sh --draft    # create the release as a draft (publish it on GitHub later)
#
# Needs: Android SDK + NDK, Rust + cargo-ndk (see README "Derleme"), the release
# key (~/.android/oqu-keystore.properties or $OQU_KEYSTORE_PROPS) and the GitHub
# CLI logged in (`gh auth login`). Release notes come from RELEASE_NOTES.md.
set -euo pipefail
cd "$(dirname "$0")"

DRAFT=()
[ "${1:-}" = "--draft" ] && DRAFT=(--draft)

VERSION=$(sed -n 's/.*oquVersionName.*?: "\([0-9.]*\)".*/\1/p' android/app/build.gradle.kts)
[ -n "$VERSION" ] || { echo "Could not read versionName from build.gradle.kts" >&2; exit 1; }
TAG="v$VERSION"

# A release signed with the debug key could not update installed copies.
PROPS="${OQU_KEYSTORE_PROPS:-$HOME/.android/oqu-keystore.properties}"
[ -f "$PROPS" ] || { echo "Release key not found: $PROPS" >&2; exit 1; }
command -v gh >/dev/null || { echo "GitHub CLI (gh) is not installed" >&2; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "Run 'gh auth login' first" >&2; exit 1; }
if gh release view "$TAG" >/dev/null 2>&1; then echo "Release $TAG already exists" >&2; exit 1; fi
if [ -n "$(git status --porcelain)" ]; then echo "Working tree is not clean; commit first" >&2; exit 1; fi

echo "==> Building $TAG"
./build-rust.sh
# build-rust.sh regenerates the Kotlin bindings; they must match what is committed
if [ -n "$(git status --porcelain -- android/app/src/main/java)" ]; then
  echo "Regenerated UniFFI bindings differ from the committed ones; commit them first:" >&2
  git status --short -- android/app/src/main/java >&2
  exit 1
fi
(cd android && ./gradlew --quiet clean assembleRelease)

OUT=android/app/build/outputs/apk/release
DIST=dist/$VERSION
rm -rf "$DIST" && mkdir -p "$DIST"
cp "$OUT/app-arm64-v8a-release.apk"   "$DIST/oqu-$VERSION-arm64-v8a.apk"
cp "$OUT/app-universal-release.apk"   "$DIST/oqu-$VERSION-universal.apk"
cp "$OUT/app-armeabi-v7a-release.apk" "$DIST/oqu-$VERSION-old-32bit-armeabi-v7a.apk"
cp "$OUT/app-x86_64-release.apk"      "$DIST/oqu-$VERSION-x86_64.apk"
# The in-app updater verifies downloads against this file.
(cd "$DIST" && if command -v sha256sum >/dev/null; then sha256sum oqu-*.apk; else shasum -a 256 oqu-*.apk; fi > SHA256SUMS.txt)

# Notes: this version's section of RELEASE_NOTES.md (up to the next "## " heading)
NOTES="$DIST/notes.md"
awk -v v="$VERSION" '
  /^## / { if (p) exit; if (index($0, " " v) && match($0, v "( |$)")) p = 1 }
  p { print }
' RELEASE_NOTES.md > "$NOTES"
[ -s "$NOTES" ] || { echo "No section for $VERSION in RELEASE_NOTES.md" >&2; exit 1; }

echo "==> Tagging and uploading $TAG"
git tag -a "$TAG" -m "Release $VERSION"
git push origin "$TAG"
gh release create "$TAG" "${DRAFT[@]}" \
  --title "ئوقۇ (Oqu) $VERSION" --notes-file "$NOTES" --verify-tag \
  "$DIST/oqu-$VERSION-arm64-v8a.apk#⭐ ÖNERİLEN / تەۋسىيە قىلىنغان — oqu-$VERSION-arm64-v8a.apk (çoğu telefon ve tablet)" \
  "$DIST/oqu-$VERSION-universal.apk#Tüm cihazlar (büyük dosya) — oqu-$VERSION-universal.apk" \
  "$DIST/oqu-$VERSION-old-32bit-armeabi-v7a.apk#Yalnızca eski 32 bit telefonlar — oqu-$VERSION-old-32bit-armeabi-v7a.apk" \
  "$DIST/oqu-$VERSION-x86_64.apk#Emülatör / Chromebook — oqu-$VERSION-x86_64.apk" \
  "$DIST/SHA256SUMS.txt#SHA256SUMS.txt"
echo "==> Done: $(gh release view "$TAG" --json url -q .url)"
