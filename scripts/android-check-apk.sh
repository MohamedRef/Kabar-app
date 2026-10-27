#!/usr/bin/env bash
# Fails loudly if an APK lacks Kabar's Kotlin glue from android/ (share target,
# ML Kit OCR), i.e. if dx did not pick up the `#[manganis::ffi]` module.
set -euo pipefail
APK="${1:-}"
[ -f "$APK" ] || { echo "No APK found from dx bundle"; exit 1; }

missing=0
for cls in app/kabar/ShareActivity app/kabar/PendingShare app/kabar/OcrBridge app/kabar/ImagePickActivity; do
  if ! unzip -p "$APK" 'classes*.dex' | LC_ALL=C grep -aqF "L$cls;"; then
    echo "Missing $cls in $APK"
    missing=1
  fi
done
if [ "$missing" -ne 0 ]; then
  echo "dx did not build android/ into the APK. See docs/RUN.md, 'Kotlin glue'."
  exit 1
fi
echo "Kotlin glue present: share target + ML Kit OCR"

# Launcher icon: Kabar's, not dx's default (crates/app/AndroidManifest.xml).
# aapt2 ships with the Android build-tools; skip quietly where it isn't on PATH.
if command -v aapt2 >/dev/null; then
  badging="$(aapt2 dump badging "$APK")"
  if ! grep -q "^application:.*icon='[^']*ic_kabar_launcher" <<<"$badging"; then
    echo "The APK's launcher icon is not Kabar's:"
    grep '^application' <<<"$badging" || true
    echo "Check [application] android_manifest in crates/app/Dioxus.toml."
    exit 1
  fi
  if ! grep -q "TTS_SERVICE" <(aapt2 dump xmltree --file AndroidManifest.xml "$APK"); then
    echo "The manifest lost its text-to-speech <queries> block."
    exit 1
  fi
  echo "Launcher icon: Kabar's; TTS queries present"
fi
