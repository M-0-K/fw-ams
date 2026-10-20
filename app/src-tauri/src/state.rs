//! State shared by the commands and the background threads.
//!
//! Lock order where several are needed: `collector` → `charge` → `cards` / `saver`.

use crate::charge::ChargeController;
use crate::saver::PowerSaver;
use crate::settings::Settings;
use crate::store::{self, Paths};
use crate::tuner::Tuner;
use fw_ams_core::cards::memory::CardMemory;
use fw_ams_core::{Collector, Snapshot};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_autostart::ManagerExt;

pub struct AppState {
    pub latest: RwLock<Option<Snapshot>>,
    pub settings: RwLock<Settings>,
    pub paths: Paths,
    /// `None` with `--mock`; shared so commands can write to the EC
    pub collector: Mutex<Option<Collector>>,
    pub charge: Mutex<ChargeController>,
    pub cards: Mutex<CardMemory>,
    pub saver: Mutex<PowerSaver>,
    pub tuner: Tuner,
    /// Wakes the poller early, e.g. after a settings change
    pub wake: Wake,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    /// Sanitize, persist and apply new settings, then tell all windows
    pub fn commit_settings(&self, app: &AppHandle, settings: Settings) -> Result<Settings, String> {
        let settings = settings.sanitized();
        store::save(&self.paths.settings, &settings).map_err(|e| e.to_string())?;
        let autostart_changed = self.settings.read().unwrap().autostart != settings.autostart;
        *self.settings.write().unwrap() = settings.clone();
        if autostart_changed {
            apply_autostart(app, settings.autostart);
        }
        // Lets the flyout and main window pick up e.g. the new theme right away
        let _ = app.emit("settings", &settings);
        self.wake.notify();
        Ok(settings)
    }

    pub fn save_cards(&self, cards: &CardMemory) {
        if let Some(path) = &self.paths.cards {
            let r = if cards.is_empty() { store::remove(path) } else { store::save(path, cards) };
            if let Err(e) = r {
                log::warn!("card memory not saved: {e}");
            }
        }
    }
}

pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    let launcher = app.autolaunch();
    // Only touch the registry / autostart entry when it differs
    if launcher.is_enabled().ok() == Some(enabled) {
        return;
    }
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    if let Err(e) = result {
        log::warn!("autostart: {e}");
    }
}

/// One-shot signal with timeout, used as an interruptible sleep
#[derive(Default)]
pub struct Wake {
    pending: Mutex<bool>,
    cv: Condvar,
}

impl Wake {
    pub fn notify(&self) {
        *self.pending.lock().unwrap() = true;
        self.cv.notify_all();
    }

    /// Sleep for `timeout` or until [`Wake::notify`] is called
    pub fn wait(&self, timeout: Duration) {
        let pending = self.pending.lock().unwrap();
        let (mut pending, _) = self.cv.wait_timeout_while(pending, timeout, |p| !*p).unwrap();
        *pending = false;
    }
}
