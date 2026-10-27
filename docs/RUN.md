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

To jump straight into a lesson or test, start the app with `KABAR_OPEN_LESSON`, e.g. `KABAR_OPEN_LESSON=bipa1.u01.l01 just run-mac`. Every file in `content/lessons/` (BIPA 1–2 and J1–J3, each lesson 7 minutes or less; see `docs/PEDAGOGY.md`) and `content/tests/` is built in automatically (`crates/app/build.rs`), so `KABAR_OPEN_LESSON=bipa1.test` opens the BIPA 1 level test ("Take a flight") and `KABAR_OPEN_LESSON=placement` the placement test (also offered on Today until you have a result). Closing a lesson saves your place; tests start over when reopened.

Lessons start marked "not reviewed yet". Finishing one with 80% or more right on the first try (with at least half the exercises answered, not skipped) counts as your approval: the badge comes off that lesson and its words in Review, and off a unit's souvenir and a level's title once all their lessons are passed. This works the same for Javanese, on every device after sync.

Data lives in `~/Library/Application Support/app.Kabar.Kabar/kabar.db`. Delete it to see the first-launch screen again (or use Settings → About Kabar → "Show the welcome screen again").

**Settings** opens as a sheet over whatever you're doing, so a lesson or Reader page stays where it was:

| Where | How to open it |
|---|---|
| Mac | **Kabar › Settings…** in the menu bar, or **⌘,**. The menu bar also has Edit (⌘C/⌘V), **Go** (⌘1 Today … ⌘7 Chat), Window and Help › Keyboard Shortcuts. |
| Phone | the **gear** at the top of every screen (or Settings in Today's links) |
| Tablet and wide windows | **Settings**, pinned to the foot of the side bar |

Sections: Learning (pronunciation, translation under sentences, sounds), Look & language (language, theme, text size, digits), Kak Nur & reminders, Android (notifications for the lock-screen player, background audio, S Pen hover, App info), Tablet (slim side bar, layout), Mac (shortcuts, mouse hover peek, slim side bar, Quick Lookup coming later), Offline & storage (packs, export, data folder), Account & sync (Kabar Cloud, devices, Cost & limits) and About. Android, Tablet and Mac only appear on those devices. Choices marked "This device only" (text size, hover peek, side bar) stay on that device; the rest sync. Escape closes the sheet. `KABAR_OPEN_SETTINGS=<section>` (`learning`, `look`, `coach`, `mac`, `storage`, `account`, `about`…) opens it at launch for previews.

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
just run-mac           # Settings → Account & sync → Sign in with Google
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
- **Settings → Kak Nur & reminders**: daily size, reminder style, study times, quiet hours, report day.

To start onboarding again, delete the database (§2). On the Galaxy devices: `just apk` and `just install <serial>` as in §3.

Reminders arrive as push notifications once Kabar Cloud's reminder alarm and Firebase are set up (CLOUD.md §3.4).

### Settings: Cost & limits, and Export everything

- **Settings → Account & sync → Cost & limits** shows "$0.00 this month", one bar per free allowance (Gemini, Workers AI, Azure TTS) with its local reset time, a warning from 80% ("…Kabar pauses it rather than pay; it's back tomorrow at 02:00"), and Cloudflare's Free-plan limits (no card, so it can't bill). The counters come from `GET /v1/usage` (CLOUD.md §2); signed out or offline, the screen still says $0 and explains the hard stops. **Turn on Claude…** asks first ("This could cost money", default No). There is no in-app engine switch yet: Claude is switched on in Kabar Cloud with `CLAUDE_ENABLED = "true"` in `crates/server/wrangler.toml` and `just deploy-cloud`.
- **Settings → Offline & storage → Export everything** writes every table of the local database to one `kabar-export-YYYY-MM-DD.json` (`-2`, `-3`… if one exists). Sign-in tokens live in the Keychain/Keystore and are never in it.
  - Mac: `~/Downloads`.
  - S24 Ultra / Tab S11: `Android/data/app.kabar/files/` (the app's external files folder). Copy it over USB (it shows in the phone's folder on the Mac with Android File Transfer/OpenMTP) or `adb pull /sdcard/Android/data/app.kabar/files/kabar-export-2026-10-26.json`. A share-sheet button needs a FileProvider in `android/` and is a follow-up.

