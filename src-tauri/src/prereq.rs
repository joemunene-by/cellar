//! Profile prerequisite installer.
//!
//! Profiles declare a `requires` list (e.g. `proton_winrt_dlls`,
//! `winetricks_mf`, `homebrew_gstreamer`). This module materialises
//! them on demand: extract+stage the Proton WinRT DLLs into a bottle,
//! drive `winetricks` for a verb like `mf`, or surface a brew command
//! the user has to run themselves (cellar cannot prompt for sudo /
//! keychain).
//!
//! Progress streams as `cellar://prereq` line events tagged with the
//! bottle id + require id; a terminal `cellar://prereq-done` event
//! carries the success bool and a short detail string.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::runtime;
use crate::wine;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PrereqError {
    UnknownRequire { id: String },
    BottleMissing { id: String },
    WineMissing,
    DependencyMissing { what: String, hint: String },
    ManualActionRequired { what: String, hint: String },
    SpawnFailed { message: String },
    IoError { message: String },
    ProcessFailed { stage: String, exit_code: i32 },
}

impl From<std::io::Error> for PrereqError {
    fn from(e: std::io::Error) -> Self {
        PrereqError::IoError { message: e.to_string() }
    }
}

#[derive(Clone, Serialize)]
struct PrereqLine {
    bottle_id: String,
    require_id: String,
    line: String,
    stream: String,
}

#[derive(Clone, Serialize)]
struct PrereqDone {
    bottle_id: String,
    require_id: String,
    success: bool,
    detail: String,
}

fn emit_line(app: &AppHandle, bottle_id: &str, require_id: &str, stream: &str, line: impl Into<String>) {
    let _ = app.emit(
        "cellar://prereq",
        PrereqLine {
            bottle_id: bottle_id.to_string(),
            require_id: require_id.to_string(),
            line: line.into(),
            stream: stream.to_string(),
        },
    );
}

fn emit_done(app: &AppHandle, bottle_id: &str, require_id: &str, success: bool, detail: impl Into<String>) {
    let _ = app.emit(
        "cellar://prereq-done",
        PrereqDone {
            bottle_id: bottle_id.to_string(),
            require_id: require_id.to_string(),
            success,
            detail: detail.into(),
        },
    );
}

#[derive(Clone, Serialize)]
pub struct CheckResult {
    /// True if the prereq is detectably already installed in the bottle.
    pub satisfied: bool,
    /// Short reason: "coremessaging.dll staged", "homebrew gstreamer
    /// arm64 present", "no detection rule". Surfaced in the drawer.
    pub detail: Option<String>,
}

/// Inspect a bottle for whether a specific prereq is already installed.
///
/// Detection is best-effort and conservative: we report `satisfied:
/// true` only when we can confirm the artefact (e.g. a key DLL in
/// `system32`, a homebrew lib dir on disk). Unknown require ids
/// return `satisfied: false, detail: Some("no detection rule")`
/// rather than erroring, so the UI can degrade gracefully.
#[tauri::command]
pub fn prereq_check(bottle_id: String, require_id: String) -> Result<CheckResult, PrereqError> {
    let prefix = wine::bottle_prefix_path(&bottle_id)
        .map_err(|_| PrereqError::BottleMissing { id: bottle_id.clone() })?;
    Ok(check_one(&prefix, &require_id))
}

/// Batch version. The frontend calls this once on drawer open with
/// the matched profile's full `requires` list to seed initial state.
#[tauri::command]
pub fn prereq_check_all(
    bottle_id: String,
    require_ids: Vec<String>,
) -> Result<HashMap<String, CheckResult>, PrereqError> {
    let prefix = wine::bottle_prefix_path(&bottle_id)
        .map_err(|_| PrereqError::BottleMissing { id: bottle_id.clone() })?;
    Ok(require_ids
        .into_iter()
        .map(|rid| {
            let res = check_one(&prefix, &rid);
            (rid, res)
        })
        .collect())
}

