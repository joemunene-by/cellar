#!/bin/bash
# NFS:MW via Whisky wine64 + D3DMetal, with virtual desktop
# set via registry (matches Whisky's own launch pattern).
set -u
WHISKY_LIB="/Users/ghost/Library/Application Support/com.isaacmarovitz.Whisky/Libraries"
WINE="$WHISKY_LIB/Wine/bin/wine64"
WINESERVER="$WHISKY_LIB/Wine/bin/wineserver"
PREFIX="/Users/ghost/.cellar/bottles/nfsmw-whisky/prefix"
GAME_DIR="/Users/ghost/Games-source/Need for Speed - Most Wanted"
GAME_EXE="speed.exe"
LOG=/tmp/nfsmw-whisky.log
PIDFILE=/tmp/nfsmw-whisky.pid
RES_W=1920
RES_H=1080

pkill -9 -f "wine64-preloader" 2>/dev/null
pkill -9 -f speed.exe 2>/dev/null
pkill -9 wineserver 2>/dev/null
sleep 2

mkdir -p "$(dirname "$PREFIX")"

echo "===== launch $(date) =====" > "$LOG"
echo "wine: $WINE" >> "$LOG"
echo "prefix: $PREFIX" >> "$LOG"
echo "virtual desktop: ${RES_W}x${RES_H}" >> "$LOG"

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

# Set the virtual desktop via registry (Whisky-style). This makes ALL
# programs in the prefix launch inside a fixed-size wine desktop window,
# so the game can't trigger NtUserChangeDisplaySettings.
echo "setting virtual desktop registry keys..." | tee -a "$LOG"
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer" /v Desktop /d Default /f >> "$LOG" 2>&1
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer\\Desktops" /v Default \
  /d "${RES_W}x${RES_H}" /f >> "$LOG" 2>&1
env "${env_base[@]}" "$WINESERVER" -w

cd "$GAME_DIR"
echo "===== game output =====" >> "$LOG"

env "${env_base[@]}" \
  WINEDEBUG=err+all,fixme-all \
  "$WINE" "./$GAME_EXE" >> "$LOG" 2>&1 &

WPID=$!
echo "$WPID" > "$PIDFILE"
echo "wine pid: $WPID" >> "$LOG"
echo "started Whisky wine64 pid $WPID (log: $LOG)"
echo "game should appear inside a ${RES_W}x${RES_H} wine desktop window"
echo "to monitor: tail -f $LOG"
echo "to kill:    kill -9 \$(cat $PIDFILE) && pkill -9 wineserver"
