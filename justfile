# Kabar task runner. Install `just` first: `brew install just`.
# Run `just` to list recipes.

set shell := ["bash", "-euo", "pipefail", "-c"]
# `.env` (git-ignored, see .env.example): Kabar Cloud URL and Google client for the app build.
set dotenv-load := true

app := "crates/app"
dist := "dist"
keystore := env_var("HOME") / ".kabar/kabar-release.jks"
key_alias := "kabar"

default:
    @just --list

# --- quality gates -----------------------------------------------------------

# fmt + clippy (zero warnings) + tests: run before every commit
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

fmt:
    cargo fmt --all

# --- content pipeline (docs/CONTENT.md) ---------------------------------------

tools := "cargo run -q --release -p kabar-tools --"

# Download corpora/dictionaries into data/raw/: `just content-fetch` (freq), `nusa`, `kaikki`, `jvwiki` or `all`
content-fetch what="freq":
    scripts/fetch-sources.sh {{what}}

# Rebuild the Indonesian frequency list (needs `just content-fetch`)
content-freq:
    {{tools}} freq id

# Import Wiktionary (needs `just content-fetch kaikki`), then cross-check the Arabic Bridge
content-kaikki:
    {{tools}} kaikki --lang id
    {{tools}} kaikki --lang jv
    {{tools}} bridge crosscheck

# Javanese frequency list (needs `just content-fetch nusa`; adds Javanese Wikipedia after `just content-fetch jvwiki`)
content-freq-jv:
    {{tools}} freq jv

# Progress of the gloss batches
content-status:
    {{tools}} gloss status

# Engine bake-off (docs/BAKEOFF.md): every golden item to every free engine. Resumable.
bakeoff engines="workers_ai,gemini_free,local=aisingapore/Gemma-SEA-LION-v4-4B-VL" *args="":
    {{tools}} bakeoff run --engines {{engines}} {{args}}

# A few minutes: the first 3 items of each set, to check keys and models
bakeoff-smoke engines="workers_ai,gemini_free,local=aisingapore/Gemma-SEA-LION-v4-4B-VL":
    {{tools}} bakeoff run --engines {{engines}} --limit 3 --out bakeoff/runs/smoke

# Answers with engine names hidden, for blind judging
bakeoff-blind run:
    {{tools}} bakeoff blind {{run}}

# Scores → report.md and a proposed routes.toml (add --write to replace the committed ones)
bakeoff-report run *args="":
    {{tools}} bakeoff report {{run}} {{args}}

# Validate every content file (schemas, glosses, Bridge, speech levels, golden sets, pack manifests); CI runs the same checks
content-check:
    {{tools}} validate

# Refresh the generated numbers in README.md (CI fails when they are stale)
readme:
    {{tools}} readme

# Markdown review sheet for one gloss batch: `just content-review 3`
content-review batch:
    {{tools}} gloss review {{batch}}

# --- Mac ---------------------------------------------------------------------

# One-time toolchain setup on the MacBook (Rust targets, dx, Java, Android SDK/NDK)
setup-mac:
    scripts/setup-mac.sh

# Run the desktop app with hot reload
run-mac:
    cd {{app}} && dx serve --platform desktop

# Preview the phone layout in a 412x915 window on the Mac
run-phone-preview:
    KABAR_WINDOW_SIZE=412x915 cargo run -p kabar-app

# Build a release .app for the Mac into dist/
bundle-mac:
    cd {{app}} && dx bundle --platform macos --release --package-types macos --out-dir ../../{{dist}}/mac

# --- Android -----------------------------------------------------------------

# One-time: create the release signing key (kept in ~/.kabar, password in Keychain)
android-keystore:
    scripts/android-keystore.sh "{{keystore}}" "{{key_alias}}"

# Live-reload the app on a connected Galaxy device (USB or wireless)
run-android device="":
    cd {{app}} && dx serve --platform android {{ if device != "" { "--device " + device } else { "" } }}

# Build a signed release APK into dist/android/kabar.apk
apk:
    rm -rf {{dist}}/android && mkdir -p {{dist}}/android
    cd {{app}} && dx bundle --platform android --release --package-types apk --target aarch64-linux-android --no-default-features --features mobile --out-dir ../../{{dist}}/android/raw
    # dx leaves a debug build; rebuild it as a release (no debuggable, R8, versionCode).
    apk="$(scripts/android-release-apk.sh)" && \
      scripts/android-check-apk.sh "$apk" && \
      scripts/android-sign.sh "$apk" "{{dist}}/android/kabar.apk" "{{keystore}}" "{{key_alias}}"