fn check_one(prefix: &Path, require_id: &str) -> CheckResult {
    match require_id {
        "proton_winrt_dlls" => {
            // coremessaging.dll is the load-bearing one — it backs
            // every Windows.System.DispatcherQueue activation. If it
            // is in system32 the staging definitely ran.
            let target = prefix.join("drive_c/windows/system32/coremessaging.dll");
            let satisfied = target.exists();
            CheckResult {
                satisfied,
                detail: Some(if satisfied {
                    "coremessaging.dll staged".into()
                } else {
                    "coremessaging.dll missing in system32".into()
                }),
            }
        }
        "winetricks_mf" => {
            // winetricks mf drops native Microsoft mfplat.dll into
            // system32 (overriding wine's stub) plus mfreadwrite,
            // mfmediaengine, mfsrcsnk. Test mfplat.dll size to
            // distinguish the real Microsoft DLL (~2 MB) from wine's
            // builtin stub (~50-100 KB).
            let target = prefix.join("drive_c/windows/system32/mfplat.dll");
            let satisfied = std::fs::metadata(&target)
                .map(|m| m.len() > 500_000)
                .unwrap_or(false);
            CheckResult {
                satisfied,
                detail: Some(if satisfied {
                    "native mfplat.dll staged".into()
                } else if target.exists() {
                    "mfplat.dll present but looks like wine builtin (run winetricks mf)".into()
                } else {
                    "mfplat.dll absent".into()
                }),
            }
        }
        "winetricks_d3dcompiler_47" => {
            let target = prefix.join("drive_c/windows/system32/d3dcompiler_47.dll");
            CheckResult {
                satisfied: target.exists(),
                detail: Some(if target.exists() {
                    "d3dcompiler_47.dll staged".into()
                } else {
                    "d3dcompiler_47.dll absent".into()
                }),
            }
        }
        "winetricks_vcrun2003" => {
            // vcrun2003 stages msvcr71.dll + msvcp71.dll.
            let target = prefix.join("drive_c/windows/system32/msvcr71.dll");
            CheckResult {
                satisfied: target.exists(),
                detail: Some(if target.exists() {
                    "msvcr71.dll staged".into()
                } else {
                    "msvcr71.dll absent".into()
                }),
            }
        }
        "homebrew_gstreamer" => {
            // Homebrew GStreamer always installs to /opt/homebrew/lib/
            // on Apple Silicon. We check both the plugin dir and a
            // marker plugin from gst-libav, since base GStreamer alone
            // is not enough for the wine MF bridge we want.
            let plugins = Path::new("/opt/homebrew/lib/gstreamer-1.0");
            let libav_marker = plugins.join("libgstlibav.dylib");
            if libav_marker.exists() {
                CheckResult {
                    satisfied: true,
                    detail: Some("homebrew gstreamer + gst-libav present".into()),
                }
            } else if plugins.exists() {
                CheckResult {
                    satisfied: false,
                    detail: Some("gstreamer found but gst-libav missing (run brew install gst-libav)".into()),
                }
            } else {
                CheckResult {
                    satisfied: false,
                    detail: Some("/opt/homebrew/lib/gstreamer-1.0 missing".into()),
                }
            }
        }
        _ => CheckResult {
            satisfied: false,
            detail: Some("no detection rule".into()),
        },
    }
}

/// Install a profile prereq into a bottle. Dispatches on `require_id`.
#[tauri::command]
pub async fn prereq_install(
    bottle_id: String,
    require_id: String,
    app: AppHandle,
) -> Result<(), PrereqError> {
    match require_id.as_str() {
        "winetricks_mf" => install_winetricks_mf(bottle_id, app).await,
        "winetricks_d3dcompiler_47" => install_winetricks_verb(bottle_id, "d3dcompiler_47", app).await,
        "winetricks_vcrun2003" => install_winetricks_verb(bottle_id, "vcrun2003", app).await,
        "proton_winrt_dlls" => install_proton_winrt(bottle_id, app).await,
        "homebrew_gstreamer" => Err(PrereqError::ManualActionRequired {
            what: "homebrew_gstreamer".into(),
            hint: "Run `brew install gstreamer gst-libav` in Terminal. cellar cannot pass through the brew password / network prompt.".into(),
        }),
        other => Err(PrereqError::UnknownRequire { id: other.to_string() }),
    }
}

