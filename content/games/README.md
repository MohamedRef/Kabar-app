# Mini-game content (`content/games/`)

Build-time content for the mini-games (spec §14), written by Claude Code and
reviewed by the learner. The game logic lives in `crates/core/src/games/`;
each module's tests are the content gate for its file.

## Pasar! (`pasar.toml`)

The items you haggle for and everything the seller says. Parsed by
`crates/core/src/games/pasar.rs` (`PasarData::parse`). Content gate:
`cargo test -p kabar-core games::pasar`.

**Review status:** `reviewed = false`. Set it to `true` only after the learner
has read and approved every item and line in all three languages. The
Javanese lines stay flagged **unverified** until then.

### Items

```toml
[[items]]
id = "kaos_batik"            # stable snake_case id
name = "kaos batik"          # as the seller says it
meaning = { arz = "…", ar = "…", en = "…" }
ask = 85000                  # the seller's first price
floor = 55000                # lowest price the seller accepts (never shown)
fair = 60000                 # what a local would pay
javanese = false             # true: the seller speaks Javanese market speech
bonus = false                # true for the one bonus round
```

- `0 < floor <= fair <= ask`, and every price is a round market price for
  its size (multiples of 500 under Rp20.000, 1.000 under Rp200.000, 5.000
  under Rp1.000.000, 50.000 under Rp10.000.000, 100.000 above).
- The bonus item is the marketplace listing from the spec's test texts (§24):
  *iphone 17 pm 512gb resmi ibox mulus nomin garansi jalan*.
- An item may carry its own `[[items.lines]]` (same format as below). For a
  situation it covers, they replace the shared lines; the bonus seller uses
  this to talk like a marketplace seller ("nego tipis", "harga nett").

### Seller lines

```toml
[[lines]]
situation = "quote"          # see the table below
javanese = false             # true: Javanese market speech
text = "Harganya {price}, Kak."
arz = "سعره ⁨{price}⁩ يا أستاذ."
ar = "سعره ⁨{price}⁩ يا أخي."
en = "It's {price}."
```

| `situation` | When | `{price}` |
|---|---|---|
| `greeting` | Opening words, before the first price | no |
| `quote` | Naming the current price ("Berapa?") | yes |
| `discount` | Lowering the price after "Boleh kurang?" (at most twice) | yes |
| `counter` | Answering an offer with a price in between | yes |
| `refuse_low` | Politely refusing a much-too-low offer, with a counter | yes |
| `accept` | Agreeing to a price | yes |
| `call_back` | Calling the learner back after they walked away (once) | yes |
| `final` | The last price: "harga pas", "wis pas" | yes |
| `let_go` | Letting the learner go, kindly | no |

- Every situation has at least two Indonesian and two Javanese variants. The
  game rotates through them from a seed-chosen start, so the same seed gives
  the same game on every device.
- `{price}` is the only placeholder. In `text` it is **spoken**: Indonesian
  words ("dua puluh lima ribu") or, for a Javanese seller, Javanese words
  ("selawe ewu"). In the meanings it is **written** as shops write it
  ("Rp25.000"). A `{price}` that starts a sentence is capitalized.
- In `arz` and `ar`, every Latin snippet and every `{price}` sits between
  FSI (U+2068) and PDI (U+2069) so right-to-left text renders correctly.
- The Indonesian seller calls the learner "Kak"; the Javanese seller "Mas"
  (the learner is addressed in the masculine, as in the rest of the app).
