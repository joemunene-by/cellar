#!/bin/bash
# will-it-run.sh — will this Windows game run under cellar/Wine on Apple Silicon?
#
# Scans a game folder for the protections that matter under Wine on a Mac and
# gives a go/no-go before you sink time and disk into it. Generalizes
# fifa18-preflight.sh to any game.
#
# HARD WALLS (won't launch at all):
#   - Hypervisor Denuvo bypass (DenuvOwO / reflex): unsigned kernel driver + x86
#     hardware virtualization. Impossible under Wine (no Ring-0), Rosetta (no
#     VT-x/SVM), or Apple Silicon (no x86 virtualization).
#   - Mandatory kernel anti-cheat: EA AntiCheat/Javelin (FC 24/25), Riot Vanguard
#     (boot driver), nProtect GameGuard. Wine loads no kernel drivers.
#
# CAUTION (single-player usually runs; online is blocked):
#   - BattlEye / EasyAntiCheat: bundled by many games for multiplayer only. GTA V
#     is the classic case — single-player runs (often via a -nobattleye launch),
#     online does not.
#   - Ordinary (non-hypervisor) Denuvo: user-mode, usually runs, can be heavy.
#
# Usage: will-it-run.sh <game_dir>
set -u

DIR="${1:-}"
if [ -z "$DIR" ] || [ ! -d "$DIR" ]; then
  echo "usage: will-it-run.sh <game_dir>   (dir missing or unreadable: '${DIR:-}')" >&2
  exit 2
fi

hits_wall=0     # hard-wall marker count
soft=0          # caution marker count
note() { printf '  [%s] %s\n' "$1" "$2"; }

# Collect top-level exes once (case-insensitive), for string scanning.
exes=()
while IFS= read -r f; do exes+=("$f"); done \
  < <(cd "$DIR" && find . -maxdepth 2 -iname "*.exe" 2>/dev/null | sort)

# Does any exe contain a given string? (safe with an empty exe list under set -u)
exe_contains() {
  local needle="$1" e
  [ "${#exes[@]}" -gt 0 ] || return 1
  for e in "${exes[@]}"; do
    if strings -a "$DIR/${e#./}" 2>/dev/null | grep -q "$needle"; then return 0; fi
  done
  return 1
}
# Does a file matching a glob exist anywhere in the tree?
has_file() { find "$DIR" -maxdepth 3 -iname "$1" 2>/dev/null | grep -q .; }

echo "==========================================================="
echo " cellar: will it run?  ($(basename "$DIR"))"
echo " folder: $DIR"
echo " exes:   ${#exes[@]} found"
echo "==========================================================="

# ---- HARD WALL 1: hypervisor Denuvo bypass -------------------------------
# The bypass needs the reflex loader / kernel-driver FILES to run. The exe's
# REFLEX_TRAP hooks are only operative when those files are present; a software
# re-crack (e.g. voices38) leaves the string but neutralizes it in user space
# and runs fine under Wine. So the string is a wall ONLY alongside the files.
hv_files=0
if has_file "SimpleSvm.sys" || has_file "hyperkd.sys"; then
  hits_wall=$((hits_wall+1)); hv_files=1
  note "WALL" "hypervisor kernel driver (SimpleSvm/hyperkd) — needs Ring-0 + x86 VT-x"
fi
if has_file "reflex.dll" || has_file "reflex.ini"; then
  hits_wall=$((hits_wall+1)); hv_files=1
  note "WALL" "reflex loader present (DenuvOwO hypervisor bypass)"
fi
if exe_contains "REFLEX_TRAP"; then
  if [ "$hv_files" -eq 1 ]; then
    note "info" "exe also has REFLEX_TRAP hooks (corroborates the hypervisor files above)"
  else
    soft=$((soft+1))
    note "caution" "exe has REFLEX_TRAP hooks but NO reflex/driver files — looks like a user-space re-crack (voices38-style); should run, confirm by launching"
  fi
