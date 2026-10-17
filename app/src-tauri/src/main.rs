// Prevents an additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod charge;
mod flyout;
mod settings;
mod tray;
mod tuner;

use charge::{ChargeControl, ChargeController};
use fw_ams_core::alerts::AlertState;
use fw_ams_core::saver::{SaverChange, SaverState};
use fw_ams_core::{mock, Collector, Snapshot};
use fw_ams_tuner::winpower::{self, PowerMode};
use fw_ams_tuner::{TunerConfig, TunerState};
use serde::Serialize;
use settings::Settings;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;
use tuner::{HistorySample, Tuner, TunerView};

/// How often to retry opening the EC while it is unavailable
const EC_RETRY: Duration = Duration::from_secs(30);
/// Sampling interval of the tuner / live charts
const TUNER_TICK: Duration = Duration::from_secs(1);

struct AppState {
    latest: RwLock<Option<Snapshot>>,
    settings: RwLock<Settings>,
    settings_path: PathBuf,
    /// `None` with `--mock`; shared so commands can write to the EC
    collector: Mutex<Option<Collector>>,
    charge: Mutex<ChargeController>,
    tuner: Tuner,
    /// Windows power mode to restore when the battery saver ends
    saver_restore: Mutex<Option<PowerMode>>,
}

type SharedState = Arc<AppState>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TunerTick {
    view: TunerView,
    sample: HistorySample,
}

#[tauri::command]
fn get_snapshot(state: State<SharedState>) -> Option<Snapshot> {
    state.latest.read().unwrap().clone()
}

/// Open the main window, optionally jumping to the details of one slot
#[tauri::command]
fn show_main(app: AppHandle, slot: Option<u8>) {
    flyout::hide(&app);
    show_main_window(&app);
    if let Some(slot) = slot {
        let _ = app.emit("navigate", slot);
    }
}

#[tauri::command]
fn test_notification(app: AppHandle) -> Result<(), String> {
    app.notification()
        .builder()
        .title("fw-ams")
        .body("Benachrichtigungen funktionieren.")
        .show()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_charge_control(state: State<SharedState>) -> ChargeControl {
    state.charge.lock().unwrap().view()
}

#[tauri::command]
fn charge_full_once(state: State<SharedState>) -> Result<ChargeControl, String> {
    let collector = state.collector.lock().unwrap();
    let mut charge = state.charge.lock().unwrap();
    charge.full_once(collector.as_ref())?;
    Ok(charge.view())
}

#[tauri::command]
fn get_tuner(state: State<SharedState>) -> Option<TunerView> {
    state.tuner.latest()
}

#[tauri::command]
fn get_history(state: State<SharedState>) -> Vec<HistorySample> {
    state.tuner.history()
}

#[tauri::command(async)]
fn tuner_set_config(state: State<'_, SharedState>, config: TunerConfig) -> Result<TunerState, String> {
    state.tuner.set_config(config)
}

#[tauri::command(async)]
fn tuner_reset(state: State<'_, SharedState>) -> Result<TunerState, String> {
    state.tuner.reset()
}

/// Shows a UAC prompt, so it must not block the main thread
#[tauri::command(async)]
fn tuner_install(state: State<'_, SharedState>) -> Result<(), String> {
    state.tuner.install()
}

#[tauri::command(async)]
fn tuner_uninstall(state: State<'_, SharedState>) -> Result<(), String> {
    state.tuner.uninstall()
}

#[tauri::command]
fn set_power_mode(mode: PowerMode) -> Result<(), String> {
    winpower::set_power_mode(mode)
}

/// Opens the PawnIO download page in the default browser
#[tauri::command]
fn open_pawnio_site() -> Result<(), String> {
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

#[tauri::command]
fn get_settings(state: State<SharedState>) -> Settings {
    state.settings.read().unwrap().clone()
}

#[tauri::command]
fn set_settings(app: AppHandle, state: State<SharedState>, settings: Settings) -> Result<Settings, String> {
    let settings = settings.sanitized();
    settings::save(&state.settings_path, &settings).map_err(|e| e.to_string())?;
    apply_autostart(&app, settings.autostart);
    *state.settings.write().unwrap() = settings.clone();
    // Lets the flyout and main window pick up the new theme right away
    let _ = app.emit("settings", &settings);
    Ok(settings)
}

fn apply_autostart(app: &AppHandle, enabled: bool) {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    if let Err(e) = result {
        log::warn!("autostart: {e}");
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::warn!("notification failed: {e}");
    }
}

fn spawn_poller(app: AppHandle, state: SharedState) {
    std::thread::spawn(move || {
        let mut alerts = AlertState::default();
        let mut saver = SaverState::default();
        let mut last_retry = Instant::now();
        loop {
            let settings = state.settings.read().unwrap().clone();
            let snapshot = {
                let mut collector = state.collector.lock().unwrap();
                if let Some(c) = collector.as_mut() {
                    if !c.ec_available() && last_retry.elapsed() >= EC_RETRY {
                        last_retry = Instant::now();
                        c.retry_ec();
                    }
                }
                let snapshot = match collector.as_ref() {
                    Some(c) => c.snapshot(),
                    None => mock::snapshot(),
                };
                let mut charge = state.charge.lock().unwrap();
                charge.sync(collector.as_ref(), &snapshot, &settings.battery);
                let _ = app.emit("charge", charge.view());
                snapshot
            };

            for alert in alerts.evaluate(&snapshot, &settings.alerts) {
                notify(&app, &alert.title, &alert.body);
            }
            let b = &settings.battery;
            match saver.evaluate(&snapshot, b.saver.then_some(b.saver_percent)) {
                Some(SaverChange::Activate) => {
                    if b.saver_power_mode {
                        let previous = winpower::power_mode();
                        match winpower::set_power_mode(PowerMode::Efficiency) {
                            Ok(()) => *state.saver_restore.lock().unwrap() = previous,
                            Err(e) => log::warn!("saver: {e}"),
                        }
                    }
                    state.tuner.set_saver(true);
                    notify(
                        &app,
                        "Energiesparmodus aktiv",
                        &format!(
                            "Akku unter {} % – Energieeffizienz und Sparprofil sind aktiv.",
                            b.saver_percent
                        ),
                    );
                }
                Some(SaverChange::Deactivate) => {
                    if let Some(mode) = state.saver_restore.lock().unwrap().take() {
                        if let Err(e) = winpower::set_power_mode(mode) {
                            log::warn!("saver restore: {e}");
                        }
                    }
                    state.tuner.set_saver(false);
                    notify(&app, "Energiesparmodus beendet", "Vorheriger Energiemodus wiederhergestellt.");
                }
                None => {}
            }
            tray::update(&app, &snapshot);
            if let Err(e) = app.emit("snapshot", &snapshot) {
                log::warn!("emit failed: {e}");
            }
            *state.latest.write().unwrap() = Some(snapshot);

            std::thread::sleep(Duration::from_millis(settings.poll_interval_ms as u64));
        }
    });
}

fn spawn_tuner(app: AppHandle, state: SharedState) {
    std::thread::spawn(move || {
        // A previous app run may have left the service in saver mode
        state.tuner.set_saver(false);
        loop {
            let snapshot = state.latest.read().unwrap().clone();
            let (view, sample) = state.tuner.tick(snapshot.as_ref());
            if let Err(e) = app.emit("tuner", TunerTick { view, sample }) {
                log::warn!("emit failed: {e}");
            }
            std::thread::sleep(TUNER_TICK);
        }
    });
}

/// fw-ams-service.exe is bundled next to the app (and built into the same target dir)
fn service_exe() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("fw-ams-service.exe")))
}

