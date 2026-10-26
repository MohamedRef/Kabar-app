#!/usr/bin/env bash
# Uploads the offline packs built by `kabar-tools packs bundle` (dist/packs/)
# to the GitHub release the app downloads from (free: GitHub Releases).
# Run on the Mac after `gh auth login`. Usage: scripts/packs-upload.sh [tag]
set -euo pipefail

tag="${1:-packs}"
dir="dist/packs"
repo="${KABAR_PACKS_REPO:-MohamedRef/Kabar-app}"

[[ -f "$dir/index.json" ]] || { echo "no $dir/index.json: run 'just packs-bundle' first" >&2; exit 1; }
command -v gh >/dev/null || { echo "install the GitHub CLI: brew install gh && gh auth login" >&2; exit 1; }

if ! gh release view "$tag" --repo "$repo" >/dev/null 2>&1; then
  gh release create "$tag" --repo "$repo" --title "Kabar offline packs" \
    --notes "Pre-generated audio per unit (Azure neural voices). Downloaded by the app from Settings → Offline packs. Rebuilt with 'just packs-bundle'; never edit by hand." \
    --latest=false
fi

# Tars first, the index last: a device that reads the new index always finds the tars it names.
shopt -s nullglob
tars=("$dir"/*.tar)
(( ${#tars[@]} )) || { echo "no packs in $dir (nothing fully synthesized yet?)" >&2; exit 1; }
gh release upload "$tag" "${tars[@]}" --repo "$repo" --clobber
gh release upload "$tag" "$dir/index.json" --repo "$repo" --clobber
echo "uploaded ${#tars[@]} pack(s) to https://github.com/$repo/releases/tag/$tag"
