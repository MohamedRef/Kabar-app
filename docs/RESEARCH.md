# M0 verification — services, crates and free tiers

Checked 26 October 2026. Status: ✅ fine as the spec says · ⚠️ works, with a caveat · 🔁 proposed change (needs the user's OK).

## Summary

| Item | Status | Finding |
|---|---|---|
| Dioxus | ✅ ⚠️ | 0.7.10 (16 Oct 2026) is current. Android runs in a WebView (wry). No official Kotlin/Java interop yet; the community route is JNI via `ndk_context` + the activity's class loader. |
| sastrawi-rs | ✅ ⚠️ | 0.5.3 (Jul 2026), MIT, Rust 2024, Indonesian + Javanese stemmers. Indonesian is excellent; Javanese is weak (~1.3k roots). Tiny user base (183 downloads). |
| fsrs | ✅ | 6.6.2 (Aug 2026), BSD-3, FSRS-6 (21 parameters), optimizer included, pure Rust. |
| workers-rs (`worker`) | ✅ | 0.8.7 (Sep 2026). |
| Cloudflare Workers Free | ⚠️ | 100k requests/day, **10 ms CPU per request**, 50 subrequests, 5 Cron Triggers per account, 128 MB memory. |
| Durable Objects Free | ✅ | Available on Free, **SQLite backend only**. 100k requests/day, 13,000 GB-s/day, 5M rows read/day, 100k rows written/day, 5 GB total. Incoming WebSocket messages bill at 20:1; hibernation avoids duration charges. |
| D1 Free | ✅ | 10 databases, 500 MB each, 5 GB total, 50 queries per invocation, Time Travel 7 days. |
| Workers AI Free | 🔁 new | 10,000 Neurons/day free. Hosts **SEA-LION v4 27B IT** (`@cf/aisingapore/gemma-sea-lion-v4-27b-it`, 128k context). See "Proposed changes". |
| Firebase Cloud Messaging | ✅ | No-cost on every Firebase plan. |
| Google Sign-In (Android) | ✅ | Credential Manager + Sign in with Google is the current API; legacy Google Sign-In is deprecated. Matches the spec. |
| Gemini API free tier | ⚠️ | Free models in Oct 2026: 3.x Flash / Flash-Lite, 2.5 Flash, Gemma 4. Google publishes no central limits table (check aistudio.google.com/rate-limit); community figures ≈10 RPM / ≈1,500 RPD per project. Free-tier inputs **are used to improve Google products and may be read by human reviewers**; must be 18+. Egypt and Indonesia are supported regions. |
| Anthropic API | ✅ | Since 7 Oct 2026, Max 5x includes $100/month and Max 20x $200/month of API credit (claim in Settings → Billing, link one Console org, no rollover, not usable for interactive Claude Code). So `claude` engine at $0 extra is real if the user is on Max. |
| Azure Speech F0 | ✅ ⚠️ | 0.5M neural TTS characters/month, 5 h STT/month. Pronunciation assessment is not listed under free. `azure-speech` crate last released Jul 2025 → use the REST API (simple, stable). |
| ML Kit Digital Ink | ✅ | Indonesian `id`, Javanese `jv-Latn`, Arabic `ar` all supported. |
| ML Kit Text Recognition v2 | ✅ | Latin-script model covers Indonesian/Javanese text. |
| Apple Vision OCR | ⚠️ | Latin text works, but Indonesian isn't an officially listed language; test on the Mac. Tesseract `ind`/`jav` stays the fallback. |
| Android window size classes | ✅ | Compact <600dp · Medium 600–840 · Expanded 840–1200 · Large 1200–1600 · Extra-large ≥1600. |
| Ollama | ✅ | Free, runs on Apple Silicon. |
| Sahabat-AI | ⚠️ | v1 (Llama-3 8B, Gemma-2 9B) is from 2024 and now older than SEA-LION v4. |
| SEA-LION v4 | ✅ | GGUFs: Apertus-SEA-LION-v4-8B-IT, Gemma-SEA-LION-v4-27B-IT, Gemma-SEA-LION-v4-4B-VL (vision). |
| Tailscale | ✅ | Free Personal plan is enough for 3 devices. Cloudflare Tunnel is also free. |
| NLLB-200 | ⚠️ | CC BY-NC 4.0 — fine for private, non-commercial use only. |
| kaikki.org | ⚠️ | Not reachable from the cloud sandbox (network policy). Download runs on the Mac. |

## Hands-on check (sastrawi-rs + fsrs)

Built a throwaway binary against both crates.

Indonesian — all correct: membaca→baca, dibersihkan→bersih, rumahnya→rumah, bermain→main, kesehatan→sehat, anak-anak→anak, pertanian→tani, ketahuan→tahu, penjualan→jual.

Javanese — partial: omahe→omah ✅, nedha→tedha ✅, mlaku-mlaku→laku ✅; **diculke, tukokke, ditukoni unchanged** ❌ (missing roots / -ke, -i suffix handling). Plan: own Javanese affix decomposer in `crates/core` backed by Wiktionary lemmas, using sastrawi-rs as one signal, not the authority.

FSRS: default parameters, first "Good" → 2.31 days. API is `FSRS::new(&params)` in 6.x.

## Proposed changes (need approval)

1. **Add a `workers_ai` engine** (Cloudflare Workers AI, same account as Kabar Cloud). SEA-LION v4 27B is a Southeast-Asia-tuned model, free up to 10k Neurons/day ≈ 300k input or ≈200k output tokens/day — plenty for one learner. Runs next to the Worker (no extra key, no region issues), and Cloudflare states it doesn't use Workers AI inputs for training (recheck before M2). It becomes a bake-off candidate alongside `local`, `gemini_free` and `claude`. Over the cap, requests fail (no surprise bill) on the Free plan.
2. **Local model:** benchmark Apertus-SEA-LION-v4-8B-IT and (if RAM ≥ 24 GB) Gemma-SEA-LION-v4-27B-IT alongside Sahabat-AI v1, instead of starting from Sahabat-AI alone. Gemma-SEA-LION-v4-4B-VL is a local candidate for `guess_sketch` / `check_strokes`.
3. **Azure:** REST API instead of the `azure-speech` crate.

## Constraints that shape the design

- 10 ms CPU per Worker request: the Worker only authenticates, routes and proxies. Sync merging lives in the per-account Durable Object; content packs are pre-built and served as static blobs.
- 100k DO rows written/day is far above one learner's needs, but ink must be batched (one row per stroke, not per point).
- 5 Cron Triggers per account: one cron (every 15 min) dispatches all scheduled jobs (daily plan, recap, weekly report, reminders).
- Gemini free tier: only learning text, never personal data (coach memory, goals, journal stay off Gemini). Coach tasks route to `workers_ai`, `local` or templates.

## Sources

- crates.io API for dioxus, sastrawi-rs, fsrs, worker, azure-speech, rusqlite, whisper-rs, ollama-rs, keyring, jni
- https://github.com/ibahasa/sastrawi-rs
- https://developers.cloudflare.com/workers/platform/limits/
- https://developers.cloudflare.com/durable-objects/platform/pricing
- https://developers.cloudflare.com/d1/platform/limits
- https://developers.cloudflare.com/workers-ai/platform/pricing/
- https://developers.cloudflare.com/workers-ai/models/gemma-sea-lion-v4-27b-it
- https://ai.google.dev/gemini-api/docs/available-regions
- https://ai.google.dev/gemini-api/terms_preview
- https://klymentiev.com/blog/gemini-api-free-tier (Oct 2026)
- https://www.aipricing.guru/news/claude-max-team-monthly-api-credits-2026/
- https://azure.microsoft.com/en-au/pricing/details/cognitive-services/speech-services/
- https://developers.google.com/ml-kit/vision/digital-ink-recognition/base-models
- https://firebase.google.com/docs/projects/billing/firebase-pricing-plans
- https://developer.android.com/develop/adaptive-apps/guides/use-window-size-classes
- https://developer.android.com/identity/sign-in/legacy-gsi-migration
- https://github.com/DioxusLabs/dioxus/issues/3972
- https://huggingface.co/collections/aisingapore/sea-lion-v4
