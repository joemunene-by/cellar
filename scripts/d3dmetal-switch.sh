#!/bin/bash
# d3dmetal-switch.sh — pin a specific D3DMetal.framework version to a bottle.
#
# The default cellar runtime uses the D3DMetal version bundled with the
# CrossOver app under ~/.cellar/runtime/. Some games may regress on a
# new D3DMetal version (CarX Street ran clean on 3.0 but had vertex
# glitches on 2.0), so this lets you per-bottle pin a specific version
# without changing the global runtime.
#
# Mechanism: writes ~/.cellar/bottles/<bottle>/d3dmetal-version with
# the chosen version string. launch-engine.sh reads it on launch and
# sets DYLD_FRAMEWORK_PATH to the matching path.
#
# Usage:
#   d3dmetal-switch.sh --list                  # show available versions
#   d3dmetal-switch.sh <bottle>                # show pinned version for bottle
#   d3dmetal-switch.sh <bottle> <version>      # pin version
#   d3dmetal-switch.sh <bottle> default        # unpin (use runtime default)
#
# Known D3DMetal sources cellar can use:
#   CrossOver 26 (3.x): ~/.cellar/runtime/CrossOver.app/.../lib64/apple_gptk/external/
#   Whisky archived (2.x): /Users/<you>/Library/Application Support/com.isaacmarovitz.Whisky/Libraries/Wine/lib/external/
#
# If neither path exists or you've installed a custom one, drop it
# under ~/.cellar/d3dmetal/<version>/D3DMetal.framework and this
# script will pick it up.
set -u

BOTTLES_DIR="$HOME/.cellar/bottles"
LOCAL_VERSIONS_DIR="$HOME/.cellar/d3dmetal"
CROSSOVER_FW="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib64/apple_gptk/external"
WHISKY_FW="$HOME/Library/Application Support/com.isaacmarovitz.Whisky/Libraries/Wine/lib/external"

discover_version_at() {
  local path="$1" label="$2"
  if [ -f "$path/D3DMetal.framework/Resources/Info.plist" ]; then
    v=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
        "$path/D3DMetal.framework/Resources/Info.plist" 2>/dev/null || echo "?")
    printf "  %-12s  %s  (%s)\n" "$v" "$label" "$path"
  fi
}

list_available() {
  echo "available D3DMetal sources:"
  discover_version_at "$CROSSOVER_FW" "(CrossOver bundle)"
  discover_version_at "$WHISKY_FW" "(Whisky archived)"
  if [ -d "$LOCAL_VERSIONS_DIR" ]; then
    for d in "$LOCAL_VERSIONS_DIR"/*/; do
      [ -d "$d" ] || continue
      ver=$(basename "$d")
      discover_version_at "$d" "(local)"
      # If the user-installed dir doesn't have version metadata, still list it.
      if [ ! -f "$d/D3DMetal.framework/Resources/Info.plist" ]; then
        printf "  %-12s  %s  (%s)\n" "$ver" "(local, version unknown)" "$d"
      fi
    done
  fi
}

if [ "${1:-}" = "--list" ] || [ -z "${1:-}" ]; then
  list_available
  [ -z "${1:-}" ] && { echo; echo "usage: $0 <bottle> [<version>|default]"; exit 1; }
  exit 0
fi

BOTTLE="$1"
PIN_FILE="$BOTTLES_DIR/$BOTTLE/d3dmetal-version"

if [ ! -d "$BOTTLES_DIR/$BOTTLE" ]; then
  echo "ERROR: bottle not found: $BOTTLES_DIR/$BOTTLE" >&2
  exit 1
fi

if [ -z "${2:-}" ]; then
  # Show current pinned version.
  if [ -f "$PIN_FILE" ]; then
    echo "$BOTTLE pinned D3DMetal: $(cat "$PIN_FILE")"
  else
    echo "$BOTTLE: no pin (uses runtime default)"
  fi
  exit 0
fi

VERSION="$2"
if [ "$VERSION" = "default" ]; then
  rm -f "$PIN_FILE"
  echo "$BOTTLE: unpinned (will use runtime default)"
  exit 0
fi

# Verify the version actually exists somewhere we can find it.
found=""
for cand in "$LOCAL_VERSIONS_DIR/$VERSION" "$CROSSOVER_FW" "$WHISKY_FW"; do
  if [ -f "$cand/D3DMetal.framework/Resources/Info.plist" ]; then
    v=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
        "$cand/D3DMetal.framework/Resources/Info.plist" 2>/dev/null || echo "")
    if [ "$v" = "$VERSION" ]; then
      found="$cand"
      break
    fi
  fi
done

if [ -z "$found" ]; then
  echo "ERROR: D3DMetal $VERSION not found in any known location." >&2
  echo "drop the framework at: $LOCAL_VERSIONS_DIR/$VERSION/D3DMetal.framework" >&2
  list_available
  exit 1
fi

echo "$VERSION" > "$PIN_FILE"
echo "$BOTTLE: pinned D3DMetal to $VERSION (source: $found)"
echo "Next launch will use DYLD_FRAMEWORK_PATH=$found"
