#!/bin/bash
# analyze-log.sh — scan a cellar launcher log for known failure patterns and
# print the matching diagnosis + remediation hint.
#
# The cellar runtime stack (wine + D3DMetal + Apple GPTK) has a small set of
# well-known failure modes that look opaque the first time they hit you and
# then become obvious once you've seen them once. This script encodes the
# recognition so the second-and-after times are zero-cost.
#
# Usage:
#   analyze-log.sh [log-path]
#
# If no path is given, walks /tmp/*.log matching cellar prefixes and
# analyzes the most recently modified one.
set -u

CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

LOG="${1:-}"
if [ -z "$LOG" ]; then
  # Auto-pick most recent cellar log.
  LOG=$(find /tmp -maxdepth 1 -type f \( \
      -name 'cellar-*.log' -o -name 'fifa*.log' -o \
      -name 'carxstreet*.log' -o -name 'nfsmw*.log' -o \
      -name 'rdr2*.log' -o -name 'skyrim*.log' \
    \) -print 2>/dev/null \
    | xargs -I{} stat -f "%m %N" {} 2>/dev/null \
    | sort -rn | head -1 | awk '{$1=""; sub(/^  */,""); print}')
  if [ -z "$LOG" ]; then
    echo "no cellar logs found in /tmp" >&2
    exit 1
  fi
  echo "auto-picked most recent: $LOG"
fi
if [ ! -f "$LOG" ]; then
  echo "log not found: $LOG" >&2
  exit 1
fi

