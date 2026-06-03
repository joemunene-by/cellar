#!/bin/bash
# make-fifa-apps.sh — generate clickable /Applications/cellar Games/FIFA <N>.app
# bundles for one or more FIFA versions.
#
# Usage:
#   make-fifa-apps.sh [version ...]
#
# Examples:
#   make-fifa-apps.sh 19              # just FIFA 19
#   make-fifa-apps.sh 14 15 16        # FIFA 14, 15, 16
#   make-fifa-apps.sh                 # all installed FIFAs detected in ~/Games-source
#
# The wrapper bakes the version into a per-version launcher script under
# ~/.cellar/launchers/launch-fifa<ver>.sh, then hands it to make-game-app.sh.
# Each per-version launcher is a one-liner that execs the master
# scripts/launch-fifa.sh with the right arg.
set -euo pipefail

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MASTER="$CELLAR_ROOT/scripts/launch-fifa.sh"
WRAP_DIR="$HOME/.cellar/launchers"
MAKER="$CELLAR_ROOT/scripts/make-game-app.sh"

if [ ! -x "$MASTER" ]; then
  echo "missing master launcher: $MASTER" >&2
  exit 1
fi
if [ ! -x "$MAKER" ]; then
  echo "missing $MAKER" >&2
  exit 1
fi

mkdir -p "$WRAP_DIR"

versions=("$@")
if [ ${#versions[@]} -eq 0 ]; then
  # auto-detect: any ~/Games-source/FIFA <N>/ dir with a recognisable exe
  while IFS= read -r d; do
    name=$(basename "$d")
    case "$name" in
      "FIFA "[0-9][0-9])
        v="${name##FIFA }"
        case "$v" in 14|15|16|17|18|19|20|21|22|23) versions+=("$v") ;; esac
        ;;
    esac
  done < <(find "$HOME/Games-source" -maxdepth 1 -type d -name "FIFA *" 2>/dev/null | sort)
  if [ ${#versions[@]} -eq 0 ]; then
    echo "no FIFA versions specified and none detected under ~/Games-source/" >&2
    echo "usage: $0 [version ...]" >&2
    exit 2
  fi
  echo "auto-detected: ${versions[*]}"
fi

for V in "${versions[@]}"; do
  case "$V" in 14|15|16|17|18|19|20|21|22|23) ;; *) echo "skip invalid version: $V" >&2; continue ;; esac
  WRAP="$WRAP_DIR/launch-fifa$V.sh"
  cat > "$WRAP" <<EOF
#!/bin/bash
exec /bin/bash "$MASTER" $V "\$@"
EOF
  chmod +x "$WRAP"
  echo "wrapper: $WRAP"
  "$MAKER" "FIFA $V" "$WRAP"
done

echo "done. apps land in /Applications/cellar Games/"