/// RyzenSMU.bin: bundled resource, or the copy fetched for development
fn pawnio_module(app: &AppHandle) -> Option<PathBuf> {
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("resources").join("RyzenSMU.bin"));
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("RyzenSMU.bin");
    bundled.filter(|p| p.is_file()).or(Some(dev))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let use_mock = args.iter().any(|a| a == "--mock") || std::env::var_os("FW_AMS_MOCK").is_some();
    let started_minimized = args.iter().any(|a| a == "--minimized");
    // Debug aid: open the tray flyout right away
    let open_flyout = args.iter().any(|a| a == "--flyout");

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(move |app| {
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            let settings = settings::load(&settings_path);
            log::info!("fw-ams starting (mock: {use_mock}), settings {settings_path:?}");

            let handle = app.handle().clone();
            apply_autostart(&handle, settings.autostart);
            let stay_in_tray = started_minimized && settings.start_minimized;

            let state: SharedState = Arc::new(AppState {
                latest: RwLock::new(None),
                settings: RwLock::new(settings),
                settings_path,
                collector: Mutex::new((!use_mock).then(Collector::new)),
                charge: Mutex::new(ChargeController::new(use_mock)),
                tuner: Tuner::new(use_mock, service_exe(), pawnio_module(&handle)),
                saver_restore: Mutex::new(None),
            });
            app.manage(state.clone());

            tray::create(&handle)?;
            flyout::create(&handle)?;
            if !stay_in_tray {
                show_main_window(&handle);
            }
            if open_flyout {
                flyout::show_in_corner(&handle);
            }
            spawn_poller(handle.clone(), state.clone());
            spawn_tuner(handle, state);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the window keeps the app running in the tray
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            // Linux flyout is opened from the menu and closes when it loses focus
            WindowEvent::Focused(false) if cfg!(target_os = "linux") && window.label() == flyout::LABEL => {
                let _ = window.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_settings,
            set_settings,
            show_main,
            test_notification,
            get_charge_control,
            charge_full_once,
            get_tuner,
            get_history,
            tuner_set_config,
            tuner_reset,
            tuner_install,
            tuner_uninstall,
            set_power_mode,
            open_pawnio_site
        ])
        .run(tauri::generate_context!())
        .expect("error while running fw-ams");
}
