# gloss-jv@v1 — prebuilt glosses for frequent Javanese words

Same output as `gloss@v1.md` (read it first: fields, voices, example rules,
quality bar), for `content/glosses/jv/batch-NNN.jsonl`, with these differences.

## The words

The list comes from everyday Javanese written by native speakers (NusaWrites,
NusaX): mostly **ngoko**, with East Javanese / Surabaya *Arek* forms (ambek,
nang, onok, opo, koyok, sampek), Central Javanese forms (karo, neng, ana, apa,
kaya, nganti), chat spellings (gak, wes, iso, yo) and some Indonesian words
that appear inside Javanese text (dan, di, mereka). Gloss each as what it is.

## Extra and changed fields

| field | rule |
|---|---|
| `lang` | `"jv"` |
| `indonesian` | **required**: the Indonesian equivalent (`mangan` → `makan`). The learner meets Javanese after BIPA 1, so this is the fastest bridge. |
| `speech_level` | `ngoko`, `madya`, `krama`, `krama_inggil` or `krama_andhap` when the word belongs to one (most of this list: `ngoko`). Omit for words that are the same at every level (names, numbers in some cases, loanwords like `hp`). |
| `lemma` / `root` / `affixes` | Javanese morphology: nasal `N-` (written `N-`: `nggawe` = `["N-","gawe"]`), `di-`, `ka-`, `-ake` / `-ke`, `-i`, `-an`, `-e` / `-ne` (definite/possessive), `-en`, `-mu`, `tak-`/`dak-`, `kok-`, reduplication `RED`. `lemma` strips only `-e`/`-ne` possessive and `-mu`/`-ku` clitics. |
| `note` | Say which region a form belongs to when it isn't general ("Surabaya / Arek", "Central Java"), give the standard spelling of chat forms (`wes` → `wis`, `iso` → `isa`/`bisa`), and mention the krama equivalent for common ngoko words ("krama: nedha"). |
| `arabic_origin` | Javanese has Arabic loans too (`wektu` ← وقت, `kabar`, `dunya`, `ilmu`, `masalah`, `sholat`): fill it when sure. |
| `examples` | Natural **ngoko** sentences (or the word's own level), 4–12 words, as a friend would say or text them. Translations in all three voices. |
| `status` | `"unverified"` (always, for Javanese, until a native speaker approves). |
| `provenance` | `{"generator":"claude-code","prompt":"gloss-jv@v1"}` |

## Egyptian analogies

Where it helps, the `arz` note can compare levels with Egyptian address:
ngoko ≈ «إنت», krama ≈ «حضرتك», krama inggil ≈ «سيادتك», while making clear that
Javanese swaps whole vocabularies, not just pronouns.
