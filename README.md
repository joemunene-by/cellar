# cellar

Mac mini M4 launcher for Windows games. Built for FitGirl repacks and
similar one-click installers that expect a real Windows-shaped
environment.

## what

A native Mac launcher that wraps the rough edges of running Windows
games on Apple Silicon. Wine plus Apple's Game Porting Toolkit (GPTK)
plus DXVK / D3DMetal handle the translation layer. cellar handles the
day-to-day:

- per-game bottles (Wine prefixes) so games never share state
- one-click flow for installer `.exe` files (FitGirl, DODI, KaOs, plain
  Inno Setup, plain MSI)
- library view, launch button, save-game backup
- per-game knobs (DXVK on/off, ESYNC, MSYNC, dpi scale, controller mode)
- repack-aware installer handling: walks Inno Setup wizards
  programmatically when possible, mirrors install progress, retries on
  the common failure modes (CRC mismatch, missing VC runtimes)

cellar is not Whisky, not CrossOver, not Heroic. It is a single-purpose
launcher tuned for Mac M-series plus repack installers. We control the
bottle creation, the GPTK install, the DXVK config, the
installer-handling flow, and the launch loop.

## the runtime stack

```
cellar  (Tauri 2 / Rust + React)
  - per-game bottles, library, install / launch UI
  - native FreeArc reader: archive_peek, no wine for inspection
  v
Wine 11.x  (Gcenx wine-staging / wine-devel, vendored)
  - Windows API translation for the install + launch loop
  - NOT sufficient on its own on M-series; see "wine-only
    is not enough on M-series" in known issues below
  v
Apple Game Porting Toolkit (GPTK)
  - D3D9 / D3D10 / D3D11 / D3D12 -> Metal via D3DMetal
  - required for game launches on Apple Silicon, not optional
  v
Rosetta 2  (system-wide on every M-series Mac)
  - x86_64 -> ARM64 translation
  v
Metal + the M4 GPU
```

## why a new tool

Whisky is the obvious incumbent. It is well-built. cellar exists because:

1. **Repack-aware install flow.** FitGirl installers are interactive
   Inno Setup wizards that fork other extractors mid-stream (zstd,
   srep, freearc). Whisky treats every `.exe` the same; cellar adds
   repack-specific handling, install-progress mirroring, retry on the
   common failure modes.

2. **One launcher per machine, not one bottle per app.** cellar groups
   bottles by game and treats DXVK / GPTK as a runtime selection per
   game, not a global toggle.

3. **Owned codebase.** Whisky moves at its own pace. cellar can take
   the FitGirl-specific shortcuts that would not make sense upstream
   (e.g. auto-trust the wizard's `Next` clicks, prefetch the common VC
   runtimes, ship the known list of game-specific shims).

## prerequisites

- Mac mini M4 (or any Apple Silicon Mac on macOS 14+)
- Rosetta 2:
  ```sh
  softwareupdate --install-rosetta --agree-to-license
  ```
