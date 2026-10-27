# Mac session checklist

Everything that can't be done from the cloud sandbox, in the order that gets
through it in one sitting on the MacBook (8 GB). Work top to bottom. Nothing
here costs money. Details live in the linked docs; this page is the order.

Other threads add their Mac-only steps here by PR, into the section they
belong to. Keep one checkbox per action.

**Time:** about 3 hours, much of it downloads and builds you can leave running.

The core is lessons, reviews, the Reader, the coach and content. Pen writing
(phone and tablet) and Mac extras are deferred (§9), so their steps are no
longer here. Pen pointing in the Reader is core and has one check in §3c.

**Release:** this page sets the Mac and devices up once. Cutting a release
(`just release`, install, the device gates) is [RELEASE.md §4](RELEASE.md).

---

## 0. Before you sit down (browser only, any device, ~30 min)

- [ ] **Claude Max credit** (while you're on Max 20x): Claude → Settings → Billing, claim the monthly API credit and link a Console org. It expires each cycle, so it goes to build-time content batches.
- [x] **Cloudflare**: free account at <https://dash.cloudflare.com/sign-up> (no card). Done.
- [ ] **Google AI Studio**: <https://aistudio.google.com> → Get API key (free tier). Keep it for step 5.4.
- [ ] **Azure**: account plus a Speech resource on **F0** (signup asks for a card for identity only; F0 never bills). Note the key and region for step 5.4.
- [ ] **Google Cloud**: create the project "Kabar" and set the consent screen to External / Testing with your Gmail as a test user ([CLOUD.md §3.2](CLOUD.md) steps 1–2). The OAuth clients come in step 4, because the Android one needs the release key.
- [ ] **Firebase**: project on the Spark plan, linked to the same Google Cloud project (used for reminders in M7).

## 1. Tooling (~30 min, mostly downloads)

```bash
git clone https://github.com/MohamedRef/Kabar-app.git && cd Kabar-app
brew install just
just setup-mac            # Rust targets, dx 0.7.10, Java 17, Android SDK + NDK r27c
source ~/.kabar/env.sh    # or open a new terminal
just check                # fmt + clippy + tests: all green
```

- [ ] `just setup-mac` finished.
- [ ] `just check` is green.
- [ ] Start the big downloads in a second terminal now so they run during the builds below:
  ```bash
  just content-fetch            # frequency sources, ~60 MB
  just content-fetch nusa       # Javanese corpora from GitHub, ~5 MB
  just content-fetch kaikki     # Wiktionary extracts, ~100 MB
  just content-fetch jvwiki     # Javanese Wikipedia dump, ~70 MB
  ```

8 GB tip: close Chrome and other heavy apps during the first Android build. If it runs out of memory, use `CARGO_BUILD_JOBS=2`.

## 2. Mac app (~20 min) — [RUN.md §2](RUN.md)

- [ ] `just run-mac`: the welcome screen appears and the sidebar works.
- [ ] In the Mac app, Cmd+V pastes into the Reader, Cmd+C/A/Z work in text fields, Cmd+, opens Settings, and Cmd+Q quits.
- [ ] `just run-phone-preview`: the compact layout shows the bottom bar.
- [ ] Build the app icon once: `cd assets/export/macos && iconutil -c icns Kabar.iconset -o Kabar.icns && cd -`.
- [ ] `just bundle-mac` → `dist/mac/Kabar.app`. Open it from Finder and check the icon in the Dock and Finder.
- [ ] Kak Nur (M7, [RUN.md §7](RUN.md)): go through onboarding, tick a plan item, open the recap and the Kak Nur screen. Tell me anything that reads wrong or feels off.
- [ ] Read Kak Nur's lines in `content/coach/kak-nur.toml` (Egyptian Arabic, MSA, English). Mark what to change or say OK, and I set `reviewed = true`.
- [ ] Play (M10, [RUN.md §10](RUN.md)): try each game once. For Kabar Pagi audio, install the Indonesian voice "Damayanti" (System Settings → Accessibility → Spoken Content → System voice → Manage Voices) and check a story plays.
- [ ] Javanese games (M11, [RUN.md §10](RUN.md)): play the Unggah-Ungguh scenes and a d/dh · t/th round. Ask a Javanese speaker (or check yourself) whether the lines in `content/games/unggah-ungguh.toml` and the pairs in `content/games/minimal-pairs.toml` sound right; tell me what to fix and I'll set `reviewed = true`.
- [ ] Watch & listen (M12, [RUN.md §14](RUN.md)): with Kabar Cloud signed in, play Sinetron episode 1 and Kabar Radio *Apa kabar, Kota Tua?*. Tell me if any character's voice or pitch sounds wrong. Then download the Radio episode on the S24 Ultra and play it in airplane mode.
- [ ] Persona chats (M12, [RUN.md §14](RUN.md)): with Kabar Cloud signed in, chat a few turns with each persona. Tell me when a reply is off, a correction is wrong, or a Javanese line sounds unnatural (those stay "not checked yet" until you or a Javanese speaker approve them).
- [ ] Review the M10 content: the stories in `content/pagi/stories.jsonl` and the Pasar! seller lines in `content/games/pasar.toml` (Javanese register especially). Say OK or mark changes, and I set them reviewed.

## 3. Galaxy S24 Ultra and Tab S11 (~45 min) — [RUN.md §3–4](RUN.md)

- [ ] On both devices, Settings → About → Software information → tap **Build number** 7 times. Then turn on Developer options → **USB debugging** and **Wireless debugging**.
- [ ] `just android-keystore` (once). Back up `~/.kabar/kabar-release.jks` somewhere safe, such as an encrypted USB stick or a password manager attachment. Losing it means reinstalling the app from scratch.
- [ ] `just apk` → `dist/android/kabar.apk`.
- [ ] `just devices`, then `just install <serial>` for the S24 Ultra and again for the Tab S11. For wireless, use `just pair` / `just connect` (RUN.md §3).
- [ ] Open every screen once on the release APK (R8 shrinks it; a missing keep rule would crash on that screen) and tell me if anything closes the app.
- [ ] Check the launcher icon (round and adaptive), the notification-bar icon and the splash screen on both devices.
- [ ] Mid-lesson, switch dark mode from quick settings, and on the Tab S11 open Kabar in split screen and pop-up view: the lesson stays exactly where it was.
- [ ] Play a Radio episode, then take a call (or play a WhatsApp voice note): the episode pauses and carries on afterwards. Starting music in another app pauses it for good. The notification shows Kabar's icon.
- [ ] In Gallery, select two screenshots and share them: Kabar is in the share sheet, reads the first and says so.
- [x] **Pen Lab** on the S24 Ultra: two reports sent 2026-10-26, findings in [PENLAB.md](PENLAB.md). The ink feels attached to the pen tip, so the web ink stays and no native Kotlin ink is needed.

## 3b. Journey Map and Lesson Player (~15 min) — [RUN.md](RUN.md)

- [ ] Mac: `just run-mac` → Journey. Drag the map, zoom with the trackpad pinch and the +/− buttons, open Jakarta, then open "Selamat pagi!".
- [ ] Mac: play both test lessons (`bipa1.u01.l01`, `bipa1.u01.l02`) to the end. Check that the Indonesian voice speaks in listen and dictation exercises (macOS has an Indonesian system voice; if it's silent, add "Damayanti" under System Settings → Accessibility → Spoken Content → System voice → Manage Voices).
- [ ] S24 Ultra: Journey shows the vertical route; "Lanjut" opens the current lesson. Answer one translation with the normal keyboard.
- [ ] Tab S11: the full map pans and pinches with your fingers. Close a lesson halfway; reopening it resumes at the same exercise.
- [ ] On both devices, check the Indonesian voice is installed: Settings → General management → Text-to-speech output → engine settings (Samsung or Google) → Install voice data → Indonesian. If the app shows "download the Indonesian voice", that's this step.
- [ ] Tell me whether listen/dictation, slow replay and review audio cards speak on Android (the app now uses Android's own TextToSpeech). The first tap after launch can take a second while the voice engine starts.

## 3c. Reader and Screenshot Decoder (~20 min) — [RUN.md §9](RUN.md)
- [ ] `git pull && just check && just run-mac`, open **Reader**, paste the §24 chat lines, hover and click words; try split mode and the three explanation voices.
- [ ] Mac OCR: Reader → "Open a screenshot" (or drag an image in) with a WhatsApp screenshot; check Apple Vision reads it (first real test of the Vision code).
- [ ] `just apk` while online (first build fetches ML Kit from Google Maven; it must print "Kotlin glue present"), then `just install <serial>` on the S24 Ultra and Tab S11.
- [ ] On each device: share text from WhatsApp → **Kabar**; select text → ⋮ → **Kabar**; take a screenshot → Share → **Kabar**. `adb logcat -s KabarShare` if a share is ignored.
- [ ] On both devices, tap a word in the Reader: the card opens with its meaning and audio.
- [ ] Cold first popover on the S24 (SPEC §25, under 100 ms): with `just logs` running, force-stop Kabar, open it, go to **Reader**, open a saved text at once and tap a word. Send me the `kabar: glossary ready in … ms` line and say whether the first card felt slower than the next ones.
- [ ] Tab S11 with the S Pen in the Reader: hover over a word (the meaning floats up), tap it with the pen (the card opens), draw a loop around two words (the phrase is looked up), draw a line along a few words (they're highlighted and saved). Your palm on the screen shouldn't do anything while the pen is near. Tell me if any of these misses or feels slow.
- [ ] Send me one or two real WhatsApp screenshots you decoded (and how well the bubbles came out), so I can tune the screenshot structuring on real data.

## 4. Google sign-in clients (~10 min) — [CLOUD.md §3.2](CLOUD.md)

- [ ] Print the release SHA-1: `keytool -list -v -keystore ~/.kabar/kabar-release.jks -alias kabar`. The password is in Keychain under "kabar-android".
- [ ] Create three OAuth clients:
  - **Web** "Kabar server"
  - **Android** "Kabar Android" (package `app.kabar` + that SHA-1)
  - **Desktop** "Kabar Mac"
- [ ] Send me the Web and Desktop client ids; I put them in `crates/server/wrangler.toml` → `GOOGLE_CLIENT_IDS` (they aren't secrets).

## 5. Kabar Cloud (~30 min) — [CLOUD.md §3](CLOUD.md)

- [ ] 5.1 `just cloud-e2e`: the offline test is green (fake Google, phone → tablet sync, sign-out).
- [ ] 5.2 `just cloud-tools` (wrangler 4 + worker-build), then `wrangler login`: the browser opens, click **Allow**.
- [ ] 5.3 `just cloud-setup`: creates the D1 database and the token signing key (a Worker secret, never in the repo), asks for your Gmail for the allowlist, and writes the D1 id into `wrangler.toml`.
- [ ] 5.4 Keys. Each one switches an engine on:
  ```bash
  cd crates/server
  wrangler secret put GEMINI_API_KEY
  wrangler secret put AZURE_SPEECH_KEY     # and set AZURE_SPEECH_REGION in wrangler.toml
  cd -
  ```
  Only in a month with Max credit: `ANTHROPIC_API_KEY` plus `CLAUDE_ENABLED = "true"`.
- [ ] 5.4b Reminders (M7, [CLOUD.md §3.4](CLOUD.md)): in the Firebase console add the Android app `app.kabar` and keep `google-services.json` for the app build, outside the repo. Then Project settings → Service accounts → Generate new private key, `cd crates/server && wrangler secret put FCM_SERVICE_ACCOUNT < that-file.json && cd -`, and delete the file.
- [ ] 5.5 `just deploy-cloud`, then `curl https://kabar-cloud.<subdomain>.workers.dev/v1/health`.
- [ ] 5.6 Commit `crates/server/wrangler.toml` (the ids aren't secrets), then send me the URL.
- [ ] 5.6b Connect the app ([CLOUD.md §3.6](CLOUD.md)): `cp .env.example .env`, fill in the URL and the Desktop client's id + secret, then `just run-mac` → Settings → **Account & devices** → **Sign in with Google**. Rebuild the APK (`just apk`, `just install <serial>`) and sign in on both Galaxy devices. Save a word in **Try sync** on the phone and watch it appear on the tablet.

## 6. Content pipeline (~30 min, needs the downloads from step 1) — [CONTENT.md](CONTENT.md), [RUN.md §6](RUN.md)

- [ ] `just content-freq`: should produce no diff in `content/freq/id.tsv`.
- [ ] `just content-kaikki`: Wiktionary import plus the Arabic Bridge cross-check report. Send me `build/bridge-todo.txt`.
- [ ] `just content-freq-jv` → `content/freq/jv.tsv`: adds Javanese Wikipedia to the NusaWrites/NusaX counts. The Javanese gloss plan doesn't move.
- [ ] `just content-check`: everything validates.
- [ ] `just content-review 1` → open the review sheet in `build/review/` and mark what's wrong.
- [ ] No rush: Javanese review with a native speaker if you can (`content/javanese/speech-levels.jsonl`, `content/glosses/jv/`, `cargo run -p kabar-tools -- gloss review --lang jv 1`). Everything Javanese stays *unverified* until then.
- [ ] Commit the changed files under `content/` and push.

## 6b. Offline packs: natural-voice audio per unit (~15 min of yours, ~3 h unattended) — [RUN.md §13](RUN.md)

Uses the same Azure F0 key as step 5.4 (free; F0 never bills). All of BIPA 1–2 and J1–J3 is ~83k characters, about a sixth of the monthly free quota.

- [ ] Put `AZURE_SPEECH_KEY` and `AZURE_SPEECH_REGION` in `.env` (git-ignored).
- [ ] `just packs-audio-plan`: prints the characters it would use this month and refuses anything over the budget.
- [ ] `just packs-audio bipa1.u01`, then play two or three files from `~/.kabar/audio/` to check the voices.
- [ ] `just packs-audio` for the rest. It can run overnight and resumes where it stopped. Don't delete `~/.kabar/audio`: it holds the clips you already used quota on.
- [ ] `just packs-bundle`, then `gh auth login` (once) and `just packs-upload`.
- [ ] Commit and push `content/packs/` (the clips' hashes), then rebuild the app (`just run-mac`, `just apk` + `just install <serial>`).
- [ ] On the phone: Settings → **Offline packs** → download BIPA 1 unit 1, switch on airplane mode, open a lesson and tap a play button. You should hear the Azure voice, not the phone's.

## 7. Engine bake-off (~1 h, can run overnight) — [BAKEOFF.md](BAKEOFF.md)

This decides which free engine answers each AI task. Until it runs, every route in `prompts/routes.toml` is *provisional*, and its answers show a badge.

- [ ] Add `CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_API_TOKEN` (template "Workers AI") and `GEMINI_API_KEY` to `.env` ([BAKEOFF.md](BAKEOFF.md) "Running it" step 1).
- [ ] `brew install ollama`, start it, then `ollama pull aisingapore/Gemma-SEA-LION-v4-4B-VL` (and the Qwen 4B if you find its tag; BAKEOFF.md step 2).
- [ ] `just bakeoff-smoke`: a few minutes, 3 items per task. Send me the last lines if anything says *stopped* or *unreachable*.
- [ ] `just bakeoff` overnight. If it stops on Workers AI's daily limit, run it again the next day; it carries on where it stopped.
- [ ] Push `bakeoff/runs/<date>/` or zip and send it. I judge it blind, then write `docs/bakeoff-report.md` and the final `prompts/routes.toml`.
- [ ] Spot-check at least 10% of the scored items, plus every Javanese one.

## 7b. Teach Kabar (~10 min, after Kabar Cloud is up) — [RUN.md §8](RUN.md)

- [ ] Mac app → **Teach Kabar**: tap **Search Google** and check it opens your browser (not a window inside Kabar). Same on the phone.
- [ ] Choose **Wiktionary** for `kabar` and tap **Teach Kabar**. If it says Wiktionary can't be reached, tell me: the WebView may block the request.
- [ ] Save the answer of `https://en.wiktionary.org/api/rest_v1/page/definition/kabar` (open it in the browser, then Save As) as `fixtures/learn/wiktionary.kabar.json`, replacing the hand-written one, and run `just check`.
- [ ] Teach one real word you met (e.g. from a WhatsApp chat), approve it, and check it shows under **My dictionary** on the tablet after sync.

## 8. What to send back

- [ ] The `workers.dev` URL.
- [ ] Anything that failed: the command and its last 30 lines of output.
- [ ] Pushed commits: `wrangler.toml`, `content/`, `bakeoff/runs/`.

## 9. Deferred, not dropped

Moved out of the core on 2026-10-26, at your word: all stylus work, on the
phone and on the tablet, until the core app is finished, plus Mac extras
that only polish the laptop. Nothing is deleted from `docs/SPEC.md` — each
one is a later phase (SPEC §23 "Deferred"), and its steps come back here
with it.

- Pen writing on both devices, and Quick Capture on the phone: handwritten answers, Card Wall flicks, the Notebook,
  Gambar!, Listening Companion scribbles, write-mode lessons and aksara Jawa
  calligraphy. The Tablet Studio (M9) and aksara (M13) code is merged and
  waits for this; its checks (deploy the ink tables, Notebook feel, the
  stroke-order review in `scripts/aksara/preview.py`) return with the pen.
- The golden captures for the pen AI tasks (`read_handwriting`,
  `guess_sketch`, `check_strokes`).
- Mac extras: the global Quick Lookup hotkey, Mac-specific polish, drawing in
  Pen Lab with the trackpad, and the optional Cloudflare plugin for Claude
  Code.

Pen Lab already measured the S24 Ultra, so when the pen comes back the web
ink is known to be good enough.