async fn install_winetricks_mf(bottle_id: String, app: AppHandle) -> Result<(), PrereqError> {
    install_winetricks_verb(bottle_id, "mf", app).await
}

async fn install_winetricks_verb(bottle_id: String, verb: &str, app: AppHandle) -> Result<(), PrereqError> {
    // Reuse the existing winetricks runner. It emits cellar://winetricks
    // events; the frontend can also subscribe to those for richer detail,
    // but we also emit our own cellar://prereq lines so a single listener
    // can drive the per-require UI.
    emit_line(&app, &bottle_id, &format!("winetricks_{}", verb), "info", format!("invoking winetricks {}", verb));
    match wine::wine_run_winetricks(bottle_id.clone(), verb.to_string(), app.clone()).await {
        Ok(exit_code) if exit_code == 0 => {
            emit_done(
                &app,
                &bottle_id,
                &format!("winetricks_{}", verb),
                true,
                format!("winetricks {} ok", verb),
            );
            Ok(())
        }
        Ok(exit_code) => {
            emit_done(
                &app,
                &bottle_id,
                &format!("winetricks_{}", verb),
                false,
                format!("winetricks {} exited {}", verb, exit_code),
            );
            Err(PrereqError::ProcessFailed {
                stage: format!("winetricks {}", verb),
                exit_code,
            })
        }
        Err(e) => {
            emit_done(
                &app,
                &bottle_id,
                &format!("winetricks_{}", verb),
                false,
                "winetricks spawn failed".to_string(),
            );
            // wine::WineError serialises to its own JSON; flatten to a
            // string for PrereqError.
            Err(PrereqError::SpawnFailed {
                message: serde_json::to_string(&e).unwrap_or_else(|_| "winetricks failed".into()),
            })
        }
    }
}

/// Known places we look for a GE-Proton tarball, in priority order.
fn proton_tarball_candidates() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    vec![
        PathBuf::from("/tmp/ge-proton.tar.gz"),
        PathBuf::from(format!("{}/.cellar/cache/ge-proton.tar.gz", home)),
        PathBuf::from(format!("{}/Downloads/ge-proton.tar.gz", home)),
    ]
}

fn find_proton_tarball() -> Option<PathBuf> {
    proton_tarball_candidates().into_iter().find(|p| p.exists())
}

const PROTON_WINRT_DLLS: &[&str] = &[
    "windows.system.dll",
    "windows.gaming.input.dll",
    "windows.media.dll",
    "windows.media.devices.dll",
    "windows.media.speech.dll",
    "windows.networking.dll",
    "windows.networking.connectivity.dll",
    "windows.networking.hostname.dll",
    "windows.perception.stub.dll",
    "windows.ui.dll",
    "windows.ui.composition.dll",
    "windows.ui.xaml.dll",
    "twinapi.appcore.dll",
    "coremessaging.dll",
    "wintypes.dll",
    "threadpoolwinrt.dll",
];

const WINRT_OVERRIDES: &[&str] = &[
    "windows.system",
    "windows.gaming.input",
    "windows.media",
    "windows.ui",
    "twinapi.appcore",
    "coremessaging",
    "wintypes",
    "threadpoolwinrt",
];

const WINRT_ACTIVATIONS: &[(&str, &str)] = &[
    ("Windows.System.DispatcherQueue", "windows.system.dll"),
    ("Windows.System.DispatcherQueueController", "windows.system.dll"),
    ("Windows.System.DispatcherQueueTimer", "windows.system.dll"),
];

