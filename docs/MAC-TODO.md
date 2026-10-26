# What's left for you on the MacBook (M0)

These need your machine, your accounts or your devices. Nothing here costs money.

## Claude Max credit
- [ ] While you're on Max 20x: claim the monthly $200 API credit in Claude → Settings → Billing and link a Console org. It expires each cycle, so we spend it on build-time content batches.

## Approve or change the plan
- [ ] Read `docs/PLAN.md` §1 (decisions D1–D5) and reply OK / change.

## Accounts (free, no card unless noted)
- [ ] Cloudflare account (Workers Free). No card needed.
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
- [ ] Create a free Cloudflare account (no card), then `just cloud-tools`, `wrangler login`, `just cloud-setup`.
- [ ] Google Cloud Console: OAuth clients (Web, Android with the release SHA-1, Desktop); put the Web + Desktop ids in `crates/server/wrangler.toml`.
- [ ] Optional: `wrangler secret put GEMINI_API_KEY` and `AZURE_SPEECH_KEY`.
- [ ] `just deploy-cloud` and send me the `workers.dev` URL.

## Kak Nur coach (M7) — full steps in docs/RUN.md §6
- [ ] `just run-mac`: go through onboarding, tick a plan item, check the recap and the Kak Nur screen. Tell me anything that reads wrong or feels off.
- [ ] Review Kak Nur's lines in `content/coach/kak-nur.toml` (Egyptian Arabic, MSA, English). Mark changes or say OK; then I set `reviewed = true`.
- [ ] Install the new APK on the S24 Ultra and Tab S11 and check the plan is the same on both once sync is on.
- [ ] Reminders (after the Kabar Cloud reminder PR lands): Firebase setup from docs/CLOUD.md and `wrangler secret put FCM_SERVICE_ACCOUNT`.