# A release for your own devices (docs/RELEASE.md, only once everything is
# done and the learner says go): every check, then
# Kabar.app and the signed APK side by side in dist/release/<version>/
release:
    just check
    cargo run -q -p kabar-tools -- validate
    just bundle-mac
    just apk
    v="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"; \
      out="{{dist}}/release/$v"; rm -rf "$out" && mkdir -p "$out"; \
      cp -R "$(find {{dist}}/mac -maxdepth 4 -name '*.app' | head -1)" "$out/Kabar.app"; \
      cp {{dist}}/android/kabar.apk "$out/kabar-$v.apk"; \
      echo "Kabar $v → $out (Kabar.app, kabar-$v.apk). Next: docs/RELEASE.md §4."

# Share a test text to Kabar on a device (M5): `just share-text "arp dicul piro"`
share-text text serial="":
    adb {{ if serial != "" { "-s " + serial } else { "" } }} shell am start -a android.intent.action.SEND -t text/plain --es android.intent.extra.TEXT "'{{text}}'" -n app.kabar/app.kabar.ShareActivity

# List connected Android devices
devices:
    adb devices -l

# Install the signed APK on a device: `just install` (only one connected) or `just install <serial>`
install serial="":
    adb {{ if serial != "" { "-s " + serial } else { "" } }} install -r {{dist}}/android/kabar.apk

# Wireless debugging, step 1 (once per device): `just pair 192.168.1.20:37099 123456`
pair addr code:
    adb pair {{addr}} {{code}}

# Wireless debugging, step 2 (each session): `just connect 192.168.1.20:41235`
connect addr:
    adb connect {{addr}}

# Stream Kabar's logs from a device (the app must be running)
logs serial="":
    adb {{ if serial != "" { "-s " + serial } else { "" } }} logcat -v color --pid="$(adb {{ if serial != "" { "-s " + serial } else { "" } }} shell pidof -s app.kabar)"

# --- Kabar Cloud (Cloudflare Workers Free) -----------------------------------

server := "crates/server"

# One-time: wrangler (Cloudflare CLI) and worker-build
cloud-tools:
    npm install -g wrangler@4
    cargo install worker-build --version ^0.8 --locked

# One-time, after `wrangler login`: D1 database, signing key, allowlist
cloud-setup:
    scripts/cloud-setup.sh

# Run Kabar Cloud locally on http://127.0.0.1:8787 (local workerd; AI calls need a login)
cloud-dev:
    cd {{server}} && wrangler d1 migrations apply kabar --local && wrangler dev

# End-to-end test on a local workerd with a fake Google: phone → tablet sync, devices, sign-out
cloud-e2e:
    node {{server}}/tests/e2e.mjs

# Deploy to <name>.<your-subdomain>.workers.dev
deploy-cloud:
    cd {{server}} && wrangler d1 migrations apply kabar --remote && wrangler deploy

# Stream live logs from the deployed Worker
cloud-logs:
    cd {{server}} && wrangler tail

# --- Offline packs (docs/RUN.md §11) -------------------------------------------

# Rebuild the unit manifests (content/packs/) after lesson edits; prints TTS characters per unit
packs:
    {{tools}} packs build

# Generate missing audio with Azure F0 inside the monthly budget: `just packs-audio` (all) or `just packs-audio bipa1.u01,bipa1.u02`
packs-audio units="" budget="300000":
    {{tools}} packs synthesize --budget {{budget}} {{ if units != "" { "--units " + units } else { "" } }}

# What `packs-audio` would spend, without calling Azure
packs-audio-plan units="" budget="300000":
    {{tools}} packs synthesize --dry-run --budget {{budget}} {{ if units != "" { "--units " + units } else { "" } }}

# Write dist/packs/<unit>.tar + index.json for every fully synthesized unit
packs-bundle:
    {{tools}} packs bundle

# Upload dist/packs to the `packs` GitHub release (needs `gh auth login`)
packs-upload tag="packs":
    scripts/packs-upload.sh {{tag}}
