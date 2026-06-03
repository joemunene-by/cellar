#!/bin/bash
# launch-redengine.sh — CDPR REDengine (Cyberpunk 2077, Witcher 3) wrapper.
#
# Two distinct cases:
#   - Witcher 3: REDengine 3, DX11. The native macOS Apple Silicon port
#     from CDPR exists for Witcher 1 and 2 only; W3 is still a wine
#     target. NotebookCheck confirms 60 FPS via CrossOver Metal 4 on
#     macOS Tahoe.
#   - Cyberpunk 2077: REDengine 4, DX12 only. Bindless + heavy shader
#     compile. Same shader class that broke CarX pre-D3DMetal 3.0.
#     Expect long initial-launch shader compile pause (~10 min on
#     first run); set CELLAR_CP77_SHADER_HINT=1 to print a heads-up
#     before launch.
#
# Usage:
#   launch-redengine.sh <game-dir>
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GAME="${1:?game dir name required}"

# Heuristic: if the game dir or its first subdir contains "cyberpunk",
# warn about the shader compile pause unless the user explicitly
# silenced the hint.
lower=$(echo "$GAME" | tr 'A-Z' 'a-z')
if [[ "$lower" == *cyberpunk* ]] && [ "${CELLAR_CP77_SHADER_HINT:-1}" = "1" ]; then
  cat >&2 <<'EOF'
HINT: Cyberpunk 2077 first launch under D3DMetal does a full shader
compile pass which can take 5-15 minutes with no UI feedback. The
process is alive (you can see GPU work in Activity Monitor); don't
kill it. Subsequent launches use the cached shaders and start fast.
Set CELLAR_CP77_SHADER_HINT=0 to suppress this message.

EOF
fi

exec /bin/bash "$CELLAR_ROOT/scripts/launch-engine.sh" redengine "$GAME"
