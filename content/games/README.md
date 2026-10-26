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
