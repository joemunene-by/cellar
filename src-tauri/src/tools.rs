//! Diagnostic tools that wrap cellar's shell scripts: bottle inspection
//! and crash-report bundling.
//!
//! The scripts (`scripts/bottle-inspect.sh`, `scripts/crash-report.sh`)
//! are the single source of truth for this logic and are reused as-is
//! rather than reimplemented in Rust. They are bundled into the .app as
//! Tauri resources (see `tauri.conf.json` `bundle.resources`), so the
//! commands work in a distributed build, not just `tauri dev`.
//!
//! Scripts are run via `bash <path>` so a missing exec bit on a bundled
//! resource never matters.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolError {
    ScriptMissing { script: String },
    SpawnFailed { message: String },
    ScriptFailed { code: Option<i32>, stderr: String },
}

/// Resolve cellar's scripts directory.
///
/// Order: the bundled resource dir (release `.app`), then the
/// compile-time source tree (`tauri dev`, where resources aren't
/// staged). The first that actually contains the requested script
/// wins.
fn scripts_dir(app: &AppHandle) -> Option<PathBuf> {
    // Bundled: <Resources>/scripts/
    if let Ok(res) = app.path().resource_dir() {
        let p = res.join("scripts");
        if p.is_dir() {
            return Some(p);
        }
    }
    // Dev: <crate>/../scripts (CARGO_MANIFEST_DIR is src-tauri/).
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.join("scripts"));
    if let Some(p) = dev {
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

fn script_path(app: &AppHandle, name: &str) -> Result<PathBuf, ToolError> {
    let dir = scripts_dir(app).ok_or_else(|| ToolError::ScriptMissing {
        script: name.to_string(),
    })?;
    let p = dir.join(name);
    if !p.is_file() {
        return Err(ToolError::ScriptMissing {
            script: name.to_string(),
        });
    }
    Ok(p)
}

async fn run_script(
    app: &AppHandle,
    name: &str,
    args: &[&str],
) -> Result<String, ToolError> {
    let script = script_path(app, name)?;
    let output = tokio::process::Command::new("bash")
        .arg(&script)
        .args(args)
        .output()
        .await
        .map_err(|e| ToolError::SpawnFailed {
            message: e.to_string(),
        })?;
    if !output.status.success() {
        return Err(ToolError::ScriptFailed {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Inspect a bottle: prefix size, wine version, DLL overrides, installed
/// programs, winetricks verbs, save backups, last launcher log. Returns
/// the script's plain-text report for display in a panel.
#[tauri::command]
pub async fn bottle_inspect(app: AppHandle, bottle_id: String) -> Result<String, ToolError> {
    run_script(&app, "bottle-inspect.sh", &[&bottle_id]).await
}

#[derive(Serialize)]
pub struct CrashReport {
    /// Absolute path to the generated .zip, parsed from the script's
    /// "DONE. Crash report: <path>" line. None if the line wasn't found
    /// (the full log is still returned).
    pub zip_path: Option<String>,
    /// The script's full stdout, for showing in the UI.
    pub log: String,
}

/// Bundle a crash report for a bottle (launcher log + bottle-inspect +
/// analyze-log + doctor output) into a single timestamped .zip under
/// /tmp. Returns the zip path plus the script's output.
#[tauri::command]
pub async fn crash_report(app: AppHandle, bottle_id: String) -> Result<CrashReport, ToolError> {
    let log = run_script(&app, "crash-report.sh", &[&bottle_id]).await?;
    // crash-report.sh prints: "DONE. Crash report: <path> (<size>)"
    let zip_path = log.lines().find_map(|line| {
        let marker = "Crash report: ";
        let idx = line.find(marker)?;
        let rest = &line[idx + marker.len()..];
        // Trim the trailing " (<size>)" suffix if present.
        let path = rest.rsplit_once(" (").map(|(p, _)| p).unwrap_or(rest);
        let path = path.trim();
        if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        }
    });
    Ok(CrashReport { zip_path, log })
}