# Pattern -> (label, hint) map. First field is the egrep regex.
# Order matters: more specific patterns first.
patterns=(
  # WinRT activation / Windows.System.DispatcherQueue (modern Unity wall)
  "RoGetActivationFactory.*Windows\\.System\\.DispatcherQueue|Failed to find library: coremessaging|combase.+activation"
  # winemac.drv HWND deadlock (FitGirl installer wall)
  "ACCESS_DENIED.*IOCTL_CONDRV|winemac.drv.*HWND|console.*ACCESS_DENIED"
  # CrossOver wineloader bootstrap miss (the WINEDLLPATH bug we hit on CarX)
  "could not load ntdll\\.so|ntdll\\.so.*No such file|winetemp-.*ntdll"
  # Media Foundation / GStreamer codec wall
  "MFCreateMediaSession.+failed|winegstreamer.+error|WindowsVideoMedia error 0xc00d36bb|CreateObjectFromByteStream"
  # D3DMetal init failure (the CarX vertex glitch / DX12 path crash)
  "D3D11CreateDevice.*0x80004005|E_FAIL|DXGI_ERROR_DEVICE_REMOVED|DXGI_ERROR_DEVICE_HUNG"
  # MoltenVK / Vulkan init failure (DXVK path)
  "vkCreateDevice.*failed|MoltenVK.*not supported|geometryShader.*not supported"
  # EA AntiCheat (FIFA 23)
  "EAAntiCheat|EAAC.+failed|kernel-mode anti-?cheat"
  # Anti-cheat generic (BattlEye, EAC)
  "BattlEye.*service|EasyAntiCheat.*failed|EAC.*kernel|Vanguard"
  # Rockstar Games Launcher / Social Club blocker
  "Rockstar Games Launcher|Social Club.*required|socialclub.dll.*load"
  # Ubisoft Connect blocker
  "Ubisoft Connect.*required|uplay_r1_loader.*not found|UbisoftGameLauncher"
  # FitGirl cls codec deadlock (the cellar v0.2 wall)
  "ClsMain.*-1|cls-.*\\.dll.*load|CreateFileMapping.*ACCESS|named event.*timeout"
  # 32-bit / WoW64 issues
  "wine: cannot find L\"C:\\\\windows\\\\system32\\\\.+\\.exe\"|missing 32-bit|wow64"
  # Steam stub missing
  "steam_api.*not initialized|SteamAPI.+failed|GoldbergStub.*missing"
  # OOM / memory pressure
  "VirtualAlloc.+failed|out of memory|cannot allocate"
  # Generic .NET failures
  "FileNotFoundException.+System\\.|TypeInitializationException"
)
labels=(
  "WinRT DispatcherQueue activation gap (modern Unity / ForzaTech)"
  "winemac.drv HWND lifecycle deadlock (FitGirl Inno Setup)"
  "CrossOver wineloader bootstrap failure (WINEDLLPATH conflict)"
  "Media Foundation codec gap (splash video / cinematics)"
  "D3D11/12 device creation failure (D3DMetal init)"
  "MoltenVK / Vulkan init failure (DXVK path)"
  "EA AntiCheat blocking launch (FIFA 23 retail)"
  "Kernel-mode anti-cheat (BattlEye / EAC / Vanguard)"
  "Rockstar Games Launcher / Social Club required"
  "Ubisoft Connect launcher required"
  "FitGirl cls-*.dll codec chain deadlock (lollypop / lolzi / lolzx)"
  "32-bit / WoW64 path issue"
  "Steam API stub missing"
  "Out of memory / VirtualAlloc failed"
  ".NET assembly missing (run winetricks dotnet48 manually)"
)
hints=(
  "Install Proton WinRT DLLs into the bottle: scripts/install-proton-winrt.sh \"\$PREFIX\". Confirm coremessaging.dll lands in prefix/drive_c/windows/system32/."
  "FitGirl Inno Setup installs hit this on wine 11.x; cellar's path is to use a community pre-installed cracked build instead. The CHANGELOG 'winemac.drv HWND lifecycle deadlock' section covers it."
  "Remove WINEDLLPATH from the launcher env (or unset before invoking wine). CrossOver wineloader uses WINEDLLPATH to find its own ntdll.so, so pointing it at apple_gptk paths breaks the bootstrap."
  "Install winetricks mf + brew install gstreamer gst-libav. Set GST_PLUGIN_PATH=/opt/homebrew/lib/gstreamer-1.0 and DYLD_LIBRARY_PATH=/opt/homebrew/lib in the launcher env."
  "Swap D3DMetal.framework to the latest CrossOver-bundled version (currently 3.x). Force DX11 in the game's own config if it defaults DX12 (FIFA 20-22 use fifasetup.ini DIRECTX_SELECT=0; Frostbite has user.cfg)."
  "Apple Silicon MoltenVK lacks geometryShader, which DXVK requires. Disable DXVK in the profile (dxvk: false) and route D3D11/12 through D3DMetal directly. For RDR2 use -sgadriver=Vulkan as the launch flag."
  "EA AntiCheat is kernel-mode and has zero wine support. Only path is a community offline-EAAC-bypass crack (the community's offline-bypass release). Online play is permanently blocked."
  "Kernel-mode anti-cheat does not run under wine on any platform. Skip the game (Valorant, late Fortnite, CoD MW2+, PUBG online) or use xCloud / GeForce Now."
  "Cracked / standalone build (community-maintained cracked releases) skips Rockstar Launcher entirely. Or use the No_GTAVLauncher exe replacement / ExeIntegrityBypassAgainstRGL.asi plugin alongside the retail exe."
  "Cracked AC / Far Cry builds ship a replacement uplay_r1_loader.dll (UplayR1Unlocker-style replacement DLLs). Make sure it's present in the game dir next to the exe."
  "cls plugin shim IPC is blocked on wine-on-Mac (cellar v0.2 stance). Cannot extract FitGirl repacks that use these codecs. Use a non-FitGirl source: community pre-installed cracked builds."
  "Game is 32-bit only; CrossOver wine 11.0 has WoW64 but the launcher might be using the 64-bit wine binary. Symlink wine to wine64 or use the appropriate Win32 build."
  "Drop a Goldberg (gbe_fork) steam_api64.dll alongside the game exe, or for always-online titles (CarX-style), keep the original cracked steam_api64.dll the release shipped with."
  "Restart the Mac to clear VirtualAlloc fragmentation. If recurring, the game is bumping into Rosetta 2's 4 GB user-address limit on 32-bit games."
  "Re-run winetricks -q dotnet48 against the bottle. dotnet48 is known broken via winetricks on Apple Silicon (winetricks #2246); may need to install manually from MS installer."
)

n=${#patterns[@]}
hits=0

echo "==> analyzing $LOG"
echo "    size: $(du -h "$LOG" | cut -f1)"
echo "    last modified: $(stat -f "%Sm" "$LOG" 2>/dev/null || date -r "$LOG")"
echo

for ((i=0; i<n; i++)); do
  match=$(grep -E "${patterns[$i]}" "$LOG" 2>/dev/null | head -3)
  if [ -n "$match" ]; then
    hits=$((hits + 1))
    echo "[HIT] ${labels[$i]}"
    echo "  Matched lines (first 3):"
    echo "$match" | sed 's/^/    /'
    echo "  Hint: ${hints[$i]}"
    echo
  fi
done

if [ $hits -eq 0 ]; then
  echo "No known failure patterns matched. The log may show a novel issue;"
  echo "tail the file directly: tail -200 \"$LOG\""
else
  echo "---"
  echo "$hits known pattern(s) matched in $LOG"
fi
