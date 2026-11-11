//! Background threads: hardware poller (snapshot, card memory, charge control, alerts, saver)
//! and the 1 Hz tuner sampler.

use crate::state::SharedState;
use crate::tuner::{HistorySample, TunerView};
use crate::tray;
use fw_ams_core::alerts::AlertState;
use fw_ams_core::mock;
use serde::Serialize;
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

/// How often to retry opening the EC while it is unavailable
const EC_RETRY: Duration = Duration::from_secs(30);
/// Sampling interval of the tuner / live charts
const TUNER_TICK: Duration = Duration::from_secs(1);
/// A poll that comes this much later than planned means the system was asleep
const RESUME_GAP: Duration = Duration::from_secs(30);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TunerTick {
    view: TunerView,
    sample: HistorySample,
}

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::warn!("notification failed: {e}");
    }
}

pub fn spawn_poller(app: AppHandle, state: SharedState) {
    std::thread::spawn(move || {
        let mut alerts = AlertState::default();
        let mut last_retry = Instant::now();
        let mut last_poll: Option<SystemTime> = None;
        // A previous app run may have left the service in saver mode. Done here, before the
        // first saver evaluation, so it cannot undo a saver that activates right away.
        if state.settings.read().unwrap().advanced_features {
            state.tuner.set_saver(false);
        }
        loop {
            let (snapshot, settings) = {
                let mut collector = state.collector.lock().unwrap();
                // Read under the collector lock: a reset holds it while it changes settings
                let settings = state.settings.read().unwrap().clone();
                let resumed = woke_from_sleep(&mut last_poll, settings.poll_interval_ms);
                if resumed {
                    log::info!("resume from standby detected, re-applying EC settings");
                }
                if let Some(c) = collector.as_mut() {
                    if !c.ec_available() && (resumed || last_retry.elapsed() >= EC_RETRY) {
                        last_retry = Instant::now();
                        c.retry_ec();
                    }
                }
                let mut snapshot = match collector.as_mut() {
                    Some(c) => c.snapshot(),
                    None => mock::snapshot(),
                };
                {
                    let mut cards = state.cards.lock().unwrap();
                    cards.apply(&mut snapshot);
                    if cards.take_save(snapshot.timestamp_ms) {
                        state.save_cards(&cards);
                    }
                }
                let mut charge = state.charge.lock().unwrap();
                if resumed {
                    // The EC may have dropped the charge current limit or reset the charge
                    // limit while the system was asleep
                    charge.resync();
                }
                charge.sync(collector.as_ref(), &snapshot, &settings.battery);
                let _ = app.emit("charge", charge.view());
                (snapshot, settings)
            };

            for alert in alerts.evaluate(&snapshot, &settings.alerts) {
                notify(&app, &alert.title, &alert.body);
            }
            let notice = state.saver.lock().unwrap().update(&snapshot, &settings.battery, &state.tuner);
            if let Some(n) = notice {
                notify(&app, n.title, &n.body);
            }
            tray::update(&app, &snapshot);
            if let Err(e) = app.emit("snapshot", &snapshot) {
                log::warn!("emit failed: {e}");
            }
            *state.latest.write().unwrap() = Some(snapshot);

            state.wake.wait(Duration::from_millis(settings.poll_interval_ms as u64));
        }
    });
}

/// True when the wall clock moved much further than the poll interval since the last poll,
/// i.e. the system was suspended in between
fn woke_from_sleep(last: &mut Option<SystemTime>, interval_ms: u32) -> bool {
    let now = SystemTime::now();
    let gap = last.and_then(|l| now.duration_since(l).ok());
    *last = Some(now);
    gap.is_some_and(|g| g > Duration::from_millis(interval_ms as u64) + RESUME_GAP)
}

pub fn spawn_tuner(app: AppHandle, state: SharedState) {
    std::thread::spawn(move || loop {
        // With the extra features off the service is left alone entirely
        if state.settings.read().unwrap().advanced_features {
            let snapshot = state.latest.read().unwrap().clone();
            let (view, sample) = state.tuner.tick(snapshot.as_ref());
            if let Err(e) = app.emit("tuner", TunerTick { view, sample }) {
                log::warn!("emit failed: {e}");
            }
        }
        std::thread::sleep(TUNER_TICK);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_resume_from_wall_clock_gap() {
        let mut last = None;
        assert!(!woke_from_sleep(&mut last, 2000), "first poll");
        assert!(!woke_from_sleep(&mut last, 2000), "regular poll");
        last = Some(SystemTime::now() - Duration::from_secs(20));
        assert!(!woke_from_sleep(&mut last, 2000), "late, but no standby");
        last = Some(SystemTime::now() - Duration::from_secs(3600));
        assert!(woke_from_sleep(&mut last, 2000), "an hour of standby");
    }
}
