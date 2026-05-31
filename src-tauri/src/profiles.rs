//! Per-game runtime profiles.
//!
//! cellar ships a bundled set of known profiles at `cellar/profiles.json`
//! (compiled in via `include_str!`). The user can override or extend
//! these at `~/.cellar/profiles.json`; when present, user entries are
//! tried before bundled ones, so a user override of "carx-street" wins
//! over the bundled profile of the same id.
//!
//! Matching is by case-insensitive substring against the game's name,
//! using `match_name_contains`. The first profile whose substring list
//! matches wins.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::library::GameSettings;

const BUNDLED: &str = include_str!("../../profiles.json");

#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Case-insensitive substrings tried against the game's name.
    /// An empty list means the profile never auto-matches; it's
    /// available as a manual selection only.
    #[serde(default)]
    pub match_name_contains: Vec<String>,
    #[serde(default)]
    pub description: String,
    pub settings: GameSettings,
    /// Free-form list of human-readable preconditions (e.g.,
    /// "proton_winrt_dlls", "winetricks_mf"). The frontend can show
    /// these as a checklist before the user launches; cellar does not
    /// auto-install them.
    #[serde(default)]
    pub requires: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct ProfilesFile {
    pub version: u32,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

fn parse(s: &str) -> Vec<Profile> {
    serde_json::from_str::<ProfilesFile>(s)
        .map(|f| f.profiles)
        .unwrap_or_default()
}

fn user_profiles_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".cellar").join("profiles.json"))
}

/// Load profiles: user overrides first, then bundled defaults. A user
/// profile whose `id` matches a bundled one shadows the bundled entry.
pub fn load_all() -> Vec<Profile> {
    let mut out: Vec<Profile> = Vec::new();
    if let Some(p) = user_profiles_path() {
        if let Ok(s) = fs::read_to_string(&p) {
            out.extend(parse(&s));
        }
    }
    let mut bundled = parse(BUNDLED);
    bundled.retain(|b| !out.iter().any(|u| u.id == b.id));
    out.extend(bundled);
    out
}

/// Find the first profile whose `match_name_contains` substring is
/// present in `game_name` (case-insensitive). Profiles with an empty
/// `match_name_contains` are skipped — they're manual-pick only.
pub fn find_for(game_name: &str) -> Option<Profile> {
    let needle = game_name.to_lowercase();
    for p in load_all() {
        for hint in &p.match_name_contains {
            if needle.contains(&hint.to_lowercase()) {
                return Some(p);
            }
        }
    }
    None
}

// ---------------------------------------------------------------
// Tauri command wrappers
// ---------------------------------------------------------------

#[tauri::command]
pub fn profiles_list() -> Vec<Profile> {
    load_all()
}

#[tauri::command]
pub fn profiles_find(game_name: String) -> Option<Profile> {
    find_for(&game_name)
}
