//! Background watcher for `~/Games-source/`.
//!
//! When a new game directory is dropped in, cellar fingerprints it
//! against the bundled engine-family profiles (`profiles::find_for`) and
//! emits a `cellar://game-detected` event so the frontend can surface a
//! toast: which profile matched and the pre-baked install command.
//!
//! `notify` is synchronous, so the watcher lives on its own std thread
//! (the `RecommendedWatcher` must stay alive for the thread's lifetime).
//! New directories are announced once per session to avoid the duplicate
//! events filesystem watchers normally fire.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use notify::{EventKind, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::profiles;

#[derive(Serialize, Clone)]
pub struct GameDetected {
    /// Directory name dropped under ~/Games-source/.
    pub name: String,
    /// Absolute path to the new directory.
    pub path: String,
    /// Matched profile id/name, if any engine-family profile fingerprinted.
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    /// Pre-baked install command the user can copy/run.
    pub suggested_cmd: String,
}

fn games_source_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join("Games-source");
    dir.is_dir().then_some(dir)
}

fn announce(app: &AppHandle, root: &Path, dir: &Path, seen: &mut HashSet<PathBuf>) {
    // Only top-level entries directly under Games-source, and only dirs.
    if dir.parent() != Some(root) || !dir.is_dir() {
        return;
    }
    if !seen.insert(dir.to_path_buf()) {
        return; // already announced this session
    }
    let name = match dir.file_name().and_then(|n| n.to_str()) {
        Some(n) => n.to_string(),
        None => return,
    };
    // Skip dotfiles / in-progress copies macOS/Finder leaves around.
    if name.starts_with('.') {
        return;
    }
    let matched = profiles::find_for(&name);
    let (profile_id, profile_name) = match &matched {
        Some(p) => (Some(p.id.clone()), Some(p.name.clone())),
        None => (None, None),
    };
    let payload = GameDetected {
        name: name.clone(),
        path: dir.to_string_lossy().to_string(),
        profile_id,
        profile_name,
        suggested_cmd: format!("./scripts/cellar-install.sh \"{}\"", name),
    };
    let _ = app.emit("cellar://game-detected", payload);
}

/// Start watching `~/Games-source/` on a background thread. No-op if the
/// directory doesn't exist. Returns immediately; the thread runs for the
/// life of the app.
pub fn start(app: AppHandle) {
    let root = match games_source_dir() {
        Some(d) => d,
        None => return, // nothing to watch yet
    };

    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match notify::recommended_watcher(tx) {
            Ok(w) => w,
            Err(_) => return,
        };
        if watcher.watch(&root, RecursiveMode::NonRecursive).is_err() {
            return;
        }

        let mut seen: HashSet<PathBuf> = HashSet::new();
        for res in rx {
            let event = match res {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                continue;
            }
            for path in event.paths {
                announce(&app, &root, &path, &mut seen);
            }
        }
    });
}
