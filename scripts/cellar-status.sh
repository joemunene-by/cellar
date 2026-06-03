#!/bin/bash
# cellar-status.sh — at-a-glance overview of the local cellar state.
#
# Lists bottles + their last-launch timestamp + save backup count, plus
# the runtime versions (wine, D3DMetal) and library size. Useful as a
# quick "where am I" check.
#
# Usage:
#   cellar-status.sh
set -u

BOTTLES_DIR="$HOME/.cellar/bottles"
BACKUPS_DIR="$HOME/.cellar/backups"
LIBRARY="$HOME/.cellar/library.json"
RUNTIME_WINE="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib/wine/x86_64-unix/wine"
RUNTIME_D3DMETAL="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib64/apple_gptk/external/D3DMetal.framework"

section() { printf "\n%s\n" "$*"; }

section "runtime"
if [ -x "$RUNTIME_WINE" ]; then
  echo "  wine: $("$RUNTIME_WINE" --version 2>/dev/null)"
else
  echo "  wine: NOT INSTALLED (run cellar setup)"
fi
if [ -f "$RUNTIME_D3DMETAL/Resources/Info.plist" ]; then
  v=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
      "$RUNTIME_D3DMETAL/Resources/Info.plist" 2>/dev/null || echo "?")
  echo "  D3DMetal: $v"
else
  echo "  D3DMetal: not found"
fi

section "bottles"
if [ ! -d "$BOTTLES_DIR" ]; then
  echo "  (none — $BOTTLES_DIR doesn't exist)"
else
  count=$(find "$BOTTLES_DIR" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | wc -l | tr -d ' ')
  if [ "$count" -eq 0 ]; then
    echo "  (none yet)"
  else
    printf "  %-40s  %-8s  %-19s  %s\n" "BOTTLE" "SIZE" "LAST TOUCHED" "BACKUPS"
    printf "  %-40s  %-8s  %-19s  %s\n" "---" "---" "---" "---"
    find "$BOTTLES_DIR" -maxdepth 1 -mindepth 1 -type d 2>/dev/null | sort | while IFS= read -r b; do
      name=$(basename "$b")
      sz=$(du -sh "$b" 2>/dev/null | cut -f1)
      touched=$(stat -f "%Sm" -t "%Y-%m-%d %H:%M" "$b/prefix/system.reg" 2>/dev/null \
        || stat -f "%Sm" -t "%Y-%m-%d %H:%M" "$b" 2>/dev/null \
        || echo "?")
      bk=0
      [ -d "$BACKUPS_DIR/$name" ] && bk=$(find "$BACKUPS_DIR/$name" -name '*.tar.gz' 2>/dev/null | wc -l | tr -d ' ')
      printf "  %-40s  %-8s  %-19s  %s\n" "$name" "$sz" "$touched" "$bk"
    done
  fi
fi

section "library"
if [ -f "$LIBRARY" ] && command -v jq >/dev/null 2>&1; then
  n=$(jq '. | length' "$LIBRARY" 2>/dev/null || echo "?")
  echo "  $n game(s) in $LIBRARY"
else
  echo "  no library.json (Tauri app not run yet, or library is empty)"
fi

section "logs"
n=$(find /tmp -maxdepth 1 -type f \( \
    -name 'cellar-*.log' -o -name 'fifa*.log' -o \
    -name 'carxstreet*.log' -o -name 'nfsmw*.log' -o \
    -name 'rdr2*.log' -o -name 'skyrim*.log' \
  \) 2>/dev/null | wc -l | tr -d ' ')
echo "  $n launcher log(s) in /tmp (run \`cellar logs\` to browse, \`cellar logs prune\` to clean)"

section "background watcher"
if launchctl list 2>/dev/null | grep -q dev.cellar.watch-games; then
  echo "  RUNNING (launchd agent dev.cellar.watch-games is loaded)"
else
  echo "  NOT RUNNING (install with: scripts/install-launchd-watch.sh)"
fi