async fn install_proton_winrt(bottle_id: String, app: AppHandle) -> Result<(), PrereqError> {
    let require_id = "proton_winrt_dlls";
    let prefix = wine::bottle_prefix_path(&bottle_id)
        .map_err(|_| PrereqError::BottleMissing { id: bottle_id.clone() })?;
    if !prefix.join("drive_c").exists() {
        return Err(PrereqError::BottleMissing { id: bottle_id });
    }
    let wine_bin = runtime::find_wine_bin().ok_or(PrereqError::WineMissing)?;
    let tarball = find_proton_tarball().ok_or(PrereqError::DependencyMissing {
        what: "GE-Proton tarball".into(),
        hint: format!(
            "Download the latest GE-Proton release tarball from \
             https://github.com/GloriousEggroll/proton-ge-custom/releases/latest \
             and save it as one of: {}",
            proton_tarball_candidates()
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(" | ")
        ),
    })?;

    emit_line(&app, &bottle_id, require_id, "info", format!("found tarball at {}", tarball.display()));

    // Extract to a working dir under /tmp. tar can take a long time on a
    // big tarball; stream progress via `-v` so the UI shows movement.
    let extract_dir = PathBuf::from("/tmp/cellar-ge-proton");
    if extract_dir.exists() {
        emit_line(&app, &bottle_id, require_id, "info", "cleaning previous extract dir");
        let _ = std::fs::remove_dir_all(&extract_dir);
    }
    std::fs::create_dir_all(&extract_dir)?;
    emit_line(&app, &bottle_id, require_id, "info", format!("extracting to {}", extract_dir.display()));

    // Extract just the wine PE DLL dirs to keep this fast. Two-pass
    // strategy mirrors install-proton-winrt.sh: try the targeted
    // --include extract first; if that produces an empty tree, fall
    // back to a full extract.
    let status = tokio::process::Command::new("tar")
        .args([
            "xzf",
            &tarball.display().to_string(),
            "-C",
            &extract_dir.display().to_string(),
            "--include=*/files/lib*/wine/x86_64-windows/*.dll",
            "--include=*/files/lib*/wine/i386-windows/*.dll",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| PrereqError::SpawnFailed { message: e.to_string() })?;
    let mut x64_dir = find_subdir(&extract_dir, "x86_64-windows");
    if !status.success() || x64_dir.is_none() {
        emit_line(&app, &bottle_id, require_id, "info", "targeted extract empty, falling back to full extract");
        let status = tokio::process::Command::new("tar")
            .args([
                "xzf",
                &tarball.display().to_string(),
                "-C",
                &extract_dir.display().to_string(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|e| PrereqError::SpawnFailed { message: e.to_string() })?;
        if !status.success() {
            emit_done(&app, &bottle_id, require_id, false, "tar exited non-zero");
            return Err(PrereqError::ProcessFailed {
                stage: "tar xzf".into(),
                exit_code: status.code().unwrap_or(-1),
            });
        }
        x64_dir = find_subdir(&extract_dir, "x86_64-windows");
    }
    let x32_dir = find_subdir(&extract_dir, "i386-windows");

    let x64 = x64_dir.ok_or_else(|| {
        let _ = app.emit("cellar://prereq-done", PrereqDone {
            bottle_id: bottle_id.clone(),
            require_id: require_id.to_string(),
            success: false,
            detail: "tarball missing x86_64-windows dir".into(),
        });
        PrereqError::DependencyMissing {
            what: "x86_64-windows DLLs".into(),
            hint: "Tarball did not contain an x86_64-windows directory. Is this a real GE-Proton release?".into(),
        }
    })?;
    emit_line(&app, &bottle_id, require_id, "info", format!("x64 source dir: {}", x64.display()));
    if let Some(d) = &x32_dir {
        emit_line(&app, &bottle_id, require_id, "info", format!("x32 source dir: {}", d.display()));
    }

    let sys32 = prefix.join("drive_c/windows/system32");
    let syswow64 = prefix.join("drive_c/windows/syswow64");
    std::fs::create_dir_all(&sys32)?;
    std::fs::create_dir_all(&syswow64)?;

    let mut copied = 0;
    for dll in PROTON_WINRT_DLLS {
        let src = x64.join(dll);
        if src.exists() {
            let dst = sys32.join(dll);
            std::fs::copy(&src, &dst)?;
            copied += 1;
            emit_line(&app, &bottle_id, require_id, "stdout", format!("staged x64 {}", dll));
        }
        if let Some(x32) = &x32_dir {
            let src32 = x32.join(dll);
            if src32.exists() {
                let dst32 = syswow64.join(dll);
                std::fs::copy(&src32, &dst32)?;
                emit_line(&app, &bottle_id, require_id, "stdout", format!("staged x32 {}", dll));
            }
        }
    }
    emit_line(&app, &bottle_id, require_id, "info", format!("staged {} x64 WinRT DLLs", copied));
    if copied == 0 {
        emit_done(&app, &bottle_id, require_id, false, "no WinRT DLLs found in tarball");
        return Err(PrereqError::DependencyMissing {
            what: "Proton WinRT DLLs".into(),
            hint: "The tarball did not contain any of the expected windows.*.dll, coremessaging.dll, etc.".into(),
        });
    }

    // Set DllOverrides so wine picks the staged native PE over its
    // builtin. wine reg add does not need wineserver up; it modifies
    // the user.reg / system.reg files directly.
    emit_line(&app, &bottle_id, require_id, "info", "setting DLL overrides");
    for dll in WINRT_OVERRIDES {
        run_wine_reg(
            &wine_bin,
            &prefix,
            "HKCU\\Software\\Wine\\DllOverrides",
            dll,
            "native,builtin",
            "REG_SZ",
            &app,
            &bottle_id,
            require_id,
        )
        .await?;
    }

    // Register WinRT activation classes so RoGetActivationFactory can
    // resolve Windows.System.DispatcherQueue etc.
    emit_line(&app, &bottle_id, require_id, "info", "registering WinRT activation classes");
    for (class_name, dll) in WINRT_ACTIVATIONS {
        let activation_key = format!(
            "HKLM\\Software\\Microsoft\\WindowsRuntime\\ActivatableClassId\\{}",
            class_name
        );
        let dll_path = format!("C:\\windows\\system32\\{}", dll);
        run_wine_reg(
            &wine_bin,
            &prefix,
            &activation_key,
            "DllPath",
            &dll_path,
            "REG_EXPAND_SZ",
            &app,
            &bottle_id,
            require_id,
        )
        .await?;
        run_wine_reg(
            &wine_bin,
            &prefix,
            &activation_key,
            "ActivationType",
            "0",
            "REG_DWORD",
            &app,
            &bottle_id,
            require_id,
        )
        .await?;
        run_wine_reg(
            &wine_bin,
            &prefix,
            &activation_key,
            "TrustLevel",
            "0",
            "REG_DWORD",
            &app,
            &bottle_id,
            require_id,
        )
        .await?;
        run_wine_reg(
            &wine_bin,
            &prefix,
            &activation_key,
            "Threading",
            "0",
            "REG_DWORD",
            &app,
            &bottle_id,
            require_id,
        )
        .await?;
        emit_line(&app, &bottle_id, require_id, "stdout", format!("registered {} -> {}", class_name, dll));
    }

    let detail = format!(
        "staged {} WinRT DLLs into bottle {}; DispatcherQueue ready",
        copied,
        &bottle_id[..bottle_id.len().min(8)]
    );
    emit_done(&app, &bottle_id, require_id, true, &detail);
    Ok(())
}

async fn run_wine_reg(
    wine_bin: &Path,
    prefix: &Path,
    key: &str,
    value_name: &str,
    data: &str,
    reg_type: &str,
    app: &AppHandle,
    bottle_id: &str,
    require_id: &str,
) -> Result<(), PrereqError> {
    let status = tokio::process::Command::new(wine_bin)
        .args(["reg", "add", key, "/v", value_name, "/t", reg_type, "/d", data, "/f"])
        .env("WINEPREFIX", prefix)
        .env("WINEDEBUG", "-all")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .status()
        .await
        .map_err(|e| PrereqError::SpawnFailed { message: e.to_string() })?;
    if !status.success() {
        let code = status.code().unwrap_or(-1);
        emit_line(app, bottle_id, require_id, "stderr", format!("reg add failed for {}/{} (exit {})", key, value_name, code));
        return Err(PrereqError::ProcessFailed {
            stage: format!("reg add {}/{}", key, value_name),
            exit_code: code,
        });
    }
    Ok(())
}

fn find_subdir(root: &Path, name: &str) -> Option<PathBuf> {
    fn walk(dir: &Path, name: &str, depth: u32) -> Option<PathBuf> {
        if depth > 6 {
            return None;
        }
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            if p.file_name().and_then(|f| f.to_str()) == Some(name) {
                return Some(p);
            }
            if let Some(found) = walk(&p, name, depth + 1) {
                return Some(found);
            }
        }
        None
    }
    walk(root, name, 0)
}
