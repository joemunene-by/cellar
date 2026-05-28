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
import { archive, wine, installer, library, isCellarError } from '../lib/invoke';
import type {
  ArchivePeek,
  Bottle,
  DetectResult,
  ExeCandidate,
  InstallerKind,
} from '../lib/invoke';

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
  const [creatingBottle, setCreatingBottle] = useState(false);

  // Step 4: running installer
  const [log, setLog] = useState<string[]>([]);
  const [installRunning, setInstallRunning] = useState(false);
  const [installExitCode, setInstallExitCode] = useState<number | null>(null);
  const logRef = useRef<HTMLPreElement | null>(null);

  // Step 5: register game
  const [gameName, setGameName] = useState('');
  const [installDir, setInstallDir] = useState('');
  const [launchExe, setLaunchExe] = useState('');
  const [exeCandidates, setExeCandidates] = useState<ExeCandidate[]>([]);
  const [scanningExes, setScanningExes] = useState(false);

  // Archive peek (native FreeArc reader). Optional, surfaced on the
  // detect step when we can plausibly point at a fg-*.bin / *.arc.
  const [peek, setPeek] = useState<ArchivePeek | null>(null);
  const [peeking, setPeeking] = useState(false);
  const [peekShowAll, setPeekShowAll] = useState(false);

  // Refresh bottles when reaching the bottle-pick step.
  useEffect(() => {
    if (step !== 'bottle') return;
    wine.listBottles().then(setBottles).catch((e) => setError(formatErr(e)));
  }, [step]);

  // When the register step opens with a fresh install_dir, default to
  // the selected bottle's drive_c so the user only needs to dig into
  // the right Program Files subfolder instead of browsing from /.
  useEffect(() => {
    if (step !== 'register' || installDir) return;
    const bottle = bottles.find((b) => b.id === selectedBottle);
    if (bottle?.prefix_path) {
      setInstallDir(`${bottle.prefix_path}/drive_c`);
    }
  }, [step, selectedBottle, bottles, installDir]);

  // When the register step opens, walk the bottle's drive_c for .exe
  // candidates the user can click instead of browsing manually. Skip
  // when the bottle is unset (defensive; should not happen).
  useEffect(() => {
    if (step !== 'register' || !selectedBottle) return;
    let cancelled = false;
    setScanningExes(true);
    wine
      .scanBottleExes(selectedBottle, 20)
      .then((res) => {
        if (!cancelled) setExeCandidates(res);
      })
      .catch((err) => {
        if (!cancelled) setError(formatErr(err));
      })
      .finally(() => {
        if (!cancelled) setScanningExes(false);
      });
    return () => {
      cancelled = true;
    };
  }, [step, selectedBottle]);

  const pickCandidate = (c: ExeCandidate) => {
    setLaunchExe(c.path);
    setInstallDir(c.parent_dir);
    // Default the game name to the .exe's folder name (usually the
    // game title) the first time, unless the user already typed one.
    if (!gameName.trim()) {
      const folder = c.parent_dir.split('/').filter(Boolean).pop() ?? '';
      setGameName(folder || c.name.replace(/\.exe$/i, ''));
    }
  };

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
    if (!newBottleName.trim() || creatingBottle) return;
    setCreatingBottle(true);
    setError(null);
    try {
      const b = await wine.createBottle(newBottleName.trim(), 'win10');
      setBottles([b, ...bottles]);
      setSelectedBottle(b.id);
      setNewBottleName('');
    } catch (err) {
      setError(formatErr(err));
    } finally {
      setCreatingBottle(false);
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
    setPeek(null);
    setPeekShowAll(false);
  };

  // Default the file dialog to the source dir so users land in the
  // folder that actually contains the fg-*.bin / *.arc files.
  const defaultPeekDir = (() => {
    if (!sourcePath) return undefined;
    // If sourcePath is a file (the picked .exe), strip the basename.
    const last = sourcePath.lastIndexOf('/');
    return last > 0 ? sourcePath.slice(0, last) : sourcePath;
  })();

  const peekArchive = async () => {
    setError(null);
    setPeek(null);
    setPeekShowAll(false);
    const picked = await open({
      directory: false,
      multiple: false,
      defaultPath: defaultPeekDir,
      filters: [
        { name: 'FreeArc archive', extensions: ['bin', 'arc'] },
        { name: 'All files', extensions: ['*'] },
      ],
    });
    if (typeof picked !== 'string') return;
    setPeeking(true);
    try {
      const result = await archive.peek(picked);
      setPeek(result);
    } catch (e) {
      setError(formatErr(e));
    } finally {
      setPeeking(false);
    }
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
            <button className="btn" onClick={peekArchive} type="button" disabled={peeking}>
              {peeking ? 'Reading archive...' : 'Preview archive contents'}
            </button>
            <button className="btn btn-ghost" onClick={reset} type="button">
              Start over
            </button>
          </div>

          {peek && <ArchivePeekPane peek={peek} showAll={peekShowAll} onToggle={() => setPeekShowAll((v) => !v)} />}
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
              disabled={creatingBottle}
            />
            <button
              className="btn"
              onClick={createBottle}
              type="button"
              disabled={!newBottleName.trim() || creatingBottle}
            >
              {creatingBottle ? 'Creating...' : 'Create'}
            </button>
          </div>
          {creatingBottle && (
            <div className="muted bottle-progress">
              Initialising Wine prefix and injecting DXVK. First-time creation usually takes
              30 to 60 seconds. Please do not click Create again; the UI will update on its own.
            </div>
          )}

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

          {(scanningExes || exeCandidates.length > 0) && (
            <div className="exe-candidates">
              <div className="meta-label">
                {scanningExes ? 'Scanning bottle for .exe files...' : 'Detected .exe files (click to use)'}
              </div>
              {!scanningExes && (
                <ul className="exe-list">
                  {exeCandidates.map((c) => (
                    <li key={c.path}>
                      <button className="exe-row" onClick={() => pickCandidate(c)} type="button">
                        <span className="exe-name">{c.name}</span>
                        <span className="exe-size">{prettyBytes(c.size)}</span>
                        <span className="exe-dir" title={c.parent_dir}>
                          {c.parent_dir.replace(/^.*\/drive_c/, 'C:')}
                        </span>
                      </button>
                    </li>
                  ))}
                  {exeCandidates.length === 0 && (
                    <li className="muted">
                      No .exe candidates found in the bottle yet. Browse manually below.
                    </li>
                  )}
                </ul>
              )}
            </div>
          )}

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

function prettyBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
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
  if (e.kind === 'home_not_found') return 'Could not resolve $HOME on this Mac. Sanity-check your user shell.';
  if (e.kind === 'not_found') return `File not found: ${e.path ?? e.id ?? '?'}`;
  if (e.kind === 'bottle_error') return `Bottle error: ${e.message ?? 'unknown'}`;
  if (e.kind === 'spawn_failed') return `Spawn failed: ${e.message ?? 'unknown'}`;
  if (e.kind === 'io_error') return `Filesystem error: ${e.message ?? 'unknown'}`;
  if (e.kind === 'already_exists') return `Bottle id collision (${e.id ?? '?'}). Try again.`;
  if (e.kind === 'not_free_arc') return `Not a FreeArc archive: ${e.detail ?? 'no signature'}`;
  if (e.kind === 'read_failed') return `Archive read failed: ${e.detail ?? 'unknown'}`;
  return `${e.kind}${e.message ? `: ${e.message}` : ''}`;
}

