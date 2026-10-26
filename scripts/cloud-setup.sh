#!/usr/bin/env bash
# One-time Kabar Cloud setup on your Cloudflare account (Workers Free plan).
# Safe to re-run: it skips what already exists. Needs `wrangler login` first.
set -euo pipefail

cd "$(dirname "$0")/../crates/server"
WRANGLER="${WRANGLER:-wrangler}"

echo "→ Checking you're logged in to Cloudflare"
"$WRANGLER" whoami >/dev/null

echo "→ D1 database 'kabar' (AI cache + free-tier counters)"
if ! grep -q '00000000-0000-0000-0000-000000000000' wrangler.toml; then
  echo "  already set in wrangler.toml"
else
  out="$("$WRANGLER" d1 create kabar 2>&1 || true)"
  id="$(printf '%s' "$out" | grep -oE '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' | head -1)"
  if [[ -z "$id" ]]; then
    id="$("$WRANGLER" d1 list --json | python3 -c 'import json,sys; print(next(d["uuid"] for d in json.load(sys.stdin) if d["name"]=="kabar"))')"
  fi
  sed -i.bak "s/00000000-0000-0000-0000-000000000000/$id/" wrangler.toml && rm wrangler.toml.bak
  echo "  created $id (commit wrangler.toml: the id is not a secret)"
fi
"$WRANGLER" d1 migrations apply kabar --remote

echo "→ Token signing key (Ed25519 seed, never leaves Cloudflare)"
if "$WRANGLER" secret list 2>/dev/null | grep -q TOKEN_SIGNING_KEY; then
  echo "  already set"
else
  openssl rand -base64 32 | tr -d '\n' | "$WRANGLER" secret put TOKEN_SIGNING_KEY
fi

echo "→ Allowlist: the Google account(s) allowed to sign in"
if "$WRANGLER" secret list 2>/dev/null | grep -q ALLOWED_EMAILS; then
  echo "  already set (change with: wrangler secret put ALLOWED_EMAILS)"
else
  read -r -p "  Your Gmail address: " email
  printf '%s' "$email" | "$WRANGLER" secret put ALLOWED_EMAILS
fi

cat <<'MSG'

Done. Next:
  1. Put your Google OAuth client ids in crates/server/wrangler.toml → GOOGLE_CLIENT_IDS
     (docs/CLOUD.md §3 shows how to create them).
  2. Optional keys:  wrangler secret put GEMINI_API_KEY   ·   wrangler secret put AZURE_SPEECH_KEY
  3. just deploy-cloud
MSG