- Xcode Command Line Tools: `xcode-select --install`
- Rust toolchain:
  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source ~/.cargo/env
  ```
- Tauri CLI: `cargo install tauri-cli --version "^2.0"`
- Node 18+ and npm (Vite + React frontend)
- Apple Game Porting Toolkit: run `./scripts/setup-gptk.sh`

## getting going

```sh
git clone https://github.com/joemunene-by/cellar.git
cd cellar
./scripts/setup-gptk.sh    # one-time GPTK + Wine + DXVK install
npm install
cargo tauri dev            # live reload during development
```

For a quick release-mode launch without dev tooling overhead:

```sh
scripts/cellar-launch.sh   # builds frontend + cellar release bin, launches
```

(Verified on macOS 15 / Apple Silicon: 12 MB arm64 binary, all
AppKit / WebKit frameworks link cleanly.)

## layout

```
cellar/
  README.md
  package.json
  vite.config.ts
  tsconfig.json
  index.html              entry HTML for the Tauri webview
  src/                    React frontend
    main.tsx
    App.tsx
    lib/invoke.ts         typed wrappers around tauri invoke()
    components/
      Library.tsx         card grid of installed games
      InstallWizard.tsx   pick + install a new game
      SettingsPane.tsx    GPTK / Wine status + defaults
    styles.css
  src-tauri/              Rust backend
    Cargo.toml
    tauri.conf.json
    build.rs
    icons/
    capabilities/default.json
    src/
      main.rs             Tauri shell, command registration
      wine.rs             bottle create / list / remove
      library.rs          ~/.cellar/library.json read / write
      runtime.rs          GPTK detect, DXVK config, launch_game
      installer.rs        repack detection + install orchestration
      archive.rs          archive_peek (native FreeArc reader)
  freearc-native/         pure-Rust FreeArc reader (own crate)
    src/                  footer, dir, decompress, extract, varint
    src/bin/              fg-arc-ls, fg-arc-files, fg-arc-x, fg-arc-dump
    README.md             reader-specific docs + format spec link
    FreeArc-archive-format.md
  freearc-shim/           PE32 helper that loads unarc.dll under wine
                          (legacy hybrid path, kept for non-supported
                          codec chains until phase-6 lands)
  scripts/
    setup-gptk.sh         one-time install of GPTK + Wine + DXVK
```

## native FreeArc reader

`freearc-native` is a sibling crate (and standalone open-source library)
that parses FreeArc archives end-to-end in pure Rust. No wine, no DLLs,
no FFI. cellar uses it from the renderer via the `archive_peek` Tauri
command to inspect a FitGirl `fg-*.bin` BEFORE running the installer.

What works today:

- footer-first parser (locate descriptor in last 4 KiB, walk back)
- HEADER / DIR / FOOTER control blocks fully parsed; DATA blocks listed
- decoders: `storing`, `lzma:*` (with synthesised header), `zstd[:N]`
- DIRECTORY block: solid-block table, dir names, full file table with
  paths, sizes, mtimes, CRC-32s
- per-file extraction with CRC verification

For the closed-source CLS plugins (`lolzi`, `lolzx`, `lolly`,
`lollypop`) FitGirl uses on the heaviest data blocks, there is a
hybrid path: a tiny PE32 helper (`freearc-cls-host/`) loads the
matching `cls-*.dll` directly via `libloading` and runs `ClsMain`
behind a stdin/stdout pipe, bypassing `unarc.dll`'s console-probe
deadlock entirely. The peek path always works regardless (DIR
blocks use lzma, not lollypop), so users see the file list before
committing to any install.

Setup is one script:

```
scripts/cls-setup.sh                  # build host + stage cls-*.dll
                                       # from any wine bottle
