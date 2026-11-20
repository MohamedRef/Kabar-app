#!/usr/bin/env bash
# Adapted from Steps-app main: scripts/android-bundle.sh.
# Builds the Android APK with dx, from crates/app, into <out-dir>.
#
# Gradle fetches its plugins and libraries from Maven while it builds, and
# Maven sometimes answers "Could not find" for files it has for several
# minutes at a time (2026-11-20: a release and a pull request in the same
# morning). When that is why the build failed, this waits and tries again;
# Rust is built by then, so a retry is only the Gradle part. Any other
# failure stops at once.
set -uo pipefail

out="${1:?usage: android-bundle.sh <out-dir>}"
waits=(60 180)
log="$(mktemp)"
trap 'rm -f "$log"' EXIT

for attempt in 1 2 3; do
  dx bundle --platform android --release --package-types apk \
    --target aarch64-linux-android --no-default-features --features mobile \
    --out-dir "$out" 2>&1 | tee "$log"
  status=${PIPESTATUS[0]}
  [ "$status" -eq 0 ] && exit 0
  if ! grep -qE "Could not resolve all (artifacts|dependencies|files)|Could not (GET|HEAD) 'https://" "$log"; then
    exit "$status"
  fi
  if [ "$attempt" -lt 3 ]; then
    wait="${waits[$((attempt - 1))]}"
    echo "::warning::Gradle couldn't fetch from Maven (attempt $attempt); trying again in ${wait}s."
    sleep "$wait"
  fi
done
echo "::error::Gradle couldn't fetch from Maven three times."
exit "$status"
