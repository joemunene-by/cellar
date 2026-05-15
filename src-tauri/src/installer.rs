//! Installer detection and run-in-bottle orchestration.
//!
//! Phase 1 ships:
//!   - heuristic detection of FitGirl / DODI / KaOs / generic InnoSetup
//!     repacks based on folder contents and filename signatures.
//!   - a "run installer in this bottle" command that streams stdout +
//!     stderr lines back to the renderer as `cellar://install` events.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::runtime;
use crate::wine;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallerError {
    NotFound { path: String },
    WineMissing,
    BottleError { message: String },
    SpawnFailed { message: String },
}

#[derive(Clone, Serialize)]
pub enum InstallerKind {
    FitGirl,
    Dodi,
    Kaos,
    InnoSetup,
    Msi,
    Unknown,
}

#[derive(Serialize)]
pub struct DetectResult {
    pub kind: InstallerKind,
    pub setup_exe: Option<String>,
    /// Free-form strings explaining what was detected (e.g. "found
    /// fitgirl-bins"). Useful for the UI to show why we labelled it.
    pub hints: Vec<String>,
}

/// Look at `path` (a directory or a single `.exe`) and guess what kind
/// of installer lives there. Returns the canonical setup.exe path when
/// we can find it.
#[tauri::command]
pub fn installer_detect(path: String) -> Result<DetectResult, InstallerError> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(InstallerError::NotFound { path });
    }

    let root = if p.is_dir() {
        p.clone()
    } else {
        p.parent().unwrap_or(&p).to_path_buf()
    };
    let mut hints: Vec<String> = Vec::new();
    let mut kind = InstallerKind::Unknown;

    if let Ok(entries) = std::fs::read_dir(&root) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.contains("fitgirl") || name.starts_with("fg-") {
                hints.push(format!("found {}", name));
                kind = InstallerKind::FitGirl;
            } else if name == "dodi-bins" || name.starts_with("dodi") {
                hints.push(format!("found {}", name));
                kind = InstallerKind::Dodi;
            } else if name.starts_with("kaos") {
                hints.push(format!("found {}", name));
                kind = InstallerKind::Kaos;
            }
        }
    }

    let setup_exe = find_setup_exe(&root).or_else(|| {
        if p.extension().and_then(|s| s.to_str()) == Some("exe") {
            Some(p.clone())
        } else {
            None
        }
    });

    if matches!(kind, InstallerKind::Unknown) {
        if let Some(ref se) = setup_exe {
            let n = se.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            if n.ends_with(".msi") {
                kind = InstallerKind::Msi;
            } else if n.contains("setup") || n.contains("install") {
                kind = InstallerKind::InnoSetup;
            }
        }
    }

    Ok(DetectResult {
        kind,
        setup_exe: setup_exe.map(|s| s.to_string_lossy().to_string()),
        hints,
    })
}

fn find_setup_exe(root: &Path) -> Option<PathBuf> {
    for candidate in &["setup.exe", "Setup.exe", "SETUP.EXE", "install.exe", "Install.exe"] {
        let p = root.join(candidate);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[derive(Clone, Serialize)]
struct InstallerLine {
    line: String,
    stream: String,
}

#[derive(Clone, Serialize)]
struct InstallerDone {
    exit_code: i32,
}

/// Run an installer .exe inside a bottle. Streams stdout / stderr to
/// `cellar://install`, emits `cellar://install-done` with the exit
/// code when the installer process terminates. Resolves to the exit
/// code so callers can `await` the full install.
#[tauri::command]
pub async fn installer_run(
    bottle_id: String,
    installer_exe: String,
    app: AppHandle,
) -> Result<i32, InstallerError> {
    let wine_bin = runtime::find_wine_bin().ok_or(InstallerError::WineMissing)?;
    let prefix = wine::bottle_prefix_path(&bottle_id).map_err(|e| {
        InstallerError::BottleError {
            message: serde_json::to_string(&e).unwrap_or_else(|_| "bottle lookup failed".into()),
        }
    })?;
    if !prefix.exists() {
        return Err(InstallerError::BottleError {
            message: "bottle prefix missing".into(),
        });
    }

    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    let mut child = Command::new(&wine_bin)
        .arg(&installer_exe)
        .env("WINEPREFIX", &prefix)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| InstallerError::SpawnFailed { message: e.to_string() })?;

    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");

    let app_o = app.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_o.emit("cellar://install", InstallerLine { line, stream: "stdout".into() });
        }
    });
    let app_e = app.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_e.emit("cellar://install", InstallerLine { line, stream: "stderr".into() });
        }
    });

    let status = child
        .wait()
        .await
        .map_err(|e| InstallerError::SpawnFailed { message: e.to_string() })?;
    let code = status.code().unwrap_or(-1);
    let _ = app.emit("cellar://install-done", InstallerDone { exit_code: code });
    Ok(code)
}
