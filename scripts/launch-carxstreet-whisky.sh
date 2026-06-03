#!/bin/bash
# CarX Street via Whisky wine64 + D3DMetal.
# Same stack that just worked for NFS:MW: Apple's GPTK-patched wine
# + D3DMetal forwarder DLLs + msync. Boot.config gfx-jobs already off
# from the earlier session, kept off (Unity Burst Job-system thrash).
set -u
WHISKY_LIB="/Users/ghost/Library/Application Support/com.isaacmarovitz.Whisky/Libraries"
WINE="$WHISKY_LIB/Wine/bin/wine64"
WINESERVER="$WHISKY_LIB/Wine/bin/wineserver"
PREFIX="/Users/ghost/.cellar/bottles/carxstreet-whisky/prefix"
GAME_DIR="/Users/ghost/Games-source/CarX Street"
GAME_EXE="CarX Street.exe"
LOG=/tmp/carxstreet-whisky.log
PIDFILE=/tmp/carxstreet-whisky.pid
RES_W=1920
RES_H=1080

pkill -9 -f "wine64-preloader" 2>/dev/null
pkill -9 -f "CarX Street.exe" 2>/dev/null
pkill -9 -f UnityCrashHandler 2>/dev/null
pkill -9 wineserver 2>/dev/null
sleep 2

mkdir -p "$(dirname "$PREFIX")"

echo "===== launch $(date) =====" > "$LOG"
echo "wine: $WINE" >> "$LOG"
echo "prefix: $PREFIX" >> "$LOG"

env_base=(
  "WINEPREFIX=$PREFIX"
  "DYLD_FRAMEWORK_PATH=$WHISKY_LIB/Wine/lib/external"
  "D3DM_SUPPORT_DXVK_DYLD=1"
  "D3DM_SUPPORT_BUFFER_DEVICE_ADDRESS=1"
  "ROSETTA_ADVERTISE_AVX=1"
  "WINEMSYNC=1"
  "WINEDLLOVERRIDES=winemenubuilder.exe="
)

if [ ! -d "$PREFIX/drive_c" ]; then
  echo "creating fresh wine prefix..." | tee -a "$LOG"
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" wineboot --init >> "$LOG" 2>&1
  env "${env_base[@]}" "$WINESERVER" -w
  echo "prefix ready" | tee -a "$LOG"
fi

# Virtual desktop registry (same trick we used for NFS:MW).
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer" /v Desktop /d Default /f >> "$LOG" 2>&1
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer\\Desktops" /v Default \
  /d "${RES_W}x${RES_H}" /f >> "$LOG" 2>&1
env "${env_base[@]}" "$WINESERVER" -w

# Verify boot.config still has gfx-jobs disabled.
BOOT="$GAME_DIR/CarX Street_Data/boot.config"
if [ -f "$BOOT" ] && grep -q "gfx-enable-gfx-jobs=1" "$BOOT"; then
  echo "boot.config has GfxJobs enabled, disabling..." | tee -a "$LOG"
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
echo "started Whisky wine64 pid $WPID (log: $LOG)"
echo "to monitor: tail -f $LOG"
echo "to kill:    kill -9 \$(cat $PIDFILE) && pkill -9 wineserver"
