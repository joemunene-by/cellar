#!/usr/bin/env bash
# One-shot setup for the FreeArc CLS hybrid path.
#
# What it does:
#   1. Build cellar-freearc-cls-host.exe (PE32 helper) if missing.
#   2. Scan the named wine bottle(s) under ~/.cellar/bottles/ for a
#      directory containing the FitGirl plugin staging set (marker
#      file: cls-lolly.dll). Real-world the directory is named
#      `cellar-headless` and lives under drive_c.
#   3. Stage EVERY file from that directory into ~/.cellar/cls/:
#      cls-*.dll plugins AND their sidecar workers (cls-*_x64.exe,
#      cls-*_x86.exe) AND companion DLLs (botva2, facompress, etc.).
#      The 2-piece plugin architecture means the .dll shells out to
#      the sidecar .exe; without the sidecars, the .dll calls fail
#      with "failed to start cls-NAME_x64.exe".
#   4. Print the env-var lines to drop into your shell rc so
#      freearc-native / cellar / fg-arc-x route closed codecs through
#      the wine helper.
#
# Usage:
#   scripts/cls-setup.sh                  # auto-scan every bottle
#   scripts/cls-setup.sh <bottle-id>      # only scan one bottle
#
# Re-run safely. Existing copies are overwritten.

set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
cls_host_dir="$here/freearc-cls-host"
cls_host_exe="$cls_host_dir/target/i686-pc-windows-gnu/release/cellar-freearc-cls-host.exe"
dest_dir="$HOME/.cellar/cls"
bottle_root="$HOME/.cellar/bottles"

echo "=== 1/3 build cls-host ==="
if [ -f "$cls_host_exe" ]; then
  echo "already built: $cls_host_exe"
else
  (cd "$cls_host_dir" && ./build.sh)
fi

echo
echo "=== 2/3 find plugin staging dir in wine bottle(s) ==="
mkdir -p "$dest_dir"
found=0
search_dirs=()
if [ $# -gt 0 ]; then
  for b in "$@"; do search_dirs+=("$bottle_root/$b"); done
else
  if [ -d "$bottle_root" ]; then
    while IFS= read -r -d '' d; do search_dirs+=("$d"); done \
      < <(find "$bottle_root" -mindepth 1 -maxdepth 1 -type d -print0)
  fi
fi

if [ ${#search_dirs[@]} -eq 0 ]; then
  echo "no bottles found under $bottle_root"
  echo "create a bottle first, run a FitGirl installer in it, then re-run."
  exit 0
fi

# Locate the staging dir by its marker file (cls-lolly.dll). Take
# the first one found. Multiple bottles with their own copy would
# all work; we just need one full set.
staging_src=""
for d in "${search_dirs[@]}"; do
  marker="$(find "$d" -type f \( -iname 'cls-lolly.dll' -o -iname 'cls-lollypop.dll' \) -print 2>/dev/null | head -n 1)"
  if [ -n "$marker" ]; then
    staging_src="$(dirname "$marker")"
    echo "found staging dir: $staging_src"
    break
  fi
done

if [ -z "$staging_src" ]; then
  echo
  echo "no plugin staging dir found in any bottle."
  echo "tip: start a FitGirl installer once (it unpacks the DLLs +"
  echo "     sidecar .exe files to a temp dir under drive_c/users/...)."
  echo "     cancel before the install completes; the files stay on"
  echo "     disk. then re-run this script."
  exit 0
fi

# Copy EVERYTHING from the staging dir except hidden / VCS noise.
# The plugin set is ~40 files: cls-*.dll, cls-*_x64.exe,
# cls-*_x86.exe, botva2.dll, facompress.dll, ISDone.dll, etc.
echo
echo "staging files to $dest_dir ..."
for f in "$staging_src"/*; do
  [ -f "$f" ] || continue
  name="$(basename "$f")"
  case "$name" in
    .*|*.git*) continue ;;
  esac
  cp -f "$f" "$dest_dir/$name"
  echo "  $name"
  found=$((found + 1))
done

if [ "$found" -eq 0 ]; then
  echo "(marker found but no files to copy from $staging_src — unusual)"
  exit 0
fi

echo
echo "=== 3/3 env vars ==="
cat <<EOF
add these to your shell rc (~/.zshrc / ~/.bashrc):

export CELLAR_CLS_HOST="$cls_host_exe"
export CELLAR_CLS_DIR="$dest_dir"
# export CELLAR_WINE=/path/to/wine    # only if not on PATH

then reload your shell and verify:
  fg-arc-x <some-fitgirl.bin> /tmp/x-test

closed-codec blocks (lollypop, lolzi, lolzx, lolly) should now
extract via wine + the cls plugin instead of being skipped.
EOF
