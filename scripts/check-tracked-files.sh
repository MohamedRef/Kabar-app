#!/usr/bin/env bash
# Fails if git tracks build output: a target*/ directory, compiled artifacts,
# or any file over 5 MB (the biggest real source file, a map contact sheet,
# is ~3.5 MB). Build into target/ or a directory outside the checkout.
set -euo pipefail
cd "$(dirname "$0")/.."
LIMIT=$((5 * 1024 * 1024))
bad=0

files="$(git ls-files)"
built="$(grep -E '(^|/)target[^/]*/|\.(rlib|rmeta|o|a|so|dylib|dll|exe|pdb|apk|aab|dex|class|wasm)$' <<<"$files" || true)"
if [ -n "$built" ]; then
  echo "Build output is tracked ($(wc -l <<<"$built") files), e.g.:"
  head -5 <<<"$built"
  bad=1
fi

big="$(git ls-files -s | awk '{print $2}' | git cat-file --batch-check='%(objectsize) %(objectname)' \
  | paste -d' ' - <(git ls-files) | awk -v limit="$LIMIT" '$1 > limit {size = $1; $1 = $2 = ""; sub(/^ +/, ""); print size, $0}')"
if [ -n "$big" ]; then
  echo "Files over 5 MB are tracked:"
  head -10 <<<"$big"
  bad=1
fi

[ "$bad" -eq 0 ] || { echo "Remove them with git rm --cached and add them to .gitignore."; exit 1; }
echo "Tracked files: no build output, nothing over 5 MB"
