/**
 * Settings tab: runtime status + bottle management.
 *
 * Runtime status checks whether wine64 is on disk, whether Apple's
 * Game Porting Toolkit is installed, and whether Rosetta 2 is present.
 * Bottle management lists existing bottles and lets the user delete
 * one (warning: this wipes the prefix directory).
 */

import { useCallback, useEffect, useState } from 'react';
import { runtime, wine, isCellarError } from '../lib/invoke';
import type { Bottle, RuntimeStatus } from '../lib/invoke';

export default function SettingsPane() {
  const [status, setStatus] = useState<RuntimeStatus | null>(null);
  const [bottles, setBottles] = useState<Bottle[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [wineTest, setWineTest] = useState<{ ok: boolean; output: string } | null>(null);
  const [testingWine, setTestingWine] = useState(false);

  const reload = useCallback(async () => {
    setError(null);
    try {
      const [s, bs] = await Promise.all([runtime.status(), wine.listBottles()]);
      setStatus(s);
      setBottles(bs);
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
                  </div>
                  <button className="btn btn-ghost" onClick={() => removeBottle(b)} type="button">
                    Delete
                  </button>
                </div>
              </li>
            ))}
          </ul>
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
  if (e.kind === 'not_found') return `Not found: ${e.id ?? e.path ?? '?'}`;
  return e.kind;
}
