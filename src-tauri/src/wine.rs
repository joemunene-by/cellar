//! Wine bottle management.
//!
//! A "bottle" in cellar is a Wine prefix plus a small metadata file.
//! Each bottle lives under `~/.cellar/bottles/<id>/`:
//!
//!   ~/.cellar/bottles/<id>/
//!     bottle.json   metadata (name, windows version, creation time)
//!     prefix/       the actual WINEPREFIX (C: drive, registry, etc.)
//!
//! New bottles are initialised by running `wineboot --init` inside the
//! prefix so the C: layout, stub DLLs, and registry hives are in place
//! before the first installer runs.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::runtime;

const BOTTLES_DIR: &str = "bottles";

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WineError {
    HomeNotFound,
    AlreadyExists { id: String },
    NotFound { id: String },
    IoError { message: String },
    SpawnFailed { message: String },
}

impl From<std::io::Error> for WineError {
    fn from(e: std::io::Error) -> Self {
        WineError::IoError { message: e.to_string() }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Bottle {
    pub id: String,
    pub name: String,
    /// Windows version to report to the guest: "win10", "win11", "win7", etc.
    pub windows_version: String,
    pub created_ms: u128,
    /// Absolute path to the Wine prefix directory. Not stored in
    /// `bottle.json` (derived from id + the cellar layout) so older
    /// bottle files still deserialise. Always populated when returned
    /// by `wine_create_bottle` or `wine_list_bottles`.
    #[serde(default)]
    pub prefix_path: String,
}

fn bottles_root() -> Result<PathBuf, WineError> {
    let home = directories::BaseDirs::new()
        .ok_or(WineError::HomeNotFound)?
        .home_dir()
        .to_path_buf();
    Ok(home.join(".cellar").join(BOTTLES_DIR))
}

fn bottle_dir(id: &str) -> Result<PathBuf, WineError> {
    Ok(bottles_root()?.join(id))
}

fn bottle_metadata_path(id: &str) -> Result<PathBuf, WineError> {
    Ok(bottle_dir(id)?.join("bottle.json"))
}

pub fn bottle_prefix_path(id: &str) -> Result<PathBuf, WineError> {
    Ok(bottle_dir(id)?.join("prefix"))
}

/// Create a new bottle and initialise its Wine prefix.
#[tauri::command]
pub async fn wine_create_bottle(
    name: String,
    windows_version: String,
) -> Result<Bottle, WineError> {
    let id = uuid::Uuid::new_v4().to_string();
    let dir = bottle_dir(&id)?;
    if dir.exists() {
        return Err(WineError::AlreadyExists { id });
    }
    fs::create_dir_all(&dir)?;
    let prefix = dir.join("prefix");
    fs::create_dir_all(&prefix)?;

    let wine_bin = runtime::find_wine_bin().ok_or(WineError::SpawnFailed {
        message: "wine64 binary not found. Run ./scripts/setup-gptk.sh first.".into(),
    })?;
    let status = tokio::process::Command::new(&wine_bin)
        .arg("wineboot")
        .arg("--init")
        .env("WINEPREFIX", &prefix)
        .env("WINEARCH", "win64")
        .status()
        .await
        .map_err(|e| WineError::SpawnFailed { message: e.to_string() })?;
    if !status.success() {
        return Err(WineError::SpawnFailed {
            message: format!("wineboot --init exited with {:?}", status.code()),
        });
    }

    let mut bottle = Bottle {
        id: id.clone(),
        name,
        windows_version,
        created_ms: now_ms(),
        prefix_path: String::new(),
    };
    fs::write(
        bottle_metadata_path(&id)?,
        serde_json::to_string_pretty(&bottle)
            .map_err(|e| WineError::IoError { message: e.to_string() })?,
    )?;
    // Populate the derived path on the in-memory copy only; we do not
    // need it in the file (the id is what's authoritative).
    bottle.prefix_path = prefix.to_string_lossy().to_string();
    Ok(bottle)
}

/// List all bottles in `~/.cellar/bottles/`, newest first.
#[tauri::command]
pub fn wine_list_bottles() -> Result<Vec<Bottle>, WineError> {
    let root = bottles_root()?;
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        let meta = entry.path().join("bottle.json");
        if !meta.exists() {
            continue;
        }
        let text = fs::read_to_string(&meta)?;
        if let Ok(mut b) = serde_json::from_str::<Bottle>(&text) {
            // Derive the prefix path so the renderer can show it / use
            // it to pre-fill the "install dir" field of the wizard.
            if let Ok(p) = bottle_prefix_path(&b.id) {
                b.prefix_path = p.to_string_lossy().to_string();
            }
            out.push(b);
        }
    }
    out.sort_by(|a, b| b.created_ms.cmp(&a.created_ms));
    Ok(out)
}

/// Delete a bottle. Removes the prefix and metadata; cannot be undone.
#[tauri::command]
pub fn wine_remove_bottle(id: String) -> Result<(), WineError> {
    let dir = bottle_dir(&id)?;
    if !dir.exists() {
        return Err(WineError::NotFound { id });
    }
    fs::remove_dir_all(&dir)?;
    Ok(())
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
