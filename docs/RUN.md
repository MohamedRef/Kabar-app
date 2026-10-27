# Run Batna

## Mac (desktop)
1. Install Rust (`curl https://sh.rustup.rs -sSf | sh`) and `just` (`brew install just`).
2. In the repo, switch to the Batna branch: `git fetch origin batna && git switch batna`.
3. `just run` opens the app. `BATNA_WINDOW_SIZE=412x900 just run` previews the phone layout. `BATNA_OPEN_SCREEN=study just run` opens a screen directly. `BATNA_OPEN_SETTINGS=look just run` opens Settings on a section (`look`, `data`, `about`; empty for the list). Theme and text size are kept per device in `device-prefs.txt` in the data folder (Settings › Your data shows where).

## S24 Ultra and Tab S11 (Android)
1. One-time setup: install the Dioxus CLI 0.7.10 (`cargo install dioxus-cli --version 0.7.10 --locked`), Java 17, and Android Studio's SDK plus NDK r27c. Then run `rustup target add aarch64-linux-android`.
2. Turn on USB debugging on the phone or tablet (Settings › About phone › Software information › tap Build number 7 times, then Developer options › USB debugging), and connect it with a cable.
3. `just android` builds and installs a debug build. The app id is `app.batna`, so it sits next to Kabar without touching it.

A signed release APK will come with the release-prep work. Until then, the debug build is the way to try it.

## Study in Anki now
One file, `/mnt/project-files/medstudy/anki/batna.apkg`, holds every written lecture as a subdeck of `Batna`. Open it with AnkiDroid on the phone or tablet, or with Anki on the Mac. Importing a newer `batna.apkg` updates the cards in place and keeps your progress. `just anki` builds it into `dist/anki/`; details in [ANKI.md](ANKI.md).
