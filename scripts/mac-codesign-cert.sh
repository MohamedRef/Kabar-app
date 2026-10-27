#!/usr/bin/env bash
# Once, on the Mac: a free, self-signed "Kabar Code Signing" certificate in
# the login Keychain. `just bundle-mac` signs Kabar.app with it, so every
# build keeps the same code identity: the Keychain keeps trusting Kabar after
# an update (the Kabar Cloud sign-in), and the updater checks a new
# Kabar.app comes from the same signer (docs/RELEASE.md §5). No Apple
# Developer account needed. macOS asks for your password once to trust it.
set -euo pipefail
NAME="Kabar Code Signing"

if security find-identity -v -p codesigning | grep -q "\"$NAME\""; then
  echo "\"$NAME\" is already in the Keychain."
  exit 0
fi
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cat >"$tmp/cert.cnf" <<CNF
[req]
distinguished_name = dn
x509_extensions = ext
prompt = no
[dn]
CN = $NAME
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
CNF
openssl req -x509 -newkey rsa:2048 -nodes -days 7300 -config "$tmp/cert.cnf" \
  -keyout "$tmp/key.pem" -out "$tmp/cert.pem" 2>/dev/null
pass="$(openssl rand -hex 16)"
openssl pkcs12 -export -inkey "$tmp/key.pem" -in "$tmp/cert.pem" -name "$NAME" \
  -out "$tmp/kabar.p12" -passout "pass:$pass" 2>/dev/null
security import "$tmp/kabar.p12" -k "$HOME/Library/Keychains/login.keychain-db" -P "$pass" -T /usr/bin/codesign
security add-trusted-cert -r trustRoot -p codeSign -k "$HOME/Library/Keychains/login.keychain-db" "$tmp/cert.pem"
security find-identity -v -p codesigning | grep "\"$NAME\""
echo "Done. Next 'just bundle-mac' signs Kabar.app as \"$NAME\"."
