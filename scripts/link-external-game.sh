#!/bin/bash
# link-external-game.sh - symlink a game folder on an external drive into
# ~/Games-source so cellar launchers (which look in ~/Games-source/<name>) find
# it. Warns if the drive is read-only (games must write saves).
#
# Usage: link-external-game.sh <external-game-dir> [link-name]
set -u
SRC="${1:?usage: link-external-game.sh <external-game-dir> [link-name]}"
[ -d "$SRC" ] || { echo "not a directory: $SRC" >&2; exit 2; }
NAME="${2:-$(basename "$SRC")}"
LINK="$HOME/Games-source/$NAME"

if ( : > "$SRC/.cellar-wtest" ) 2>/dev/null; then
  rm -f "$SRC/.cellar-wtest"
else
  echo "WARNING: $SRC is READ-ONLY (NTFS?). The game can't write saves." >&2
  echo "         Reformat the drive to exFAT or APFS for saves to work." >&2
fi

mkdir -p "$HOME/Games-source"
if [ -e "$LINK" ] || [ -L "$LINK" ]; then
  echo "already exists: $LINK (remove it first: rm \"$LINK\")" >&2; exit 3
fi
ln -s "$SRC" "$LINK"
echo "linked: $LINK -> $SRC"
echo "vet + add it with:  cellar-add-game.sh \"$LINK\" --profile <id>"
