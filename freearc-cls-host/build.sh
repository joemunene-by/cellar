#!/usr/bin/env bash
# Build the CLS plugin host (cellar-freearc-cls-host.exe) for i686
# Windows. The closed-source cls-*.dll plugins FitGirl uses are
# 32-bit, so the loader process must be 32-bit too.
#
# Preferred toolchain: cargo-zigbuild (uses Zig's linker, which
# resolves the unwinder symbols mingw-w64 14.x splits across
# libgcc_eh / libunwind in a way rustc cannot link by default).
# Fallback: mingw-w64 + plain cargo (may fail on mingw 14.x).
#
# One-time setup for the preferred path:
#   macOS:  brew install zig
#           cargo install cargo-zigbuild
#   Linux:  install zig from https://ziglang.org/download/
#           cargo install cargo-zigbuild
#   both:   rustup target add i686-pc-windows-gnu

set -euo pipefail
cd "$(dirname "$0")"

if ! rustup target list --installed | grep -q '^i686-pc-windows-gnu$'; then
    echo "installing rust target i686-pc-windows-gnu..."
    rustup target add i686-pc-windows-gnu
fi

OUT="target/i686-pc-windows-gnu/release/cellar-freearc-cls-host.exe"

if command -v cargo-zigbuild >/dev/null 2>&1; then
    echo "using cargo-zigbuild (preferred)"
    cargo zigbuild --release --target i686-pc-windows-gnu
elif command -v i686-w64-mingw32-gcc >/dev/null 2>&1; then
    echo "using mingw-w64 (fallback; may fail on mingw 14.x due to"
    echo "libgcc_eh / libunwind split — install cargo-zigbuild if it"
    echo "errors out:  brew install zig && cargo install cargo-zigbuild)"
    cargo build --release --target i686-pc-windows-gnu
else
    echo "missing cross-compile toolchain. install one of:"
    echo ""
    echo "  preferred:  brew install zig && cargo install cargo-zigbuild"
    echo "  fallback :  brew install mingw-w64  (mingw 14.x is buggy with rustc)"
    echo ""
    exit 1
fi

if [ -f "$OUT" ]; then
    echo
    echo "built: $OUT"
    ls -lh "$OUT"
    file "$OUT" 2>/dev/null || true
fi
