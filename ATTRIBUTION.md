# Attribution

Every dataset, corpus, font and sound used by Kabar, with its license.

## Data
- **Natural Earth** 1:10m Admin 0 countries (`ne_10m_admin_0_countries`), public domain — <https://www.naturalearthdata.com/>, via <https://github.com/nvkelso/natural-earth-vector>. Clipped, simplified and restyled into the Journey Map base maps (`crates/app/assets/journey/*.svg`) by `cargo run -p kabar-tools --bin journey-map`. "Made with Natural Earth."
- (Wiktionary via kaikki.org, CC BY-SA, arrives in M3.)

## Fonts
_None bundled yet._ The UI uses system fonts and falls back through IBM Plex Sans Arabic / Noto (OFL) if installed. Bundled fonts arrive with the visual assets.

## Sounds
_None yet._

## Artwork
- Custom UI icons (`assets/icons/custom/`): original SVGs drawn for Kabar on the Lucide 24 px grid (`docs/VISUAL-ASSETS.md`, prompt 3). The preview page loads Lucide 0.456.0 (ISC) from unpkg for comparison only.
- App icon ("Dawn over sawah"): original artwork created for Kabar with Codex (`docs/VISUAL-ASSETS.md`, prompts 1–2). Masters in `assets/src/icon/`; every platform size is exported by `scripts/assets/export_icons.py`.
- Background pattern: original kawung-inspired geometry generated in code (`crates/app/src/theme.rs`).
