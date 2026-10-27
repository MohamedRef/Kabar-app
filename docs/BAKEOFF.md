# Engine bake-off

Goal: for each runtime AI task (`docs/SPEC.md` §19), find the cheapest **free** engine that passes the task's quality bar. Re-run whenever a model or prompt changes. Report every gap; never silently degrade.

## Candidates

| Engine | Model(s) | Where it runs |
|---|---|---|
| `workers_ai` (proposed) | `@cf/aisingapore/gemma-sea-lion-v4-27b-it`; a vision model for sketches | Cloudflare, free 10k Neurons/day |
| `local` | Gemma-SEA-LION-v4-4B-VL and Qwen-SEA-LION-v4-4B-VL (Q4, fit 8 GB); one check of Apertus-SEA-LION-v4-8B-IT / Sahabat-AI Gemma-2 9B | Ollama on the Mac (8 GB); optional bigger run on Daytona |
| `gemini_free` | current free Flash and Flash-Lite | Google AI Studio key |
| `claude` (reference / optional) | current Claude model | Anthropic API (Max credit if available) |

## Protocol

1. Golden sets live in `content/golden/<task>.yaml`. M0 ships the **seed** (`content/golden/seed.yaml`, built from §24 test texts); M4 grows each task to ≥ 30 items.
2. The runner (`just bakeoff`, below) sends each item to each enabled engine with the task's prompt from `prompts/` and the app's own checks (the router's one repair retry included), and records the answer, latency and tokens to `bakeoff/runs/<date>/<engine>/<task>.jsonl`. Resumable and idempotent: it skips items already answered with the same prompt version and input, and an engine that runs out of free quota or can't be reached stops without recording, so the same command finishes later.
3. Scoring per item, 0–3, on the task's rubric (below). Claude Code judges blind (engine names hidden), you spot-check ≥ 10 % plus every Javanese item.
4. **Pass bar:** mean ≥ 2.5, no item scored 0 on correctness, valid JSON ≥ 98 %, p50 latency under the task's budget.
5. Output: `docs/bakeoff-report.md` with a table task × engine and the route table written to `prompts/routes.toml`.

## Rubrics (0–3 each)

| Task | Correctness | Also scored |
|---|---|---|
| analyze_text | meaning in context, normalization, root/affix | alignment spans, language tag |
| explain_sentence | picks the right grammar entries | register/culture note accuracy, Egyptian Arabic naturalness |
| translate_text | one entry per line, all three voices filled | Egyptian Arabic naturalness, meaning in context (chat spellings, traps), tone kept |
| grade_answer | right verdict (accept paraphrases, reject errors) | feedback is specific and kind |
| roleplay_turn | level-appropriate, correct register | stays in persona, inline corrections |
| structure_screenshot | speakers, bubbles, timestamps right | keeps original text exactly |
| read_handwriting | exact transcription | — |
| guess_sketch / check_strokes | plausible guess / right verdict | answer in Indonesian |
| coach tasks (daily_plan, recap, reports, coach_message) | facts match data | §2 tone rules; never routed to `gemini_free` |
| story_episode / radio_script / mind_map | level fit, uses target words | engaging, correct |
| placement_test / level_check | level estimate within ±1 sub-level | — |

Latency budgets (p50): popover enrichment 2 s, grading 3 s, roleplay 4 s, batch tasks (stories, reports) 30 s.

## Running it (on the Mac)

Everything here is free: Workers AI's daily Neurons, Gemini's free tier,
Ollama on the Mac. Claude stays out unless you add it on purpose.

1. **Keys** in `.env` (git-ignored; `just` reads it):
   ```bash
   CLOUDFLARE_ACCOUNT_ID=…        # dash.cloudflare.com → Workers & Pages → Account ID (right column)
   CLOUDFLARE_API_TOKEN=…         # My Profile → API Tokens → Create Token → template "Workers AI" (read)
   GEMINI_API_KEY=…               # aistudio.google.com → Get API key
   # optional: OLLAMA_URL (default http://localhost:11434), WORKERS_AI_MODEL,
   # WORKERS_AI_VISION_MODEL, GEMINI_MODEL — defaults are Kabar Cloud's
   ```
   Workers AI is called through Cloudflare's REST API with the same models and
   the same free daily allocation as Kabar Cloud, so the bake-off doesn't need
   the Worker deployed.
2. **Local models:** `brew install ollama`, start it (`ollama serve`, or the
   app), then `ollama pull aisingapore/Gemma-SEA-LION-v4-4B-VL`. For
   Qwen-SEA-LION-v4-4B-VL, look for an `aisingapore/` tag with
   `ollama search` / ollama.com; if there isn't one, pull its GGUF from
   Hugging Face (`ollama pull hf.co/<repo>:Q4_K_M`). One local model at a
   time is enough RAM on 8 GB.
3. **Smoke run** (3 items per task, a few minutes): `just bakeoff-smoke`.
4. **Full run:** `just bakeoff`. Gemini's free tier allows 10 requests a
   minute, so the runner paces itself; the whole set takes a few hours and can
   run overnight. If Workers AI's daily Neurons run out, it stops that engine
   and says so: run the same command the next day to finish.
5. **Blind judging:** `just bakeoff-blind <run>` writes `blind/<task>.jsonl`
   (answers with engines replaced by letters) and `blind/<task>.rubric.md`; the
   letter → engine key goes to `key.jsonl` beside it. Claude Code judges from
   `blind/` only and writes `scores/<task>.jsonl`, one line per answer:
   `{"item": "…", "letter": "A", "score": 0-3, "note": "why"}`. Answers that
   failed the checks aren't judged; they count against the valid rate.
6. **Report:** `just bakeoff-report <run>` writes `report.md` and a proposed
   `routes.toml` into the run. A route is *vetted* only when an engine passed
   the bar on **every** item of the set (a smoke run never vets anything);
   otherwise it stays provisional and the report lists it under *Gaps*. With
   `--write` (`just bakeoff-report <run> --write`) it replaces
   `docs/bakeoff-report.md` and `prompts/routes.toml`.

Items tagged `needs-capture` (the pen tasks, paused) are skipped and counted
until their images exist.