## 8. Teach Kabar (spec §26)

Open **Teach Kabar** (علّم كبر) from the side bar on the Mac and tablet, or from the links at the foot of Today on the phone. Type the word, optionally the sentence you met it in, then paste what you found (or a link, or use Wiktionary) and tap **Teach Kabar**. The draft appears below for review: approve it, edit it, ask for another version, or discard it. Approved words show in **My dictionary** and in Reader popovers with a "taught by you" badge.

Writing the draft needs an AI engine, so until the app is connected to Kabar Cloud the screen says "not connected yet". Search Google opens the browser; nothing from Google is read automatically.

## 9. Reader, Share-to-Kabar and the Screenshot Decoder (M5)

### Reader on the Mac
`just run-mac` → **Reader** (القارئ). Paste the §24 chat lines and read: hover a word to peek, click for the full card, drag-select a phrase, right-click → Save. Try split mode (جنب بعض) and the three explanation voices. Words come from the content pack (2,000 Indonesian and 1,000 Javanese glosses, 581 Arabic Bridge cards, the Javanese speech-level table), all marked "not reviewed yet". A Javanese word's card shows its Indonesian equivalent and every speech level (try `Monggo dhahar, Bu.`).

**Teach Kabar from the Reader:** open a word's card → **Teach Kabar** (it's the main button when Kabar has no gloss). The Teach screen opens with the word and its sentence filled in; go back to the Reader and the text is still open. Approved words show first in the card with a "taught by you" badge; a draft shows a "review it" chip instead.

**Explain this sentence:** open a word's card → **💡 Explain the sentence**, or select a phrase → **Explain this sentence**. The sentence is outlined in the text and the panel lists the grammar points from `content/grammar` that fit it (title, summary, pattern, an example, a common mistake, the Arabic bridge), found on the device with no network. When Kabar Cloud is linked, an AI explanation (meaning in your voice, what the grammar cards don't cover, register, culture) appears on top. Reader texts can be private chats, so this request never goes to Gemini's free tier. Javanese answers stay marked "not reviewed yet". Try `Saya sedang membaca buku.` and `Rumahnya sudah dibersihkan kemarin.`

**Import a web page:** paste just a link (`https://…`) into the Reader and press Read, or share a link to Kabar on the phone. Kabar fetches the page itself (no browser restrictions), keeps the article text, and saves it with a 🔗 icon. If a page can't be read (login wall, PDF, offline), Kabar asks you to paste the text instead.

### How the Kotlin glue gets into the APK
`android/` (repo root) is a small Gradle library module: the share target (`ShareActivity`), the pending-share hand-off (`PendingShare`) and on-device OCR with ML Kit Text Recognition v2 (`OcrBridge`). Nothing to configure: the `#[manganis::ffi("../../android")]` marker in `crates/app/src/platform/android.rs` makes `dx` (0.7.10) copy the folder into its generated Gradle project as `:plugins:plugin`, add it as an app dependency, and let the manifest merger add the share activity. Gradle downloads ML Kit from Google's Maven repository (free) on the first Android build after this change, so be online for that one.

`just apk` now stops with "dx did not build android/ into the APK" if the Kotlin classes are missing.

### App icon on Android and the Mac
The phone and tablet show Kabar's own launcher icon (adaptive, with a themed-icon layer for One UI). dx always writes its default Dioxus icon into the app it generates, so Kabar ships its icon from the `android/` module under the name `ic_kabar_launcher` and uses its own manifest, `crates/app/AndroidManifest.xml` (`[application] android_manifest` in `crates/app/Dioxus.toml`). That file is dx's template with Kabar's icon and the text-to-speech `<queries>`: **add any new Android permission or intent there**, Dioxus.toml no longer feeds the manifest. `just apk` (and CI's preview build) stops if the APK's icon is not Kabar's, as long as the Android build-tools' `aapt2` is on your PATH.

