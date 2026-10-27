# Attribution

Every dataset, corpus, font and sound used by Kabar, with its license.

## Data

| Dataset | Used for | License | Where |
|---|---|---|---|
| [wordfreq](https://github.com/rspeer/wordfreq) 3.1.1, `small_id` (Robyn Speer et al.; built from Wikipedia, OpenSubtitles 2018 and Twitter via Exquisite Corpus) | Indonesian frequency list | data CC BY-SA 4.0 (code Apache-2.0) | `content/freq/id.tsv` (derived, CC BY-SA 4.0) |
| [hermitdave/FrequencyWords](https://github.com/hermitdave/FrequencyWords) 2018 `id_full` (Hermit Dave; from [OpenSubtitles 2018](https://opus.nlpl.eu/OpenSubtitles/corpus/version/OpenSubtitles), P. Lison & J. Tiedemann, LREC 2016) | Indonesian frequency list | CC BY-SA 4.0 (code MIT) | `content/freq/id.tsv` |
| [NusaWrites](https://github.com/IndoNLP/nusa-writes) (commit `b337f51`), NusaAlinea paragraphs + NusaKalimat, Javanese (S. Cahyawijaya, H. Lovenia et al., IJCNLP-AACL 2023) | Javanese frequency list | Apache-2.0 | `content/freq/jv.tsv` |
| [NusaX](https://github.com/IndoNLP/nusax) (commit `75b0be5`), sentiment, Javanese (G. I. Winata, A. F. Aji et al., EACL 2023) | Javanese frequency list | CC BY-SA 4.0 | `content/freq/jv.tsv` (derived, CC BY-SA 4.0) |
| [Javanese Wikipedia](https://jv.wikipedia.org) articles dump | Javanese frequency list (built on the Mac) | CC BY-SA 4.0 | `content/freq/jv.tsv` |
| [Wiktionary](https://en.wiktionary.org) via [kaikki.org](https://kaikki.org) (wiktextract, Tatu Ylonen) | dictionary import, Arabic etymologies for the Arabic Bridge cross-check | CC BY-SA 4.0 and GFDL | `build/dict/`, `content/bridge/wiktionary-*.jsonl` |
| [Wiktionary](https://en.wiktionary.org) definition API (`/api/rest_v1/page/definition/<word>`) | Teach Kabar's free in-app lookup (spec §26): the excerpt is kept with the learner's taught entry, with a link back to the page | CC BY-SA 4.0 and GFDL | `taught_entries` sources (on the learner's devices only) |
| [g2p_id](https://github.com/bookbot-kids/g2p_id) `g2p_id/resources/lexicon_id.tsv` (Bookbot; built on [ipa-dict](https://github.com/open-dict-data/ipa-dict) `ma.txt`, MIT) | Indonesian pronunciations (e vs é, glottal k) for the Arabic-script pronunciation, cut by `kabar-tools pron import` | Apache-2.0 (`content/pron/g2p_id-LICENSE.txt`) | `content/pron/id.tsv` (derived: 18k of its 27.6k words, recoded) |
| [Natural Earth](https://www.naturalearthdata.com/) 1:10m Admin 0 countries (`ne_10m_admin_0_countries`, via [nvkelso/natural-earth-vector](https://github.com/nvkelso/natural-earth-vector)) | Journey Map base maps: clipped, simplified and restyled by `cargo run -p kabar-tools --bin journey-map` ("Made with Natural Earth") | public domain | `crates/app/assets/journey/*.svg` |

Glosses, examples, Arabic Bridge cards, the Javanese speech-level table and the golden sets under `content/glosses/`, `content/bridge/`, `content/javanese/` and `content/golden/` are original, written for Kabar with Claude Code (`prompts/gloss@v1.md`, `prompts/gloss-jv@v1.md`, `prompts/bridge@v1.md`, `prompts/speech-levels@v1.md`) and reviewed by the learner; Javanese stays unverified until a native speaker checks it. The golden screenshots in `fixtures/golden/` are synthetic, rendered for Kabar with fictional people. The About screen must credit Wiktionary (CC BY-SA) and the frequency sources above.

Kak Nur's lines (`content/coach/`), Kabar Pagi stories (`content/pagi/`) and Pasar! seller lines, Unggah-Ungguh scenes and Minimal Pairs (`content/games/`) are original, written for Kabar with Claude Code and reviewed by the learner.

## Curriculum references
Cited for structure only (unit titles, grammar topics, competency wording); no text or
exercises are copied. Kabar's lessons and grammar explanations are original.
- Permendikbud No. 27 Tahun 2017, SKL Kursus dan Pelatihan BIPA — Indonesian government regulation.
- *Sahabatku Indonesia* BIPA 1–7 (2019), Badan Pengembangan dan Pembinaan Bahasa, Kemendikbud — free government textbooks (bipa.kemendikdasmen.go.id).
- Full source list with URLs: `curriculum/indonesian.yaml` → `sources`.

## Fonts
The UI uses system fonts and falls back through IBM Plex Sans Arabic / Noto (OFL) if installed.

- **Noto Naskh Arabic** Regular (notofonts.github.io, hinted) — © 2022 The Noto Project Authors, SIL Open Font License 1.1 (`content/fonts/NotoNaskhArabic-OFL.txt`). Subset to the Arabic block as `content/fonts/NotoNaskhArabic-Kabar.ttf` (`pyftsubset --unicodes=U+0020,U+0021,U+002D,U+002E,U+0600-06FF,U+200C-200F,U+25CC --layout-features='*'`) and bundled for the Arabic-script pronunciation, which needs ڠ ێ ۆ پ چ and full vowel marks on every device.
- **Noto Sans Javanese** Regular v2.005 — © 2022 The Noto Project Authors, SIL Open Font License 1.1 (`content/fonts/NotoSansJavanese-OFL.txt`). Bundled in the app for the Aksara Jawa practice sheets, and the source that `scripts/aksara/strokes.py` traces the draft stroke data in `content/aksara/strokes.json` from (a derived dataset, same license terms apply to the font itself).

## Sounds
- **Offline pack audio** (`packs` GitHub release, built by `kabar-tools packs`): Kabar's own lesson lines read by **Microsoft Azure AI Speech** neural voices id-ID-GadisNeural, id-ID-ArdiNeural, jv-ID-SitiNeural and jv-ID-DimasNeural, generated on the free F0 tier. Used under the [Microsoft Product Terms](https://www.microsoft.com/licensing/terms/) for Azure AI Speech and its [Code of Conduct for text to speech](https://learn.microsoft.com/legal/ai-code-of-conduct). The clips are synthetic voices reading Kabar's own text; they are not recordings of real people.

## On-device models and system frameworks
- **Google ML Kit Text Recognition v2** (`com.google.mlkit:text-recognition:16.0.1`, Latin script, model bundled in the APK; used by `android/` for the Screenshot Decoder on Android). Free of charge; proprietary, governed by the [ML Kit Terms of Service](https://developers.google.com/ml-kit/terms) and the [Google APIs Terms of Service](https://developers.google.com/terms). Recognition runs entirely on the device: images and recognized text are not sent to Google. Per Google's [ML Kit data disclosure](https://developers.google.com/ml-kit/android-data-disclosure), ML Kit may send anonymous usage and performance metrics (device info, API latency), never image content.
- **Apple Vision** (`VNRecognizeTextRequest`): macOS system framework, used for the Screenshot Decoder on the Mac under the macOS license; on-device, nothing bundled.

## Artwork
- Custom UI icons (`assets/icons/custom/`): original SVGs drawn for Kabar on the Lucide 24 px grid (`docs/VISUAL-ASSETS.md`, prompt 3). The preview page loads Lucide 0.456.0 (ISC) from unpkg for comparison only.
- App icon ("Dawn over sawah"): original artwork created for Kabar with Codex (`docs/VISUAL-ASSETS.md`, prompts 1–2). Masters in `assets/src/icon/`; every platform size is exported by `scripts/assets/export_icons.py`.
- Souvenir cards: original artwork created for Kabar. Six cards were drawn with Codex (`docs/VISUAL-ASSETS.md`, prompts 4b–4c); the other 101 are drawn in code by `scripts/assets/draw_souvenirs.py`. Masters are in `assets/src/souvenirs/`, and `scripts/assets/export_souvenirs.py` exports the web sizes.
- Journey Map art (paper backdrops and 13 landmark vignettes): original artwork created for Kabar with Codex (`docs/VISUAL-ASSETS.md`, prompt 5). Masters in `assets/src/map/`; WebP sizes exported by `scripts/assets/export_map.py` and embedded by `crates/app/src/map_art.rs`. The map geometry itself is Natural Earth (see above).
- Background pattern: original kawung-inspired geometry generated in code (`crates/app/src/theme.rs`).
