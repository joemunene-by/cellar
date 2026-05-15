/**
 * Install tab: a three-step wizard.
 *
 *   1. Pick a folder or installer .exe via the native file dialog.
 *   2. cellar detects the installer kind (FitGirl / DODI / KaOs /
 *      InnoSetup / MSI / unknown) and shows the resolved setup.exe.
 *   3. Pick or create a bottle, then run the installer. stdout / stderr
 *      stream into a live log; on success the user adds the game to
 *      the library with the install dir + launch .exe.
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { wine, installer, library, isCellarError } from '../lib/invoke';
import type { Bottle, DetectResult, InstallerKind } from '../lib/invoke';

type Step = 'pick' | 'detect' | 'bottle' | 'run' | 'register';

export default function InstallWizard() {
  const [step, setStep] = useState<Step>('pick');
  const [error, setError] = useState<string | null>(null);

  // Step 1: source path
  const [sourcePath, setSourcePath] = useState<string | null>(null);

  // Step 2: detection result
  const [detection, setDetection] = useState<DetectResult | null>(null);

  // Step 3: bottle selection
  const [bottles, setBottles] = useState<Bottle[]>([]);
  const [selectedBottle, setSelectedBottle] = useState<string | null>(null);
  const [newBottleName, setNewBottleName] = useState('');

  // Step 4: running installer
  const [log, setLog] = useState<string[]>([]);
  const [installRunning, setInstallRunning] = useState(false);
  const [installExitCode, setInstallExitCode] = useState<number | null>(null);
  const logRef = useRef<HTMLPreElement | null>(null);

  // Step 5: register game
  const [gameName, setGameName] = useState('');
  const [installDir, setInstallDir] = useState('');
  const [launchExe, setLaunchExe] = useState('');

  // Refresh bottles when reaching the bottle-pick step.
  useEffect(() => {
    if (step !== 'bottle') return;
    wine.listBottles().then(setBottles).catch((e) => setError(formatErr(e)));
  }, [step]);

  // Subscribe to installer events while running.
  useEffect(() => {
    if (!installRunning) return;
    let mounted = true;
    let unlist: UnlistenFn[] = [];
    (async () => {
      const u1 = await listen<{ line: string; stream: string }>('cellar://install', (e) => {
        setLog((l) => [...l, `${e.payload.stream === 'stderr' ? '[err] ' : ''}${e.payload.line}`]);
      });
      const u2 = await listen<{ exit_code: number }>('cellar://install-done', (e) => {
        setInstallExitCode(e.payload.exit_code);
        setInstallRunning(false);
        if (e.payload.exit_code === 0) {
          setStep('register');
        }
      });
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
  }, [installRunning]);

  // Auto-scroll the log.
  useEffect(() => {
    if (logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight;
  }, [log]);

  const pickSource = useCallback(async () => {
    setError(null);
    try {
      const picked = await open({
        multiple: false,
        directory: false,
        title: 'Pick a repack folder or installer .exe',
        filters: [{ name: 'Installer', extensions: ['exe', 'msi'] }],
      });
      if (typeof picked !== 'string') return;
      setSourcePath(picked);
      const det = await installer.detect(picked);
      setDetection(det);
      setStep('detect');
    } catch (err) {
      setError(formatErr(err));
    }
  }, []);

  const pickFolder = useCallback(async () => {
    setError(null);
    try {
      const picked = await open({
        multiple: false,
        directory: true,
        title: 'Pick a repack folder',
      });
      if (typeof picked !== 'string') return;
      setSourcePath(picked);
      const det = await installer.detect(picked);
      setDetection(det);
      setStep('detect');
    } catch (err) {
      setError(formatErr(err));
    }
  }, []);

  const createBottle = async () => {
    if (!newBottleName.trim()) return;
    try {
      const b = await wine.createBottle(newBottleName.trim(), 'win10');
      setBottles([b, ...bottles]);
      setSelectedBottle(b.id);
      setNewBottleName('');
    } catch (err) {
      setError(formatErr(err));
    }
  };

  const runInstaller = async () => {
    if (!detection?.setup_exe || !selectedBottle) return;
    setError(null);
    setLog([]);
    setInstallExitCode(null);
    setInstallRunning(true);
    setStep('run');
    try {
      await installer.run(selectedBottle, detection.setup_exe);
    } catch (err) {
      setInstallRunning(false);
      setError(formatErr(err));
    }
  };

  const registerGame = async () => {
    if (!selectedBottle || !gameName.trim() || !installDir.trim() || !launchExe.trim()) return;
    try {
      await library.add({
        name: gameName.trim(),
        bottleId: selectedBottle,
        installDir: installDir.trim(),
        launchExe: launchExe.trim(),
      });
      reset();
    } catch (err) {
      setError(formatErr(err));
    }
  };

  const pickInstallDir = async () => {
    const picked = await open({ multiple: false, directory: true, title: 'Pick the install directory inside the bottle' });
    if (typeof picked === 'string') setInstallDir(picked);
  };

  const pickLaunchExe = async () => {
    const picked = await open({
      multiple: false,
      directory: false,
      title: 'Pick the game .exe to launch',
      filters: [{ name: 'Executable', extensions: ['exe'] }],
    });
    if (typeof picked === 'string') setLaunchExe(picked);
  };

  const reset = () => {
    setStep('pick');
    setSourcePath(null);
    setDetection(null);
    setLog([]);
    setInstallExitCode(null);
    setGameName('');
    setInstallDir('');
    setLaunchExe('');
    setError(null);
  };

  return (
    <div className="pane">
      <header className="pane-header">
        <h2 className="pane-title">Install a game</h2>
        <span className="step-pill">step: {step}</span>
      </header>

      {error && <div className="error-box">{error}</div>}

      {step === 'pick' && (
        <div className="install-step">
          <p>Point cellar at a repack folder (recommended) or a single installer .exe.</p>
          <div className="install-actions">
            <button className="btn btn-primary" onClick={pickFolder} type="button">
              Pick repack folder
            </button>
            <button className="btn" onClick={pickSource} type="button">
              Pick installer .exe
            </button>
          </div>
        </div>
      )}

      {step === 'detect' && detection && (
        <div className="install-step">
          <h3>Detected: {kindLabel(detection.kind)}</h3>
          <div className="install-meta">
            <div>
              <span className="meta-label">source</span>
              <code>{sourcePath}</code>
            </div>
            <div>
              <span className="meta-label">setup .exe</span>
              <code>{detection.setup_exe ?? '(none found)'}</code>
            </div>
            {detection.hints.length > 0 && (
              <div>
                <span className="meta-label">hints</span>
                <ul className="hints">
                  {detection.hints.map((h, i) => (
                    <li key={i}>{h}</li>
                  ))}
                </ul>
              </div>
            )}
          </div>
          <div className="install-actions">
            <button className="btn btn-primary" onClick={() => setStep('bottle')} type="button" disabled={!detection.setup_exe}>
              Continue
            </button>
            <button className="btn btn-ghost" onClick={reset} type="button">
              Start over
            </button>
          </div>
        </div>
      )}

      {step === 'bottle' && (
        <div className="install-step">
          <h3>Pick a bottle</h3>
          <p>Each game gets its own Wine prefix so registry / DLL state stays isolated.</p>

          <ul className="bottle-list">
            {bottles.map((b) => (
              <li key={b.id}>
                <label>
                  <input
                    type="radio"
                    name="bottle"
                    value={b.id}
                    checked={selectedBottle === b.id}
                    onChange={() => setSelectedBottle(b.id)}
                  />
                  <strong>{b.name}</strong>
                  <span className="bottle-meta">
                    {b.windows_version}, created {new Date(b.created_ms).toLocaleDateString()}
                  </span>
                </label>
              </li>
            ))}
            {bottles.length === 0 && <li className="muted">No bottles yet. Create one below.</li>}
          </ul>

          <div className="new-bottle">
            <input
              className="input"
              placeholder="New bottle name (e.g. cyberpunk-bottle)"
              value={newBottleName}
              onChange={(e) => setNewBottleName(e.target.value)}
            />
            <button className="btn" onClick={createBottle} type="button" disabled={!newBottleName.trim()}>
              Create
            </button>
          </div>

          <div className="install-actions">
            <button
              className="btn btn-primary"
              onClick={runInstaller}
              type="button"
              disabled={!selectedBottle}
            >
              Run installer
            </button>
            <button className="btn btn-ghost" onClick={() => setStep('detect')} type="button">
              Back
            </button>
          </div>
        </div>
      )}

      {step === 'run' && (
        <div className="install-step">
          <h3>Installer running</h3>
          <p className="muted">
            The Wine window is the installer itself. Drive it like a normal Windows wizard. cellar
            captures stdout / stderr below.
          </p>
          <pre className="log" ref={logRef}>
            {log.join('\n') || '...'}
          </pre>
          {installExitCode !== null && (
            <div className={installExitCode === 0 ? 'install-ok' : 'install-fail'}>
              installer exited with code {installExitCode}
            </div>
          )}
        </div>
      )}

      {step === 'register' && (
        <div className="install-step">
          <h3>Add to library</h3>
          <p>
            Last step. Tell cellar where the installed game lives inside the bottle and which .exe
            to launch.
          </p>

          <div className="form">
            <label>
              Game name
              <input
                className="input"
                value={gameName}
                onChange={(e) => setGameName(e.target.value)}
                placeholder="e.g. Cyberpunk 2077"
              />
            </label>
            <label>
              Install directory
              <div className="input-with-button">
                <input
                  className="input"
                  value={installDir}
                  onChange={(e) => setInstallDir(e.target.value)}
                  placeholder="~/.cellar/bottles/<id>/prefix/drive_c/Games/..."
                />
                <button className="btn btn-ghost" onClick={pickInstallDir} type="button">
                  Browse
                </button>
              </div>
            </label>
            <label>
              Launch .exe
              <div className="input-with-button">
                <input
                  className="input"
                  value={launchExe}
                  onChange={(e) => setLaunchExe(e.target.value)}
                  placeholder="e.g. Cyberpunk2077.exe"
                />
                <button className="btn btn-ghost" onClick={pickLaunchExe} type="button">
                  Browse
                </button>
              </div>
            </label>
          </div>

          <div className="install-actions">
            <button
              className="btn btn-primary"
              onClick={registerGame}
              type="button"
              disabled={!gameName.trim() || !installDir.trim() || !launchExe.trim()}
            >
              Add to library
            </button>
            <button className="btn btn-ghost" onClick={reset} type="button">
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function kindLabel(k: InstallerKind): string {
  switch (k) {
    case 'FitGirl':
      return 'FitGirl repack';
    case 'Dodi':
      return 'DODI repack';
    case 'Kaos':
      return 'KaOs repack';
    case 'InnoSetup':
      return 'Inno Setup installer';
    case 'Msi':
      return 'MSI installer';
    default:
      return 'Unknown installer (will run anyway)';
  }
}

function formatErr(err: unknown): string {
  const e = isCellarError(err);
  if (!e) return String(err);
  if (e.kind === 'wine_missing') return 'Wine binary not found. Run scripts/setup-gptk.sh.';
  if (e.kind === 'not_found') return `File not found: ${e.path ?? '?'}`;
  if (e.kind === 'bottle_error') return `Bottle error: ${e.message ?? 'unknown'}`;
  if (e.kind === 'spawn_failed') return `Spawn failed: ${e.message ?? 'unknown'}`;
  if (e.kind === 'already_exists') return `Bottle id collision (${e.id ?? '?'}). Try again.`;
  return e.kind;
}
