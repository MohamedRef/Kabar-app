#!/usr/bin/env bash
# Rebuilds the APK that `dx bundle --platform android --release` left behind
# as a real release build, and prints the unsigned APK's path.
#
# dx 0.7.10 only runs gradle's assembleRelease when Dioxus.toml carries the
# keystore and its passwords ([bundle.android]), which would put a secret in
# the repo. Without them it runs assembleDebug, so the APK is debuggable, has
# no R8, and its versionCode is always 1. This script reuses the gradle
# project dx generated, so the Rust library and the Kotlin glue are the same:
#
# - versionCode comes from the workspace version (see version_code below);
# - only arm64 native libraries, run straight from the APK;
# - our R8 keep rules go next to dx's (android/consumer-rules.pro covers
#   app.kabar.*; the ones here cover dx's own classes that wry calls by name);
# - assembleRelease builds a non-debuggable, R8-shrunk APK, left unsigned
#   for scripts/android-sign.sh (or CI's preview key) to sign.
set -euo pipefail
cd "$(dirname "$0")/.."

ROOT="$(find target/dx -path '*/release/android/app/gradlew' -print -quit 2>/dev/null | xargs -r dirname)"
[ -n "$ROOT" ] || { echo "No dx release gradle project under target/dx; run dx bundle --release first" >&2; exit 1; }
GRADLE_APP="$ROOT/app/build.gradle.kts"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

# 1.2.3-alpha.4 → 10203104: major, minor, patch, then the stage (alpha
# 100+N, beta 500+N, rc 800+N, a final release 999), so every alpha sorts
# before its beta and the beta before the release.
version_code() {
  local v="$1" core pre major minor patch stage n
  core="${v%%-*}"
  pre=""
  [ "$core" != "$v" ] && pre="${v#*-}"
  IFS=. read -r major minor patch <<<"$core"
  n="${pre##*.}"
  case "$pre" in
    "") stage=999 ;;
    alpha.*) stage=$((100 + n)) ;;
    beta.*) stage=$((500 + n)) ;;
    rc.*) stage=$((800 + n)) ;;
    *) echo "Unknown pre-release '$pre' in version $v" >&2; return 1 ;;
  esac
  echo $(((major * 10000 + minor * 100 + patch) * 1000 + stage))
}
CODE="$(version_code "$VERSION")"

sed -i.bak -E "s/versionCode = [0-9]+/versionCode = $CODE/" "$GRADLE_APP"
rm -f "$GRADLE_APP.bak"
grep -q "versionCode = $CODE" "$GRADLE_APP" || { echo "Could not set versionCode in $GRADLE_APP" >&2; exit 1; }

# arm64 only (the S24 Ultra and Tab S11; ML Kit also ships x86, x86_64 and
# armv7 copies of its OCR pipeline, ~30 MB unpacked), stored uncompressed and
# page-aligned so Android maps them from the APK (extractNativeLibs="false"
# in crates/app/AndroidManifest.xml) instead of copying them out at install.
if ! grep -q "kabar: native libs" "$GRADLE_APP"; then
  cat >>"$GRADLE_APP" <<'EOF'

// kabar: native libs (scripts/android-release-apk.sh)
android {
    defaultConfig {
        ndk {
            abiFilters.clear()
            abiFilters += "arm64-v8a"
        }
    }
    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }
}
EOF
fi

# dx's build.gradle.kts applies every *.pro file in the app module.
cat >"$ROOT/app/kabar-keep.pro" <<'EOF'
# wry and dx's generated activity are reached from Rust over JNI by name.
-keep class dev.dioxus.main.** { *; }
-keep class app.kabar.** { *; }
-keepclasseswithmembernames,includedescriptorclasses class * { native <methods>; }
EOF

(cd "$ROOT" && ./gradlew --quiet assembleRelease >&2)

APK="$ROOT/app/build/outputs/apk/release/app-release-unsigned.apk"
[ -f "$APK" ] || APK="$(find "$ROOT/app/build/outputs/apk/release" -name '*.apk' -print -quit)"
[ -f "$APK" ] || { echo "assembleRelease produced no APK" >&2; exit 1; }
echo "versionName $VERSION, versionCode $CODE" >&2
echo "$APK"
