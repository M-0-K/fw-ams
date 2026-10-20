//! Small JSON files in the app config directory.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Files of the app (settings, card memory, values to restore)
#[derive(Debug, Clone)]
pub struct Paths {
    pub settings: PathBuf,
    /// `None` with `--mock`: the mock layout must not end up in the real card memory
    pub cards: Option<PathBuf>,
    pub charge: Option<PathBuf>,
}

impl Paths {
    pub fn new(dir: &Path, mock: bool) -> Self {
        Self {
            settings: dir.join("settings.json"),
            cards: (!mock).then(|| dir.join("cards.json")),
            charge: (!mock).then(|| dir.join("charge-restore.json")),
        }
    }
}

/// Must match `identifier` in tauri.conf.json: Tauri names the app config dir after it
pub const IDENTIFIER: &str = "io.github.fw-ams";

/// The app config dir without a running Tauri app (same rule as Tauri's `app_config_dir`)
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    base.map(|b| b.join(IDENTIFIER))
}

/// Missing or unreadable files give the default
pub fn load<T: DeserializeOwned + Default>(path: &Path) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Writes through a temporary file, so a crash never leaves half a file behind
pub fn save<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
    std::fs::rename(&tmp, path)
}

pub fn remove(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_matches_tauri_config() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], IDENTIFIER);
    }
}
