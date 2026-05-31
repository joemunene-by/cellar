#!/bin/bash
# CarX Street via hybrid: cellar's wine-staging 11.8 (has DispatcherQueue)
# + Whisky's D3DMetal.framework loaded via DYLD + Whisky's d3d* forwarder
# DLLs staged into the bottle's system32/syswow64.
set -u
WINE="/Users/ghost/.cellar/wine-staging/Wine Staging.app/Contents/Resources/wine/bin/wine"
WINESERVER="/Users/ghost/.cellar/wine-staging/Wine Staging.app/Contents/Resources/wine/bin/wineserver"
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
  "DYLD_FRAMEWORK_PATH=$WHISKY_LIB/Wine/lib/external"
  "DYLD_LIBRARY_PATH=/opt/homebrew/lib"
  "GST_PLUGIN_PATH=/opt/homebrew/lib/gstreamer-1.0"
  "D3DM_SUPPORT_DXVK_DYLD=1"
  "D3DM_SUPPORT_BUFFER_DEVICE_ADDRESS=1"
  "ROSETTA_ADVERTISE_AVX=1"
  "WINEESYNC=0"
  "WINEDLLOVERRIDES=winemenubuilder.exe=d"
)

if [ ! -d "$PREFIX/drive_c" ]; then
  echo "creating fresh wine prefix on cellar wine-staging 11.8..." | tee -a "$LOG"
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" wineboot --init >> "$LOG" 2>&1
  env "${env_base[@]}" "$WINESERVER" -w
fi

# Stage Whisky's D3DMetal forwarder DLLs into the prefix.
echo "staging Whisky D3DMetal forwarder DLLs..." | tee -a "$LOG"
for dll in d3d9.dll d3d10core.dll d3d11.dll dxgi.dll; do
  if [ -f "$WHISKY_X64/$dll" ]; then
    cp "$WHISKY_X64/$dll" "$PREFIX/drive_c/windows/system32/$dll"
    echo "  64-bit $dll OK" >> "$LOG"
  fi
  if [ -f "$WHISKY_X32/$dll" ]; then
    cp "$WHISKY_X32/$dll" "$PREFIX/drive_c/windows/syswow64/$dll"
    echo "  32-bit $dll OK" >> "$LOG"
  fi
done

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

env "${env_base[@]}" \
  WINEDEBUG=err+all,fixme-all \
  "$WINE" "./$GAME_EXE" >> "$LOG" 2>&1 &

WPID=$!
echo "$WPID" > "$PIDFILE"
echo "wine pid: $WPID" >> "$LOG"
echo "started cellar wine-staging 11.8 + Whisky D3DMetal pid $WPID (log: $LOG)"
echo "to monitor: tail -f $LOG"
echo "to kill:    kill -9 \$(cat $PIDFILE) && pkill -9 wineserver"
