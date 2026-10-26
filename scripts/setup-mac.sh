#!/usr/bin/env bash
# One-time setup of the Kabar toolchain on an Apple Silicon Mac.
# Safe to re-run: every step checks before installing.
set -euo pipefail

say() { printf '\n\033[1;34m==> %s\033[0m\n' "$*"; }

NDK_VERSION="27.2.12479018"   # NDK r27c (LTS)
PLATFORM="android-35"
BUILD_TOOLS="35.0.0"

say "Xcode command line tools"
xcode-select -p >/dev/null 2>&1 || xcode-select --install

say "Homebrew"
command -v brew >/dev/null || { echo "Install Homebrew first: https://brew.sh"; exit 1; }

say "Rust"
command -v rustup >/dev/null || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
# shellcheck disable=SC1091
source "$HOME/.cargo/env"
rustup toolchain install stable
rustup target add aarch64-apple-darwin aarch64-linux-android wasm32-unknown-unknown

say "Dioxus CLI (dx) 0.7.10"
if ! dx --version 2>/dev/null | grep -q "0.7.10"; then
  cargo install dioxus-cli --version 0.7.10 --locked
fi

say "just, Java 17, Android command-line tools"
brew install just openjdk@17
brew install --cask android-commandlinetools

JAVA_HOME="$(brew --prefix openjdk@17)/libexec/openjdk.jdk/Contents/Home"
ANDROID_HOME="$(brew --prefix)/share/android-commandlinetools"
export JAVA_HOME ANDROID_HOME

say "Android SDK packages (accept the licenses when asked)"
yes | sdkmanager --licenses >/dev/null || true
sdkmanager "platform-tools" "platforms;${PLATFORM}" "build-tools;${BUILD_TOOLS}" "ndk;${NDK_VERSION}"

ENV_FILE="$HOME/.kabar/env.sh"
mkdir -p "$HOME/.kabar"
cat > "$ENV_FILE" <<ENV
# Kabar toolchain (written by scripts/setup-mac.sh)
export JAVA_HOME="$JAVA_HOME"
export ANDROID_HOME="$ANDROID_HOME"
export ANDROID_NDK_HOME="\$ANDROID_HOME/ndk/${NDK_VERSION}"
export NDK_HOME="\$ANDROID_NDK_HOME"
export PATH="\$ANDROID_HOME/platform-tools:\$ANDROID_HOME/build-tools/${BUILD_TOOLS}:\$JAVA_HOME/bin:\$PATH"
ENV

if ! grep -q ".kabar/env.sh" "$HOME/.zprofile" 2>/dev/null; then
  echo '[ -f "$HOME/.kabar/env.sh" ] && source "$HOME/.kabar/env.sh"' >> "$HOME/.zprofile"
fi

say "Done. Open a new terminal (or run: source ~/.kabar/env.sh), then:"
echo "  just check            # fmt + clippy + tests"
echo "  just run-mac          # desktop app"
echo "  just android-keystore # once, before the first APK"
echo "  just apk && just install"
