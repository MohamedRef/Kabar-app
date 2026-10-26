# Attribution

Every dataset, corpus, font and sound used by Kabar, with its license.

## Data

| Dataset | Used for | License | Where |
|---|---|---|---|
| [wordfreq](https://github.com/rspeer/wordfreq) 3.1.1, `small_id` (Robyn Speer et al.; built from Wikipedia, OpenSubtitles 2018 and Twitter via Exquisite Corpus) | Indonesian frequency list | data CC BY-SA 4.0 (code Apache-2.0) | `content/freq/id.tsv` (derived, CC BY-SA 4.0) |
| [hermitdave/FrequencyWords](https://github.com/hermitdave/FrequencyWords) 2018 `id_full` (Hermit Dave; from [OpenSubtitles 2018](https://opus.nlpl.eu/OpenSubtitles/corpus/version/OpenSubtitles), P. Lison & J. Tiedemann, LREC 2016) | Indonesian frequency list | CC BY-SA 4.0 (code MIT) | `content/freq/id.tsv` |
| [Javanese Wikipedia](https://jv.wikipedia.org) articles dump | Javanese frequency list (built on the Mac) | CC BY-SA 4.0 | `content/freq/jv.tsv` |
| [Wiktionary](https://en.wiktionary.org) via [kaikki.org](https://kaikki.org) (wiktextract, Tatu Ylonen) | dictionary import, Arabic etymologies for the Arabic Bridge cross-check | CC BY-SA 4.0 and GFDL | `build/dict/`, `content/bridge/wiktionary-*.jsonl` |

Glosses, examples and Arabic Bridge cards under `content/glosses/` and `content/bridge/` are original, written for Kabar with Claude Code (`prompts/gloss@v1.md`, `prompts/bridge@v1.md`) and reviewed by the learner. The About screen must credit Wiktionary (CC BY-SA) and the frequency sources above.

## Fonts
_None bundled yet._ The UI uses system fonts and falls back through IBM Plex Sans Arabic / Noto (OFL) if installed. Bundled fonts arrive with the visual assets.

## Sounds
_None yet._

## Artwork
- Background pattern: original kawung-inspired geometry generated in code (`crates/app/src/theme.rs`).
