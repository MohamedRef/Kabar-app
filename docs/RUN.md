# Running Kabar (M1–M2, M5, M7)

Everything below runs on the MacBook. Commands are `just` recipes from the repo root.
For the first Mac session, follow the ordered checklist in [MAC-TODO.md](MAC-TODO.md); this page has the details.

## 1. One-time setup (about 20–30 min, mostly downloads)

```bash
brew install just
just setup-mac        # Rust targets, dx 0.7.10, Java 17, Android SDK + NDK r27c
source ~/.kabar/env.sh   # or open a new terminal
just check            # fmt + clippy + tests must be green
```

8 GB RAM tip: close Chrome/Android Studio while the first Android build runs; it's the heaviest step.

## 2. Mac

```bash
just run-mac             # desktop app with hot reload
just run-phone-preview   # same app in a 412×915 window, to check the phone layout
```

To jump straight into a lesson or test, start the app with `KABAR_OPEN_LESSON`, e.g. `KABAR_OPEN_LESSON=bipa1.u01.l01 just run-mac`. Every file in `content/lessons/` (all of BIPA 1–2) and `content/tests/` is built in automatically (`crates/app/build.rs`), so `KABAR_OPEN_LESSON=bipa1.test` opens the BIPA 1 level test ("Take a flight") and `KABAR_OPEN_LESSON=placement` the placement test (also offered on Today until you have a result). Closing a lesson saves your place; tests start over when reopened.

Data lives in `~/Library/Application Support/app.Kabar.Kabar/kabar.db`. Delete it to see the first-launch screen again (or use Settings → "Show the welcome screen again").

## 3. Galaxy S24 Ultra and Tab S11

### Enable debugging (once per device)
1. Settings → About phone/tablet → Software information → tap **Build number** 7 times.
2. Settings → Developer options → turn on **USB debugging** and **Wireless debugging**.

### Connect
- **USB:** plug in, accept the "Allow USB debugging" prompt, then `just devices`.
- **Wireless** (Mac and device on the same Wi-Fi):
  1. Developer options → Wireless debugging → **Pair device with pairing code**.
  2. `just pair <ip:pair-port> <code>` (once per device).
  3. `just connect <ip:port>` using the IP and port shown on the Wireless debugging screen (port changes each time).

### Install
```bash
just android-keystore    # once: creates ~/.kabar/kabar-release.jks, password in Keychain. Back it up.
just apk                 # signed release APK → dist/android/kabar.apk
just devices             # note each serial
just install <serial>    # repeat for the S24 Ultra and the Tab S11
```

For day-to-day development use live reload instead: `just run-android <serial>`.

If Android blocks the install, allow "Install unknown apps" for the shell/USB source when prompted. Updates keep your data as long as the same keystore signs them.

## 4. Pen Lab (what to send back)

On each Galaxy device: open Kabar → Pen Lab (on the phone: Settings → Pen Lab).

1. Hover the S Pen over the white area without touching (2–3 s).
2. Write a word slowly, then fast. Press lightly, then hard.
3. Tilt the pen while writing.
4. Hold the S Pen button while drawing.
5. Rest your palm on the screen while writing.
6. If your pen has an eraser end or eraser mode, try it.
7. Tap **Save report**, then **Copy JSON** and paste it to me (one report per device; DeX mode on the tablet is a bonus third one).

Also tell me how the ink *feels*: does it trail behind the pen tip, and by roughly how much?

## 5. Kabar Cloud (M2)

Full guide: `docs/CLOUD.md`. Short version, on the Mac:

```bash
just cloud-e2e       # offline test: fake Google, phone + tablet, live sync, sign-out
just cloud-tools     # once: wrangler + worker-build
wrangler login       # once: your free Cloudflare account
just cloud-setup     # once: D1, signing key, allowlist
just deploy-cloud    # after adding GOOGLE_CLIENT_IDS (CLOUD.md §3.2)
# Reminders (M7): Firebase service account → `wrangler secret put FCM_SERVICE_ACCOUNT` (CLOUD.md §3.4)
```

Then connect the app (CLOUD.md §3.6):

```bash
cp .env.example .env   # put in the workers.dev URL and the Desktop client id + secret
just run-mac           # Settings → Account & devices → Sign in with Google
just apk && just install <serial>   # same on the S24 Ultra and the Tab S11
```

Check it: save a word in **Try sync** on the phone; it appears on the tablet within seconds. The Devices list shows every signed-in device and can sign one out.

Once signed in, AI features (Teach Kabar, and later the persona chats) run through Kabar Cloud's AI proxy. Lesson and review audio switches to Azure's neural voices after `wrangler secret put AZURE_SPEECH_KEY` (CLOUD.md §3.3). Each line is fetched once and then kept on the device, so replays work offline. Without the key, or when the monthly free characters run out, the device's own voice reads the line.

## 6. Content pipeline (vocabulary, Arabic Bridge)

Details in `docs/CONTENT.md`. On the Mac:

