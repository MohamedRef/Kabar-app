# bridge@v1 — Arabic Bridge cards

Build-time prompt (`docs/SPEC.md` §12, §18) for Claude Code sessions that write
`content/bridge/id.jsonl`. Output must match `schemas/bridge-entry.schema.json`
and pass `kabar-tools bridge validate`.

## Goal

Every Indonesian word of Arabic origin a learner may meet, as a card that shows
the Arabic word, its root, how the meaning moved, and which regular sound
changes (`content/bridge/sound-rules.json`) explain the spelling. The learner
is a native Egyptian Arabic speaker: the Bridge turns words they already know
into instant Indonesian vocabulary, and the False Friends Alarm stops the ones
that would mislead them.

Wiktionary (via `kabar-tools kaikki` + `bridge crosscheck`) later confirms each
card or flags it, so only include words you are confident come from Arabic
(directly, or via Malay/Persian: set `via`).

## One card per line

| field | rule |
|---|---|
| `lang` | `"id"` |
| `word` | Indonesian word, lowercase, standard KBBI spelling (`selamat`, `masyarakat`) |
| `arabic` | Arabic source word with helpful tashkeel (`خَبَر`, `سَلَامَة`) |
| `root` | Arabic root letters separated by spaces (`خ ب ر`); omit for non-derived words |
| `arabic_meaning` | {`arz`,`ar`,`en`}: what the Arabic word means |
| `meaning` | {`arz`,`ar`,`en`}: what the Indonesian word means |
| `shift` | `same` · `narrowed` · `broadened` · `shifted` · `false_friend` |
| `shift_note` | {`arz`,`ar`,`en`}: required unless `same`. One or two sentences that make the difference stick. |
| `sound_rules` | ids from `sound-rules.json` that this word shows (can be empty) |
| `via` | optional intermediate language (`Malay`, `Persian`) |
| `confidence` | `high` (well known, you are sure) · `medium` · `low` |
| `sources` | `["claude-code"]` |
| `status` | `"unverified"` |

`false_friend` = an Egyptian reading the word as the Arabic they know would get
it wrong: kalimat (sentence, not «كلمة» word), masyarakat (society, not
«مشاركة» participation), ijazah (diploma, not «إجازة» vacation), logat (accent,
not «لغة» language). Use `shifted` when the link is helpful but the meaning
moved (mungkin "maybe" ← «ممكن» "possible"; selamat "congratulations/safe" ←
«سلامة»).

## Voices

`arz` is natural Cairene colloquial (عايز، كده، مش، يعني). `ar` concise MSA.
`en` plain English.

## Example line

```json
{"lang":"id","word":"kabar","arabic":"خَبَر","root":"خ ب ر","arabic_meaning":{"arz":"خبر","ar":"خبر، نبأ","en":"news"},"meaning":{"arz":"خبر، أخبار","ar":"خبر، أخبار","en":"news"},"shift":"same","sound_rules":["kha_k"],"confidence":"high","sources":["claude-code"],"status":"unverified"}
```
