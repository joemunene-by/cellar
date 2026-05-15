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

/// Locate the GPTK-patched wine64 binary. Lookup order:
///   1. `CELLAR_WINE` env var (manual override)
///   2. Apple Game Porting Toolkit install locations
///   3. Homebrew game-porting-toolkit formula
///   4. Plain wine64 on `PATH`
pub fn find_wine_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CELLAR_WINE") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let candidates = [
        "/usr/local/opt/game-porting-toolkit/bin/wine64",
        "/opt/homebrew/opt/game-porting-toolkit/bin/wine64",
        "/Applications/Game Porting Toolkit.app/Contents/Resources/wine/bin/wine64",
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
    PathBuf::from("/opt/homebrew/opt/game-porting-toolkit").exists()
        || PathBuf::from("/usr/local/opt/game-porting-toolkit").exists()
        || PathBuf::from("/Applications/Game Porting Toolkit.app").exists()
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

/// Launch a game from the library by id. Spawns the wine process and
/// returns immediately; the game runs detached.
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
    cmd.env("DXVK_ENABLE", if game.settings.dxvk { "1" } else { "0" });
    cmd.env("WINEESYNC", if game.settings.esync { "1" } else { "0" });
    cmd.env("WINEMSYNC", if game.settings.msync { "1" } else { "0" });
    for (k, v) in &game.settings.env {
        cmd.env(k, v);
    }
    cmd.current_dir(&game.install_dir);
    cmd.stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());

    cmd.spawn()
        .map_err(|e| RuntimeError::SpawnFailed { message: e.to_string() })?;
    Ok(())
}
