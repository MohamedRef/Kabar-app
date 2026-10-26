# Kabar Cloud fixtures

Upstream response bodies used by `crates/server` tests (no test calls a live API).

These were **written by hand from each provider's documented response shape**,
because the build sandbox has no API keys. Replace them with real recordings
the first time the bake-off runs on the Mac (`just bakeoff` saves raw bodies),
keeping the same file names.

| File | Shape |
|---|---|
| `workers_ai_analyze.json` | Workers AI text generation: `{ "response": "..." }` |
| `gemini_analyze.json` | Gemini `generateContent`: `candidates[0].content.parts[].text` |
| `claude_analyze.json` | Anthropic Messages API: `content[]` text blocks, `stop_reason` |
| `fcm_token.json` | Google OAuth token endpoint (JWT bearer grant): `access_token`, `expires_in` |
| `fcm_send_ok.json` | FCM HTTP v1 `messages:send` success: `{ "name": "projects/…/messages/…" }` |
| `fcm_unregistered.json` | FCM HTTP v1 error for a dead token: 404 `NOT_FOUND` with `errorCode: UNREGISTERED` |
