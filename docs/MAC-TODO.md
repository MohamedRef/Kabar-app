# Mac session checklist

Everything that can't be done from the cloud sandbox, in the order that gets
through it in one sitting on the MacBook (8 GB). Work top to bottom. Nothing
here costs money. Details live in the linked docs; this page is the order.

Other threads add their Mac-only steps here by PR, into the section they
belong to. Keep one checkbox per action.

**Time:** about 3–4 hours, much of it downloads and builds you can leave running.

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
- [ ] `just run-phone-preview`: the compact layout shows the bottom bar.
- [ ] Build the app icon once: `cd assets/export/macos && iconutil -c icns Kabar.iconset -o Kabar.icns && cd -`.
- [ ] `just bundle-mac` → `dist/mac/Kabar.app`. Open it from Finder and check the icon in the Dock and Finder.
- [ ] Draw in Pen Lab with the trackpad once to check that strokes appear under the pointer.
- [ ] Kak Nur (M7, [RUN.md §7](RUN.md)): go through onboarding, tick a plan item, open the recap and the Kak Nur screen. Tell me anything that reads wrong or feels off.
- [ ] Read Kak Nur's lines in `content/coach/kak-nur.toml` (Egyptian Arabic, MSA, English). Mark what to change or say OK, and I set `reviewed = true`.
- [ ] Play (M10, [RUN.md §10](RUN.md)): try each game once. For Kabar Pagi audio, install the Indonesian voice "Damayanti" (System Settings → Accessibility → Spoken Content → System voice → Manage Voices) and check a story plays.
- [ ] Javanese games (M11, [RUN.md §10](RUN.md)): play the Unggah-Ungguh scenes and a d/dh · t/th round. Ask a Javanese speaker (or check yourself) whether the lines in `content/games/unggah-ungguh.toml` and the pairs in `content/games/minimal-pairs.toml` sound right; tell me what to fix and I'll set `reviewed = true`.
- [ ] Review the M10 content: the stories in `content/pagi/stories.jsonl` and the Pasar! seller lines in `content/games/pasar.toml` (Javanese register especially). Say OK or mark changes, and I set them reviewed.

## 3. Galaxy S24 Ultra and Tab S11 (~45 min) — [RUN.md §3–4](RUN.md)

- [ ] On both devices, Settings → About → Software information → tap **Build number** 7 times. Then turn on Developer options → **USB debugging** and **Wireless debugging**.
- [ ] `just android-keystore` (once). Back up `~/.kabar/kabar-release.jks` somewhere safe, such as an encrypted USB stick or a password manager attachment. Losing it means reinstalling the app from scratch.
- [ ] `just apk` → `dist/android/kabar.apk`.
- [ ] `just devices`, then `just install <serial>` for the S24 Ultra and again for the Tab S11. For wireless, use `just pair` / `just connect` (RUN.md §3).
- [ ] Check the launcher icon (round and adaptive), the notification-bar icon and the splash screen on both devices.
- [ ] **Pen Lab** on each device (phone: Settings → Pen Lab), following the 7 steps in RUN.md §4. Save, **Copy JSON**, and send me one report per device. DeX on the tablet is a bonus third report.
- [ ] Tell me how the ink *feels*: does it trail behind the pen tip, and by roughly how much?
- [ ] **Golden captures** for the pen tasks (~30 min, any time): `cargo run -p kabar-tools -- golden stats` lists 104 items tagged `needs-capture` in `content/golden/read_handwriting.yaml`, `guess_sketch.yaml` and `check_strokes.yaml`. Each item's `input.capture` says what to write or draw, on which device and with which pen. Save each as `fixtures/golden/<task>/<item id>.png` (strokes as JSON if the app offers it) and push or send them; I wire them in.

## 3b. Journey Map and Lesson Player (~15 min) — [RUN.md](RUN.md)

