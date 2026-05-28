#!/usr/bin/env bash
# Build (if needed) + launch cellar.app on macOS.
#
# Use this for a quick "does the GUI come up?" check from a local
# Mac Terminal (Aqua session). cargo tauri dev gives you live
# reload during development; this script is for verifying the
# release build of the binary works.
#
# Re-runs are fast — cargo skips work that is already up to date,
# vite only rebuilds when src/ changed.

set -euo pipefail
cd "$(dirname "$0")/.."

# Make sure npm + cargo-tauri are reachable. Non-interactive shells
# (e.g. via ssh) do not source .zshrc, so PATH may be missing them.
export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"

if ! command -v npm >/dev/null 2>&1; then
  echo "npm not on PATH; install Node 18+ (e.g. brew install node)"
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not on PATH; install Rust (https://rustup.rs)"
  exit 1
fi
if ! command -v cargo-tauri >/dev/null 2>&1; then
  echo "cargo-tauri not installed; installing..."
  cargo install tauri-cli --version "^2.0"
fi

bin="src-tauri/target/release/cellar"
build_needed=0
if [ ! -x "$bin" ]; then
  build_needed=1
else
  # Rebuild when source has changed since last build.
  if [ -n "$(find src src-tauri/src freearc-native/src -type f -newer "$bin" 2>/dev/null | head -n 1)" ]; then
    build_needed=1
  fi
fi

if [ "$build_needed" = "1" ]; then
  echo "building frontend + cellar..."
  npm install --no-audit --no-fund --silent
  npm run build
  cargo tauri build --no-bundle
fi

echo
echo "launching $bin"
echo "(close the window to exit; ctrl-c here also kills it)"
exec "$bin"
