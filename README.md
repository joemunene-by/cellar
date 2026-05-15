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
  v
Wine 9.x  (GPTK-patched build, vendored or homebrew)
  - Windows API translation
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
  scripts/
    setup-gptk.sh         one-time install of GPTK + Wine + DXVK
```

## roadmap

**phase 1 (v0.1, today):** Tauri 2 + Rust shell. Library UI placeholder.
Wine bottle create / list / remove via shell. Plain installer launch
(no FitGirl-specific automation yet). Manual library entry.

**phase 2 (v0.2):** Repack detection. Recognise FitGirl / DODI / KaOs
folder structure. Walk the Inno Setup wizard programmatically when
possible. Surface install progress in the UI.

**phase 3 (v0.3):** Polished library. Card grid, last-played, total
play time, launch button, delete game. Per-game settings (DXVK toggle,
ESYNC, MSYNC, dpi, custom env vars).

**phase 4 (v0.4):** Save-game backup. iCloud Drive or any chosen
external path. Per-game schedule. Optional sync of game settings across
machines via a private git mirror of `~/.cellar/library.json`.

**phase 5 (v0.5):** Game-specific shims. Catalogue of "this game needs
font X installed, registry tweak Y, launch arg Z". User clicks
**Install**, cellar applies the shim automatically.

## license

MIT (code only). Nothing in this repo distributes games, repacks, or
any copyrighted content. cellar is a launcher; the user supplies the
game files.
