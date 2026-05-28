# cellar-freearc-native

Pure-Rust reader for the FreeArc archive container format
(`.arc`, FitGirl's `fg-*.bin`). No wine, no DLLs, no FFI.

This crate parses the on-disk structure and decodes the codecs that
have open-source implementations. It does NOT decode the closed-source
CLS plugins FitGirl ships (`lolzi`, `lolzx`, `lolly`, `lollypop`); a
hybrid path can invoke those under wine, but that lives in a separate
crate.

## why this exists

`cellar`'s earlier path loaded `unarc.dll` directly and called
`FreeArcExtract`. On wine 11.8 + macOS that deadlocks inside
`unarc.dll`'s console-mode probe state machine: even with
`FreeConsole()` called pre-load, the IOCTL returns `ACCESS_DENIED`
from wine's mac console driver and unarc waits on an event that
nothing signals. So we route around `unarc.dll`: do the archive walk
ourselves in Rust and dispatch each compressed block to either a
native Rust decoder or a direct `cls-*.dll` call.

## what works today

- Footer-first parsing: locate the descriptor in the last 4 KiB,
  decode it, walk back to the FOOTER block.
- Control-block listing: HEADER, DIR, FOOTER are fully parsed; DATA
  blocks are listed.
- Decompressors:
  - `storing` (memcpy)
  - `lzma:*` with parameters parsed from the method string
  - `zstd[:level]` via the zstd crate
- DIRECTORY block: solid-block table, directory list, full file
  table with paths, sizes, mtimes, and CRC-32s.

## CLIs

```
fg-arc-ls     <archive>             list footer + control blocks
fg-arc-files  <archive>             list every file inside the archive
fg-arc-x      <archive> <out-dir>   extract (skips codecs we can't decode)
fg-arc-c      <input-dir> <archive> create an archive (storing codec)
fg-arc-survey <dir>                 walk a tree, classify each archive by
                                    "extractable today / needs wine / unknown"
fg-arc-dump   --kind dir <archive>  decompress one control block + hexdump
```

`fg-arc-files` works on any FreeArc archive, including FitGirl bins
whose DATA solid blocks use closed-source plugins. Listing files
does not require decoding the file bytes.

`fg-arc-survey` is the fast triage tool: walks a directory, finds
every file with the `ArC\x01` footer signature (cheap last-4-KiB
scan, no false positives on unrelated `*.bin` game-asset files),
classifies each as NATIVE / HYBRID / BLOCKED / BROKEN, prints a
per-file line + summary tally. See `RESULTS.md` for snapshots.

Example on a FitGirl test archive:

```
$ fg-arc-files fg-05.bin
d              0 00000000 solid=0   main
d              0 00000000 solid=0   miles
-          93696 fecb2076 solid=1   miles/mssmp3.asi
-         153088 f5494b3e solid=1   miles/mssvoice.asi
-         109976 f32625a0 solid=1   logo.bmp
...
-           1594 fbd28a4e solid=1   installscript.vdf

30 files, 27638886 bytes original
```

## phases

- Phase 1 — naive forward scanner. Misread the format, replaced.
- Phase 2 — footer-first reader + LZMA decoder for the footer block.
- Phase 3 — DIRECTORY block parser + file table.
- Phase 4 — byte-range extraction with per-file CRC verification.
- Phase 5a — zstd decoder. srep, ppmd still pending.
- Phase 7a — `archive_peek` Tauri command on the cellar side, backed
  by this crate via a path dep. Renderer can list files in any
  FreeArc archive without invoking wine.
- Phase 7b — UI hook on cellar's installer page: "Preview contents"
  button opens a file-list pane via `archive_peek`, codec badges
  show native / hybrid / unsupported per codec.
- Phase 6a — hybrid wine path for single-codec CLS calls
  (`lolzi`, `lolzx`, `lolly`, `lollypop`, `srep`, `delta`). Dispatch
  in `cls_host.rs` shells out to the sibling
  `cellar-freearc-cls-host` PE32 binary under wine. Configured via
  `CELLAR_CLS_HOST` + `CELLAR_CLS_DIR` env vars; falls back cleanly
  to `UnsupportedCompressor` when either is missing.
- Phase 6b (planned) — chain handling. FitGirl's typical method is
  `srep+dispack+delta+lollypop`, applied left-to-right at encode
  time; decode walks the chain right-to-left. The single-codec
  path is the building block.

## references

- FreeArc archive format spec: `FreeArc-archive-format.md` in this
  directory.
- Canonical C++ reference: `ArcStructure.h` from `xredor/unarc`
  (https://github.com/xredor/unarc), which mirrors Bulat-Ziganshin's
  FreeArc archiver source.

## license

MIT.
