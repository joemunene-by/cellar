#!/bin/bash
# uninstall-bottle.sh — remove a cellar bottle cleanly.
#
# Deletes:
#   ~/.cellar/bottles/<bottle>/             (prefix + all per-bottle state)
#   /Applications/cellar Games/<name>.app    (if it exists; opt-in via --apps)
#   /tmp/cellar-<bottle>.log + friends       (opt-in via --logs)
#
# Save backups under ~/.cellar/backups/<bottle>/ are LEFT IN PLACE so the
# user can rebuild from saves later. Add --backups to nuke those too.
#
# Usage:
#   uninstall-bottle.sh <bottle>                     # prefix only
#   uninstall-bottle.sh <bottle> --apps              # + matching .app
#   uninstall-bottle.sh <bottle> --logs              # + /tmp logs
#   uninstall-bottle.sh <bottle> --backups           # + save backups
#   uninstall-bottle.sh <bottle> --all               # everything
#   uninstall-bottle.sh <bottle> --dry-run           # print what would be removed
#   uninstall-bottle.sh --list                       # list bottles
set -u

BOTTLES_DIR="$HOME/.cellar/bottles"
BACKUPS_DIR="$HOME/.cellar/backups"
APPS_DIR="/Applications/cellar Games"

if [ "${1:-}" = "--list" ]; then
  echo "bottles under $BOTTLES_DIR:"
  find "$BOTTLES_DIR" -maxdepth 1 -mindepth 1 -type d -exec basename {} \; 2>/dev/null | sort
  exit 0
fi

BOTTLE="${1:?bottle name required (run with --list to enumerate)}"
shift
INCLUDE_APPS=0
INCLUDE_LOGS=0
INCLUDE_BACKUPS=0
DRY=0
while [ $# -gt 0 ]; do
  case "$1" in
    --apps)    INCLUDE_APPS=1 ;;
    --logs)    INCLUDE_LOGS=1 ;;
    --backups) INCLUDE_BACKUPS=1 ;;
    --all)     INCLUDE_APPS=1; INCLUDE_LOGS=1; INCLUDE_BACKUPS=1 ;;
    --dry-run|-n) DRY=1 ;;
    *) echo "unknown flag: $1" >&2; exit 1 ;;
  esac
  shift
done

PREFIX_DIR="$BOTTLES_DIR/$BOTTLE"
if [ ! -d "$PREFIX_DIR" ]; then
  echo "ERROR: bottle not found: $PREFIX_DIR" >&2
  echo "run --list to see available bottles" >&2
  exit 1
fi

removals=()
remove() {
  if [ $DRY -eq 1 ]; then
    echo "  [dry-run] would remove: $1"
  else
    echo "  removing: $1"
    rm -rf "$1"
  fi
  removals+=("$1")
}

echo "==> uninstall bottle: $BOTTLE"
remove "$PREFIX_DIR"

if [ $INCLUDE_APPS -eq 1 ]; then
  # The .app might be named after the bottle slug or after the game; try
  # both. The bottle name is usually "<profile>-<game-slug>", so strip
  # the profile prefix to guess the display name.
  for app in "$APPS_DIR/$BOTTLE.app" "$APPS_DIR/${BOTTLE#*-}.app"; do
    [ -d "$app" ] && remove "$app"
  done
fi

if [ $INCLUDE_LOGS -eq 1 ]; then
  for log in "/tmp/cellar-$BOTTLE.log" "/tmp/$BOTTLE.log" "/tmp/cellar-$BOTTLE.pid" "/tmp/$BOTTLE.pid"; do
    [ -f "$log" ] && remove "$log"
  done
fi

if [ $INCLUDE_BACKUPS -eq 1 ]; then
  [ -d "$BACKUPS_DIR/$BOTTLE" ] && remove "$BACKUPS_DIR/$BOTTLE"
fi

echo
if [ $DRY -eq 1 ]; then
  echo "DRY RUN: ${#removals[@]} item(s) would be removed. Re-run without --dry-run to actually delete."
else
  echo "DONE: ${#removals[@]} item(s) removed."
  if [ $INCLUDE_BACKUPS -ne 1 ] && [ -d "$BACKUPS_DIR/$BOTTLE" ]; then
    echo "Save backups kept at $BACKUPS_DIR/$BOTTLE/ (add --backups to nuke)."
  fi
fi
