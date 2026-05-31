#!/bin/bash
# CarX Street via GPTK 3.0-3 with a FRESH prefix built by GPTK 3's
# own wineboot (not cellar wine 11.8). All our fixes layered in by
# setup-gptk3-prefix.sh: Proton WinRT for DispatcherQueue, vcrun2003,
# Win7 mode, virtual desktop, D3DMetal forwarders, MF builtin override.
set -u
GPTK_APP="$HOME/.cellar/gptk-3/Game Porting Toolkit.app"
WINE="$GPTK_APP/Contents/Resources/wine/bin/wine64"
GPTK_EXTERNAL="$GPTK_APP/Contents/Resources/wine/lib/external"
PREFIX="$HOME/.cellar/bottles/carxstreet-gptk3/prefix"
GAME_DIR="/Users/ghost/Games-source/CarX Street"
GAME_EXE="CarX Street.exe"
LOG=/tmp/carxstreet-gptk3-fresh.log
PIDFILE=/tmp/carxstreet-gptk3-fresh.pid

pkill -9 -f "wine64-preloader" 2>/dev/null
pkill -9 -f "CarX Street.exe" 2>/dev/null
pkill -9 wineserver 2>/dev/null
sleep 2

echo "===== launch $(date) =====" > "$LOG"
echo "wine: $WINE" >> "$LOG"
echo "wine version: $("$WINE" --version 2>&1)" >> "$LOG"
echo "prefix: $PREFIX (fresh GPTK 3 build)" >> "$LOG"
echo "D3DMetal: $GPTK_EXTERNAL/D3DMetal.framework (63 MB)" >> "$LOG"
echo "===== game output =====" >> "$LOG"

env_base=(
  "WINEPREFIX=$PREFIX"
  "DYLD_FRAMEWORK_PATH=$GPTK_EXTERNAL"
  "ROSETTA_ADVERTISE_AVX=1"
  "WINEESYNC=0"
  "WINEDLLOVERRIDES=winemenubuilder.exe=d;mf=b;mfplat=b;mfreadwrite=b;mfmediaengine=b;mfsrcsnk=b"
  "MVK_CONFIG_USE_METAL_PRIVATE_API=1"
)

cd "$GAME_DIR"
env "${env_base[@]}" \
  WINEDEBUG=err+all,fixme-all \
  "$WINE" "./$GAME_EXE" >> "$LOG" 2>&1 &

WPID=$!
echo "$WPID" > "$PIDFILE"
echo "wine pid: $WPID" >> "$LOG"
echo "started GPTK 3 fresh-prefix pid $WPID (log: $LOG)"
echo "to monitor: tail -f $LOG"
echo "to kill:    kill -9 \$(cat $PIDFILE) && pkill -9 wineserver"
