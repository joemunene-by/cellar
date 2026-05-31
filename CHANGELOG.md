# Changelog

## Unreleased

### Added — modern Unity titles on M-series (CarX Street recipe)

The CarX Street launch tonight validated the hybrid runtime end-to-end
for a Unity 2022 IL2CPP + Burst + Havok title with always-online auth.
Locked into the repo:

- **`scripts/install-proton-winrt.sh`** — extracts the WinRT family
  DLLs (`coremessaging`, `wintypes`, `twinapi.appcore`, the full
  `windows.*` set) from a GE-Proton tarball, stages them into a
  cellar bottle's `system32` / `syswow64`, sets DLL overrides to
  prefer native, and registers `Windows.System.DispatcherQueue*`
  under `HKLM\Software\Microsoft\WindowsRuntime\ActivatableClassId\`
  pointing at `coremessaging.dll`. Solves the
  `RoGetActivationFactory: Failed to find library` wall on wine 11.x
  without waiting for upstream wine to ship the COM impl.
- **`scripts/launch-carxstreet-hybrid.sh`** updated with
  `GST_PLUGIN_PATH=/opt/homebrew/lib/gstreamer-1.0` and
  `DYLD_LIBRARY_PATH=/opt/homebrew/lib`. Wine's `winegstreamer.so`
  routes Media Foundation decode through Homebrew's GStreamer +
  `gst-libav` (FFmpeg) when the host has `brew install gstreamer
  gst-libav`. Fixes Unity's `WindowsVideoMedia error 0xc00d36bb`
  on the splash video.
- **README "known issues" rewrite** — the Unity 2022 IL2CPP entry
  now documents the working hybrid recipe (Proton WinRT DLLs +
  Whisky D3DMetal + native Microsoft `mf.dll` from `winetricks mf`
  + Homebrew GStreamer + the right Steam-stub for the game's crack
  style) instead of the earlier "blocked" verdict.

### Notes

- Goldberg (`gbe_fork`) is the clean default Steam-API stub for
  non-online titles. For always-online titles where the crack
  (e.g. RUNE for CarX Street) bundles its own server-response
  spoof inside the cracked `steam_api64.dll`, keep that original
  DLL + its loader (`RUNE64.dll`) — Goldberg only stubs Steam,
  it does not impersonate game-specific auth backends.
- The Proton WinRT DLLs work as plain PE binaries loaded into wine
  on macOS; their imports resolve to wine's combase / kernel32 /
  ntdll without needing Linux-specific wine extensions.

### Added — per-game runtime profiles + extra Metal knobs

The hard-won CarX Street launch recipe (Proton WinRT + MF codec
overrides + Burst-friendly env vars) was scattered across an
out-of-tree shell script. v0.2 lifts the configuration into the
launcher itself so other Unity 2022 IL2CPP titles inherit the same
treatment automatically.

- **`profiles.json`** at the repo root. Three bundled profiles:
  - `carx-street`: full Unity 2022 IL2CPP + Burst recipe with the
    MF codec DLL overrides and the MVK / Rosetta env vars locked in.
  - `nfs-most-wanted-2005`: DXVK-on D3D9 baseline.
  - `unity-il2cpp-2022`: conservative fallback for unidentified
    Unity 2022 IL2CPP titles; `match_name_contains` empty so it is
    manual-select only.

  Users override at `~/.cellar/profiles.json`. A user profile whose
  `id` matches a bundled entry shadows the bundled one.
- **Auto-apply in `library_add`**: a new game's name is matched
  (case-insensitive substring) against each profile's
  `match_name_contains`. The first hit's `settings` replace the
  default starting point; falls back to `GameSettings::default()` if
  nothing matches. Two new Tauri commands: `profiles_list`,
  `profiles_find`.
- **`metal_fences: bool`** in `GameSettings`. Exports
  `MVK_ALLOW_METAL_FENCES=1` to use Metal fences instead of
  events/semaphores on the DXVK/MoltenVK path. No effect when DXVK
  is off (D3DMetal direct path bypasses MoltenVK entirely).
- **`metal_hud: bool`** in `GameSettings`. Exports
  `MTL_HUD_ENABLED=1`. Apple's Metal HUD overlay (FPS, GPU usage,
  frame time) without touching the game's own UI.
- **`dll_overrides: Option<String>`** in `GameSettings`. Semicolon-
  separated extras *appended* to the DXVK defaults rather than
  replacing them. The carx-street profile uses this for the MF
  codec passthrough (`mf=b;mfplat=b;mfreadwrite=b;mfmediaengine=b;
  mfsrcsnk=b`).
- **`LSPrefersRosetta2AheadOfTime=YES`** + `LSRequiresNativeExecution
  =NO` added to the `Info.plist` that `scripts/make-game-app.sh`
  emits. The effect on a shell-script wrapper bundle is uncertain
  (the wine binary is the real translation target, not the bash
  wrapper) but the flag is harmless and gives Launch Services the
  right hint if it ever does propagate.

### Changed — `WINEDLLOVERRIDES` composition

`runtime.rs` previously hardcoded `WINEDLLOVERRIDES=d3d11,d3d10core,
dxgi=n` when DXVK was on, and a per-game `env.WINEDLLOVERRIDES`
silently replaced the whole string, dropping the DXVK redirectors.

Composition is now: `<DXVK defaults if dxvk on>` + `;` +
`<settings.dll_overrides>`, then the per-game `env` map still wins
as a full escape hatch via `Command::env` last-write-wins. This makes
`settings.dll_overrides` the right field for additive overrides; `env`
is reserved for total custom rewrites.

## v0.2 (previous main)

The "native FreeArc reader + writer + honest wine verdict" release.

### Added

- **`freearc-native` crate** — pure-Rust FreeArc archive library, no
  wine, no FFI. Footer-first parser, full DIRECTORY block + file
  table, per-file extraction with CRC verification. Decoders for
  `storing`, `lzma:*` (with synthesised LZMA1 header), `zstd[:N]`.
  Chain dispatch (`srep+dispack+delta+lollypop` style) walks
  right-to-left and routes each stage through the appropriate path.
- **Archive writer** (`freearc-native::writer`) and `fg-arc-c` CLI.
  Writes valid FreeArc archives with the `storing` codec. Cargo
  integration test verifies write → read → extract → byte-identity
  round-trip without depending on any FitGirl bin. Cross-platform
  verified on Linux (x86_64) and Apple Silicon Mac.
- **Standalone CLIs**:
  - `fg-arc-ls` — list footer + control blocks
  - `fg-arc-files` — list every file inside (works on any archive,
    regardless of codec; DIR blocks use lzma which we decode natively)
  - `fg-arc-x` — extract every file the codec dispatch can handle,
    with per-file CRC verification
  - `fg-arc-c` — create an archive from a directory (`storing`)
  - `fg-arc-dump` — debug helper, decompress + hexdump one control block
- **`archive_peek` Tauri command** + **"Preview contents" pane in
  the install wizard**. Lets users see file count, total bytes,
  codec list (with native / hybrid / unsupported badges) and the
  first 30 files BEFORE running the installer. Works on any
  FitGirl bin since DIR blocks always use lzma.
- **`freearc-cls-host` crate** — PE32 helper that loads any
  `cls-*.dll` via libloading and runs `ClsMain`. Cross-compiled via
  cargo-zigbuild (no mingw needed). Dispatch in `cls_host.rs` shells
  out to this binary under wine for closed-source CLS codecs.
  Tracing mode (`CELLAR_CLS_TRACE=1`) logs every callback dispatch.
- **`scripts/freearc-smoke.sh`** — parser sanity check across a
  directory of archives, with magic-byte pre-filter so it doesn't
  flood on unrelated `*.bin` files.
- **`scripts/cls-setup.sh`** — auto-discovers the FitGirl plugin
  staging directory in any wine bottle, stages every file
  (`cls-*.dll`, sidecar `_x64`/`_x86` workers, helper DLLs) into
  `~/.cellar/cls/`, prints env-var setup lines.
- **`scripts/cls-smoke.sh`** — probes every staged plugin with
  empty input through the CLS host, verifies LoadLibrary +
  GetProcAddress + ClsMain dispatch work. All 8 FitGirl plugins
  (lolzi, lolzx, lolly, lollypop, lollypop2, srep, MSC, zstd) PASS.

### Verified

- All Cargo tests green: 19 unit tests across `varint`, `dir`,
  `decompress`, `extract`, `cls_host`, `writer`.
- Round-trip `fg-arc-c → fg-arc-ls → fg-arc-files → fg-arc-x → diff`
  succeeds on Linux x86_64 and Apple Silicon Mac with both text and
  random binary input.
- CLS architecture verified: every plugin LoadLibrary + ClsMain
  resolve + INIT callback succeeds on Whisky wine 7.7 and Gcenx
  wine-staging 11.x.

### Blocked (won't ship in v0.2)

Full extraction of FitGirl `fg-*.bin` archives whose data blocks
use the `lollypop` codec chain. The architecture works; the wine-
on-Mac runtime cannot deliver the shared-memory IPC that lollypop's
2-piece shim+worker design relies on.

Evidence (all on Apple Silicon Mac, macOS 15):
- via `freearc-cls-host` direct call: `ClsMain(CLS_DECOMPRESS)`
  returns -1 with zero callbacks fired between INIT and the
  failure. Same outcome on wine-staging 11.8, wine-devel 11.8,
  and Whisky wine 7.7. The shim aborts before it asks us for any
  input.
- via the legacy `freearc-shim` (= `unarc.dll FreeArcExtract`, the
  exact entry point FitGirl's own installer calls): emits one
  `progress 9/9577819` callback then sits at **0% CPU for 10+
  minutes**. Pure IPC deadlock waiting on a kernel event the
  worker never signals. Same on wine 7.7 and 11.8.

Confirmed root cause: FitGirl's `cls-*.dll` plugins use Windows
shared-memory IPC (CreateFileMapping + named events) to talk to
their `cls-*_x64.exe` / `cls-*_x86.exe` worker. Wine's
implementation of that on Apple Silicon doesn't deliver. Out of
our hands until wine 12 or CrossOver 25 (or someone fixes it
upstream).

What this means in practice:
- the `archive_peek` UI works on every FitGirl bin (DIR blocks
  use lzma, not lollypop)
- the `fg-arc-x` extraction works on archives that use only
  `storing` / `lzma` / `zstd` codecs — that covers most plain
  `.arc` archives and a subset of game-installer payloads
- it does NOT work on a typical CoD-MW3-class FitGirl bin
  (lollypop)

### Repo state

```
freearc-native/      Rust crate, the open-source reader/writer
freearc-cls-host/    PE32 helper for the (currently blocked) hybrid path
freearc-shim/        legacy wrapper around unarc.dll (deadlocks, kept for ref)
src-tauri/           cellar app: wine bottle mgmt, library, archive_peek
src/                 React frontend with the Preview contents pane
scripts/             smoke + setup helpers
```

### Honesty about what cellar can do for a Mac user today

- pick an installer folder, see what's in the FreeArc payload
  before committing to install: yes
- extract a `storing/lzma/zstd`-only FreeArc archive: yes
- launch a wine bottle and run an installer GUI: yes (same as
  Whisky / Crossover, no new value here)
- extract a real-world FitGirl `fg-*.bin` that uses lollypop:
  no, blocked on the wine-on-Mac IPC issue above

## v0.1

Initial Tauri 2 + Rust scaffold. Wine bottle create/list/remove.
Repack-folder detection (FitGirl / DODI / KaOs / Inno Setup) via
filename heuristics. Plain `installer_run` Tauri command. Manual
library entries.
