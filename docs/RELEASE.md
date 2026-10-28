# Kabar releases: alpha, then beta, then stable

Kabar is for one learner on his own devices: the Galaxy S24 Ultra, the Galaxy
Tab S11 and the Mac. A release is a signed APK sideloaded onto both Galaxy
devices and a Kabar.app built on the Mac. There are no store listings and no
paid developer accounts (decided 2026-10-26). Everything here costs $0.

**No release is cut until everything is finished** (the learner's call,
2026-10-26). This page and `just release` are prepared ahead so that day is
routine, but nothing is built for release or tagged before he says so.

## 1. Stages

| Stage | Version | When |
|---|---|---|
| **Alpha** (first) | `1.0.0-alpha.N` | Every milestone M1–M13 is done except what SPEC §23 defers. That includes all content: BIPA 1–7, J1–J6, 10k + 3k glosses. It also includes the performance pass and routes vetted by the bake-off. All §2 gates are green, and the learner says go. |
| **Beta** | `1.0.0-beta.N` | Later: after daily use of the alpha and the fixes it brings. The learner decides. |
| **Stable** | `1.0.0` | Later still. The learner decides. |

Deferred, after the core (SPEC §23): pen writing on every device
(handwritten answers, Notebook, Gambar!, Listening Companion scribbles,
write-mode lessons, aksara calligraphy), Android home-screen widgets (parked;
the draft is kept), Mac Quick Lookup and Mac-only polish. Pen pointing in
the Reader (hover, tap, lasso, highlight) is in. Studio (the mind map only)
and Pen Lab appear only on the tablet. Each stage
installs over the previous one and keeps your data.

## 2. Gates

A release is cut only when every gate in this table is green.

