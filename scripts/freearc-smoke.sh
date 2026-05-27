#!/usr/bin/env bash
# Smoke-test the native FreeArc reader against real archives.
#
# Builds freearc-native release binaries, then runs:
#   - fg-arc-ls    on every archive (lists footer + control blocks)
#   - fg-arc-files on every archive (lists files inside)
# It does NOT run fg-arc-x: extraction can be slow on big archives
# and is gated on the codecs the archive actually uses.
#
# Usage:
#   scripts/freearc-smoke.sh path/to/dir-of-fitgirl-bins
#   scripts/freearc-smoke.sh archive1.bin archive2.bin
#   scripts/freearc-smoke.sh    # auto-discover under ~/Downloads
#
# Output goes to stdout; one section per archive. Exit code is 0
# unless cargo build itself failed.

set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here/freearc-native"

echo "=== build ==="
cargo build --release --quiet
LS="$here/freearc-native/target/release/fg-arc-ls"
FILES="$here/freearc-native/target/release/fg-arc-files"

inputs=()
if [ $# -gt 0 ]; then
  for arg in "$@"; do
    if [ -d "$arg" ]; then
      while IFS= read -r -d '' f; do
        inputs+=("$f")
      done < <(find "$arg" -type f \( -iname '*.bin' -o -iname '*.arc' \) -print0 | sort -z)
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
echo "=== ${#inputs[@]} archive(s) found ==="
for arc in "${inputs[@]}"; do
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
