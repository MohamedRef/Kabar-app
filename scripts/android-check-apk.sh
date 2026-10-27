#!/usr/bin/env bash
# Fails loudly if an APK lacks Kabar's Kotlin glue from android/ (share target,
# ML Kit OCR), i.e. if dx did not pick up the `#[manganis::ffi]` module.
set -euo pipefail
APK="${1:-}"

# Failures become annotations on GitHub, where job logs are hard to reach.
fail() {
  for line in "$@"; do
    if [ -n "${GITHUB_ACTIONS:-}" ]; then echo "::error::$line"; else echo "$line"; fi
  done
  exit 1
}

[ -f "$APK" ] || fail "No APK found from dx bundle"

# Read everything first: `producer | grep -q` under pipefail fails whenever
# grep stops early and the producer gets SIGPIPE.
dex="$(mktemp)"
trap 'rm -f "$dex"' EXIT
unzip -p "$APK" 'classes*.dex' >"$dex"
missing=0
for cls in app/kabar/ShareActivity app/kabar/PendingShare app/kabar/OcrBridge app/kabar/ImagePickActivity app/kabar/ExportSaveActivity; do
  if ! LC_ALL=C grep -aqF "L$cls;" "$dex"; then
    echo "Missing $cls in $APK"
    [ -n "${GITHUB_ACTIONS:-}" ] && echo "::error::Missing $cls in the APK"
    missing=1
  fi
done
if [ "$missing" -ne 0 ]; then
  fail "dx did not build android/ into the APK. See docs/RUN.md, 'Kotlin glue'."
fi
echo "Kotlin glue present: share target + ML Kit OCR"

# Launcher icon: Kabar's, not dx's default (crates/app/AndroidManifest.xml).
# aapt2 ships with the Android build-tools; skip quietly where it isn't on PATH.
if command -v aapt2 >/dev/null; then
  badging="$(aapt2 dump badging "$APK")"
  manifest="$(aapt2 dump xmltree --file AndroidManifest.xml "$APK")"
  # Compare resource ids, not file names: a release build shortens resource
  # paths (res/mipmap-anydpi-v26/ic_kabar_launcher.xml becomes res/Xy.xml).
  icon_id="$(grep -o 'android:icon([^)]*)=@0x[0-9a-f]*' <<<"$manifest" | sed -n '1s/.*=@//p')"
  resources="$(aapt2 dump resources "$APK")"
  kabar_id="$(grep -E '^ *resource 0x[0-9a-f]+ mipmap/ic_kabar_launcher( |$)' <<<"$resources" | awk 'NR == 1 {print $2}')"
  if [ -z "$icon_id" ] || [ "$icon_id" != "$kabar_id" ]; then
    fail "The APK's launcher icon is not Kabar's (manifest icon ${icon_id:-none}, ic_kabar_launcher ${kabar_id:-missing})." \
      "Check [application] android_manifest in crates/app/Dioxus.toml."
  fi
  grep -q "TTS_SERVICE" <<<"$manifest" || fail "The manifest lost its text-to-speech <queries> block."
  # A debuggable APK runs slower and lets anyone with USB read Kabar's data.
  if grep -q "^application-debuggable" <<<"$badging"; then
    fail "The APK is debuggable: build it with scripts/android-release-apk.sh."
  fi
  if grep -q "versionCode='1'" <<<"$badging"; then
    fail "The APK's versionCode is still dx's 1."
  fi
  echo "Launcher icon: Kabar's; TTS queries present; release build ($(grep -o "versionCode='[0-9]*'" <<<"$badging"))"
fi
