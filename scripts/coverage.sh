#!/usr/bin/env bash
# Line coverage of the whole workspace, the same number CI's `coverage` job
# checks (docs/TESTING.md). Runs `cargo test --workspace` plus the browser
# fixture exporters CI runs, under cargo-llvm-cov, and fails when line
# coverage is below the floor in scripts/coverage-floor.
#
#   scripts/coverage.sh            summary per file + total, checks the floor
#   scripts/coverage.sh --html     also writes target/llvm-cov/html/index.html
#   scripts/coverage.sh --lcov F   also writes an lcov file to F
#
# Needs: rustup component add llvm-tools-preview; cargo install cargo-llvm-cov --locked
set -euo pipefail
cd "$(dirname "$0")/.."

APP=kabar-app
floor="$(tr -d '[:space:]' < scripts/coverage-floor)"
fixtures="${CARGO_TARGET_DIR:-target}/coverage-fixtures"

cargo llvm-cov clean --workspace
cargo llvm-cov --no-report --workspace --locked
# The browser jobs render real screens through these ignored exporters; the
# code they reach is tested there, so it counts here too.
APP_BROWSER_FIXTURES="$fixtures" cargo llvm-cov --no-report -p "$APP" --no-default-features --locked \
  -- --ignored export_

cargo llvm-cov report --summary-only
case "${1:-}" in
  --html) cargo llvm-cov report --html ;;
  --lcov) cargo llvm-cov report --lcov --output-path "$2" ;;
esac

total="$(cargo llvm-cov report --json --summary-only | python3 -c 'import json,sys; print("%.2f" % json.load(sys.stdin)["data"][0]["totals"]["lines"]["percent"])')"
echo "Line coverage: ${total}% (floor ${floor}%)"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  echo "### Line coverage: ${total}% (floor ${floor}%)" >> "$GITHUB_STEP_SUMMARY"
fi
python3 -c "import sys; sys.exit(0 if float('$total') >= float('$floor') else 1)" || {
  echo "Line coverage ${total}% is below the floor ${floor}%: add tests for the code this change adds (docs/TESTING.md)." >&2
  exit 1
}
