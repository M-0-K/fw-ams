// Prevents an additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod flyout;
mod settings;
mod tray;

use fw_ams_core::alerts::AlertState;
use fw_ams_core::{mock, Collector, Snapshot};
use settings::Settings;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;

/// How often to retry opening the EC while it is unavailable
const EC_RETRY: Duration = Duration::from_secs(30);

struct AppState {
    latest: RwLock<Option<Snapshot>>,
    settings: RwLock<Settings>,
    settings_path: PathBuf,
}

type SharedState = Arc<AppState>;

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

fn spawn_poller(app: AppHandle, state: SharedState, use_mock: bool) {
    std::thread::spawn(move || {
        let mut collector = (!use_mock).then(Collector::new);
        let mut alerts = AlertState::default();
        let mut last_retry = Instant::now();
        loop {
            if let Some(c) = collector.as_mut() {
                if !c.ec_available() && last_retry.elapsed() >= EC_RETRY {
                    last_retry = Instant::now();
                    c.retry_ec();
                }
            }
            let snapshot = match &collector {
                Some(c) => c.snapshot(),
                None => mock::snapshot(),
            };

            let settings = state.settings.read().unwrap().clone();
            for alert in alerts.evaluate(&snapshot, &settings.alerts) {
                if let Err(e) = app
                    .notification()
                    .builder()
                    .title(&alert.title)
                    .body(&alert.body)
                    .show()
                {
                    log::warn!("notification failed: {e}");
                }
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
            spawn_poller(handle, state, use_mock);
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
        .invoke_handler(tauri::generate_handler![get_snapshot, get_settings,
            set_settings,
            show_main,
            test_notification
        ])
        .run(tauri::generate_context!())
        .expect("error while running fw-ams");
}
