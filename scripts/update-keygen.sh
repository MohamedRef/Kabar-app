#!/usr/bin/env bash
# Creates Kabar's update signing key (Ed25519) once, on the Mac. The public
# half goes into crates/app/update-key.pub (commit it: builds trust it); the
# secret half goes straight into the macOS Keychain (service "kabar-update")
# and never touches a file. docs/RELEASE.md §5.
set -euo pipefail
cd "$(dirname "$0")/.."

if security find-generic-password -s kabar-update >/dev/null 2>&1; then
  echo "The Keychain already holds a kabar-update key. Keep using it: installed builds trust it." >&2
  exit 0
fi
secret="$(cargo run -q --release -p kabar-tools -- update keygen)"
security add-generic-password -U -s kabar-update -a "$USER" -w "$secret"
unset secret
echo "Stored the secret in the Keychain (kabar-update). Commit crates/app/update-key.pub."
echo "Back the secret up with the Android key (MAC-TODO §3): security find-generic-password -s kabar-update -w"
