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
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

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
    // Some Wine builds (notably Whisky's 7.7 with the 32on64 patch set)
    // depend on tools that live next to the wine64 binary, plus PATH
    // for fallbacks. macOS GUI processes can launch with a minimal
    // PATH, so we explicitly prepend the wine binary's dir and add
    // /opt/homebrew/bin for completeness.
    let wine_dir = wine_bin
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let cur_path = std::env::var("PATH").unwrap_or_default();
    let augmented_path = if wine_dir.is_empty() {
        format!("/opt/homebrew/bin:/usr/local/bin:{}", cur_path)
    } else {
        format!("{}:/opt/homebrew/bin:/usr/local/bin:{}", wine_dir, cur_path)
    };

    eprintln!(
        "[cellar] wineboot --init: wine={} prefix={}",
        wine_bin.display(),
        prefix.display()
    );

    let output = tokio::process::Command::new(&wine_bin)
        .arg("wineboot")
        .arg("--init")
        .env("WINEPREFIX", &prefix)
        .env("WINEARCH", "win64")
        .env("PATH", &augmented_path)
        .output()
        .await
        .map_err(|e| WineError::SpawnFailed {
            message: format!("could not spawn wine64 ({}): {}", wine_bin.display(), e),
        })?;
    let stderr_tail = String::from_utf8_lossy(&output.stderr)
        .lines()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ");
    eprintln!(
        "[cellar] wineboot --init exit={:?} stderr_tail={}",
        output.status.code(),
        stderr_tail
    );
    if !output.status.success() {
        return Err(WineError::SpawnFailed {
            message: format!(
                "wineboot --init exited with {:?}. stderr tail: {}",
                output.status.code(),
                stderr_tail
            ),
        });
    }

    // Best-effort DXVK injection. Failure here does not abort bottle
    // creation: many games (and most installers) run fine on WineD3D
    // until the user explicitly wants D3D11 throughput.
    let _ = inject_dxvk_into(&prefix);

    // Pre-create C:\Games so installers default to a writable path
    // rather than Z:\Games. Wine's default Z: drive maps to the host
    // filesystem root, which on macOS is read-only for unprivileged
    // processes; many installers (notably FitGirl repacks) default to
    // Z:\Games and silently fail with "Error 5: Access denied".
    let _ = fs::create_dir_all(prefix.join("drive_c").join("Games"));

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

#[derive(Serialize)]
pub struct ExeCandidate {
    pub path: String,
    pub name: String,
    pub parent_dir: String,
    pub size: u64,
    pub modified_ms: u128,
}

/// Walk a bottle's `drive_c` looking for `.exe` files that are
/// plausibly game launchers. Skips the Windows system trees and a few
/// common installer-leftover directories, sorts by size descending
/// (game executables tend to be the biggest in their folder), and
/// returns up to `max_count` candidates.
///
/// Used by the install wizard's register step to give the user a
/// clickable list of "did you mean this .exe?" instead of forcing
/// them to browse the bottle filesystem manually.
#[tauri::command]
pub fn wine_scan_bottle_exes(
    id: String,
    max_count: Option<usize>,
) -> Result<Vec<ExeCandidate>, WineError> {
    let prefix = bottle_prefix_path(&id)?;
    let drive_c = prefix.join("drive_c");
    if !drive_c.exists() {
        return Err(WineError::NotFound { id });
    }

    let mut found: Vec<ExeCandidate> = Vec::new();
    for entry in walkdir::WalkDir::new(&drive_c)
        .max_depth(8)
        .into_iter()
        .filter_entry(|e| !is_uninteresting_dir(e.file_name().to_string_lossy().as_ref()))
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.to_lowercase().ends_with(".exe") {
            continue;
        }
        // Skip the obvious non-game .exes shipped with wine itself or
        // dropped by installers as helpers.
        let lower = name.to_lowercase();
        if matches!(
            lower.as_str(),
            "uninstall.exe"
                | "unins000.exe"
                | "unins001.exe"
                | "unins002.exe"
                | "setup.exe"
                | "regsvr32.exe"
                | "rundll32.exe"
                | "winemenubuilder.exe"
                | "iexplore.exe"
                | "explorer.exe"
                | "vcredist_x64.exe"
                | "vcredist_x86.exe"
        ) {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size = meta.len();
        // Skip tiny stubs (under 64KB are almost never the game).
        if size < 64 * 1024 {
            continue;
        }
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let parent_dir = entry
            .path()
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        found.push(ExeCandidate {
            path: entry.path().to_string_lossy().to_string(),
            name,
            parent_dir,
            size,
            modified_ms,
        });
    }

    // Bigger first; game launcher tends to be the heaviest .exe in its
    // folder (often >50 MB once textures and DLLs are linked in).
    found.sort_by(|a, b| b.size.cmp(&a.size));
    found.truncate(max_count.unwrap_or(20));
    Ok(found)
}

fn is_uninteresting_dir(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "windows"
            | "programdata"
            | "$recycle.bin"
            | "users"
            | "documents and settings"
            | "system volume information"
    )
}

