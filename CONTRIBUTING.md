# Contributing to cellar

Thanks for the interest. cellar is a small project with a clean
architecture and a long list of upgrades that haven't been built yet.

## Quick orient

- Everything user-facing lives under [`scripts/`](scripts/) as standalone
  bash scripts.
- Runtime config per engine family lives in [`profiles.json`](profiles.json).
- The Tauri shell on top is optional polish; CLI is the source of truth.
- [`scripts/validate-profiles.sh`](scripts/validate-profiles.sh) is the CI
  gate. It catches wine-grammar bugs and stale claims; if your PR fails
  it, fix the profile before merging.
- [`scripts/cellar-doctor.sh`](scripts/cellar-doctor.sh) is the user-side
  health check. New runtime dependencies should be added there too.

## Common tasks

### Adding a new game to an already-covered engine family

No code change. Drop the game under `~/Games-source/<Name>/` and run:

```sh
scripts/find-profile.sh "<Name>"          # confirm which profile matches
scripts/cellar-install.sh <profile> "<Name>"
scripts/make-cellar-app.sh <profile> "<Name>"
```

If the profile auto-match doesn't fire because the game name doesn't
include a known substring, add the substring to
`profiles.json` -> `<profile>.match_name_contains[]` and submit the
one-line PR. Run `scripts/validate-profiles.sh` first.

### Adding a new engine-family profile

Use the [new-profile issue template](.github/ISSUE_TEMPLATE/new-profile.yml)
to scope, then add an entry to `profiles.json`. Schema:

```json
{
  "id": "kebab-case-id",
  "name": "Human-readable name",
  "match_name_contains": ["substring 1", "substring 2"],
  "description": "Engine, API, anti-cheat status, launcher requirement, any gotchas.",
  "settings": {
    "dxvk": false,
    "esync": false,
    "msync": true,
    "metal_fences": false,
    "metal_hud": false,
    "dll_overrides": "winemenubuilder.exe=;d3d11,d3d12,dxgi,d3d10core=n,b;nvapi,nvapi64=",
    "env": { "ROSETTA_ADVERTISE_AVX": "1" },
    "launch_args": []
  },
  "requires": ["winetricks_vcrun2019", "winetricks_corefonts", "winetricks_d3dcompiler_47"]
}
```

Key constraints (validator enforces):
- `dll_overrides` tokens must be wine grammar: `n` / `b` / `n,b` / `b,n` /
  empty. `disabled` and `d` are NOT valid (wine silently ignores them).
- `requires` entries that are winetricks verbs must use the known set
  (`vcrun2019 vcrun2022 corefonts d3dcompiler_47 dotnet48 mf msxml3
  msxml6 wmp10 wmp11 quartz dxvk`). Other prefixes (`proton_winrt_dlls`,
  `homebrew_*`) are user-facing hints.
- `launch_args` must not include flags that don't apply to the matched
  titles (e.g. no `-dx11` on Elden Ring / Hogwarts Legacy — both
  DX12-only at engine build).

### Adding a dedicated launcher

Some engines need engine-specific tweaks the generic profile can't carry
because the profile is shared (e.g. RDR2's `-sgadriver=Vulkan` doesn't
apply to GTA V which uses the same `rage-rockstar` profile). Pattern:

```bash
#!/bin/bash
# launch-FOO.sh — thin wrapper around launch-engine.sh
set -u
CELLAR_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GAME="${1:?game dir name required}"
exec /bin/bash "$CELLAR_ROOT/scripts/launch-engine.sh" PROFILE_ID "$GAME" EXTRA_GAME_ARGS
```

See `scripts/launch-rdr2.sh` and `scripts/launch-bethesda.sh` for
working examples (the latter detects SKSE / F4SE loaders and passes
`--exe` to override the auto-resolution).

## What NOT to submit

- **Scene-release-group prescriptions.** Don't name SteamRIP, Online-Fix,
  MKDEV, SkidrowReloaded, CODEX, etc. as recommended sources. cellar's
  position is "the user supplies a standalone cracked / pre-installed
  build" without endorsing a specific release group. Tools that exist as
  open source on GitHub themselves (Goldberg / gbe_fork, UplayR1Unlocker,
  ExeIntegrityBypassAgainstRGL) are fine to reference; they're
  no different from any other GitHub-hosted dependency.
- **Game files.** This repo distributes nothing. Game files are entirely
  the user's responsibility.
- **Profiles for kernel-mode anti-cheat games.** Valorant (Vanguard),
  Fortnite (BattlEye+EAC), CoD MW2+ (Ricochet), PUBG online. They will
  never run under wine on any platform.

## Code style

- Bash scripts: `set -u` (not `set -e` — we often want to continue past
  individual failures and report at the end), explicit error messages to
  stderr with `>&2`, `bash -n` clean.
- JSON / Markdown: keep diffs minimal; if you're touching a profile,
  touch only that entry.
- Commit messages: `<area>: <one-line summary>` followed by a paragraph
  explaining the why. Match the existing CHANGELOG voice (factual,
  technical, no marketing).

## License

By submitting a PR you agree your contribution will be licensed under
the MIT License (see [`LICENSE`](LICENSE)).
