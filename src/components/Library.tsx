/**
 * Library tab: card grid of installed games. Click a card to launch
 * the game in its bottle. Remove via the per-card menu.
 *
 * Phase 1 keeps this minimal: name + bottle id + launch button. Future
 * phases will add cover art, last played, total play time, per-game
 * settings drawer.
 */

import { useCallback, useEffect, useState } from 'react';
import { library, runtime, isCellarError } from '../lib/invoke';
import type { Game } from '../lib/invoke';

export default function Library() {
  const [games, setGames] = useState<Game[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

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

  const launch = async (game: Game) => {
    setBusyId(game.id);
    setError(null);
    try {
      await runtime.launch(game.id);
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
          {games.map((g) => (
            <li key={g.id} className="game-card">
              <div className="game-card-title">{g.name}</div>
              <div className="game-card-meta">
                <span title={g.bottle_id}>bottle {g.bottle_id.slice(0, 8)}</span>
                {g.last_played_ms && (
                  <span>last played {new Date(g.last_played_ms).toLocaleDateString()}</span>
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
                <button className="btn btn-ghost" onClick={() => remove(g)} type="button">
                  Remove
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function formatErr(err: unknown): string {
  const e = isCellarError(err);
  if (!e) return String(err);
  if (e.kind === 'wine_missing') return 'Wine binary not found. Run scripts/setup-gptk.sh.';
  if (e.kind === 'game_not_found') return `Game not found (id: ${e.id ?? '?'})`;
  if (e.kind === 'bottle_error') return `Bottle error: ${e.message ?? 'unknown'}`;
  if (e.kind === 'spawn_failed') return `Spawn failed: ${e.message ?? 'unknown'}`;
  return e.kind;
}
