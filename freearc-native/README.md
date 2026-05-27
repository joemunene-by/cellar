# cellar-freearc-native

Pure-Rust reader for the FreeArc archive container format, the wrapper
FitGirl repacks (`fg-*.bin`) use. The goal is decompression on macOS
and Linux without needing wine at all for archives that only use open
algorithms (`storing`, `lzma`, `lz4`, `zstd`, `srep`). For FitGirl-
custom CLS algorithms (`lolzi`, `lolzx`, `lolly`, `lollypop`,
`lollypop2`) we add a hybrid path that calls the closed-source
`cls-*.dll` plugins via libloading under wine, but bypasses
`unarc.dll`'s state machine that wedges on macOS.

## why this exists

The earlier `freearc-shim` binary (sibling crate) loads `unarc.dll`
directly and calls `FreeArcExtract`. On wine 11.8 + macOS that path
deadlocks inside `unarc.dll`'s console-mode probe state machine, even
with `FreeConsole()` called pre-load; the IOCTL still returns
`ACCESS_DENIED` from the wine macOS console driver and unarc's
recovery branch waits on an event that nothing signals.

The native reader does the archive walk and block routing in Rust,
where we control every code path. For each compressed block we either
call a native Rust decoder (open algorithms) or load the matching
`cls-*.dll` directly without going through unarc's broken outer
loop.

## phases

- **phase 1 (done)** scan: walk the archive, find every `ArC\x01`
  block header, report kind byte and method string. Diagnostic only,
  no decompression. `cargo run --release --bin freearc-scan fg-05.bin`.
- **phase 2 (next)** native decoders: `storing` (memcpy), `lzma`
  (via `lzma-rs`), `lz4`, `zstd`, `srep`. Enough to extract
  CoD-MW3 `fg-05.bin`, which uses only `storing` + `lzma`.
- **phase 3 (after that)** hybrid CLS path: for blocks tagged
  `lolzi` etc., load the corresponding `cls-*.dll` via libloading
  and call its decode entry. Tests against larger FitGirl bins.
- **phase 4 (cellar integration)** swap `installer.rs` to call this
  binary instead of `cellar-freearc.exe` so the GUI install flow
  goes through the native path on macOS.

## scope check on a real archive

```
$ freearc-scan fg-05.bin --verbose
scanning fg-05.bin (9578531 bytes)
found 4 blocks
           0  kind=0x00  method=""
           8  kind=0x02  method="storing"
     9578408  kind=0x06  method="lzma:mfbt4:d1m"
     9578501  kind=0x08  method="lzma:mfbt4:d1m"
```

The bulk of this archive is `storing` (raw) data; the two trailing
LZMA blocks are the file table. Extraction is `tiny LZMA decode +
byte-range copy`, no exotic codecs. CoD MW3's fg-05.bin is a clean
phase-2 target.

## build

```
cargo build --release
```

Pure Rust, no system dependencies. Runs natively on macOS / Linux /
Windows. Wine only enters the picture in phase 3 when we link the
CLS plugin DLLs.
