#!/bin/bash
# Build a fresh wine prefix for CarX Street using GPTK 3's wine64,
# then layer all our existing fixes onto it: Proton WinRT for
# DispatcherQueue, vcrun2003, Win7 mode, virtual desktop registry,
# DLL overrides.
set -u
GPTK_APP="$HOME/.cellar/gptk-3/Game Porting Toolkit.app"
WINE="$GPTK_APP/Contents/Resources/wine/bin/wine64"
WINESERVER="$GPTK_APP/Contents/Resources/wine/bin/wineserver"
GPTK_EXTERNAL="$GPTK_APP/Contents/Resources/wine/lib/external"
PREFIX="$HOME/.cellar/bottles/carxstreet-gptk3/prefix"

pkill -9 -f "wine64-preloader" 2>/dev/null
pkill -9 wineserver 2>/dev/null
sleep 2

# Delete any old GPTK 3 prefix and start fresh
rm -rf "$PREFIX"
mkdir -p "$(dirname "$PREFIX")"

env_base=(
  "WINEPREFIX=$PREFIX"
  "DYLD_FRAMEWORK_PATH=$GPTK_EXTERNAL"
  "WINEDLLOVERRIDES=winemenubuilder.exe=d;mf=b;mfplat=b;mfreadwrite=b;mfmediaengine=b;mfsrcsnk=b"
)

echo "=== step 1: wineboot --init under GPTK 3 wine ==="
env "${env_base[@]}" WINEDEBUG=-all "$WINE" wineboot --init 2>&1 | tail -3
env "${env_base[@]}" "$WINESERVER" -w

echo ""
echo "=== step 2: set Windows 7 mode ==="
env "${env_base[@]}" WINEDEBUG=-all "$WINE" winecfg /v win7 2>&1 | tail -3
env "${env_base[@]}" "$WINESERVER" -w

echo ""
echo "=== step 3: virtual desktop registry (1920x1080) ==="
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer" /v Desktop /d Default /f >/dev/null 2>&1
env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
  "HKCU\\Software\\Wine\\Explorer\\Desktops" /v Default /d "1920x1080" /f >/dev/null 2>&1
env "${env_base[@]}" "$WINESERVER" -w
echo "  done"

echo ""
echo "=== step 4: vcrun2003 (msvcr71 etc.) ==="
export PATH=/opt/homebrew/bin:$PATH
env "${env_base[@]}" WINE="$WINE" winetricks -q vcrun2003 2>&1 | tail -3

echo ""
echo "=== step 5: stage GPTK 3 D3DMetal forwarder DLLs ==="
GPTK_X64="$GPTK_APP/Contents/Resources/wine/lib/wine/x86_64-windows"
GPTK_X32="$GPTK_APP/Contents/Resources/wine/lib/wine/i386-windows"
for dll in d3d8.dll d3d8thk.dll d3d9.dll d3d10.dll d3d10_1.dll d3d10core.dll d3d11.dll d3d12.dll dxgi.dll; do
  [ -f "$GPTK_X64/$dll" ] && cp "$GPTK_X64/$dll" "$PREFIX/drive_c/windows/system32/$dll"
  [ -f "$GPTK_X32/$dll" ] && cp "$GPTK_X32/$dll" "$PREFIX/drive_c/windows/syswow64/$dll"
done
echo "  done"

echo ""
echo "=== step 6: stage Proton WinRT DLLs + register DispatcherQueue ==="
# Extracted Proton WinRT files from earlier
PROTON_X64=$(find /tmp/ge-proton -type d -name "x86_64-windows" | head -1)
PROTON_X32=$(find /tmp/ge-proton -type d -name "i386-windows" | head -1)
for dll in coremessaging.dll wintypes.dll twinapi.appcore.dll \
           windows.gaming.input.dll windows.applicationmodel.dll windows.media.dll \
           windows.media.devices.dll windows.media.speech.dll windows.networking.dll \
           windows.networking.connectivity.dll windows.networking.hostname.dll \
           windows.perception.stub.dll windows.ui.dll threadpoolwinrt.dll; do
  [ -f "$PROTON_X64/$dll" ] && cp "$PROTON_X64/$dll" "$PREFIX/drive_c/windows/system32/$dll"
done
# Register DispatcherQueue classes to coremessaging.dll
for cls in Windows.System.DispatcherQueue Windows.System.DispatcherQueueController \
           Windows.System.DispatcherQueueTimer Windows.System.DispatcherQueueShutdownStartingEventArgs; do
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
    "HKLM\\Software\\Microsoft\\WindowsRuntime\\ActivatableClassId\\$cls" \
    /v DllPath /t REG_EXPAND_SZ /d "C:\\windows\\system32\\coremessaging.dll" /f >/dev/null 2>&1
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
    "HKLM\\Software\\Microsoft\\WindowsRuntime\\ActivatableClassId\\$cls" \
    /v ActivationType /t REG_DWORD /d 0 /f >/dev/null 2>&1
done
env "${env_base[@]}" "$WINESERVER" -w
echo "  done"

echo ""
echo "=== step 7: verify ==="
ls "$PREFIX/drive_c/windows/system32/" | grep -iE "(coremessaging|msvcr71|d3d11)" | head -5
echo ""
echo "GPTK 3 prefix ready at $PREFIX"
echo "Next: launch CarX Street.app pointing at /tmp/launch-carxstreet-gptk3-fresh.sh"
