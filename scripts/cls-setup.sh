#!/usr/bin/env bash
# One-shot setup for the FreeArc CLS hybrid path.
#
# What it does:
#   1. Build cellar-freearc-cls-host.exe (PE32 helper) if missing.
#   2. Scan the named wine bottle(s) under ~/.cellar/bottles/ for any
#      cls-*.dll the FitGirl installer left behind in its temp payload.
#   3. Stage the DLLs into ~/.cellar/cls/ for the dispatch path to find.
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
echo "=== 2/3 find cls-*.dll in wine bottle(s) ==="
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

for d in "${search_dirs[@]}"; do
  echo "scanning $d ..."
  while IFS= read -r -d '' dll; do
    name="$(basename "$dll")"
    cp -f "$dll" "$dest_dir/$name"
    echo "  staged $name from $dll"
    found=$((found + 1))
  done < <(find "$d" -type f -iname 'cls-*.dll' -print0 2>/dev/null)
done

if [ "$found" -eq 0 ]; then
  echo
  echo "no cls-*.dll found in any bottle."
  echo "tip: start a FitGirl installer once (it unpacks the DLLs to"
  echo "     a temp dir under drive_c/users/...). cancel before the"
  echo "     install completes, then re-run this script. the DLLs"
  echo "     should still be on disk."
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
