# Kabar 1.0 release

Kabar is for one learner on his own devices: the Galaxy S24 Ultra, the Galaxy
Tab S11 and the Mac. A release is a signed APK sideloaded onto both Galaxy
devices and a Kabar.app built on the Mac. There are no store listings and no
paid developer accounts (decided 2026-10-26). Everything here costs $0.

## 1. What 1.0 is

**In 1.0:** everything merged on `main` when the release is cut:

- Onboarding, Kak Nur (daily plan, recap, weekly report, reminders,
  rewards, journal) and the cost dashboard.
- Journey Map, Lesson Player, placement and the level tests.
- BIPA 1, 2 and 5 lessons, and Javanese J1–J3.
- FSRS review with classic cards and 1-minute bursts, plus stats, the
  monthly level check and the skill radar.
- The Reader, the Screenshot Decoder, Share-to-Kabar, Explain this
  sentence and Teach Kabar.
- Play: the Arabic Bridge games, Kabar Pagi, Rupiah Reader, Pasar!,
  Unggah-Ungguh and Minimal Pairs.
- Watch & listen: Sinetron and Kabar Radio, with offline packs.
- Kabar Cloud sync across the three devices, the AI router and the themes.

**In 1.x updates** (phased, not dropped; installs over 1.0 and keeps your
data):

- BIPA 3, 4, 6 and 7 lessons.
- J4–J6 lessons (their grammar is already in).
- Android home-screen widgets.
- A performance pass.
- Routes vetted by the bake-off.

**After the core** (SPEC §23 "Deferred"): all pen work on the phone and the
tablet, aksara calligraphy, Mac Quick Lookup and Mac-only polish. Studio
and Pen Lab stay in the build, but appear only on the tablet.

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

## 3. Versions and updates

- The version is `version` in the root `Cargo.toml`. It is `1.0.0` for the
  release, and later updates are `1.x.y`.
- Every APK is signed with your release key (`~/.kabar/kabar-release.jks`,
  password in Keychain). Android installs an update over the old app only if
  it's signed with the same key. **Back the key up** (MAC-TODO §3): losing
  it means uninstalling and starting over.
- The CI preview APK uses a throwaway preview key. Don't install it over
  the release: uninstall it first, or keep only the release build.
- Before installing an update, Settings → **Your data → Export everything**
  is a cheap backup. Sync also keeps everything in Kabar Cloud.

## 4. Release day on the Mac

The order matters. Steps marked once are done a single time; later updates
are only steps 5 to 7.

1. **Once:** accounts, all free (MAC-TODO §0). Skip the Play Store and Apple
   Developer.
2. **Once:** tooling. `just setup-mac`, then `just check` (MAC-TODO §1).
3. **Once:** the release key, `just android-keystore`, and its backup
   (MAC-TODO §3).
4. **Once:** Kabar Cloud. The Google sign-in clients, `just cloud-setup`,
   the keys, then `just deploy-cloud` (MAC-TODO §4–5). Fill `.env` from
   `.env.example`.
5. Set `version` in `Cargo.toml` (1.0.0 the first time). `git pull`, then
   **`just release`**. It runs every check, then puts `Kabar.app` and
   `kabar-<version>.apk` in `dist/release/<version>/`.
6. Install:
   - **Mac:** drag `Kabar.app` to Applications.
   - **S24 Ultra and Tab S11:** `just devices`, then
     `just install <serial>` for each. Use the APK from step 5;
     `dist/android/kabar.apk` is the same file.
7. Sign in on all three devices (Settings → Account & devices) and walk the
   **devices** gates in §2. Tell me what fails; nothing gets tagged until
   they pass.
8. Tag it: `git tag v1.0.0 && git push origin v1.0.0`. This is only a
   marker; the builds stay on your devices and nothing gets published.
