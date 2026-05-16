/**
 * Typed wrappers around `@tauri-apps/api/core` invoke().
 *
 * Every native command lives behind a function here so React components
 * import a stable, typed surface rather than calling raw command names.
 *
 * Tauri 2 auto-converts JS `camelCase` arg keys to Rust `snake_case`
 * on the wire (e.g. JS `windowsVersion` maps to Rust `windows_version`).
 * We always pass camelCase from this module; the Rust side keeps its
 * idiomatic naming. Sending snake_case from JS produces a
 * `missing required key <camelCaseName>` error.
 */

import { invoke } from '@tauri-apps/api/core';

// ----------------------- Types -----------------------

export interface Bottle {
  id: string;
  name: string;
  windows_version: string;
  created_ms: number;
  /// Absolute path to the Wine prefix (the directory whose drive_c
  /// subdir is the guest C:\). Populated by both create + list.
  prefix_path: string;
}

export interface GameSettings {
  dxvk: boolean;
  esync: boolean;
  msync: boolean;
  env: Record<string, string>;
  launch_args: string[];
}

export interface Game {
  id: string;
  name: string;
  bottle_id: string;
  install_dir: string;
  launch_exe: string;
  last_played_ms: number | null;
  total_play_ms: number;
  settings: GameSettings;
}

export interface RuntimeStatus {
  wine_path: string | null;
  wine_version: string | null;
  gptk_present: boolean;
  rosetta_installed: boolean;
}

export type InstallerKind = 'FitGirl' | 'Dodi' | 'Kaos' | 'InnoSetup' | 'Msi' | 'Unknown';

export interface DetectResult {
  kind: InstallerKind;
  setup_exe: string | null;
  hints: string[];
}

// ----------------------- API -----------------------

export interface ExeCandidate {
  path: string;
  name: string;
  parent_dir: string;
  size: number;
  modified_ms: number;
}

export const wine = {
  createBottle: (name: string, windowsVersion: string) =>
    invoke<Bottle>('wine_create_bottle', { name, windowsVersion }),
  listBottles: () => invoke<Bottle[]>('wine_list_bottles'),
  removeBottle: (id: string) => invoke<void>('wine_remove_bottle', { id }),
  injectDxvk: (id: string) => invoke<void>('wine_inject_dxvk', { id }),
  bottleDxvkStatus: (id: string) => invoke<boolean>('wine_bottle_dxvk_status', { id }),
  bottleSmokeTest: (id: string) =>
    invoke<{ ok: boolean; stdout: string; stderr: string; exit_code: number }>('wine_bottle_smoke_test', { id }),
  scanBottleExes: (id: string, maxCount = 20) =>
    invoke<ExeCandidate[]>('wine_scan_bottle_exes', { id, maxCount }),
  runWinetricks: (id: string, verb: string) =>
    invoke<number>('wine_run_winetricks', { id, verb }),
};

export const library = {
  list: () => invoke<Game[]>('library_list'),
  add: (params: { name: string; bottleId: string; installDir: string; launchExe: string }) =>
    invoke<Game>('library_add', params),
  remove: (id: string) => invoke<void>('library_remove', { id }),
  updateSettings: (id: string, settings: GameSettings) =>
    invoke<void>('library_update_settings', { id, settings }),
};

export const runtime = {
  status: () => invoke<RuntimeStatus>('runtime_status'),
  testWine: () => invoke<string>('runtime_test_wine'),
  launch: (gameId: string) => invoke<void>('runtime_launch', { gameId }),
};

export const installer = {
  detect: (path: string) => invoke<DetectResult>('installer_detect', { path }),
  run: (bottleId: string, installerExe: string) =>
    invoke<number>('installer_run', { bottleId, installerExe }),
};

/** Best-effort discrimination on the {kind, ...} error shape used by all
 *  cellar backend commands. */
export function isCellarError(err: unknown): { kind: string; [k: string]: unknown } | null {
  if (typeof err === 'object' && err && 'kind' in err) {
    return err as { kind: string; [k: string]: unknown };
  }
  return null;
}
