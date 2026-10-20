//! fw-ams-service: privileged helper for CPU tuning (Windows service, LocalSystem).
//!
//! The app runs without admin rights and talks to this service over a named pipe that only
//! SYSTEM, administrators and the installing user may open. The service accepts typed,
//! clamped requests only (see `fw_ams_tuner::protocol`).
//!
//! Usage:
//!   fw-ams-service install [--user-sid S-1-5-…] [--module RyzenSMU.bin]
//!                                                 copy to Program Files, register, start
//!   fw-ams-service uninstall [--purge]            stop, unregister, remove files, restore the
//!                                                 Windows power plan; --purge also deletes
//!                                                 profiles and logs in %ProgramData%w-ams
//!   fw-ams-service console                        run in the foreground (debugging, needs admin)
//!   fw-ams-service run                            entry point used by the service manager

#[cfg(windows)]
mod logger;
#[cfg(windows)]
mod server;
#[cfg(windows)]
mod service;

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let purge = args.iter().any(|a| a == "--purge");
    if matches!(args.first().map(String::as_str), Some("install" | "uninstall")) {
        // Elevated runs have no visible console: errors go to service.log
        logger::init(false, !purge);
    }
    let code = match args.first().map(String::as_str) {
        Some("install") => {
            let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
            report(service::install(value("--user-sid"), value("--module").map(Into::into)))
        }
        Some("uninstall") => report(service::uninstall(purge)),
        Some("console") => {
            logger::init(true, true);
            server::run_console();
            0
        }
        Some("run") => {
            logger::init(false, true);
            report(service::run_dispatcher())
        }
        _ => {
            eprintln!("Usage: fw-ams-service install [--user-sid SID] [--module PATH] | uninstall [--purge] | console | run");
            2
        }
    };
    std::process::exit(code);
}

#[cfg(windows)]
fn report(r: Result<(), String>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            log::error!("{e}");
            1
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("fw-ams-service is only available on Windows. See docs/linux-advanced.md.");
    std::process::exit(2);
}
