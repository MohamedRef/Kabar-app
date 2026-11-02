#!/usr/bin/env bash
# Once, on the Mac, before the first release from CI (docs/RELEASE.md §4):
# makes Kabar's release keys where the other scripts keep them (or reuses
# them), then copies them into this repository's Actions secrets so
# .github/workflows/release.yml can build, sign and publish. Run it again
# any time: it reuses every key that exists and only refreshes the secrets.
#
# | Secret                          | From                                            |
# |---------------------------------|-------------------------------------------------|
# | KABAR_UPDATE_SECRET             | Keychain kabar-update (just update-keygen)      |
# | KABAR_ANDROID_KEYSTORE          | ~/.kabar/kabar-release.jks, base64 (just android-keystore) |
# | KABAR_ANDROID_KEYSTORE_PASSWORD | Keychain kabar-android                          |
# | KABAR_MAC_CERT_P12              | ~/.kabar/kabar-codesign.p12, base64 (just mac-codesign-cert) |
# | KABAR_MAC_CERT_PASSWORD         | Keychain kabar-codesign                         |
#
# The values go from the Keychain straight into `gh secret set` on stdin:
# never into a file in the repo, an argument list or the terminal.
# Needs gh (logged in with access to the repo), a JDK (keytool) and cargo.
set -euo pipefail
cd "$(dirname "$0")/.."
repo="${KABAR_REPO:-MohamedRef/Kabar-app}"

command -v gh >/dev/null || { echo "install the GitHub CLI: brew install gh && gh auth login" >&2; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "log in first: gh auth login" >&2; exit 1; }

if ! security find-generic-password -s kabar-update >/dev/null 2>&1; then
  scripts/update-keygen.sh
fi
grep -q '^[A-Za-z0-9+/]\{43\}=$' crates/app/update-key.pub \
  || { echo "crates/app/update-key.pub holds no key, but the Keychain already has kabar-update: put its public half back first" >&2; exit 1; }
scripts/android-keystore.sh "$HOME/.kabar/kabar-release.jks" kabar
if [ ! -f "$HOME/.kabar/kabar-codesign.p12" ]; then
  if security find-identity -v -p codesigning | grep -q '"Kabar Code Signing"'; then
    echo "\"Kabar Code Signing\" is in the Keychain but ~/.kabar/kabar-codesign.p12 is missing: export it there (Keychain Access › Export) and store its password as kabar-codesign" >&2
    exit 1
  fi
  scripts/mac-codesign-cert.sh
fi

put() { gh secret set "$1" --repo "$repo" >/dev/null && echo "set $1"; }
printf %s "$(security find-generic-password -s kabar-update -w)" | put KABAR_UPDATE_SECRET
base64 < "$HOME/.kabar/kabar-release.jks" | tr -d '\n' | put KABAR_ANDROID_KEYSTORE
printf %s "$(security find-generic-password -s kabar-android -w)" | put KABAR_ANDROID_KEYSTORE_PASSWORD
base64 < "$HOME/.kabar/kabar-codesign.p12" | tr -d '\n' | put KABAR_MAC_CERT_P12
printf %s "$(security find-generic-password -s kabar-codesign -w)" | put KABAR_MAC_CERT_PASSWORD
gh secret list --repo "$repo"

echo
echo "Done. Commit crates/app/update-key.pub if it changed:"
grep -v '^#' crates/app/update-key.pub
echo "Back up ~/.kabar/ and the Keychain items kabar-update, kabar-android, kabar-codesign together."
