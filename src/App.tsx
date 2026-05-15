/**
 * cellar: top-level layout. Sidebar with Library / Install / Settings,
 * right pane renders the active tab.
 */

import { useState } from 'react';
import Library from './components/Library';
import InstallWizard from './components/InstallWizard';
import SettingsPane from './components/SettingsPane';

type Tab = 'library' | 'install' | 'settings';

export default function App() {
  const [tab, setTab] = useState<Tab>('library');

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
