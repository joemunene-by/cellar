#!/bin/bash
# CarX Street via hybrid: cellar's wine-staging 11.8 (has DispatcherQueue)
# + Whisky's D3DMetal.framework loaded via DYLD + Whisky's d3d* forwarder
# DLLs staged into the bottle's system32/syswow64.
set -u
WINE="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib/wine/x86_64-unix/wine"
WINESERVER="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/CrossOver-Hosted Application/wineserver"
WHISKY_LIB="/Users/ghost/Library/Application Support/com.isaacmarovitz.Whisky/Libraries"
WHISKY_X64="$WHISKY_LIB/Wine/lib/wine/x86_64-windows"
WHISKY_X32="$WHISKY_LIB/Wine/lib/wine/i386-windows"
PREFIX="/Users/ghost/.cellar/bottles/carxstreet-hybrid/prefix"
GAME_DIR="/Users/ghost/Games-source/CarX Street"
GAME_EXE="CarX Street.exe"
LOG=/tmp/carxstreet-hybrid.log
PIDFILE=/tmp/carxstreet-hybrid.pid
RES_W=1920
RES_H=1080

pkill -9 -f "wine64-preloader" 2>/dev/null
pkill -9 -f "CarX Street.exe" 2>/dev/null
pkill -9 -f UnityCrashHandler 2>/dev/null
pkill -9 wineserver 2>/dev/null
sleep 2

mkdir -p "$(dirname "$PREFIX")"

echo "===== launch $(date) =====" > "$LOG"
echo "wine: $WINE ($("$WINE" --version 2>&1))" >> "$LOG"
echo "D3DMetal: $WHISKY_LIB/Wine/lib/external/D3DMetal.framework" >> "$LOG"

env_base=(
  "WINEPREFIX=$PREFIX"
  "DYLD_FRAMEWORK_PATH=$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver/lib64/apple_gptk/external"
  "CX_ROOT=$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver"
  "DYLD_LIBRARY_PATH=/opt/homebrew/lib"
  "GST_PLUGIN_PATH=/opt/homebrew/lib/gstreamer-1.0"
  "D3DM_SUPPORT_DXVK_DYLD=1"
  "D3DM_SUPPORT_BUFFER_DEVICE_ADDRESS=1"
  "ROSETTA_ADVERTISE_AVX=1"
  "WINEESYNC=0"
  "WINEDLLOVERRIDES=winemenubuilder.exe=d;d3d11,d3d12,dxgi,d3d10core=n,b"
  "MVK_CONFIG_USE_METAL_PRIVATE_API=1"
  "MVK_CONFIG_USE_METAL_ARGUMENT_BUFFERS=2"
  "MVK_CONFIG_FAST_MATH_ENABLED=1"
)

if [ ! -d "$PREFIX/drive_c" ]; then
  echo "creating fresh wine prefix on cellar wine-staging 11.8..." | tee -a "$LOG"
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" wineboot --init >> "$LOG" 2>&1
  env "${env_base[@]}" "$WINESERVER" -w
fi

# NOTE: Whisky's D3DMetal forwarder DLLs WERE staged here, but they require
# Whisky's wine 7.7 ABI and fail under cellar's wine 11.8 (D3D11CreateDevice
# returns E_FAIL = 0x80004005). Upstream DXVK 2.7.1 is installed via
# `winetricks dxvk` instead — it goes D3D11 -> Vulkan -> MoltenVK -> Metal
# on the same wine 11.8 that runs the rest of the bottle. DXVK is more
# vertex-format-correct than Unity's native Vulkan path, which is what we
# need to fix the Burst+Rosetta vertex glitches.
echo "DXVK d3d11.dll already in place ($(stat -f %z "$PREFIX/drive_c/windows/system32/d3d11.dll") bytes)" >> "$LOG"

# Tell wine to prefer the staged native d3d* over its builtin.
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\DllOverrides" /v d3d9 /d native,builtin /f >> "$LOG" 2>&1
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\DllOverrides" /v d3d11 /d native,builtin /f >> "$LOG" 2>&1
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\DllOverrides" /v d3d10core /d native,builtin /f >> "$LOG" 2>&1
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\DllOverrides" /v dxgi /d native,builtin /f >> "$LOG" 2>&1

# Virtual desktop registry.
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer" /v Desktop /d Default /f >> "$LOG" 2>&1
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer\\Desktops" /v Default \
  /d "${RES_W}x${RES_H}" /f >> "$LOG" 2>&1
env "${env_base[@]}" "$WINESERVER" -w

# Make sure GfxJobs are off.
BOOT="$GAME_DIR/CarX Street_Data/boot.config"
if [ -f "$BOOT" ] && grep -q "gfx-enable-gfx-jobs=1" "$BOOT"; then
  sed -i.bak \
    -e 's/^gfx-enable-gfx-jobs=1/gfx-enable-gfx-jobs=0/' \
    -e 's/^gfx-enable-native-gfx-jobs=1/gfx-enable-native-gfx-jobs=0/' \
    "$BOOT"
fi

cd "$GAME_DIR"
echo "===== game output =====" >> "$LOG"

# Force the D3D11 backend. v1.11 ships a D3D12/ directory and defaults
# to D3D12 if available; Apple D3DMetal's D3D11 -> Metal path is more
# vertex-correct than its D3D12 -> Metal path under macOS 15, so this
# avoids the metallic-surface vertex glitch on cars + buildings.
# (v1.6 did not ship D3D12 and ran D3D11 by default.)
env "${env_base[@]}" \
  WINEDEBUG=err+all,fixme-all \
  "$WINE" "./$GAME_EXE" -force-d3d11 >> "$LOG" 2>&1 &

WPID=$!
echo "$WPID" > "$PIDFILE"
echo "wine pid: $WPID" >> "$LOG"
echo "started cellar wine-staging 11.8 + Whisky D3DMetal pid $WPID (log: $LOG)"
echo "to monitor: tail -f $LOG"
echo "to kill:    kill -9 \$(cat $PIDFILE) && pkill -9 wineserver"
