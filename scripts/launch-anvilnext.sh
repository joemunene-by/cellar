#!/bin/bash
# launch-anvilnext.sh — Ubisoft AnvilNext / Dunia / Disrupt wrapper.
#
# Ubisoft Connect (formerly Uplay) is the launcher wall on retail
# builds. Cracked / standalone releases ship a replacement
# uplay_r1_loader.dll alongside the game exe that stubs the auth
# handshake. This wrapper sanity-checks that the unlocker DLL is
# present in the game dir before launching, since an empty
# WINEDLLOVERRIDES would just make LoadLibrary fail and the game
# would abort with a confusing "Ubisoft Connect not installed" error.
#
# Usage:
#   launch-anvilnext.sh <game-dir>
#
# Set CELLAR_NO_UPLAY_CHECK=1 to skip the uplay_r1_loader.dll check
# (e.g. if you're running a retail build with Ubisoft Connect actually
# installed in the prefix and don't want the unlocker check).
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GAME="${1:?game dir name required}"
GAME_DIR="$HOME/Games-source/$GAME"

if [ ! -d "$GAME_DIR" ]; then
  echo "ERROR: game dir not found: $GAME_DIR" >&2
  exit 3
fi

if [ "${CELLAR_NO_UPLAY_CHECK:-0}" != "1" ]; then
  # Check for the unlocker DLL anywhere in the top 3 levels.
  found=$(find "$GAME_DIR" -maxdepth 3 -iname 'uplay_r1_loader*.dll' 2>/dev/null | head -1)
  if [ -z "$found" ]; then
    echo "WARNING: no uplay_r1_loader*.dll found in $GAME_DIR." >&2
    echo "Retail Ubisoft builds need Ubisoft Connect installed; cracked builds" >&2
    echo "ship an UplayR1Unlocker-style replacement DLL. Without one, the game" >&2
    echo "will likely abort with 'Ubisoft Connect not installed'." >&2
    echo "(set CELLAR_NO_UPLAY_CHECK=1 to suppress this check.)" >&2
    echo >&2
  else
    echo "found uplay_r1_loader: ${found#$GAME_DIR/}"
  fi
fi

exec /bin/bash "$CELLAR_ROOT/scripts/launch-engine.sh" anvilnext-ubisoft "$GAME"
