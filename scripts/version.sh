#!/usr/bin/env bash
# Kabar's version and build number (docs/RELEASE.md §1). Mohamed,
# 2026-11-08: calendar versions and one build counter.
#
#   scripts/version.sh release          the next release: YEAR.MONTH.N
#   scripts/version.sh preview          a preview: the latest release's version
#   scripts/version.sh apply VERSION    write VERSION into Cargo.toml
#
# release and preview print "VERSION BUILD" on one line.
#
# - VERSION of a release is YEAR.MONTH.N in Cairo time, N counting this
#   month's release tags (v2026.11.1, v2026.11.2, …) plus one. A preview
#   keeps the latest release's version (Cargo.toml's before the first one):
#   it is that release plus later work, told apart by its build.
# - BUILD is the number of commits on HEAD: one counter for previews and
#   releases that only grows on main. Android's versionCode is 20000000 +
#   BUILD (kabar_core::update::version_code), the Mac's CFBundleVersion is
#   BUILD.
# - apply writes VERSION into Cargo.toml, so the build (the app's own
#   label, the APK's versionName, the Mac's CFBundleShortVersionString)
#   carries it. CI only; never commit the result.
#
# Needs the full history and tags (actions/checkout with fetch-depth: 0).
set -euo pipefail
cd "$(dirname "$0")/.."

kind="${1:-}"
if [ "$kind" = apply ]; then
  version="${2:-}"
  [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "apply needs a version like 2026.11.1, got '$version'" >&2; exit 2; }
  # perl, not sed: the same on the Linux and macOS runners. Only the first
  # version line, [workspace.package]'s.
  perl -0pi -e "s/^version = \".*?\"/version = \"$version\"/m" Cargo.toml
  grep -q "^version = \"$version\"" Cargo.toml || { echo "could not set version in Cargo.toml" >&2; exit 1; }
  exit 0
fi
case "$kind" in release | preview) ;; *) echo "usage: scripts/version.sh release|preview|apply VERSION" >&2; exit 2 ;; esac

if [ "$(git rev-parse --is-shallow-repository)" = true ]; then
  echo "shallow clone: the build number needs the full history (fetch-depth: 0)" >&2
  exit 1
fi
build="$(git rev-list --count HEAD)"

# Release tags are vYEAR.MONTH.N; the older v1.0.0-alpha.N never match (four-digit year).
calendar_tags() { git tag -l "v$1" | grep -E '^v[0-9]{4}\.[0-9]{1,2}\.[0-9]+$' || true; }

if [ "$kind" = release ]; then
  ym="$(TZ=Africa/Cairo date +%Y.%-m)"
  last="$(calendar_tags "$ym.*" | sed "s/^v$ym\.//" | sort -n | tail -1)"
  version="$ym.$((${last:-0} + 1))"
else
  version="$(calendar_tags '*' | sed 's/^v//' | sort -t. -k1,1n -k2,2n -k3,3n | tail -1)"
  [ -n "$version" ] || version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
fi

echo "$version $build"