```bash
just content-fetch           # frequency sources (~60 MB) into data/raw/
just content-freq            # rebuilds content/freq/id.tsv (should produce no diff)
just content-fetch nusa      # Javanese corpora from GitHub (~5 MB)
just content-fetch kaikki    # Wiktionary extracts from kaikki.org (~100 MB)
just content-kaikki          # import + Arabic Bridge cross-check report
just content-fetch jvwiki    # Javanese Wikipedia dump (~70 MB)
just content-freq-jv         # content/freq/jv.tsv (NusaWrites + NusaX + jvwiki)
just content-check           # validate everything
just content-review 1        # review sheet for gloss batch 1 → build/review/
```

Commit the changed files under `content/` after the kaikki and jvwiki steps.

## 7. Kak Nur coach (M7)

On the Mac: `just run-mac`. After the welcome screen, Kak Nur's onboarding asks your name, why you're learning, Indonesian/Javanese, 5/15/30 minutes, a target, study times, reminders and where to start.

- **Today**: Kak Nur's greeting, "5, 15 or 30 minutes today?", the plan, Pelita and Koin. Tick an item when you've done it (until reviews and lessons land, ticking is how study gets logged). The recap card appears once the goal is met or in the evening.
- **Kak Nur** (nav): weekly report, Liga Pribadi, achievements, the Warung and a link to the learning journal.
- **Journal** (nav on the Mac and tablet; on the phone from Today's links under the plan, or from Kak Nur): every day's reviews, new words, lessons and level tests, practice, plan items, purchases, souvenirs and taught words, newest first, with the day's minutes and XP. Filter by All / Study / Words / Reflections; "Show older days" pages back two weeks at a time. Write a reflection on any day (typed, several lines); edit or delete it. Reflections sync between devices and are never sent to any AI. Handwritten reflections come with Tablet Studio.
- **Settings → Kak Nur**: reminder style, study times, quiet hours, report day.

To start onboarding again, delete the database (§2). On the Galaxy devices: `just apk` and `just install <serial>` as in §3.

Reminders arrive as push notifications once Kabar Cloud's reminder alarm and Firebase are set up (CLOUD.md §3.4).

### Settings: Cost & limits, and Export everything

- **Settings → Cost & limits** shows "$0.00 this month", one bar per free allowance (Gemini, Workers AI, Azure TTS) with its local reset time, a warning from 80% ("…Kabar pauses it rather than pay; it's back tomorrow at 02:00"), and Cloudflare's Free-plan limits (no card, so it can't bill). The counters come from `GET /v1/usage` (CLOUD.md §2); signed out or offline, the screen still says $0 and explains the hard stops. **Turn on Claude…** asks first ("This could cost money", default No). There is no in-app engine switch yet: Claude is switched on in Kabar Cloud with `CLAUDE_ENABLED = "true"` in `crates/server/wrangler.toml` and `just deploy-cloud`.
- **Settings → Your data → Export everything** writes every table of the local database to one `kabar-export-YYYY-MM-DD.json` (`-2`, `-3`… if one exists). Sign-in tokens live in the Keychain/Keystore and are never in it.
  - Mac: `~/Downloads`.
  - S24 Ultra / Tab S11: `Android/data/app.kabar/files/` (the app's external files folder). Copy it over USB (it shows in the phone's folder on the Mac with Android File Transfer/OpenMTP) or `adb pull /sdcard/Android/data/app.kabar/files/kabar-export-2026-10-26.json`. A share-sheet button needs a FileProvider in `android/` and is a follow-up.

## 8. Teach Kabar (spec §26)

Open **Teach Kabar** (علّم كبر) from the side bar on the Mac and tablet, or from Settings on the phone. Type the word, optionally the sentence you met it in, then paste what you found (or a link, or use Wiktionary) and tap **Teach Kabar**. The draft appears below for review: approve it, edit it, ask for another version, or discard it. Approved words show in **My dictionary** and in Reader popovers with a "taught by you" badge.

Writing the draft needs an AI engine, so until the app is connected to Kabar Cloud the screen says "not connected yet". Search Google opens the browser; nothing from Google is read automatically.

## 9. Reader, Share-to-Kabar and the Screenshot Decoder (M5)

### Reader on the Mac
`just run-mac` → **Reader** (القارئ). Paste the §24 chat lines and read: hover a word to peek, click for the full card, drag-select a phrase, right-click → Save. Try split mode (جنب بعض) and the three explanation voices. Words come from the content pack (2,000 Indonesian and 1,000 Javanese glosses, 581 Arabic Bridge cards, the Javanese speech-level table), all marked "not reviewed yet". A Javanese word's card shows its Indonesian equivalent and every speech level (try `Monggo dhahar, Bu.`).

**Teach Kabar from the Reader:** open a word's card → **Teach Kabar** (it's the main button when Kabar has no gloss). The Teach screen opens with the word and its sentence filled in; go back to the Reader and the text is still open. Approved words show first in the card with a "taught by you" badge; a draft shows a "review it" chip instead.

