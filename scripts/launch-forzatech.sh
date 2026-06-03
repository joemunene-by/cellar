#!/bin/bash
# launch-forzatech.sh — Forza Horizon 4/5 (ForzaTech engine) wrapper.
#
# Only the Steam build is a realistic target on wine. The Microsoft
# Store UWP variant depends on UWP APIs that wine doesn't implement
# reliably enough for FH5 specifically (ProtonDB FH5 reports confirm
# this). This wrapper sanity-checks that the game dir does NOT look
# like a UWP install before launching.
#
# Usage:
#   launch-forzatech.sh <game-dir>
#
# Set CELLAR_FORCE_UWP=1 to skip the UWP check (you're on your own).
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GAME="${1:?game dir name required (e.g. 'Forza Horizon 5')}"
GAME_DIR="$HOME/Games-source/$GAME"

if [ ! -d "$GAME_DIR" ]; then
  echo "ERROR: game dir not found: $GAME_DIR" >&2
  exit 3
fi

# UWP installs have AppxManifest.xml at the root or in a Content/ subdir.
# Steam builds have ForzaHorizon5.exe (or similar) without AppxManifest.
if [ "${CELLAR_FORCE_UWP:-0}" != "1" ]; then
  if find "$GAME_DIR" -maxdepth 3 -iname 'AppxManifest.xml' 2>/dev/null | head -1 | grep -q .; then
    cat >&2 <<'EOF'
ERROR: This appears to be a Microsoft Store (UWP) build of Forza,
       not the Steam build. UWP Forza on wine is broken in ways no
       launcher recipe currently solves. The cellar forzatech profile
       only targets the Steam build.

Set CELLAR_FORCE_UWP=1 if you want to override this check anyway.
EOF
    exit 4
  fi
fi

exec /bin/bash "$CELLAR_ROOT/scripts/launch-engine.sh" forzatech "$GAME"
