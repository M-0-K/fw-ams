//! Low-battery power saver: carries out what the core state machine decides (Windows power
//! mode, tuner saver profile) and puts the previous power mode back afterwards.

use crate::settings::BatterySettings;
use crate::tuner::Tuner;
use fw_ams_core::saver::{SaverChange, SaverState};
use fw_ams_core::Snapshot;
use fw_ams_tuner::winpower::{self, PowerMode};

pub struct Notice {
    pub title: &'static str,
    pub body: String,
}

#[derive(Default)]
pub struct PowerSaver {
    state: SaverState,
    /// Windows power mode to restore when the saver ends
    restore: Option<PowerMode>,
}

impl PowerSaver {
    /// Evaluate one snapshot; returns a notification when the saver switched
    pub fn update(&mut self, s: &Snapshot, cfg: &BatterySettings, tuner: &Tuner) -> Option<Notice> {
        match self.state.evaluate(s, cfg.saver.then_some(cfg.saver_percent))? {
            SaverChange::Activate => {
                if cfg.saver_power_mode {
                    let previous = winpower::power_mode();
                    match winpower::set_power_mode(PowerMode::Efficiency) {
                        Ok(()) => self.restore = previous,
                        Err(e) => log::warn!("saver: {e}"),
                    }
                }
                tuner.set_saver(true);
                Some(Notice {
                    title: "Energiesparmodus aktiv",
                    body: format!(
                        "Akku unter {} % – Energieeffizienz und Sparprofil sind aktiv.",
                        cfg.saver_percent
                    ),
                })
            }
            SaverChange::Deactivate => {
                self.leave(tuner);
                Some(Notice {
                    title: "Energiesparmodus beendet",
                    body: "Vorheriger Energiemodus wiederhergestellt.".into(),
                })
            }
        }
    }

    /// Leave the saver right away (reset, quit). Returns true if it was active.
    pub fn stop(&mut self, tuner: &Tuner) -> bool {
        let was_active = self.state.reset();
        if was_active {
            self.leave(tuner);
        }
        was_active
    }

    fn leave(&mut self, tuner: &Tuner) {
        if let Some(mode) = self.restore.take() {
            if let Err(e) = winpower::set_power_mode(mode) {
                log::warn!("saver restore: {e}");
            }
        }
        tuner.set_saver(false);
    }
}
