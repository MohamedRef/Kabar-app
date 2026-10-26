# Kabar Cloud (M2, reminders from M7)

Kabar Cloud is a Rust Cloudflare Worker (`crates/server`) on the **Workers Free** plan. It does six things:

1. **Sign-in**: verifies your Google ID token, checks the allowlist, and issues Kabar tokens (15-min access, 60-day rotating refresh).
2. **Devices**: lists every signed-in device with its last sync; signs one out remotely (its socket closes at once).
3. **Live sync**: one Durable Object per account is the WebSocket hub (hibernation, so idle sockets cost nothing).
4. **Key vault**: Gemini, Azure and Anthropic keys live only in Worker secrets. Devices never see them.
5. **AI/TTS proxy**: Workers AI (SEA-LION v4 27B), Gemini free tier, Claude (off), Azure neural TTS, with a shared cache and hard stops under every free limit.
6. **Kak Nur's reminders (M7)**: a Durable Object alarm every 15 minutes runs the coach's reminder rules on the synced data and pushes through Firebase Cloud Messaging.

Cost: **$0**. Nothing here needs a card on file.

## 1. Layout

| Piece | Where | Notes |
|---|---|---|
| Worker | `crates/server/src/wasm/mod.rs` | routing, auth, proxy calls; thin to stay inside 10 ms CPU |
| Durable Object `Account` | `crates/server/src/wasm/account.rs` | devices + op log + row versions in DO SQLite |
| Sync hub logic | `crates/server/src/hub.rs` | plain Rust, tested against real SQLite |
| Reminder dispatcher | `crates/server/src/coach.rs` | reads coach rows, calls `kabar_core::coach` rules |
| FCM HTTP v1 | `crates/server/src/fcm.rs` | service-account JWT (RS256), token cache, data-only messages |
| Merge rules | `crates/core/src/sync.rs` | shared with the app, so both converge the same way |
| Wire types | `crates/protocol/src/{sync,cloud,ai}.rs` | versioned (`PROTOCOL_VERSION`, `AI_WIRE_VERSION`) |
| D1 `kabar` | `crates/server/migrations/` | `ai_cache`, `quota_usage` |
| Sync client (device side) | `crates/core/src/sync_client.rs` | pure state machine: hello, catch-up, one push in flight, acks |
| App engine | `crates/app/src/cloud/` | Google sign-in (PKCE + loopback), session in Keychain/Keystore, WebSocket, reconnect backoff |
| Local merge state | `crates/app/src/store.rs` | `sync_rows` + `outbox`; every synced write goes through `write_synced` |

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
| `PUT /v1/devices/me/push` | `{ token }` (this device's FCM token; `""` turns push off) → `{ registered, push_configured }` |
| `GET /v1/sync` | WebSocket; messages in `crates/protocol/src/sync.rs` |
| `GET /v1/vault` | which engines are configured (never the keys) |
| `POST /v1/ai` | `EngineCall` → `EngineReply` |
| `POST /v1/tts` | `{ segments: [{ voice, text, pitch_percent? }], rate_percent }` → `audio/mpeg` |
| `PUT /v1/blobs/{sha256}` | raw PNG or JPEG ≤ 1 MB whose SHA-256 is the name → `204`; kept 7 days in the account's Durable Object (no R2: it needs a card). `400` for a bad name, type or hash. A `/v1/ai` call with `image_sha256` reads it, `404 blob_missing` once it's gone, and runs on `WORKERS_AI_VISION_MODEL` (provisional until the bake-off), Gemini `inline_data` or a Claude image block |

Sync in one paragraph: the device sends `hello { last_seq }`, gets the missing ops in pages and `caught_up`, then pushes its outbox as `push { batch, ops ≤ 50 }` and gets an `ack` listing what can leave the outbox. Other devices' ops arrive as `ops` the moment they're stored. LWW tables keep the newest HLC, append-only tables insert once, ink strokes only take tombstones. Unknown tables, wrong device ids and clocks more than 5 minutes ahead are rejected with a reason.

Reminders in one paragraph: the Account Durable Object arms an alarm when a device signs in or sends its push token, then every quarter hour (at most 96 a day) reads `goals/profile`, `reminders/reminders`, today's `daily_plans` row, the newest `reports` row and the last 21 days of `xp_events`, and asks `coach::reminders::decide` and `coach::report::report_due`. A reminder goes to every signed-in device with a push token as an FCM **data-only** message `{ kind, line, seed }` (`line` is the snake_case name of Kak Nur's `voice::Line`, `seed` the local day number): no personal text leaves Kabar; the app renders the line itself. After a delivery, the server writes the new `ReminderState` back as a sync op from device `server` (an id no real device may use), so a reminder sent once is seen by every device. The weekly report is pushed once per week. Dead tokens (FCM `UNREGISTERED`) are forgotten; signing out forgets the device's token. While no device has a push token, or `FCM_SERVICE_ACCOUNT` is unset, the alarm stops re-arming itself and costs nothing; the next push-token call starts it again.

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

