#!/bin/bash
# launch-engine.sh — generic profile-driven cellar launcher
#
# Usage:
#   launch-engine.sh <profile-id> <game-dir-name>
#
# Reads runtime config from cellar/profiles.json:
#   - .profiles[id=<profile-id>].settings.dll_overrides   -> WINEDLLOVERRIDES
#   - .profiles[id=<profile-id>].settings.env             -> env vars
#   - .profiles[id=<profile-id>].settings.launch_args     -> extra args passed to game exe
#   - .profiles[id=<profile-id>].requires                 -> winetricks_* verbs installed on first boot
#
# Creates a per-(profile, game) bottle under ~/.cellar/bottles/<profile-id>-<slug>/prefix
# Resolves the exe case-insensitively from ~/Games-source/<game-dir-name>/
#
# This launcher handles the engine-family generic case. Games that need
# engine-specific config patches (FIFA's fifasetup.ini DIRECTX_SELECT,
# CarX's Proton WinRT + MF codecs, etc.) keep their own dedicated
# launcher scripts and don't go through this path.
#
# Examples:
#   launch-engine.sh frostbite-multi "Need for Speed Heat"
#   launch-engine.sh rage-rockstar "Grand Theft Auto V"
#   launch-engine.sh d3d9-classic "Grand Theft Auto San Andreas"
#   launch-engine.sh unreal-engine-4-5 "Elden Ring"
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROFILES="$CELLAR_ROOT/profiles.json"

# Optional flags before the positional args:
#   --exe NAME  -- skip auto-resolution, launch this exe (relative to game dir)
# Anything after <profile-id> <game-dir> becomes extra args passed to the game,
# concatenated with profile.settings.launch_args.
EXPLICIT_EXE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --exe) EXPLICIT_EXE="${2:?--exe needs a value}"; shift 2 ;;
    --) shift; break ;;
    -h|--help)
      echo "usage: $0 [--exe NAME] <profile-id> <game-dir> [game-args ...]" >&2
      exit 0
      ;;
    -*) echo "unknown flag: $1" >&2; exit 1 ;;
    *) break ;;
  esac
done

PROFILE="${1:?profile id required (see profiles.json for valid ids)}"
GAME="${2:?game dir name required (relative to ~/Games-source/)}"
shift 2
EXTRA_GAME_ARGS=("$@")

# Precheck tooling.
if ! command -v jq >/dev/null 2>&1; then
  echo "jq not found, install with: brew install jq" >&2
  exit 2
fi
if [ ! -f "$PROFILES" ]; then
  echo "profiles.json not found at $PROFILES" >&2
  exit 2
fi
if ! jq -e ".profiles[] | select(.id == \"$PROFILE\")" "$PROFILES" >/dev/null 2>&1; then
  echo "profile id not found: $PROFILE" >&2
  echo "available profiles:" >&2
  jq -r '.profiles[].id' "$PROFILES" >&2
  exit 1
fi

# Extract profile fields.
DLL_OVERRIDES=$(jq -r ".profiles[] | select(.id == \"$PROFILE\") | .settings.dll_overrides // \"\"" "$PROFILES")
LAUNCH_ARGS_JSON=$(jq -c ".profiles[] | select(.id == \"$PROFILE\") | .settings.launch_args // []" "$PROFILES")
REQUIRES_JSON=$(jq -c ".profiles[] | select(.id == \"$PROFILE\") | .requires // []" "$PROFILES")

# Build the env array from profile.settings.env (JSON object -> KEY=VAL strings).
env_extra=()
while IFS= read -r line; do
  [ -z "$line" ] && continue
  env_extra+=("$line")
done < <(jq -r ".profiles[] | select(.id == \"$PROFILE\") | .settings.env // {} | to_entries[] | \"\(.key)=\(.value)\"" "$PROFILES")

# Compute bottle slug.
slug() { echo "$1" | tr 'A-Z ' 'a-z-' | tr -dc 'a-z0-9-' | sed 's/--*/-/g; s/^-//; s/-$//'; }
GAME_SLUG=$(slug "$GAME")
BOTTLE="$PROFILE-$GAME_SLUG"

WINE="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib/wine/x86_64-unix/wine"
WINESERVER="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/CrossOver-Hosted Application/wineserver"
WINETRICKS="$(command -v winetricks || echo /opt/homebrew/bin/winetricks)"
PREFIX="$HOME/.cellar/bottles/$BOTTLE/prefix"
GAME_DIR="$HOME/Games-source/$GAME"
LOG="/tmp/cellar-$BOTTLE.log"
PIDFILE="/tmp/cellar-$BOTTLE.pid"
RES_W=1920
RES_H=1080

pkill -9 -f "wine64-preloader" 2>/dev/null
pkill -9 wineserver 2>/dev/null
sleep 2