```

It builds `cellar-freearc-cls-host.exe`, scans `~/.cellar/bottles/`
for any `cls-*.dll` that an earlier installer run left in temp,
copies them to `~/.cellar/cls/`, and prints the env vars to add
(`CELLAR_CLS_HOST`, `CELLAR_CLS_DIR`).

Standalone CLIs:

```
fg-arc-ls    <archive>             list footer + control blocks
fg-arc-files <archive>             list every file in the archive
fg-arc-x     <archive> <out-dir>   extract files (skips unsupported codecs)
fg-arc-dump  --kind dir <archive>  decompress one control block + hexdump
```

See `freearc-native/README.md` and `freearc-native/FreeArc-archive-format.md`
for the format spec and reader internals.

## roadmap

**v0.1:** Tauri 2 + Rust shell. Wine bottle create / list / remove.
Plain installer launch. Manual library entry. Repack detection
(FitGirl / DODI / KaOs / Inno Setup) via folder heuristics.

**v0.2 (current):** Pure-Rust FreeArc reader AND writer
(`freearc-native`) covering footer-first parsing, the full
directory block, per-file extraction with CRC, archive creation
via `fg-arc-c`, and decoders for `storing` / `lzma:*` / `zstd`.
Read + write round-trip is verified by a Cargo integration test
(see `writer.rs`) so the open-codec path is provably correct end-
to-end without depending on FitGirl bins. `archive_peek` Tauri
command + UI "Preview contents" pane lets users see what's in a
`fg-*.bin` before committing to install. CLS plugin host
(`freearc-cls-host`) plus dispatch wiring for the hybrid path
covers the *architecture* for the closed codecs (`lolzi`, `lolzx`,
`lolly`, `lollypop`); see "known issues" below for why the round-
trip currently does not complete on wine-on-Mac.

**v0.3:** Polished library. Card grid, last-played, total play time,
launch button, delete game. Per-game settings (DXVK toggle, ESYNC,
MSYNC, dpi, custom env vars).

**v0.4:** Save-game backup. iCloud Drive or any chosen external
path. Per-game schedule. Optional sync of `~/.cellar/library.json`
across machines via a private git mirror.

**v0.5:** Game-specific shims. Catalogue of "this game needs font X,
registry tweak Y, launch arg Z". One-click apply.

## license

MIT (code only). Nothing in this repo distributes games, repacks, or
any copyrighted content. cellar is a launcher; the user supplies the
game files.

## known issues

### wine-only is not enough on M-series (GPTK is required)

Empirically verified on Mac mini M4 / macOS 15.6 / wine-staging 11.8:

- **D3D9 games (e.g. NFS:MW 2005).** Every DXVK build, upstream
  or Mac-targeted (Gcenx, Whisky), ships d3d10/d3d11/dxgi only and
  intentionally omits d3d9 — because DXVK requires Vulkan
  `geometryShader`, and MoltenVK reports that feature as
  unsupported on every Apple Silicon GPU (M1–M4). Falling back
  to wine's builtin `wined3d -> opengl32` path lets speed.exe
  load but it NULL-derefs at startup (page fault at speed+0x2e6d5f
  on its first display-mode enumeration), because wined3d on the
  macOS GL backend returns a mode list the game does not expect.

- **Modern Unity titles with Burst/IL2CPP (e.g. CarX Street).**
  On wine 11.8: deadlocked on the wine `NtAlertThreadByThreadId`
  / `os_sync_wait_on_address` thundering-herd bug, 100%+ CPU
  with no frames. On Whisky's GPTK-patched wine 7.7 + D3DMetal:
  the deadlock is gone, Unity's memory subsystem comes up, but
  the IL2CPP runtime then calls `RoGetActivationFactory` for
  `Windows.System.DispatcherQueue`, which wine 7.7 does not
  implement (added upstream in wine 9.x). Setting the prefix
  to report as Windows 7 does NOT make Unity take a different
  code path — the call is unconditional. Without a newer wine
  (CrossOver 24+ ships wine 9.x with D3DMetal; a future GPTK
  release would update Whisky downstream), modern Unity 2022+
  IL2CPP titles are blocked on M-series.

Both wine-only failures (D3D9 NULL deref, Unity Job-system
thrash) are fixed by **GPTK** (Apple's wine + **D3DMetal**, the
D3D9 / 10 / 11 / 12 to Metal translator). The Unity DispatcherQueue
gap is upstream-wine-wide as of wine 11.x: the WinRT class has
an IDL header (winehq MR !2489) but no working COM implementation
in mainline. Verified empirically with the hybrid path —
cellar wine-staging 11.8 + Whisky's `D3DMetal.framework` +
Whisky's d3d* forwarder DLLs (see
`scripts/launch-carxstreet-hybrid.sh`) — the swapchain renders
and the process survives further than Whisky alone (no Job-system
deadlock thanks to newer wine), but `RoGetActivationFactory` for
`Windows.System.DispatcherQueue` still fails and Unity exits on
the first frame. CrossOver's proprietary patches implement the
class; only that and the native Mac builds work for Unity 2022+
IL2CPP titles today on Apple Silicon. Several recent Unity Mac
ports — including CarX Street itself — now ship a native Apple
Silicon build via Steam, which sidesteps cellar entirely for
those titles.

`scripts/setup-gptk.sh` installs the canonical
`apple/apple/game-porting-toolkit` formula, with Whisky's bundled
wine as a documented fallback when that formula fails (the
upstream Apple tap depends on `openssl@1.1`, which Homebrew has
since dropped from core; the script handles this case
explicitly).

If a `cellar` user tries to launch a game with no GPTK detected,
the installer wizard should surface this requirement and offer
to run `setup-gptk.sh` automatically. Treat plain wine-staging as
a developer-only smoke configuration for the install pipeline,
not as a supported launch runtime.

### winemac.drv HWND lifecycle deadlock (FitGirl installs)

FitGirl repacks use Inno Setup 5.5 + ISDone.dll + unarc.dll +
botva2.dll + cls-*.dll. On wine 11.8 with the Mac driver, unarc.dll
deadlocks inside its console-mode probe state machine after the
first successful progress callback: the IOCTL_CONDRV_GET_MODE call
returns ACCESS_DENIED from the wine macOS console driver, unarc
misreads that as "interactive console", and waits forever on an
event nothing signals.

Tested mitigations (stack patch, comctl32 native, WinXP mode,
virtual desktop, FreeConsole) do not fix the underlying winemac.drv
bug. cellar v0.2 routes around it for archive inspection (see
above), but full extraction of `lollypop`-using FitGirl bins still
needs the workaround below.

### CLS plugin shim IPC blocked on wine-on-Mac

The architecture for the CLS hybrid path is correct and proven end-
to-end: `cls-smoke.sh` confirms wine + `cellar-freearc-cls-host.exe`
+ every staged `cls-*.dll` link and dispatch `ClsMain` cleanly
(`PASS` on all 8 plugins shipped with FitGirl). What does NOT work
is the FitGirl plugin's internal 2-piece design.

Every `cls-*.dll` FitGirl ships is a thin shim that fork-exec's a
sibling worker (`cls-NAME_x64.exe` / `cls-NAME_x86.exe`) and
exchanges data with it via Windows shared-memory IPC (named
`CreateFileMapping` + event handles). On wine 11.x (both Staging and
Devel builds) running on macOS 15 / Apple Silicon, that IPC fails:

- with the real `_x64.exe` sidecar, the shim's `CreateFileMapping`
  call returns a notification "can't open read file mapping" and
  the call hangs waiting on a never-signalled event;
- with `_x86.exe` aliased as `_x64.exe`, the shim spawns it, then
  hangs the same way during the shared-memory handshake;
- with only `_x86.exe` present (real, not aliased), the shim's
  hard-coded `CreateProcess("cls-NAME_x64.exe")` fails and
  `ClsMain(CLS_DECOMPRESS)` returns -1 instantly *with zero
  callbacks fired* — proving the shim aborts before touching its
  decoder.

Verified by tracing every callback dispatch in cls-host
(`CELLAR_CLS_TRACE=1`). Only `CLS_INIT` (which succeeds) and
`CLS_DECOMPRESS` (which immediately returns -1) are reached.

The freearc-shim path (calling `unarc.dll FreeArcExtract` directly,
which is what the FitGirl installer itself does) gets one step
further — it emits a `progress 9/N` callback, then deadlocks at
**0% CPU for 10+ minutes**. unarc has spawned a CLS worker and is
blocked on a kernel event the worker never signals. Same outcome
on wine-staging 11.8 (mac driver) and Whisky wine 7.7. Not slow
processing; pure IPC deadlock.

Workaround today: use a repack that uses only open codecs
(`storing`, `lzma`, `zstd`) — the existing `fg-arc-x` CLI extracts
those archives directly. For `lollypop`-based FitGirl bins, manual
extraction on a Windows machine (or under bottled wine on Linux
with the older driver) is the only path. Re-investigating once
wine 12 or CrossOver 25 ships an Apple-Silicon-clean shared memory
implementation.

### 32-bit Inno Setup stubs

FitGirl repacks use PE32 (32-bit) Inno Setup stubs. They run under
the bundled wine just fine; the deadlock above is independent of
the stub bitness.
