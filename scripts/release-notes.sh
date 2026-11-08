#!/usr/bin/env bash
# The GitHub release page for a Kabar release (docs/RELEASE.md §4): what's
# new from the signed feed's newest history entry (the changes/ lines merged
# since the last release), how to install, and the checksums.
#
#   scripts/release-notes.sh FEED/update.json CHANNEL [NOTES_EN] [NOTES_AR_EG]
#
# Prints Markdown. The version, build and commit come from the manifest.
set -euo pipefail

manifest="${1:?usage: scripts/release-notes.sh FEED/update.json CHANNEL [notes-en] [notes-ar-eg]}"
channel="${2:?channel}"
notes_en="${3:-}"
notes_ar_eg="${4:-}"
v="$(jq -r .version "$manifest")"
build="$(jq -r .build "$manifest")"
commit="$(jq -r '.commit // ""' "$manifest")"

echo "**Kabar $v** · build $build${commit:+ · \`$commit\`} · $channel channel"
echo
if [ -n "$notes_en$notes_ar_eg" ]; then
  [ -z "$notes_ar_eg" ] || echo "$notes_ar_eg"
  [ -z "$notes_en" ] || echo "$notes_en"
  echo
fi
echo "## What's new"
echo
# The newest entry is this release's; a release with no changes/ lines says so.
lines="$(jq -r '
  def section($k; $title):
    [.history[0].changes[]? | select(.kind == $k)] as $c
    | if ($c | length) > 0 then
        "### \($title)\n\n" + ($c | map("- " + .ar_eg + (if (.en // "") != "" then "<br>_" + .en + "_" else "" end)) | join("\n")) + "\n"
      else empty end;
  [section("new"; "New"), section("improved"; "Improved"), section("fixed"; "Fixed")] | join("\n")
' "$manifest")"
if [ -n "$lines" ]; then echo "$lines"; else echo "Small fixes and housekeeping."; fi
echo
cat <<MD
## Install

| Device | File | First time only |
|---|---|---|
| Android (S24 Ultra, Tab S11) | \`Kabar-$v-android.apk\` | Open it on the device and allow installs from your browser. It installs over any earlier Kabar and keeps your data. |
| Mac (Apple silicon) | \`Kabar-$v-mac.zip\` | Unzip, move Kabar.app to ~/Applications, then run \`xattr -dr com.apple.quarantine ~/Applications/Kabar.app\` once (it is self-signed). |

Installed apps update themselves from the $channel feed; you only need these files for a new device.

\`SHA256SUMS.txt\` lists each file's SHA-256 (\`shasum -a 256 -c SHA256SUMS.txt\`).
MD
