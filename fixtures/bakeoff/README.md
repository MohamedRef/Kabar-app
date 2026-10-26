# Bake-off fixtures

Provider replies, in each provider's own envelope, for the bake-off runner's
tests (`crates/tools/src/bakeoff/tests.rs`). Hand-written in the documented
shapes until the first real run on the Mac records some: Ollama `/api/chat`,
Cloudflare's Workers AI REST API (`/accounts/{id}/ai/run/{model}`), Gemini
`generateContent`. Tests never call live APIs.
