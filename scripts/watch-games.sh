#!/bin/bash
# watch-games.sh — fswatch daemon for ~/Games-source/. When a new dir
# appears, runs find-profile.sh against the name and posts a macOS
# notification with the suggested cellar-install command.
#
# Usage:
#   watch-games.sh          # foreground (Ctrl-C to stop)
#   watch-games.sh --once   # check current state, exit (no daemon)
#
# This is a daemon you typically run via:
#   nohup scripts/watch-games.sh > /tmp/cellar-watch.log 2>&1 &
# or wrap as a launchd plist (see CHANGELOG for an example).
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GAMES_DIR="$HOME/Games-source"
STATE_FILE="$HOME/.cellar/watch-state.txt"
mkdir -p "$(dirname "$STATE_FILE")"

if [ ! -d "$GAMES_DIR" ]; then
  echo "$GAMES_DIR does not exist; create it first" >&2
  exit 1
fi

notify() {
  local title="$1" body="$2"
  if command -v osascript >/dev/null 2>&1; then
    osascript -e "display notification \"$body\" with title \"$title\"" 2>/dev/null || true
  fi
  echo "[$(date +%H:%M:%S)] $title — $body"
}

handle_new() {
  local name="$1"
  echo "[$(date +%H:%M:%S)] new game dir: $name"
  if [ -x "$CELLAR_ROOT/scripts/find-profile.sh" ]; then
    out=$("$CELLAR_ROOT/scripts/find-profile.sh" "$name" 2>&1 | head -1)
    case "$out" in
      "Best match: "*)
        profile="${out#Best match: }"
        notify "cellar: $name detected" "Profile match: $profile. Run scripts/cellar-install.sh $profile \"$name\""
        ;;
      *)
        notify "cellar: $name detected" "No profile auto-matched. Run scripts/find-profile.sh \"$name\" to pick manually."
        ;;
    esac
  fi
}

# Snapshot current state to detect what's new vs known.
current_dirs=$(find "$GAMES_DIR" -maxdepth 1 -mindepth 1 -type d -exec basename {} \; 2>/dev/null | sort)
if [ -f "$STATE_FILE" ]; then
  known=$(cat "$STATE_FILE")
else
  known=""
fi

if [ "${1:-}" = "--once" ]; then
  # Diff current vs known, notify on new, update state, exit.
  while IFS= read -r d; do
    [ -z "$d" ] && continue
    if ! echo "$known" | grep -qxF "$d"; then
      handle_new "$d"
    fi
  done <<< "$current_dirs"
  echo "$current_dirs" > "$STATE_FILE"
  exit 0
fi

# Daemon mode. Initialize state if first run.
[ ! -f "$STATE_FILE" ] && echo "$current_dirs" > "$STATE_FILE"

if ! command -v fswatch >/dev/null 2>&1; then
  echo "fswatch missing, install with: brew install fswatch" >&2
  exit 2
fi

echo "watching $GAMES_DIR for new game dirs..."
fswatch --event Created -0 -1 -r -l 2 "$GAMES_DIR" 2>/dev/null | \
while IFS= read -rd '' changed; do
  # Only react to top-level dirs being added.
  rel="${changed#$GAMES_DIR/}"
  case "$rel" in
    */*) continue ;; # nested change, ignore
  esac
  if [ -d "$changed" ]; then
    name=$(basename "$changed")
    known=$(cat "$STATE_FILE" 2>/dev/null || echo "")
    if ! echo "$known" | grep -qxF "$name"; then
      handle_new "$name"
      echo "$name" >> "$STATE_FILE"
    fi
  fi
done