- Javanese lines are market ngoko with the polite krama words sellers use with
  customers (*monggo, nggih, mawon, ngapunten, mboten saged*), and the market
  words the spec asks for: *larang, murah, wis pas, ora isa kurang* (the
  learner's own lines, with *piro*, are in `pasar::phrase`).

### Tone

Friendly market banter. The seller jokes about thin margins and calls you
back, but never insults, pressures or shames the buyer. No more than one
exclamation mark per line. The tests reject a list of insults in every
language.

## Unggah-Ungguh Simulator (`unggah-ungguh.toml`)

Short Javanese scenes where each turn offers the same thing said at different
speech levels; the Respect Meter rises when the choice fits the person and
dips when it doesn't, with a kind explanation every time. Parsed by
`crates/core/src/games/unggah.rs` (`UnggahData::parse`) together with
`content/javanese/speech-levels.jsonl`. Content gate:
`cargo test -p kabar-core games::unggah`.

**Review status:** `reviewed = false`. Every scene shows the **unverified**
pill until the learner approves it and this is set to `true`.

### Format

```toml
[[scenes]]
id = "uu.grandmother"        # stable id; the curriculum links to it
level = "j3"                 # Javanese level that opens it (j1, j2, j3)
title.arz = "…"              # every text below is { arz, ar, en }
setting.arz = "…"
partner.name = "Mbah Sri"    # its words may appear in any line
partner.owed = "krama"       # ngoko | madya | krama | krama_inggil | krama_andhap
partner.role.arz = "…"

[[scenes.turns]]
id = "eaten"                 # unique within the scene
situation.arz = "…"
partner_says.text = "Wis. Kowe wis mangan?"          # optional
partner_says.uses = ["already:ngoko", "you_singular:ngoko", "eat:ngoko"]
partner_says.meaning.arz = "…"

[[scenes.turns.choices]]     # three per turn, exactly one `fits`
text = "Sampun, Mbah. Kula sampun nedha."
level = "krama"              # the sentence's overall level
fit = "fits"                 # fits | too_plain | too_stiff | self_raised
uses = ["already:krama", "i_me:krama", "eat:krama"]
meaning.arz = "…"
why.arz = "…"                # shown after every pick
```

- **`uses`** lists every speech-level form in the line as `row_id:level`. The
  row must exist in `speech-levels.jsonl`, it must list a form at exactly
  that level, and one of those forms must appear in the text as whole words
  (case and punctuation ignored). The tests go further: every word of every
  line must be a listed form or part of the partner's name, so the game never
  teaches a form the table doesn't have. When the table and memory disagree,
  the table wins; a missing word means a new table row first.
- **`fit`**: `fits` (+15 on the meter), `too_stiff` (−5: odd, not rude, e.g.
  krama to a close friend), `too_plain` (−20: ngoko or plain krama where
  honour words are owed), `self_raised` (−25: an honour word about yourself,
  the classic *kula dhahar*). The meter starts at 60, stays within 0–100, and
  80 or more at the end is "Sopan!".
- Choice order is shuffled per turn from the game seed.
- In `arz` and `ar`, every Latin snippet sits between FSI (U+2068) and PDI
  (U+2069). `en` has no Arabic.

### Scenes

| id | Level | Partner | Fits |
|---|---|---|---|
| `uu.friend` | J1 | Bayu, a friend your age | ngoko |
| `uu.market` | J2 | Bu Darmi, an older fruit seller | polite market speech (light krama with madya words) |
| `uu.grandmother` | J3 | Mbah Sri, a friend's grandmother | krama; krama inggil for her, krama/andhap for you |
| `uu.teacher` | J3 | Pak Harjo, your teacher | krama; krama inggil for him |
| `uu.elder` | J3 | Pak Karto, an elderly neighbour | krama; krama inggil for him |

`uu.grandmother` opens with "Have you eaten?" at three levels
(*Kowe wis mangan? / Sampeyan sampun nedha? / Panjenengan sampun dhahar?*),
the same sentence as lesson `j3.u01.l04` (Respect Meter).

### Tone

Explanations are warm and specific: say what the choice does and what fits
better, never shame. The tests reject a list of shaming words (en and ar).
Elders speak ngoko to the learner (they may); the learner answers upward.

## Minimal Pairs (`minimal-pairs.toml`)

Javanese d/dh and t/th (spec §13). Javanese d and t are dental, like Arabic
د and ت; dh and th are made with the tongue tip curled back behind the gum
ridge. Parsed by `crates/core/src/games/pairs.rs` (`PairsData::parse`).
Content gate: `cargo test -p kabar-core games::pairs`.

**Review status:** `reviewed = false`. Every pair is Javanese content and
shows as **unverified** until the learner approves the file. `confidence`
(`high`, `medium`, `low`) is how sure we are of the pair itself: both words
real, with these spellings and meanings. Fewer correct pairs beat many
doubtful ones; spelling variants (chat spellings such as *dewe* for *dhewe*)
are never pairs.

```toml
[[tips]]                     # one per contrast, for an Egyptian speaker
contrast = "d_dh"            # or "t_th"
arz = "…"
ar = "…"
en = "…"

[[pairs]]
id = "mp.wedi-wedhi"         # mp.<plain>-<curled>; lessons link to it
contrast = "d_dh"
kind = "minimal"             # minimal: two Javanese words
                             # cross: Indonesian word / Javanese cousin
plain = { word = "wedi", lang = "jv", meaning = { arz = "…", ar = "…", en = "afraid" } }
curled = { word = "wedhi", lang = "jv", meaning = { arz = "…", ar = "…", en = "sand" } }
level = "j1"                 # j1, j2 or j3 on the Javanese track
confidence = "high"
note = { arz = "…", ar = "…", en = "…" }   # optional
```

- The curled word is the plain word with each `d`/`t` that carries the
  contrast written `dh`/`th` (`dada` → `dhadha`); the parser checks this and
  the id.
- The curled side is always Javanese (`jv`). A `minimal` pair's plain side is
  Javanese too; a `cross` pair's plain side is Indonesian (`id`).
- `j1` is for easy, common pairs met alongside the first lessons that
  introduce dh/th (*dhewe, pakdhe*); `j2`/`j3` for the rest.
- A game uses `minimal` pairs first and fills the rest with `cross` pairs.
  Listen mode is offered only with a real Javanese voice; Indonesian voices
  can't make the curled sounds.
- The tips say dh/th may *feel* a bit like ض/ط in weight, but never that they
  are the same sound: the tongue sits in a different place.
- In `arz` and `ar`, every Latin snippet sits between FSI (U+2068) and PDI
  (U+2069).
