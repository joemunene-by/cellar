#!/usr/bin/env bash
# Build the CLS plugin host (cellar-freearc-cls-host.exe) for i686
# Windows. The closed-source cls-*.dll plugins FitGirl uses are
# 32-bit, so the loader process must be 32-bit too.
#
# Prereqs (one-time):
#   macOS:  brew install mingw-w64
#   Linux:  sudo apt install mingw-w64
#   both:   rustup target add i686-pc-windows-gnu

set -euo pipefail
cd "$(dirname "$0")"

if ! rustup target list --installed | grep -q '^i686-pc-windows-gnu$'; then
    echo "installing rust target i686-pc-windows-gnu..."
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

OUT="target/i686-pc-windows-gnu/release/cellar-freearc-cls-host.exe"
if [ -f "$OUT" ]; then
    echo
    echo "built: $OUT"
    ls -lh "$OUT"
    file "$OUT" 2>/dev/null || true
fi
