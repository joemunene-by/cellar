//! Per-bottle D3DMetal version pinning.
//!
//! The default cellar runtime uses the D3DMetal.framework bundled with
//! the CrossOver app under `~/.cellar/runtime/`. Some games regress on a
//! newer D3DMetal (CarX Street ran clean on 3.0 but glitched on 2.0), so
//! this lets the user pin a specific version per bottle.
//!
//! Mechanism (matches `scripts/d3dmetal-switch.sh`): a chosen version is
//! written to `~/.cellar/bottles/<id>/d3dmetal-version`; `launch-engine.sh`
//! reads it and points `DYLD_FRAMEWORK_PATH` at the matching framework.
//! Pinning is done natively here rather than via the script because it is
//! just a one-line file read/write — no shell needed.

use std::path::PathBuf;

use serde::Serialize;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum D3DMetalError {
    NoHome,
    BottleMissing { id: String },
    Io { message: String },
}

const PIN_FILE: &str = "d3dmetal-version";
/// Sentinel meaning "no pin — use the runtime default".
const DEFAULT: &str = "default";

fn cellar_root() -> Result<PathBuf, D3DMetalError> {
    let home = std::env::var("HOME").map_err(|_| D3DMetalError::NoHome)?;
    Ok(PathBuf::from(home).join(".cellar"))
}

fn bottle_dir(id: &str) -> Result<PathBuf, D3DMetalError> {
    let dir = cellar_root()?.join("bottles").join(id);
    if !dir.is_dir() {
        return Err(D3DMetalError::BottleMissing { id: id.to_string() });
    }
    Ok(dir)
}

/// List the version labels the user can pin to: always "default", plus
/// any user-installed framework under `~/.cellar/d3dmetal/<version>/`.
#[tauri::command]
pub fn d3dmetal_list() -> Vec<String> {
    let mut out = vec![DEFAULT.to_string()];
    if let Ok(root) = cellar_root() {
        let local = root.join("d3dmetal");
        if let Ok(entries) = std::fs::read_dir(&local) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    if let Some(name) = e.file_name().to_str() {
                        out.push(name.to_string());
                    }
                }
            }
        }
    }
    out
}

/// Return the version pinned for a bottle, or None if it uses the
/// runtime default.
#[tauri::command]
pub fn d3dmetal_get(bottle_id: String) -> Result<Option<String>, D3DMetalError> {
    let pin = bottle_dir(&bottle_id)?.join(PIN_FILE);
    match std::fs::read_to_string(&pin) {
        Ok(s) => {
            let v = s.trim();
            if v.is_empty() || v == DEFAULT {
                Ok(None)
            } else {
                Ok(Some(v.to_string()))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(D3DMetalError::Io {
            message: e.to_string(),
        }),
    }
}

/// Pin a bottle to a D3DMetal version, or unpin it (version == "default"
/// removes the pin file so the runtime default is used).
#[tauri::command]
pub fn d3dmetal_set(bottle_id: String, version: String) -> Result<(), D3DMetalError> {
    let pin = bottle_dir(&bottle_id)?.join(PIN_FILE);
    if version.trim().is_empty() || version == DEFAULT {
        match std::fs::remove_file(&pin) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(D3DMetalError::Io {
                message: e.to_string(),
            }),
        }
    } else {
        std::fs::write(&pin, format!("{}\n", version.trim())).map_err(|e| D3DMetalError::Io {
            message: e.to_string(),
        })
    }
}
