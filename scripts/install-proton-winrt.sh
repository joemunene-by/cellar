#!/bin/bash
# Stage Proton's Windows.System.dll (which actually implements DispatcherQueue)
# into the carxstreet-hybrid wine prefix so Unity 2022 IL2CPP can find it.
#
# Mainline wine has only the IDL header for DispatcherQueue (winehq MR !2489);
# Proton's wine fork has a working impl. The PE-side dll is a plain PE binary
# that loads fine in wine on macOS, since its imports are standard combase /
# kernel32 / ntdll.
set -u
TARBALL=/tmp/ge-proton.tar.gz
EXTRACT=/tmp/ge-proton
PREFIX="/Users/ghost/.cellar/bottles/carxstreet-hybrid/prefix"
WINE="/Users/ghost/.cellar/wine-staging/Wine Staging.app/Contents/Resources/wine/bin/wine"

if [ ! -f "$TARBALL" ]; then
  echo "no tarball at $TARBALL — download it first"
  exit 1
fi
if [ ! -d "$PREFIX/drive_c" ]; then
  echo "no wine prefix at $PREFIX — run launch-carxstreet-hybrid.sh once first"
  exit 1
fi

echo "extracting GE-Proton wine DLLs (just the WinRT pieces)..."
mkdir -p "$EXTRACT"
tar tzf "$TARBALL" 2>/dev/null | head -3
echo ""
# GE-Proton layout: GE-Proton10-34/files/lib64/wine/x86_64-windows/*.dll
# and             /files/lib/wine/i386-windows/*.dll
# Extract just the wine PE DLL dirs to keep this fast.
tar xzf "$TARBALL" -C "$EXTRACT" --include="*/files/lib*/wine/x86_64-windows/*.dll" --include="*/files/lib*/wine/i386-windows/*.dll" 2>/dev/null || \
tar xzf "$TARBALL" -C "$EXTRACT" 2>/dev/null

X64_DIR=$(find "$EXTRACT" -type d -name "x86_64-windows" | head -1)
X32_DIR=$(find "$EXTRACT" -type d -name "i386-windows" | head -1)
echo "found x64 dir: $X64_DIR"
echo "found x32 dir: $X32_DIR"

if [ -z "$X64_DIR" ]; then
  echo "x86_64-windows dir not found in extracted tarball"
  ls "$EXTRACT" | head
  exit 1
fi

echo ""
echo "WinRT DLLs that GE-Proton ships:"
ls "$X64_DIR" | grep -iE "(windows\.|winrt|wineviron|combase|coremessaging|twinapi|dxcore)" | head -20

# Copy the WinRT family DLLs Unity 2022 typically calls into.
WINRT_DLLS=(
  windows.system.dll
  windows.gaming.input.dll
  windows.media.dll
  windows.media.devices.dll
  windows.media.speech.dll
  windows.networking.dll
  windows.networking.connectivity.dll
  windows.networking.hostname.dll
  windows.perception.stub.dll
  windows.ui.dll
  windows.ui.composition.dll
  windows.ui.xaml.dll
  twinapi.appcore.dll
  coremessaging.dll
  wintypes.dll
  threadpoolwinrt.dll
)

echo ""
echo "staging into prefix..."
copied=0
for dll in "${WINRT_DLLS[@]}"; do
  if [ -f "$X64_DIR/$dll" ]; then
    cp "$X64_DIR/$dll" "$PREFIX/drive_c/windows/system32/$dll"
    copied=$((copied+1))
    echo "  x64 $dll"
  fi
  if [ -n "$X32_DIR" ] && [ -f "$X32_DIR/$dll" ]; then
    cp "$X32_DIR/$dll" "$PREFIX/drive_c/windows/syswow64/$dll"
    echo "  x32 $dll"
  fi
done
echo "staged $copied 64-bit WinRT DLLs"

# Tell wine to prefer native (Proton) over builtin for these.
echo ""
echo "setting DLL overrides..."
WHISKY_LIB="/Users/ghost/Library/Application Support/com.isaacmarovitz.Whisky/Libraries"
env_base=(
  "WINEPREFIX=$PREFIX"
  "DYLD_FRAMEWORK_PATH=$WHISKY_LIB/Wine/lib/external"
  "WINEDLLOVERRIDES=winemenubuilder.exe="
)
for dll in windows.system windows.gaming.input windows.media windows.ui twinapi.appcore coremessaging wintypes threadpoolwinrt; do
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
    "HKCU\\Software\\Wine\\DllOverrides" /v "$dll" /d native,builtin /f 2>/dev/null
done

# Register WinRT activation classes so RoGetActivationFactory finds them.
echo ""
echo "registering WinRT activation classes..."
for cls in \
  "Windows.System.DispatcherQueue:windows.system.dll" \
  "Windows.System.DispatcherQueueController:windows.system.dll" \
  "Windows.System.DispatcherQueueTimer:windows.system.dll"
do
  CLASS_NAME="${cls%%:*}"
  DLL_PATH="${cls##*:}"
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
    "HKLM\\Software\\Microsoft\\WindowsRuntime\\ActivatableClassId\\$CLASS_NAME" \
    /v DllPath /t REG_EXPAND_SZ \
    /d "C:\\windows\\system32\\$DLL_PATH" /f 2>/dev/null
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
    "HKLM\\Software\\Microsoft\\WindowsRuntime\\ActivatableClassId\\$CLASS_NAME" \
    /v ActivationType /t REG_DWORD /d 0 /f 2>/dev/null
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
    "HKLM\\Software\\Microsoft\\WindowsRuntime\\ActivatableClassId\\$CLASS_NAME" \
    /v TrustLevel /t REG_DWORD /d 0 /f 2>/dev/null
  env "${env_base[@]}" WINEDEBUG=-all "$WINE" reg add \
    "HKLM\\Software\\Microsoft\\WindowsRuntime\\ActivatableClassId\\$CLASS_NAME" \
    /v Threading /t REG_DWORD /d 0 /f 2>/dev/null
  echo "  registered $CLASS_NAME -> $DLL_PATH"
done

env "${env_base[@]}" "$WHISKY_LIB/Wine/bin/wineserver" -w 2>/dev/null
"/Users/ghost/.cellar/wine-staging/Wine Staging.app/Contents/Resources/wine/bin/wineserver" -w 2>/dev/null

echo ""
echo "done. Now relaunch CarX Street from Launchpad."
