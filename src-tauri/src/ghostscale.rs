//! Link cellar games to ghostscale, the neural upscaler overlay.
//!
//! A game is linked when ghostscale's preferences hold `cellar.<game name>` = its ghostscale profile id.
//! The game's clickable app reads that through `ghostscale-hook.sh` and hands the launch to ghostscale.

use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;

const DOMAIN: &str = "com.joemunene.ghostscale";
const GAMES_DIR: &str = "/Applications/cellar Games";
const HOOK: &str = include_str!("../../scripts/ghostscale-hook.sh");

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GhostscaleError {
    NotInstalled,
    NoGameApp { name: String },
    Io { message: String },
}

impl From<std::io::Error> for GhostscaleError {
    fn from(e: std::io::Error) -> Self {
        GhostscaleError::Io { message: e.to_string() }
    }
}

#[derive(Serialize)]
pub struct LinkableGame {
    pub profile: String,
    pub name: String,
    pub linked: bool,
}

#[derive(Serialize)]
pub struct GhostscaleStatus {
    pub installed: bool,
    pub games: Vec<LinkableGame>,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn app() -> PathBuf {
    home().join("Applications/ghostscale.app")
}

fn profile_dirs() -> Vec<PathBuf> {
    vec![
        home().join("Library/Application Support/ghostscale/profiles"),
        app().join("Contents/Resources/profiles"),
    ]
}

fn linked_profile(name: &str) -> Option<String> {
    let out = Command::new("defaults").args(["read", DOMAIN, &format!("cellar.{name}")]).output().ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !v.is_empty()).then_some(v)
}

fn game_wrapper(name: &str) -> PathBuf {
    PathBuf::from(GAMES_DIR).join(format!("{name}.app/Contents/MacOS/{name}"))
}

/// Game apps built before the ghostscale link existed launch their script directly; add the hook in front.
fn ensure_hook(name: &str) -> Result<(), GhostscaleError> {
    let launchers = home().join(".cellar/launchers");
    std::fs::create_dir_all(&launchers)?;
    std::fs::write(launchers.join("ghostscale-hook.sh"), HOOK)?;

    let wrapper = game_wrapper(name);
    let text = std::fs::read_to_string(&wrapper)
        .map_err(|_| GhostscaleError::NoGameApp { name: name.to_string() })?;
    if !text.contains("ghostscale-hook.sh") {
        std::fs::write(&wrapper, add_hook(&text, name))?;
    }
    Ok(())
}

fn add_hook(wrapper: &str, name: &str) -> String {
    let mut out = String::new();
    for line in wrapper.lines() {
        if let Some(script) = line.strip_prefix("exec /bin/bash \"").and_then(|r| r.split('"').next()) {
            out.push_str("hook=\"$HOME/.cellar/launchers/ghostscale-hook.sh\"\n");
            out.push_str(&format!("[ -f \"$hook\" ] && . \"$hook\" \"{name}\" \"{script}\"\n"));
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[tauri::command]
pub fn ghostscale_status() -> GhostscaleStatus {
    let installed = app().is_dir();
    let mut games: Vec<LinkableGame> = Vec::new();
    for dir in profile_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Some(profile) = path.file_stem().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            let Some(name) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| v["name"].as_str().map(str::to_string))
            else {
                continue;
            };
            if games.iter().any(|g| g.profile == profile) || !game_wrapper(&name).exists() {
                continue;
            }
            let linked = linked_profile(&name).as_deref() == Some(profile.as_str());
            games.push(LinkableGame { profile, name, linked });
        }
    }
    games.sort_by(|a, b| a.name.cmp(&b.name));
    GhostscaleStatus { installed, games }
}

#[tauri::command]
pub fn ghostscale_link(name: String, profile: String, on: bool) -> Result<(), GhostscaleError> {
    if !app().is_dir() {
        return Err(GhostscaleError::NotInstalled);
    }
    let key = format!("cellar.{name}");
    if on {
        ensure_hook(&name)?;
        Command::new("defaults").args(["write", DOMAIN, &key, &profile]).status()?;
    } else {
        Command::new("defaults").args(["delete", DOMAIN, &key]).status()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::add_hook;

    #[test]
    fn hook_goes_before_the_launch() {
        let before = "#!/bin/bash\n# launcher wrapper for cellar game: ACC\nexec /bin/bash \"/x/acc.sh\" >>/tmp/cellar-game.log 2>&1\n";
        let after = add_hook(before, "ACC");
        let lines: Vec<&str> = after.lines().collect();
        assert_eq!(lines[2], "hook=\"$HOME/.cellar/launchers/ghostscale-hook.sh\"");
        assert_eq!(lines[3], "[ -f \"$hook\" ] && . \"$hook\" \"ACC\" \"/x/acc.sh\"");
        assert!(lines[4].starts_with("exec /bin/bash \"/x/acc.sh\""));
    }
}
