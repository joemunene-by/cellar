/**
 * Settings tab: runtime status + bottle management.
 *
 * Runtime status checks whether wine64 is on disk, whether Apple's
 * Game Porting Toolkit is installed, and whether Rosetta 2 is present.
 * Bottle management lists existing bottles and lets the user delete
 * one (warning: this wipes the prefix directory).
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { runtime, wine, isCellarError } from '../lib/invoke';
import type { Bottle, RuntimeStatus } from '../lib/invoke';

const WINETRICKS_VERBS: { verb: string; label: string; minutes: string }[] = [
  { verb: 'vcrun2022', label: 'Visual C++ 2015-2022', minutes: '~2-4 min' },
  { verb: 'vcrun2019', label: 'Visual C++ 2015-2019', minutes: '~2-4 min' },
  { verb: 'dotnet48', label: '.NET Framework 4.8', minutes: '~5-10 min' },
  { verb: 'corefonts', label: 'Core fonts (Tahoma, Arial, ...)', minutes: '~1 min' },
];

export default function SettingsPane() {
  const [status, setStatus] = useState<RuntimeStatus | null>(null);
  const [bottles, setBottles] = useState<Bottle[]>([]);
  const [dxvkState, setDxvkState] = useState<Record<string, boolean>>({});
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [wineTest, setWineTest] = useState<{ ok: boolean; output: string } | null>(null);
  const [testingWine, setTestingWine] = useState(false);

  const reload = useCallback(async () => {
    setError(null);
    try {
      const [s, bs] = await Promise.all([runtime.status(), wine.listBottles()]);
      setStatus(s);
      setBottles(bs);
      const probes = await Promise.all(
        bs.map(async (b) => [b.id, await wine.bottleDxvkStatus(b.id).catch(() => false)] as const),
      );
      setDxvkState(Object.fromEntries(probes));
    } catch (err) {
      setError(formatErr(err));
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  const testWine = async () => {
    setTestingWine(true);
    setWineTest(null);
    try {
      const out = await runtime.testWine();
      setWineTest({ ok: true, output: out });
    } catch (err) {
      setWineTest({ ok: false, output: formatErr(err) });
    } finally {
      setTestingWine(false);
    }
  };

  const removeBottle = async (b: Bottle) => {
    if (!confirm(`Delete bottle "${b.name}"? This removes the Wine prefix and any games installed inside.`)) {
      return;
    }
    try {
      await wine.removeBottle(b.id);
      await reload();
    } catch (err) {
      setError(formatErr(err));
    }
  };

  const injectDxvk = async (b: Bottle) => {
    setBusyId(b.id);
    try {
      await wine.injectDxvk(b.id);
      const ok = await wine.bottleDxvkStatus(b.id);
      setDxvkState((s) => ({ ...s, [b.id]: ok }));
    } catch (err) {
      setError(formatErr(err));
    } finally {
      setBusyId(null);
    }
  };

  const [smokeResults, setSmokeResults] = useState<Record<string, { ok: boolean; detail: string }>>({});

  const smokeTest = async (b: Bottle) => {
    setBusyId(b.id);
    setSmokeResults((s) => ({ ...s, [b.id]: { ok: false, detail: 'running...' } }));
    try {
      const r = await wine.bottleSmokeTest(b.id);
      const detail = r.ok
        ? `wine + cmd.exe + prefix all working (exit ${r.exit_code})`
        : `wine ran but the marker did not echo (exit ${r.exit_code}). stderr tail: ${r.stderr.split('\n').slice(-2).join(' | ')}`;
      setSmokeResults((s) => ({ ...s, [b.id]: { ok: r.ok, detail } }));
    } catch (err) {
      setSmokeResults((s) => ({ ...s, [b.id]: { ok: false, detail: formatErr(err) } }));
    } finally {
      setBusyId(null);
    }
  };

  // Winetricks UI state. One operation at a time, shared log panel.
  const [trickVerb, setTrickVerb] = useState<Record<string, string>>({});
  const [trickRunning, setTrickRunning] = useState<{ bottleId: string; verb: string } | null>(null);
  const [trickLog, setTrickLog] = useState<string[]>([]);
  const [trickResult, setTrickResult] = useState<{ verb: string; exit_code: number } | null>(null);
  const trickLogRef = useRef<HTMLPreElement | null>(null);

  useEffect(() => {
    let mounted = true;
    let unlist: UnlistenFn[] = [];
    (async () => {
      const u1 = await listen<{ bottle_id: string; verb: string; line: string; stream: string }>(
        'cellar://winetricks',
        (e) => {
          setTrickLog((l) => [
            ...l,
            `${e.payload.stream === 'stderr' ? '[err] ' : ''}${e.payload.line}`,
          ]);
        },
      );
      const u2 = await listen<{ bottle_id: string; verb: string; exit_code: number }>(
        'cellar://winetricks-done',
        (e) => {
          setTrickResult({ verb: e.payload.verb, exit_code: e.payload.exit_code });
          setTrickRunning(null);
        },
      );
      if (!mounted) {
        u1();
        u2();
        return;
      }
      unlist = [u1, u2];
    })();
    return () => {
      mounted = false;
      unlist.forEach((u) => u());
    };
  }, []);

  useEffect(() => {
    if (trickLogRef.current) trickLogRef.current.scrollTop = trickLogRef.current.scrollHeight;
  }, [trickLog]);

  const runVerb = async (b: Bottle) => {
    const verb = trickVerb[b.id] ?? WINETRICKS_VERBS[0].verb;
    setTrickLog([`$ winetricks --unattended ${verb}`]);
    setTrickResult(null);
    setTrickRunning({ bottleId: b.id, verb });
    try {
      await wine.runWinetricks(b.id, verb);
    } catch (err) {
      setTrickRunning(null);
      setError(formatErr(err));
    }
  };

  return (
    <div className="pane">
      <header className="pane-header">
        <h2 className="pane-title">Settings</h2>
        <button className="btn" onClick={reload} type="button">
          Refresh
        </button>
      </header>

      {error && <div className="error-box">{error}</div>}

      <section className="settings-section">
        <h3>Runtime</h3>
        {!status ? (
          <p className="muted">Loading...</p>
        ) : (
          <table className="kv">
            <tbody>
              <tr>
                <td>Wine binary</td>
                <td>{status.wine_path ? <code>{status.wine_path}</code> : <span className="bad">missing</span>}</td>
              </tr>
              <tr>
                <td>Wine version</td>
                <td>{status.wine_version ?? <span className="muted">n/a</span>}</td>
              </tr>
              <tr>
                <td>Game Porting Toolkit</td>
                <td>{status.gptk_present ? <span className="ok">installed</span> : <span className="bad">missing</span>}</td>
              </tr>
              <tr>
                <td>Rosetta 2</td>
                <td>{status.rosetta_installed ? <span className="ok">installed</span> : <span className="bad">missing</span>}</td>
              </tr>
            </tbody>
          </table>
        )}
        {status && (!status.wine_path || !status.gptk_present || !status.rosetta_installed) && (
          <div className="hint-box">
            One or more pieces of the runtime stack are missing. Run{' '}
            <code>./scripts/setup-gptk.sh</code> from the cellar repo root to install them.
          </div>
        )}

        {status?.wine_path && (
          <div className="wine-test-row">
            <button className="btn" onClick={testWine} disabled={testingWine} type="button">
              {testingWine ? 'Testing...' : 'Test wine'}
            </button>
            {wineTest && (
              <code className={wineTest.ok ? 'ok' : 'bad'}>
                {wineTest.output}
              </code>
            )}
          </div>
        )}
      </section>

      <section className="settings-section">
        <h3>Bottles</h3>
        {bottles.length === 0 ? (
          <p className="muted">No bottles yet. Create one from the Install tab.</p>
        ) : (
          <ul className="bottle-list">
            {bottles.map((b) => (
              <li key={b.id}>
                <div className="bottle-row">
                  <div>
                    <strong>{b.name}</strong>
                    <div className="bottle-meta">
                      {b.windows_version}, created {new Date(b.created_ms).toLocaleString()} ({b.id.slice(0, 8)})
                    </div>
                    <div className="bottle-meta">
                      DXVK: {dxvkState[b.id] ? <span className="ok">injected</span> : <span className="bad">missing</span>}
                    </div>
                  </div>
                  <div className="bottle-actions">
                    <button
                      className="btn"
                      onClick={() => smokeTest(b)}
                      disabled={busyId === b.id}
                      type="button"
                      title="Run wine64 cmd /c echo against the bottle to confirm it works end-to-end"
                    >
                      Smoke test
                    </button>
                    <button
                      className="btn"
                      onClick={() => injectDxvk(b)}
                      disabled={busyId === b.id}
                      type="button"
                    >
                      {busyId === b.id ? 'Injecting...' : dxvkState[b.id] ? 'Re-inject DXVK' : 'Inject DXVK'}
                    </button>
                    <button className="btn btn-ghost" onClick={() => removeBottle(b)} type="button">
                      Delete
                    </button>
                  </div>
                </div>
                {smokeResults[b.id] && (
                  <div className={`smoke-result ${smokeResults[b.id].ok ? 'ok' : 'bad'}`}>
                    {smokeResults[b.id].ok ? '✓ ' : '✗ '}
                    {smokeResults[b.id].detail}
                  </div>
                )}
                <div className="winetricks-row">
                  <select
                    className="select"
                    value={trickVerb[b.id] ?? WINETRICKS_VERBS[0].verb}
                    onChange={(e) => setTrickVerb((s) => ({ ...s, [b.id]: e.target.value }))}
                    disabled={!!trickRunning}
                  >
                    {WINETRICKS_VERBS.map((v) => (
                      <option key={v.verb} value={v.verb}>
                        {v.label} ({v.minutes})
                      </option>
                    ))}
                  </select>
                  <button
                    className="btn"
                    onClick={() => runVerb(b)}
                    disabled={!!trickRunning}
                    type="button"
                  >
                    {trickRunning?.bottleId === b.id
                      ? `Installing ${trickRunning.verb}...`
                      : 'Install runtime'}
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}

        {(trickRunning || trickLog.length > 0) && (
          <div className="winetricks-panel">
            <div className="winetricks-header">
              <span>
                {trickRunning
                  ? `Running winetricks ${trickRunning.verb} on bottle ${trickRunning.bottleId.slice(0, 8)}`
                  : `Last run: ${trickResult?.verb ?? '?'} exited ${trickResult?.exit_code ?? '?'}`}
              </span>
              {!trickRunning && (
                <button
                  className="btn btn-ghost"
                  onClick={() => {
                    setTrickLog([]);
                    setTrickResult(null);
                  }}
                  type="button"
                >
                  Clear
                </button>
              )}
            </div>
            <pre className="winetricks-log" ref={trickLogRef}>
              {trickLog.join('\n') || '...'}
            </pre>
          </div>
        )}
      </section>

      <section className="settings-section">
        <h3>Data</h3>
        <p className="muted">
          cellar stores everything under <code>~/.cellar/</code>. Library: <code>library.json</code>.
          Bottles: <code>bottles/&lt;id&gt;/prefix</code>.
        </p>
      </section>
    </div>
  );
}

function formatErr(err: unknown): string {
  const e = isCellarError(err);
  if (!e) return String(err);
  if (e.kind === 'wine_missing') return 'Wine binary not found. Run scripts/setup-gptk.sh.';
  if (e.kind === 'home_not_found') return 'Could not resolve $HOME on this Mac.';
  if (e.kind === 'not_found') return `Not found: ${e.id ?? e.path ?? '?'}`;
  if (e.kind === 'bottle_error') return `Bottle error: ${e.message ?? 'unknown'}`;
  if (e.kind === 'spawn_failed') return `Spawn failed: ${e.message ?? 'unknown'}`;
  if (e.kind === 'io_error') return `Filesystem error: ${e.message ?? 'unknown'}`;
  if (e.kind === 'already_exists') return `Already exists (${e.id ?? '?'}).`;
  return `${e.kind}${e.message ? `: ${e.message}` : ''}`;
}
