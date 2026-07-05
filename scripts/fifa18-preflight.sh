#!/bin/bash
# FIFA 18 preflight: will this build run under Cellar/Wine on Apple Silicon?
#
# The single thing that decides it is the crack TYPE baked into the build:
#   - Hypervisor / kernel-driver Denuvo bypass (reflex + SimpleSvm/hyperkd):
#     the Denuvo unlock happens via x86 hardware virtualization (VMRUN/VMXON)
#     inside a Ring-0 kernel driver. Wine has no Ring-0, Rosetta has no VT-x/SVM,
#     and an M-series chip has no x86 virtualization at all. HARD WALL, unfixable
#     by any Wine change. This is the FitGirl/DenuvOwO "hypervisor release".
#   - Denuvo fully STRIPPED from the exe (CPY-style, or old non-hypervisor
#     anadius): Denuvo is gone, only the Origin emulator DLL remains, and that
#     runs fine under Wine. This one BOOTS.
#
# This script inspects a game folder and reports which kind it is, so you get a
# go/no-go verdict without burning a launch attempt.
#
# Usage: fifa18-preflight.sh [game_dir]
#   game_dir defaults to ~/Games-source/FIFA 18
set -u

DIR="${1:-$HOME/Games-source/FIFA 18}"

if [ ! -d "$DIR" ]; then
  echo "game dir not found: $DIR" >&2
  exit 2
fi

# Resolve the main exe case-insensitively (FIFA18.exe / fifa18.exe / suffixed).
EXE=""
while IFS= read -r f; do EXE="$f"; break; done \
  < <(cd "$DIR" && find . -maxdepth 1 -iname "fifa18*.exe" 2>/dev/null | sort)
[ -z "$EXE" ] && while IFS= read -r f; do EXE="$f"; break; done \
  < <(cd "$DIR" && find . -maxdepth 1 -iname "fifa*.exe" 2>/dev/null | sort)

echo "==========================================================="
echo " FIFA 18 preflight"
echo " folder: $DIR"
echo " exe:    ${EXE:-<none found>}"
echo "==========================================================="

hv=0   # count of hypervisor-crack markers found
note() { printf '  [%s] %s\n' "$1" "$2"; }

# --- Marker 1: hypervisor kernel drivers -----------------------------------
drv=""
[ -e "$DIR/driver_amd/SimpleSvm.sys" ] && drv="$drv SimpleSvm.sys"
[ -e "$DIR/driver_intel/hyperkd.sys" ] && drv="$drv hyperkd.sys"
if [ -n "$drv" ]; then
  hv=$((hv+1)); note "HYPERVISOR" "kernel driver(s):$drv  (need Ring-0 + x86 VT-x/SVM)"
else
  note "ok" "no hypervisor kernel driver in folder"
fi

# --- Marker 2: reflex loader ------------------------------------------------
if [ -e "$DIR/reflex.dll" ] || [ -e "$DIR/reflex.ini" ]; then
  hv=$((hv+1)); note "HYPERVISOR" "reflex loader present (talks to the hypervisor driver)"
else
  note "ok" "no reflex loader"
fi

# --- Marker 3: reflex traps compiled into the exe ---------------------------
if [ -n "$EXE" ] && strings -a "$DIR/$EXE" 2>/dev/null | grep -q "REFLEX_TRAP"; then
  hv=$((hv+1)); note "HYPERVISOR" "exe contains REFLEX_TRAP hooks (Denuvo welded to the hypervisor)"
elif [ -n "$EXE" ]; then
  note "ok" "exe has no REFLEX_TRAP hooks"
fi

# --- Marker 4: live Denuvo hash in anadius.cfg ------------------------------
if [ -e "$DIR/anadius.cfg" ] && grep -qiE '"DenuvoExeHash"[[:space:]]*"[0-9a-f]{8,}"' "$DIR/anadius.cfg"; then
  hv=$((hv+1)); note "HYPERVISOR" "anadius.cfg lists a live DenuvoExeHash (Denuvo still in the exe)"
fi

# --- Origin emulator (fine either way, just informational) ------------------
if [ -e "$DIR/anadius64.dll" ] || [ -e "$DIR/CryptBase.dll" ]; then
  note "info" "Origin emulator present (works under Wine on its own)"
fi

echo "-----------------------------------------------------------"
if [ "$hv" -gt 0 ]; then
  echo " VERDICT: WILL NOT RUN under Wine on this Mac."
  echo " This is a hypervisor / kernel-driver Denuvo build ($hv marker(s))."
  echo " No Cellar or Wine change can fix it - the block is below Wine"
  echo " (Ring-0 + x86 virtualization hardware that Apple Silicon lacks)."
  echo
  echo " To make FIFA 18 boot here, swap in a Denuvo-STRIPPED build:"
  echo "   - Denuvo removed from the exe (CPY-style, or old non-hypervisor anadius)"
  echo "   - no reflex.dll, no SimpleSvm.sys / hyperkd.sys, no REFLEX_TRAP"
  echo "   - ideally matching build 1.0.57.57320 so the game data still loads"
  echo " Then re-run this preflight - it should come back GREEN - and launch with:"
  echo "   ~/Desktop/cellar/scripts/launch-fifa.sh 18"
  exit 1
else
  echo " VERDICT: LOOKS RUNNABLE (no hypervisor markers found)."
  echo " This appears to be a Denuvo-free / normally-cracked build."
  echo " Launch it with:"
  echo "   ~/Desktop/cellar/scripts/launch-fifa.sh 18"
  echo " (First boot is slow - Frostbite shader compile. Watch /tmp/fifa18.log.)"
  exit 0
fi