/// Locate the winetricks script. Order:
///   1. `CELLAR_WINETRICKS` env var (manual override)
///   2. `~/.cellar/bin/winetricks` (cellar's own fresh copy)
///   3. Whisky's bundled copy
///   4. Homebrew install (`/opt/homebrew/bin/winetricks`)
///
/// We prefer cellar's own copy because Whisky pins to whatever
/// version it shipped with, and winetricks SHA tables drift out of
/// date within weeks of Microsoft pushing a new VC++ redist.
fn find_winetricks() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CELLAR_WINETRICKS") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let cellar_owned = PathBuf::from(format!("{}/.cellar/bin/winetricks", home));
    if cellar_owned.exists() {
        return Some(cellar_owned);
    }
    let whisky = PathBuf::from(format!(
        "{}/Library/Application Support/com.isaacmarovitz.Whisky/Libraries/winetricks",
        home
    ));
    if whisky.exists() {
        return Some(whisky);
    }
    let brewed = PathBuf::from("/opt/homebrew/bin/winetricks");
    if brewed.exists() {
        return Some(brewed);
    }
    None
}

/// Verbs whose payload is served by Microsoft over HTTPS. For these
/// we set `WINETRICKS_FORCE=1` to skip the bundled SHA check, because
/// (a) Microsoft routinely updates the redists faster than winetricks
/// can ship a new known-good SHA, and (b) HTTPS to microsoft.com /
/// download.visualstudio.microsoft.com is the actual integrity check
/// we are relying on anyway. SHA-skip is NOT applied to third-party
/// verbs (corefonts pulls from SourceForge, etc.) where the SHA is
/// the only thing keeping us honest.
fn verb_skips_sha_check(verb: &str) -> bool {
    verb.starts_with("vcrun")
        || verb.starts_with("dotnet")
        || verb.starts_with("d3dx")
        || verb == "ucrtbase2019"
        || verb == "dotnetdesktop6"
        || verb == "dotnetdesktop7"
}

#[derive(Clone, Serialize)]
struct WinetricksLine {
    bottle_id: String,
    verb: String,
    line: String,
    stream: String,
}

#[derive(Clone, Serialize)]
struct WinetricksDone {
    bottle_id: String,
    verb: String,
    exit_code: i32,
}

/// Run winetricks against a bottle to install a runtime / font / config
/// "verb" (e.g. `vcrun2022`, `corefonts`, `dotnet48`). Streams stdout +
/// stderr as `cellar://winetricks` events tagged with the bottle id and
/// verb; emits a final `cellar://winetricks-done` with the exit code.
///
/// Runs with `--unattended` so prompt-driven verbs do not hang waiting
/// for a GUI click. Disables the winetricks self-version-check so an
/// offline machine does not stall on the http probe.
#[tauri::command]
pub async fn wine_run_winetricks(
    id: String,
    verb: String,
    app: AppHandle,
) -> Result<i32, WineError> {
    let prefix = bottle_prefix_path(&id)?;
    if !prefix.exists() {
        return Err(WineError::NotFound { id });
    }
    let wine_bin = runtime::find_wine_bin().ok_or(WineError::SpawnFailed {
        message: "wine64 not found (run setup-gptk.sh first)".into(),
    })?;
    let winetricks = find_winetricks().ok_or(WineError::SpawnFailed {
        message: "winetricks not found. Install Whisky or `brew install winetricks`.".into(),
    })?;

    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    let mut cmd = Command::new("bash");
    cmd.arg(&winetricks)
        .arg("--unattended")
        .arg(&verb)
        .env("WINEPREFIX", &prefix)
        .env("WINE", &wine_bin)
        .env("WINETRICKS_LATEST_VERSION_CHECK", "disabled")
        // Legacy large-address-aware flag so 32-bit allocations do
        // not collide at the 2 GB boundary inside the redist installer.
        .env("WINE_LARGE_ADDRESS_AWARE", "1");

    if verb_skips_sha_check(&verb) {
        cmd.env("WINETRICKS_FORCE", "1");
        eprintln!(
            "[cellar] winetricks {}: WINETRICKS_FORCE=1 (Microsoft download, trusting HTTPS)",
            verb
        );
    }
    eprintln!(
        "[cellar] winetricks: script={} verb={} prefix={}",
        winetricks.display(),
        verb,
        prefix.display()
    );

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| WineError::SpawnFailed { message: e.to_string() })?;

    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");

    let id_out = id.clone();
    let verb_out = verb.clone();
    let app_o = app.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_o.emit(
                "cellar://winetricks",
                WinetricksLine {
                    bottle_id: id_out.clone(),
                    verb: verb_out.clone(),
                    line,
                    stream: "stdout".into(),
                },
            );
        }
    });
    let id_err = id.clone();
    let verb_err = verb.clone();
    let app_e = app.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app_e.emit(
                "cellar://winetricks",
                WinetricksLine {
                    bottle_id: id_err.clone(),
                    verb: verb_err.clone(),
                    line,
                    stream: "stderr".into(),
                },
            );
        }
    });

    let status = child
        .wait()
        .await
        .map_err(|e| WineError::SpawnFailed { message: e.to_string() })?;
    let code = status.code().unwrap_or(-1);
    let _ = app.emit(
        "cellar://winetricks-done",
        WinetricksDone {
            bottle_id: id,
            verb,
            exit_code: code,
        },
    );
    Ok(code)
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
