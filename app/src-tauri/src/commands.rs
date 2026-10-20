//! Tauri commands called by the UI.

use crate::charge::ChargeControl;
use crate::reset::{self, ResetReport, ResetRequest};
use crate::settings::Settings;
use crate::state::SharedState;
use crate::tuner::{HistorySample, TunerView};
use crate::{flyout, show_main_window};
use fw_ams_core::Snapshot;
use fw_ams_tuner::winpower::{self, PowerMode};
use fw_ams_tuner::{TunerConfig, TunerState};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_notification::NotificationExt;

#[tauri::command]
pub fn get_snapshot(state: State<SharedState>) -> Option<Snapshot> {
    state.latest.read().unwrap().clone()
}

/// Open the main window, optionally jumping to the details of one slot
#[tauri::command]
pub fn show_main(app: AppHandle, slot: Option<u8>) {
    flyout::hide(&app);
    show_main_window(&app);
    if let Some(slot) = slot {
        let _ = app.emit("navigate", slot);
    }
}

#[tauri::command]
pub fn test_notification(app: AppHandle) -> Result<(), String> {
    app.notification()
        .builder()
        .title("fw-ams")
        .body("Benachrichtigungen funktionieren.")
        .show()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<SharedState>) -> Settings {
    state.settings.read().unwrap().clone()
}

#[tauri::command]
pub fn set_settings(app: AppHandle, state: State<SharedState>, settings: Settings) -> Result<Settings, String> {
    state.commit_settings(&app, settings)
}

/// Forget the remembered card of one slot, or of all slots with `None`
#[tauri::command]
pub fn forget_cards(state: State<SharedState>, slot: Option<u8>) {
    let mut cards = state.cards.lock().unwrap();
    cards.forget(slot);
    state.save_cards(&cards);
    state.wake.notify();
}

/// Switch the extra features off or reset to factory settings; may show a UAC prompt
#[tauri::command(async)]
pub fn reset_system(app: AppHandle, state: State<'_, SharedState>, request: ResetRequest) -> ResetReport {
    reset::run(&app, &state, request)
}

#[tauri::command]
pub fn get_charge_control(state: State<SharedState>) -> ChargeControl {
    state.charge.lock().unwrap().view()
}

#[tauri::command]
pub fn charge_full_once(state: State<SharedState>) -> Result<ChargeControl, String> {
    let collector = state.collector.lock().unwrap();
    let mut charge = state.charge.lock().unwrap();
    charge.full_once(collector.as_ref())?;
    Ok(charge.view())
}

#[tauri::command]
pub fn get_tuner(state: State<SharedState>) -> Option<TunerView> {
    state.tuner.latest()
}

#[tauri::command]
pub fn get_history(state: State<SharedState>) -> Vec<HistorySample> {
    state.tuner.history()
}

#[tauri::command(async)]
pub fn tuner_set_config(state: State<'_, SharedState>, config: TunerConfig) -> Result<TunerState, String> {
    state.tuner.set_config(config)
}

#[tauri::command(async)]
pub fn tuner_reset(state: State<'_, SharedState>) -> Result<TunerState, String> {
    state.tuner.reset()
}

/// Shows a UAC prompt, so it must not block the main thread
#[tauri::command(async)]
pub fn tuner_install(state: State<'_, SharedState>) -> Result<(), String> {
    state.tuner.install()
}

/// Keeps the profiles for a later reinstall; the power plan is restored by the service
#[tauri::command(async)]
pub fn tuner_uninstall(state: State<'_, SharedState>) -> Result<(), String> {
    state.tuner.uninstall(false)
}

#[tauri::command]
pub fn set_power_mode(mode: PowerMode) -> Result<(), String> {
    winpower::set_power_mode(mode)
}

/// Opens the PawnIO download page in the default browser
#[tauri::command]
pub fn open_pawnio_site() -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg("https://pawnio.eu/")
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        Err("Nur unter Windows verfügbar".into())
    }
}
