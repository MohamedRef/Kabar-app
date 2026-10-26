# Kabar task runner. Install `just` first: `brew install just`.
# Run `just` to list recipes.

set shell := ["bash", "-euo", "pipefail", "-c"]

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
    cd {{app}} && dx bundle --platform android --release --package-types apk --out-dir ../../{{dist}}/android/raw
    scripts/android-sign.sh "$(find {{dist}}/android/raw -name '*.apk' | head -1)" "{{dist}}/android/kabar.apk" "{{keystore}}" "{{key_alias}}"

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
