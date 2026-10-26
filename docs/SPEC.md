# Kabar — Build Brief for Claude Code (v4)

> *Apa kabar?* — from خبر to **kabar**.
> A personal Indonesian + Javanese learning app, built in Rust, for one learner: me.

Version 4 · 26 October 2026 · supersedes v1–v3 and the v3.1 addendum

---

## 0. Read me first (Claude Code)

You're building **Kabar** for exactly one user. I'm a native Egyptian Arabic speaker, I read English fluently, and I'm a beginner in Indonesian and Javanese. Private use only, never published.

Two non-negotiables: **it costs $0/month to run**, and **it must not feel cheap** — no loss of quality compared with a paid-AI version. Principle 5 (§2), the content pipeline (§18) and the quality system (§19) explain how.

How to work with me:

1. Save this file verbatim as `docs/SPEC.md`, then write a short `CLAUDE.md` with the conventions that matter every session. If older versions of this brief exist anywhere, v4 wins.
2. Verify before you trust. Check current docs, pricing and free-tier limits for every service and dependency named here (Dioxus, sastrawi-rs, fsrs, workers-rs, Cloudflare Workers / D1 / Durable Objects, Firebase Cloud Messaging, Google Sign-In, Gemini API, Azure Speech, Ollama, ML Kit, and the Anthropic API at docs.claude.com). If anything is outdated, unmaintained or no longer free, tell me and propose the closest alternative.
3. Start with **Milestone 0**: write `docs/PLAN.md` (architecture, crate layout, data model, auth, sync design, a free-tier budget per service, risks) and run the **engine bake-off** (§19). Wait for my approval before writing app code.
4. In M1, build a tiny **Pen Lab** screen that I run on my S24 Ultra and Tab S11. It logs exactly which pen events arrive (hover, pressure, tilt, barrel button, eraser, coalesced/predicted events) and measures ink latency. Decide pen features from that real data, not from assumptions.
5. Then go milestone by milestone. After each one: `cargo fmt`, `clippy` with zero warnings, tests green; tell me exactly how to run it on the Mac and install it on both Galaxy devices; summarize what changed; commit with a clear message.
6. Never drop a feature from this brief. If something is hard, phase it.
7. Ask before adding anything that could cost money, or changing a stack decision.
8. Reply in whatever language I write in (Egyptian Arabic or English). Code, comments, commits and docs stay in English.
9. No secrets in the repo. Ever.

## 1. Vision

Kabar is a clean-room "best of" language app, built the way the ArtCraft suite rebuilds famous software in Rust: study what the most successful apps do best, and merge it into one app that's mine.

| From | What Kabar takes |
|---|---|
| Duolingo | the path, streaks, XP, daily goal, bite-sized exercises |
| Airlearn | learn first, practice next (short teaching slides before quizzes), skip ahead to advanced modules, leagues |
| LingQ | import anything; every word colored by how well I know it |
| Language Reactor | original and translation side by side with synchronized highlighting |
| Anki | FSRS spaced repetition, tuned to my own review history |
| Speak / Praktika | AI conversation partners |
| HelloTalk | inline corrections of what I write |
| GoodNotes-style apps | handwriting, notebooks and ink as a first-class way to study |

Plus things none of them have: the **Arabic Bridge**, the **Unggah-Ungguh Simulator**, the **Screenshot Decoder**, a pen-first **Tablet Studio**, a coach who follows up every day, and a story world that grows with my level.

The feeling: a calm, warm study companion — part travel journal, part market, part tutor. Never childish, never noisy.

**First launch:** the screen greets me with *"Apa kabar?"* and reveals that *kabar* comes from Arabic «خبر». My first word is one I already know.

## 2. Experience principles

These five rules beat any single feature. When in doubt, choose what serves them.

1. **Smooth.** Local-first: reviews, lessons and popovers run from local data with no spinners. Prefetch the next exercise and its audio. Optimistic updates, background sync. Open straight into "continue where I left off". Undo instead of confirmation dialogs.
2. **Encouraging.** Warm, specific praise ("you nailed -kan verbs today"). Mistakes are framed as tomorrow's plan. Never guilt, fear, fake urgency or dark patterns. Celebrate effort and comebacks, not only perfection.
3. **Always following up.** Kabar notices and responds: a daily plan, a short end-of-day recap, a weekly report, a monthly level check, and a plan that adapts to what it sees.
4. **One account, one journey.** Sign in once per device. Progress, reviews, notes, ink and settings are the same everywhere, in near real time. Start on the phone, continue on the tablet at the exact same exercise.
5. **Free by default, no compromise on quality.** Quality-critical thinking happens at build time with the best model available (Claude Code), gets reviewed by me, and ships inside the app. At runtime, each AI task goes to the best *free* engine that passes that task's quality bar — measured, not assumed. Paid engines are an optional switch, never a dependency.

## 3. My devices & the pen