| Gate | Where it's checked |
|---|---|
| fmt, clippy `-D warnings`, workspace tests, content `validate`, golden sets | CI (`fmt, clippy, tests`) |
| Android arm64 compiles; a preview APK builds | CI (`Android arm64 compiles`, `Android preview APK`) |
| The Mac app compiles, its tests pass and Kabar.app bundles | CI (`Mac app compiles`) |
| core and protocol build for wasm32; Kabar Cloud end-to-end on local workerd | CI |
| Desktop QA pass at phone width and tablet width: the core loop, tone, bidi, no horizontal scroll | in the cloud, Xvfb (reports in the project's `qa/` folder) |
| `ATTRIBUTION.md` covers every dataset, corpus, font and sound | review before tagging |
| No secrets in the repo | review before tagging (`git grep` for keys) |
| Signed in on all three devices; a change appears on the others within about 5 s | **devices** (§4) |
| Lanjut resumes the exact exercise on another device | **devices** |
| Reminders stop on every device once the day's goal is met | **devices** |
| Popover under 100 ms on the S24 Ultra; smooth scrolling on the Tab S11 | **devices** |
| A shared WhatsApp screenshot reads with popovers in Arabic | **devices** |
| The cost dashboard shows $0 with paid engines off | **devices** |
| Android backup stays small and keeps progress: `adb shell bmgr backupnow app.kabar`, then `adb shell dumpsys backup` shows Kabar well under 25 MB; after a restore, progress is back and Cloud asks to sign in again (#370) | **devices** |

## 3. Versions and updates

- The version is `version` in the root `Cargo.toml`: `1.0.0-alpha.1` for the
  first alpha, then `-alpha.2`, …, `-beta.1`, …, `1.0.0`.
- Every APK is signed with your release key (`~/.kabar/kabar-release.jks`,
  password in Keychain). Android installs an update over the old app only if
  it's signed with the same key. **Back the key up** (MAC-TODO §3): losing
  it means uninstalling and starting over.
- The CI preview APK uses a throwaway preview key. Don't install it over
  the release: uninstall it first, or keep only the release build.
- From the first alpha on, the devices update themselves (§5): publishing
  a release is `just publish-update`, not a cable.
- Before installing an update, Settings → **Your data → Export everything**
  is a cheap backup. Sync also keeps everything in Kabar Cloud.

## 4. Release day on the Mac (only once the learner says go)

The order matters. Steps marked once are done a single time; later updates
are only steps 5 to 7.

1. **Once:** accounts, all free (MAC-TODO §0). Skip the Play Store and Apple
   Developer.
2. **Once:** tooling. `just setup-mac`, then `just check` (MAC-TODO §1).
3. **Once:** the release key, `just android-keystore`, and its backup
   (MAC-TODO §3). Then `just mac-codesign-cert` (Kabar.app's signing
   identity) and the update key, `just update-keygen`: commit
   `crates/app/update-key.pub` and back the secret up with the Android key
   (§5).
4. **Once:** Kabar Cloud. The Google sign-in clients, `just cloud-setup`,
   the keys, then `just deploy-cloud` (MAC-TODO §4–5). Fill `.env` from
   `.env.example`.
5. Set `version` in `Cargo.toml` (`1.0.0-alpha.1` the first time). `git pull`, then
   **`just release`**. It runs every check, then puts `Kabar.app` and
   `kabar-<version>.apk` in `dist/release/<version>/`.
6. Install. The first time by hand; every later release is
   `just publish-update "<what's new>" "<الجديد>"` and the devices update
   themselves (§5):
   - **Mac:** drag `Kabar.app` to Applications.
   - **S24 Ultra and Tab S11:** `just devices`, then
     `just install <serial>` for each. Use the APK from step 5;
     `dist/android/kabar.apk` is the same file.
7. Sign in on all three devices (Settings → Account & devices) and walk the
   **devices** gates in §2. Tell me what fails; nothing gets tagged until
   they pass.
8. Tag it, e.g. `git tag v1.0.0-alpha.1 && git push origin v1.0.0-alpha.1`. This is only a
   marker. The builds reach your devices only through the update feed
   (§5), which nobody else installs from.

## 5. Updates over the air

Every device pulls a published update by itself: no cable, no store, still
$0 (GitHub Releases of the public repository).

| | Android (S24 Ultra, Tab S11) | Mac |
|---|---|---|
| Feed | `update.json` + `update.json.sig` on the channel's release | same |
| Build | the signed APK | `Kabar.app` zipped with `ditto` |
| Install | PackageInstaller; Android checks it's the same package and signing key | unzip beside the running app, swap with renames, reopen |
| Automatic mode | downloads in the background on Wi-Fi only (mobile data and Data Saver wait; the offer still shows); installs quietly when Kabar is in the background and not mid-lesson (Android 12+, once Kabar installed itself), otherwise one tap | downloads in the background; goes in at the next launch |
| What's new | before installing (banner, notification, Settings), once after the update, and the whole changelog in Settings | same |
| "New version" notification | from the app's own checks while it's in the background, and from a JobScheduler job about every 4 hours while Kabar is closed (`UpdateCheckJob`, calling Rust's `update::background_check` over JNI); a Download or Install now button | from the app's own checks while it runs (UNUserNotificationCenter, bundled app only) |

**The "new version" notification.** One per version and step (out, then
downloaded and ready), worded in the learner's language with the first
three lines of what's new (below). Never
while Kabar is in front (its banner says it there), never inside Kak Nur's
quiet hours (the next check after they end sends it), never for a version
whose banner was closed, and not at all with Settings › Updates' switch off,
in "Only when I check" mode, or when the device's notification switch is off.
A tap opens the "what's new" sheet; the button also starts the download or
the install.

**What's new.** Every pull request the learner would notice adds
`changes/<topic>.yaml`, one line per change, in Egyptian Arabic and English
(`ar` optional), each `new`, `improved` or `fixed`:

```yaml
- kind: fixed
  ar_eg: الصوت مبقاش بيقطع لما تقفل الشاشة
  en: Audio no longer cuts out with the screen off
```

A file holding `[]` says there's nothing to tell (CI, refactors, docs).
When a feed is built (CI for `preview`, `just publish-update` for releases),
`kabar-tools update manifest --previous <current update.json> --changes`
gathers the files added since the commit the feed carries now, one pull
request at a time; a merged pull request with no file falls back to its
title (English), and titles starting with CI, docs, refactor, test and the
like are skipped. That build's entry goes on top of the feed's `history`
(40 builds, inside the signed `update.json`, so nothing extra to download or
trust), and the notes older apps read are its first three lines. CI warns
on an app pull request with no changes file; `kabar-tools update changes
[--since <commit>]` previews what the next build will say.

The app keeps every history it verifies (`updates/history.json`) and shows
it in one sheet, laid out like Morphe Manager's update dialog: the installed
build → the new one, how many builds that is and a count per kind, then a
timeline with the newest build open and a card per kind ("Show all" past
six lines). It opens from the Today banner's "What's new?", from the
notification, and from Settings › About › Updates (also "Changelog": every
build so far); downloading and installing happen in it too. At the first
launch after an update it opens once by itself, listing everything since
the build that ran before.

**Channels.** `alpha`, `beta`, `stable` for release builds (the learner
picks in Settings; a beta also reaches alpha, a release reaches everyone),
and `preview` for CI builds of `main`, which is fixed for them. Each channel
is its own GitHub release (`update-alpha`, …, and the existing `preview`).

**What a device checks, in order.** (1) The Ed25519 signature over the exact
bytes of `update.json`, with the public key built into the app; nothing in
an unsigned or changed manifest is read. (2) The format, then that the
manifest is for the channel it asked for and newer than what's installed
(version, then build number), and that file names are plain names next to
the manifest (no URLs or paths). (3) The download's size and SHA-256 against
the signed manifest, streamed while downloading and again right before
installing. (4) Android: the APK's package, signing certificate and
`versionCode` against the installed Kabar, then Android's own checks.
Mac: once Kabar.app is signed with the free "Kabar Code Signing"
certificate (`just mac-codesign-cert`, once), a new bundle must pass
`codesign --verify --deep --strict` and match the running app's designated
requirement. The same identity on every build also keeps the Keychain
trusting Kabar after an update (the Kabar Cloud sign-in).
A feed caught mid-upload (new signature, old manifest) is read once more
20 seconds later before it counts as a failure.
Anything that fails is deleted and never installed; the current Kabar stays.

**Keys.** The release update key is made once on the Mac
(`just update-keygen`): the secret goes straight into the Keychain
(`kabar-update`), the public half into `crates/app/update-key.pub`, which is
committed. Until then release builds say updates switch on after that step.
Back the secret up with the Android key: losing it means one manual install
on each device with a new key. Preview builds use a second CI key kept in the
Actions cache next to the preview APK key; if the cache expires both keys
change and a preview needs one manual reinstall anyway; Settings then says
so instead of a generic error.

**Nothing is published before the learner says go**: the preview feed
follows `main` as the preview APK always has; the release feeds don't exist
until `just publish-update` runs on release day (§4, after step 5).
