# Attribution

Every dataset, corpus, font and sound used by Kabar, with its license.

## Data

| Dataset | Used for | License | Where |
|---|---|---|---|
| [wordfreq](https://github.com/rspeer/wordfreq) 3.1.1, `small_id` (Robyn Speer et al.; built from Wikipedia, OpenSubtitles 2018 and Twitter via Exquisite Corpus) | Indonesian frequency list | data CC BY-SA 4.0 (code Apache-2.0) | `content/freq/id.tsv` (derived, CC BY-SA 4.0) |
| [hermitdave/FrequencyWords](https://github.com/hermitdave/FrequencyWords) 2018 `id_full` (Hermit Dave; from [OpenSubtitles 2018](https://opus.nlpl.eu/OpenSubtitles/corpus/version/OpenSubtitles), P. Lison & J. Tiedemann, LREC 2016) | Indonesian frequency list | CC BY-SA 4.0 (code MIT) | `content/freq/id.tsv` |
| [Javanese Wikipedia](https://jv.wikipedia.org) articles dump | Javanese frequency list (built on the Mac) | CC BY-SA 4.0 | `content/freq/jv.tsv` |
| [Wiktionary](https://en.wiktionary.org) via [kaikki.org](https://kaikki.org) (wiktextract, Tatu Ylonen) | dictionary import, Arabic etymologies for the Arabic Bridge cross-check | CC BY-SA 4.0 and GFDL | `build/dict/`, `content/bridge/wiktionary-*.jsonl` |
| [Natural Earth](https://www.naturalearthdata.com/) 1:10m Admin 0 countries (`ne_10m_admin_0_countries`, via [nvkelso/natural-earth-vector](https://github.com/nvkelso/natural-earth-vector)) | Journey Map base maps: clipped, simplified and restyled by `cargo run -p kabar-tools --bin journey-map` ("Made with Natural Earth") | public domain | `crates/app/assets/journey/*.svg` |

Glosses, examples and Arabic Bridge cards under `content/glosses/` and `content/bridge/` are original, written for Kabar with Claude Code (`prompts/gloss@v1.md`, `prompts/bridge@v1.md`) and reviewed by the learner. The About screen must credit Wiktionary (CC BY-SA) and the frequency sources above.

## Curriculum references
Cited for structure only (unit titles, grammar topics, competency wording); no text or
exercises are copied. Kabar's lessons and grammar explanations are original.
- Permendikbud No. 27 Tahun 2017, SKL Kursus dan Pelatihan BIPA — Indonesian government regulation.
- *Sahabatku Indonesia* BIPA 1–7 (2019), Badan Pengembangan dan Pembinaan Bahasa, Kemendikbud — free government textbooks (bipa.kemendikdasmen.go.id).
- Full source list with URLs: `curriculum/indonesian.yaml` → `sources`.

## Fonts
_None bundled yet._ The UI uses system fonts and falls back through IBM Plex Sans Arabic / Noto (OFL) if installed. Bundled fonts arrive with the visual assets.

## Sounds
_None yet._

## On-device models and system frameworks
- **Google ML Kit Text Recognition v2** (`com.google.mlkit:text-recognition:16.0.1`, Latin script, model bundled in the APK; used by `android/` for the Screenshot Decoder on Android). Free of charge; proprietary, governed by the [ML Kit Terms of Service](https://developers.google.com/ml-kit/terms) and the [Google APIs Terms of Service](https://developers.google.com/terms). Recognition runs entirely on the device: images and recognized text are not sent to Google. Per Google's [ML Kit data disclosure](https://developers.google.com/ml-kit/android-data-disclosure), ML Kit may send anonymous usage and performance metrics (device info, API latency), never image content.
- **Apple Vision** (`VNRecognizeTextRequest`): macOS system framework, used for the Screenshot Decoder on the Mac under the macOS license; on-device, nothing bundled.

## Artwork
- Custom UI icons (`assets/icons/custom/`): original SVGs drawn for Kabar on the Lucide 24 px grid (`docs/VISUAL-ASSETS.md`, prompt 3). The preview page loads Lucide 0.456.0 (ISC) from unpkg for comparison only.
- App icon ("Dawn over sawah"): original artwork created for Kabar with Codex (`docs/VISUAL-ASSETS.md`, prompts 1–2). Masters in `assets/src/icon/`; every platform size is exported by `scripts/assets/export_icons.py`.
- Background pattern: original kawung-inspired geometry generated in code (`crates/app/src/theme.rs`).
