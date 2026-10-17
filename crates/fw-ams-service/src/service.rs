//! Service registration and the service control handler.

use crate::logger::data_dir;
use crate::server::{self, ServiceConfig, SERVICE_CONFIG};
use fw_ams_tuner::protocol::{SERVICE_DISPLAY_NAME, SERVICE_NAME};
use fw_ams_tuner::smu::pawnio::{find_module, MODULE_FILE};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use windows_service::service::{
    ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode, ServiceInfo,
    ServiceStartType, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
use windows_service::{define_windows_service, service_dispatcher};

const EXE_NAME: &str = "fw-ams-service.exe";

/// Admin-only location the service runs from, so a normal user cannot swap the binary
fn install_dir() -> PathBuf {
    std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
        .join("fw-ams-tuner")
}

fn err(what: &str) -> impl Fn(windows_service::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

fn stop_and_delete(manager: &ServiceManager) -> Result<(), String> {
    let Ok(service) = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
    ) else {
        return Ok(());
    };
    if service
        .query_status()
        .is_ok_and(|s| s.current_state != ServiceState::Stopped)
    {
        let _ = service.stop();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(15) {
            if service
                .query_status()
                .is_ok_and(|s| s.current_state == ServiceState::Stopped)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    service.delete().map_err(err("Dienst entfernen"))?;
    drop(service);
    // Deletion completes once all handles are closed
    std::thread::sleep(Duration::from_millis(500));
    Ok(())
}

pub fn install(user_sid: Option<String>, module: Option<PathBuf>) -> Result<(), String> {
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(err("Dienstverwaltung (Admin-Rechte?)"))?;
    stop_and_delete(&manager)?;

    // Copy binary and PawnIO module to Program Files
    let dir = install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let target = dir.join(EXE_NAME);
    if exe != target {
        std::fs::copy(&exe, &target).map_err(|e| format!("Kopieren nach {}: {e}", target.display()))?;
    }
    match module.filter(|m| m.is_file()).or_else(find_module) {
        Some(m) if m != dir.join(MODULE_FILE) => {
            std::fs::copy(&m, dir.join(MODULE_FILE)).map_err(|e| format!("{MODULE_FILE} kopieren: {e}"))?;
        }
        Some(_) => {}
        None => log::warn!("{MODULE_FILE} not found next to the installer, SMU tuning will be unavailable"),
    }

    let data = data_dir();
    std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    let cfg = ServiceConfig { user_sid };
    std::fs::write(
        data.join(SERVICE_CONFIG),
        serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: target,
        launch_arguments: vec![OsString::from("run")],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };
    let service = manager
        .create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START)
        .map_err(err("Dienst anlegen"))?;
    let _ = service.set_description(
        "fw-ams: CPU-Powerlimits, Temperaturgrenze und Curve Optimizer über PawnIO (nur geprüfte Befehle).",
    );
    service.start::<&str>(&[]).map_err(err("Dienst starten"))?;
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(err("Dienstverwaltung (Admin-Rechte?)"))?;
    stop_and_delete(&manager)?;
    let dir = install_dir();
    let _ = std::fs::remove_file(dir.join(EXE_NAME));
    let _ = std::fs::remove_file(dir.join(MODULE_FILE));
    let _ = std::fs::remove_dir(&dir);
    Ok(())
}

define_windows_service!(ffi_service_main, service_main);

pub fn run_dispatcher() -> Result<(), String> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main).map_err(err("Dienst-Dispatcher"))
}

fn service_main(_args: Vec<OsString>) {
    if let Err(e) = run_service() {
        log::error!("service failed: {e}");
    }
}

fn run_service() -> Result<(), windows_service::Error> {
    let (tx, rx) = mpsc::channel();
    let handler = move |control| match control {
        ServiceControl::Stop | ServiceControl::Shutdown => {
            let _ = tx.send(());
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let status = service_control_handler::register(SERVICE_NAME, handler)?;
    let set = |state, accept| {
        status.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accept,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::from_secs(5),
            process_id: None,
        })
    };
    set(ServiceState::StartPending, ServiceControlAccept::empty())?;
    let running = server::start();
    set(
        ServiceState::Running,
        ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
    )?;
    let _ = rx.recv();
    set(ServiceState::StopPending, ServiceControlAccept::empty())?;
    running.shutdown();
    log::info!("fw-ams-service stopped");
    set(ServiceState::Stopped, ServiceControlAccept::empty())?;
    Ok(())
}
