//! Persistent game library at `~/.cellar/library.json`.
//!
//! A game ties a `bottle_id` (Wine prefix) to an installed-path plus
//! the .exe to launch. Per-game settings (DXVK, ESYNC, MSYNC, custom
//! env, extra launch args) live alongside.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LibraryError {
    HomeNotFound,
    NotFound { id: String },
    IoError { message: String },
}

impl From<std::io::Error> for LibraryError {
    fn from(e: std::io::Error) -> Self {
        LibraryError::IoError { message: e.to_string() }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GameSettings {
    /// DXVK on for D3D9 / D3D11 translation. Off falls back to WineD3D
    /// OpenGL, which is slower but more compatible with very old games.
    pub dxvk: bool,
    /// ESYNC speeds up multi-threaded games via eventfd-style signalling.
    pub esync: bool,
    /// MSYNC is the Apple-Silicon-specific fast synchronisation primitive.
    pub msync: bool,
    /// MoltenVK fences for low-overhead GPU sync on the DXVK/MoltenVK
    /// path. No effect when DXVK is off (D3DMetal direct path).
    #[serde(default)]
    pub metal_fences: bool,
    /// Apple's Metal HUD overlay (FPS, GPU usage, frame time). Useful
    /// for diagnosing perf without touching the game's own HUD code.
    #[serde(default)]
    pub metal_hud: bool,
    /// Extra wine DLL overrides appended to DXVK's d3d11/d3d10core/dxgi
    /// when DXVK is on. Pass-through to WINEDLLOVERRIDES; semicolon-
    /// separated. For full control, set `env.WINEDLLOVERRIDES` instead;
    /// that wins over both DXVK defaults and this field.
    #[serde(default)]
    pub dll_overrides: Option<String>,
    /// Extra env vars to set when launching, e.g. {"DXVK_HUD": "fps"}.
    pub env: HashMap<String, String>,
    /// Extra args passed after the .exe path.
    pub launch_args: Vec<String>,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            dxvk: true,
            esync: true,
            msync: true,
            metal_fences: false,
            metal_hud: false,
            dll_overrides: None,
            env: HashMap::new(),
            launch_args: Vec::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub bottle_id: String,
    pub install_dir: String,
    pub launch_exe: String,
    pub last_played_ms: Option<u128>,
    pub total_play_ms: u128,
    pub settings: GameSettings,
}

#[derive(Default, Serialize, Deserialize)]
pub struct LibraryFile {
    pub games: Vec<Game>,
}

#[derive(Default)]
pub struct Library {
    file: Mutex<LibraryFile>,
}

impl Library {
    pub fn storage_path() -> Result<PathBuf, LibraryError> {
        let home = directories::BaseDirs::new()
            .ok_or(LibraryError::HomeNotFound)?
            .home_dir()
            .to_path_buf();
        Ok(home.join(".cellar").join("library.json"))
    }

    pub fn load() -> Self {
        let file = Self::storage_path()
            .ok()
            .and_then(|p| fs::read_to_string(&p).ok())
            .and_then(|s| serde_json::from_str::<LibraryFile>(&s).ok())
            .unwrap_or_default();
        Self { file: Mutex::new(file) }
    }

    pub fn save(&self) -> Result<(), LibraryError> {
        let p = Self::storage_path()?;
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&*self.file.lock().unwrap())
            .map_err(|e| LibraryError::IoError { message: e.to_string() })?;
        fs::write(&p, json)?;
        Ok(())
    }

    pub fn list(&self) -> Vec<Game> {
        self.file.lock().unwrap().games.clone()
    }

    pub fn add(&self, game: Game) -> Result<(), LibraryError> {
        self.file.lock().unwrap().games.push(game);
        self.save()
    }

    pub fn remove(&self, id: &str) -> Result<(), LibraryError> {
        let mut f = self.file.lock().unwrap();
        let before = f.games.len();
        f.games.retain(|g| g.id != id);
        if f.games.len() == before {
            return Err(LibraryError::NotFound { id: id.to_string() });
        }
        drop(f);
        self.save()
    }

    pub fn update_settings(&self, id: &str, settings: GameSettings) -> Result<(), LibraryError> {
        let mut f = self.file.lock().unwrap();
        let game = f
            .games
            .iter_mut()
            .find(|g| g.id == id)
            .ok_or_else(|| LibraryError::NotFound { id: id.to_string() })?;
        game.settings = settings;
        drop(f);
        self.save()
    }

    /// Stamp last_played_ms to "right now". Called when runtime_launch
    /// fires the wine subprocess so the Library card shows accurate
    /// recency even if the user kills the game before it exits cleanly.
    pub fn mark_played_now(&self, id: &str) -> Result<(), LibraryError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let mut f = self.file.lock().unwrap();
        let game = f
            .games
            .iter_mut()
            .find(|g| g.id == id)
            .ok_or_else(|| LibraryError::NotFound { id: id.to_string() })?;
        game.last_played_ms = Some(now);
        drop(f);
        self.save()
    }

    /// Add to total_play_ms when a game process exits. The Rust side
    /// of runtime_launch spawns a tokio task that waits on the wine
    /// child and calls this; survives the user closing the cellar UI
    /// because tokio tasks live as long as the cellar process does.
    pub fn add_play_time(&self, id: &str, ms: u128) -> Result<(), LibraryError> {
        let mut f = self.file.lock().unwrap();
        let game = f
            .games
            .iter_mut()
            .find(|g| g.id == id)
            .ok_or_else(|| LibraryError::NotFound { id: id.to_string() })?;
        game.total_play_ms = game.total_play_ms.saturating_add(ms);
        drop(f);
        self.save()
    }
}

// ---------------------------------------------------------------
// Tauri command wrappers
// ---------------------------------------------------------------

#[tauri::command]
pub fn library_list(library: State<'_, Library>) -> Vec<Game> {
    library.list()
}

#[tauri::command]
pub fn library_add(
    name: String,
    bottle_id: String,
    install_dir: String,
    launch_exe: String,
    library: State<'_, Library>,
) -> Result<Game, LibraryError> {
    // Auto-apply a matching profile's settings. Falls back to defaults
    // if no profile in the bundled or user set matches the game name.
    let settings = crate::profiles::find_for(&name)
        .map(|p| p.settings)
        .unwrap_or_default();
    let game = Game {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        bottle_id,
        install_dir,
        launch_exe,
        last_played_ms: None,
        total_play_ms: 0,
        settings,
    };
    library.add(game.clone())?;
    Ok(game)
}

#[tauri::command]
pub fn library_remove(id: String, library: State<'_, Library>) -> Result<(), LibraryError> {
    library.remove(&id)
}

#[tauri::command]
pub fn library_update_settings(
    id: String,
    settings: GameSettings,
    library: State<'_, Library>,
) -> Result<(), LibraryError> {
    library.update_settings(&id, settings)
}
