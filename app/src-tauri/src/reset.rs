//! Switching the extra features off and the factory reset: undo everything fw-ams changed on
//! the system and report each step, so the user sees what is back to normal.
//!
//! - CPU tuning: profiles cleared, SMU and Windows power plan values back to their originals
//!   (via the service; uninstalling it restores the power plan as well)
//! - charge limit back to the EC's own value, charge current limit removed
//! - power saver left, previous Windows power mode restored
//! - settings: extra features off (or everything to defaults), autostart entry removed
//! - card memory cleared (factory reset only)
//!
//! [`headless`] is the part the Windows uninstaller runs (`fw-ams-app --restore-system`); the
//! service is removed by the uninstaller itself (`fw-ams-service uninstall --purge`).

use crate::charge::ChargeController;
use crate::settings::{BatterySettings, Settings};
use crate::state::AppState;
use crate::store::{self, Paths};
use fw_ams_core::Collector;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResetRequest {
    /// All settings to their defaults and the card memory cleared. Otherwise only the extra
    /// features are switched off.
    pub factory: bool,
    /// Also uninstall the helper service and delete its profiles (UAC prompt)
    pub remove_service: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StepResult {
    Done,
    /// Nothing to do
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResetStep {
    pub label: String,
    pub result: StepResult,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResetReport {
    pub steps: Vec<ResetStep>,
}

impl ResetReport {
    fn push(&mut self, label: &str, result: StepResult, detail: impl Into<Option<String>>) {
        self.steps.push(ResetStep {
            label: label.into(),
            result,
            detail: detail.into(),
        });
    }

    fn outcome(&mut self, label: &str, r: Result<String, String>) {
        match r {
            Ok(d) => self.push(label, StepResult::Done, d),
            Err(e) => self.push(label, StepResult::Failed, e),
        }
    }
}

pub fn run(app: &AppHandle, state: &AppState, req: ResetRequest) -> ResetReport {
    let mut report = ResetReport::default();

    // Settings first, under the poller's locks, so it cannot write the old values again
    {
        let collector = state.collector.lock().unwrap();
        let mut charge = state.charge.lock().unwrap();
        let old = state.settings.read().unwrap().clone();
        let new = if req.factory {
            Settings::default()
        } else {
            Settings {
                advanced_features: false,
                battery: BatterySettings::default(),
                ..old.clone()
            }
        };
        let label = if req.factory { "Einstellungen" } else { "Erweiterte Funktionen" };
        let detail = match (req.factory, old.autostart && !new.autostart) {
            (true, true) => "Werkseinstellungen, Autostart-Eintrag entfernt".to_string(),
            (true, false) => "Werkseinstellungen".to_string(),
            (false, _) => "ausgeschaltet – fw-ams überwacht nur noch".to_string(),
        };
        report.outcome(label, state.commit_settings(app, new).map(|_| detail));
        report.outcome("Akku", charge.restore(collector.as_ref()));
    }

    let saver = if state.saver.lock().unwrap().stop(&state.tuner) {
        (StepResult::Done, "beendet, vorheriger Windows-Energiemodus wiederhergestellt")
    } else {
        (StepResult::Skipped, "war nicht aktiv")
    };
    report.push("Energiesparmodus", saver.0, saver.1.to_string());

    tuner_steps(state, req, &mut report);

    if req.factory {
        let mut cards = state.cards.lock().unwrap();
        cards.forget(None);
        state.save_cards(&cards);
        report.push("Gemerkte Karten", StepResult::Done, "gelöscht".to_string());
    }
    state.wake.notify();
    log::info!("reset ({req:?}): {:?}", report.steps);
    report
}

fn tuner_steps(state: &AppState, req: ResetRequest, report: &mut ResetReport) {
    const LABEL: &str = "CPU-Tuning";
    let tuner = &state.tuner;
    if tuner.is_mock() {
        report.outcome(LABEL, tuner.reset().map(|_| "Simulation zurückgesetzt".into()));
        return;
    }
    let setup = tuner.setup();
    if !setup.service_installed {
        report.push(LABEL, StepResult::Skipped, "Hilfsdienst nicht installiert – nichts verändert".to_string());
    } else if setup.service_running {
        report.outcome(
            LABEL,
            tuner.reset().map(|_| {
                "Profile geleert, Powerlimits, Curve Optimizer und Windows-Energieplan auf Standard".into()
            }),
        );
    } else if req.remove_service {
        report.push(
            LABEL,
            StepResult::Skipped,
            "Hilfsdienst läuft nicht – die Deinstallation stellt den Energieplan wieder her, \
             SMU-Werte gelten nur bis zum Neustart"
                .to_string(),
        );
    } else {
        report.push(
            LABEL,
            StepResult::Failed,
            "Hilfsdienst installiert, aber nicht erreichbar – gespeicherte Profile würden beim \
             nächsten Start wieder angewendet. Hilfsdienst mit entfernen."
                .to_string(),
        );
    }

    if req.remove_service && setup.service_installed {
        report.outcome(
            "Hilfsdienst",
            tuner
                .uninstall(true)
                .map(|_| "entfernt, Profile und Protokoll gelöscht".into()),
        );
    }
    if req.factory && setup.pawnio_installed {
        report.push(
            "PawnIO-Treiber",
            StepResult::Skipped,
            "separat installiert – bei Bedarf unter Windows-Einstellungen → Apps deinstallieren".to_string(),
        );
    }
}

/// `fw-ams-app --restore-system`, run by the uninstaller without UI: the charge limit goes back
/// to the EC's own value and the charge current limit is removed. Needs no admin rights.
/// Returns the process exit code.
pub fn headless() -> i32 {
    let Some(dir) = store::config_dir() else {
        eprintln!("Kein App-Konfigurationsordner gefunden");
        return 1;
    };
    let paths = Paths::new(&dir, false);
    let collector = Collector::new();
    let mut charge = ChargeController::new(false, paths.charge);
    match charge.restore(Some(&collector)) {
        Ok(done) => {
            println!("Akku: {done}");
            0
        }
        Err(e) => {
            eprintln!("Akku: {e}");
            1
        }
    }
}
