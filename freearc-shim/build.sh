#!/usr/bin/env bash
# Build cellar-freearc.exe for i686-pc-windows-gnu, which is the only
# ABI that can load FitGirl's 32-bit unarc.dll.
#
# Prereqs (one-time):
#   macOS:  brew install mingw-w64
#   Linux:  sudo apt install mingw-w64
#   both:   rustup target add i686-pc-windows-gnu
set -euo pipefail

cd "$(dirname "$0")"

if ! rustup target list --installed | grep -q '^i686-pc-windows-gnu$'; then
    echo "missing rust target i686-pc-windows-gnu, installing..."
    rustup target add i686-pc-windows-gnu
fi

if ! command -v i686-w64-mingw32-gcc >/dev/null 2>&1; then
    echo "missing mingw cross-compiler (i686-w64-mingw32-gcc)."
    echo "install via:"
    echo "  macOS:  brew install mingw-w64"
    echo "  Linux:  sudo apt install mingw-w64"
    exit 1
fi

cargo build --release --target i686-pc-windows-gnu

OUT="target/i686-pc-windows-gnu/release/cellar-freearc.exe"
if [ -f "$OUT" ]; then
    echo
    echo "built: $OUT"
    ls -lh "$OUT"
    file "$OUT" 2>/dev/null || true
fi
