# cellar-freearc

A headless wrapper around `unarc.dll`'s `FreeArcExtract` export. Decompresses
FitGirl-style `fg-*.bin` archives under wine without ever rendering a window,
so it bypasses the FitGirl Inno wizard / `botva2.dll` / wine + macOS
`SetWindowSubclass` failure that blocks the normal install path on Apple
Silicon Macs.

## why this exists

FitGirl repacks ship their actual game data inside FreeArc archives (the
`fg-*.bin` files). On Windows, FitGirl's Inno Setup wizard loads
`unarc.dll` + `ISDone.dll` + a chain of `cls-*` plugins and drives the
extraction from inside the wizard's GUI process.

On Apple Silicon, that GUI chain is unreachable. wine 11's `winemac.drv`
returns an HWND that `botva2.dll` can not subclass; the wizard catches the
resulting Win32 error 1400 and aborts before a single file lands on disk.
Native `comctl32`, virtual desktops, Windows XP compat mode, an 8 MB PE
stack patch, every documented winetricks gap fill: tested, none help. The
break is in wine's window-handle lifecycle on macOS, not in something the
caller can configure around.

The decompression itself has nothing to do with windows. `unarc.dll` is a
console DLL that operates on file paths and a progress callback. If you
load it from a process that never asks for a window, none of the wizard
failure ever happens. That is what this binary is.

## what it does

```
cellar-freearc <archive.bin> <output-dir>
```

- Loads `unarc.dll` from its own directory or the system PATH.
- Resolves `FreeArcExtract`.
- Calls `FreeArcExtract(callback, "x", "-y", "-o+", "-dp<outdir>", "--", archive, NULL)`.
- Prints `filename / progress / wrote / error` lines to stdout / stderr.
- Exits 0 on success, 4 if `FreeArcExtract` returned a nonzero code, 3
  if the DLL could not be loaded.

Plugins (`cls-*.dll`, `cls-*.exe`) must be in the same directory as
`unarc.dll` so the standard FreeArc plugin discovery finds them.

## build

This binary must be 32-bit because FitGirl ships a 32-bit `unarc.dll`
and a process can only load DLLs of its own bitness.

### option A: mingw (macOS / Linux)

```
brew install mingw-w64        # macOS
sudo apt install mingw-w64    # Linux

rustup target add i686-pc-windows-gnu
cd freearc-shim
cargo build --release --target i686-pc-windows-gnu
```

### option B: zigbuild (no mingw needed, just Python)

Useful if you do not have homebrew / sudo, or want the build to be
fully user-mode. Verified producing a working 396 KB PE32 binary
on Linux without any system packages.

```
pip install ziglang cargo-zigbuild     # or: cargo install cargo-zigbuild
rustup target add i686-pc-windows-gnu
cd freearc-shim
cargo zigbuild --release --target i686-pc-windows-gnu
```

Either path drops the binary at:
`target/i686-pc-windows-gnu/release/cellar-freearc.exe`

## run

The `unarc.dll` and `cls-*` plugins live inside FitGirl's
`setup-multi*.exe`. Pull them out with `innoextract`:

```
brew install innoextract                                  # or: apt install innoextract
innoextract setup-multi2.exe -d ./payload
ls payload/tmp/    # unarc.dll, ISDone.dll, cls-lolzi.dll, cls-srep_x86.exe, ...
```

Stage the binary and DLLs together, then drive extraction per `fg-*.bin`:

```
mkdir -p ~/.cellar/freearc-staging
cp payload/tmp/*.{dll,exe,ini} ~/.cellar/freearc-staging/
cp target/i686-pc-windows-gnu/release/cellar-freearc.exe ~/.cellar/freearc-staging/
cd ~/.cellar/freearc-staging

for bin in /path/to/repack/fg-*.bin; do
    WINEPREFIX=~/.cellar/bottles/<id>/prefix \
        wine cellar-freearc.exe "$bin" "C:\\Games\\YourGame"
done
```

## how cellar uses this

`cellar` itself can drive this binary as a subprocess from its
`installer.rs` flow. When `installer_detect` recognises a FitGirl repack
(by the `fg-*.bin` naming pattern next to a `setup-multi*.exe`),
`installer_run` can prefer the headless path:

1. `innoextract` the setup exe into the bottle's `drive_c/cellar-payload/`.
2. Copy this binary and `unarc.dll` + `cls-*` next to it.
3. Loop the `fg-*.bin` files through `wine cellar-freearc.exe`.
4. Move the resulting files into the user-chosen install directory.
5. Skip the broken Inno wizard entirely.

That wiring is the next commit after this one. This commit is the
standalone tool.

## scope

What this does:

- Single-archive FreeArc extraction via the standard `unarc.dll`
  + `cls-*` plugin chain. The plugin chain is whatever the
  `arc.ini` next to the binaries declares; we do not interpret
  the orchestration ourselves.

What this does not do, yet:

- DiskFreeSpaceCheck. The Inno wizard does this; if you point
  the tool at a too-small disk it will fail mid-extraction.
- MD5 verification. FitGirl ships `MD5/*.md5` sidecars; the
  Inno wizard verifies them on completion. We do not.
- Multi-stage selective installs. Some FitGirl repacks bundle
  optional language packs in separate `fg-selective-*.bin` files
  with checkbox controls in the wizard. Right now you decide
  which `.bin` files to feed in by which paths you pass on the
  CLI.

All three are reasonable follow-ups once the basic extraction
flow is shown to work end-to-end.

## license

MIT. `unarc.dll` itself is part of FreeArc by Bulat Ziganshin and is
not redistributed here; users extract it from their own legally
acquired installer.
