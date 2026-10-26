# speech-levels@v1 — the Javanese speech-level table

Build-time prompt (`docs/SPEC.md` §13, §17) for `content/javanese/speech-levels.jsonl`.
Output matches `schemas/speech-level-entry.schema.json` and passes
`kabar-tools levels validate`. Everything is `unverified` until the learner or a
native speaker approves it; the table is exported for a tutor to check.

## What a row is

One meaning with its forms at each level. Javanese swaps whole words between
levels: *mangan* (ngoko) → *nedha* (krama) → *dhahar* (krama inggil, for the
person you respect). A level left empty means "no special word; use the next
level down" (krama inggil → krama → madya → ngoko).

- **ngoko**: plain speech to friends, younger people, yourself.
- **madya**: the in-between market register (*samang*, *niki*, *nika*); fill it
  only for words that really have a madya form.
- **krama**: polite speech to elders, strangers, people of higher status.
- **krama inggil**: honorifics for the *other* person's body, actions and
  belongings (*panjenengan*, *dhahar*, *sare*, *rawuh*, *dalem*).
- **krama andhap**: humble words for *your own* actions toward a respected
  person (*sowan*, *matur*, *nyuwun*, *ngaturi*, *caos*).

## Fields

| field | rule |
|---|---|
| `id` | snake_case English meaning (`eat`, `house`, `you_singular`, `go_home`) |
| `pos` | as in glosses (`noun`, `verb`, `pronoun`, …) |
| `meaning` | {`arz`,`ar`,`en`} short gloss |
| `indonesian` | the standard Indonesian word |
| `ngoko`, `madya`, `krama`, `krama_inggil`, `krama_andhap` | lists of lowercase forms (common spelling first; Javanese Latin spelling with dh/th, é/è only where standard) |
| `note` | optional {`arz`,`ar`,`en`}: who you'd say each form to, common mistakes (never use krama inggil about yourself), regional notes. Egyptian analogies help: ngoko ≈ «إنت», krama ≈ «حضرتك», krama inggil ≈ «سيادتك». |
| `examples` | 1–3 sentences, each tagged with its `level` and using that level's form, with {`arz`,`ar`,`en`} translations. The §24 test sentence for "Have you eaten?" belongs on `eat`. |
| `confidence` | `high` for standard textbook pairs, `medium` when sources disagree, `low` when unsure |
| `sources` | `["claude-code"]` |
| `status` | `"unverified"` |

## Coverage

The standard krama/krama inggil vocabulary a learner needs for J3–J4 (Solo,
Yogyakarta): pronouns and address terms, body parts, everyday verbs (eat,
drink, sleep, go, come, sit, say, ask, give, take, see, hear, know, want, have,
bathe, die, be born, be sick…), family, house and belongings, time words,
question words, numbers that change (*siji/setunggal*, *loro/kalih*,
*telu/tiga*…), demonstratives (*iki/menika/punika*), negation and particles
(*ora/mboten*, *wis/sampun*, *durung/dereng*, *arep/badhe*, *isih/taksih*), and
common adjectives. Aim for 300–450 rows, most frequent first.
