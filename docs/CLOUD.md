# Kabar Cloud (M2)

Kabar Cloud is a Rust Cloudflare Worker (`crates/server`) on the **Workers Free** plan. It does five things:

1. **Sign-in**: verifies your Google ID token, checks the allowlist, and issues Kabar tokens (15-min access, 60-day rotating refresh).
2. **Devices**: lists every signed-in device with its last sync; signs one out remotely (its socket closes at once).
3. **Live sync**: one Durable Object per account is the WebSocket hub (hibernation, so idle sockets cost nothing).
4. **Key vault**: Gemini, Azure and Anthropic keys live only in Worker secrets. Devices never see them.
5. **AI/TTS proxy**: Workers AI (SEA-LION v4 27B), Gemini free tier, Claude (off), Azure neural TTS, with a shared cache and hard stops under every free limit.

Cost: **$0**. Nothing here needs a card on file.

## 1. Layout

| Piece | Where | Notes |
|---|---|---|
| Worker | `crates/server/src/wasm/mod.rs` | routing, auth, proxy calls; thin to stay inside 10 ms CPU |
| Durable Object `Account` | `crates/server/src/wasm/account.rs` | devices + op log + row versions in DO SQLite |
| Sync hub logic | `crates/server/src/hub.rs` | plain Rust, tested against real SQLite |
| Merge rules | `crates/core/src/sync.rs` | shared with the app, so both converge the same way |
| Wire types | `crates/protocol/src/{sync,cloud,ai}.rs` | versioned (`PROTOCOL_VERSION`, `AI_WIRE_VERSION`) |
| D1 `kabar` | `crates/server/migrations/` | `ai_cache`, `quota_usage` |

Devices live in the account's Durable Object rather than D1 (a small change to PLAN.md §5/§6): a remote sign-out and closing that device's socket then happen in one place, with no second database to keep consistent.

## 2. API

All JSON. Authenticated routes take `Authorization: Bearer <access_token>`. Errors are `{ "code": "...", "message": "..." }` (`ErrorCode` in `crates/protocol/src/cloud.rs`); `/v1/ai` errors are the router's `EngineError`.

| Route | What |
|---|---|
| `GET /v1/health` | `{ name, protocol }` |
| `POST /v1/auth/google` | `{ id_token, device }` → `Session` |
| `POST /v1/auth/refresh` | `{ refresh_token }` → new `Session` (the old refresh token stops working) |
| `POST /v1/auth/signout` | signs this device out |
| `GET /v1/devices` | Devices screen: `{ devices: [DeviceSummary] }` |
| `DELETE /v1/devices/{id}` | remote sign-out |
| `GET /v1/sync` | WebSocket; messages in `crates/protocol/src/sync.rs` |
| `GET /v1/vault` | which engines are configured (never the keys) |
| `POST /v1/ai` | `EngineCall` → `EngineReply` |
| `POST /v1/tts` | `{ segments: [{ voice, text }], rate_percent }` → `audio/mpeg` |

Sync in one paragraph: the device sends `hello { last_seq }`, gets the missing ops in pages and `caught_up`, then pushes its outbox as `push { batch, ops ≤ 50 }` and gets an `ack` listing what can leave the outbox. Other devices' ops arrive as `ops` the moment they're stored. LWW tables keep the newest HLC, append-only tables insert once, ink strokes only take tombstones. Unknown tables, wrong device ids and clocks more than 5 minutes ahead are rejected with a reason.

## 3. Setup (once, about 20 minutes)

### 3.1 Cloudflare
1. Create a free account at <https://dash.cloudflare.com/sign-up> (no card).
2. On the Mac:
   ```bash
   just cloud-tools      # wrangler 4 + worker-build
   wrangler login        # opens the browser
   just cloud-setup      # D1 database, token signing key, your Gmail on the allowlist
   ```
   `cloud-setup` writes the D1 id into `crates/server/wrangler.toml`. Commit that change; the id is not a secret.

### 3.2 Google sign-in (Google Cloud Console, free)
1. <https://console.cloud.google.com/> → create a project "Kabar".
2. **Google Auth Platform → Branding/Audience**: External, keep it in *Testing*, add your Gmail as a test user.
3. **Clients → Create client**, three times:
   - **Web application** "Kabar server". Android's Credential Manager puts this id in the token's `aud`.
   - **Android** "Kabar Android": package `app.kabar`, SHA-1 from `keytool -list -v -keystore ~/.kabar/kabar-release.jks -alias kabar`.
   - **Desktop app** "Kabar Mac" (used with PKCE + loopback).
4. Put the **web** and **desktop** client ids in `crates/server/wrangler.toml`:
   ```toml
   GOOGLE_CLIENT_IDS = "1234-web.apps.googleusercontent.com,1234-desktop.apps.googleusercontent.com"
   ```

### 3.3 Optional keys (each one switches an engine on)
```bash
cd crates/server
wrangler secret put GEMINI_API_KEY     # aistudio.google.com → Get API key (free tier)
wrangler secret put AZURE_SPEECH_KEY   # Azure Speech resource on F0; set AZURE_SPEECH_REGION in wrangler.toml
# Only in months with Max credit, and set CLAUDE_ENABLED = "true":
wrangler secret put ANTHROPIC_API_KEY
```
Workers AI needs no key.

### 3.4 Deploy
```bash
just deploy-cloud
curl https://kabar-cloud.<your-subdomain>.workers.dev/v1/health
```
Tell me the URL; the app will point at it.

## 4. Local development

```bash
just cloud-e2e   # local workerd + fake Google: phone → tablet sync, devices, sign-out, privacy rule
just cloud-dev   # http://127.0.0.1:8787 (Workers AI calls need `wrangler login`)
just cloud-logs  # live logs from the deployed Worker
```

## 5. Free-tier hard stops

Counters are in D1 `quota_usage`; a call that would cross a limit is refused before it's made (`quota_exhausted` with `retry_after_s` until the UTC reset). Change them in `wrangler.toml`.

| Service | Default stop | Free limit |
|---|---|---|
| Workers AI | 300 requests/day | 10,000 Neurons/day |
| Gemini free | 1,000 requests/day | ≈1,500/day (unofficial) |
| Azure TTS | 450,000 chars/month | 500,000 chars/month |
| Claude | 300 requests/month, **off** | paid unless Max credit |

## 6. Privacy

- `task.is_personal()` + `gemini_free` is refused by the server too, not only by the router.
- Personal answers never go into the shared D1 cache.
- Keys are Worker secrets; `/v1/vault` only says whether they exist.
- A signed-out device's access token is refused by the AI/TTS proxy at once (checked against its Durable Object), not 15 minutes later.

## 7. Known gaps (phased, not dropped)

- Vision tasks (`structure_screenshot`, `read_handwriting`, …) answer `unavailable` until the blob store lands.
- FCM push and the 15-minute cron dispatcher arrive with M7 (reminders).
- Op-log compaction (monthly snapshot) arrives once the log is big enough to matter.
- `fixtures/server/*.json` are written from the providers' documented shapes; replace them with real recordings on the first bake-off run.
