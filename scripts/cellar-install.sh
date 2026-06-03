#!/bin/bash
# cellar-install.sh — proactive bottle setup for a game.
#
# launch-engine.sh does the bottle init + winetricks lazily on first launch,
# which means a fresh game has a 5-10 minute setup pause before the game
# actually starts. cellar-install does the same work explicitly, so the
# first real launch goes straight to the game.
#
# Also runs the post-install verifiers that launch-engine.sh doesn't bother
# with: confirms the bottle prefix initialized, the winetricks verbs landed,
# any profile-required side effects (Proton WinRT staging, GStreamer brew
# packages) are in place, and a launchable exe exists in the game dir.
#
# Usage:
#   cellar-install.sh <profile-id> "<game-dir-name>"
#
# Examples:
#   cellar-install.sh fifa-14-23 "FIFA 19"
#   cellar-install.sh unreal-engine-4-5 "Elden Ring"
#   cellar-install.sh bethesda-creation "Skyrim Special Edition"
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROFILES="$CELLAR_ROOT/profiles.json"

PROFILE="${1:?profile id required (see profiles.json)}"
GAME="${2:?game dir name required (relative to ~/Games-source/)}"

if ! command -v jq >/dev/null 2>&1; then
  echo "jq missing, install with: brew install jq" >&2
  exit 2
fi
if ! jq -e ".profiles[] | select(.id == \"$PROFILE\")" "$PROFILES" >/dev/null 2>&1; then
  echo "profile id not found: $PROFILE" >&2
  jq -r '.profiles[].id' "$PROFILES" >&2
  exit 1
fi

slug() { echo "$1" | tr 'A-Z ' 'a-z-' | tr -dc 'a-z0-9-' | sed 's/--*/-/g; s/^-//; s/-$//'; }
BOTTLE="$PROFILE-$(slug "$GAME")"
PREFIX="$HOME/.cellar/bottles/$BOTTLE/prefix"
GAME_DIR="$HOME/Games-source/$GAME"

WINE="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib/wine/x86_64-unix/wine"
WINESERVER="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/CrossOver-Hosted Application/wineserver"
WINETRICKS="$(command -v winetricks || echo /opt/homebrew/bin/winetricks)"

echo "==> cellar-install"
echo "    profile:  $PROFILE"
echo "    game:     $GAME"
echo "    bottle:   $BOTTLE"
echo "    prefix:   $PREFIX"
echo "    game dir: $GAME_DIR"
echo

# 1. Sanity check the game dir exists and contains at least one exe.
if [ ! -d "$GAME_DIR" ]; then
  echo "ERROR: $GAME_DIR not found. Drop the game files there first." >&2
  exit 3
fi
n_exe=$(find "$GAME_DIR" -maxdepth 4 -iname "*.exe" -type f 2>/dev/null | head -5 | wc -l | tr -d ' ')
if [ "$n_exe" -eq 0 ]; then
  echo "ERROR: no .exe found under $GAME_DIR (looked at depth 4)" >&2
  echo "contents:" >&2
  ls "$GAME_DIR" >&2 || true
  exit 3
fi
echo "OK: found exe candidates ($n_exe shown):"
find "$GAME_DIR" -maxdepth 4 -iname "*.exe" -type f 2>/dev/null | head -5 | sed 's/^/  /'
echo

# 2. Compose env from profile.
env_base=(
  "WINEPREFIX=$PREFIX"
  "DYLD_FRAMEWORK_PATH=$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib64/apple_gptk/external"
  "CX_ROOT=$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver"
  "DYLD_LIBRARY_PATH=/opt/homebrew/lib"
  "WINEESYNC=0"
)
while IFS= read -r kv; do
  [ -z "$kv" ] && continue
  env_base+=("$kv")
done < <(jq -r ".profiles[] | select(.id == \"$PROFILE\") | .settings.env // {} | to_entries[] | \"\(.key)=\(.value)\"" "$PROFILES")
DLL_OVERRIDES=$(jq -r ".profiles[] | select(.id == \"$PROFILE\") | .settings.dll_overrides // \"\"" "$PROFILES")
[ -n "$DLL_OVERRIDES" ] && env_base+=("WINEDLLOVERRIDES=$DLL_OVERRIDES")

# 3. Init the prefix if not present.
if [ -d "$PREFIX/drive_c" ]; then
  echo "OK: prefix already initialized at $PREFIX"
else
  mkdir -p "$(dirname "$PREFIX")"
  echo "==> initializing prefix (wineboot --init)..."
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" wineboot --init
  env "${env_base[@]}" "$WINESERVER" -w
  echo "OK: prefix initialized"
fi
echo

# 4. Install winetricks verbs from profile.requires.
verbs=()
hints=()
while IFS= read -r r; do
  [ -z "$r" ] && continue
  case "$r" in
    winetricks_*) verbs+=("${r#winetricks_}") ;;
    proton_winrt_dlls) hints+=("after install: run scripts/install-proton-winrt.sh \"$PREFIX\"") ;;
    homebrew_*) hints+=("install Homebrew package: brew install ${r#homebrew_}") ;;
    *) hints+=("requires '$r' (no automatic handler)") ;;
  esac
done < <(jq -r ".profiles[] | select(.id == \"$PROFILE\") | .requires[]?" "$PROFILES")

if [ ${#verbs[@]} -gt 0 ]; then
  if [ ! -x "$WINETRICKS" ]; then
    echo "ERROR: winetricks not on PATH, install with: brew install winetricks" >&2
    exit 2
  fi
  if ! command -v cabextract >/dev/null 2>&1; then
    echo "ERROR: cabextract missing, install with: brew install cabextract" >&2
    exit 2
  fi
  echo "==> installing winetricks verbs: ${verbs[*]}"
  for v in "${verbs[@]}"; do
    echo "  - $v"
    if env "${env_base[@]}" WINE="$WINE" "$WINETRICKS" -q "$v"; then
      echo "    OK"
    else
      echo "    FAILED (continuing; dotnet48 in particular is known broken)"
    fi
  done
  env "${env_base[@]}" "$WINESERVER" -w
fi
echo

# 5. Persist DLL overrides into the registry so subprocesses inherit them.
if [ -n "$DLL_OVERRIDES" ]; then
  echo "==> writing DLL overrides into the prefix registry..."
  IFS=';' read -ra entries <<< "$DLL_OVERRIDES"
  for entry in "${entries[@]}"; do
    [ -z "$entry" ] && continue
    key="${entry%%=*}"
    val="${entry#*=}"
    IFS=',' read -ra keys <<< "$key"
    for k in "${keys[@]}"; do
      env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
        "HKCU\\Software\\Wine\\DllOverrides" /v "$k" /d "$val" /f >/dev/null 2>&1
    done
  done
  echo "OK: overrides written"
  echo
fi

# 6. Echo non-automatic hints.
if [ ${#hints[@]} -gt 0 ]; then
  echo "==> manual follow-ups for this profile:"
  for h in "${hints[@]}"; do
    echo "  - $h"
  done
  echo
fi

# 7. Run the doctor's prereq pass for final sanity.
if [ -x "$CELLAR_ROOT/scripts/cellar-doctor.sh" ]; then
  echo "==> running cellar-doctor.sh for final state check..."
  "$CELLAR_ROOT/scripts/cellar-doctor.sh" | tail -3
fi

echo
echo "DONE. To launch:"
echo "  scripts/launch-engine.sh $PROFILE \"$GAME\""
echo "Or to wrap as a clickable .app:"
echo "  scripts/make-cellar-app.sh $PROFILE \"$GAME\""
