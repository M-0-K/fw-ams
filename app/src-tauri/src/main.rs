// Prevents an additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod charge;
mod commands;
mod flyout;
mod poller;
mod reset;
mod saver;
mod settings;
mod state;
mod store;
mod tray;
mod tuner;

use charge::ChargeController;
use fw_ams_core::cards::memory::CardMemory;
use fw_ams_core::Collector;
use saver::PowerSaver;
use state::{AppState, SharedState, Wake};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use store::Paths;
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tuner::Tuner;

pub fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
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

    let app = tauri::Builder::default()
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
            let paths = Paths::new(&app.path().app_config_dir()?, use_mock);
            let settings = settings::load(&paths.settings);
            log::info!("fw-ams starting (mock: {use_mock}), settings {:?}", paths.settings);

            let handle = app.handle().clone();
            state::apply_autostart(&handle, settings.autostart);
            let stay_in_tray = started_minimized && settings.start_minimized;
            let cards: CardMemory = paths.cards.as_deref().map(store::load).unwrap_or_default();

            let state: SharedState = Arc::new(AppState {
                latest: RwLock::new(None),
                settings: RwLock::new(settings),
                collector: Mutex::new((!use_mock).then(Collector::new)),
                charge: Mutex::new(ChargeController::new(use_mock, paths.charge.clone())),
                cards: Mutex::new(cards),
                saver: Mutex::new(PowerSaver::default()),
                tuner: Tuner::new(use_mock, service_exe(), pawnio_module(&handle)),
                wake: Wake::default(),
                paths,
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
            poller::spawn_poller(handle.clone(), state.clone());
            poller::spawn_tuner(handle, state);
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
            commands::get_snapshot,
            commands::get_settings,
            commands::set_settings,
            commands::show_main,
            commands::test_notification,
            commands::forget_cards,
            commands::reset_system,
            commands::get_charge_control,
            commands::charge_full_once,
            commands::get_tuner,
            commands::get_history,
            commands::tuner_set_config,
            commands::tuner_reset,
            commands::tuner_install,
            commands::tuner_uninstall,
            commands::set_power_mode,
            commands::open_pawnio_site
        ])
        .build(tauri::generate_context!())
        .expect("error while building fw-ams");

    app.run(|app, event| {
        // A saver that is active when the app quits would otherwise leave Windows in
        // "best power efficiency"
        if let RunEvent::Exit = event {
            if let Some(state) = app.try_state::<SharedState>() {
                state.saver.lock().unwrap().stop(&state.tuner);
            }
        }
    });
}