| Device | Role | Signature experience |
|---|---|---|
| **Galaxy S24 Ultra** (Android, 6.8", built-in S Pen) | the pocket coach: daily plan, reviews, Reader on the go, Kabar Radio | one-handed UI, pen hover popovers, lasso lookup, handwritten answers, Quick Capture, Share-to-Kabar |
| **Galaxy Tab S11** (Android tablet, S Pen, DeX) | the **Studio**: deep lessons, reading, writing, creativity | three-pane Study Desk, Kabar Notebook, ink everywhere, split-screen companion, DeX keyboard + mouse |
| **MacBook Air M2** (macOS, Apple Silicon) | build machine, desk study, and **Kabar Home** (local AI helper, §8) | native window, keyboard shortcuts, global Quick Lookup |

**A day with Kabar:** Kak Nur's plan and Kabar Pagi on the phone with morning coffee → Kabar Radio in my earbuds on the way out → a lesson and Notebook session on the tablet in the evening → reviews on the phone before sleep → a 30-second recap. Every device always knows where I am.

**One input model on every device:**

| Action | Mac / DeX mouse | S Pen (phone & tablet) | Finger |
|---|---|---|---|
| Peek at a word | hover | hover (pen near the screen) | tap |
| Open the full word card | click | tap with the pen | long-press |
| Look up a phrase | drag-select | draw a loop around it (lasso) | drag-select |
| Save a phrase for review | right-click → Save | highlight stroke | long-press → Save |
| Answer an exercise | type | write by hand or type | type / tap |
| Erase ink | — | eraser or barrel button (if exposed) | undo button |

**Pen engineering:**

- Use Pointer Events with `pointerType = "pen"`: pressure, tilt, hover, plus coalesced and predicted events for smooth, low-latency ink; a desynchronized canvas context where supported. Bridge with JNI/Kotlin wherever the Dioxus Android WebView falls short (the Pen Lab tells us where).
- Palm rejection: while the pen is in range, ignore touch on ink surfaces.
- Ink rendering runs inside the WebView (a small TS/JS or WASM module) for latency. Strokes are stored as vectors (points + pressure + time) in Rust and synced.
- Handwriting recognition: ML Kit Digital Ink Recognition on-device through a Kotlin bridge (verify Indonesian support), with an AI fallback. Every text field must also work with Samsung's handwriting keyboard.

**Platform notes:**

- Targets: macOS (aarch64-apple-darwin) and Android (arm64-v8a only). A web build is a bonus; iOS is not needed.
- Toolchain setup on the Mac (Rust targets, Android SDK/NDK, Java) is part of M1 — script it (`justfile` or `xtask`).
- Android distribution: a signed APK I sideload, no Play Store. Document the install flow from the Mac (adb over USB and wireless debugging).
- Layout adapts to window width, not device name: compact (phone), medium (tablet portrait, split screen), expanded (tablet landscape, DeX, Mac). Use Android's current window size class breakpoints — verify them.
- DeX or any mouse behaves like the Mac: real hover, right-click menu, keyboard shortcuts.

## 4. The Tablet Studio (Galaxy Tab S11)

The tablet is where Kabar becomes creative.

- **Study Desk** (landscape): three panes — Journey/Library | original text | explanation. Drag a word with the pen into the Review Basket or the Notebook.
- **Kabar Notebook (دفتر):** an infinite canvas per unit. Handwrite notes, drop in live word cards (with audio and hover), draw arrows and mind maps (family words around a family tree). Handwriting is transcribed in the background and becomes searchable. Synced, and viewable on the phone and Mac.
- **Gambar!** (draw it): Kabar says a word ("kucing", "pasar", "hujan"); I sketch it; AI vision guesses what I drew, in Indonesian. Funny, fast and memorable.
- **Card Wall:** a review session laid out like a table. I flick cards with the pen into *tahu* (know) or *belum* (not yet) piles; the FSRS grade comes from where and how fast I sort.
- **Listening Companion:** Kabar beside YouTube in split screen. When I hear a word, I scribble it the way I think it's spelled; Kabar suggests the right spelling, meaning and audio, and saves it.
- **Write-mode lessons:** dictation and translation answered by hand, with ink feedback — wrong letters circled, then the correct form written in.
- **Mind-map generator:** "make me a map of *di pasar*" → an editable map I extend with the pen.
- **Aksara Jawa calligraphy:** pressure-sensitive brush, animated stroke order, tracing → free writing; AI vision checks the shapes.
- **Drag & drop:** text or images dragged from another app in split screen land in the Reader (verify cross-app drag & drop into the WebView).
- **DeX mode:** desktop-style window, shortcuts and mouse hover, exactly like the Mac.

## 5. The S Pen on the phone (Galaxy S24 Ultra)

- **Quick Capture:** write a word or phrase I just saw or heard; Kabar transcribes, explains and saves it. Investigate whether pen-removal events are available to open it automatically; otherwise use a quick-settings tile or app shortcut.
- Pen hover popovers in the Reader, lessons and persona chats.
- Lasso lookup and highlight-to-save, same as on the tablet.
- Reviews: answer production cards by handwriting.
- One-handed design for finger use: everything important within thumb reach. The pen is a bonus, never required.

## 6. Coach & continuous follow-up

**Kak Nur** — *Kak* is the friendly Indonesian word for an older sibling, and *Nur* is نور. Speaks Egyptian Arabic (or English) with Indonesian sprinkled in; warm, honest, specific; remembers my goals, struggles and wins. Its voice and message templates are written at build time (§18); runtime AI only fills in the personal details.

- **Onboarding:** why I'm learning, minutes per day, a target (e.g., BIPA 2 by a date), preferred study times, reminder style. Then a two-minute placement chat with Mas Ardi decides where my journey starts.
- **Daily plan** (*Rencana Hari Ini*): built from due reviews, the current unit and weak spots, sized by one question: "5, 15 or 30 minutes today?" Same plan on every device. Reviews come first: 5 min = reviews (≤2 min) + the next lesson; 15 min = reviews (~6) + one lesson (~6) + a weak spot (~3); 30 min = two such blocks with a break offer, adding a short read or listen. At most 5 / 12 / 20 new words and 2 new grammar points a day; a comeback after a break starts with the light 5-minute plan (`docs/PEDAGOGY.md`).
- **End-of-day recap:** 30 seconds — what I learned, one thing to remember, tomorrow's first step.
- **Smart reminders:** at my chosen times, nudged toward when I actually study (learned from my history). Quiet hours respected; one tap to snooze. **Account-aware:** once I've met today's goal on any device, the others stay silent.
- **Weekly report** (Thursday evening by default): time, new words, retention, streak, strongest and weakest skills, a note from Kak Nur, next week's focus. The plan adapts automatically.
- **Monthly level check:** a 10-minute mini placement test that updates the skill radar (reading, listening, writing, speaking, vocabulary) and the forecast: "at this pace you reach BIPA 3 around <date>".
- **Welcome back:** after a break, a short, light session. No shame, no streak drama.
- **Rewards:** Pelita, the streak lamp (freezes earned, never bought) · Koin spent in the Warung on cosmetic batik themes · Liga Pribadi, a weekly league against my own past weeks · achievements named after proverbs (*"Sedikit demi sedikit, lama-lama menjadi bukit"*) · souvenirs in the Suitcase.
- **Learning journal:** an automatic timeline of everything I've done, plus optional handwritten reflections on the tablet.
- **Widgets** (Android, stretch): Pelita + due count + word of the day on both Galaxy home screens; tap to start reviewing.

## 7. Account & sync

- **Kabar account:** sign in once per device with Google (Android: Credential Manager via a Kotlin bridge; Mac: system-browser OAuth with PKCE). The server accepts only my Google account (allowlist).
- **Tokens:** short-lived access + refresh tokens in Android Keystore / macOS Keychain. A **Devices** screen shows each device and its last sync, and can sign one out remotely.
- **Live sync:** one Durable Object per account acts as the WebSocket hub (use WebSocket hibernation to stay inside free limits — verify); changes show up on my other devices within seconds. Offline changes queue and replay.
- **Lanjut (handoff):** opening Kabar on another device shows "You were on exercise 7 of Lesson 3 on your S24 Ultra, 4 min ago — continue here?"
- **What syncs:** progress, reviews, word statuses, settings, daily plans, notebooks and ink, highlights, journal, conversations, coach memory, AI-cache entries, content-pack versions. Audio and other large blobs sync on demand and are cached per device.
- **Conflict rules:** append-only events for reviews, attempts and XP; last-writer-wins with hybrid logical clocks for statuses and settings; ink strokes are immutable objects (deleting is an event).
- **Push:** Firebase Cloud Messaging for Android reminders and live nudges; native notifications on the Mac; locally scheduled reminders as the offline fallback.
- **Security & backups:** HTTPS only, encryption at rest, a free backup path for D1 (evaluate D1 Time Travel and free object storage), one-click full export from Settings.

## 8. Stack & architecture

- **Rust** (latest stable, edition 2024), Cargo workspace:
  - `crates/core` — pure logic, no UI: tokenizer, normalizer, morphology, dictionary, language ID, SRS, curriculum engine, coach logic, AI router, TTS client, caching, sync engine.
  - `crates/protocol` — shared API types (serde), compiled for native and wasm32.
  - `crates/app` — Dioxus UI, one codebase for macOS, Android and web.
  - `crates/server` — **Kabar Cloud**, a Rust Cloudflare Worker (workers-rs) on the Workers Free plan: auth, D1 storage, the Durable Object sync hub, push via FCM, the key vault (Worker secrets), AI/TTS proxy with a shared cache, and scheduled jobs for daily plans and weekly reports (verify Cron Triggers on the free plan). Keep per-request CPU tiny; heavy work runs in Durable Objects or on devices. The devices never store API keys.
  - `crates/home` — **Kabar Home**, an optional helper on my MacBook: Ollama (local LLM), NLLB-200 translation, Whisper. Reachable from the Galaxy devices on my Wi-Fi, and optionally anywhere via a secure tunnel (evaluate Tailscale and Cloudflare Tunnel). Kabar works fully without it.
  - `crates/tools` — the content pipeline (§18), dictionary import, audio pre-generation, the bake-off runner, backups.
  - `android/` — minimal Kotlin glue: sign-in, push, share target, ML Kit (OCR + handwriting), widgets, pen extras.
- **UI: Dioxus** (latest stable; 0.7.x as of this writing), styled with HTML/CSS. Chosen over egui (ArtCraft's choice) because Kabar needs correct Arabic shaping + RTL, rich popovers, ink, and Android.
- **Local storage:** SQLite via rusqlite (bundled, FTS5) with migrations. Offline-first: the app works fully from local data and content packs; the server is for sync, keys, push and shared caches.
- **Offline mode:** if Kabar Cloud is unreachable, everything local keeps working, changes queue up, and AI features say "offline" gracefully.

## 9. Design language

- **Palette from Java:** Merapi black sand (deep charcoal), *sawah* green (rice terraces), Bromo dawn (warm orange-pink accents), batik indigo, and **Bridge gold** reserved for Arabic-origin words. Light and dark themes, WCAG AA contrast.
- **Patterns:** subtle, original, procedurally generated SVG backgrounds inspired by geometric batik motifs (kawung-like circles, parang-like diagonals). Generated in code, never copied images.
- **Ink:** batik-indigo pen by default; highlighter colors match word statuses; an optional paper texture in the Notebook.
- **Type:** a clean Latin sans for Indonesian/Javanese; a beautiful Arabic face for explanations (e.g., IBM Plex Sans Arabic or Noto Naskh Arabic — OFL only). Optional Eastern Arabic numerals (٠١٢٣) in the Arabic UI.
- **RTL done right:** logical CSS properties everywhere, mirrored navigation in the Arabic UI, Latin snippets isolated with `<bdi>`.
- **Sound** (toggleable): soft gamelan-like tones — a gong when a lesson ends, a light *kendang* tap for correct answers. Synthesized or CC0; listed in ATTRIBUTION.md.
- **Motion:** calm and quick (150–250 ms), respects reduced-motion. Subtle haptics on Android.
- **Original art only:** no copyrighted characters, mascots or brand art.

## 10. The Journey Map

- **Indonesian (BIPA 1 → 7)** is an island-hopping route: Jakarta (BIPA 1) → Bali (2) → Lombok (3) → Sulawesi (4) → Kalimantan (5) → Sumatra (6) → Raja Ampat, Papua (7). Each island is a level, each town a unit, each stop a lesson.
- **Javanese (J1 → J6)** is a train line across Java: Surabaya (J1) → Madiun (J2) → Solo (J3) → Yogyakarta (J4) → a branch line to Semarang (J5) → Kotagede, the old Mataram capital (J6).
- An original, stylized SVG map drawn from public-domain geodata (e.g., Natural Earth).
- Each finished stop earns a **souvenir card** — a short culture note (batik, wayang, gamelan, rendang, angklung…) written at build time and kept in my **Suitcase**.
- **"Take a flight"** = placement test. Skipping ahead is always allowed.
- Phone: a vertical scrolling route. Tablet / Mac: the full map with pan and zoom (pinch, pen or trackpad).

## 11. Reader & Screenshot Decoder — the heart of Kabar

**Input:** paste or type text, share from any app, drag & drop, or a screenshot (URL import is a stretch goal).

- **Share-to-Kabar (Android):** a share target for text and images, so anything from WhatsApp, Instagram, TikTok or Chrome lands straight in the Reader.
- **Screenshot Decoder:** on-device OCR first (ML Kit Text Recognition on Android, Apple Vision on the Mac, Tesseract `ind`/`jav` as a fallback), keeping text boxes and layout; then AI structures it (chat bubbles, speakers, timestamps, UI labels); then the Reader takes over.
- **Inline mode:** every token colored by status (new / learning / known / ignored); Arabic-origin words underlined in Bridge gold.
- **Split mode:** original on one side, explanation (Arabic or English) on the other, with token-to-span alignment that highlights both ways.
- **Pen layer:** lasso a phrase to look it up, highlight to save, write margin notes that sync with the text.
- **Word popover** (peek and open per the input-model table), answered from local data first:
  - surface form and normalized form (arp → arep, ng → nang, dr → dari)
  - language tag: Indonesian / Javanese / English loan / slang
  - root + affix breakdown (di- + bersih + -kan, + -nya)
  - part of speech, prebuilt glosses (§18), and the meaning *in this sentence*
  - Javanese speech level with its ngoko / krama / krama inggil equivalents
  - Arabic Bridge card (when relevant)
  - audio, "add to review", status buttons, 👍/👎
- **Explain this sentence:** grammar (assembled from the grammar library), register, culture, and a natural paraphrase.
- **Explanation voice:** Egyptian colloquial (بالمصري) / Modern Standard Arabic / English — my choice, per session.
- **Mac Quick Lookup:** a global hotkey opens a small floating window that analyzes whatever is in the clipboard (verify global-shortcut support in Dioxus desktop).

## 12. The Arabic Bridge — جسر

My unfair advantage: a large share of Indonesian vocabulary comes from Arabic (one study counted 832 words marked as Arabic loans in KBBI, the official dictionary, and most kept their meaning).

- **Bridge gold** underline + badge on Arabic-origin words; the card shows the Arabic word, its root, and whether the meaning shifted.
- **"You already know N words":** a quick-win deck built from Arabic loans.
- **Sound-Shift Detective** (mini-game): learn the regular sound changes, then guess the Arabic origin:
  - ف → p: pikir ← فكر, paham ← فهم, pasal ← فصل
  - خ → k: kabar ← خبر
  - ق → k: kalbu ← قلب
  - final ة → -at: selamat ← سلامة, lezat ← لذّة
- **False Friends Alarm:** loud warnings where the meaning moved — *kalimat* means "sentence", not «كلمة» "word"; *mungkin* means "maybe", from «ممكن» "possible".
- **Data:** extracted from Wiktionary etymologies and categories ("Indonesian terms borrowed from Arabic", "terms derived from the Arabic root …") via kaikki.org, then checked and enriched at build time (§18).

## 13. Unggah-Ungguh — Javanese respect levels

- Every Javanese word card shows its level (ngoko, krama, krama inggil, krama andhap) and its equivalents across levels.
- **Unggah-Ungguh Simulator** (game): a situation — a friend, a market seller, a friend's grandmother, a teacher, a village elder. I pick, type or write the right form; a **Respect Meter** reacts and explains why.
- Explain with Egyptian analogies: ngoko ≈ «إنت», krama ≈ «حضرتك», krama inggil ≈ elevating the other person like «سيادتك» — while making clear that Javanese goes much further, swapping whole vocabularies, not just pronouns.
- Javanese content is flagged **unverified** until I approve it; an export button lets me show lists to a native speaker or tutor.

## 14. Lessons, exercises & mini-games

**Lesson pattern** (Airlearn-style): 1–4 teaching slides → 6–10 exercises (review lessons 8–12) → mini dialogue (4–6 lines) → new-words summary (3–8 new words in a learn lesson) → souvenir. All of it is prebuilt content (§18).

**Lesson size:** every lesson fits in **7 minutes** (target 4–6), with at most 2 new grammar points. Bigger topics are split into more lessons, never squeezed. The evidence, the per-kind caps and the time model are in `docs/PEDAGOGY.md`; the caps live in `kabar_core::lesson::pacing` and the content validator rejects any lesson over them.

**Exercise types:** multiple choice · match pairs · translate both ways (free text or handwriting, graded against prebuilt reference answers + AI for open answers, with specific, kind feedback) · build a sentence from a word bank · listen and pick/type · fill the blank · dictation (typed or handwritten) · speaking (later).

**Mini-games:**

- **Pasar!** — haggle with a market seller under a budget: numbers, prices, polite refusals, Javanese market words (*piro*, *larang*, *murah*). Bonus round: haggle for the "iphone 17 pm" from the test texts.
- **Rupiah Reader** — speed drills reading prices aloud (Rp26.000.000 → *dua puluh enam juta rupiah*).
- **Minimal Pairs** (Javanese) — hear the difference between d / dh and t / th.
- **Gambar!**, **Card Wall** and **Listening Companion** — see the Tablet Studio (§4); simplified versions on the phone.
- **Kabar Pagi** — a 2-minute morning micro-story built from my due words plus a few new ones, with audio.
- **Sinetron** — a serialized story with an original recurring cast that grows with my level: Pak Budi's *warung* in Madiun and his daughter Sari, who studies in Cairo — so Java and Egypt meet. One short episode unlocks per finished unit.
- **Kabar Radio** — a daily 5-minute two-voice audio episode (dialogue + recap) from my words, downloadable for offline listening.

## 15. Conversation personas

Original characters, each with their own Azure neural voice:

| Persona | Who | Register | Voice |
|---|---|---|---|
| Mas Dimas | student in Yogyakarta | ngoko with friends, krama with elders | jv-ID-DimasNeural |
| Bu Siti | market seller in Solo | Javanese; mirrors my politeness | jv-ID-SitiNeural |
| Mbak Gadis | office colleague in Jakarta | colloquial Jakarta Indonesian with English mixed in | id-ID-GadisNeural |
| Mas Ardi | hotel receptionist in Bali | polite standard Indonesian | id-ID-ArdiNeural |

Each persona has a prebuilt style guide and example dialogues (§18). Level-aware vocabulary, register switch, inline corrections with explanations, every message hoverable (reuse the Reader engine), audio on every line, typed or handwritten replies, and a "replay as dialogue" button.

## 16. Review engine

- **FSRS** via the `fsrs` crate (the algorithm Anki uses), optimized on my own review log once there's enough data.
- Card types: recognition, production (typed or handwritten), audio → meaning, cloze from the sentence where I saved the word, Arabic Bridge cards.
- Three ways to review: classic cards, Card Wall (tablet), and quick 1-minute bursts (phone).
- **Stats:** heatmap, known words per language, retention, time studied, review forecast.

## 17. Language engine (`crates/core`)

- **Tokenizer:** reduplication (anak-anak, mlaku-mlaku), clitics (-nya, -ku, -mu, -lah, -kah, -pun; Javanese -e / -ne), numbers, emoji, punctuation, and Indonesian / Javanese / English mixed in one sentence.
- **Chat-slang normalizer:** yg → yang, gk / ga / gak → tidak / nggak, udh → sudah, dr → dari, arp → arep, ng → nang / neng, sek / sik, and more. Seeded from Wiktionary "text messaging" entries plus a curated list; unknown forms resolved by AI and queued for my review.
- **Language ID per token:** dictionary membership + marker words (Javanese: ora, opo, piye, arep, wis, durung, karo, kowe, sing · Indonesian: tidak, apa, bagaimana, mau, sudah, belum, dengan, kamu, yang) + sentence context, with AI fallback.
- **Morphology:** the `sastrawi-rs` crate (Indonesian and Javanese stemmers) for roots, plus our own affix decomposition for display — Indonesian meN- allomorphs, ber-, ter-, di-, ke-, peN-, per-, se-, -kan, -i, -an, confixes ke-an / peN-an / per-an / ber-an; Javanese nasal N-, di-, ka-, -ake / -ke, -i, -an, -e / -ne, -en.
- **Dictionary:** import the kaikki.org Wiktionary JSONL extracts for Indonesian and Javanese into SQLite (lemmas, POS, senses, forms, IPA, etymology, categories), merged with the prebuilt Arabic/English glosses and examples from the content pipeline. Wiktionary attribution (CC BY-SA) on the About screen.
- **Speech-level table:** ngoko ↔ krama ↔ krama inggil / krama andhap, built in the content pipeline (unverified until I review).
- **Caching:** every AI result stored by (task, input hash, engine, prompt version), shared through Kabar Cloud, reusable offline.
- **Speed:** popovers come from local data first; AI enriches them asynchronously.

## 18. Content pipeline (build time) — where the quality lives

Generated once by Claude Code (use the strongest model available to you), validated against JSON schemas, reviewed by me in batches, committed under `content/`, and shipped as versioned **content packs** (delta-synced through Kabar Cloud):

- **Frequency lists** for Indonesian and Javanese from openly licensed corpora (verify licenses; record them in ATTRIBUTION.md).
- **Glosses** for the top ~10,000 Indonesian and ~3,000 Javanese words: Egyptian Arabic, MSA and English meanings, part of speech, two example sentences each, register notes, false-friend warnings. Start with the top 2,000.
- **Grammar library** covering every grammar point in BIPA 1–7 and J1–J6, each explained three ways (Egyptian Arabic, MSA, English) with examples. "Explain this sentence" assembles answers from it.
- **The full curriculum** (§21): every unit's slides, exercises with reference answers, mini dialogues and placement tests.
- **Arabic Bridge dataset:** every Arabic-origin word found in Wiktionary, checked and enriched (Arabic word, root, meaning shift, sound-shift rule).
- **Javanese speech-level table**, flagged unverified until reviewed.
- **Story and coach assets:** souvenir cards, Sinetron episodes, Kabar Radio scripts for the first units, Kak Nur message templates, persona style guides and example dialogues.
- **Golden test sets** for every runtime AI task (§19), with reference answers.

Scripts must be resumable, batched and idempotent, paced across multiple sessions to stay within my Claude plan's limits.

## 19. AI router & quality system (runtime)

**Deterministic first:** tokenizer, normalizer, stemmer, prebuilt dictionary, grammar library and reference answers handle everything they can before any model is called.

**Engines** — one trait, routed per task, each with an on/off switch:

- `local` — Ollama on Kabar Home running an open Indonesian/Javanese model. Start with Sahabat-AI 8–9B GGUF; benchmark alternatives such as SEA-LION; choose the quantization that fits my Mac's RAM.
- `gemini_free` — Google Gemini API free tier (Flash / Flash-Lite), called through Kabar Cloud, with queueing and backoff for rate limits. Free-tier inputs may be used by Google to improve its models, so only learning text is ever sent — never personal data.
- `claude` — Anthropic API, off unless I enable it. If I'm on a Claude Max plan, link its monthly API credits and let the router use Claude where it clearly wins, at no extra cost.

**Tasks:** `analyze_text` (contextual meaning + alignment) · `explain_sentence` (picks grammar-library entries + short glue text) · `grade_answer` · `roleplay_turn` · `placement_test` · `level_check` · `daily_plan` · `daily_recap` · `weekly_report` · `coach_message` · `story_episode` · `radio_script` · `structure_screenshot` · `read_handwriting` · `guess_sketch` · `check_strokes` · `mind_map` · `learn_entry` (§26).

**Quality system:**

- Every task has a rubric and a golden set (§18). The **engine bake-off** (M0, re-run whenever a model or prompt changes) scores each engine per task on quality, speed and cost, using Claude Code as the judge plus my spot checks. The router uses the cheapest engine that passes the bar for that task.
- If no free engine passes a task: (1) improve prompts and few-shot examples, (2) move more of the task to build time, (3) only then route it to `claude` if enabled. Report every gap to me — never silently degrade.
- Javanese answers from free engines are cross-checked against the dictionary and speech-level table (and a second engine when cheap); low confidence shows an "unverified" badge.
- 👍/👎 on every AI answer: 👎 regenerates with the next engine up, replaces the cached answer, and adds the case to the golden set.
- Strict JSON validated with serde; retry once with the error; then a friendly message.
- Versioned prompt templates in `prompts/`, each with the learner profile (Egyptian Arabic native, reads English, current levels, known-word count, goals, recent struggles from coach memory).
- **Budget:** $0 by default with a hard stop. The cost dashboard shows $0 and warns before anything would cost money.

## 20. Audio & speech

- **Azure Speech** neural voices (id-ID Gadis / Ardi, jv-ID Siti / Dimas) on the free tier (F0: 0.5M neural characters per month). Pre-generate just in time — the current and next unit — inside the monthly quota; cache forever on the server and devices; downloadable offline packs per unit. SSML for two-voice dialogues (Kabar Radio).
- Prefer the pure-Rust `azure-speech` crate if maintained; otherwise the REST API.
- If the quota runs out: cached audio first, then Android system TTS / macOS voices for Indonesian; new Javanese audio waits for the monthly reset.
- **Speech recognition** (later): Whisper on Kabar Home (whisper.cpp / whisper-rs); Azure free-tier hours as fallback; check pronunciation-assessment support for id-ID.

## 21. Curriculum

- **Indonesian = BIPA 1–7**, the official program for foreign learners (Permendikbud No. 27/2017; competency standards adapted from CEFR). Research the official competency elements per level, the topic sequence of the government *Sahabatku Indonesia* textbook series, and the CEFR mapping (don't assume it). Output `curriculum/indonesian.yaml`: level → units → lessons, each with can-do objectives, grammar points, vocabulary themes, culture notes and estimated minutes.
- **Javanese = J1–J6**, unlocking once BIPA 1 is done, then running in parallel:
  - J1 Surabaya — survival ngoko and chat Javanese, with a taste of the Arek dialect (gak, koen, rek)
  - J2 Madiun — everyday ngoko in Mataraman style: family, food, market, time, numbers
  - J3 Solo — krama basics: when and with whom
  - J4 Yogyakarta — krama inggil, krama andhap, unggah-ungguh etiquette
  - J5 Semarang — reading real texts (Javanese Wikipedia, news)
  - J6 Kotagede — aksara Jawa (Javanese script)

  Explain Javanese by comparing it with Indonesian and through Arabic. Output `curriculum/javanese.yaml`.
- If I share screenshots of Airlearn's course map, mirror its structure and pacing (ordering, slide → practice flow) in our YAML; write all lesson content fresh.
- Lesson content comes from the content pipeline (§18), cached and editable by me. Weak words and grammar points from my reviews and Kak Nur's notes get pulled into upcoming lessons.

## 22. Data model (starting point)

Refine in PLAN.md. Every synced table carries `account_id`, `device_id`, a hybrid-logical-clock timestamp and a tombstone flag.

- **Language:** lexemes, senses, glosses, examples, forms, etymology_arabic, abbreviations, speech_levels, grammar_points, taught_entries (my personal dictionary, §26)
- **Reading:** texts, text_tokens, word_status, highlights
- **Learning:** cards, review_log, curriculum_nodes, lessons, exercises, attempts, conversations, stories, souvenirs
- **Ink:** notebooks, ink_documents, ink_strokes
- **Coach:** goals, daily_plans, reminders, reports, journal_entries, coach_memory, skill_scores
- **Account:** accounts, devices, sessions (for Lanjut), push_tokens, sync_events
- **Quality:** engine_runs, quality_feedback, golden_items
- **System:** content_packs, ai_cache, audio_cache, quota_usage, settings, cost_log

## 23. Milestones

| # | Milestone | Done when |
|---|---|---|
| M0 | Research → `docs/PLAN.md`, free-tier verification, engine bake-off | I approve the plan and the engine choices |
| M1 | Workspace, toolchain scripts, Dioxus app on Mac + S24 Ultra + Tab S11, i18n + RTL, themes, adaptive layouts, SQLite, **Pen Lab** | the same app opens on all three devices and Pen Lab reports real pen data |
| M2 | Kabar Cloud v0 on Cloudflare's free plan: Google sign-in, Devices screen, live sync, key vault, AI/TTS proxy | a word saved on the phone appears on the tablet within seconds |
| M3 | Dictionary import + language engine + tests | the test texts tokenize, normalize and stem correctly |
| M4 | Content pipeline v1: frequency lists, top-2,000 glosses, grammar library for BIPA 1–2, golden sets | I review and approve the first content pack |
| M5 | Reader + Screenshot Decoder + Share-to-Kabar + pen hover, lasso and highlight | I share a WhatsApp screenshot and read it with popovers |
| M6 | Review (FSRS) + handwritten answers + Card Wall + stats | a week of reviews works across devices |
| M7 | Kak Nur: onboarding, daily plan, account-aware reminders (FCM), recap, weekly report | reminders stop on all devices once I've studied on one |
| M8 | Journey Map + Lesson Player + BIPA 1–2 + placement + Lanjut handoff | I start a lesson on the phone and finish it on the tablet |
| M9 | Tablet Studio: Notebook, Gambar!, Listening Companion, mind maps, write-mode lessons | a notebook page written on the tablet is searchable on the Mac |
| M10 | Arabic Bridge games, Kabar Pagi, Rupiah Reader, Pasar! | the morning story arrives with audio |
| M11 | Javanese J1–J3, Unggah-Ungguh Simulator, Minimal Pairs | I pass J1 |
| M12 | Personas, Sinetron, Kabar Radio | a 5-minute episode plays offline on the phone |
| M5+ | **Teach Kabar** (§26), phase T1: `learn_entry`, review screen, personal dictionary, popover entry point | I teach Kabar a word it didn't know, approve it, and see it in a popover on another device |
| M13 | Content pipeline v2 (all levels, 10k + 3k glosses) + polish: monthly level check, skill radar + forecast, aksara Jawa calligraphy, widgets, Mac Quick Lookup, offline packs, performance | everything in §25 passes |

## 24. Test texts

Use these in unit tests, the golden sets and demos.

Javanese chat (a random WhatsApp screenshot from the internet):

- "Sek tak lapor pusat" → hold on, I'll report to HQ (sek = hold on, tak = I'll, lapor = report, pusat = center / HQ)
- "ng prabowo?" → to Prabowo? (ng = nang = to; Prabowo is a proper noun)
- "piye dhe" → so, what do you think? (piye = how; dhe = friendly address, likely short for Pakdhe / Budhe)
- "arp dicul piro" → how much will you sell it for? (arp = arep; dicul = diculke, let go / sell; piro = pira, how much)

Indonesian marketplace listing:

- "iphone 17 pm 512gb resmi ibox mulus nomin garansi jalan minat wa" (pm = Pro Max · resmi ibox = official iBox unit · mulus = flawless · nomin = no minus, no defects · garansi jalan = warranty active · minat wa = interested? WhatsApp me)

Indonesian morphology: "Saya sedang membaca buku." · "Rumahnya dibersihkan kemarin." · "Anak-anak bermain di taman." · "Kesehatan itu penting." · "Apa kabar?"

Javanese speech levels — "Have you eaten?": ngoko "Kowe wis mangan?" · krama "Sampeyan sampun nedha?" · krama inggil "Panjenengan sampun dhahar?"

Dialect and code-mixing: Surabaya Arek "Koen wis mangan ta, rek?" · Jakarta slang "Which is, gue literally capek banget."

Arabic Bridge: kabar, waktu, selamat, lezat, kursi, ilmu, pikir, paham, kalbu, kalimat (false friend), mungkin (meaning shift).

## 25. Quality bar & definition of done

- Unit tests for tokenizer, normalizer, morphology, language ID, FSRS integration and sync merge rules; AI parsing tested against recorded JSON fixtures (no live API calls in tests).
- Every runtime AI task passes its golden set with the engine the router picked; the bake-off report is in `docs/`.
- Monthly running cost is $0 with all paid engines off; the cost dashboard proves it.
- Popover from local data in under 100 ms on the S24 Ultra; ink that feels attached to the pen tip on both Galaxy devices (measure it in Pen Lab); smooth scrolling on the Tab S11; no jank on the Mac.
- Sync: an edit shows up on my other online devices within about 5 seconds; Lanjut resumes the exact exercise.
- Reminders never fire after the daily goal is met on any device.
- Every user-facing message passes the §2 tone rules: specific, warm, no guilt.
- Only AI, TTS, auth, push and sync traffic leaves my devices, and nothing personal goes to free-tier AI engines.
- ATTRIBUTION.md lists every dataset, corpus, font and sound.
- Accessibility: scalable text, screen-reader labels, keyboard navigation on Mac and DeX.
- **Kabar is "done" when** I sign in on all three devices, they stay in sync, Kak Nur keeps me on track every day, I can finish a BIPA unit with the pen on the tablet, and I can share a random Javanese screenshot from my phone and understand every word — in Arabic — for $0 a month.

## 26. Teach Kabar — كبر بيتعلّم منّي

Kabar will meet words and phrases it doesn't know: new slang, a Javanese word from a chat, a brand-new meme. I want to teach it, the way I'd explain a word to a friend, and have it remember.

**Flow:**

1. In a word popover, "Explain this sentence" or anywhere a word is unknown, I tap **Teach Kabar** (علّم كبر). The word, its language and the sentence I met it in are filled in.
2. I give Kabar what I found:
   - **paste** an explanation (from Google, a dictionary, a friend's message),
   - a **link** (Kabar reads the page text when it can; otherwise it asks me to paste the part that matters),
   - a **screenshot** (read with on-device OCR, §11),
   - or an optional **in-app lookup** that only uses free sources: open a Google search for "‹word› artinya" in the browser, and Wiktionary's free definition API. No paid search API.
3. Kabar rewrites what I gave it into its **own gloss format and style** (the same shape as the prebuilt glosses, §18): Egyptian Arabic, MSA and English meanings; part of speech and register; root and affixes (checked against the language engine, §17); two example sentences; a usage note; the Arabic Bridge card and False Friends warning when the word comes from Arabic (§12); Javanese speech level and its ngoko / krama / krama inggil pair when relevant (§13). It says plainly where my source was unclear or where it disagrees with it.
4. I **review** the draft: approve it, edit any field, ask for another version (👎 → next engine up, §19), or throw it away. Nothing is final until I approve it.
5. Approved entries go into my **personal dictionary**: a layer above the prebuilt glosses that syncs to all my devices (LWW per word), shows in popovers with a **"taught by you"** badge, can be added to review (§16) and feeds the next content batch (§18) so the build-time content keeps growing. Drafts don't show as answers in popovers, only as a "you have a draft — review it" chip.

**Rules:**

- AI goes through the router (§19) as task `learn_entry`, with a versioned prompt, serde validation (retry once, then a friendly message) and a cache keyed on the source text.
- Privacy (§19): what I paste from the web and the sentence are learning text. Before anything is sent, e-mail addresses and phone numbers are scrubbed. Screenshots (which can show chats, names, numbers) are treated as **personal**: never sent to an engine that trains on inputs.
- Kabar never overwrites a prebuilt gloss with mine; both stay, mine shown first. Javanese entries stay flagged **unverified** until I approve them, like all Javanese content.
- My sources (link, title, the excerpt used) are kept with the entry so I can check them later.

**Phases:** T1 (with M5): the task, the review screen, the personal dictionary, sync, the popover entry point, paste / link (when the page allows it) / screenshot via OCR, Google + Wiktionary lookup. T2 (M13): links fetched through Kabar Cloud when the page blocks the app, screenshots without OCR text via a vision engine, "taught by you" deck in Review, taught entries exported into the next content batch.

Start with Milestone 0.
