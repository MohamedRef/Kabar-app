#!/usr/bin/env bash
# Dry run of a content-only update (docs/RELEASE.md §5, "Publishing
# preflight"): everything publishing does, nothing it uploads. Runs on any
# machine with no secrets; signs with a throwaway key, never the release or
# preview key.
#
#   scripts/update-preflight.sh [channel]     (default: preview)
#
# 1. validate the source content            5. manifest (APK carried, not rebuilt)
# 2. build the bundle (device checks)        6. sign (throwaway key) and verify
# 3. ids against the bundle on the feed      7. report: sizes, digests, impact
# 4. what changed against the feed
#
# Exit status is non-zero when any step fails. Output: dist/preflight/.
set -euo pipefail
cd "$(dirname "$0")/.."

channel="${1:-preview}"
case "$channel" in preview | alpha | beta | stable) ;; *) echo "unknown channel $channel" >&2; exit 2 ;; esac
repo="${KABAR_UPDATE_REPO:-MohamedRef/Kabar-app}"
tag="$([[ $channel == preview ]] && echo preview || echo "update-$channel")"
tools="cargo run -q -p kabar-tools --"
out=dist/preflight
rm -rf "$out" && mkdir -p "$out/live" "$out/feed"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
step() { printf '\n== %s\n' "$*"; }

step "1. Source content"
$tools validate | tail -1

step "2. Bundle (the checks devices run before taking it)"
$tools update bundle --out "$out/bundle"

step "3–4. Against what the $channel feed publishes now"
live_manifest="$out/live/update.json"
base=()
carry=()
if curl -fsSL --retry 3 -o "$live_manifest" "https://github.com/$repo/releases/download/$tag/update.json"; then
  live_content="$(jq -r '.content.file // empty' "$live_manifest")"
  if [[ -n $live_content ]] && curl -fsSL --retry 3 -o "$out/live/$live_content" \
      "https://github.com/$repo/releases/download/$tag/$live_content"; then
    base=(--base-bundle "$out/live/$live_content")
  fi
  # A content-only update keeps every build the feed has: no APK is made.
  while read -r p; do carry+=(--carry "$p"); done < <(jq -r '.assets[].platform' "$live_manifest")
else
  echo "no $channel feed yet: nothing to compare with"
  rm -f "$live_manifest"
fi
if ((${#base[@]})); then
  $tools ids check "${base[@]}"
  $tools content diff "${base[@]}" --head-bundle "$out/bundle/kabar-content.bundle" | tee "$out/impact.md"
else
  echo "no published bundle: ids and impact compare with origin/main instead"
  $tools ids check --base origin/main
  $tools content diff --base origin/main | tee "$out/impact.md"
fi

step "5. Manifest (content-only: builds carried from the live feed)"
previous=()
[[ -f $live_manifest ]] && previous=(--previous "$live_manifest")
build="$( [[ -f $live_manifest ]] && jq -r '.build + 1' "$live_manifest" || echo 1)"
$tools update manifest --channel "$channel" --version "$version" --build "$build" \
  --commit "$(git rev-parse --short HEAD)" "${previous[@]}" --changes "${carry[@]}" \
  --content "$out/bundle/kabar-content.bundle" --out "$out/feed"

step "6. Sign with a throwaway key, verify like a device"
secret="$($tools update keygen --public-out "$out/throwaway.pub" --force 2>/dev/null)"
KABAR_UPDATE_SECRET="$secret" $tools update sign "$out/feed" --public-key "$out/throwaway.pub"
unset secret
carried=()
for c in "${carry[@]}"; do [[ $c == --carry ]] || carried+=(--carried "$c"); done
$tools update verify "$out/feed" --public-key "$out/throwaway.pub" "${carried[@]}"

step "7. Report"
jq -r '"| Field | Value |\n|---|---|",
  "| channel | \(.channel) |", "| version | \(.version) build \(.build) |",
  "| content file | \(.content.file) |", "| size | \(.content.size) bytes |",
  "| sha256 | \(.content.sha256) |", "| ui_build (stylesheets) | \(.content.ui_build) |",
  "| builds | \([.assets[] | "\(.platform) b\(.build // "?")"] | join(", ")) (carried, no APK built) |"' \
  "$out/feed/update.json" | tee "$out/report.md"
echo
echo "Dry run passed. Nothing was uploaded; the real feed is published by CI (preview) or 'just publish-update'."
