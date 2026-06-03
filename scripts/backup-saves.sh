#!/bin/bash
# backup-saves.sh — snapshot a cellar bottle's save state to a timestamped tarball.
#
# Save data lives under each prefix's emulated Windows user dir:
#   ~/.cellar/bottles/<bottle>/prefix/drive_c/users/<host-user>/
#       Documents/                    (most modern saves; FIFA, Rockstar, Witcher,
#                                      Bethesda My Games, BioWare, KONAMI)
#       AppData/Local/                (RE Engine, UE4, Hogwarts, Cyberpunk local)
#       AppData/Roaming/              (Elden Ring, Sekiro, DS3, REDengine roaming)
#       AppData/LocalLow/             (Unity titles, some indie games)
#
# Strategy: tar those four roots, gzip, drop the artifact under
# ~/.cellar/backups/<bottle>/<YYYY-MM-DD-HHMMSS>.tar.gz. Skips
# transient files (cache/, temp/, log files) via tar --exclude.
#
# Usage:
#   backup-saves.sh <bottle-name>
#
# Examples:
#   backup-saves.sh carxstreet-hybrid
#   backup-saves.sh fifa19
#   backup-saves.sh rage-rockstar-grand-theft-auto-v
#
# List bottles:
#   backup-saves.sh --list
set -euo pipefail

BOTTLES_DIR="$HOME/.cellar/bottles"
BACKUPS_DIR="$HOME/.cellar/backups"

if [ "${1:-}" = "--list" ] || [ -z "${1:-}" ]; then
  echo "available bottles under $BOTTLES_DIR:"
  if [ -d "$BOTTLES_DIR" ]; then
    find "$BOTTLES_DIR" -maxdepth 1 -mindepth 1 -type d -exec basename {} \; | sort
  else
    echo "  (none — $BOTTLES_DIR does not exist yet)"
  fi
  [ -z "${1:-}" ] && { echo; echo "usage: $0 <bottle-name>"; exit 1; }
  exit 0
fi

BOTTLE="$1"
PREFIX="$BOTTLES_DIR/$BOTTLE/prefix"
USERS_DIR="$PREFIX/drive_c/users"

if [ ! -d "$PREFIX" ]; then
  echo "ERROR: bottle prefix not found: $PREFIX" >&2
  echo "available bottles:" >&2
  find "$BOTTLES_DIR" -maxdepth 1 -mindepth 1 -type d -exec basename {} \; 2>/dev/null | sort >&2
  exit 1
fi
if [ ! -d "$USERS_DIR" ]; then
  echo "ERROR: $USERS_DIR not found (prefix not initialized?)" >&2
  exit 1
fi

# Pick the wine user dir. Usually matches $USER, but a fresh prefix might
# only have 'Public' if wineboot didn't complete.
WINE_USER=""
for candidate in "$USER" Public; do
  if [ -d "$USERS_DIR/$candidate" ]; then
    WINE_USER="$candidate"
    break
  fi
done
if [ -z "$WINE_USER" ]; then
  # Fall back to whatever's in the users dir.
  WINE_USER=$(find "$USERS_DIR" -maxdepth 1 -mindepth 1 -type d -exec basename {} \; | head -1)
fi
if [ -z "$WINE_USER" ]; then
  echo "ERROR: no wine user found under $USERS_DIR" >&2
  exit 1
fi

USER_DIR="$USERS_DIR/$WINE_USER"
TS=$(date +%Y-%m-%d-%H%M%S)
DEST_DIR="$BACKUPS_DIR/$BOTTLE"
DEST="$DEST_DIR/$TS.tar.gz"
mkdir -p "$DEST_DIR"

# Roots to back up, relative to USER_DIR.
roots=(Documents AppData/Local AppData/Roaming AppData/LocalLow)
found=()
for r in "${roots[@]}"; do
  if [ -d "$USER_DIR/$r" ]; then
    found+=("$r")
  fi
done
if [ ${#found[@]} -eq 0 ]; then
  echo "ERROR: no save roots present under $USER_DIR (Documents / AppData / AppData/Local / AppData/Roaming)" >&2
  exit 1
fi

echo "bottle:     $BOTTLE"
echo "wine user:  $WINE_USER"
echo "user dir:   $USER_DIR"
echo "roots:      ${found[*]}"
echo "dest:       $DEST"
echo

# Build the tar with sensible excludes for transient files. tar -C cd's so
# the archive entries are relative paths under USER_DIR.
tar -czf "$DEST" \
  --exclude='*.tmp' \
  --exclude='*.cache' \
  --exclude='*.log' \
  --exclude='Cache/*' \
  --exclude='cache/*' \
  --exclude='Temp/*' \
  --exclude='temp/*' \
  --exclude='CrashReport*' \
  --exclude='*.dmp' \
  -C "$USER_DIR" "${found[@]}"

size=$(du -h "$DEST" | cut -f1)
echo "wrote $size at $DEST"

# Print short retention hint without touching old backups.
existing=$(find "$DEST_DIR" -maxdepth 1 -name '*.tar.gz' -type f 2>/dev/null | wc -l | tr -d ' ')
echo "($existing total backup(s) under $DEST_DIR; this script never deletes old ones)"
