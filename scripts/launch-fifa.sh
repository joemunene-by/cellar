#!/bin/bash
# FIFA 14-23 launcher via cellar's hybrid runtime
# (CrossOver 26 wine 11.0 binary + Apple GPTK D3DMetal 3.0 forwarders
# loaded from CrossOver's lib64/apple_gptk/external).
#
# Engine / API by version:
#   FIFA 14:        Impact engine,   D3D9
#   FIFA 15, 16:    Ignite engine,   D3D11
#   FIFA 17, 18:    Frostbite,       D3D11
#   FIFA 19:        Frostbite,       D3D11 native
#   FIFA 20-22:     Frostbite,       D3D12 default, DX11 fallback (we force DX11)
#   FIFA 23:        Frostbite,       D3D12 only, EAAC kernel-mode (retail)
#
# FIFA 23 caveats:
#   - The retail build ships EA AntiCheat (kernel-mode, no wine support).
#     This launcher relies on the community offline-EAAC-bypass cracks
#     (community pre-installed cracked builds that strip EAAC entirely). For online
#     play EAAC is still a hard wall.
#   - FIFA 23 is D3D12-only (no DX11 fallback in fifa_setup). Frostbite's
#     DX12 path uses bindless + min16float shaders, same class of shader
#     that broke CarX before D3DMetal 3.0. D3DMetal 3.0 fixed CarX, so v23
#     is worth a real try, but treat it as experimental until we have a
#     working boot logged.
set -u

VER="${1:?FIFA version required (14 through 23)}"
case "$VER" in
  14|15|16|17|18|19|20|21|22|23) ;;
  *) echo "unsupported version: $VER (allowed: 14-23)" >&2; exit 1 ;;
esac

WINE="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib/wine/x86_64-unix/wine"
WINESERVER="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/CrossOver-Hosted Application/wineserver"
WINETRICKS="$(command -v winetricks || echo /opt/homebrew/bin/winetricks)"
PREFIX="$HOME/.cellar/bottles/fifa$VER/prefix"
GAME_DIR="$HOME/Games-source/FIFA $VER"
# GAME_EXE is resolved case-insensitively at launch time from GAME_DIR.
GAME_EXE=""
LOG="/tmp/fifa$VER.log"
PIDFILE="/tmp/fifa$VER.pid"
RES_W=1920
RES_H=1080

pkill -9 -f "wine64-preloader" 2>/dev/null
# pkill -f uses regex on BSD/macOS; this catches FIFA<N>.exe regardless of case.
pkill -9 -f "[Ff][Ii][Ff][Aa]${VER}" 2>/dev/null
pkill -9 -f "EAAntiCheat" 2>/dev/null
pkill -9 -f "EALaunchHelper" 2>/dev/null
pkill -9 -f "OriginWebHelperService" 2>/dev/null
pkill -9 wineserver 2>/dev/null
sleep 2

mkdir -p "$(dirname "$PREFIX")"

echo "===== launch $(date) FIFA $VER =====" > "$LOG"
echo "wine: $WINE ($("$WINE" --version 2>&1))" >> "$LOG"
echo "game dir: $GAME_DIR" >> "$LOG"

# Free seized input devices (quit Steam so it releases USB controllers).
if [ -f "$(dirname "$0")/free-input.sh" ]; then
  . "$(dirname "$0")/free-input.sh"
  cellar_free_input "$LOG"
fi

