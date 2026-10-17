//! Engine loop and named pipe server.

use crate::logger::data_dir;
use fw_ams_tuner::client::wide;
use fw_ams_tuner::protocol::{Request, Response, PIPE_NAME};
use fw_ams_tuner::winpower::{self, WindowsPowerPlan};
use fw_ams_tuner::Engine;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::FromRawHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows_sys::Win32::Foundation::{GetLastError, LocalFree, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::{FlushFileBuffers, PIPE_ACCESS_DUPLEX};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

const TICK: Duration = Duration::from_secs(1);

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct ServiceConfig {
    /// User allowed to talk to the service (set at install time)
    pub user_sid: Option<String>,
}

pub const SERVICE_CONFIG: &str = "service.json";

fn load_service_config() -> ServiceConfig {
    std::fs::read_to_string(data_dir().join(SERVICE_CONFIG))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// SYSTEM and administrators: full access; the installing user (or, without a stored SID,
/// interactive users): read/write
fn pipe_sddl(sid: Option<&str>) -> String {
    let user = match sid {
        Some(s) if s.starts_with("S-1-") && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') => s,
        _ => "IU",
    };
    format!("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;{user})")
}

pub struct Running {
    pub stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
}

impl Running {
    pub fn shutdown(self) {
        self.stop.store(true, Ordering::SeqCst);
        // Unblock ConnectNamedPipe by connecting once
        let _ = std::fs::OpenOptions::new().read(true).write(true).open(PIPE_NAME);
        for t in self.threads {
            let _ = t.join();
        }
    }
}

pub fn start() -> Running {
    let smu = fw_ams_tuner::open_smu();
    if let Err(e) = &smu {
        log::warn!("SMU not available: {e}");
    }
    let mut engine = Engine::new(
        smu,
        Box::new(WindowsPowerPlan),
        Some(data_dir()),
        winpower::on_ac().unwrap_or(true),
    );
    engine.start();
    let engine = Arc::new(Mutex::new(engine));
    let stop = Arc::new(AtomicBool::new(false));

    let ticker = {
        let (engine, stop) = (engine.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                engine.lock().unwrap().tick(winpower::on_ac());
                std::thread::sleep(TICK);
            }
        })
    };
    let server = {
        let (engine, stop) = (engine.clone(), stop.clone());
        std::thread::spawn(move || serve(engine, stop))
    };
    log::info!("fw-ams-service {} running", env!("CARGO_PKG_VERSION"));
    Running {
        stop,
        threads: vec![ticker, server],
    }
}

pub fn run_console() {
    let running = start();
    eprintln!("fw-ams-service läuft im Vordergrund. Beenden mit Strg+C.");
    // Ctrl+C terminates the process; values in the SMU stay until reboot either way
    while !running.stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn serve(engine: Arc<Mutex<Engine>>, stop: Arc<AtomicBool>) {
    let sddl = wide(&pipe_sddl(load_service_config().user_sid.as_deref()));
    let name = wide(PIPE_NAME);
    while !stop.load(Ordering::SeqCst) {
        // SAFETY: descriptor freed with LocalFree; pipe handle owned by `File` below
        let handle = unsafe {
            let mut sd = std::ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, std::ptr::null_mut()) == 0 {
                log::error!("invalid pipe security descriptor");
                return;
            }
            let sa = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd,
                bInheritHandle: 0,
            };
            let h = CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                256 * 1024,
                64 * 1024,
                0,
                &sa,
            );
            LocalFree(sd as _);
            h
        };
        if handle == INVALID_HANDLE_VALUE {
            log::error!("CreateNamedPipe failed: {}", unsafe { GetLastError() });
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }
        // SAFETY: valid pipe handle
        let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) } != 0
            || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        // SAFETY: we own the handle; File closes it on drop
        let file = unsafe { std::fs::File::from_raw_handle(handle as _) };
        if connected && !stop.load(Ordering::SeqCst) {
            if let Err(e) = handle_client(&file, &engine) {
                log::warn!("pipe client: {e}");
            }
            unsafe {
                FlushFileBuffers(handle);
                DisconnectNamedPipe(handle);
            }
        }
        drop(file);
    }
}

fn handle_client(file: &std::fs::File, engine: &Mutex<Engine>) -> std::io::Result<()> {
    let mut line = String::new();
    BufReader::new(file).read_line(&mut line)?;
    let response = match serde_json::from_str::<Request>(&line) {
        Ok(req) => process(req, engine),
        Err(e) => Response::Error {
            message: format!("Ungültige Anfrage: {e}"),
        },
    };
    let mut out = serde_json::to_string(&response)?;
    out.push('\n');
    let mut w = file;
    w.write_all(out.as_bytes())
}

fn process(req: Request, engine: &Mutex<Engine>) -> Response {
    let mut e = engine.lock().unwrap();
    match req {
        Request::GetState => {}
        Request::SetConfig { config } => e.set_config(config),
        Request::SetSaver { active } => e.set_saver(active),
        Request::Reset => e.reset(),
        Request::PmDump => {
            return match e.pm_dump() {
                Ok((version, data)) => Response::PmDump { version, data },
                Err(err) => Response::Error { message: err.to_string() },
            }
        }
    }
    Response::State {
        state: Box::new(e.state()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sddl_rejects_injection() {
        assert!(pipe_sddl(Some("S-1-5-21-1-2-3-1001")).ends_with("(A;;GRGW;;;S-1-5-21-1-2-3-1001)"));
        assert!(pipe_sddl(Some("S-1-5)(A;;GA;;;WD")).ends_with("(A;;GRGW;;;IU)"));
        assert!(pipe_sddl(None).ends_with("(A;;GRGW;;;IU)"));
    }
}
