#!/usr/bin/env bash
# Publishes the release that `just release` left in dist/release/<version>/
# as an over-the-air update (docs/RELEASE.md §5): one signed feed per channel
# the version belongs to (an alpha → alpha; a beta → alpha + beta; a release
# → all three), each on its own GitHub release (update-alpha, …).
#
# Only once the learner says go. Needs `gh auth login` and the Keychain key
# from `just update-keygen`. Usage: scripts/update-publish.sh "<notes en>" "<notes ar-EG>"
set -euo pipefail
cd "$(dirname "$0")/.."

notes_en="${1:-}"
notes_ar_eg="${2:-}"
repo="${KABAR_UPDATE_REPO:-MohamedRef/Kabar-app}"
v="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
rel="dist/release/$v"
tools="cargo run -q --release -p kabar-tools --"

[[ -d "$rel/Kabar.app" && -f "$rel/kabar-$v.apk" ]] || { echo "no $rel: run 'just release' first" >&2; exit 1; }
command -v gh >/dev/null || { echo "install the GitHub CLI: brew install gh && gh auth login" >&2; exit 1; }
secret="$(security find-generic-password -s kabar-update -w)" || { echo "no update key: run 'just update-keygen'" >&2; exit 1; }

case "$(uname -m)" in
  arm64) mac=macos-arm64 ;;
  x86_64) mac=macos-x86_64 ;;
  *) echo "unknown Mac architecture $(uname -m)" >&2; exit 1 ;;
esac
# ditto keeps the bundle's symlinks, modes and signature; the app unzips with ditto too.
rm -f "$rel/Kabar.app.zip"
ditto -c -k --keepParent "$rel/Kabar.app" "$rel/Kabar.app.zip"

published=0
for channel in alpha beta stable; do
  out="dist/update/$channel"
  # What's new: changes/ since the version this channel carries now, on top
  # of its history (none yet on a first publish).
  previous="$(mktemp)"
  gh release download "update-$channel" --repo "$repo" --pattern update.json --output "$previous" --clobber \
    2>/dev/null || rm -f "$previous"
  # The tool refuses a version that doesn't belong on a channel (an alpha on stable).
  if ! $tools update manifest --channel "$channel" --version "$v" --commit "$(git rev-parse --short HEAD)" \
      --notes-en "$notes_en" --notes-ar-eg "$notes_ar_eg" --previous "$previous" --changes \
      --asset "android-arm64=$rel/kabar-$v.apk" --asset "$mac=$rel/Kabar.app.zip" --out "$out" 2>/dev/null; then
    continue
  fi
  KABAR_UPDATE_SECRET="$secret" $tools update sign "$out"
  $tools update verify "$out"
  tag="update-$channel"
  if ! gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
    gh release create "$tag" --repo "$repo" --prerelease --latest=false --title "Kabar updates ($channel)" \
      --notes "Kabar's $channel update feed. The app reads update.json, checks its signature and each file's SHA-256. Published with 'just publish-update'; never edit by hand."
  fi
  # Builds first, the signed manifest last: a device that reads the new
  # manifest always finds the files it names.
  shopt -s nullglob
  gh release upload "$tag" "$out"/kabar-* --repo "$repo" --clobber
  gh release upload "$tag" "$out/update.json.sig" "$out/update.json" --repo "$repo" --clobber
  echo "published $v on $channel: https://github.com/$repo/releases/tag/$tag"
  published=$((published + 1))
done
unset secret
(( published )) || { echo "$v belongs on no channel?" >&2; exit 1; }