### 3.4 Firebase Cloud Messaging (reminders, free Spark plan)
1. <https://console.firebase.google.com/> → **Add project** → pick the existing Google Cloud project "Kabar" (so sign-in and push share one project). Analytics: off. The plan stays **Spark** (free); FCM has no charges.
2. **Project settings → General → Add app → Android**: package `app.kabar`. Download `google-services.json` for the app build (not a secret, but keep it out of the repo, like the rest of `~/.kabar`).
3. **Project settings → Service accounts → Generate new private key**. This downloads a JSON file. It *is* a secret:
   ```bash
   cd crates/server
   wrangler secret put FCM_SERVICE_ACCOUNT < ~/Downloads/kabar-*-firebase-adminsdk-*.json
   rm ~/Downloads/kabar-*-firebase-adminsdk-*.json
   ```
4. `GET /v1/vault` now says `"push_configured": true`. The app sends its token on every launch, which starts the reminder alarm.

### 3.5 Deploy
```bash
just deploy-cloud
curl https://kabar-cloud.<your-subdomain>.workers.dev/v1/health
```
Tell me the URL; the app will point at it.

### 3.6 Connect the app
1. `cp .env.example .env` and fill in `KABAR_CLOUD_URL` (the URL above) and the **Desktop** client's id and secret (§3.2). `.env` is git-ignored; `just` loads it and the app reads it at build time.
2. `just run-mac` → **Settings → Account & devices → Sign in with Google**. The browser opens, you pick your Gmail, and the page says to go back to Kabar.
3. `just apk` and `just install <serial>` for the S24 Ultra and the Tab S11, then sign in the same way on each.
4. Save a word under **Try sync** on one device: it shows up on the others within seconds.

## 4. Local development

```bash
just cloud-e2e   # local workerd + fake Google: phone → tablet sync, devices, sign-out, privacy rule,
                 # then the app's own sync engine on two devices against the same server
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
| Reminder alarm | 96 alarms/day/account (one per 15 min) | Durable Objects free: 100,000 requests/day; each `setAlarm()` is one of 100,000 free rows written/day |
| FCM | one message per device per reminder | free on Spark, no quota charges |

## 6. Privacy

- `task.is_personal()` + `gemini_free` is refused by the server too, not only by the router.
- Personal answers never go into the shared D1 cache.
- Keys are Worker secrets; `/v1/vault` only says whether they exist.
- A signed-out device's access token is refused by the AI/TTS proxy at once (checked against its Durable Object), not 15 minutes later.

## 7. Known gaps (phased, not dropped)

- **Android sign-in** uses the same browser + loopback flow as the Mac for now. The one-tap Credential Manager sheet needs the Kotlin bridge in `android/`; the server already accepts its tokens (Web client id).
- **Push tokens**: the app doesn't send its FCM token yet; that arrives with the Android notification code (M7).
- On Linux (development and CI only) the session is kept in memory, so you sign in on every launch. The Mac uses the Keychain and Android the Keystore.

- Vision tasks (`structure_screenshot`, `read_handwriting`, …) answer `unavailable` until the blob store lands.
- Op-log compaction (monthly snapshot) arrives once the log is big enough to matter.
- `fixtures/server/*.json` are written from the providers' documented shapes; replace them with real recordings on the first bake-off run (the `fcm_*.json` ones on the first real push).
