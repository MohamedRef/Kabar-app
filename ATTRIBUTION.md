# Attribution

Every dataset, corpus, font and sound used by Kabar, with its license.

## Data
_None yet._ (Wiktionary via kaikki.org, CC BY-SA, arrives in M3.)

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