mkdir -p "$(dirname "$PREFIX")"

echo "===== launch $(date) =====" > "$LOG"
echo "profile: $PROFILE" >> "$LOG"
echo "game: $GAME" >> "$LOG"
echo "bottle: $BOTTLE" >> "$LOG"
echo "game dir: $GAME_DIR" >> "$LOG"

# Base env shared with the dedicated launchers (CrossOver wine + apple_gptk D3DMetal 3.0).
env_base=(
  "WINEPREFIX=$PREFIX"
  "DYLD_FRAMEWORK_PATH=$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib64/apple_gptk/external"
  "CX_ROOT=$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver"
  "DYLD_LIBRARY_PATH=/opt/homebrew/lib"
  "WINEESYNC=0"
)

# Append profile env on top of the base.
for kv in "${env_extra[@]}"; do
  env_base+=("$kv")
done

# WINEDLLOVERRIDES is set last from the profile so it takes precedence.
if [ -n "$DLL_OVERRIDES" ]; then
  env_base+=("WINEDLLOVERRIDES=$DLL_OVERRIDES")
fi

if [ ! -d "$PREFIX/drive_c" ]; then
  echo "creating fresh wine prefix..." | tee -a "$LOG"
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" wineboot --init >> "$LOG" 2>&1
  env "${env_base[@]}" "$WINESERVER" -w

  # Pull winetricks verbs out of profile.requires (anything starting "winetricks_").
  verbs=()
  while IFS= read -r r; do
    case "$r" in
      winetricks_*) verbs+=("${r#winetricks_}") ;;
    esac
  done < <(jq -r '.[]' <<< "$REQUIRES_JSON")

  if [ ${#verbs[@]} -gt 0 ]; then
    if [ ! -x "$WINETRICKS" ]; then
      echo "winetricks not found at $WINETRICKS, install with: brew install winetricks" >&2
      exit 2
    fi
    if ! command -v cabextract >/dev/null 2>&1; then
      echo "cabextract not found, install with: brew install cabextract" >&2
      echo "(d3dcompiler_47 and several other winetricks verbs need it)" >&2
      exit 2
    fi
    echo "installing winetricks verbs: ${verbs[*]}" | tee -a "$LOG"
    # Each verb tried independently so one failure doesn't abort the rest.
    # dotnet48 in particular is known broken on Apple Silicon prefixes
    # (winetricks #2246, #1792).
    for v in "${verbs[@]}"; do
      echo "  $v..." | tee -a "$LOG"
      env "${env_base[@]}" WINE="$WINE" "$WINETRICKS" -q "$v" >> "$LOG" 2>&1 || \
        echo "  $v failed, continuing" | tee -a "$LOG"
    done
    env "${env_base[@]}" "$WINESERVER" -w
  fi

  # Non-winetricks requires get printed as hints for the user.
  while IFS= read -r r; do
    case "$r" in
      winetricks_*) ;;
      proton_winrt_dlls)
        echo "HINT: profile requires Proton WinRT DLLs. Run:" | tee -a "$LOG"
        echo "  scripts/install-proton-winrt.sh $PREFIX" | tee -a "$LOG"
        ;;
      homebrew_*)
        pkg="${r#homebrew_}"
        echo "HINT: profile requires Homebrew $pkg. Run: brew install $pkg" | tee -a "$LOG"
        ;;
      *) echo "HINT: profile lists requirement '$r' (no automatic handler)" | tee -a "$LOG" ;;
    esac
  done < <(jq -r '.[]' <<< "$REQUIRES_JSON")
fi

# Persist DLL overrides into the registry so subprocesses inherit them even
# if env propagation gets dropped.
if [ -n "$DLL_OVERRIDES" ]; then
  IFS=';' read -ra entries <<< "$DLL_OVERRIDES"
  for entry in "${entries[@]}"; do
    [ -z "$entry" ] && continue
    key="${entry%%=*}"
    val="${entry#*=}"
    IFS=',' read -ra keys <<< "$key"
    for k in "${keys[@]}"; do
      env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
        "HKCU\\Software\\Wine\\DllOverrides" /v "$k" /d "$val" /f >> "$LOG" 2>&1
    done
  done
fi

# Virtual desktop.
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer" /v Desktop /d Default /f >> "$LOG" 2>&1
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer\\Desktops" /v Default \
  /d "${RES_W}x${RES_H}" /f >> "$LOG" 2>&1
env "${env_base[@]}" "$WINESERVER" -w

