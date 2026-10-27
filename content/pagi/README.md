# Kabar Pagi story bank

Offline micro-stories for **Kabar Pagi** (spec §14): a 2-minute morning story
built from the learner's due words plus a few new ones, read aloud sentence by
sentence. When a free AI engine answers, the story comes from the
`story_episode` task (`prompts/story_episode@v1.md`); offline, or when no free
engine passes, the app picks the best match from this bank
(`StoryBank::best` in `crates/core/src/pagi.rs`).

## Format

`stories.jsonl`: one JSON object per line, the same shape the AI returns
(`Story` in `crates/core/src/pagi.rs`):

```json
{
  "title": "Pagi di Warung",
  "level": "bipa1",
  "sentences": [
    { "id": "Pagi ini Andi lapar.", "arz": "الصبح ده أندي جعان.", "ar": "أندي جائع هذا الصباح.", "en": "This morning Andi is hungry." }
  ],
  "words": ["pagi", "warung", "mau", "minum", "teh", "makan", "enak", "datang"]
}
```

- `level`: `bipa1` … `bipa7`. The bank holds 16 BIPA 1 and 8 BIPA 2 stories.
- `sentences`: 6–10 here (the validator allows 6–14), at most 220 Indonesian
  words per story. `id` Indonesian; `arz` Egyptian colloquial Arabic; `ar`
  Modern Standard Arabic; `en` English. Names are written in Arabic script in
  the Arabic lines; a Latin word that must stay (a word being taught, like
  *kabar*) sits between FSI U+2068 and PDI U+2069, as in `content/coach/`.
- `words`: the 5–12 key target words, as dictionary forms. Each must appear
  in the story, bare or affixed (`membeli` counts for `beli`); the tests check
  this with `uses_words`, so the matcher and the list stay honest.

## Content

Original stories written for Kabar with Claude Code at build time. Recurring
friendly cast and settings so mornings feel familiar (all in `content/cast.yaml`): Adit and Bu Murni's
warung, Tasya's family at home and at the beach, Omar (an Egyptian student in
Yogyakarta) at class and the mosque, Rina at the office, Pak Udin at the
market, the angkot, ojek rides, rainy afternoons. A few stories link Egypt and
Indonesia (a phone call to Cairo, Ramadan in Yogyakarta, Arabic loanwords).
The words come from the most frequent Indonesian words (`content/freq/id.tsv`,
gloss batches 1–10), so an offline story usually overlaps today's due words.

## Review status

**Unverified** until the learner reviews them: read each story, check that the
Indonesian sounds natural and the Egyptian Arabic sounds like Cairo, and fix
anything that doesn't. Record the review here (date, which stories).

## Gates

`cargo test -p kabar-core pagi` parses the whole bank with `StoryBank::parse`
(every story must pass `Story::validate`), checks the 16/8 split, sentence and
word counts, listening time, unique titles, and that the most frequent
everyday words are covered by at least one story.
