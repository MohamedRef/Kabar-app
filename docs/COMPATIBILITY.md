# Compatibility and versioning

Kabar has several things that are versioned on their own: the app, the
signed update feed, the content bundle the feed carries, the stylesheets in
that bundle, the learner's database and the sync protocol. This page states,
for each one, what a newer or older counterpart does, so a change never
strands a device that has not updated yet. The code is the source of truth;
each rule names where it lives and the test that holds it.

## The numbers

| Thing | Where | What orders it | Who checks it |
|---|---|---|---|
| App version | workspace `version` (`1.2.3-alpha.4`) | `Version` in `crates/core/src/update.rs` | the updater; Android's `versionCode` |
| App build | the CI run number for `preview`, `0` for releases | `(version, build)` | the updater, the content bundle |
| Android `versionCode` | `Version::version_code` = `scripts/android-release-apk.sh` | must grow on every install | Android itself |
| Feed format | `UPDATE_FORMAT` in `crates/protocol/src/update.rs` | integer | `verify_manifest` |
| Bundle container | `MAGIC` = `KABARCB1` in `crates/protocol/src/bundle.rs` | the magic bytes | `bundle::decode` |
| Bundle build | the feed's `(version, build)` | same as the app | `content_update`, `content::validated` |
| Stylesheet build | `ContentFile::ui_build` | exact match | `content::validated` |
| Database schema | `MIGRATIONS` in `crates/app/src/store.rs` (`user_version`) | integer | `store::migrate` |
| Sync protocol | `PROTOCOL_VERSION` in `crates/protocol/src/lib.rs` | integer | Kabar Cloud's hub |
| Content ids | `docs/CONTENT-IDS.md` | never reused | `kabar-tools ids check` in CI |

## The update feed

- **The signature comes first.** `update.json` is verified over its exact
  bytes before anything in it is read, so the rules below only ever apply
  to a feed Kabar's own key signed.
- **A newer feed format is refused politely.** `verify_manifest` reads only
  `format` first; above `UPDATE_FORMAT` the device says Kabar itself needs a
  manual update instead of failing on a field it doesn't know. Bump
  `UPDATE_FORMAT` only for a change an old app would misread, never for an
  addition.
- **New fields are additions.** Every field added after the first feed is
  `#[serde(default)]` and old apps ignore unknown fields (no
  `deny_unknown_fields` on the feed types). `history` and `content` arrived
  this way. Test: `a_manifest_without_history_still_parses`.
- **New platforms and change kinds are skipped, not fatal.** An asset whose
  `platform`, or a "what's new" line whose `kind`, this build doesn't know is
  dropped while reading; the rest of the feed still works. A damaged entry
  of a known platform still fails the feed. Tests:
  `a_feed_with_a_platform_or_change_kind_from_a_newer_build_still_parses`,
  `a_damaged_entry_of_a_known_platform_still_fails_the_feed`.
  Builds older than this rule (before the "Compatibility rules" change)
  refuse a feed with an unknown platform or kind, so do not publish one
  while a device that matters still runs such a build.
- **The channel must match.** A stable device never takes an alpha feed and
  a preview build only follows `preview`.

## The content bundle

- **Container.** The `KABARCB1` magic names the record layout. A different
  layout gets a new magic; an old app then fails `decode`, keeps what it
  has, and never activates the file.
- **Build order.** A bundle is used only while its `(version, build)` is at
  least the running app's (`content_update`, `content::validated`). An app
  update ships newer content built in, so the older bundle is dropped.
- **Keys.** A bundle may carry keys a build doesn't read yet (a newer app's
  table); they are ignored. Every key the running build needs must be
  present (`bundle_check::check_keys`). Test: `keys_from_a_newer_app_are_allowed_but_not_known`
  in `crates/core/src/bundle_check.rs`.
- **Files.** Every lesson, test, curriculum and the cast must parse with the
  running build's parsers before the bundle is taken
  (`bundle_check::check_files`). Content that needs a newer parser therefore
  never replaces readable content on an old app: the old app keeps its
  last-known-good bundle (see `docs/RELEASE.md`, "Last-known-good content"),
  remembers the digest for its own build so it doesn't download it again,
  and tries it once more after an app update.
- **Stylesheets** apply only on the exact build in `ui_build`, so a newer
  look never lands on older screens; other builds use the content and keep
  their own CSS.
- **Ids.** A lesson, exercise, test item or story id learner records point
  at is never removed, renamed or repurposed without an entry in
  `content/ids-retired.txt` (`docs/CONTENT-IDS.md`).

## The learner's data

- **Database.** `MIGRATIONS` only grows; each entry runs once and bumps
  `user_version`. A database from a newer build (a higher `user_version`) is
  refused rather than opened and damaged (`refuses_a_newer_schema`).
- **Sync.** Client and Kabar Cloud exchange `PROTOCOL_VERSION` in `Hello`;
  a mismatch is refused with a message, never half-applied. Bump it only for
  an incompatible wire change; additions use `#[serde(default)]`.
- Reviews, attempts and XP are append-only events; statuses and settings are
  last-writer-wins by HLC. Neither is rewritten by an update.

## Checklist for a change

| You are changing | Do |
|---|---|
| A field in `update.json` | add it with `#[serde(default)]`; old apps ignore it |
| A new platform or change kind | add the enum variant; check no device still runs a build older than the skip rule |
| The meaning of a feed field | bump `UPDATE_FORMAT` |
| The bundle's byte layout | new `MAGIC` |
| A content format the old parser can't read | ship it with an app build; old apps keep their last good content |
| Stylesheets | nothing: `ui_build` is set by `kabar-tools update bundle` |
| The database | append to `MIGRATIONS`, never edit an old entry |
| A sync wire type | add with `#[serde(default)]`, or bump `PROTOCOL_VERSION` |
| A content id | follow `docs/CONTENT-IDS.md` |
