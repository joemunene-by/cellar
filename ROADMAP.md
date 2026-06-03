# cellar roadmap

Public plan. Issues / PRs welcome on anything listed here. See
[`CHANGELOG.md`](CHANGELOG.md) for the lineage of what's already shipped.

## Near-term (v0.1 → v0.2)

### Verify the first non-CarX engine-family profile

Right now CarX Street (Unity 2022 IL2CPP) and NFS Most Wanted 2005
(D3D9) are the only verified-working titles. Every other profile
in `profiles.json` is engine-fingerprint-correct but has no public
Apple Silicon success record. Priority order for first test boots:

1. **FIFA 19** (Frostbite D3D11 native, no anti-cheat, has a CodeWeavers
   compat entry) via `scripts/launch-fifa.sh 19`.
2. **GTA San Andreas / Vice City / III** via `scripts/launch-engine.sh
   d3d9-classic` (smallest install, lowest risk).
3. **Monster Hunter World** via `scripts/launch-engine.sh re-engine`
   (oldest of the RE Engine family, well-trodden under Proton).
4. **Skyrim Special Edition** via `scripts/launch-bethesda.sh` with
   SKSE auto-detect (best-trodden modern Bethesda title under wine).
5. **Elden Ring** via `scripts/launch-engine.sh unreal-engine-4-5`
   (popular UE4 target; DX12-only, so first real test of Frostbite-
   style DX12 → D3DMetal 3.0 on a non-Frostbite engine).

Each successful boot gets a CHANGELOG entry + an upgrade of the
"verified" tag in the profile description.

### Tauri frontend wire-up

Backend already has `profiles_list` and `profiles_find` Tauri commands
in `src-tauri/src/profiles.rs`. Frontend `src/components/Library.tsx`
doesn't call them yet. Wire up:

- Display matched-profile badge per game in the library.
- "Launch" button calls the right launcher (`launch-engine.sh` for
  generic, `launch-fifa.sh` / `launch-rdr2.sh` / `launch-bethesda.sh`
  for dedicated).
- "Install" button calls `cellar-install.sh` upfront so the first
  launch doesn't wait on winetricks.
- "Inspect" button surfaces `bottle-inspect.sh` output in a panel.

## Medium-term (v0.2 → v0.5)

### Crash report capture

When a launch fails, auto-collect:
- The launcher log.
- `bottle-inspect.sh` output.
- `analyze-log.sh` output (pattern matches + remediation hints).
- `cellar-doctor.sh` output.

Bundle into a single `crash-report-<bottle>-<timestamp>.zip` for
sharing in issues. Probably a new `scripts/crash-report.sh` plus a
hook in `launch-engine.sh` to invoke it on non-zero exit.

### Auto-mount detection for Games-source

`fswatch`-based daemon that watches `~/Games-source/` for new
directories, runs `find-profile.sh` against each, and notifies via
`osascript -e 'display notification'` with the suggested
`cellar-install.sh` command pre-baked.

### D3DMetal version switcher

Some games might prefer D3DMetal 2.x (older shader translation rules)
over 3.0; the CarX vertex glitch fix was the other direction. Let
the user pick per bottle:

```sh
scripts/d3dmetal-switch.sh <bottle> 3.0   # default
scripts/d3dmetal-switch.sh <bottle> 2.0   # for engines that hit a 3.0 regression
```

### Dedicated launchers for the remaining engine families

Right now only Frostbite (via `launch-fifa.sh`), RAGE (`launch-rdr2.sh`),
Bethesda Creation (`launch-bethesda.sh`), Unity (`launch-carxstreet-
hybrid.sh`), and D3D9 (`launch-nfsmw-whisky.sh`) have dedicated
launchers. The remaining seven engine families fall through to the
generic `launch-engine.sh`. Add dedicated wrappers as engine-specific
quirks emerge:

- `launch-re-engine.sh` — handle RE Village's DX11 / DX12 toggle.
- `launch-anvilnext.sh` — handle the ACInitiates.dat presence check.
- `launch-redengine.sh` — Cyberpunk-specific shader cache pre-warm.
- `launch-forzatech.sh` — handle ForzaTech's controller dependency on
  XInput injection.
- `launch-pes.sh` — year-parametrized like `launch-fifa.sh`.

## Long-term (v0.5+)

### Save game cloud sync

`backup-saves.sh` already snapshots locally. Add an optional rsync /
git-annex / iCloud Drive sync layer so save state survives an
SSD swap or machine migration.

### Per-bottle benchmarks

Surface fps / GPU load / frametime captures from each launch via a
hook in `launch-engine.sh` that scrapes `MTL_HUD_ENABLED=1` output
into a sqlite db. Useful for tracking when a CrossOver / D3DMetal
upgrade regresses or improves a known game.

### Workflow integrations

GitHub Actions:
- Auto-build the Tauri `.app` for tagged releases on macOS runners.
- Auto-publish `validate-profiles.sh` results as a PR check status.

### Native arm64 wine experiments

CrossOver's arm64 wine path (vs the current x86_64 wine + Rosetta 2)
would skip the Rosetta translation layer for the wine process itself.
Track CodeWeavers' progress and migrate when stable.

## Won't fix

- **Kernel-mode anti-cheat games.** Valorant (Vanguard), late Fortnite
  (Hyperion), CoD MW2+ (Ricochet), PUBG online, Genshin Impact
  (mHyProt). Wine has no path to load Windows kernel drivers; this is
  permanent across every wine-based launcher.
- **Online Rockstar.** Take-Two has banned wine sessions on GTA Online
  and RDR Online. Single-player only.
- **Repacks using the FitGirl / DODI cls codec chain.** The shared-
  memory IPC primitive the `cls-*.dll` shim uses to talk to its `_x64.exe`
  worker is broken on wine-on-Mac (documented in CHANGELOG v0.2 stance
  + retested under CrossOver wine 11.0). Until wine 12 or a future
  CrossOver fixes the underlying file-mapping + named-event behavior,
  these repacks won't extract. Use any non-FitGirl, non-DODI source.
