#!/bin/bash
# launch-bethesda.sh — Bethesda Creation Engine wrapper with SKSE/F4SE auto-detect.
#
# Why a dedicated wrapper: Skyrim SE / AE, Fallout 4, Fallout NV, and Fallout 3
# all benefit from being launched via their script extender loader (SKSE64,
# F4SE, NVSE, FOSE) instead of the main exe whenever the loader is present.
# The extender hooks the game's address space at startup to expose its plugin
# ABI; without the loader, mods that depend on SKSE/F4SE/etc just don't run.
# This wrapper detects the loader and passes it as --exe to launch-engine.sh.
#
# Loader names (case-insensitive resolution):
#   Skyrim SE / AE:       skse64_loader.exe
#   Skyrim LE (legacy):   skse_loader.exe
#   Fallout 4:            f4se_loader.exe
#   Fallout NV:           nvse_loader.exe
#   Fallout 3:            fose_loader.exe
#   Oblivion:             obse_loader.exe
#
# Usage:
#   launch-bethesda.sh <game-dir>
#
# If you want to bypass SKSE auto-detect and launch the bare game, set
# CELLAR_NO_SKSE=1 in the environment.
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GAME="${1:?game dir name required (e.g. 'Skyrim Special Edition')}"
GAME_DIR="$HOME/Games-source/$GAME"

if [ ! -d "$GAME_DIR" ]; then
  echo "ERROR: game dir not found: $GAME_DIR" >&2
  exit 3
fi

LOADER=""
if [ "${CELLAR_NO_SKSE:-0}" != "1" ]; then
  for cand in skse64_loader.exe skse_loader.exe f4se_loader.exe nvse_loader.exe fose_loader.exe obse_loader.exe; do
    match=$(find "$GAME_DIR" -maxdepth 2 -iname "$cand" 2>/dev/null | head -1)
    if [ -n "$match" ]; then
      LOADER="${match#$GAME_DIR/}"
      break
    fi
  done
fi

if [ -n "$LOADER" ]; then
  echo "detected script extender loader: $LOADER"
  exec /bin/bash "$CELLAR_ROOT/scripts/launch-engine.sh" --exe "$LOADER" bethesda-creation "$GAME"
else
  echo "no SKSE/F4SE/NVSE/FOSE/OBSE loader found, launching bare game"
  exec /bin/bash "$CELLAR_ROOT/scripts/launch-engine.sh" bethesda-creation "$GAME"
fi
