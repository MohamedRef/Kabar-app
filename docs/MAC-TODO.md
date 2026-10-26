# What's left for you on the MacBook (M0)

These need your machine, your accounts or your devices. Nothing here costs money.

## Answer two questions
- [ ] MacBook Air M2 RAM: 8 / 16 / 24 GB? (decides which local models are worth testing)
- [ ] Are you on Claude Max (5x or 20x)? If yes, claim the monthly API credit in Claude → Settings → Billing and link a Console org.

## Approve or change the plan
- [ ] Read `docs/PLAN.md` §1 (decisions D1–D5) and reply OK / change.

## Accounts (free, no card unless noted)
- [ ] Cloudflare account (Workers Free). No card needed.
- [ ] Google AI Studio API key for the Gemini free tier.
- [ ] Azure account + Speech resource on **F0** (Azure signup asks for a card for identity; F0 never bills).
- [ ] Firebase project (Spark plan) for FCM, plus a Google Cloud OAuth client (Android + desktop) — I'll give exact steps in M2.

## Bake-off prep (run when M1 tooling lands; commands will be in the justfile)
- [ ] `brew install ollama` and pull the shortlisted GGUFs (Apertus-SEA-LION-v4-8B-IT, Sahabat-AI Gemma-2 9B; 27B only if RAM ≥ 24 GB).
- [ ] Download kaikki.org Indonesian and Javanese JSONL (the cloud sandbox can't reach kaikki.org).
- [ ] Run the bake-off runner and send me the `bakeoff/runs/` folder; I score it with you.

## Devices (M1)
- [ ] Enable Developer options + USB / Wireless debugging on the S24 Ultra and Tab S11.
- [ ] Install Android Studio (for SDK/NDK) — the `just setup-mac` script will handle the rest.
- [ ] Run Pen Lab on both Galaxy devices and send me the report.
