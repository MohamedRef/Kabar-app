#!/usr/bin/env bash
# Fails loudly if an APK lacks Kabar's Kotlin glue from android/ (share target,
# ML Kit OCR), i.e. if dx did not pick up the `#[manganis::ffi]` module.
set -euo pipefail
APK="${1:-}"
[ -f "$APK" ] || { echo "No APK found from dx bundle"; exit 1; }

missing=0
for cls in app/kabar/ShareActivity app/kabar/PendingShare app/kabar/OcrBridge; do
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
