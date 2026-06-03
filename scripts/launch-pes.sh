#!/bin/bash
# launch-pes.sh — PES / eFootball wrapper.
#
# Two distinct eras:
#   - PES 2019-2021: Konami Fox Engine, DX11 native. PES 2021 accepts
#     a -dx11 launch flag (informational; it defaults DX11 anyway on
#     PC). Folder usually named "Pro Evolution Soccer 2021" or
#     "eFootball PES 2021".
#   - eFootball 2024+: Unreal Engine 4 underneath. Doesn't take the
#     same launch flags as PES; follow the unreal-engine-4-5 profile
#     more closely. Folder usually named "eFootball" or "eFootball 2024".
#
# Usage:
#   launch-pes.sh <game-dir>
#
# Heuristic: if the game dir contains the substring "efootball" (case-
# insensitive), use the unreal-engine-4-5 profile; otherwise fall
# through to pes-foxengine.
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GAME="${1:?game dir name required (e.g. 'PES 2021' or 'eFootball')}"

lower=$(echo "$GAME" | tr 'A-Z' 'a-z')
case "$lower" in
  *efootball*) PROFILE="unreal-engine-4-5" ;;
  *) PROFILE="pes-foxengine" ;;
esac

echo "using profile: $PROFILE"
exec /bin/bash "$CELLAR_ROOT/scripts/launch-engine.sh" "$PROFILE" "$GAME"
