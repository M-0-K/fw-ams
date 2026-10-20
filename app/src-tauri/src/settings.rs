//! User settings, persisted as JSON in the app config directory.

use crate::store;
use fw_ams_core::alerts::AlertSettings;
use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

/// Color scheme of all windows
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    /// Follow the OS setting
    #[default]
    System,
    Light,
    Dark,
}

/// Battery care and low-battery behaviour
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct BatterySettings {
    /// fw-ams writes the charge limit to the EC (on change, at start and when the charger is
    /// plugged in). Off: the BIOS setting stays in charge.
    pub manage_charge_limit: bool,
    /// Stop charging here, %
    pub charge_max: u8,
    /// Resume charging below this, % (current Framework firmware ignores it)
    pub charge_min: u8,
    /// Charge current limit as C-rate (0 = no limit, 0.5 = half the design capacity per hour)
    pub charge_rate: f32,
    /// Power saver below `saver_percent` on battery
    pub saver: bool,
    pub saver_percent: u8,
    /// Saver also switches the Windows power mode to "Beste Energieeffizienz"
    pub saver_power_mode: bool,
}

impl Default for BatterySettings {
    fn default() -> Self {
        Self {
            manage_charge_limit: false,
            charge_max: 80,
            charge_min: 0,
            charge_rate: 0.0,
            saver: false,
            saver_percent: 20,
            saver_power_mode: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    /// Extra features that change the system: CPU tuning, charge limit/current, power saver.
    /// Off: fw-ams only monitors and writes nothing to EC, SMU or Windows power settings.
    pub advanced_features: bool,
    pub poll_interval_ms: u32,
    pub autostart: bool,
    /// When started by autostart, stay in the tray
    pub start_minimized: bool,
    pub theme: Theme,
    pub alerts: AlertSettings,
    pub battery: BatterySettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            advanced_features: true,
            poll_interval_ms: 2000,
            autostart: false,
            start_minimized: true,
            theme: Theme::System,
            alerts: AlertSettings::default(),
            battery: BatterySettings::default(),
        }
    }
}

impl Settings {
    pub const MIN_POLL_MS: u32 = 500;
    pub const MAX_POLL_MS: u32 = 60_000;

    pub fn sanitized(mut self) -> Self {
        self.poll_interval_ms = self.poll_interval_ms.clamp(Self::MIN_POLL_MS, Self::MAX_POLL_MS);
        self.alerts.battery_low_percent = self.alerts.battery_low_percent.clamp(2, 99);
        // The critical level must stay below the warning level
        self.alerts.battery_critical_percent = self
            .alerts
            .battery_critical_percent
            .clamp(1, self.alerts.battery_low_percent - 1);
        let b = &mut self.battery;
        b.charge_max = b.charge_max.clamp(fw_ams_core::source::ec::CHARGE_LIMIT_MIN_MAX, 100);
        b.charge_min = b.charge_min.min(b.charge_max.saturating_sub(5));
        b.charge_rate = if b.charge_rate.is_finite() { b.charge_rate.clamp(0.0, 1.0) } else { 0.0 };
        b.saver_percent = b.saver_percent.clamp(5, 50);
        self.alerts.weak_charger_watts = self.alerts.weak_charger_watts.clamp(5, 240);
        self.alerts.high_temp_celsius = self.alerts.high_temp_celsius.clamp(50, 110);
        if !self.advanced_features {
            // Nothing may touch the system while the extra features are off
            self.battery = BatterySettings::default();
        }
        self
    }
}

pub fn load(path: &Path) -> Settings {
    store::load::<Settings>(path).sanitized()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_battery_levels() {
        let mut s = Settings::default();
        s.alerts.battery_low_percent = 10;
        s.alerts.battery_critical_percent = 30;
        s.battery.charge_max = 10;
        s.battery.charge_min = 90;
        s.battery.charge_rate = f32::NAN;
        let s = s.sanitized();
        assert_eq!(s.alerts.battery_critical_percent, 9);
        assert_eq!(s.battery.charge_max, 25);
        assert_eq!(s.battery.charge_min, 20);
        assert_eq!(s.battery.charge_rate, 0.0);
    }

    #[test]
    fn advanced_off_drops_battery_management() {
        let mut s = Settings::default();
        s.battery.manage_charge_limit = true;
        s.battery.saver = true;
        s.advanced_features = false;
        let s = s.sanitized();
        assert_eq!(s.battery, BatterySettings::default());
    }
}
