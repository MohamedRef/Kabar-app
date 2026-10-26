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
2. The runner (`cargo run -p kabar-tools -- bakeoff`, built in M1/M4) sends each item to each enabled engine with the task's prompt from `prompts/`, records output, latency, tokens and cost to `bakeoff/runs/<date>/<engine>/<task>.jsonl`. Resumable and idempotent (skips items already run with the same prompt version).
3. Scoring per item, 0–3, on the task's rubric (below). Claude Code judges blind (engine names hidden), you spot-check ≥ 10 % plus every Javanese item.
4. **Pass bar:** mean ≥ 2.5, no item scored 0 on correctness, valid JSON ≥ 98 %, p50 latency under the task's budget.
5. Output: `docs/bakeoff-report.md` with a table task × engine and the route table written to `prompts/routes.toml`.

## Rubrics (0–3 each)

| Task | Correctness | Also scored |
|---|---|---|
| analyze_text | meaning in context, normalization, root/affix | alignment spans, language tag |
| explain_sentence | picks the right grammar entries | register/culture note accuracy, Egyptian Arabic naturalness |
| grade_answer | right verdict (accept paraphrases, reject errors) | feedback is specific and kind |
| roleplay_turn | level-appropriate, correct register | stays in persona, inline corrections |
| structure_screenshot | speakers, bubbles, timestamps right | keeps original text exactly |
| read_handwriting | exact transcription | — |
| guess_sketch / check_strokes | plausible guess / right verdict | answer in Indonesian |
| coach tasks (daily_plan, recap, reports, coach_message) | facts match data | §2 tone rules; never routed to `gemini_free` |
| story_episode / radio_script / mind_map | level fit, uses target words | engaging, correct |
| placement_test / level_check | level estimate within ±1 sub-level | — |

Latency budgets (p50): popover enrichment 2 s, grading 3 s, roleplay 4 s, batch tasks (stories, reports) 30 s.
