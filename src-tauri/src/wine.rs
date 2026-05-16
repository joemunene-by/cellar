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
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::runtime;

/// DXVK DLL names cellar copies into fresh bottles. DXVK 2.x dropped
/// the D3D9 / D3D10_1 redirectors so the list is tight (D3D10, D3D11,
/// DXGI). Older games that use D3D9 fall back to wine's builtin
/// WineD3D which is slower but functional.
const DXVK_DLLS: [&str; 3] = ["d3d10core.dll", "d3d11.dll", "dxgi.dll"];

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

/// Locate a DXVK DLL pack on disk. Order:
///   1. `CELLAR_DXVK_DIR` env var (manual override; must have x64/ + x32/)
///   2. Whisky's bundled DXVK 2.x set under its app support dir
///
/// Returns None when no DXVK source is found; callers should treat
/// that as "best-effort skip", not a hard error, so fresh bottles
/// still come up usable for WineD3D-compatible games.
fn find_dxvk_source() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CELLAR_DXVK_DIR") {
        let path = PathBuf::from(p);
        if path.join("x64").join("d3d11.dll").exists() {
            return Some(path);
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let whisky = PathBuf::from(format!(
        "{}/Library/Application Support/com.isaacmarovitz.Whisky/Libraries/DXVK",
        home
    ));
    if whisky.join("x64").join("d3d11.dll").exists() {
        return Some(whisky);
    }
    None
}

/// Copy the DXVK x64 / x32 DLL set into the bottle's `system32` and
/// `syswow64`. Idempotent: re-running just overwrites the same files.
pub fn inject_dxvk_into(prefix: &Path) -> Result<(), WineError> {
    let src = find_dxvk_source().ok_or_else(|| WineError::IoError {
        message: "DXVK source not found. Install Whisky or set CELLAR_DXVK_DIR.".into(),
    })?;

    let system32 = prefix.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix.join("drive_c").join("windows").join("syswow64");
    fs::create_dir_all(&system32)?;
    fs::create_dir_all(&syswow64)?;

    for dll in DXVK_DLLS {
        let x64_src = src.join("x64").join(dll);
        let x32_src = src.join("x32").join(dll);
        if x64_src.exists() {
            fs::copy(&x64_src, system32.join(dll))?;
        }
        if x32_src.exists() {
            fs::copy(&x32_src, syswow64.join(dll))?;
        }
    }
    Ok(())
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

    // Best-effort DXVK injection. Failure here does not abort bottle
    // creation: many games (and most installers) run fine on WineD3D
    // until the user explicitly wants D3D11 throughput.
    let _ = inject_dxvk_into(&prefix);

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

/// Manually re-inject the DXVK DLL pack into a bottle. Useful when
/// the user installs Whisky after creating a bottle, or when a game's
/// installer overwrites the DLLs with stub versions.
#[tauri::command]
pub fn wine_inject_dxvk(id: String) -> Result<(), WineError> {
    let prefix = bottle_prefix_path(&id)?;
    if !prefix.exists() {
        return Err(WineError::NotFound { id });
    }
    inject_dxvk_into(&prefix)
}

/// Report whether a bottle has the cellar DXVK DLL set in place. Used
/// by the Settings UI to show a per-bottle "DXVK installed" indicator.
#[tauri::command]
pub fn wine_bottle_dxvk_status(id: String) -> Result<bool, WineError> {
    let prefix = bottle_prefix_path(&id)?;
    let probe = prefix
        .join("drive_c")
        .join("windows")
        .join("system32")
        .join("d3d11.dll");
    Ok(probe.exists())
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
