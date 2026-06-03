#!/bin/bash
# bottle-inspect.sh — print everything useful about a cellar bottle without
# launching anything.
#
# Reports on: prefix path + size, wine version that created it, DLL
# overrides currently set in the registry, installed Program Files entries,
# winetricks verbs known to be installed, save backups available for the
# bottle, last launcher log timestamp.
#
# Usage:
#   bottle-inspect.sh <bottle>
#   bottle-inspect.sh --list      # list available bottles
set -u

BOTTLES_DIR="$HOME/.cellar/bottles"
BACKUPS_DIR="$HOME/.cellar/backups"

if [ "${1:-}" = "--list" ] || [ -z "${1:-}" ]; then
  echo "bottles under $BOTTLES_DIR:"
  if [ -d "$BOTTLES_DIR" ]; then
    find "$BOTTLES_DIR" -maxdepth 1 -mindepth 1 -type d -exec basename {} \; | sort
  else
    echo "  (none)"
  fi
  [ -z "${1:-}" ] && { echo; echo "usage: $0 <bottle>"; exit 1; }
  exit 0
fi

BOTTLE="$1"
PREFIX="$BOTTLES_DIR/$BOTTLE/prefix"
if [ ! -d "$PREFIX" ]; then
  echo "ERROR: bottle not found: $PREFIX" >&2
  exit 1
fi

echo "=== bottle: $BOTTLE ==="
echo "prefix: $PREFIX"
echo "size:   $(du -sh "$PREFIX" 2>/dev/null | cut -f1)"

# Created / last touched.
if [ -f "$PREFIX/.update-timestamp" ]; then
  ts=$(cat "$PREFIX/.update-timestamp" 2>/dev/null | head -1)
  echo "created/touched: $ts"
fi
if [ -f "$PREFIX/system.reg" ]; then
  echo "system.reg modified: $(stat -f "%Sm" "$PREFIX/system.reg" 2>/dev/null)"
fi
echo

echo "=== wine version ==="
WINE="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib/wine/x86_64-unix/wine"
if [ -x "$WINE" ]; then
  echo "$($WINE --version 2>/dev/null)"
else
  echo "(wine binary not found at $WINE)"
fi
echo

echo "=== DLL overrides (HKCU\\Software\\Wine\\DllOverrides) ==="
if [ -f "$PREFIX/user.reg" ]; then
  awk '
    /^\[Software\\\\Wine\\\\DllOverrides\]/ { in_section=1; next }
    /^\[/ { in_section=0 }
    in_section && /^"[^"]+"="[^"]*"$/ { print "  " $0 }
  ' "$PREFIX/user.reg" | head -40
else
  echo "  (user.reg not found)"
fi
echo

echo "=== drive_c installed software ==="
for d in "$PREFIX/drive_c/Program Files"/*/  "$PREFIX/drive_c/Program Files (x86)"/*/; do
  [ -d "$d" ] && echo "  $(basename "$d")"
done | sort -u | head -20
echo

echo "=== winetricks log (last 30 lines if present) ==="
if [ -f "$PREFIX/winetricks.log" ]; then
  tail -30 "$PREFIX/winetricks.log" | sed 's/^/  /'
else
  echo "  (no winetricks.log in prefix)"
fi
echo

echo "=== save backups ==="
if [ -d "$BACKUPS_DIR/$BOTTLE" ]; then
  ls -lh "$BACKUPS_DIR/$BOTTLE"/*.tar.gz 2>/dev/null | tail -5 | sed 's/^/  /' || echo "  (none)"
else
  echo "  (no backups; run scripts/backup-saves.sh $BOTTLE to create one)"
fi
echo

echo "=== last launch log ==="
# Match either prefix patterns (fifa<N>.log, carxstreet-*.log) or the
# generic cellar-<bottle>.log written by launch-engine.sh.
for cand in \
    "/tmp/cellar-$BOTTLE.log" \
    "/tmp/$BOTTLE.log" \
    "/tmp/${BOTTLE#*-}.log"; do
  if [ -f "$cand" ]; then
    echo "  $cand"
    echo "  last modified: $(stat -f "%Sm" "$cand")"
    echo "  size: $(du -h "$cand" | cut -f1)"
    echo "  scripts/cellar-logs.sh $(basename "$cand")  # to tail"
    echo "  scripts/analyze-log.sh $cand  # to scan for known failure patterns"
    exit 0
  fi
done
echo "  (no matching log in /tmp; bottle hasn't launched yet, or log was pruned)"
