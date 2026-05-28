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
  v
Apple Game Porting Toolkit (GPTK)
  - D3D11 / D3D12 -> Metal via D3DMetal
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
cargo tauri dev
```

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

**v0.1 (today):** Tauri 2 + Rust shell. Wine bottle create / list /
remove via shell. Plain installer launch. Manual library entry.
Repack detection (FitGirl / DODI / KaOs / Inno Setup) via folder
heuristics. `archive_peek` exposes the native FreeArc reader to the
UI; UI hook still pending.

**v0.2:** Hybrid wine path for the closed-source CLS plugins
(`lolzi`, `lolzx`, `lolly`, `lollypop`). Loads each `cls-*.dll`
under wine directly, bypassing `unarc.dll`'s outer state machine
(which wedges on the wine-on-Mac console-IOCTL bug — see issue
notes below). Unlocks native extraction for the full FitGirl set.

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
bug. The native FreeArc reader (above) is the planned path forward:
once the hybrid CLS plugin loader lands in v0.2, cellar will route
around `unarc.dll` entirely for FitGirl bins.

Workaround today: use a repack that uses only open codecs
(`storing`, `lzma`, `zstd`) — the existing `fg-arc-x` CLI extracts
those archives directly. For `lollypop`-based FitGirl bins, manual
extraction on a Windows machine (or under bottled wine on Linux
with the older driver) is the only path until v0.2.

### 32-bit Inno Setup stubs

FitGirl repacks use PE32 (32-bit) Inno Setup stubs. They run under
the bundled wine just fine; the deadlock above is independent of
the stub bitness.