### How the Kotlin glue gets into the APK
`android/` (repo root) is a small Gradle library module: the share target (`ShareActivity`), the pending-share hand-off (`PendingShare`) and on-device OCR with ML Kit Text Recognition v2 (`OcrBridge`). Nothing to configure: the `#[manganis::ffi("../../android")]` marker in `crates/app/src/platform/android.rs` makes `dx` (0.7.10) copy the folder into its generated Gradle project as `:plugins:plugin`, add it as an app dependency, and let the manifest merger add the share activity. Gradle downloads ML Kit from Google's Maven repository (free) on the first Android build after this change, so be online for that one.

`just apk` now stops with "dx did not build android/ into the APK" if the Kotlin classes are missing.

The bundled Latin OCR model makes the APK a few MB bigger. OCR runs on the phone; images never leave it.

### Try it on the S24 Ultra / Tab S11
```bash
just apk && just install <serial>          # same as before
just share-text "arp dicul piro" <serial>  # simulates sharing text to Kabar
```
Then on the device:
1. **Text:** in WhatsApp/Chrome, long-press a message or select text → Share → **Kabar**. Kabar opens with it in the Reader.
2. **Selection menu:** select text in any app → ⋮ (more) → **Kabar**.
3. **Screenshot:** take a screenshot → Share → **Kabar**. Kabar opens and reads the text in it (ML Kit, on-device).
4. Share again while Kabar is already open: it comes to the front with the new item.

`adb logcat -s KabarShare` shows why a share was ignored (e.g. an unreadable image).

### Mac
The Screenshot Decoder uses Apple Vision (built into macOS, nothing to install). It recognizes Indonesian when your macOS supports it and falls back to its general Latin model otherwise; language auto-correction is only on when Indonesian is supported, so Javanese and slang aren't "corrected" into English. Try it with `just run-mac` Reader → "Open a screenshot", or drag an image onto the Reader.

Tesseract (`ind`/`jav`) as a third OCR engine is phased for later.
## 10. Play: Arabic Bridge games, Kabar Pagi, Rupiah Reader, Pasar! (M10)

Open **Play** (ألعاب) in the sidebar or the phone's bottom bar.

- **Kamu sudah tahu N kata**: the Arabic loans you already understand. Tap a card to flip it; "I know it" adds the word to your log.
- **Sound-Shift Detective**: pick a sound change (ف → p, خ → k, ة → -at …) or all, then 10 rounds.
- **False Friends Alarm**: words that look Arabic but mean something else (kalimat, mungkin).
- **Rupiah Reader**: type how a price is read (Rp26.000.000 → *dua puluh enam juta rupiah*). Speed counts, accuracy more.
- **Pasar!**: haggle within a budget. The iPhone listing is the bonus round; Javanese sellers are marked unverified.
- **Kabar Pagi**: the morning story from your due words plus a few new ones. On Today it appears every morning until you've read it. Audio uses the Mac's Indonesian voice (System Settings → Accessibility → Spoken Content → System voice → Manage Voices → Indonesian "Damayanti", if it isn't installed). Android's WebView has no voices, so on the Galaxy devices audio arrives with Kabar Cloud's TTS (follow-up).

### Javanese manners and sounds (M11)

- **Unggah-Ungguh**: talk with Bayu (a friend, J1), Bu Darmi at the market (J2), and Mbah, a teacher and an elder neighbour (J3). Each turn gives the same thing said at different speech levels. The Respect Meter rises when the choice fits the person and dips when it doesn't, and Kak Nur always explains why. Scenes above your Javanese target level show which level opens them. The lesson j3.u01.l03 ends with a button that opens this game.
- **d/dh · t/th**: tell Javanese *wedi* (afraid) from *wedhi* (sand), and Indonesian *dada* from Javanese *dhadha*. **Listen** mode appears only when the device has a Javanese voice; Indonesian voices can't make the curled sound, so Kabar never uses one for this. Without a Javanese voice you get **Read** mode, where you see the meaning and pick the spelling. Javanese audio from Kabar Cloud's TTS (jv-ID voices) is a follow-up.
- All Javanese here is marked unverified until you approve it (`content/games/unggah-ungguh.toml`, `content/games/minimal-pairs.toml`).

## 11. Troubleshooting

- `dx` can't find the NDK: `echo $ANDROID_NDK_HOME` should print the NDK path; re-run `source ~/.kabar/env.sh`.
- `just logs <serial>` streams the app's log while it's running.
- Build out of memory on 8 GB: `CARGO_BUILD_JOBS=2 just apk`.
- `dx did not build android/ into the APK`: make sure `dx --version` says 0.7.10 (it must match the `manganis` version in `crates/app/Cargo.toml`), then check that `target/dx/kabar/release/android/app/settings.gradle` contains `include ':plugins:plugin'`.
- Gradle can't resolve `com.google.mlkit:text-recognition`: the first build needs internet (Google Maven).
