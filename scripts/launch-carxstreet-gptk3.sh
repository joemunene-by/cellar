#!/bin/bash
# CarX Street via Apple GPTK 3.0-3 — the canonical Apple-blessed path.
# Uses GPTK 3's wine64 (wine 7.7 base, Apple-patched) + GPTK 3 D3DMetal
# (latest, 63 MB) for vertex-correct Unity rendering. Reuses the existing
# carxstreet-hybrid prefix (still has our Proton WinRT install for
# DispatcherQueue, native mf.dll, MF codecs).
set -u
GPTK_APP="$HOME/.cellar/gptk-3/Game Porting Toolkit.app"
WINE="$GPTK_APP/Contents/Resources/wine/bin/wine64"
WINESERVER="$GPTK_APP/Contents/Resources/wine/bin/wineserver"
GPTK_X64="$GPTK_APP/Contents/Resources/wine/lib/wine/x86_64-windows"
GPTK_X32="$GPTK_APP/Contents/Resources/wine/lib/wine/i386-windows"
GPTK_EXTERNAL="$GPTK_APP/Contents/Resources/wine/lib/external"
PREFIX="$HOME/.cellar/bottles/carxstreet-hybrid/prefix"
GAME_DIR="/Users/ghost/Games-source/CarX Street"
GAME_EXE="CarX Street.exe"
LOG=/tmp/carxstreet-gptk3.log
PIDFILE=/tmp/carxstreet-gptk3.pid
RES_W=1920
RES_H=1080

pkill -9 -f "wine64-preloader" 2>/dev/null
pkill -9 -f "CarX Street.exe" 2>/dev/null
pkill -9 wineserver 2>/dev/null
sleep 2

echo "===== launch $(date) =====" > "$LOG"
echo "wine: $WINE ($("$WINE" --version 2>&1 | head -1))" >> "$LOG"
echo "prefix: $PREFIX" >> "$LOG"
echo "D3DMetal: $GPTK_EXTERNAL/D3DMetal.framework" >> "$LOG"
echo "===== game output =====" >> "$LOG"

env_base=(
  "WINEPREFIX=$PREFIX"
  "DYLD_FRAMEWORK_PATH=$GPTK_EXTERNAL"
  # NOTE: omit DYLD_LIBRARY_PATH + GST_PLUGIN_PATH for GPTK 3.
  # GPTK 3's wine 7.7 actively enumerates GStreamer plugins on startup;
  # Homebrew GStreamer is arm64-native and the wine64 binary is x86_64
  # under Rosetta, so every plugin load fails with arch-mismatch and
  # wineserver crashes before Unity reaches Vulkan/D3DMetal init.
  # cellar wine 11.8 doesn't have this problem because it does not try
  # to load GStreamer at startup the same way. Video decode falls back
  # to wine's internal handling; splash video may glitch but the game
  # will at least reach the renderer.
  "D3DM_SUPPORT_DXVK_DYLD=1"
  "D3DM_SUPPORT_BUFFER_DEVICE_ADDRESS=1"
  "ROSETTA_ADVERTISE_AVX=1"
  "WINEESYNC=0"
  "WINEDLLOVERRIDES=winemenubuilder.exe=d;mf=b;mfplat=b;mfreadwrite=b;mfmediaengine=b;mfsrcsnk=b"
  "MVK_CONFIG_USE_METAL_PRIVATE_API=1"
  "MVK_CONFIG_USE_METAL_ARGUMENT_BUFFERS=2"
)

# Stage GPTK 3's full d3d* forwarder set into the prefix (overwrites Whisky's older ones)
echo "staging GPTK 3 d3d* forwarders..." | tee -a "$LOG"
for dll in d3d8.dll d3d8thk.dll d3d9.dll d3d10.dll d3d10_1.dll d3d10core.dll d3d11.dll d3d12.dll dxgi.dll; do
  if [ -f "$GPTK_X64/$dll" ]; then
    cp "$GPTK_X64/$dll" "$PREFIX/drive_c/windows/system32/$dll"
  fi
  if [ -f "$GPTK_X32/$dll" ]; then
    cp "$GPTK_X32/$dll" "$PREFIX/drive_c/windows/syswow64/$dll"
  fi
done
echo "done staging" >> "$LOG"

cd "$GAME_DIR"
env "${env_base[@]}" \
  WINEDEBUG=err+all,fixme-all \
  "$WINE" "./$GAME_EXE" >> "$LOG" 2>&1 &

WPID=$!
echo "$WPID" > "$PIDFILE"
echo "wine pid: $WPID" >> "$LOG"
echo "started GPTK 3 + D3DMetal pid $WPID (log: $LOG)"
echo "to monitor: tail -f $LOG"
echo "to kill:    kill -9 \$(cat $PIDFILE) && pkill -9 wineserver"
