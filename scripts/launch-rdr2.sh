#!/bin/bash
# launch-rdr2.sh — Red Dead Redemption 2 wrapper around launch-engine.sh.
#
# Why a dedicated wrapper: the rage-rockstar profile covers four games
# (GTA V, GTA IV, RDR2, Max Payne 3), each with different command-line
# needs. RDR2 specifically benefits from running the Vulkan renderer via
# `-sgadriver=Vulkan` instead of the DX12 default, because MoltenVK has
# historically been more stable than D3DMetal's DX12 path. (Note: the
# correct flag is `-sgadriver=Vulkan`, not bare `-vulkan`; sources in
# CHANGELOG.)
#
# Usage:
#   launch-rdr2.sh [game-dir]
#
# Default game-dir is "Red Dead Redemption 2" under ~/Games-source/.
# Pass a different name for cracked builds that use a different folder.
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GAME="${1:-Red Dead Redemption 2}"
exec /bin/bash "$CELLAR_ROOT/scripts/launch-engine.sh" rage-rockstar "$GAME" -sgadriver=Vulkan
