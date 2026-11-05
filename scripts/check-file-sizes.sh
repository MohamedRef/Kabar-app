#!/usr/bin/env bash
# Warns (never fails) about Rust files over 800 lines, so a growing file gets
# split before parallel PRs keep colliding in it. The rule is in AGENTS.md
# § Small files. Set FILE_SIZE_STRICT=1 to make it fail instead.
set -euo pipefail
cd "$(dirname "$0")/.."
LIMIT=800
big="$(git ls-files '*.rs' | xargs wc -l | awk -v limit="$LIMIT" '$2 != "total" && $1 > limit' | sort -rn)"
if [ -z "$big" ]; then
  echo "File sizes: every Rust file is at most $LIMIT lines"
  exit 0
fi
echo "File sizes: $(wc -l <<<"$big") Rust files are over $LIMIT lines (AGENTS.md § Small files)."
echo "Split one when you are already changing it a lot; never only to silence this list:"
sed 's/^/  /' <<<"$big"
if [ "${GITHUB_ACTIONS:-}" = true ]; then
  while read -r n f; do echo "::warning file=$f::$n lines (over $LIMIT); see AGENTS.md § Small files"; done <<<"$big"
fi
[ "${FILE_SIZE_STRICT:-0}" = 1 ] && exit 1 || exit 0
