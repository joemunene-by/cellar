/**
 * cellar: top-level layout. Sidebar with Library / Install / Settings,
 * right pane renders the active tab.
 */

import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import Library from './components/Library';
import InstallWizard from './components/InstallWizard';
import SettingsPane from './components/SettingsPane';
import type { GameDetected } from './lib/invoke';

type Tab = 'library' | 'install' | 'settings';

export default function App() {
  const [tab, setTab] = useState<Tab>('library');
  // Toasts raised by the ~/Games-source watcher (cellar://game-detected).
  const [detected, setDetected] = useState<GameDetected[]>([]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<GameDetected>('cellar://game-detected', (e) => {
      // De-dupe by path in case the watcher double-fires.
      setDetected((prev) =>
        prev.some((d) => d.path === e.payload.path) ? prev : [...prev, e.payload],
      );
    }).then((u) => {
      unlisten = u;
    });
    return () => unlisten?.();
  }, []);

  const dismiss = (path: string) =>
    setDetected((prev) => prev.filter((d) => d.path !== path));

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="sidebar-brand">cellar</div>
        <nav className="sidebar-nav">
          <TabButton active={tab === 'library'} label="Library" onClick={() => setTab('library')} />
          <TabButton active={tab === 'install'} label="Install" onClick={() => setTab('install')} />
          <TabButton active={tab === 'settings'} label="Settings" onClick={() => setTab('settings')} />
        </nav>
        <div className="sidebar-footer">v0.1.0</div>
      </aside>
      <main className="main">
        {tab === 'library' && <Library />}
        {tab === 'install' && <InstallWizard />}
        {tab === 'settings' && <SettingsPane />}
      </main>

      {detected.length > 0 && (
        <div className="toast-stack">
          {detected.map((d) => (
            <div className="toast" key={d.path}>
              <button
                className="toast-close"
                onClick={() => dismiss(d.path)}
                type="button"
                title="Dismiss"
              >
                ✕
              </button>
              <div className="toast-title">New game detected</div>
              <div className="toast-body">
                <strong>{d.name}</strong>
                {d.profile_name ? (
                  <span> matched profile <strong>{d.profile_name}</strong>.</span>
                ) : (
                  <span> — no engine profile matched (pick one manually).</span>
                )}
              </div>
              <div className="toast-cmd">
                <code>{d.suggested_cmd}</code>
                <button
                  className="btn btn-ghost btn-small"
                  type="button"
                  onClick={() => navigator.clipboard?.writeText(d.suggested_cmd)}
                  title="Copy install command"
                >
                  Copy
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function TabButton({ active, label, onClick }: { active: boolean; label: string; onClick: () => void }) {
  return (
    <button
      className={`sidebar-tab${active ? ' sidebar-tab-active' : ''}`}
      onClick={onClick}
      type="button"
    >
      {label}
    </button>
  );
}
