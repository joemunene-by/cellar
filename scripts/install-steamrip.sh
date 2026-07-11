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

# Find the game folder by SIZE, not by "biggest exe" - a SteamRIP redist AIO
# (VisualCppRedist_AIO ~30MB) can be larger than the game's own launcher exe, so
# exe size is unreliable. SteamRIP always sits the game folder next to a
# _CommonRedist folder; the game folder is by far the biggest sibling.
CR=$(find "$TMP" -type d -iname "_CommonRedist" 2>/dev/null | head -1)
if [ -n "$CR" ]; then
  ROOT=$(dirname "$CR")
else
  # No _CommonRedist: unwrap any single-child enclosing folders, then use that.
  ROOT="$TMP"
  while :; do
    n=$(find "$ROOT" -mindepth 1 -maxdepth 1 2>/dev/null | wc -l | tr -d ' ')
    only=$(find "$ROOT" -mindepth 1 -maxdepth 1 2>/dev/null | head -1)
    if [ "$n" = "1" ] && [ -d "$only" ]; then ROOT="$only"; else break; fi
  done
fi
# Game folder = biggest immediate subdirectory of ROOT that isn't a redist dir.
GAMEDIR=$(find "$ROOT" -mindepth 1 -maxdepth 1 -type d \
            ! -iname "_CommonRedist" ! -iname "__Installer" ! -iname '$RECYCLE.BIN' \
            -exec du -s {} + 2>/dev/null | sort -rn | head -1 | cut -f2-)
# Fallback: game files may sit directly in ROOT (no game subfolder).
if [ -z "$GAMEDIR" ] && ls "$ROOT"/*.exe >/dev/null 2>&1; then GAMEDIR="$ROOT"; fi
[ -n "$GAMEDIR" ] && [ -d "$GAMEDIR" ] || {
  echo "could not find the game folder in the archive:" >&2; ls -la "$ROOT" >&2
  echo "(extraction kept for inspection: $TMP)" >&2; exit 5; }

# SAFETY NET: a real game is large. If the pick is under 1GB something is wrong -
# do NOT delete the extraction (an earlier bug nuked a 75GB extract this way).
KB=$(du -sk "$GAMEDIR" 2>/dev/null | cut -f1)
if [ "${KB:-0}" -lt 1048576 ]; then
  echo "detected game folder '$GAMEDIR' is under 1GB - looks wrong; deleting nothing." >&2
  echo "extraction kept at: $TMP  (place the real folder manually)" >&2
  exit 5
fi

NAME=$(basename "$GAMEDIR")
DEST="$BASE/$NAME"
[ -e "$DEST" ] && { echo "already exists: $DEST (move aside first; temp kept: $TMP)" >&2; exit 6; }
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