fi
if [ -f "$DIR/anadius.cfg" ] && grep -qiE '"DenuvoExeHash"[[:space:]]*"[0-9a-f]{8,}"' "$DIR/anadius.cfg"; then
  hits_wall=$((hits_wall+1)); note "WALL" "anadius.cfg lists a live DenuvoExeHash (Denuvo still in the exe)"
fi

# ---- HARD WALL 2: mandatory kernel anti-cheat / boot drivers -------------
if has_file "EAAntiCheat*" || exe_contains "EAAntiCheat"; then
  hits_wall=$((hits_wall+1)); note "WALL" "EA AntiCheat / Javelin — mandatory kernel anti-cheat, Wine/Proton unsupported"
fi
if has_file "vgk.sys" || has_file "vgc.exe"; then
  hits_wall=$((hits_wall+1)); note "WALL" "Riot Vanguard — boot-time kernel driver, impossible under Wine"
fi
if has_file "GameGuard*" || has_file "npggNT.des" || has_file "nProtect*"; then
  hits_wall=$((hits_wall+1)); note "WALL" "nProtect GameGuard (kernel anti-cheat)"
fi

# ---- CAUTION: online-only anti-cheat (single-player still runs) ----------
if has_file "BEService*.exe" || has_file "BEDaisy.sys" || has_file "BattlEye"; then
  soft=$((soft+1)); note "caution" "BattlEye — online/multiplayer blocked; single-player usually runs (-nobattleye)"
fi
if has_file "EasyAntiCheat*.exe" || has_file "easyanticheat*.sys"; then
  soft=$((soft+1)); note "caution" "EasyAntiCheat — online/multiplayer blocked; single-player usually runs"
fi

# ---- CAUTION: ordinary (user-mode) Denuvo --------------------------------
if [ "$hits_wall" -eq 0 ] && exe_contains "Denuvo"; then
  soft=$((soft+1)); note "caution" "ordinary Denuvo (user-mode) — usually runs, can be heavy/flaky"
fi

# ---- CAUTION: UWP / Xbox app package (finicky under Wine) ----------------
if has_file "AppxManifest.xml" || has_file "MicrosoftGame.config" || has_file "gamelaunchhelper.exe"; then
  soft=$((soft+1)); note "caution" "UWP / Xbox app package — Xbox sign-in + UWP sandboxing make these finicky under Wine (Game Pass builds, e.g. Forza)"
fi

# ---- INFO: rendering path + software-crack signatures --------------------
if exe_contains "d3d12" && ! exe_contains "d3d11"; then
  note "info" "looks DX12-only — cellar runs DX12 via dxmt/D3DMetal, but it is heavier; DX11 titles are smoother"
fi
if [ -f "$DIR/anadius.cfg" ] && grep -qi "voices38" "$DIR/anadius.cfg" 2>/dev/null; then
  note "info" "voices38 software crack (no hypervisor) — the Wine-friendly kind, should run"
fi

echo "-----------------------------------------------------------"
if [ "$hits_wall" -gt 0 ]; then
  echo " VERDICT: WILL NOT RUN under cellar/Wine on this Mac ($hits_wall hard-wall marker(s))."
  echo " The block is a kernel driver / CPU virtualization feature that lives BELOW"
  echo " Wine — no Wine or cellar change reaches it. Get a build without that"
  echo " protection (Denuvo-stripped exe / offline-cracked anti-cheat)."
  exit 1
elif [ "$soft" -gt 0 ]; then
  echo " VERDICT: RUNS, with caveats — see the [caution] notes above."
  echo " Single-player is usually fine; kernel anti-cheat blocks online play, and"
  echo " ordinary Denuvo can be heavy. Launch via the matching cellar launcher, or:"
  echo "   launch-engine.sh <profile> \"$(basename "$DIR")\""
  exit 0
else
  echo " VERDICT: LOOKS RUNNABLE — no hard-wall protections found."
  echo " Launch via the matching cellar launcher, or:"
  echo "   launch-engine.sh <profile> \"$(basename "$DIR")\""
  exit 0
fi
