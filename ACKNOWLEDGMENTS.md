# Acknowledgments

cellar wouldn't exist without the upstream projects that solve every
hard problem this launcher composes together. Concrete debts:

## Wine + CrossOver

- **[Wine](https://www.winehq.org/)** — the Win32 / Win64 implementation
  that everything else rides on top of. Decades of work to make
  `LoadLibrary` and `CreateFileMapping` mean the right thing on
  non-Windows hosts.
- **[CrossOver](https://www.codeweavers.com/crossover)** — CodeWeavers'
  productionized wine fork. cellar's runtime uses CrossOver 26's wine
  11.0 binary plus the bundled D3DMetal framework.

## Apple GPTK + Metal

- **[Apple Game Porting Toolkit](https://developer.apple.com/games/game-porting-toolkit/)**
  — the D3DMetal framework that translates D3D9 / D3D10 / D3D11 / D3D12
  calls into Metal. Without this, modern Windows games would not boot
  on Apple Silicon under wine at all.
- **[MoltenVK](https://github.com/KhronosGroup/MoltenVK)** — the
  Vulkan-over-Metal layer used by DXVK (and RDR2's `-sgadriver=Vulkan`
  path). Maintained by The Brenwill Workshop and the Khronos Group.

## Wine ecosystem

- **[Whisky](https://github.com/IsaacMarovitz/Whisky)** — Isaac Marovitz'
  Mac wine launcher, the prior art cellar learned the most from. The
  hybrid runtime approach (apple_gptk D3DMetal forwarders + wine 11)
  was directly inspired by Whisky's stack.
- **[DXVK](https://github.com/doitsujin/dxvk)** — Philip Rebohle's
  D3D11/D3D12-to-Vulkan translator. cellar uses it for the few engines
  where the D3DMetal direct path doesn't work.
- **[Winetricks](https://github.com/Winetricks/winetricks)** — the
  community recipe collection for installing common runtime DLLs into
  a wine prefix.
- **[Proton](https://github.com/ValveSoftware/Proton)** — Valve's
  steam-runtime fork of wine on Linux. cellar pulls Proton's WinRT
  family DLLs (coremessaging, wintypes, the Windows.* set) for engines
  that need `Windows.System.DispatcherQueue` activation.
- **[GE-Proton](https://github.com/GloriousEggroll/proton-ge-custom)**
  — Glorious Eggroll's Proton fork, source of the WinRT DLL tarball
  cellar's `install-proton-winrt.sh` consumes.

## Compatibility data

- **[ProtonDB](https://www.protondb.com)** — community wine compatibility
  ratings. Almost every "this works on Linux Proton, should work on
  cellar with X tweak" claim in cellar's docs traces back to a ProtonDB
  report.
- **[AppleGamingWiki](https://www.applegamingwiki.com)** — Mac-specific
  wine compatibility wiki.
- **[PCGamingWiki](https://www.pcgamingwiki.com)** — primary source for
  game-engine + launch-flag facts (e.g. RDR2's `-sgadriver=Vulkan`).
- **[CodeWeavers Compatibility DB](https://www.codeweavers.com/compatibility)**
  — first-party CrossOver compat info, particularly useful for FIFA 19
  (the only modern FIFA with a CodeWeavers entry).

## CLI tooling

- **[jq](https://github.com/jqlang/jq)** — Stephen Dolan's JSON
  processor. `launch-engine.sh`, `cellar-install.sh`,
  `validate-profiles.sh`, `find-profile.sh` all use it to consume
  `profiles.json`.
- **[cabextract](https://www.cabextract.org.uk/)** — Stuart Caie's
  cabinet-file extractor. Required by winetricks for `d3dcompiler_47`
  and other Microsoft-redistributable verbs.
- **[Paragon NTFS for Mac](https://www.paragon-software.com/home/ntfs-mac/)**
  — used as the writable-NTFS bridge during the GTA V install dance
  documented in CHANGELOG.

## Open-source DRM tools referenced for technical context

- **[Goldberg Emulator / gbe_fork](https://github.com/Detanup01/gbe_fork)**
  — Steam emulator. Standard piece of any wine-on-Mac launcher's toolkit
  for games whose retail builds insist on Steam being present.
- **[UplayR1Unlocker](https://github.com/acidicoala/UplayR1Unlocker)**
  — acidicoala's Ubisoft Connect bypass DLL. Referenced in the
  `anvilnext-ubisoft` profile description as the canonical replacement
  for the retail `uplay_r1_loader.dll`.

## Visual design

- The wine-rack-of-bottles mark in `assets/` is original work for cellar,
  inspired by the visual language of [Tailwind](https://tailwindcss.com)
  and [shadcn/ui](https://ui.shadcn.com) (geometric, flat, two-tone with
  a single accent).
