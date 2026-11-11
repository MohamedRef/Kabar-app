# Infrastructure parity: Batna ↔ Kabar

Engineering audit F24 (2026-10-29). Batna and Kabar are separate apps in
separate repos and stay that way: no shared crate, no merged repo, no mass
renames. But much of their plumbing was copied from one to the other
(sync, store, install identity, auth, updates), and a fix that lands in one
can miss the other. This file is the same in both repos.

**Rule:** a PR that fixes or changes anything in the table below says in its
description whether the sibling app needs the same change, and either links
the sibling PR or says why not ("intentional difference"). Update this
table in both repos when a row changes.

Paths: **B** = `MohamedRef/Batna-app` `crates/app/src/…`, **K** =
`MohamedRef/Kabar-app` `crates/app/src/…` (core/protocol/server paths are
spelled out).

| Area | Contract (what both must do) | Batna | Kabar | Same test idea in both |
|---|---|---|---|---|
| Install identity | A database copied from another install takes a new device id (marker outside backups) | B `store/install.rs` | K `store/install.rs` | restored copy gets its own id, outbox restamped |
| Sync outbox | Every local write to a synced table queues an op in the same transaction | B `store/sync.rs` | K `store.rs` + `store/sync_rows.rs` | write → one op; LWW keeps only the newest |
| Refused ops (F12, F13) | Ops the hub refuses for good, and unreadable outbox rows, are held with their reason, shown in Settings, sent again or let go; newest 1000 | B `store/held.rs`, `cloud/held_ui.rs` (`sync_held`) | K `store/held.rs`, `account/held.rs` (`held_ops`) | refused → held → sent again; unreadable → held, rest still go; trim |
| Unknown table / clock skew | Ops the hub can't take *yet* wait instead of being dropped | B `crates/protocol/src/client.rs`: `ClockSkew` and `UnknownTable` stay in the outbox | K: every refusal is held (`held_ops`) and «ابعتها تاني» re-offers it | intentional for now; port Batna's waiting if Kabar's Worker deploys can lag |
| Lock waits (F16) | UI connection waits ≤ 750 ms for the sync lock, sync connection ≤ 5 s; writes begin IMMEDIATE; export snapshot deferred | B `store/contention.rs` | K `store/contention.rs` | hold `BEGIN IMMEDIATE` elsewhere, time the UI write |
| Export snapshot (F14) | Export reads every table in one deferred read transaction | B `store/export.rs` | K `store/export.rs` | a write from another connection mid-export is left out |
| Atomic lesson finish (F09) | Finish event, cards and cleared place land together or not at all, with Retry | B `store/places.rs` | K `Store::atomically`, `lesson/player.rs` | a refused finish leaves nothing half-written |
| Read failures (F10) | A failed read is announced with "read again", never shown as empty | B `store/health.rs` | K `persistence/reads.rs` | a broken read shows the notice, keeps the last good copy |
| Save failures (F02, F03) | Every grading/settings write acts on its result; nothing fire-and-forget | B `record_logged`, `bank/unsaved.rs` | K F09/F10 paths | `query_only` refuses, then accepts the retry |
| Google JWKS (F20) | Forced refetch for an unknown `kid` is rate-limited | B `crates/server/src/google.rs` `may_refetch` | K `crates/server/src/google.rs` `may_refetch` | unknown kid twice → one fetch |
| Content bundle bounds (F18) | Every inflate is bounded; a many-file bundle also bounds entries and key length | B `crates/core/src/pack.rs` `MAX_UNPACKED` (one JSON blob, no entries) | K `crates/protocol/src/bundle.rs`, `embedded.rs` | oversized bundle refused |
| Study-day clock (F19) | Past moments read with the offset in force then (device zone history) | B `core/src/zone.rs`, `store::zone` | K `core/src/srs.rs` `DayClock::with_history`, `zone.rs` | DST change keeps a past answer's day |
| Content check binding (F25) | A "content is right" check is bound to the content's fingerprint | B: Mohamed's verdicts carry the item `fingerprint` (`review::verify`) | K `core/src/fingerprint.rs`, `LessonVerified.fingerprint` | changed content asks again |
| Answer replay (F15) | A local in-order answer steps one item; full replay stays the oracle | B `core/src/replay.rs` | K: replays per screen, cached by `change_stamp` | step == full replay over random logs |
| Live content switch (F17) | Bounded live switches, or Arc generations | B `content.rs` `MAX_LIVE_RELOADS` | K: bundle switches at start only | intentional difference |

## Intentional differences

- Kabar has three UI languages; Batna is Egyptian Arabic only.
- Batna's Worker can lag the app (separate deploy), so it keeps ops for a
  table the hub lacks; Kabar holds every refusal for the learner to send
  again.
- Batna's verdicts were already fingerprint-bound per item; Kabar's lesson
  checks got the same idea per lesson (F25).
- Product content, settings, release identities and secrets are never
  shared.
