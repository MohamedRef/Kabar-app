# Golden sets

One file per runtime AI task (`docs/SPEC.md` §19), `content/golden/<task>.yaml`,
shaped like `schemas/golden-set.schema.json` (`kabar_protocol::content::GoldenSet`).
`seed.yaml` is the M0 seed and stays as it was. The bake-off (`docs/BAKEOFF.md`)
sends each item's `input` to every engine and judges the answer against
`expect`, `criteria` and `avoid`, using the set's `rubric` on top of the 0–3
scale. `kabar-tools golden validate` (and `cargo test`) checks every file.

Rules for every item:

- `input` is **exactly what the app sends**: the router hashes it for the cache
  and renders it into the prompt as `{{input}}`. Keep the field names below.
- Use the §24 test texts wherever they fit, then everyday material around them
  (Cairo ↔ Java, markets, chat, family, study, travel).
- Personal tasks (`placement_test`, `level_check`, `daily_plan`, `daily_recap`,
  `weekly_report`, `coach_message`) use **synthetic** learner data only.
- Coach-facing text follows the §2 tone rules: warm, specific, no guilt, fear,
  fake urgency or dark patterns. Put violations in `avoid`.
- Javanese items are `verified: false` until a native speaker checks them.
- Vision items point `image` at a fixture under `fixtures/golden/`, or carry the
  tag `needs-capture` with `input.capture` saying exactly what to record on a
  device.
- At least 30 items per task, ids unique within the file.

## Input shapes

| Task | `input` | `expect` |
|---|---|---|
| `analyze_text` | `{text, words[]}`: `words` is the tokenizer's word list; the model aligns by index | `AnalyzeText` from `crates/core/src/ai/tasks.rs`, exactly (one token per word) |
| `explain_sentence` | `{text, lang, voice: arz\|ar\|en, grammar_candidates[] (ids from content/grammar)}` | `{grammar_ids[], register, culture?, paraphrase, explanation}` |
| `grade_answer` | `{exercise, references[], answer}` | `GradeAnswer` from `crates/core/src/ai/tasks.rs`, exactly |
| `roleplay_turn` | `{persona: mas_dimas\|bu_siti\|mbak_gadis\|mas_ardi, level, scenario, history[{role: persona\|learner, text}], learner}` | `{reply, corrections[{text, fix, why_en, why_ar}], register}` |
| `placement_test` | `{lang, answers[{item, prompt, answer, correct}]}` | `{level, confidence, strengths[], gaps[]}` |
| `level_check` | `{lang, current_level, stats{…}, samples[]}` | `{level, moved, evidence[], next_focus[]}` |
| `daily_plan` | `{date, goal_minutes, due_reviews, weak_points[], next_lesson, streak_days, time_of_day}` | `{items[{kind, minutes, why}], message_ar, message_en}` |
| `daily_recap` | `{date, minutes, reviews{done, correct}, lessons_done[], mistakes[], goal_met}` | `{message_ar, message_en, highlight, tomorrow}` |
| `weekly_report` | `{week, days[{date, minutes, reviews, accuracy}], new_words, lessons_done[], weak_points[]}` | `{summary_ar, summary_en, wins[], focus_next_week[]}` |
| `coach_message` | `{trigger: missed_day\|comeback\|goal_met\|streak\|struggle\|morning, context{…}}` | `{message_ar, message_en}` |
| `story_episode` | `{series: sinetron\|kabar_pagi, level, target_words[], due_words[], previous_summary?, length_words}` | `{title, text, glossary[{word, gloss_en, gloss_ar}], summary}` |
| `radio_script` | `{level, target_words[], voices[2], minutes}` | `{title, lines[{voice, text}], recap_ar}` |
| `structure_screenshot` | `{ocr_blocks[{text, box[x,y,w,h]}], image}` | `{kind: chat\|listing\|post\|ui, messages[{speaker, side, text, time?}]}` |
| `read_handwriting` | `{lang, expected?, capture?}` (image or strokes in `image`) | `{text}` |
| `guess_sketch` | `{target_word?, capture?}` | `{guess_id, alternatives[]}` |
| `check_strokes` | `{target, script: latin\|aksara_jawa, capture?}` | `{correct, issues[]}` |
| `mind_map` | `{topic, level, known_words[], lang}` | `{center, branches[{label, words[{word, gloss_en, gloss_ar}]}]}` |

The router thread writes each task's prompt against these shapes; change a
shape only together with its prompt.

## Choices the sets made where the shapes were open

The router's prompts must agree with these (or the sets change with them):

- `register` (analyze_text, explain_sentence, roleplay_turn): the list in
  `prompts/analyze_text@v1.md`; roleplay adds `ngoko`, `madya`, `krama`,
  `krama_inggil`, `casual`, `standard`.
- `explain_sentence.paraphrase` is a natural rendering in the voice's language.
  Javanese items pick from `content/grammar/jv`.
- `structure_screenshot`: `side` is `left`/`right` for chat bubbles (`right` is
  speaker `me`) and `center` for listings, posts and UI. UI blocks use speaker
  `app`, with "Label: value" rows. OCR blocks leave out emoji.
- `guess_sketch`: a blank or unreadable canvas answers `guess_id: tidak_jelas`.
- Levels are `bipa1`…`bipa7` and `j1`…`j6` (coach tasks) or `BIPA 1`/`J1`
  (story, radio).
- Coach sub-shapes: `next_lesson {id, title, minutes}|null`,
  `lessons_done[{id, title}]`, `mistakes[{prompt, answer, expected}]`,
  `week` is the Monday's date, and `confidence` is `high|medium|low`.
  `daily_plan.items[].kind` is `review|lesson|weak_spot|roleplay|reading|listening`.
- `level_check` never moves a learner down and never skips a level; a learner
  moves up when the level test reaches its `pass_mark` (0.75 up to bipa2/j4,
  0.70 above). This is a policy proposal pending the learner's approval.
