#!/bin/bash
# dxmt-switch.sh — opt a single bottle into the DXMT graphics backend (experimental).
#
# WHAT / WHY
#   cellar runs DirectX games through D3DMetal (Apple's Game Porting Toolkit),
#   which is the right default and is what the current games use. The CrossOver
#   26 runtime cellar ships ALSO bundles DXMT — a newer Metal-native Direct3D 11
#   implementation (github.com/3Shain/dxmt). For some DX11 titles DXMT holds a
#   steadier frame-rate than D3DMetal, which carries GPU-synchronisation
#   overhead on Apple Silicon. This tool switches ONE bottle to DXMT, per-bottle
#   and reversibly, without touching launch-engine.sh or any other game.
#
# NOT FOR UNREAL ENGINE 4 (yet)
#   DXMT's D3D11 *deferred context* support is still incomplete, and UE4 titles
#   (Assetto Corsa Competizione, Ready or Not) drive rendering through deferred
#   contexts — they should stay on D3DMetal until DXMT lands that. Good
#   candidates are non-UE DX11 games where D3DMetal frame-times spike.
#
# MECHANISM (mirrors scripts/d3dmetal-switch.sh's per-bottle, marker-file approach)
#   on:  copies DXMT's d3d11/d3d10core/dxgi/winemetal DLLs into the bottle
#        (system32 = 64-bit, syswow64 = 32-bit), backing up whatever it replaces
#        to <dll>.d3dmetal-bak; drops winemetal.so into the runtime wine unix
#        lib dir so the paired winemetal.dll can load (shared, inert for bottles
#        that don't load it); writes <bottle>/graphics-backend = dxmt as a marker.
#        The profile's existing d3d11/dxgi "native,builtin" override then loads
#        DXMT's native DLLs from the bottle instead of D3DMetal's framework ones.
#   off: restores the .d3dmetal-bak backups (or removes the DXMT DLLs) so the
#        D3DMetal framework takes back over, and clears the marker.
#
# USAGE
#   dxmt-switch.sh <bottle> on        # switch bottle to DXMT
#   dxmt-switch.sh <bottle> off       # revert to D3DMetal (default)
#   dxmt-switch.sh <bottle> status    # show the bottle's current backend
#   dxmt-switch.sh --list             # list every bottle and its backend
set -u

CX="$HOME/.cellar/runtime/CrossOver.app/Contents/SharedSupport/CrossOver"
DXMT_WIN64="$CX/lib/dxmt/x86_64-windows"
DXMT_WIN32="$CX/lib/dxmt/i386-windows"
DXMT_SO="$CX/lib/dxmt/x86_64-unix/winemetal.so"
WINE_UNIX_DIR="$CX/lib/wine/x86_64-unix"
BOTTLES_DIR="$HOME/.cellar/bottles"
# DLLs DXMT provides in both arches; winemetal is DXMT's Metal bridge.
DXMT_DLLS="d3d11 d3d10core dxgi winemetal"

die() { echo "ERROR: $*" >&2; exit 1; }

backend_of() {
  local marker="$BOTTLES_DIR/$1/graphics-backend"
  [ -f "$marker" ] && cat "$marker" || echo "d3dmetal"
}

list_bottles() {
  echo "backend    bottle"
  echo "-------    ------"
  for b in "$BOTTLES_DIR"/*/; do
    [ -d "$b" ] || continue
    local name; name=$(basename "$b")
    printf "%-9s  %s\n" "$(backend_of "$name")" "$name"
  done
}

copy_arch() {
  # copy_arch <src-dir> <dst-system-dir>
  local src="$1" dst="$2"
  [ -d "$dst" ] || return 0
  for dll in $DXMT_DLLS; do
    [ -f "$src/$dll.dll" ] || continue
    if [ -f "$dst/$dll.dll" ] && [ ! -f "$dst/$dll.dll.d3dmetal-bak" ]; then
      cp -p "$dst/$dll.dll" "$dst/$dll.dll.d3dmetal-bak"   # preserve original once
    fi
    cp -f "$src/$dll.dll" "$dst/$dll.dll"
  done
}

restore_arch() {
  local dst="$1"
  [ -d "$dst" ] || return 0
  for dll in $DXMT_DLLS; do
    if [ -f "$dst/$dll.dll.d3dmetal-bak" ]; then
      mv -f "$dst/$dll.dll.d3dmetal-bak" "$dst/$dll.dll"   # put the original back
    else
      rm -f "$dst/$dll.dll"                                 # nothing here before DXMT
    fi
  done
}

enable_dxmt() {
  local pfx="$BOTTLES_DIR/$BOTTLE/prefix"
  [ -d "$pfx/drive_c" ] || die "bottle prefix not found: $pfx"
  [ -d "$DXMT_WIN64" ] || die "DXMT not found in runtime ($DXMT_WIN64) — needs CrossOver 26+"
  copy_arch "$DXMT_WIN64" "$pfx/drive_c/windows/system32"
  copy_arch "$DXMT_WIN32" "$pfx/drive_c/windows/syswow64"
  # winemetal.so is paired with winemetal.dll by name from the wine unix lib dir.
  if [ -f "$DXMT_SO" ] && [ ! -f "$WINE_UNIX_DIR/winemetal.so" ]; then
    cp -f "$DXMT_SO" "$WINE_UNIX_DIR/winemetal.so"
  fi
  echo "dxmt" > "$BOTTLES_DIR/$BOTTLE/graphics-backend"
  echo "$BOTTLE: switched to DXMT (experimental). Relaunch to apply."
  echo "  revert any time with: dxmt-switch.sh $BOTTLE off"
}

disable_dxmt() {
  local pfx="$BOTTLES_DIR/$BOTTLE/prefix"
  restore_arch "$pfx/drive_c/windows/system32"
  restore_arch "$pfx/drive_c/windows/syswow64"
  rm -f "$BOTTLES_DIR/$BOTTLE/graphics-backend"
  echo "$BOTTLE: reverted to D3DMetal (default). Relaunch to apply."
}

# ---- args ----
if [ "${1:-}" = "--list" ]; then list_bottles; exit 0; fi
BOTTLE="${1:-}"
ACTION="${2:-status}"
[ -n "$BOTTLE" ] || { echo "usage: $0 <bottle> on|off|status   |   $0 --list" >&2; exit 2; }
[ -d "$BOTTLES_DIR/$BOTTLE" ] || die "no such bottle: $BOTTLE (see: $0 --list)"

case "$ACTION" in
  on|dxmt)       enable_dxmt ;;
  off|d3dmetal)  disable_dxmt ;;
  status)        echo "$BOTTLE: $(backend_of "$BOTTLE")" ;;
  *)             echo "unknown action: $ACTION (use on|off|status)" >&2; exit 2 ;;
esac