# Base env shared with CarX hybrid (CrossOver wine + apple_gptk D3DMetal 3.0).
# No DXVK: FIFA 14 is D3D9 (routed through D3DMetal directly); 15-19 are D3D11
# native; 20/21/22 default DX12 but we force DX11 via fifasetup.ini in each
# per-version section below; 23 is D3D12-only.
#
# WINEDLLOVERRIDES grammar (wine ntdll/loader.c): "n" native, "b" builtin,
# empty = disabled. The literal token "disabled" is NOT in the grammar and
# silently falls back to default load order (which for nvapi/nvapi64 means
# the wine stub loads). Trailing equals + nothing is how you actually
# disable a DLL via env.
env_base=(
  "WINEPREFIX=$PREFIX"
  "DYLD_FRAMEWORK_PATH=$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib64/apple_gptk/external"
  "CX_ROOT=$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver"
  "DYLD_LIBRARY_PATH=/opt/homebrew/lib"
  "ROSETTA_ADVERTISE_AVX=1"
  "WINEESYNC=0"
  "WINEDLLOVERRIDES=winemenubuilder.exe=;d3d11,d3d12,dxgi,d3d10core=n,b;nvapi,nvapi64="
  "MVK_CONFIG_USE_METAL_PRIVATE_API=1"
  "MVK_CONFIG_USE_METAL_ARGUMENT_BUFFERS=2"
  "MVK_CONFIG_FAST_MATH_ENABLED=1"
)

# Live FPS/frametime overlay via Apple's Metal HUD. Default from ~/.cellar/fps-hud
# (set by scripts/fps-hud.sh); CELLAR_METAL_HUD=0/1 overrides per-launch.
if [ "${CELLAR_METAL_HUD:-$(cat "$HOME/.cellar/fps-hud" 2>/dev/null || echo 1)}" = "1" ]; then
  env_base+=("MTL_HUD_ENABLED=1")
fi

if [ ! -d "$PREFIX/drive_c" ]; then
  echo "creating fresh wine prefix for FIFA $VER..." | tee -a "$LOG"
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" wineboot --init >> "$LOG" 2>&1
  env "${env_base[@]}" "$WINESERVER" -w

  if [ ! -x "$WINETRICKS" ]; then
    echo "winetricks not found at $WINETRICKS, install with: brew install winetricks" >&2
    exit 2
  fi
  # d3dcompiler_47 silently fails without cabextract on a fresh prefix
  # (winetricks issue #1012). Bail loudly if it's missing.
  if ! command -v cabextract >/dev/null 2>&1; then
    echo "cabextract not found, install with: brew install cabextract" >&2
    echo "(d3dcompiler_47 verb fails without it)" >&2
    exit 2
  fi
  echo "installing winetricks deps (vcrun2019, corefonts, d3dcompiler_47)..." | tee -a "$LOG"
  env "${env_base[@]}" WINE="$WINE" "$WINETRICKS" -q vcrun2019 corefonts d3dcompiler_47 >> "$LOG" 2>&1
  env "${env_base[@]}" "$WINESERVER" -w

  # dotnet48 is known broken via winetricks on Apple Silicon prefixes
  # (winetricks #2246 broken download, #1792 arbitrary failures). FIFA does
  # not strictly need .NET 4.8 to launch the game itself, only some EA
  # launcher utilities do, and we are bypassing those. Try it anyway, but
  # do not fail the install if it errors.
  echo "trying dotnet48 (best-effort, ok if it fails)..." | tee -a "$LOG"
  env "${env_base[@]}" WINE="$WINE" "$WINETRICKS" -q dotnet48 >> "$LOG" 2>&1 || \
    echo "dotnet48 install failed, continuing without it" | tee -a "$LOG"
  env "${env_base[@]}" "$WINESERVER" -w
fi

# DLL override registry (mirror of env, persisted for subprocess inheritance).
for k in d3d9 d3d11 d3d10core d3d12 dxgi; do
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
    "HKCU\\Software\\Wine\\DllOverrides" /v "$k" /d native,builtin /f >> "$LOG" 2>&1
done
for k in nvapi nvapi64; do
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
    "HKCU\\Software\\Wine\\DllOverrides" /v "$k" /d "" /f >> "$LOG" 2>&1
done

# Virtual desktop registry (windowed at requested resolution).
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer" /v Desktop /d Default /f >> "$LOG" 2>&1
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer\\Desktops" /v Default \
  /d "${RES_W}x${RES_H}" /f >> "$LOG" 2>&1
env "${env_base[@]}" "$WINESERVER" -w

