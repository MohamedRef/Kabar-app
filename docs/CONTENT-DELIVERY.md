# Content delivery: what we measured and what we decided

The roadmap asked two questions about the signed content bundle: should it
be split into modules, and should devices download only what changed? Both
were measured on real history before deciding. This page keeps the numbers
and the decisions, and says when to look again.

## How it was measured

The bundle of each of the last 60 first-parent commits on `main` (29 Oct to
1 Nov 2026, about 17 merges a day) was built with today's
`kabar-tools update bundle` from `git archive` of that commit. Consecutive
bundles were then compared record by record. A record is one key with its
DEFLATE data (`crates/protocol/src/bundle.rs`).

To repeat it, build each commit's bundle into a folder and compare the
records of neighbours: `key len (u32 LE), key, data len (u32 LE), data`
after the 8-byte magic.

## What the bundle holds

The bundle is 9.83 MB in 1,777 records:

| Records | Count | Size |
|---|---:|---:|
| `LESSONS/*` | 1,222 | 4.15 MB |
| `GLOSS_BATCHES/*` | 100 | 2.34 MB |
| `GLOSS_BATCHES_JV/*` | 30 | 0.83 MB |
| `STORIES/*` | 107 | 0.71 MB |
| `RADIO/*` | 107 | 0.60 MB |
| `GRAMMAR/*` | 160 | 0.45 MB |
| `FREQ_ID` | 1 | 0.23 MB |
| `TESTS/*` | 14 | 0.14 MB |
| everything else (personas, CSS, bridge, curricula, speech levels, cast) | 36 | 0.38 MB |

## How it changes

Each of the 59 merges compared with the one before:

| What changed | Merges | Bytes that differ |
|---|---:|---|
| Nothing (byte-identical bundle) | 37 | 0 |
| Stylesheets only | 21 | 0.9 to 42.8 kB (at most 0.43 %) |
| Content | 1 | 1.26 MB (12.8 %: 102 stories, 17 lessons, Radio, glosses) |

Before #523 and #524, every merge cost every device a full 9.8 MB download,
and so did every app update.

## Decisions

### Identical content: never downloaded again (done)

- #523 skips a bundle whose digest matches one the device already keeps.
  It also remembers a bundle this app build can't read, until the app
  changes.
- #524 bakes the digest of the built-in content into the preview APK, so
  an app update doesn't download the content it already carries.

Together these remove 37 of the 59 downloads, plus one per app update. They
needed no format or protocol change.

### Modular bundles: not now

Splitting the bundle into separately signed modules (per level, or lessons
apart from glosses) would not save much and would cost more than it is worth:

- **No module is optional.** The learner works through every level offline,
  the Reader needs all glosses, and lessons point at grammar, stories and
  Radio. Every device would fetch every module anyway.
- **What changes is spread out.** The one content merge touched stories,
  lessons, Radio, glosses, grammar, personas and tests at once, so module
  boundaries would not have isolated it.
- **Validation and recovery are whole-bundle.** `bundle_check::check_files`
  checks ids across curricula, lessons and tests together, and the
  last-known-good, rejection and crash-loop rules all act on one digest.
  Modules would need a consistent set of versions, a validator across
  modules, and a last-known-good set. That multiplies the failure cases the
  recovery code has to get right.
- **Audio is already separate.** Large media lives in audio packs, which
  are installed on demand.

### Record-level differential download: not now, with a trigger

The 21 stylesheet-only merges still download 9.8 MB to change at most
43 kB. A record-level delta would cut that by over 99 %, and the content
merge by 87 %. The design would add no new artifact:

1. Publish a signed record index (key, offset, length, digest per record)
   as an optional manifest field.
2. Fetch only the changed records from the existing bundle file with HTTP
   `Range`.
3. Rebuild the bundle locally and check the whole-file SHA-256 against the
   signed manifest before anything else happens.
4. Fall back to a full download on any mismatch.

It is not built yet because:

- **The cost is low.** Automatic downloads already run on Wi-Fi only,
  never on a metered network or with Android's Data Saver on.
- **The cadence will drop.** Release channels publish far less often than
  `preview`, which follows every merge during development.
- **The risk is real.** It adds a new failure surface (index, ranges,
  reassembly) to the path that must never brick content.

Build it when one of these becomes true:

- automatic downloads are allowed on mobile data;
- the bundle grows past about 25 MB;
- a release channel starts publishing stylesheet-only updates more than
  weekly.
