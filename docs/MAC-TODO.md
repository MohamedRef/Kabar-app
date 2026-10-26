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
- [ ] **Cloudflare**: free account at <https://dash.cloudflare.com/sign-up> (no card).
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

## 3. Galaxy S24 Ultra and Tab S11 (~45 min) — [RUN.md §3–4](RUN.md)

- [ ] On both devices, Settings → About → Software information → tap **Build number** 7 times. Then turn on Developer options → **USB debugging** and **Wireless debugging**.
- [ ] `just android-keystore` (once). Back up `~/.kabar/kabar-release.jks` somewhere safe, such as an encrypted USB stick or a password manager attachment. Losing it means reinstalling the app from scratch.
- [ ] `just apk` → `dist/android/kabar.apk`.
- [ ] `just devices`, then `just install <serial>` for the S24 Ultra and again for the Tab S11. For wireless, use `just pair` / `just connect` (RUN.md §3).
- [ ] Check the launcher icon (round and adaptive), the notification-bar icon and the splash screen on both devices.
- [ ] **Pen Lab** on each device (phone: Settings → Pen Lab), following the 7 steps in RUN.md §4. Save, **Copy JSON**, and send me one report per device. DeX on the tablet is a bonus third report.
- [ ] Tell me how the ink *feels*: does it trail behind the pen tip, and by roughly how much?

## 4. Google sign-in clients (~10 min) — [CLOUD.md §3.2](CLOUD.md)

- [ ] Print the release SHA-1: `keytool -list -v -keystore ~/.kabar/kabar-release.jks -alias kabar`. The password is in Keychain under "kabar-android".
- [ ] Create three OAuth clients:
  - **Web** "Kabar server"
  - **Android** "Kabar Android" (package `app.kabar` + that SHA-1)
  - **Desktop** "Kabar Mac"
- [ ] Put the Web and Desktop client ids in `crates/server/wrangler.toml` → `GOOGLE_CLIENT_IDS`.

## 5. Kabar Cloud (~30 min) — [CLOUD.md §3](CLOUD.md)

- [ ] 5.1 `just cloud-e2e`: the offline test is green (fake Google, phone → tablet sync, sign-out).
- [ ] 5.2 `just cloud-tools`, then `wrangler login` (opens the browser).
- [ ] 5.3 `just cloud-setup`: creates D1, the signing key and your allowlist, and writes the D1 id into `wrangler.toml`.
- [ ] 5.4 Keys. Each one switches an engine on:
  ```bash
  cd crates/server
  wrangler secret put GEMINI_API_KEY
  wrangler secret put AZURE_SPEECH_KEY     # and set AZURE_SPEECH_REGION in wrangler.toml
  cd -
  ```
  Only in a month with Max credit: `ANTHROPIC_API_KEY` plus `CLAUDE_ENABLED = "true"`.
- [ ] 5.5 `just deploy-cloud`, then `curl https://kabar-cloud.<subdomain>.workers.dev/v1/health`.
- [ ] 5.6 Commit `crates/server/wrangler.toml` (the ids aren't secrets), then send me the URL.

## 6. Content pipeline (~30 min, needs the downloads from step 1) — [CONTENT.md](CONTENT.md), [RUN.md §6](RUN.md)

- [ ] `just content-freq`: should produce no diff in `content/freq/id.tsv`.
- [ ] `just content-kaikki`: Wiktionary import plus the Arabic Bridge cross-check report.
- [ ] `just content-freq-jv` → `content/freq/jv.tsv` (Javanese frequency list).
- [ ] `just content-check`: everything validates.
- [ ] `just content-review 1` → open the review sheet in `build/review/` and mark what's wrong.
- [ ] Commit the changed files under `content/` and push.

## 7. Engine bake-off (~1 h, can run overnight) — [BAKEOFF.md](BAKEOFF.md)

This decides which free engine answers each AI task. Until it runs, every route in `prompts/routes.toml` is *provisional*, and its answers show a badge.

- [ ] `brew install ollama`, then pull the two 4B SEA-LION v4 models (Gemma-SEA-LION-v4-4B-VL and Qwen-SEA-LION-v4-4B-VL at Q4). Exact `ollama pull` names will be in BAKEOFF.md with the runner.
- [ ] Optional: add `DAYTONA_API_KEY` as an environment secret so the heavy 8–9B check can run there instead of the Mac.
- [ ] Run the bake-off runner (command will be in BAKEOFF.md once it lands) and send me `bakeoff/runs/`. I score it blind with you, then write `docs/bakeoff-report.md` and the final `prompts/routes.toml`.
- [ ] Spot-check at least 10% of the scored items, plus every Javanese one.

## 8. What to send back

- [ ] Two (or three) Pen Lab JSON reports, plus a sentence on how the ink feels.
- [ ] The `workers.dev` URL.
- [ ] Anything that failed: the command and its last 30 lines of output.
- [ ] Pushed commits: `wrangler.toml`, `content/`, `bakeoff/runs/`.
