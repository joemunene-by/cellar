#!/bin/zsh
# Debug run for cellar-freearc.exe: captures wine process/pipe events
# and hard-kills the wine tree after 30s so a subprocess deadlock can't
# eat your entire morning.
#
# Run from a LOCAL Mac Terminal (Aqua session). Trace lands at
# ~/Desktop/cellar-freearc-trace.log.

set -u
WINE=~/.cellar/wine-staging/"Wine Staging.app"/Contents/Resources/wine/bin/wine
WP=~/.cellar/bottles/b3145bc1-ab7d-42a1-bc19-6e7cd5d83641/prefix
SRC=~/Games-source/"Call of Duty - Modern Warfare 3 [FitGirl Repack]"
TRACE=~/Desktop/cellar-freearc-trace.log
TIMEOUT_SEC=30

# Clean any leftover wine processes from earlier runs.
pkill -9 -f "cellar-freearc.exe" 2>/dev/null
pkill -9 -f "cls-.*\.exe" 2>/dev/null
pkill -9 wineserver 2>/dev/null
sleep 1

# Symlink the source into the bottle for a clean C:\ path.
rm -f "$WP/drive_c/CoD-src"
ln -s "$SRC" "$WP/drive_c/CoD-src"
rm -rf "$WP/drive_c/Games/CoD-debug"
mkdir -p "$WP/drive_c/Games/CoD-debug"

echo "SECURITYSESSIONID=$SECURITYSESSIONID (should be non-empty)"
echo "trace -> $TRACE"
echo "running with 30s timeout..."
cd ~/.cellar/freearc-staging

# Launch wine in background, capture stderr to the trace file.
env WINEPREFIX="$WP" \
    WINEDEBUG="+process,+pipe,+server,+module,err+all" \
    "$WINE" cellar-freearc.exe \
        "C:\\CoD-src\\fg-05.bin" \
        "C:\\Games\\CoD-debug" \
    > /tmp/cellar-freearc-stdout 2> "$TRACE" &
WINE_PID=$!

# Watchdog: kill the whole wine tree after TIMEOUT_SEC.
(
    sleep "$TIMEOUT_SEC"
    if kill -0 "$WINE_PID" 2>/dev/null; then
        echo "[watchdog] $TIMEOUT_SEC sec elapsed, killing wine tree"
        pkill -9 -f "cellar-freearc.exe" 2>/dev/null
        pkill -9 -f "cls-.*\.exe" 2>/dev/null
        pkill -9 wineserver 2>/dev/null
    fi
) &
WATCHDOG_PID=$!

wait "$WINE_PID" 2>/dev/null
RC=$?
kill "$WATCHDOG_PID" 2>/dev/null

echo
echo "exit=$RC"
echo
echo "=== stdout (cellar-freearc app output) ==="
cat /tmp/cellar-freearc-stdout 2>/dev/null | grep -vE "mvk|VK_|Vulkan|Apple|Metal|Shading|Family|Tier|GPU|model|type:|memory|pipeline|Read-Write|vendor|device" | head -30
echo
echo "=== trace highlights: process spawn + last 30 lines ==="
grep -E "CreateProcess|StartProcess|process_create|exec_process|CreateFile.*\.exe|exception|err:" "$TRACE" 2>/dev/null | head -20
echo "..."
tail -30 "$TRACE" 2>/dev/null
echo
echo "files in target:"
find "$WP/drive_c/Games/CoD-debug" -type f 2>/dev/null | head
echo "count: $(find "$WP/drive_c/Games/CoD-debug" -type f 2>/dev/null | wc -l)"
echo
echo "trace size: $(wc -l < "$TRACE" 2>/dev/null) lines at $TRACE"
