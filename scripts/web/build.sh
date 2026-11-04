#!/usr/bin/env bash
# Builds the Kabar web app (docs/WEB.md) into target/web/kabar: a static site
# (index.html, kabar.js, kabar_bg.wasm, sw.js) any static host can serve.
#   scripts/web/build.sh            release build
#   scripts/web/build.sh --dev      faster, unoptimised
# Needs the wasm32 target (rust-toolchain.toml) and wasm-bindgen-cli at the
# same version as Cargo.lock's wasm-bindgen; wasm-opt (binaryen) is used when
# it is installed.
# adapted from batna-app main: scripts/web/build.sh
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
profile=web-release
[[ "${1:-}" == "--dev" ]] && profile=dev
out="$root/target/web/kabar"
want=$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/version = |"/, ""); print; exit}' Cargo.lock)
have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')
if [[ "$have" != "$want" ]]; then
  echo "wasm-bindgen-cli $want is needed (found '${have:-none}'): cargo install wasm-bindgen-cli@$want --locked" >&2
  exit 1
fi
cargo build -p kabar-app --bin kabar --target wasm32-unknown-unknown \
  --no-default-features --features web --profile "$profile"
dir=$([[ $profile == dev ]] && echo debug || echo "$profile")
rm -rf "$out" && mkdir -p "$out"
wasm-bindgen --target web --no-typescript --out-dir "$out" --out-name kabar \
  "target/wasm32-unknown-unknown/$dir/kabar.wasm"
if [[ $profile != dev ]] && command -v wasm-opt >/dev/null; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-reference-types \
    "$out/kabar_bg.wasm" -o "$out/kabar_bg.wasm"
fi
cp web/index.html web/manifest.webmanifest web/_headers "$out/"
cp assets/export/web/favicon-32.png assets/export/web/icon-192.png assets/export/web/icon-512.png \
  assets/export/web/maskable-512.png "$out/"
# The service worker keeps exactly this build's files: its name is their
# hash, so any change makes a new worker the browser installs in the background.
files=$(cd "$out" && find . -type f ! -name _headers ! -name index.html | sed 's|^\./||' | sort)
build=$(cd "$out" && sha256sum index.html $files | sha256sum | cut -c1-16)
list=$(printf '"./"'; for f in $files; do printf ', "./%s"' "$f"; done)
sed -e "s|__BUILD__|$build|" -e "s|\"__FILES__\"|$list|" web/sw.js > "$out/sw.js"
echo "Kabar web: $out ($(du -sh "$out" | cut -f1))"
# Cloudflare's static assets take files up to 25 MiB (docs/WEB.md § Size).
if [[ $profile != dev ]]; then
  size=$(wc -c < "$out/kabar_bg.wasm")
  limit=$((25 * 1024 * 1024))
  echo "kabar_bg.wasm: $size bytes ($((size * 100 / limit))% of the 25 MiB limit)"
  if (( size > limit )); then
    echo "kabar_bg.wasm is over Cloudflare's 25 MiB per-file limit: see docs/WEB.md § Size" >&2
    exit 1
  fi
fi