const PEEK_PREVIEW_COUNT = 30;

function ArchivePeekPane({
  peek,
  showAll,
  onToggle,
}: {
  peek: ArchivePeek;
  showAll: boolean;
  onToggle: () => void;
}) {
  const visible = showAll ? peek.files : peek.files.slice(0, PEEK_PREVIEW_COUNT);
  const remaining = peek.files.length - visible.length;
  const archiveName = peek.archive_path.split('/').pop() ?? peek.archive_path;
  return (
    <div className="archive-peek">
      <header className="archive-peek-header">
        <strong>{archiveName}</strong>
        <span className="muted">
          {peek.file_count} files, {prettyBytes(peek.total_uncompressed_bytes)} original
          ({prettyBytes(peek.archive_bytes)} on disk)
        </span>
      </header>

      <div className="archive-peek-codecs">
        <span className="meta-label">codecs</span>
        <ul>
          {peek.codecs.map((c, i) => (
            <li key={i} className={`codec-${c.support}`}>
              <code>{c.method}</code>
              <span className={`codec-badge codec-badge-${c.support}`}>
                {c.support === 'native' && 'native'}
                {c.support === 'hybrid' && 'needs wine + cls dll'}
                {c.support === 'unsupported' && 'unsupported'}
              </span>
            </li>
          ))}
        </ul>
      </div>

      {peek.partial_reason && (
        <div className="muted archive-peek-warning">{peek.partial_reason}</div>
      )}

      <details className="archive-peek-files" open>
        <summary>
          {visible.length} of {peek.file_count} files
        </summary>
        <table>
          <thead>
            <tr>
              <th>path</th>
              <th>size</th>
              <th>crc32</th>
            </tr>
          </thead>
          <tbody>
            {visible.map((f, i) => (
              <tr key={i} className={f.is_dir ? 'is-dir' : undefined}>
                <td>
                  <code>{f.is_dir ? `${f.path}/` : f.path}</code>
                </td>
                <td>{f.is_dir ? '' : prettyBytes(f.size)}</td>
                <td>
                  <code>{f.is_dir ? '' : (f.crc >>> 0).toString(16).padStart(8, '0')}</code>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {remaining > 0 && (
          <button className="btn btn-ghost" onClick={onToggle} type="button">
            Show {remaining} more
          </button>
        )}
        {showAll && peek.files.length > PEEK_PREVIEW_COUNT && (
          <button className="btn btn-ghost" onClick={onToggle} type="button">
            Collapse to {PEEK_PREVIEW_COUNT}
          </button>
        )}
      </details>
    </div>
  );
}