# Resolve exe case-insensitively. Match the game name first (Frostbite NFS
# Heat ships "NeedForSpeedHeat.exe", GTA V ships "GTA5.exe", etc.), then
# fall back to anything that looks like a game exe at the top of the dir.
if [ ! -d "$GAME_DIR" ]; then
  echo "ERROR: game dir not found: $GAME_DIR" | tee -a "$LOG" >&2
  echo "place a standalone (no launcher, no anti-cheat) build at $GAME_DIR/" >&2
  exit 3
fi
RESOLVED_EXE=""
if [ -n "$EXPLICIT_EXE" ]; then
  if [ -f "$GAME_DIR/$EXPLICIT_EXE" ]; then
    RESOLVED_EXE="$EXPLICIT_EXE"
    echo "explicit exe: $RESOLVED_EXE" >> "$LOG"
  else
    echo "ERROR: --exe $EXPLICIT_EXE not found under $GAME_DIR" | tee -a "$LOG" >&2
    exit 4
  fi
fi
# Search strategy: prefer exes that look like the game name at the top of the
# dir, then fall through to deeper Unreal-style Binaries/Win64/* paths
# (Elden Ring, Hogwarts, Dark Souls etc. all keep their real exe there with
# a launcher at the top). Skip uninstaller / crash-handler / setup helpers.
slug_nosep=$(echo "$GAME_SLUG" | tr -d '-')
skip_re='(^|/)(Uu?[Nn][Ii][Nn][Ss][Tt][Aa][Ll][Ll]|[Cc]rash[Hh]andler|[Cc]rash[Rr]eport|UnityCrashHandler|EAAntiCheat|EALaunchHelper|[Ss]etup|[Cc]onfig|[Bb]enchmark|RGL|[Ll]auncher|[Pp]rerequisite|vc_redist)'
# Pattern × depth matrix. Top-of-dir first because most games keep the
# launcher there. UE Shipping exes live at depth 3 (Binaries/Win64/foo.exe).
# DXVK / Goldberg loaders at deeper paths aren't game binaries; we cap at
# depth 4 and skip the helper names regex above.
if [ -z "$RESOLVED_EXE" ]; then
  candidates=()
  while IFS= read -r f; do
    case "$f" in *$'\n'*) continue ;; esac
    candidates+=("$f")
  done < <(
    cd "$GAME_DIR" && {
      find . -maxdepth 1 -iname "${GAME_SLUG}*.exe" 2>/dev/null
      find . -maxdepth 1 -iname "${slug_nosep}*.exe" 2>/dev/null
      find . -maxdepth 1 -iname "*.exe" 2>/dev/null
      find . -maxdepth 4 -ipath "*/Binaries/Win64/*.exe" 2>/dev/null
      find . -maxdepth 4 -ipath "*/Binaries/Win32/*.exe" 2>/dev/null
      find . -maxdepth 4 -ipath "*/Bin64/*.exe" 2>/dev/null
      find . -maxdepth 4 -ipath "*/bin/*.exe" 2>/dev/null
    } | awk '!seen[$0]++'
  )
  for f in "${candidates[@]}"; do
    base=$(basename "$f")
    if [[ "$f" =~ $skip_re ]]; then continue; fi
    RESOLVED_EXE="${f#./}"
    break
  done
fi
if [ -z "$RESOLVED_EXE" ]; then
  echo "ERROR: no game exe found in $GAME_DIR" | tee -a "$LOG" >&2
  echo "expected something like ${GAME_SLUG}.exe at the top of the dir," >&2
  echo "or <Game>-Win64-Shipping.exe under Binaries/Win64/ for UE titles" >&2
  ls "$GAME_DIR" >&2 || true
  exit 4
fi
echo "resolved exe: $RESOLVED_EXE" >> "$LOG"

# Build launch arg list from profile.settings.launch_args.
launch_args=()
while IFS= read -r a; do
  [ -z "$a" ] && continue
  launch_args+=("$a")
done < <(jq -r '.[]' <<< "$LAUNCH_ARGS_JSON")

cd "$GAME_DIR"
echo "===== game output =====" >> "$LOG"
# Combined args: profile launch_args first, then any extra positional args
# the caller passed after <game-dir> (e.g. -sgadriver=Vulkan from launch-rdr2.sh).
all_args=("${launch_args[@]}" "${EXTRA_GAME_ARGS[@]}")
echo "game args: ${all_args[*]:-(none)}" >> "$LOG"
env "${env_base[@]}" \
  WINEDEBUG=err+all,fixme-all \
  "$WINE" "./$RESOLVED_EXE" "${all_args[@]}" >> "$LOG" 2>&1 &

WPID=$!
echo "$WPID" > "$PIDFILE"
echo "wine pid: $WPID" >> "$LOG"
echo "started $GAME via profile $PROFILE, pid $WPID (log: $LOG)"
echo "to monitor: tail -f $LOG"
echo "to kill:    kill -9 \$(cat $PIDFILE) && pkill -9 wineserver"