`just apk` builds a real release: dx 0.7.10 only runs gradle's release build when the keystore passwords sit in `Dioxus.toml`, which would put a secret in the repo, so `scripts/android-release-apk.sh` re-runs `assembleRelease` on the gradle project dx generated. The APK is not debuggable, is shrunk by R8, and takes its `versionCode` from the workspace version (`1.0.0-alpha.1` → 10000101, so every alpha installs over the one before). The check stops if an APK is debuggable or still has versionCode 1.

After changing the icon masters in `assets/src/icon/`, run `python3 scripts/assets/export_icons.py`; it refreshes `assets/export/` and the copies in `android/src/main/res/`. The Mac `.app` icon is built by `dx bundle` from the 1024 px PNG in `assets/export/macos/Kabar.iconset/`.

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
5. **Open a screenshot inside Kabar:** Reader → **Open a screenshot** (افتح سكرين شوت) opens the system photo picker (no permission needed); pick a screenshot and Kabar reads it the same way as a shared one. Dragging an image onto the Reader works on the Mac only.

`adb logcat -s KabarShare KabarPick` shows why a share or a picked image was ignored (e.g. an unreadable image).

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

## 11. Tablet Studio (M9)

Open **Studio** (الاستوديو) in the sidebar, or from Settings on the phone. Six tabs, all on the same ink engine:

- **Notebook** (الكشكول): notebooks of infinite pages. The pen writes; two fingers (or the mouse wheel) pan and ctrl+wheel or a pinch zooms; the eraser tool rubs out whole strokes. **Add a word card** drops a live card on the page: it shows the gloss in your explanation voice and speaks the word. Every line you write is listed under **Your lines** — tap ✎ to scroll to it, or type what you wrote so search finds it. The search box searches handwriting, cards and map nodes across every notebook.
- **Gambar!**: Kabar names a word (drawable ones you already know come first), you sketch it in 60 seconds, and Kabar guesses. Each round logs practice to Kak Nur.
- **Mind map** (خريطة كلمات): type a topic (`di pasar`), pick `id` or `jv`, and Kabar lays out 3–6 branches you then extend with the pen. Javanese maps carry an "unverified" badge until you approve them.
- **Listening Companion** (رفيق السماع): meant for split screen next to YouTube. Scribble a word the way it sounded, or type it, and Kabar suggests real words by sound (old spellings `oe/dj/tj`, `ng/ny`, `f→p`, final `k`), each with its gloss, audio, **Add to review**, and **Teach Kabar** when nothing fits.
- **Dictation** (إملا): Kabar says a word, you write it with the pen, and the feedback circles the wrong letters, marks missing ones and strikes extra ones. The same pad goes into write-mode lessons in the Lesson Player.
- **Aksara Jawa** (أكشارا جاوة, M13): pick a letter, a vowel or final sign on ka, or a word. **Watch** draws the strokes in order over a faint letter (Replay repeats it), **Trace** puts the faint letter and numbered start points under the sheet, **Write** leaves only the baseline. The brush thickens with S Pen pressure (pen and eraser are there too). **Check shape** is offline and instant: it says which numbered stroke is missing and where, extra ink, a mirrored letter, or "this reads as ga, not ka". **Ask the teacher** sends the sheet to the `check_strokes` vision task when you're signed in to Kabar Cloud. The stroke order is a draft traced from Noto Sans Javanese and shows "not reviewed yet" until you (or a teacher) approve it; review it with `python3 scripts/aksara/preview.py content/fonts/NotoSansJavanese-Regular.ttf content/aksara/strokes.json out.png`.

**Reading handwriting, sketches and aksara** goes through Kabar Cloud's image upload, so it needs you signed in to Kabar Cloud (Settings). Signed out or offline, **Read my writing** and **Ask the teacher** say why and every screen falls back: type the line yourself in the Notebook and the pad, judge your own sketch in Gambar!, and use the offline shape check for aksara. Everything else (ink, pan/zoom, cards, maps, search, sync) works offline.

Ink syncs like every other row (`notebooks`, `ink_documents`, `ink_strokes`, `notebook_items`, `ink_transcripts`), so a page you write on the tablet shows up on the phone. The three new tables need a Kabar Cloud redeploy: `just deploy-cloud`.

## 12. Stats and the monthly level check (spec §16, §6)

