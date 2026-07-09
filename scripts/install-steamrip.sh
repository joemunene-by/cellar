#!/bin/bash
# install-steamrip.sh - extract a SteamRIP .rar into a game folder (internal or
# external SSD) and vet it. Uses unar (SteamRIP archives are RAR5; 7z fails with
# "Unsupported Method"). Handles the enclosing folder SteamRIP wraps around
# multi-root archives, and picks the real game folder (dir of the biggest exe).
#
# Usage: install-steamrip.sh <archive.rar> [target-base-dir]
#   target-base-dir defaults to ~/Games-source. Pass a /Volumes/<SSD>/... path to
#   install to an external drive (must be exFAT/APFS - NTFS mounts read-only).
set -u
ARC="${1:?usage: install-steamrip.sh <archive.rar> [target-base-dir]}"
BASE="${2:-$HOME/Games-source}"
[ -f "$ARC" ] || { echo "archive not found: $ARC" >&2; exit 2; }
command -v unar >/dev/null 2>&1 || { echo "need unar: brew install unar" >&2; exit 2; }
mkdir -p "$BASE" 2>/dev/null || { echo "cannot create target: $BASE" >&2; exit 3; }
if ! ( : > "$BASE/.cellar-wtest" ) 2>/dev/null; then
  echo "target not writable: $BASE (NTFS drive? reformat exFAT/APFS)" >&2; exit 3
fi
rm -f "$BASE/.cellar-wtest"

echo "extracting $(basename "$ARC") -> $BASE (this takes a while)..."
TMP="$BASE/.steamrip-extract.$$"; mkdir -p "$TMP"
if ! unar -q -o "$TMP" -f "$ARC"; then echo "extract failed" >&2; rm -rf "$TMP"; exit 4; fi

# The real game folder = directory containing the biggest .exe (game exes are big;
# vcredist/DirectX redists under _CommonRedist are tiny).
BIGEXE=$(find "$TMP" -type f -iname "*.exe" -size +15M 2>/dev/null \
        | while IFS= read -r f; do printf '%s\t%s\n' "$(stat -f %z "$f" 2>/dev/null)" "$f"; done \
        | sort -rn | head -1 | cut -f2-)
[ -n "$BIGEXE" ] || { echo "no game exe (>15MB) found in archive:" >&2; ls "$TMP" >&2; rm -rf "$TMP"; exit 5; }
GAMEDIR=$(dirname "$BIGEXE")
NAME=$(basename "$GAMEDIR")
DEST="$BASE/$NAME"
[ -e "$DEST" ] && { echo "already exists: $DEST (move it aside first)" >&2; rm -rf "$TMP"; exit 6; }
mv "$GAMEDIR" "$DEST"
rm -rf "$TMP"
echo "installed: $DEST  ($(du -sh "$DEST" 2>/dev/null | cut -f1))"
echo

SELF="$(cd "$(dirname "$0")" && pwd)"
[ -x "$SELF/will-it-run.sh" ] && "$SELF/will-it-run.sh" "$DEST"
echo
echo "next steps:"
echo "  - build a clickable app:  cellar-add-game.sh \"$DEST\" --profile <id>"
echo "  - external install? link it:  link-external-game.sh \"$DEST\""
echo "  - reclaim space:  rm \"$ARC\""
