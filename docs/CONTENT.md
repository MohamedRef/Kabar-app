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
| `content/freq/jv.tsv` | Javanese frequency list | `kabar-tools freq jv` (on the Mac) |
| `content/glosses/id/plan.tsv` | frozen work plan: rank, word, batch | `kabar-tools gloss plan` |
| `content/glosses/id/batch-NNN.jsonl` | 100 glosses per batch | Claude Code + `prompts/gloss@v1.md` |
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

Javanese has no openly licensed frequency list we can download from the cloud
sandbox; it is counted from the Javanese Wikipedia dump on the Mac
(`just content-fetch jvwiki && just content-freq-jv`). Wikipedia is formal, so
chat words (arep, piye) rank lower than in real use; the Javanese glosses (M13)
will mix in the curriculum's word lists.

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