- **Stats**: Review → "All stats" (under the forecast), or Today → "All stats". Study heatmap (20 weeks), time studied, known words per language, true retention (mature/young, 7 and 30 days) and the 30-day review forecast. Hover a cell or bar on the Mac for its numbers; every chart also has a sentence under it.
- **Monthly level check**: offered on Today once a month (the first one about 4 weeks after you start); "Later" hides it for 3 days on that device. You can also take it any time from Stats. It's ~10 minutes of your current level's test items, then the skill radar vs last month, Kak Nur's decision and "BIPA N around <date>". Fully offline. Levels without test items yet (BIPA 3+, Javanese) get a history-only check.
- Preview on the Mac: `KABAR_OPEN_SCREEN=stats just run-mac` or `KABAR_OPEN_SCREEN=level_check just run-mac`.
- Demo data for screenshots (a throwaway database, never your real one): `KABAR_DEMO_DB=/tmp/kabar-demo.db cargo test -p kabar-app seed_stats_demo -- --ignored`, then run the app with its data dir pointing there (see `crates/app/src/stats/demo.rs`).

## 13. Offline packs (audio per unit)

Each unit has a pack of pre-generated Azure neural audio (Gadis/Ardi for Indonesian, Siti/Dimas for Javanese; dialogue lines in the speaker's voice) so its lessons sound natural with no network. Manifests are committed under `content/packs/`; the audio lives on the `packs` GitHub release of this repository (free), never in git.

**Budget.** BIPA 1–2 and J1–J3 need about 83,000 characters in total (BIPA 1 ≈ 24.6k, BIPA 2 ≈ 29.2k, J1–J3 ≈ 29k), far under Azure F0's 500,000 a month. The tool refuses to spend more than `--budget` per calendar month (default 300,000, leaving the rest for Kabar Cloud's TTS proxy) and keeps its own count in `~/.kabar/audio/usage.json`. F0 never bills: past the quota Azure just says no until the next month.

On the Mac, with `AZURE_SPEECH_KEY` and `AZURE_SPEECH_REGION` in `.env` (git-ignored; same F0 resource as Kabar Cloud):

```bash
just packs                     # after lesson edits: refresh content/packs/ (`just content-check` fails until you do)
just packs-audio-plan          # what it would spend this month, without calling Azure
just packs-audio bipa1.u01     # try one unit first and listen to a few clips in ~/.kabar/audio/
just packs-audio               # the rest (resumable; F0 allows 20 requests a minute, so ~3,500 clips take ~3 h)
just packs-bundle              # dist/packs/<unit>.tar + index.json (only fully synthesized units)
gh auth login                  # once
just packs-upload              # uploads to the `packs` release
git add content/packs && git commit -m "Packs: audio hashes" && git push
```

Commit `content/packs/` after bundling: the app build embeds `content/packs/index.json`, so it lists every unit offline. Javanese text is still unverified; if a reviewed lesson changes, `just packs` marks only the changed lines as missing and `just packs-audio` regenerates just those.

**In the app:** Settings → **Offline packs** lists every unit with its size, Download / Update / Remove, total storage, and "Download the current and next unit automatically" (on by default: opening a lesson fetches its unit and the next one in the background). The Lesson Player plays a downloaded clip first, then the device's own voice (Android TextToSpeech, or the WebView voice on the Mac), exactly as before. A run-time `KABAR_PACKS_URL` points the app at another release; an empty one turns downloads off.

## 14. Watch & listen and persona chats (M12)

Open **Watch & listen** (اتفرج واسمع): in the sidebar on the Mac and the Tab S11, and from the links under Today on the S24 Ultra.

