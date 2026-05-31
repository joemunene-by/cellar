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

### Added — frontend wiring for profiles + new toggles

The Library tab's per-game Settings drawer now surfaces everything
the backend gained. New sections, in drawer order:

- **Profile** at the top. Shows whether the game's name matched a
  bundled profile (with description and `requires` checklist).
  "Apply profile…" button opens an inline picker listing every
  available profile (bundled and user). "Re-apply" button on the
  matched profile resets the drawer to the profile's defaults
  after manual edits. Picker rows show profile name, description,
  and `match_name_contains` patterns for transparency.
- **Metal Fences** toggle. Greyed out when DXVK is off, since
  `MVK_ALLOW_METAL_FENCES` only affects the DXVK/MoltenVK path.
- **Metal HUD** toggle. Always available. Sets `MTL_HUD_ENABLED=1`.
- **DLL overrides** single-line input, with helper text pointing
  at the right escape hatch (`env.WINEDLLOVERRIDES` for full
  override; this field for additive entries on top of DXVK).

`src/lib/invoke.ts`:

- `GameSettings` interface extended with `metal_fences`, `metal_hud`,
  `dll_overrides`.
- New `Profile` interface mirroring the Rust struct.
- New `profiles.list()` and `profiles.find(gameName)` invoke wrappers.
- Drawer state initialiser backfills the three new fields when an
  older `library.json` is read (which is what existing installs will
  have on first upgrade).

`src/styles.css`: adds `.profile-actions`, `.profile-picker`,
`.profile-row`, `.profile-info`, `.profile-desc`, `.profile-match`,
`.requires-list` for the new drawer section.

Both `cargo check` and `tsc --noEmit` clean.

### Added — profile prerequisite runner

Profiles declare `requires` (e.g. `proton_winrt_dlls`,
`winetricks_mf`, `homebrew_gstreamer`). v0.2 turns those declarations
into one-click installs in the drawer.

- **`src-tauri/src/prereq.rs`**. New module + Tauri command
  `prereq_install(bottle_id, require_id)`. Dispatches by
  `require_id`:
  - `winetricks_mf` (and other `winetricks_*` ids) delegates to the
    existing `wine_run_winetricks` for the matching verb.
  - `proton_winrt_dlls` is a full Rust re-implementation of
    `scripts/install-proton-winrt.sh`. Detects a GE-Proton tarball at
    `/tmp/ge-proton.tar.gz`, `~/.cellar/cache/ge-proton.tar.gz`, or
    `~/Downloads/ge-proton.tar.gz`. Extracts (targeted first, full
    fallback), stages the WinRT DLL set
    (`coremessaging.dll`, `wintypes.dll`, `windows.system.dll`,
    `windows.gaming.input.dll`, `windows.ui.dll`, ...) into the
    bottle's `system32` (and `syswow64` if the 32-bit dir exists),
    sets `HKCU\Software\Wine\DllOverrides` to native,builtin for
    each, and registers the `Windows.System.DispatcherQueue*`
    activation classes under
    `HKLM\Software\Microsoft\WindowsRuntime\ActivatableClassId\`.
    Streams progress as `cellar://prereq` events; emits
    `cellar://prereq-done` with success + detail. If the tarball is
    absent, returns `DependencyMissing` with the GE-Proton release
    URL and the candidate paths.
  - `homebrew_gstreamer` returns `ManualActionRequired` with the
    exact `brew install gstreamer gst-libav` command the user has to
    run themselves (cellar cannot pass the brew/keychain prompt).
- **Frontend wiring**. The drawer's Profile section turns each
  `requires` entry into an interactive row: per-prereq Install
  button with live status (`idle` / `running` / `ok` / `failed` /
  `manual`), inline last-status detail, retry on failure. Listener
  on `cellar://prereq-done` flips status when the backend finishes.
  Per-row CSS: green border on success, red on failure, blue on
  running, amber on manual-action-required.
- New `invoke.ts` types: `PrereqLine`, `PrereqDone`. New wrapper:
  `prereq.install(bottleId, requireId)`.

cargo check + tsc --noEmit both clean.

### Added — prereq satisfaction check

The Install buttons were a footgun: nothing told the user whether
the bottle already had the WinRT DLLs staged or the Microsoft
mfplat.dll installed, so a returning user would always see "Install"
even when re-installation was redundant.

- `prereq_check(bottle_id, require_id)` and `prereq_check_all(
  bottle_id, [require_id])` Tauri commands. Inspect the bottle on
  disk for the load-bearing artefact:
  - `proton_winrt_dlls`: `coremessaging.dll` in `system32`
  - `winetricks_mf`: `mfplat.dll` size > 500 KB (distinguishes the
    real Microsoft DLL ~2 MB from wine's builtin stub ~50-100 KB)
  - `winetricks_d3dcompiler_47`: presence of `d3dcompiler_47.dll`
  - `winetricks_vcrun2003`: presence of `msvcr71.dll`
  - `homebrew_gstreamer`: presence of `/opt/homebrew/lib/
    gstreamer-1.0/libgstlibav.dylib` (base GStreamer is not enough,
    we also need `gst-libav` for FFmpeg-backed codecs)
- Drawer mount now batches `prereq.checkAll` against the matched
  profile's `requires` list. Each entry that comes back satisfied
  gets its row coloured green from the start, button labelled
  "Re-install" instead of "Install", and the detail text shows
  why it was considered installed.
- Unknown `require_id`s return `satisfied: false, detail: "no
  detection rule"` so old frontend ↔ new backend (or vice versa)
  never crashes the drawer.

### Added — GE-Proton tarball auto-download

The Proton WinRT install previously failed with
`DependencyMissing` if no tarball was pre-staged. The user had to
go fetch ~700 MB from GitHub themselves, save it to one of three
exact paths, then click Install again.

`prereq.rs::download_ge_proton_tarball` lifts that step into cellar.
First click on `proton_winrt_dlls` now does the whole flow:

1. GET https://api.github.com/repos/GloriousEggroll/proton-ge-custom/
   releases/latest via curl (`-H "User-Agent: cellar-launcher"`,
   GitHub requires a UA).
2. Parse the JSON, find the `.tar.gz` asset, read its size and the
   release tag.
3. curl the `browser_download_url` to
   `~/.cellar/cache/ge-proton.tar.gz`. Heartbeat task polls the
   destination file size every 3 s and emits a `cellar://prereq`
   line with current MB downloaded + percent, so the drawer shows
   forward motion on the long fetch.
4. Sanity-check the resulting file is at least 100 MB (anything
   smaller is almost certainly a GitHub HTML error page that curl
   wrote as-is). Wipe and error out if not.

Implementation uses `curl` as a subprocess instead of pulling in
reqwest + rustls. macOS ships curl in the base system, and
cellar's dep tree stays small (no extra TLS stack to compile and
audit).

If a local tarball already exists at any of the candidate paths
(`/tmp/`, `~/.cellar/cache/`, `~/Downloads/`), it's still
preferred — the auto-download only fires when nothing is staged.

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
