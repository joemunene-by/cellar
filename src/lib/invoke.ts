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
  /// MoltenVK fences for low-overhead GPU sync; only matters when DXVK is on.
  metal_fences: boolean;
  /// MTL_HUD_ENABLED=1 — Apple's Metal HUD overlay (FPS / GPU / frame time).
  metal_hud: boolean;
  /// Extra `WINEDLLOVERRIDES` entries appended to DXVK's defaults.
  /// Semicolon-separated, e.g. `mf=b;mfplat=b;mfreadwrite=b`.
  dll_overrides: string | null;
  env: Record<string, string>;
  launch_args: string[];
}

export interface Profile {
  id: string;
  name: string;
  match_name_contains: string[];
  description: string;
  settings: GameSettings;
  requires: string[];
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

export const profiles = {
  list: () => invoke<Profile[]>('profiles_list'),
  find: (gameName: string) => invoke<Profile | null>('profiles_find', { gameName }),
};

export interface CheckResult {
  satisfied: boolean;
  detail: string | null;
}

export const prereq = {
  install: (bottleId: string, requireId: string) =>
    invoke<void>('prereq_install', { bottleId, requireId }),
  check: (bottleId: string, requireId: string) =>
    invoke<CheckResult>('prereq_check', { bottleId, requireId }),
  checkAll: (bottleId: string, requireIds: string[]) =>
    invoke<Record<string, CheckResult>>('prereq_check_all', { bottleId, requireIds }),
};

/** Event payloads. cellar://prereq fires per output line during a
 *  prereq install; cellar://prereq-done fires once when it finishes
 *  (success or failure). Subscribe via @tauri-apps/api/event listen. */
export interface PrereqLine {
  bottle_id: string;
  require_id: string;
  line: string;
  /** "stdout" / "stderr" / "info" */
  stream: string;
}

export interface PrereqDone {
  bottle_id: string;
  require_id: string;
  success: boolean;
  detail: string;
}

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

// ----------------------- Archive (native FreeArc reader) -----------------------

export interface PeekFile {
  path: string;
  size: number;
  crc: number;
  is_dir: boolean;
}

export type PeekSupport = 'native' | 'hybrid' | 'unsupported';

export interface PeekCodec {
  method: string;
  /** "native" works with no help; "hybrid" needs the wine-side CLS
   *  plugin host plus a matching cls-*.dll; "unsupported" means no
   *  decode path exists in this version. */
  support: PeekSupport;
}

export interface ArchivePeek {
  archive_path: string;
  archive_bytes: number;
  file_count: number;
  total_uncompressed_bytes: number;
  codecs: PeekCodec[];
  partial_reason: string | null;
  files: PeekFile[];
}

export const archive = {
  peek: (path: string) => invoke<ArchivePeek>('archive_peek', { path }),
};

// ----------------------- Diagnostics (bottle inspect + crash report) -----------------------

export interface CrashReport {
  /** Absolute path to the generated .zip, or null if not parsed. */
  zip_path: string | null;
  /** The crash-report script's full stdout. */
  log: string;
}

export const tools = {
  /** Plain-text bottle report (prefix size, wine version, overrides,
   *  installed programs, winetricks verbs, save backups, last log). */
  bottleInspect: (bottleId: string) => invoke<string>('bottle_inspect', { bottleId }),
  /** Bundle a crash report .zip for a bottle. */
  crashReport: (bottleId: string) => invoke<CrashReport>('crash_report', { bottleId }),
};

// ----------------------- D3DMetal per-bottle version pin -----------------------

export const d3dmetal = {
  /** Version labels the user can pin to: "default" plus any local installs. */
  list: () => invoke<string[]>('d3dmetal_list'),
  /** The version pinned for a bottle, or null when it uses the runtime default. */
  get: (bottleId: string) => invoke<string | null>('d3dmetal_get', { bottleId }),
  /** Pin a bottle to a version; "default" unpins. */
  set: (bottleId: string, version: string) =>
    invoke<void>('d3dmetal_set', { bottleId, version }),
};

// ----------------------- Games-source watcher event -----------------------

/** Payload of the cellar://game-detected event emitted when a new game
 *  directory appears under ~/Games-source/. Subscribe via
 *  @tauri-apps/api/event listen. */
export interface GameDetected {
  name: string;
  path: string;
  profile_id: string | null;
  profile_name: string | null;
  suggested_cmd: string;
}

/** Best-effort discrimination on the {kind, ...} error shape used by all
 *  cellar backend commands. */
export function isCellarError(err: unknown): { kind: string; [k: string]: unknown } | null {
  if (typeof err === 'object' && err && 'kind' in err) {
    return err as { kind: string; [k: string]: unknown };
  }
  return null;
}
