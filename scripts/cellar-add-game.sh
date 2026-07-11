#!/bin/bash
# cellar-add-game.sh - end to end: vet a game folder, then build a clickable
# .app for it (via the generic engine launcher + a profile, or a custom script).
# The runtime launcher is copied into ~/.cellar/launchers so the .app can reach
# it (macOS TCC blocks a double-clicked app from reading ~/Desktop).
#
# Usage:
#   cellar-add-game.sh <game-dir> --profile <id>        # generic engine launcher
#   cellar-add-game.sh <game-dir> --launcher <script>   # your own launch script
#   [--name "Display Name"]
set -u
DIR=""; NAME=""; PROFILE=""; LAUNCHER=""
while [ $# -gt 0 ]; do
  case "$1" in
    --name)     NAME="$2"; shift 2;;
    --profile)  PROFILE="$2"; shift 2;;
    --launcher) LAUNCHER="$2"; shift 2;;
    -*)         echo "unknown flag: $1" >&2; exit 2;;
    *)          DIR="$1"; shift;;
  esac
done
[ -d "$DIR" ] || { echo "usage: cellar-add-game.sh <game-dir> (--profile ID | --launcher script) [--name X]" >&2; exit 2; }
SELF="$(cd "$(dirname "$0")" && pwd)"
NAME="${NAME:-$(basename "$DIR")}"

echo "=== vetting $NAME ==="
if [ -x "$SELF/will-it-run.sh" ]; then
  "$SELF/will-it-run.sh" "$DIR" || { echo "hard-wall protection detected - not adding (see above)." >&2; exit 1; }
fi

mkdir -p "$HOME/.cellar/launchers"
if [ -z "$LAUNCHER" ]; then
  [ -n "$PROFILE" ] || { echo "pass --profile <id> (see profiles.json) or --launcher <script>" >&2; exit 2; }
  # Copy the engine launcher + its dep into ~/.cellar (TCC-safe for the .app).
  cp "$SELF/launch-engine.sh" "$HOME/.cellar/launchers/launch-engine.sh"
  cp "$SELF/free-input.sh"    "$HOME/.cellar/launchers/free-input.sh" 2>/dev/null || true
  chmod +x "$HOME/.cellar/launchers/"launch-engine.sh 2>/dev/null || true
  # The copied launcher resolves profiles.json as ~/.cellar/profiles.json
  # (dirname/.. of ~/.cellar/launchers), so the bundled profiles must live there.
  cp "$SELF/../profiles.json" "$HOME/.cellar/profiles.json"
  slug=$(echo "$NAME" | tr 'A-Z ' 'a-z-' | tr -dc 'a-z0-9-')
  LAUNCHER="$HOME/.cellar/launchers/launch-$slug.sh"
  printf '#!/bin/bash\nexec /bin/bash "%s" "%s" "%s"\n' \
    "$HOME/.cellar/launchers/launch-engine.sh" "$PROFILE" "$(basename "$DIR")" > "$LAUNCHER"
  chmod +x "$LAUNCHER"
  echo "runtime launcher: $LAUNCHER (profile $PROFILE)"
fi

echo "=== building clickable app ==="
"$SELF/make-game-app.sh" "$NAME" "$LAUNCHER" "$DIR"
echo "done -> /Applications/cellar Games/$NAME.app"