- **Sinetron**: Pak Budi in Madiun and his daughter Sari in Cairo. Episode N opens when its Journey unit is finished (levels before the one your placement test started you at count as finished). Tap **Play** and the episode plays line by line, with the transcript following along. Tap any line to hear it again, tap **…** to see one line's meaning, or use **Translations** to show them all. **Note** chips explain culture and grammar. **A bit slower** plays at 80 %. After the last line, **On to the questions** runs the episode's questions in the Lesson Player, and finishing them marks the episode watched on every device.
- **Kabar Radio**: one 3–5 minute show per unit with Gadis and Ardi (intro, dialogue, word spotlight, recap, outro). Listening to the end marks it listened and counts toward today's minutes.
- **Voices** are Azure's neural voices through Kabar Cloud (sign in under Account & devices). The four voices are shared, so pitch tells the Sinetron characters apart. Without Kabar Cloud, Indonesian lines are read by the device's voice (the Mac's "Damayanti", see section 10).
- **Offline**: **Download for offline** on an episode fetches every line and stores the episode as a single MP3 track. It then plays with no network, for example in airplane mode on the S24 Ultra. **Free up space** at the bottom of the list removes all downloads. An episode takes about 2 MB and about 3,000 of Azure's 500,000 free characters a month.
- To try it offline: open Kabar Radio → *Apa kabar, Kota Tua?* → **Download for offline**, wait for ✓, turn on airplane mode, then **Play**.
- **Chat** (دردش): sidebar on the Mac and the Tab, and the links under Today on the phone. Pick Mas Dimas or Bu Siti (Javanese), or Mbak Gadis or Mas Ardi (Indonesian), then a scene. The chat is pitched at your Journey level (Javanese personas speak Indonesian with a Javanese word a turn until you start the Javanese journey).
  - Type a reply (Enter sends, Shift+Enter is a new line), or **✍ Handwrite** with the pen on the Tab.
  - Mistakes show under your message with the more natural form and a one-line reason. **✓ Spot on** means there was nothing to fix.
  - Tap any word in a reply (or hover it with the S Pen) for the Reader's word card. 🔊 plays the reply in the persona's own voice.
  - **Talk to me in** switches ngoko/krama (Javanese) or casual/standard (Indonesian).
  - **Replay as dialogue** plays the whole chat with two voices, your lines as corrected.
  - AI-written Javanese shows **Not checked yet**. Chats sync to your other devices. Replies need Kabar Cloud (Workers AI first). What you type is sent as learning text, with e-mail addresses and phone numbers removed first.
- **Screen off** (S24 Ultra, Tab S11): a *downloaded* episode plays in Android's media player, so it keeps going with the screen off or in another app, with play/pause on the lock screen and in the notification shade. Leaving the episode stops it. Episodes that aren't downloaded play inside the app and pause when the screen goes off. Daily Radio episodes "from my words" are waiting on the `radio_script` bake-off on the Mac.

## 15. Art and sounds (to do on the Mac)

The art and the lesson sounds are drawn and synthesized in code, so nothing needs downloading for them. Re-run these after editing a scene or a sound:

```sh
pip3 install cairosvg Pillow numpy PyYAML      # once; ffmpeg comes from `brew install ffmpeg`
python3 scripts/assets/draw_souvenirs.py bipa3.u06   # one card (or no argument for all)
python3 scripts/assets/export_souvenirs.py
python3 scripts/assets/draw_misc.py                  # hero, empty states, gong, notebook paper
python3 scripts/assets/synth_sounds.py               # gong and kendang, 3 variations each
```

**Kenney UI clicks.** kenney.nl is blocked from the cloud container, so this one is a Mac step:
1. Open <https://kenney.nl/assets/interface-sounds> and download the pack. It is free, and the license is CC0.
2. Unzip it and pick 2 or 3 soft sounds: a click, a toggle and a page turn.
3. Copy them, as `.ogg`, to `assets/sounds/ui/`, together with the pack's `License.txt`.
4. Tell Claude which files you picked. Claude then wires them in (buttons and page turns, off with the sounds switch) and adds the row to `ATTRIBUTION.md`.

## 16. Troubleshooting

- `dx` can't find the NDK: `echo $ANDROID_NDK_HOME` should print the NDK path; re-run `source ~/.kabar/env.sh`.
- `just logs <serial>` streams the app's log while it's running.
- Build out of memory on 8 GB: `CARGO_BUILD_JOBS=2 just apk`.
- `dx did not build android/ into the APK`: make sure `dx --version` says 0.7.10 (it must match the `manganis` version in `crates/app/Cargo.toml`), then check that `target/dx/kabar/release/android/app/settings.gradle` contains `include ':plugins:plugin'`.
- Gradle can't resolve `com.google.mlkit:text-recognition`: the first build needs internet (Google Maven).
