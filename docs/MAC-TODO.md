# What's left for you on the MacBook (M0)

These need your machine, your accounts or your devices. Nothing here costs money.

## Claude Max credit
- [ ] While you're on Max 20x: claim the monthly $200 API credit in Claude → Settings → Billing and link a Console org. It expires each cycle, so we spend it on build-time content batches.

## Approve or change the plan
- [ ] Read `docs/PLAN.md` §1 (decisions D1–D5) and reply OK / change.

## Accounts (free, no card unless noted)
- [x] Cloudflare account (Workers Free). No card needed.
- [ ] Google AI Studio API key for the Gemini free tier.
- [ ] Azure account + Speech resource on **F0** (Azure signup asks for a card for identity; F0 never bills).
- [ ] Firebase project (Spark plan) for FCM, plus a Google Cloud OAuth client (Android + desktop) — I'll give exact steps in M2.

## Bake-off prep (run when M1 tooling lands; commands will be in the justfile)
- [ ] `brew install ollama` and pull the 4B SEA-LION v4 GGUFs (8 GB RAM fits only ~4B models comfortably).
- [ ] Optional: add `DAYTONA_API_KEY` as an environment secret so I can run heavy builds/bake-off there.
- [ ] Download kaikki.org Indonesian and Javanese JSONL (the cloud sandbox can't reach kaikki.org).
- [ ] Run the bake-off runner and send me the `bakeoff/runs/` folder; I score it with you.

## Devices (M1) — full steps in docs/RUN.md
- [ ] `brew install just && just setup-mac` (no Android Studio needed).
- [ ] `just check`, then `just run-mac`.
- [ ] Enable Developer options + USB / Wireless debugging on the S24 Ultra and Tab S11.
- [ ] `just android-keystore`, `just apk`, `just install <serial>` on both devices.
- [ ] Run Pen Lab on both Galaxy devices and send me the JSON reports + how the ink feels.

## Kabar Cloud (M2) — full steps in docs/CLOUD.md
In this order, from the `kabar-app` folder. Nothing here costs money or needs a card.
- [x] Create a free Cloudflare account.
- [ ] `just cloud-tools` (installs wrangler 4 + worker-build).
- [ ] `wrangler login`: the browser opens, click **Allow**.
- [ ] `just cloud-setup`: creates the D1 database, the token signing key (a Worker secret, never in the repo) and asks for your Gmail for the allowlist. Commit the D1 id it writes into `crates/server/wrangler.toml` (the id is not a secret).
- [ ] Google Cloud Console (docs/CLOUD.md §3.2): consent screen in *Testing* with your Gmail as test user, then Web and Desktop OAuth clients. Send me both client ids and I put them in `wrangler.toml`.
- [ ] Android OAuth client: needs the release SHA-1, so do it after `just android-keystore` (Devices section): `keytool -list -v -keystore ~/.kabar/kabar-release.jks -alias kabar`.
- [ ] Optional keys: `cd crates/server && wrangler secret put GEMINI_API_KEY` (and `AZURE_SPEECH_KEY` once the F0 resource exists).
- [ ] `just deploy-cloud`, then `curl https://kabar-cloud.<subdomain>.workers.dev/v1/health` and send me the URL.
- [ ] Optional, if you use Claude Code on the Mac: `claude plugin marketplace add cloudflare/skills` then `claude plugin install cloudflare@cloudflare` (Cloudflare's own skills + MCP; it signs in to your Cloudflare account in the browser). Kabar keeps deploying with wrangler.

## Kak Nur coach (M7) — full steps in docs/RUN.md §6
- [ ] `just run-mac`: go through onboarding, tick a plan item, check the recap and the Kak Nur screen. Tell me anything that reads wrong or feels off.
- [ ] Review Kak Nur's lines in `content/coach/kak-nur.toml` (Egyptian Arabic, MSA, English). Mark changes or say OK; then I set `reviewed = true`.
- [ ] Install the new APK on the S24 Ultra and Tab S11 and check the plan is the same on both once sync is on.
- [ ] Reminders (docs/CLOUD.md §3.4): in the Firebase console add Firebase to the "Kabar" Google Cloud project (Spark, free) and the Android app `app.kabar`; keep `google-services.json` for the app build, outside the repo.
- [ ] Generate a service-account private key, then `cd crates/server && wrangler secret put FCM_SERVICE_ACCOUNT < that-file.json`, delete the file, `just deploy-cloud`.
