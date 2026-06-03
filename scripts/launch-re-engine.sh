#!/bin/bash
# launch-re-engine.sh — Capcom RE Engine wrapper.
#
# RE Engine ships an in-game graphics-API toggle that persists in the
# game's config (RE Village, RE4 Remake) or via a launch-flag override
# (RE2/3 Remake, DMC5). This wrapper accepts --api dx11|dx12 and forwards
# the matching launch arg.
#
# Default: dx11 because the D3D11 -> D3DMetal path has fewer regressions
# than DX12 -> D3DMetal across the family. RE Village can be forced to
# DX11 only by editing its config file (no command-line override), in
# which case --api dx11 here is informational only.
#
# Usage:
#   launch-re-engine.sh <game-dir>
#   launch-re-engine.sh <game-dir> --api dx12
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GAME="${1:?game dir name required (e.g. 'Resident Evil Village')}"
API="dx11"
shift
while [ $# -gt 0 ]; do
  case "$1" in
    --api) API="${2:?--api needs dx11 or dx12}"; shift 2 ;;
    *) echo "unknown flag: $1" >&2; exit 1 ;;
  esac
done

case "$API" in
  dx11|dx12) ;;
  *) echo "--api must be dx11 or dx12 (got '$API')" >&2; exit 1 ;;
esac

extra_args=()
if [ "$API" = "dx11" ]; then
  # Most RE Engine titles accept -dx11; DMC5 has it documented, RE2/3
  # respect it via config edits. RE Village ignores it (config-only).
  extra_args+=("-dx11")
fi

exec /bin/bash "$CELLAR_ROOT/scripts/launch-engine.sh" re-engine "$GAME" "${extra_args[@]}"