- [ ] Mac: `just run-mac` → Journey. Drag the map, zoom with the trackpad pinch and the +/− buttons, open Jakarta, then open "Selamat pagi!".
- [ ] Mac: play both test lessons (`bipa1.u01.l01`, `bipa1.u01.l02`) to the end. Check that the Indonesian voice speaks in listen and dictation exercises (macOS has an Indonesian system voice; if it's silent, add "Damayanti" under System Settings → Accessibility → Spoken Content → System voice → Manage Voices).
- [ ] S24 Ultra: Journey shows the vertical route; "Lanjut" opens the current lesson. Answer one translation with the Samsung handwriting keyboard and the S Pen.
- [ ] Tab S11: the full map pans with the pen and pinches with two fingers. Close a lesson halfway; reopening it resumes at the same exercise.
- [ ] On both devices, check the Indonesian voice is installed: Settings → General management → Text-to-speech output → engine settings (Samsung or Google) → Install voice data → Indonesian. If the app shows "download the Indonesian voice", that's this step.
- [ ] Tell me whether listen/dictation, slow replay and review audio cards speak on Android (the app now uses Android's own TextToSpeech). The first tap after launch can take a second while the voice engine starts.

## 3c. Reader and Screenshot Decoder (~20 min) — [RUN.md §9](RUN.md)
- [ ] `git pull && just check && just run-mac`, open **Reader**, paste the §24 chat lines, hover and click words; try split mode and the three explanation voices.
- [ ] Mac OCR: Reader → "Open a screenshot" (or drag an image in) with a WhatsApp screenshot; check Apple Vision reads it (first real test of the Vision code).
- [ ] `just apk` while online (first build fetches ML Kit from Google Maven; it must print "Kotlin glue present"), then `just install <serial>` on the S24 Ultra and Tab S11.
- [ ] On each device: share text from WhatsApp → **Kabar**; select text → ⋮ → **Kabar**; take a screenshot → Share → **Kabar**. `adb logcat -s KabarShare` if a share is ignored.
- [ ] Pen in the Reader: hover a word (peek), tap it (card), loop around a phrase (lookup), stroke across a phrase (highlight saved). Tell me how it feels and anything that misfires.
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
- [ ] 5.7 Optional, if you use Claude Code on the Mac: `claude plugin marketplace add cloudflare/skills`, then `claude plugin install cloudflare@cloudflare` (Cloudflare's own skills + MCP; signs in to Cloudflare in the browser). Kabar keeps deploying with wrangler.

## 6. Content pipeline (~30 min, needs the downloads from step 1) — [CONTENT.md](CONTENT.md), [RUN.md §6](RUN.md)

- [ ] `just content-freq`: should produce no diff in `content/freq/id.tsv`.
- [ ] `just content-kaikki`: Wiktionary import plus the Arabic Bridge cross-check report. Send me `build/bridge-todo.txt`.
- [ ] `just content-freq-jv` → `content/freq/jv.tsv`: adds Javanese Wikipedia to the NusaWrites/NusaX counts. The Javanese gloss plan doesn't move.
- [ ] `just content-check`: everything validates.
- [ ] `just content-review 1` → open the review sheet in `build/review/` and mark what's wrong.
- [ ] No rush: Javanese review with a native speaker if you can (`content/javanese/speech-levels.jsonl`, `content/glosses/jv/`, `cargo run -p kabar-tools -- gloss review --lang jv 1`). Everything Javanese stays *unverified* until then.
- [ ] Commit the changed files under `content/` and push.

## 7. Engine bake-off (~1 h, can run overnight) — [BAKEOFF.md](BAKEOFF.md)

This decides which free engine answers each AI task. Until it runs, every route in `prompts/routes.toml` is *provisional*, and its answers show a badge.

- [ ] `brew install ollama`, then pull the two 4B SEA-LION v4 models (Gemma-SEA-LION-v4-4B-VL and Qwen-SEA-LION-v4-4B-VL at Q4). Exact `ollama pull` names will be in BAKEOFF.md with the runner.
- [ ] Optional: add `DAYTONA_API_KEY` as an environment secret so the heavy 8–9B check can run there instead of the Mac.
- [ ] Run the bake-off runner (command will be in BAKEOFF.md once it lands) and send me `bakeoff/runs/`. I score it blind with you, then write `docs/bakeoff-report.md` and the final `prompts/routes.toml`.
- [ ] Spot-check at least 10% of the scored items, plus every Javanese one.

## 7b. Teach Kabar (~10 min, after Kabar Cloud is up) — [RUN.md §9](RUN.md)

- [ ] Mac app → **Teach Kabar**: tap **Search Google** and check it opens your browser (not a window inside Kabar). Same on the phone.
- [ ] Choose **Wiktionary** for `kabar` and tap **Teach Kabar**. If it says Wiktionary can't be reached, tell me: the WebView may block the request.
- [ ] Save the answer of `https://en.wiktionary.org/api/rest_v1/page/definition/kabar` (open it in the browser, then Save As) as `fixtures/learn/wiktionary.kabar.json`, replacing the hand-written one, and run `just check`.
- [ ] Teach one real word you met (e.g. from a WhatsApp chat), approve it, and check it shows under **My dictionary** on the tablet after sync.

## 7c. Tablet Studio (~20 min, best on the Tab S11) — [RUN.md §11](RUN.md)

- [ ] `just deploy-cloud` once, so the three new ink tables (`notebook_items`, `ink_transcripts`, and the `ink_documents`/`ink_strokes`/`notebooks` set) are accepted by the server. Without it, ink stays on the device.
- [ ] Tab S11 → **Studio** → **Notebook**: write a few lines with the S Pen while resting your palm on the glass. Tell me if any palm touch leaves a mark, and whether the ink still feels stuck to the pen tip.
- [ ] Two fingers to pan, pinch to zoom, then the eraser tool. Does erasing feel like it catches the stroke you aim at?
- [ ] **Add a word card** for `pasar`, move on to a second page, come back: the card should still be there, and on the phone after sync.
- [ ] Search `pasar` in the Notebook side bar and tap the hit: it should scroll the page to it.
- [ ] **Gambar!**: play two rounds. It will ask you to judge your own drawing for now; tell me whether the word choice felt reasonable.
- [ ] **Listening Companion**: open YouTube in split screen, pause on a word you didn't catch, and write it as it sounded (e.g. `moerah`, `sinaoe`). The suggestions should include the real word; tell me when they don't.
- [ ] **Mind map**: `di pasar`. It needs an AI engine, so it may say "not connected yet" until Kabar Cloud answers.
- [ ] **Dictation**: write one word with the pen, then type it in the box and tap Check; the feedback should circle the wrong letters.

## 8. What to send back

- [ ] Two (or three) Pen Lab JSON reports, plus a sentence on how the ink feels — and the same for the Studio's Notebook (palm rejection, eraser, pan/zoom).
- [ ] The `workers.dev` URL.
- [ ] Anything that failed: the command and its last 30 lines of output.
- [ ] Pushed commits: `wrangler.toml`, `content/`, `bakeoff/runs/`.
