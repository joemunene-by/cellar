#!/bin/bash
# extract-icon.sh — pull a Windows .exe's embedded icon and build a macOS .icns.
# Lets cellar game apps show the real game icon instead of a generic placeholder.
# Needs icoutils (brew install icoutils); sips + iconutil are built into macOS.
#
# Usage:
#   extract-icon.sh <source.exe | game-dir> <out.icns>
#
# If given a directory, uses the largest top-level *.exe.
set -u
SRC="${1:?source .exe or game dir required}"
OUT="${2:?output .icns path required}"

if ! command -v wrestool >/dev/null 2>&1 || ! command -v icotool >/dev/null 2>&1; then
  echo "icoutils not installed (brew install icoutils)" >&2
  exit 2
fi

# Resolve an exe if given a directory (prefer the biggest top-level exe).
EXE="$SRC"
if [ -d "$SRC" ]; then
  name=$(cd "$SRC" && ls -S *.exe 2>/dev/null | head -1)
  [ -n "$name" ] && EXE="$SRC/$name"
fi
[ -f "$EXE" ] || { echo "no exe found at: $SRC" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# RT_GROUP_ICON (type 14) -> .ico -> PNGs -> pick the largest.
wrestool -x -t 14 "$EXE" -o "$work/" 2>/dev/null
for ico in "$work"/*.ico; do [ -f "$ico" ] && icotool -x -o "$work/" "$ico" 2>/dev/null; done
big=$(ls -S "$work"/*.png 2>/dev/null | head -1)
[ -n "$big" ] || { echo "no icon resource in $(basename "$EXE")" >&2; exit 1; }

iconset="$work/icon.iconset"; mkdir -p "$iconset"
for sz in 16 32 128 256 512; do
  sips -z "$sz" "$sz" "$big" --out "$iconset/icon_${sz}x${sz}.png" >/dev/null 2>&1
  d=$((sz*2)); sips -z "$d" "$d" "$big" --out "$iconset/icon_${sz}x${sz}@2x.png" >/dev/null 2>&1
done
iconutil -c icns "$iconset" -o "$OUT" || { echo "iconutil failed" >&2; exit 1; }
src_px=$(sips -g pixelWidth "$big" 2>/dev/null | awk '/pixelWidth/{print $2}')
echo "wrote $OUT (from $(basename "$EXE"), ${src_px}px source)"
