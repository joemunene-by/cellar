# cellar-freearc-cls-host

Tiny PE32 host that loads one FreeArc CLS compression plugin DLL
and runs one decompress call. Built for the FitGirl path where the
closed-source plugins (`cls-lolzi.dll`, `cls-lolzx.dll`,
`cls-lolly.dll`, `cls-lollypop.dll`) need to run, but `unarc.dll`'s
outer state machine deadlocks on wine-on-Mac.

## what it does

```
echo "compressed_bytes" | wine cellar-freearc-cls-host.exe \
    --dll C:\\path\\to\\cls-lolly.dll \
    --params "d2g:al1"  > decompressed.bin
```

Reads compressed bytes on stdin, writes decompressed bytes on
stdout. One process invocation per solid block. The `--params`
string is the bit after the codec name in a FreeArc method string
(e.g. for `lolly:d2g:al1` you pass `--params d2g:al1`).

Exit codes:
- `0` — wrote decompressed bytes to stdout
- `2` — failed to read stdin
- `3` — `--params` contains an interior NUL
- `4` — `LoadLibrary` failed (DLL missing / wrong bitness)
- `5` — `GetProcAddress(ClsMain)` failed (DLL is not a CLS plugin)
- `6` — failed to write stdout
- `1..=127` (other) — `ClsMain(CLS_DECOMPRESS)` returned non-zero

## why it works where unarc.dll does not

`unarc.dll` has a console-mode probe inside `FreeArcExtract`: it
calls `IOCTL_CONDRV_GET_MODE` to figure out if stdout is an
interactive console, then waits on an event that controls the
progress-bar UI. On wine 11 with the macOS console driver the
IOCTL returns `ACCESS_DENIED`, unarc misreads that as "interactive
console", and waits forever on an event nothing signals.

The CLS plugins themselves never touch the console — they just
talk to the callback table we hand them. So loading them directly
sidesteps the wedged outer loop.

## CLS ABI (single-export, callback-driven)

One function per DLL: `ClsMain(int operation, ClsCallback cb, void* ctx)`,
all `extern "C"` cdecl. The plugin uses the callback for every IO
and memory request. Reference: `Compression/_CLS/cls.h` in the
`xredor/unarc` source tree on GitHub.

## build

```
./build.sh
```

Cross-compiles to `target/i686-pc-windows-gnu/release/cellar-freearc-cls-host.exe`.
Needs mingw-w64 (`brew install mingw-w64` on Mac, `apt install
mingw-w64` on Linux) plus `rustup target add i686-pc-windows-gnu`.

## license

MIT. Does not ship any cls-*.dll — the user supplies those from
their existing FitGirl install.
