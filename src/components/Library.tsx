/**
 * Library tab: card grid of installed games plus a slide-out settings
 * drawer per game.
 *
 *   - Click Play to launch via wine.
 *   - Click Settings on a card to slide in the drawer: toggles for
 *     DXVK / ESYNC / MSYNC, custom env vars, extra launch args.
 *   - Last-played and total play time are stamped by runtime_launch
 *     and a background tokio task that waits for the child exit.
 */

import { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { library, prereq, profiles, runtime, isCellarError } from '../lib/invoke';
import type { Game, GameSettings, PrereqDone, Profile } from '../lib/invoke';

export default function Library() {
  const [games, setGames] = useState<Game[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [editing, setEditing] = useState<Game | null>(null);
  // Bundled + user profiles, loaded once. Match per-card on the fly so
  // renames or profile updates take effect without a backend round-trip.
  const [profileList, setProfileList] = useState<Profile[]>([]);

  // Per-game readiness derived from prereq.checkAll. Three states:
  //   'ready'        every prereq in the matched profile is satisfied
  //   'needs_setup'  at least one prereq is missing
  //   'no_profile'   no profile matched, or matched profile has no requires
  type Readiness = 'ready' | 'needs_setup' | 'no_profile';
  const [readiness, setReadiness] = useState<Record<string, Readiness>>({});

  const reload = useCallback(async () => {
    setError(null);
    try {
      setGames(await library.list());
    } catch (err) {
      setError(formatErr(err));
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  useEffect(() => {
    profiles
      .list()
      .then(setProfileList)
      .catch(() => setProfileList([]));
  }, []);

  // Compute per-game readiness whenever the games list or profileList
  // changes. One prereq.checkAll per game whose name matches a profile
  // that has any requires. For a typical library (a handful of games)
  // that's a small enough number of round-trips to skip caching.
  useEffect(() => {
    if (!games || games.length === 0 || profileList.length === 0) return;
    let mounted = true;
    (async () => {
      const next: Record<string, Readiness> = {};
      for (const g of games) {
        const matched = matchProfile(profileList, g.name);
        if (!matched || matched.requires.length === 0) {
          next[g.id] = 'no_profile';
          continue;
        }
        try {
          const results = await prereq.checkAll(g.bottle_id, matched.requires);
          const allOk = Object.values(results).every((r) => r.satisfied);
          next[g.id] = allOk ? 'ready' : 'needs_setup';
        } catch {
          next[g.id] = 'needs_setup';
        }
      }
      if (mounted) setReadiness(next);
    })();
    return () => {
      mounted = false;
    };
  }, [games, profileList]);

  const launch = async (game: Game) => {
    setBusyId(game.id);
    setError(null);
    try {
      await runtime.launch(game.id);
      // Re-poll the library a moment later to pick up the last_played
      // bump from runtime_launch's mark_played_now call.
      setTimeout(reload, 800);
    } catch (err) {
      setError(formatErr(err));
    } finally {
      setBusyId(null);
    }
  };

  const remove = async (game: Game) => {
    if (!confirm(`Remove ${game.name} from the library? The install files stay on disk.`)) return;
    try {
      await library.remove(game.id);
      await reload();
    } catch (err) {
      setError(formatErr(err));
    }
  };

  if (games === null) {
    return <div className="pane-loading">Loading library...</div>;
  }

  return (
    <div className="pane">
      <header className="pane-header">
        <h2 className="pane-title">Library</h2>
        <button className="btn" onClick={reload} type="button">
          Refresh
        </button>
      </header>

      {error && <div className="error-box">{error}</div>}

      {games.length === 0 ? (
        <div className="empty-state">
          <h3>No games yet.</h3>
          <p>
            Head to the <strong>Install</strong> tab to add your first one. Point cellar at a
            FitGirl repack folder or a plain Windows installer .exe.
          </p>
        </div>
      ) : (
        <ul className="game-grid">
          {games.map((g) => {
            const matched = matchProfile(profileList, g.name);
            const r = readiness[g.id];
            return (
            <li key={g.id} className="game-card">
              <div className="game-card-title">
                {g.name}
                {matched && (
                  <span
                    className="profile-badge"
                    title={`Bundled profile: ${matched.name}\n${matched.description}`}
                  >
                    {matched.name}
                  </span>
                )}
                {r === 'ready' && (
                  <span className="ready-badge" title="All profile prerequisites installed">
                    ✓ Ready
                  </span>
                )}
                {r === 'needs_setup' && (
                  <button
                    className="needs-setup-badge"
                    title="One or more profile prerequisites are missing — click to open Settings and set them up"
                    type="button"
                    onClick={() => setEditing(g)}
                  >
                    ⚠ Needs setup
                  </button>
                )}
              </div>
              <div className="game-card-meta">
                <span title={g.bottle_id}>bottle {g.bottle_id.slice(0, 8)}</span>
                <span>{prettyDuration(g.total_play_ms)} played</span>
                {g.last_played_ms && (
                  <span>last {prettyAgo(g.last_played_ms)}</span>
                )}
              </div>
              <div className="game-card-exe" title={g.launch_exe}>
                {g.launch_exe.split(/[\\/]/).pop()}
              </div>
              <div className="game-card-actions">
                <button
                  className="btn btn-primary"
                  onClick={() => launch(g)}
                  disabled={busyId === g.id}
                  type="button"
                >
                  {busyId === g.id ? 'Launching...' : 'Play'}
                </button>
                <button className="btn" onClick={() => setEditing(g)} type="button">
                  Settings
                </button>
                <button className="btn btn-ghost" onClick={() => remove(g)} type="button">
                  Remove
                </button>
              </div>
            </li>
            );
          })}
        </ul>
      )}

      {editing && (
        <SettingsDrawer
          game={editing}
          onClose={() => setEditing(null)}
          onSaved={async () => {
            setEditing(null);
            await reload();
          }}
        />
      )}
    </div>
  );
}

function SettingsDrawer({
  game,
  onClose,
  onSaved,
}: {
  game: Game;
  onClose: () => void;
  onSaved: () => void;
}) {
  // Backfill defaults for older library.json files that predate the
  // metal_fences / metal_hud / dll_overrides fields.
  const [settings, setSettings] = useState<GameSettings>({
    ...game.settings,
    metal_fences: game.settings.metal_fences ?? false,
    metal_hud: game.settings.metal_hud ?? false,
    dll_overrides: game.settings.dll_overrides ?? null,
  });
  const [envText, setEnvText] = useState(envToText(game.settings.env));
  const [argsText, setArgsText] = useState(game.settings.launch_args.join('\n'));
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Profile picker state.
  const [profileList, setProfileList] = useState<Profile[] | null>(null);
  const [matchedProfile, setMatchedProfile] = useState<Profile | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);

  // Prereq install state, keyed by require_id.
  type PrereqState = 'idle' | 'running' | 'ok' | 'failed' | 'manual';
  const [prereqStatus, setPrereqStatus] = useState<Record<string, PrereqState>>({});
  const [prereqDetail, setPrereqDetail] = useState<Record<string, string>>({});
  const [setupAllRunning, setSetupAllRunning] = useState(false);

  useEffect(() => {
    let mounted = true;
    (async () => {
      try {
        const [all, matched] = await Promise.all([profiles.list(), profiles.find(game.name)]);
        if (!mounted) return;
        setProfileList(all);
        setMatchedProfile(matched);
        // Seed prereq satisfaction state for the matched profile. Best-
        // effort: any failure leaves entries in 'idle' so the user can
        // still click Install.
        if (matched && matched.requires.length > 0) {
          try {
            const results = await prereq.checkAll(game.bottle_id, matched.requires);
            if (!mounted) return;
            const status: Record<string, PrereqState> = {};
            const detail: Record<string, string> = {};
            for (const [rid, res] of Object.entries(results)) {
              if (res.satisfied) status[rid] = 'ok';
              if (res.detail) detail[rid] = res.detail;
            }
            setPrereqStatus(status);
            setPrereqDetail(detail);
          } catch {
            // soft fail: leave everything idle
          }
        }
      } catch {
        // Profiles are a soft feature — silently fall back if missing.
        if (mounted) setProfileList([]);
      }
    })();
    return () => {
      mounted = false;
    };
  }, [game.name, game.bottle_id]);

  useEffect(() => {
    let mounted = true;
    let unlist: UnlistenFn | null = null;
    (async () => {
      const u = await listen<PrereqDone>('cellar://prereq-done', (e) => {
        if (!mounted) return;
        if (e.payload.bottle_id !== game.bottle_id) return;
        setPrereqStatus((s) => ({
          ...s,
          [e.payload.require_id]: e.payload.success ? 'ok' : 'failed',
        }));
        setPrereqDetail((s) => ({ ...s, [e.payload.require_id]: e.payload.detail }));
      });
      if (mounted) unlist = u;
      else u();
    })();
    return () => {
      mounted = false;
      if (unlist) unlist();
    };
  }, [game.bottle_id]);

  const runPrereq = async (rid: string) => {
    setPrereqStatus((s) => ({ ...s, [rid]: 'running' }));
    setPrereqDetail((s) => ({ ...s, [rid]: 'starting...' }));
    try {
      await prereq.install(game.bottle_id, rid);
      // 'ok' state will land via the cellar://prereq-done listener.
    } catch (err) {
      const e = isCellarError(err);
      if (e?.kind === 'manual_action_required') {
        setPrereqStatus((s) => ({ ...s, [rid]: 'manual' }));
        setPrereqDetail((s) => ({ ...s, [rid]: String(e.hint ?? 'manual action required') }));
      } else {
        setPrereqStatus((s) => ({ ...s, [rid]: 'failed' }));
        setPrereqDetail((s) => ({ ...s, [rid]: formatErr(err) }));
      }
    }
  };

  // "Set up profile" — installs every unsatisfied prereq in sequence.
  // 'manual' rows are skipped silently (user has to handle them outside
  // cellar, e.g. `brew install gst-libav`); the row itself surfaces the
  // instruction so they know what to do.
  const setupAll = async () => {
    if (!matchedProfile) return;
    setSetupAllRunning(true);
    try {
      for (const rid of matchedProfile.requires) {
        const current = prereqStatus[rid] ?? 'idle';
        if (current === 'ok' || current === 'running' || current === 'manual') continue;
        await runPrereq(rid);
        // small pause so the cellar://prereq-done listener for the
        // previous step has a chance to flip its row to 'ok' before
        // the next one starts. Not strictly necessary for correctness
        // but feels much better in the UI.
        await new Promise((r) => setTimeout(r, 300));
      }
    } finally {
      setSetupAllRunning(false);
    }
  };

  const applyProfile = (p: Profile) => {
    if (!confirm(`Apply profile "${p.name}"? Overwrites every toggle, env var, dll_overrides, and launch args in this drawer (you still have to click Save to persist).`)) {
      return;
    }
    setSettings(p.settings);
    setEnvText(envToText(p.settings.env));
    setArgsText(p.settings.launch_args.join('\n'));
    setPickerOpen(false);
  };

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const next: GameSettings = {
        ...settings,
        env: parseEnvText(envText),
        launch_args: argsText.split('\n').map((s) => s.trim()).filter(Boolean),
      };
      await library.updateSettings(game.id, next);
      onSaved();
    } catch (err) {
      setError(formatErr(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="drawer-backdrop" onClick={onClose}>
      <div className="drawer" onClick={(e) => e.stopPropagation()}>
        <header className="drawer-header">
          <h3>{game.name}</h3>
          <button className="pane-btn" onClick={onClose} type="button" title="Close">
            ✕
          </button>
        </header>

        {error && <div className="error-box">{error}</div>}

        <section className="drawer-section">
          <h4>Profile</h4>
          {matchedProfile ? (
            <p className="muted">
              Matches bundled profile <strong>{matchedProfile.name}</strong>.{' '}
              {matchedProfile.description}
            </p>
          ) : (
            <p className="muted">No bundled profile matches this game name. Defaults apply.</p>
          )}
          {matchedProfile && matchedProfile.requires.length > 0 && (() => {
            const missing = matchedProfile.requires.filter((r) => {
              const s = prereqStatus[r] ?? 'idle';
              return s !== 'ok' && s !== 'manual';
            });
            const allDone = missing.length === 0;
            return (
              <>
                <div className="requires-header">
                  <span className="muted">
                    {allDone
                      ? 'All prerequisites installed.'
                      : `${missing.length} of ${matchedProfile.requires.length} prerequisite${matchedProfile.requires.length === 1 ? '' : 's'} missing.`}
                  </span>
                  {!allDone && (
                    <button
                      className="btn btn-primary btn-small"
                      onClick={setupAll}
                      disabled={setupAllRunning}
                      type="button"
                      title="Run every Install button in sequence. Manual-only entries (homebrew etc.) are skipped."
                    >
                      {setupAllRunning ? 'Setting up…' : `Set up profile (${missing.length})`}
                    </button>
                  )}
                </div>
                <ul className="requires-list">
                  {matchedProfile.requires.map((r) => {
                const status = prereqStatus[r] ?? 'idle';
                const detail = prereqDetail[r];
                return (
                  <li key={r} className={`require-row require-${status}`}>
                    <div className="require-name">
                      <code>{r}</code>
                      {detail && <span className="require-detail">{detail}</span>}
                    </div>
                    <button
                      className="btn btn-ghost btn-small"
                      disabled={status === 'running'}
                      onClick={() => runPrereq(r)}
                      type="button"
                    >
                      {status === 'idle' && 'Install'}
                      {status === 'running' && 'Installing…'}
                      {status === 'ok' && 'Re-install'}
                      {status === 'failed' && 'Retry'}
                      {status === 'manual' && 'Show how'}
                    </button>
                  </li>
                );
              })}
                </ul>
              </>
            );
          })()}
          <div className="profile-actions">
            <button
              className="btn"
              onClick={() => setPickerOpen((v) => !v)}
              type="button"
              disabled={!profileList || profileList.length === 0}
            >
              {pickerOpen ? 'Hide profiles' : 'Apply profile…'}
            </button>
            {matchedProfile && (
              <button
                className="btn btn-ghost"
                onClick={() => applyProfile(matchedProfile)}
                type="button"
                title="Re-apply the matched profile's settings, overwriting any local edits"
              >
                Re-apply {matchedProfile.name}
              </button>
            )}
          </div>
          {pickerOpen && profileList && (
            <ul className="profile-picker">
              {profileList.map((p) => (
                <li key={p.id} className="profile-row">
                  <div className="profile-info">
                    <strong>{p.name}</strong>
                    <div className="profile-desc">{p.description}</div>
                    {p.match_name_contains.length > 0 && (
                      <div className="profile-match">
                        matches: {p.match_name_contains.map((s) => `"${s}"`).join(', ')}
                      </div>
                    )}
                  </div>
                  <button
                    className="btn btn-ghost"
                    onClick={() => applyProfile(p)}
                    type="button"
                  >
                    Apply
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="drawer-section">
          <h4>Runtime toggles</h4>
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={settings.dxvk}
              onChange={(e) => setSettings({ ...settings, dxvk: e.target.checked })}
            />
            <span>DXVK (D3D11 / D3D10 / DXGI native-DLL override)</span>
          </label>
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={settings.esync}
              onChange={(e) => setSettings({ ...settings, esync: e.target.checked })}
            />
            <span>ESYNC (eventfd-style multi-thread sync)</span>
          </label>
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={settings.msync}
              onChange={(e) => setSettings({ ...settings, msync: e.target.checked })}
            />
            <span>MSYNC (Apple Silicon fast sync primitive)</span>
          </label>
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={settings.metal_fences}
              onChange={(e) => setSettings({ ...settings, metal_fences: e.target.checked })}
              disabled={!settings.dxvk}
            />
            <span>
              Metal Fences <code>MVK_ALLOW_METAL_FENCES=1</code>{' '}
              <span className="muted">(DXVK/MoltenVK path only)</span>
            </span>
          </label>
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={settings.metal_hud}
              onChange={(e) => setSettings({ ...settings, metal_hud: e.target.checked })}
            />
            <span>
              Metal HUD overlay <code>MTL_HUD_ENABLED=1</code>{' '}
              <span className="muted">(FPS / GPU / frame time)</span>
            </span>
          </label>
        </section>

        <section className="drawer-section">
          <h4>DLL overrides</h4>
          <p className="muted">
            Appended to DXVK's <code>d3d11,d3d10core,dxgi=n</code> when DXVK is on.
            Semicolon-separated. Example for Unity 2022 IL2CPP with native MF
            codecs: <code>mf=b;mfplat=b;mfreadwrite=b;mfmediaengine=b;mfsrcsnk=b</code>.
            For full override, set <code>WINEDLLOVERRIDES</code> in env vars instead.
          </p>
          <input
            className="input"
            type="text"
            value={settings.dll_overrides ?? ''}
            onChange={(e) =>
              setSettings({ ...settings, dll_overrides: e.target.value || null })
            }
            placeholder="winemenubuilder.exe=d;mf=b;mfplat=b"
          />
        </section>

        <section className="drawer-section">
          <h4>Env vars</h4>
          <p className="muted">One per line, `KEY=value`. Example: `DXVK_HUD=fps`.</p>
          <textarea
            className="input"
            rows={4}
            value={envText}
            onChange={(e) => setEnvText(e.target.value)}
            placeholder="DXVK_HUD=fps&#10;__GL_SYNC_TO_VBLANK=0"
          />
        </section>

        <section className="drawer-section">
          <h4>Extra launch args</h4>
          <p className="muted">One per line. Passed after the .exe.</p>
          <textarea
            className="input"
            rows={3}
            value={argsText}
            onChange={(e) => setArgsText(e.target.value)}
            placeholder="-nointro&#10;-windowed"
          />
        </section>

        <section className="drawer-section">
          <h4>Paths</h4>
          <table className="kv">
            <tbody>
              <tr><td>Install dir</td><td><code>{game.install_dir}</code></td></tr>
              <tr><td>Launch .exe</td><td><code>{game.launch_exe}</code></td></tr>
              <tr><td>Bottle</td><td><code>{game.bottle_id}</code></td></tr>
            </tbody>
          </table>
        </section>

        <div className="drawer-actions">
          <button className="btn btn-primary" onClick={save} disabled={saving} type="button">
            {saving ? 'Saving...' : 'Save'}
          </button>
          <button className="btn btn-ghost" onClick={onClose} type="button">
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

function envToText(env: Record<string, string>): string {
  return Object.entries(env)
    .map(([k, v]) => `${k}=${v}`)
    .join('\n');
}

function parseEnvText(text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const line of text.split('\n')) {
    const t = line.trim();
    if (!t || t.startsWith('#')) continue;
    const eq = t.indexOf('=');
    if (eq <= 0) continue;
    out[t.slice(0, eq).trim()] = t.slice(eq + 1).trim();
  }
  return out;
}

function matchProfile(list: Profile[], gameName: string): Profile | null {
  const needle = gameName.toLowerCase();
  for (const p of list) {
    for (const hint of p.match_name_contains) {
      if (needle.includes(hint.toLowerCase())) return p;
    }
  }
  return null;
}

function prettyDuration(ms: number): string {
  if (ms < 60_000) return '<1m';
  const minutes = Math.floor(ms / 60_000);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const remM = minutes % 60;
  if (hours < 24) return remM ? `${hours}h ${remM}m` : `${hours}h`;
  const days = Math.floor(hours / 24);
  return `${days}d ${hours % 24}h`;
}

function prettyAgo(ms: number): string {
  const elapsed = Date.now() - ms;
  if (elapsed < 60_000) return 'just now';
  if (elapsed < 3_600_000) return `${Math.floor(elapsed / 60_000)}m ago`;
  if (elapsed < 86_400_000) return `${Math.floor(elapsed / 3_600_000)}h ago`;
  const days = Math.floor(elapsed / 86_400_000);
  if (days < 30) return `${days}d ago`;
  return new Date(ms).toLocaleDateString();
}

function formatErr(err: unknown): string {
  const e = isCellarError(err);
  if (!e) return String(err);
  if (e.kind === 'wine_missing') return 'Wine binary not found. Run scripts/setup-gptk.sh.';
  if (e.kind === 'game_not_found') return `Game not found (id: ${e.id ?? '?'})`;
  if (e.kind === 'bottle_error') return `Bottle error: ${e.message ?? 'unknown'}`;
  if (e.kind === 'spawn_failed') return `Spawn failed: ${e.message ?? 'unknown'}`;
  if (e.kind === 'not_found') return `Not found: ${e.id ?? '?'}`;
  return `${e.kind}${e.message ? `: ${e.message}` : ''}`;
}