# Force DX11 via the game's config INI. Only applies to FIFA 20/21/22 where
# fifasetup exposes a DX12-vs-DX11 toggle and DX11 is the safer path under
# D3DMetal. FIFA 14 (D3D9), 15-19 (D3D11 native), and 23 (D3D12-only) don't
# have a DIRECTX_SELECT knob.
#
# File: Documents/FIFA <year>/fifasetup.ini (plaintext INI, NOT installerdata.xml).
# Key:  DIRECTX_SELECT = 0  (0=DX11, 1=DX12)
# This is the same edit the Windows community uses to fix DXGI_ERROR_DEVICE_REMOVED
# on the DX12 path; documented across windowsreport / Steam forum / drivereasy.
case "$VER" in
  20|21|22)
    DOCS="$PREFIX/drive_c/users/$USER/Documents/FIFA $VER"
    mkdir -p "$DOCS"
    INI="$DOCS/fifasetup.ini"
    if [ -f "$INI" ]; then
      if grep -qE "^DIRECTX_SELECT" "$INI"; then
        sed -i.bak -E 's|^DIRECTX_SELECT[[:space:]]*=.*|DIRECTX_SELECT = 0|' "$INI"
        echo "patched DIRECTX_SELECT=0 (DX11) in $INI" >> "$LOG"
      else
        echo "DIRECTX_SELECT = 0" >> "$INI"
        echo "appended DIRECTX_SELECT=0 (DX11) to $INI" >> "$LOG"
      fi
    else
      mkdir -p "$DOCS"
      echo "DIRECTX_SELECT = 0" > "$INI"
      echo "seeded $INI with DIRECTX_SELECT=0 (DX11)" >> "$LOG"
    fi
    ;;
esac

# Sanity check on game files before launching. Exe casing varies across FIFA
# versions (e.g. FIFA 14-17 are lowercase "fifaNN.exe", 18-23 are uppercase
# "FIFANN.exe", and various cracked releases use suffixes like _x64). Resolve
# case-insensitively against the actual files in the game dir, only at launch
# time, rather than hard-coding casing.
if [ ! -d "$GAME_DIR" ]; then
  echo "ERROR: game dir not found: $GAME_DIR" | tee -a "$LOG" >&2
  echo "place a standalone cracked build (no Origin / EA App launcher) (no Origin / EA App) at:" >&2
  echo "  $GAME_DIR/" >&2
  exit 3
fi
# Prefer the literal "FIFA<ver>.exe" if present (case-insensitive); else fall
# back to the first fifa*.exe at the top of the game dir.
RESOLVED_EXE=""
while IFS= read -r f; do
  base=$(basename "$f")
  RESOLVED_EXE="$base"
  break
done < <(cd "$GAME_DIR" && find . -maxdepth 1 -iname "fifa${VER}*.exe" 2>/dev/null | sort)
if [ -z "$RESOLVED_EXE" ]; then
  while IFS= read -r f; do
    base=$(basename "$f")
    RESOLVED_EXE="$base"
    break
  done < <(cd "$GAME_DIR" && find . -maxdepth 1 -iname "fifa*.exe" 2>/dev/null | sort)
fi
if [ -z "$RESOLVED_EXE" ]; then
  echo "ERROR: no fifa*.exe found in $GAME_DIR" | tee -a "$LOG" >&2
  echo "expected something like fifa${VER}.exe or FIFA${VER}.exe at the top of the game dir" >&2
  ls "$GAME_DIR" >&2 || true
  exit 4
fi
GAME_EXE="$RESOLVED_EXE"
echo "resolved exe: $GAME_EXE" >> "$LOG"

cd "$GAME_DIR"
echo "===== game output =====" >> "$LOG"

env "${env_base[@]}" \
  WINEDEBUG=err+all,fixme-all \
  "$WINE" "./$GAME_EXE" >> "$LOG" 2>&1 &

WPID=$!
echo "$WPID" > "$PIDFILE"
echo "wine pid: $WPID" >> "$LOG"
echo "started FIFA $VER on cellar hybrid runtime, pid $WPID (log: $LOG)"
echo "to monitor: tail -f $LOG"
echo "to kill:    kill -9 \$(cat $PIDFILE) && pkill -9 wineserver"
