//! User settings, persisted as JSON in the app config directory.

use fw_ams_core::alerts::AlertSettings;
use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    pub poll_interval_ms: u32,
    pub autostart: bool,
    /// When started by autostart, stay in the tray
    pub start_minimized: bool,
    pub alerts: AlertSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            poll_interval_ms: 2000,
            autostart: false,
            start_minimized: true,
            alerts: AlertSettings::default(),
        }
    }
}

impl Settings {
    pub const MIN_POLL_MS: u32 = 500;
    pub const MAX_POLL_MS: u32 = 60_000;

    pub fn sanitized(mut self) -> Self {
        self.poll_interval_ms = self.poll_interval_ms.clamp(Self::MIN_POLL_MS, Self::MAX_POLL_MS);
        self.alerts.battery_low_percent = self.alerts.battery_low_percent.clamp(1, 99);
        self
    }
}

pub fn load(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
        .unwrap_or_default()
        .sanitized()
}

pub fn save(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(settings)?)
}
