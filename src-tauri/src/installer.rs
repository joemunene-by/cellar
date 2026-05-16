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

/// Find the installer .exe inside a repack folder.
///
/// FitGirl and similar repackers don't use a fixed filename. Real
/// examples we have seen in the wild:
///
///   setup.exe                    plain Inno Setup
///   setup-multi.exe              FitGirl multi-language
///   setup-multi2.exe             FitGirl multi-language v2
///   setup-multi3.exe             ... and so on through multi5+
///   setup-fitgirl-selective.exe  FitGirl selective installer
///   setup-language.exe           a sub-installer the main one chains to
///   install.exe                  the DODI / KaOs convention
///
/// We walk the top level (no recursion), collect every plausible
/// installer .exe, and return the highest-priority one. The priority
/// order matters: when both `setup-multi2.exe` and `setup-language.exe`
/// exist, the user wants the multi2 one because that's the wizard that
/// drives the install; `setup-language.exe` is a sub-process.
/// If `installer_exe` lives outside the bottle's drive_c tree, drop a
/// symlink at `drive_c/cellar-sources/<parent-folder-name>` pointing
/// at the installer's parent folder, and return the rewritten path
/// that goes through that symlink. Wine then sees the installer on
/// `C:\cellar-sources\<name>\<exe>` instead of `Z:\<host-path>\<exe>`.
///
/// Returns None when the installer is already inside drive_c or when
/// the symlink creation fails (in which case the caller should fall
/// back to the original path and let wine do its Z: translation).
fn stage_installer_into_bottle(prefix: &Path, installer_exe: &str) -> Option<String> {
    let src = PathBuf::from(installer_exe);
    let drive_c = prefix.join("drive_c");
    let drive_c_canon = drive_c.canonicalize().ok()?;
    let src_canon = src.canonicalize().ok()?;
    if src_canon.starts_with(&drive_c_canon) {
        return None; // already inside the bottle
    }
    let src_parent = src.parent()?;
    let folder_name = src_parent.file_name()?.to_string_lossy().to_string();
    let staging_root = drive_c.join("cellar-sources");
    std::fs::create_dir_all(&staging_root).ok()?;
    let link_path = staging_root.join(&folder_name);
    // Replace any existing link so re-runs always point at the latest source.
    let _ = std::fs::remove_file(&link_path);
    let _ = std::fs::remove_dir_all(&link_path);
    std::os::unix::fs::symlink(src_parent, &link_path).ok()?;
    let exe_name = src.file_name()?;
    Some(link_path.join(exe_name).to_string_lossy().to_string())
}

fn find_setup_exe(root: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    let mut hits: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_lowercase())
                .map(|n| n.ends_with(".exe") && (n.starts_with("setup") || n.starts_with("install")))
                .unwrap_or(false)
        })
        .collect();

    if hits.is_empty() {
        return None;
    }
    hits.sort_by_key(|p| installer_exe_priority(p));
    hits.into_iter().next()
}

/// Lower number wins. Drives the pick in `find_setup_exe`.
fn installer_exe_priority(path: &Path) -> u32 {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_lowercase())
        .unwrap_or_default();
    // Sub-installers that the main wizard chains to: lowest priority
    // so they never get picked when the main one exists.
    if name.contains("language") || name.contains("redist") || name.contains("vcredist") {
        return 100;
    }
    // Plain `setup.exe` is the canonical Inno Setup name.
    if name == "setup.exe" {
        return 0;
    }
    // FitGirl selective installer (lets you skip languages / extras).
    if name.starts_with("setup-fitgirl-selective") {
        return 1;
    }
    // FitGirl multi-language: setup-multi.exe, setup-multi2.exe, ...
    if name.starts_with("setup-multi") {
        return 2;
    }
    // Any other setup-*.exe variant.
    if name.starts_with("setup") {
        return 3;
    }
    // install.exe and friends (DODI / KaOs).
    if name == "install.exe" {
        return 4;
    }
    if name.starts_with("install") {
        return 5;
    }
    99
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
///
/// If the installer's parent folder is OUTSIDE the bottle (i.e. wine
/// would see it through the default `Z:` mapping for `/`), we stage
/// it into `drive_c/cellar-sources/<basename>` via a symlink so the
/// installer runs from `C:\cellar-sources\<basename>` instead. Inno
/// Setup treats Z: as a network drive and refuses to spawn the
/// 64-bit helper from there; the symlink dodge gives it a local
/// fixed-drive path with zero extra copy.
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

    let installer_exe = stage_installer_into_bottle(&prefix, &installer_exe)
        .unwrap_or(installer_exe);

    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    // Always pass /LOG to Inno Setup installers (most FitGirl / DODI /
    // generic repacks use Inno). The flag is ignored by non-Inno
    // installers so it is safe to always include. Path is C: relative
    // to the bottle, which is always writable.
    let inno_log_path = "C:\\cellar-install.log";
    eprintln!(
        "[cellar] installer_run: wine={} prefix={} exe={} log={}",
        wine_bin.display(),
        prefix.display(),
        installer_exe,
        inno_log_path
    );

    // Wrap the installer in wine's virtual desktop. Wine's bare
    // Cocoa driver leaves the installer attached to a "no driver"
    // root window; FitGirl's ISDone progress UI then fails to spawn
    // its child progress window (Inno error 1400, ERROR_INVALID_
    // WINDOW_HANDLE). Running through `explorer /desktop=<name>,WxH`
    // gives the installer a stable wine-managed desktop window to
    // parent its dialogs to. This is the same workaround Whisky's
    // own install path uses internally.
    let mut child = Command::new(&wine_bin)
        .arg("explorer")
        .arg("/desktop=cellar-installer,1280x720")
        .arg(&installer_exe)
        .arg(format!("/LOG={}", inno_log_path))
        .env("WINEPREFIX", &prefix)
        // Disable audio for installer runs. Whisky's wine 7.7
        // winecoreaudio driver null-derefs in
        // ca_channel_layout_to_channel_mask when an installer tries
        // to play sound (notably FitGirl's background music during
        // unpack). The crash takes the whole installer down with a
        // page fault and exit 5, even though no audio is actually
        // needed for the install to succeed. Game launches in
        // runtime::runtime_launch still get audio.
        .env("WINEAUDIODRIVER", "")
        // Quieter wine log so the install log we surface is signal,
        // not the firehose of fixmes that mostly do not matter.
        .env("WINEDEBUG", "-all,err+seh")
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
