# Kabar custom icons

Original, created for Kabar; same license as the project

22 hand-authored SVG icons, built to sit beside Lucide at a 24 × 24 viewBox with 2px round strokes. These are original drawings, not copies or traces of Lucide icons or any existing batik motifs.

## Revisions

- `false-friend`, `card-wall`, `souvenir` and `krama-inggil` were redrawn after review: the first two were hard to read, `souvenir` was a document instead of a luggage tag, and the two bow lines on `krama-inggil` merged into one at 24 px.

## Icon inventory

| File | Meaning | Closest Lucide reference | Mirror for RTL? |
| --- | --- | --- | --- |
| `bridge-word.svg` | Arabic-origin word / shared vocabulary marker | `landmark` | Yes |
| `false-friend.svg` | Words that look familiar but have different meanings | `messages-square` | Yes |
| `souvenir.svg` | Collected cultural mementos and vocabulary keepsakes | `tag` | Yes |
| `suitcase.svg` | Travel vocabulary and saved journey items | `briefcase` | Yes |
| `take-a-flight.svg` | Placement test / starting the learning journey | `plane-takeoff` | No — directional meaning |
| `journey-island.svg` | An island destination in the learning journey | `palmtree` | Yes |
| `train-stop.svg` | Train stop / a milestone along the route | `train-front` | Yes |
| `respect-meter.svg` | Context-dependent Javanese politeness / respect level | `gauge` | No — directional meaning |
| `ngoko.svg` | Ngoko: everyday, familiar Javanese register (open palm) | `hand` | Yes |
| `krama.svg` | Krama: polite Javanese register (one bow line) | `hand` | Yes |
| `krama-inggil.svg` | Krama inggil: highest respect / honorific register (two bow lines) | `hand` | Yes |
| `pasar.svg` | Pasar: traditional market / marketplace lesson | `store` | Yes |
| `rupiah.svg` | Indonesian money / rupiah currency content | `banknote` | Yes |
| `kabar-pagi.svg` | Morning learning digest: sunrise and an open book | `book-open` | Yes |
| `kabar-radio.svg` | Audio segment / radio-style listening practice | `radio` | Yes |
| `sinetron.svg` | TV-style story scene / Indonesian drama lesson | `tv` | Yes |
| `coach.svg` | Supportive learning coach / encouraging feedback | `message-circle` | Yes |
| `lasso.svg` | Select and annotate words / freehand selection tool | `pencil` | No — directional meaning |
| `card-wall.svg` | Collection of cards / vocabulary card wall | `layers` | Yes |
| `gong.svg` | Javanese gong / gamelan cultural content | `music` | Yes |
| `kendang.svg` | Kendang drum / Javanese rhythm and culture | `music-2` | Yes |
| `streak-ember.svg` | Encouraging learning streak / calm flame with a leaf | `flame` | Yes |

## RTL guidance

All custom icons remain legible when reflected. Normally it is appropriate to mirror them in RTL layout.

Do **not** automatically mirror the following directional icons:

- `take-a-flight.svg`: the ascending plane conveys a chosen travel direction.
- `respect-meter.svg`: the needle position represents a gauge reading; mirroring changes the level.
- `lasso.svg`: the pen-tip endpoint communicates the direction of selection/annotation.

The remaining 19 custom icons are safe to mirror. The three Javanese hand variants use the same hand drawing; the only additions are one (`krama`) or two (`krama-inggil`) bow lines above it.

## Usage

- Each SVG has `fill="none"`, `stroke="currentColor"`, `stroke-width="2"`, round line caps and joins, and a 24×24 viewBox. Do not rasterize for normal interface use.
- In the app, **inline** the SVG or import it as a component so it inherits the surrounding CSS `color`. Example: `<span style="color: var(--accent)">...inline SVG...</span>`. The tiny solid dots in `bridge-word` and `false-friend` also use `currentColor`.
- If loading via `<img src="...">`, `currentColor` will not inherit from the parent CSS in most browsers. The preview handles white on charcoal by applying a CSS filter to the embedded file images.
- The `preview.html` page works locally for all Kabar SVGs. The reference column needs an internet connection because it loads Lucide **0.456.0** from a pinned unpkg CDN URL.
- The preview includes every custom icon at 16, 20, 24 and 48 px on white, on `#22201E`, and mirrored horizontally.

## Color policy

Icons do not hard-code a hue. Use `currentColor` to apply Merapi charcoal, Bridge gold, status colors or other tokens controlled by the Kabar design system. The only use of `fill` is tiny solid dots; there is no text or lettering.
