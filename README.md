<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo-dark.svg">
    <img src="assets/logo.svg" alt="cellar" width="280">
  </picture>
</p>

<h1 align="center">cellar</h1>

<p align="center">

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-macOS%2015%20%C2%B7%20Apple%20Silicon-black)](https://www.apple.com/macos/)
[![Wine](https://img.shields.io/badge/wine-CrossOver%2026%20%C2%B7%2011.0-purple)](https://www.codeweavers.com/crossover)
[![D3DMetal](https://img.shields.io/badge/D3DMetal-3.0-success)](https://developer.apple.com/games/game-porting-toolkit/)
[![Built with](https://img.shields.io/badge/built%20with-Tauri%202%20%C2%B7%20Rust%20%C2%B7%20React-orange)](https://tauri.app)
[![Status](https://img.shields.io/badge/status-alpha-yellow)](#known-issues)
[![Last commit](https://img.shields.io/github/last-commit/joemunene-by/cellar)](https://github.com/joemunene-by/cellar/commits/main)

</p>

Mac mini M4 launcher for Windows games. Drop a game under
`~/Games-source/<Name>/`, pick the matching engine-family profile, get a
clickable `.app` in `/Applications/cellar Games/`. Built for FitGirl-
adjacent repacks and standalone cracked builds that expect a real
Windows-shaped environment.

## quick start

```sh
# clone + one-time GPTK install
git clone https://github.com/joemunene-by/cellar.git
cd cellar
./scripts/setup-gptk.sh

# health check
./scripts/cellar-doctor.sh

# match a game to an engine-family profile
./scripts/find-profile.sh "Need for Speed Heat"
#   Best match: frostbite-multi

# proactive bottle setup (so first launch goes straight to the game)
./scripts/cellar-install.sh frostbite-multi "Need for Speed Heat"

# make it a clickable app
./scripts/make-cellar-app.sh frostbite-multi "Need for Speed Heat"

# if a launch fails, scan the log for known failure patterns
./scripts/analyze-log.sh
```

Covered out of the box: **60+ games across 10 engines** via runtime
profiles in [`profiles.json`](profiles.json). See the [scripts
reference](#scripts-reference) for the full tooling.

## demo

> **Coming next**: short capture of the install → launch → gameplay loop
> on FIFA 19 (first non-CarX engine-family profile slated for
> verification). Placeholder reserved at `assets/demo.gif`.

<details>
<summary>What works today (without the GIF)</summary>

- **CarX Street** — Unity 2022 IL2CPP + Burst + Havok, verified end-
  to-end on M4 / macOS 15.6 / CrossOver wine 11.0 + D3DMetal 3.0. The
  bottle, launcher, and `.app` wrapper at
  `/Applications/cellar Games/CarX Street.app` all work as designed.
- **NFS Most Wanted 2005** — D3D9, verified via Goldberg Steam stub on
  the same stack.

Everything else in [`profiles.json`](profiles.json) is engine-
fingerprint-correct but unverified on Apple Silicon. See the FIFA
disclaimer in [CHANGELOG](CHANGELOG.md) for why that distinction
matters, and [ROADMAP](ROADMAP.md) for the test queue.

</details>

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

## scripts reference

Every tool lives under [`scripts/`](scripts/) and is a standalone bash
script. The full loop has no required UI; the Tauri shell on top is
optional polish.

### user-facing

| Script | What it does |
|---|---|
| `find-profile.sh "<game>"` | Match a game name to an engine-family profile |
| `cellar-install.sh <profile> "<game>"` | Proactive bottle setup (wineboot + winetricks + overrides) |
| `launch-engine.sh [--exe NAME] <profile> "<game>" [args]` | Generic profile-driven launcher |
| `make-cellar-app.sh <profile> "<game>"` | Wrap a profile invocation as `/Applications/cellar Games/<name>.app` |
| `cellar-logs.sh` | Browse `/tmp/*.log` cellar launcher logs (list / tail / open / prune) |
| `analyze-log.sh [log]` | Match a launcher log against 15 known failure signatures, print diagnosis + fix |
| `bottle-inspect.sh <bottle>` | Read-only introspection: prefix, wine version, DLL overrides, installed software, backups |
| `backup-saves.sh <bottle>` | Tarball the bottle's Documents + AppData state to `~/.cellar/backups/` |
| `cellar-doctor.sh` | One-shot health check (Rosetta, CrossOver, wine, D3DMetal, jq, winetricks, profiles) |

### engine-family launchers

| Script | Covers |
|---|---|
| `launch-fifa.sh <14..23>` | FIFA 14-23 (Impact / Ignite / Frostbite) with per-version DIRECTX_SELECT patch |
| `make-fifa-apps.sh [versions]` | Generate `.app`s for one or more FIFA versions, auto-detects installed |
| `launch-rdr2.sh [dir]` | RDR2 with `-sgadriver=Vulkan` for the MoltenVK path |
| `launch-bethesda.sh <dir>` | Bethesda with SKSE / F4SE / NVSE / FOSE / OBSE auto-detect |
| `launch-carxstreet-hybrid.sh` | CarX Street (verified working: Unity 2022 IL2CPP + Burst + Havok) |
| `launch-nfsmw-whisky.sh` | NFS Most Wanted 2005 (verified working: D3D9 + Goldberg stub) |

### setup + maintenance

| Script | What it does |
|---|---|
| `setup-gptk.sh` | One-time install of Apple Game Porting Toolkit + Wine + DXVK |
| `install-proton-winrt.sh <prefix>` | Stage Proton's WinRT family DLLs into a bottle (DispatcherQueue activation) |
| `validate-profiles.sh` | Schema + grammar check on `profiles.json`; CI-friendly exit codes |
| `make-game-app.sh "<name>" <script>` | Generic `.app` wrapper used by both `make-fifa-apps.sh` and `make-cellar-app.sh` |

### internal / experimental

`cls-setup.sh`, `cls-smoke.sh`, `freearc-smoke.sh`, and the
`scripts/launch-carxstreet-*` variants document earlier hybrid-runtime
experiments. See [`CHANGELOG.md`](CHANGELOG.md) for the lineage.

### debug knobs

```sh
CELLAR_METAL_HUD=1   ./scripts/launch-engine.sh ...   # Apple Metal perf overlay
CELLAR_WINEDEBUG=+heap ./scripts/launch-engine.sh ... # custom wine debug flags
CELLAR_NO_SKSE=1     ./scripts/launch-bethesda.sh ... # skip SKSE auto-detect
```

## engine-family profiles

Runtime profiles in [`profiles.json`](profiles.json) encode the recipe
(DLL overrides, winetricks set, launch args, runtime prereqs) per
engine. Adding a game in an already-covered family is a profile match,
not new code.

| Profile id | Engine | Sample games |
|---|---|---|
| `carx-street` | Unity 2022 IL2CPP | CarX Street (verified) |
| `nfs-most-wanted-2005` | D3D9 | NFS Most Wanted 2005 (verified) |
| `fifa-14-23` | Impact / Ignite / Frostbite | FIFA 14-23 (dedicated launcher) |
| `frostbite-multi` | Frostbite | NFS Heat / Unbound, Battlefield, Mass Effect Andromeda, Dragon Age Inquisition |
| `rage-rockstar` | Rockstar RAGE | GTA V, GTA IV, RDR2, Max Payne 3 |
| `d3d9-classic` | D3D9 (32-bit) | GTA San Andreas / Vice City / III |
| `unreal-engine-4-5` | UE4 / UE5 | Elden Ring, Sekiro, Dark Souls III, Hogwarts Legacy |
| `re-engine` | Capcom RE Engine | RE2/3/4 Remake, Village, DMC5, Monster Hunter |
| `anvilnext-ubisoft` | Ubisoft AnvilNext / Dunia / Disrupt | AC Origins / Odyssey / Valhalla, Far Cry 5/6 |
| `redengine` | CDPR REDengine 3/4 | Cyberpunk 2077, Witcher 3 |
| `bethesda-creation` | Bethesda Creation | Skyrim, Skyrim SE, Fallout 4/NV/3 |
| `forzatech` | ForzaTech | Forza Horizon 4 / 5 (Steam build only) |
| `pes-foxengine` | Fox Engine / UE4 | PES 2019-2021, eFootball 2024+ |
| `unity-il2cpp-2022` | Unity 2022 IL2CPP (fallback) | any Unity 2022 game not matched above |

`scripts/validate-profiles.sh` keeps these consistent (grammar of
`WINEDLLOVERRIDES`, known winetricks verbs, no `-dx11` on DX12-only
titles, etc.).

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
  CHANGELOG.md
  profiles.json           bundled per-game runtime profiles
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
      profiles.rs         bundled + user profile loader, name match
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
registry tweak Y, launch arg Z". One-click apply. Foundation
landed early via `profiles.json` (bundled set of per-game
runtime profiles, auto-applied on `library_add` when a game's
name matches a profile's `match_name_contains`, user overrides
at `~/.cellar/profiles.json`). The CarX Street hybrid recipe
(Proton WinRT DLLs + MF codec overrides + the MVK / Rosetta
env vars) is the first profile to ship. Remaining v0.5 work is
the install-time runner that materializes a profile's prereqs
(`winetricks mf`, Proton WinRT staging, Homebrew GStreamer
install hint) instead of just declaring them in `requires`.

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
in mainline. **Resolved in cellar** via a hybrid runtime:
cellar wine-staging 11.8 + Whisky's `D3DMetal.framework` +
Whisky's d3d* forwarder DLLs + Proton's WinRT family DLLs
(`coremessaging.dll`, `wintypes.dll`, `twinapi.appcore.dll`,
`windows.gaming.input.dll`, etc.) staged into the bottle via
`scripts/install-proton-winrt.sh`, with the WinRT activation
registry entries pointing `Windows.System.DispatcherQueue*` at
`coremessaging.dll`. See `scripts/launch-carxstreet-hybrid.sh`
for the full env. CarX Street (Unity 2022 IL2CPP + Burst +
Havok + always-online auth) launches and renders end-to-end on
M4 with this recipe.

Additional per-title shims wired into the hybrid path:
- **Native Microsoft `mf.dll`** via `winetricks mf` (the wine
  builtin is a stub that fails Unity's `CreateObjectFromByteStream`
  on splash videos).
- **Homebrew GStreamer + `gst-libav`** for the actual codec
  decode — wine's `winegstreamer` is the bridge; with
  `GST_PLUGIN_PATH=/opt/homebrew/lib/gstreamer-1.0` and
  `DYLD_LIBRARY_PATH=/opt/homebrew/lib` exported, wine routes
  Media Foundation decode requests through GStreamer + FFmpeg
  + Apple `VTHW_h264`.
- **Steam API stub** — Goldberg (`gbe_fork`) is the clean
  default for non-online titles; for always-online titles whose
  cracks bundle a local server-response stub (e.g. RUNE for
  CarX Street), keep the original cracked `steam_api64.dll`
  alongside the crack's loader (`RUNE64.dll`). Both paths are
  documented in `scripts/launch-carxstreet-hybrid.sh`.

Several recent Unity Mac ports — including CarX Street itself —
also ship a native Apple Silicon build via Steam, which sidesteps
cellar entirely for legit-licensed users.

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

### FIFA 14-23 (Impact / Ignite / Frostbite)

`scripts/launch-fifa.sh <version>` reuses the CarX hybrid runtime
(CrossOver wine 11.0 + Apple GPTK D3DMetal 3.0) for ten FIFA titles
spanning three engines. It creates a per-version bottle at
`~/.cellar/bottles/fifa<ver>/prefix`, runs
`winetricks vcrun2019 corefonts d3dcompiler_47` on first boot (with
`dotnet48` attempted best-effort because winetricks #2246 / #1792
break it on Apple Silicon prefixes), sets `WINEDLLOVERRIDES` to
route `d3d11,d3d12,dxgi,d3d10core=n,b` (D3DMetal handles them) and
`nvapi,nvapi64=` (empty value = disabled per wine's loader grammar;
the literal `disabled` is NOT a valid token and silently falls back
to default). For FIFA 14 it adds `d3d9=native,builtin` via reg add.
For v20/21/22 the script patches `Documents/FIFA <year>/fifasetup.ini`
with `DIRECTX_SELECT = 0` (0=DX11, 1=DX12), the actual community-
documented DX toggle. `scripts/make-fifa-apps.sh <14..23>` produces
a clickable `/Applications/cellar Games/FIFA <N>.app` bundle per
installed version, same wrapper pattern as CarX.

Exe casing varies across versions (14-17 are lowercase `fifaNN.exe`,
18-23 are uppercase, cracked releases sometimes add `_x64` suffixes),
so the launcher resolves the exe case-insensitively from the game
dir at launch time rather than hard-coding the name.

Per-version status:

| Version | Engine | API | Anti-cheat | Notes |
|---|---|---|---|---|
| FIFA 14 | Impact | D3D9 | none | D3DMetal d3d9 path, same as NFS:MW |
| FIFA 15 | Ignite | D3D11 | none | should "just work" |
| FIFA 16 | Ignite | D3D11 | none | should "just work" |
| FIFA 17 | Frostbite | D3D11 | none | first Frostbite FIFA |
| FIFA 18 | Frostbite | D3D11 | none | probable |
| FIFA 19 | Frostbite | D3D11 native | none | best modern starting point, CrossOver compat entry exists |
| FIFA 20 | Frostbite | DX12 default, DX11 fallback | none | force DX11, skip Origin |
| FIFA 21 | Frostbite | DX12 default, DX11 fallback | none | force DX11, skip Origin, ProtonDB mixed |
| FIFA 22 | Frostbite | DX12 default, DX11 fallback | none | force DX11, ProtonDB Gold baseline |
| FIFA 23 | Frostbite | D3D12 only | EAAC kernel-mode (retail) | experimental, see below |

FIFA 23 retail ships EA AntiCheat which has no wine support
(kernel-mode by design). Community pre-installed cracked builds
strip EAAC for offline / career play, which is the only path on
cellar. With EAAC out of the picture the next risk is Frostbite's
DX12 path on D3DMetal, same class of bindless + min16float shaders
that produced the CarX vertex glitch under D3DMetal 2.0 and was
fixed by D3DMetal 3.0. v23 is worth a try; ordered last in the
test queue until a working boot is logged.

Source-selection rule applies across the whole family: every FIFA
must be standalone (no EA App / Origin / EA Desktop) **and** must
not be a FitGirl or DODI repack. The `cls-*.dll` shared-memory
IPC deadlock documented below blocks the lollypop / lolzi / lolzx
codec chain on wine 11 + macOS 15. community pre-installed build and community-bypass build pre-
installed builds skip both the EA launcher and the proprietary
codec chain, so they are the only viable sources here.

Reality check: as of writing there is no public report of any FIFA
from 21 onward reaching the main menu on Apple Silicon via D3DMetal
in any configuration. AppleGamingWiki has a FIFA 22 page but
nothing surfaced from it confirms a working boot. Treat the recipe
above as best-effort engineering against the engine / API / anti-
cheat profile; expect to debug. FIFA 19 (D3D11 native, no anti-
cheat, has a CodeWeavers compatibility entry) is the lowest-risk
starting point.

### Engine-family profiles + generic launcher

Beyond CarX, NFS:MW, and FIFA (which have dedicated launchers in
`scripts/`), `profiles.json` carries ten engine-family runtime
profiles that the generic launcher `scripts/launch-engine.sh
<profile-id> <game-dir>` consumes directly. Adding a new game in
one of these families usually means just placing it under
`~/Games-source/<name>/` and running the matching profile, no new
script needed.

Engine families covered:

| Profile id | Engine | Example games |
|---|---|---|
| `frostbite-multi` | Frostbite | NFS Heat / Unbound / Payback, Battlefield 1 / 4 / V, Star Wars Battlefront, Mass Effect Andromeda, Dragon Age Inquisition, Anthem, Mirror's Edge Catalyst |
| `rage-rockstar` | Rockstar RAGE | GTA V, GTA IV, RDR2, Max Payne 3 |
| `d3d9-classic` | older D3D9 | GTA San Andreas / Vice City / III |
| `unreal-engine-4-5` | UE4 / UE5 | Elden Ring, Sekiro, Dark Souls III, Hogwarts Legacy, Hellblade |
| `re-engine` | Capcom RE Engine | RE2/3/4 Remake, Village, RE7, DMC5, Monster Hunter Rise / World |
| `anvilnext-ubisoft` | Ubisoft engines | Assassin's Creed Origins / Odyssey / Valhalla / Black Flag, Far Cry 5 / 6, Watch Dogs |
| `redengine` | CDPR | Cyberpunk 2077, Witcher 3 (prefer native Mac port) |
| `bethesda-creation` | Bethesda Creation | Skyrim / SE / AE, Fallout 4 / NV / 3 / 76 |
| `forzatech` | ForzaTech | Forza Horizon 4 / 5 (needs Proton WinRT DLLs, same as CarX) |
| `pes-foxengine` | Konami Fox + UE4 | PES 2019-2021, eFootball 2024+ |

Each profile encodes the DLL override pattern (which DXs route
through D3DMetal, which DRM DLLs to neutralize like `socialclub` or
`uplay_r1_loader`), the winetricks set, default launch args (UE
profiles auto-add `-dx11`), and the `requires` list (winetricks
verbs the launcher will install on first boot, plus hints like
`proton_winrt_dlls` for engines that need WinRT runtime classes).

Generic launcher usage:

```sh
# minimum
scripts/launch-engine.sh <profile-id> "<game-dir-name>"

# concrete
scripts/launch-engine.sh frostbite-multi "Need for Speed Heat"
scripts/launch-engine.sh rage-rockstar "Grand Theft Auto V"
scripts/launch-engine.sh d3d9-classic "Grand Theft Auto San Andreas"
scripts/launch-engine.sh unreal-engine-4-5 "Elden Ring"
scripts/launch-engine.sh re-engine "Monster Hunter World"
scripts/launch-engine.sh bethesda-creation "Skyrim Special Edition"
```

The bottle lands at `~/.cellar/bottles/<profile-id>-<game-slug>/prefix`
(one bottle per game, so titles in the same engine family don't
share Wine state). Logs at `/tmp/cellar-<bottle>.log`. Same `make-
game-app.sh` wrapper can turn any of these into a clickable
`/Applications/cellar Games/<name>.app`.

#### Engine-family caveats worth knowing

- **Frostbite (non-FIFA)** — each game has its own DX-version
  toggle location: NFS Heat / Unbound write `Documents/Need for
  Speed Heat/settings.ini`; Battlefield uses `user.cfg` with
  `RenderDevice.Dx12Enabled 0`. The profile doesn't patch these
  automatically; force DX11 in the game's own settings the first
  time it boots, and it'll persist.
- **Rockstar RAGE** — RDR2 supports `-vulkan` on the command line,
  which routes through MoltenVK instead of D3DMetal's DX12 path
  and has been the more stable option on Linux Proton historically.
  Pass `-vulkan` manually for RDR2 by editing the launcher.
- **D3D9 classics** — Resolution often defaults to 640x480
  windowed; set Wine's virtual desktop or the game's own preset.
  Widescreen Fix / SilentPatch / ASI loader mods all work as
  drop-in `.asi` / `.dll` files in the game directory.
- **UE4 / UE5** — `-dx11` is in `launch_args` by default. Hogwarts
  Legacy uses Nanite + Lumen which adds shader compile stutter
  that's unrelated to wine.
- **ForzaTech** — Needs `proton_winrt_dlls` like CarX. Run
  `scripts/install-proton-winrt.sh "$PREFIX"` against the bottle
  after the first wineboot.
- **REDengine** — Witcher 3 has a native macOS port (CDPR shipped
  it for Apple Silicon). Prefer that over wine for Witcher 3
  specifically. Cyberpunk 2077 is genuinely experimental on Mac;
  no documented Apple Silicon success.

#### What gets blocked

Hard blocks that no profile can route around:

- Kernel-mode anti-cheat: Vanguard (Valorant), Hyperion (CoD MW2+
  2022+), late-game Fortnite combo. No wine support, period.
- Rockstar online (GTA Online, RDR Online) — Take-Two has banned
  wine sessions. Single-player only.
- FitGirl / DODI repacks using the `cls-*.dll` codec chain
  (lollypop, lolzi, lolzx) — documented further down in
  "CLS plugin shim IPC blocked".

For these, the answer is either xCloud / GeForce Now (Valorant,
Fortnite) or a native Mac port if EA / Ubisoft / Capcom ever ships
one, not cellar.

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
