//! Runtime detection (Wine, GPTK, Rosetta) and game launch.

use std::path::PathBuf;
use std::process::Stdio;

use serde::Serialize;
use tauri::State;

use crate::library::Library;
use crate::wine;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeError {
    GameNotFound { id: String },
    WineMissing,
    BottleError { message: String },
    SpawnFailed { message: String },
}

#[derive(Serialize)]
pub struct RuntimeStatus {
    pub wine_path: Option<String>,
    pub wine_version: Option<String>,
    pub gptk_present: bool,
    pub rosetta_installed: bool,
}

/// Locate a wine64 binary. Lookup order, best-to-worst:
///   1. `CELLAR_WINE` env var (manual override)
///   2. MythicApp/wine bundled with Mythic.app (CrossOver 24 base,
///      Apple Silicon native, D3DMetal + DXVK + DXMT, ACTIVE)
///   3. CrossOver.app (CodeWeavers, paid, wine 11 in CrossOver 26)
///   4. Homebrew apple/apple/game-porting-toolkit formula (Apple GPTK)
///   5. Apple GPTK .app bundle in /Applications
///   6. Whisky's bundled wine 7.7 (ARCHIVED 2025-05; known to hang
///      on FitGirl's unarc.dll under its experimental 32on64 wow64)
///   7. Plain wine64 on `PATH` (last resort)
///
/// Mythic is preferred because Whisky's pinned wine 7.7 has a
/// documented freearc-decompressor bug under its new-style wow64,
/// which makes every FitGirl repack fail with Inno error 1400
/// "Invalid window handle". CrossOver-24-base wines do not have
/// this bug; MythicApp ships the open-source build of that.
pub fn find_wine_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CELLAR_WINE") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let mythic_wine =
        "/Applications/Mythic.app/Contents/Resources/Engine/Wine/bin/wine64".to_string();
    let crossover_wine = "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine64"
        .to_string();
    let whisky_wine = format!(
        "{}/Library/Application Support/com.isaacmarovitz.Whisky/Libraries/Wine/bin/wine64",
        home
    );
    let candidates: [&str; 8] = [
        mythic_wine.as_str(),
        crossover_wine.as_str(),
        "/usr/local/opt/game-porting-toolkit/bin/wine64",
        "/opt/homebrew/opt/game-porting-toolkit/bin/wine64",
        "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64",
        whisky_wine.as_str(),
        "/opt/homebrew/bin/wine64",
        "/usr/local/bin/wine64",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn gptk_installed() -> bool {
    // Any of the modern wine sources counts: Mythic, CrossOver, GPTK
    // formula, GPTK app, or Whisky. cellar uses whichever finds the
    // wine binary at runtime.
    if PathBuf::from("/Applications/Mythic.app").exists()
        || PathBuf::from("/Applications/CrossOver.app").exists()
        || PathBuf::from("/opt/homebrew/opt/game-porting-toolkit").exists()
        || PathBuf::from("/usr/local/opt/game-porting-toolkit").exists()
        || PathBuf::from("/Applications/Game Porting Toolkit.app").exists()
    {
        return true;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(format!(
        "{}/Library/Application Support/com.isaacmarovitz.Whisky/Libraries/Wine/bin/wine64",
        home
    ))
    .exists()
}

fn rosetta_installed() -> bool {
    PathBuf::from("/Library/Apple/usr/share/rosetta/rosetta").exists()
}

/// Report what's installed (wine path/version, GPTK, Rosetta).
#[tauri::command]
pub fn runtime_status() -> RuntimeStatus {
    let wine_path = find_wine_bin();
    let wine_version = wine_path.as_ref().and_then(|p| {
        std::process::Command::new(p)
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
    });
    RuntimeStatus {
        wine_path: wine_path.map(|p| p.to_string_lossy().to_string()),
        wine_version,
        gptk_present: gptk_installed(),
        rosetta_installed: rosetta_installed(),
    }
}

/// Run `wine64 --version` to confirm the binary actually starts.
/// Returns the trimmed stdout on success. Useful as a "smoke test"
/// button in Settings; failures here usually mean a quarantine flag,
/// a Rosetta issue, or a missing GPTK dep, not a path problem.
#[tauri::command]
pub async fn runtime_test_wine() -> Result<String, RuntimeError> {
    let wine_bin = find_wine_bin().ok_or(RuntimeError::WineMissing)?;
    let output = tokio::process::Command::new(&wine_bin)
        .arg("--version")
        .output()
        .await
        .map_err(|e| RuntimeError::SpawnFailed { message: e.to_string() })?;
    if !output.status.success() {
        return Err(RuntimeError::SpawnFailed {
            message: format!(
                "wine64 --version exited {:?}. stderr: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Launch a game from the library by id. Spawns the wine process and
/// returns immediately; the game runs detached. A background tokio
/// task records the play duration into the library when the child
/// exits, so the Library card's total_play_ms is accurate.
#[tauri::command]
pub async fn runtime_launch(
    game_id: String,
    library: State<'_, Library>,
) -> Result<(), RuntimeError> {
    let game = library
        .list()
        .into_iter()
        .find(|g| g.id == game_id)
        .ok_or_else(|| RuntimeError::GameNotFound { id: game_id.clone() })?;

    let wine_bin = find_wine_bin().ok_or(RuntimeError::WineMissing)?;
    let prefix = wine::bottle_prefix_path(&game.bottle_id).map_err(|e| {
        RuntimeError::BottleError {
            message: serde_json::to_string(&e).unwrap_or_else(|_| "bottle path lookup failed".into()),
        }
    })?;
    if !prefix.exists() {
        return Err(RuntimeError::BottleError {
            message: format!("bottle prefix missing for game {}", game.name),
        });
    }

    let mut cmd = tokio::process::Command::new(&wine_bin);
    cmd.arg(&game.launch_exe);
    for arg in &game.settings.launch_args {
        cmd.arg(arg);
    }
    cmd.env("WINEPREFIX", &prefix);
    if game.settings.dxvk {
        // Tell wine to prefer the native DXVK DLLs over its own builtins
        // for the three D3D11 / DXGI redirectors. Without this, even
        // a DLL-injected bottle still loads the WineD3D builtins.
        cmd.env("WINEDLLOVERRIDES", "d3d11,d3d10core,dxgi=n");
    }
    cmd.env("WINEESYNC", if game.settings.esync { "1" } else { "0" });
    cmd.env("WINEMSYNC", if game.settings.msync { "1" } else { "0" });
    for (k, v) in &game.settings.env {
        cmd.env(k, v);
    }
    cmd.current_dir(&game.install_dir);
    cmd.stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());

    // Mark last-played BEFORE spawn so the Library shows the launch
    // attempt even if the user kills it 1 second in.
    let _ = library.mark_played_now(&game_id);

    let mut child = cmd
        .spawn()
        .map_err(|e| RuntimeError::SpawnFailed { message: e.to_string() })?;

    // Snapshot the Library handle for the background waiter task.
    // State<'_, Library> can not cross tasks but Library is just an
    // Arc-equivalent under the hood (Tauri manages it via Arc), so
    // we re-resolve the path-based singleton in the waiter via the
    // standalone Library::load() that reads the same on-disk file.
    let game_id_waiter = game_id.clone();
    let started_at = std::time::Instant::now();
    tokio::spawn(async move {
        let _ = child.wait().await;
        let played_ms = started_at.elapsed().as_millis();
        // Re-open the library so we do not need the Tauri State here.
        let lib = Library::load();
        let _ = lib.add_play_time(&game_id_waiter, played_ms);
    });

    Ok(())
}
