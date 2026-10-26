# Running Kabar (M1–M2, M7)

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

To jump straight into a lesson (until the Journey Map links them), start the app with `KABAR_OPEN_LESSON`, e.g. `KABAR_OPEN_LESSON=bipa1.u01.l01 just run-mac`. The lessons built in so far are `bipa1.u01.l01` (Selamat pagi!) and `bipa1.u01.l02` (Apa kabar?). Closing a lesson saves your place; opening it again continues there.

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
- **Kak Nur** (nav): weekly report, Liga Pribadi, achievements, the Warung and the learning journal.
- **Settings → Kak Nur**: reminder style, study times, quiet hours, report day.

To start onboarding again, delete the database (§2). On the Galaxy devices: `just apk` and `just install <serial>` as in §3.

Reminders arrive as push notifications once Kabar Cloud's reminder alarm and Firebase are set up (CLOUD.md §3.4).

## 8. Teach Kabar (spec §26)

Open **Teach Kabar** (علّم كبر) from the side bar on the Mac and tablet, or from Settings on the phone. Type the word, optionally the sentence you met it in, then paste what you found (or a link, or use Wiktionary) and tap **Teach Kabar**. The draft appears below for review: approve it, edit it, ask for another version, or discard it. Approved words show in **My dictionary** and, once the Reader lands, in popovers with a "taught by you" badge.

Writing the draft needs an AI engine, so until the app is connected to Kabar Cloud the screen says "not connected yet". Search Google opens the browser; nothing from Google is read automatically.

## 9. Troubleshooting

- `dx` can't find the NDK: `echo $ANDROID_NDK_HOME` should print the NDK path; re-run `source ~/.kabar/env.sh`.
- `just logs <serial>` streams the app's log while it's running.
- Build out of memory on 8 GB: `CARGO_BUILD_JOBS=2 just apk`.
