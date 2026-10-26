# gloss@v1 — prebuilt glosses for frequent Indonesian words

Build-time prompt (`docs/SPEC.md` §18) for Claude Code sessions, or a batch
API run, that write `content/glosses/id/batch-NNN.jsonl`. Output must match
`schemas/gloss-entry.schema.json` and pass `kabar-tools gloss validate`.

## The learner

A native **Egyptian Arabic** speaker (Cairo) who reads English fluently and is
a beginner in Indonesian. Glosses show up in a word popover and in lessons; the
learner picks the explanation voice: Egyptian colloquial, MSA, or English.

## Input

A batch: lines of `rank<TAB>word` from `kabar-tools gloss words <N>`, 100 words,
most frequent first. Words are lowercase surface forms as they appear in text
(membuat, rumahnya, anak-anak, kau, nggak).

## Output

Exactly one JSON object per line, **in the same order as the input, one per
input word, nothing else** (no blank lines, no comments, no Markdown). Fields:

| field | rule |
|---|---|
| `lang` | `"id"` |
| `word` | the input word, unchanged |
| `rank` | the input rank, as a number |
| `lemma` | the word without inflectional clitics (-nya, -ku, -mu, -lah, -kah, -pun): `rumahnya` → `rumah`, `membuatnya` → `membuat`. Otherwise the word itself. |
| `root` | the base when the word is derived (`membuat` → `buat`, `kesehatan` → `sehat`, `anak-anak` → `anak`). Omit for bare roots and particles. |
| `affixes` | display breakdown in order, including the root: `["meN-","buat"]`, `["ke-","sehat","-an"]`, `["di-","bersih","-kan"]`, `["rumah","-nya"]`, `["anak","RED"]` (RED = reduplication), `["ber-","main"]`. Nasal prefix is always written `meN-` / `peN-`. Omit when there is no root. |
| `senses` | 1–4, most common meaning first. Each: `pos`, `gloss` {`arz`,`ar`,`en`}, optional `register`. Glosses are short (1–6 words; synonyms separated by "، " or ", "). Only meanings a learner will actually meet. |
| `examples` | exactly **2** (3 only if a second sense really needs one). Each: `text` (Indonesian), `translation` {`arz`,`ar`,`en`}, `sense` (0-based index into `senses`). |
| `note` | optional {`arz`,`ar`,`en`}: one short, genuinely useful usage note (register, a common confusion, a fixed phrase). Skip if nothing worth saying. |
| `arabic_origin` | only when the word really comes from Arabic (directly or via Malay/Persian): the Arabic source word, vocalized where it helps, e.g. `"خَبَر"`. Never guess; omit when unsure. |
| `false_friend` | only for Arabic-origin words whose Indonesian meaning would mislead an Egyptian: {`arz`,`ar`,`en`} warning, e.g. kalimat = sentence, not «كلمة». Requires `arabic_origin`. |
| `status` | `"unverified"` |
| `provenance` | `{"generator":"claude-code","prompt":"gloss@v1"}` |

`pos` values: `noun`, `proper_noun`, `verb`, `adjective`, `adverb`, `pronoun`,
`preposition`, `conjunction`, `particle`, `numeral`, `classifier`, `determiner`,
`interjection`, `phrase`.
`register` values (omit for neutral): `formal`, `informal`, `colloquial`
(Jakarta/chat: gue, lo, nggak, banget), `slang`, `literary`, `religious`.

## Voices

- **`arz` — Egyptian colloquial, as a Cairene would text a friend.** عايز، دلوقتي،
  إزيك، مش، كده، بتاع، أوي، لسه، فين، إمتى، ليه. Not MSA with a few colloquial
  words: if it would sound stiff in Cairo, rewrite it. Arabic script only.
- **`ar` — Modern Standard Arabic**, concise and correct, with light tashkeel only
  where a word would be ambiguous.
- **`en` — plain English.**

Translations of examples are natural in each voice, not word-for-word.

## Examples (the sentences)

- Natural, everyday Indonesian a learner could meet in the first year: 4–12
  words, mostly A1–A2 vocabulary around the target word.
- Must contain the **exact** input word form (case-insensitive); end with `.`,
  `?` or `!`.
- Colloquial words get colloquial sentences (gue, lo, nggak…); standard words get
  standard sentences.
- Prefer content that fits the learner's world (Cairo, Java, family, food,
  markets, study, travel, phones). Use names like Budi, Sari, Ahmad, Dewi.
- Never personal data, never anything offensive. For swear words, give the
  meaning plainly and mark `register: slang` plus a `note` about how strong it is.

## Quality bar

- Correctness first. If a word is ambiguous (e.g. `kali` = times / river, `bisa`
  = can / venom), list the common sense first and add the other only if a
  learner will meet it.
- Subtitle-style words (kau, -mu, ku) are glossed as what they are (informal
  pronouns) with a note on register.
- Loanwords from English/Dutch: gloss normally; no `arabic_origin`.
- Indonesian words from Arabic are very common (waktu, kabar, selamat, pikir,
  paham, kursi, ilmu, dunia, hak, hukum, rakyat, masyarakat, mungkin, kalimat,
  ijazah, musim, hewan, akal, ahli…). Always fill `arabic_origin` for them; the
  Arabic Bridge is the learner's biggest head start.
- False friends worth flagging (examples): kalimat (sentence ≠ كلمة), masyarakat
  (society ≠ مشاركة), ijazah (diploma ≠ إجازة/vacation), logat (accent ≠ لغة),
  mungkin (maybe, from ممكن "possible": a meaning shift).
- Keep every string tidy: no leading/trailing spaces, no TODO.

## One line, pretty-printed here for reading

```json
{"lang":"id","word":"membaca","rank":900,"lemma":"membaca","root":"baca","affixes":["meN-","baca"],
 "senses":[{"pos":"verb","gloss":{"arz":"يقرا","ar":"يقرأ","en":"to read"}}],
 "examples":[
  {"text":"Saya sedang membaca buku.","translation":{"arz":"أنا بقرا كتاب دلوقتي.","ar":"أنا أقرأ كتابًا الآن.","en":"I'm reading a book."},"sense":0},
  {"text":"Sari suka membaca di kereta.","translation":{"arz":"ساري بتحب تقرا في القطر.","ar":"تحب ساري القراءة في القطار.","en":"Sari likes reading on the train."},"sense":0}],
 "status":"unverified","provenance":{"generator":"claude-code","prompt":"gloss@v1"}}
```
