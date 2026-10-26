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
