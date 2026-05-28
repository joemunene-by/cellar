#!/usr/bin/env bash
# Smoke-test the native FreeArc reader against real archives.
#
# Builds freearc-native release binaries, then runs:
#   - fg-arc-ls    on every archive (lists footer + control blocks)
#   - fg-arc-files on every archive (lists files inside)
# It does NOT run fg-arc-x: extraction can be slow on big archives
# and is gated on the codecs the archive actually uses.
#
# Files are pre-filtered with a cheap last-4-KiB magic-byte check
# for "ArC\x01" so the script does not flood with output when the
# input dir contains hundreds of unrelated .bin game-asset files
# (Unity, Need for Speed, CarX Street, etc. all use the .bin
# extension for non-FreeArc data).
#
# Usage:
#   scripts/freearc-smoke.sh path/to/dir
#   scripts/freearc-smoke.sh fg-01.bin fg-02.bin
#   scripts/freearc-smoke.sh    # auto-discover under ~/Downloads
#
# Default discovery matches fg-*.bin (FitGirl convention) and *.arc.
# Paths passed as args are accepted regardless of name; they still
# go through the magic-byte pre-check.

set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here/freearc-native"

echo "=== build ==="
cargo build --release --quiet
LS="$here/freearc-native/target/release/fg-arc-ls"
FILES="$here/freearc-native/target/release/fg-arc-files"

# Cheap magic-byte check: read the last 4 KiB, look for ArC\x01.
# Returns 0 if the file is plausibly a FreeArc archive, non-zero
# otherwise.
is_freearc() {
  local f="$1"
  local size
  size="$(stat -f%z "$f" 2>/dev/null || stat -c%s "$f" 2>/dev/null || echo 0)"
  if [ "$size" -lt 4 ]; then return 1; fi
  local skip=0
  if [ "$size" -gt 4096 ]; then skip=$((size - 4096)); fi
  dd if="$f" bs=1 skip="$skip" count=4096 2>/dev/null | \
    grep -q -a $'ArC\x01'
}

inputs=()
if [ $# -gt 0 ]; then
  for arg in "$@"; do
    if [ -d "$arg" ]; then
      while IFS= read -r -d '' f; do
        inputs+=("$f")
      done < <(find "$arg" -type f \( -iname 'fg-*.bin' -o -iname '*.arc' \) -print0 | sort -z)
    else
      inputs+=("$arg")
    fi
  done
else
  echo "no arg given; scanning ~/Downloads for fg-*.bin / *.arc..."
  while IFS= read -r -d '' f; do
    inputs+=("$f")
  done < <(find "$HOME/Downloads" -type f \( -iname 'fg-*.bin' -o -iname '*.arc' \) -print0 2>/dev/null | sort -z)
fi

if [ ${#inputs[@]} -eq 0 ]; then
  echo "no archives found. pass paths as args."
  exit 0
fi

echo
echo "=== ${#inputs[@]} candidate(s); checking signatures ==="
real_archives=()
skipped=0
for arc in "${inputs[@]}"; do
  if is_freearc "$arc"; then
    real_archives+=("$arc")
  else
    skipped=$((skipped + 1))
  fi
done

if [ "$skipped" -gt 0 ]; then
  echo "$skipped of ${#inputs[@]} candidate(s) lacked the ArC\\x01 signature (not FreeArc)"
fi

if [ ${#real_archives[@]} -eq 0 ]; then
  echo
  echo "no real FreeArc archives in the scan path."
  exit 0
fi

echo
echo "=== ${#real_archives[@]} real archive(s) ==="
for arc in "${real_archives[@]}"; do
  echo
  echo "================================================================"
  echo "archive: $arc"
  echo "size: $(stat -f%z "$arc" 2>/dev/null || stat -c%s "$arc") bytes"
  echo "================================================================"

  echo "--- fg-arc-ls ---"
  if ! "$LS" "$arc"; then
    echo "(fg-arc-ls failed)"
    continue
  fi

  echo
  echo "--- fg-arc-files (summary only) ---"
  if ! "$FILES" "$arc" 2>&1 | tail -5; then
    echo "(fg-arc-files failed)"
  fi
done

echo
echo "=== done ==="
