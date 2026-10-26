# Content pipeline (build time)

Quality-critical content is generated at build time, validated, reviewed by
the learner and committed under `content/` (`docs/SPEC.md` §18). The tool is
`crates/tools` (`kabar-tools`); every command is idempotent and resumable.

## Layout

| Path | What | Made by |
|---|---|---|
| `scripts/fetch-sources.sh` | downloads pinned sources into `data/raw/` (git-ignored) | — |
| `content/freq/id.tsv` | Indonesian frequency list, 30k words | `kabar-tools freq id` |
| `content/freq/id.exclude.tsv` | junk kept out of the list (English, names, subtitle noise), with reasons | by hand |
| `content/freq/jv.tsv` | Javanese frequency list, 20k words | `kabar-tools freq jv` |
| `content/freq/jv.exclude.tsv` | junk and names kept out of the Javanese list | by hand |
| `content/glosses/id/plan.tsv` | frozen work plan: rank, word, batch | `kabar-tools gloss plan` |
| `content/glosses/id/batch-NNN.jsonl` | 100 glosses per batch | Claude Code + `prompts/gloss@v1.md` |
| `content/glosses/jv/plan.tsv`, `batch-NNN.jsonl` | Javanese glosses (with `indonesian` and `speech_level`), unverified | Claude Code + `prompts/gloss-jv@v1.md` |
| `content/javanese/speech-levels.jsonl` | Javanese speech-level table: ngoko, madya, krama, krama inggil, krama andhap per meaning | Claude Code + `prompts/speech-levels@v1.md` |
| `content/golden/<task>.yaml` | golden sets for the 17 runtime AI tasks (`content/golden/README.md`) | Claude Code |
| `fixtures/golden/` | images for vision golden items | rendered synthetically, or captured on the devices |
| `content/bridge/sound-rules.json` | Arabic → Indonesian sound changes | by hand |
| `content/bridge/id.jsonl` | Arabic Bridge cards | Claude Code + `prompts/bridge@v1.md` |
| `content/bridge/wiktionary-{id,jv}.jsonl` | Wiktionary entries with an Arabic etymology | `kabar-tools kaikki` (on the Mac) |
| `build/dict/{id,jv}.jsonl` | full normalized Wiktionary import (git-ignored) | `kabar-tools kaikki` |
| `schemas/*.schema.json` | JSON Schemas, generated from `kabar_protocol::content` | `kabar-tools schemas` |

## Frequency lists

Indonesian blends wordfreq (Wikipedia + subtitles + Twitter) with the
OpenSubtitles 2018 counts: a word's Zipf score is the mean of the sources that
list it, minus 0.3 when only one does. Words that aren't Latin lowercase, are
shorter than two letters, or repeat a letter three times are dropped, then the
exclude list. Subtitles push conversational words (aku, kau, kamu) up, which
suits a learner who wants to understand chats.

Javanese is counted from text written by native speakers: NusaWrites
(NusaAlinea paragraphs and NusaKalimat) and the NusaX sentiment set, fetched
from GitHub with `just content-fetch nusa` (~5 MB). That text is mostly ngoko
and includes East Javanese and chat spellings (gak, wes, iso), which is what
the learner will read. On the Mac, `just content-fetch jvwiki && just
content-freq-jv` adds the Javanese Wikipedia dump as a second source with the
same mean-of-sources merge; plan rows never move, so finished Javanese gloss
batches stay valid.

## Glosses

`gloss plan --count N` appends the next N most frequent unplanned words; rows
never move, so finished batches stay valid when the list is rebuilt. A batch is
done when `gloss validate --batch N` passes. `gloss next` prints the first batch
that isn't done, which is how a later session (or the Message Batches API with
Max credit, for the 10k pass in M13) resumes.

Validation goes beyond the schema: Arabic script in `arz`/`ar`, English in `en`,
1–4 senses, 2–3 examples that contain the word and end with punctuation,
affixes that include the root, `false_friend` only with `arabic_origin`, and
batch order matching the plan. `kabar-tools validate` adds the gloss ↔ Bridge
cross-check, and `cargo test` runs all of it in CI.

Everything is `status: unverified` until the learner approves it.
`just content-review 3` writes a Markdown sheet for batch 3 to `build/review/`.

## Arabic Bridge

Cards are written from knowledge first (`sources: ["claude-code"]`, honest
`confidence`), because kaikki.org can't be reached from the cloud sandbox. On
the Mac, `just content-fetch kaikki && just content-kaikki` imports Wiktionary,
then `bridge crosscheck` reports cards Wiktionary confirms, cards where the
Arabic word differs, and Wiktionary's Arabic loans that have no card yet
(`build/bridge-todo.txt`, the input for the next card batch). `bridge crosscheck
--write` adds `wiktionary` to the sources of confirmed cards.

## Javanese speech levels

`content/javanese/speech-levels.jsonl` holds one meaning per row with its forms
at each level (*mangan* → *nedha* → *dhahar*). `levels validate` checks that
ngoko and at least one higher level are present, that forms are lowercase
Latin, and that each example uses a form of its own level. The router
cross-checks Javanese AI answers against this table (§19). Everything is
unverified until a native speaker checks it; `levels stats` prints coverage.

## Golden sets

`content/golden/<task>.yaml` holds 30+ items per runtime AI task for the
bake-off (`docs/BAKEOFF.md`); the rules and input shapes are in
`content/golden/README.md`. `golden validate` checks every set, and for the
typed tasks (`analyze_text`, `grade_answer`) that `expect` deserializes into the
router's output struct and passes its validation. Handwriting, sketch and stroke
items are tagged `needs-capture` until they are recorded on the Galaxy devices;
`golden stats` lists them.
