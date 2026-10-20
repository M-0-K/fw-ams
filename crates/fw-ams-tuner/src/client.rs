//! App side of the service connection (Windows): pipe requests, service status, elevated
//! install via UAC.
//!
//! Every request has a deadline: the pipe is synchronous, so the exchange runs on a worker
//! thread whose blocking I/O is cancelled when the service does not answer in time. After a
//! timeout further requests fail fast for a short while, so a hung service cannot pile up
//! worker threads (the app polls once per second).

use crate::protocol::{Request, Response, PIPE_NAME, SERVICE_NAME};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long the service may take for one request (SMU writes take milliseconds)
pub const TIMEOUT: Duration = Duration::from_secs(5);
/// After a timeout, requests fail right away for this long
const BACKOFF: Duration = Duration::from_secs(15);

static UNRESPONSIVE_SINCE: Mutex<Option<Instant>> = Mutex::new(None);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// Pipe does not exist: service not installed or not running
    NotRunning,
    /// Pipe exists but this user may not use it
    AccessDenied,
    /// The service did not answer in time
    Timeout,
    Io(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::NotRunning => write!(f, "Dienst nicht erreichbar"),
            ClientError::AccessDenied => write!(f, "Kein Zugriff auf den Dienst (anderer Benutzer?)"),
            ClientError::Timeout => write!(f, "Dienst antwortet nicht (Zeitüberschreitung)"),
            ClientError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// One request to the fw-ams service, with [`TIMEOUT`]
pub fn request(req: &Request) -> Result<Response, ClientError> {
    {
        let mut since = UNRESPONSIVE_SINCE.lock().unwrap();
        if since.is_some_and(|t| t.elapsed() < BACKOFF) {
            return Err(ClientError::Timeout);
        }
        *since = None;
    }
    let r = request_at(PIPE_NAME, req, TIMEOUT);
    if matches!(r, Err(ClientError::Timeout)) {
        *UNRESPONSIVE_SINCE.lock().unwrap() = Some(Instant::now());
    }
    r
}

/// One request to the pipe `pipe`, giving up after `timeout`
pub fn request_at(pipe: &str, req: &Request, timeout: Duration) -> Result<Response, ClientError> {
    let (tx, rx) = mpsc::channel();
    let (pipe, req) = (pipe.to_owned(), req.clone());
    let worker = std::thread::spawn(move || {
        let _ = tx.send(exchange(&pipe, &req));
    });
    if let Ok(r) = rx.recv_timeout(timeout) {
        let _ = worker.join();
        return r;
    }
    // Abort the blocking read/write. Retried a few times: the worker may be between two
    // calls when the first cancel arrives.
    for _ in 0..20 {
        cancel_io(&worker);
        if rx.recv_timeout(Duration::from_millis(50)).is_ok() {
            let _ = worker.join();
            break;
        }
    }
    log::warn!("service did not answer within {timeout:?}");
    Err(ClientError::Timeout)
}

fn cancel_io(worker: &std::thread::JoinHandle<()>) {
    use std::os::windows::io::AsRawHandle;
    // SAFETY: the handle belongs to a thread that is still joinable
    unsafe {
        windows_sys::Win32::System::IO::CancelSynchronousIo(worker.as_raw_handle() as _);
    }
}

fn exchange(pipe: &str, req: &Request) -> Result<Response, ClientError> {
    let mut tries = 0;
    let file = loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(pipe) {
            Ok(f) => break f,
            Err(e) => match e.raw_os_error() {
                Some(2) => return Err(ClientError::NotRunning),
                Some(5) => return Err(ClientError::AccessDenied),
                // ERROR_PIPE_BUSY: the service is answering another request
                Some(231) if tries < 20 => {
                    tries += 1;
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                _ => return Err(ClientError::Io(e.to_string())),
            },
        }
    };
    let mut line = serde_json::to_string(req).map_err(|e| ClientError::Io(e.to_string()))?;
    line.push('\n');
    let mut writer = &file;
    writer
        .write_all(line.as_bytes())
        .map_err(|e| ClientError::Io(e.to_string()))?;
    let mut reader = BufReader::new(&file);
    let mut answer = String::new();
    reader
        .read_line(&mut answer)
        .map_err(|e| ClientError::Io(e.to_string()))?;
    serde_json::from_str(&answer).map_err(|e| ClientError::Io(format!("Ungültige Antwort: {e}")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    NotInstalled,
    Stopped,
    Running,
    Unknown,
}

#[cfg(windows)]
pub fn service_status() -> ServiceStatus {
    use windows_sys::Win32::System::Services::*;
    let name = wide(SERVICE_NAME);
    // SAFETY: handles are checked and closed; pointers valid for the calls
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if scm.is_null() {
            return ServiceStatus::Unknown;
        }
        let svc = OpenServiceW(scm, name.as_ptr(), SERVICE_QUERY_STATUS);
        let status = if svc.is_null() {
            ServiceStatus::NotInstalled
        } else {
            let mut st: SERVICE_STATUS = std::mem::zeroed();
            let s = if QueryServiceStatus(svc, &mut st) != 0 && st.dwCurrentState == SERVICE_RUNNING {
                ServiceStatus::Running
            } else {
                ServiceStatus::Stopped
            };
            CloseServiceHandle(svc);
            s
        };
        CloseServiceHandle(scm);
        status
    }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// SID of the user running this process, e.g. "S-1-5-21-…"
#[cfg(windows)]
pub fn current_user_sid() -> Option<String> {
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree};
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    // SAFETY: buffers sized by the first GetTokenInformation call; handles closed
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let mut len = 0u32;
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr() as _, len, &mut len) != 0;
        CloseHandle(token);
        if !ok {
            return None;
        }
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s: windows_sys::core::PWSTR = std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut s) == 0 {
            return None;
        }
        let len = (0..).take_while(|i| *s.add(*i) != 0).count();
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(s, len));
        LocalFree(s as _);
        Some(sid)
    }
}

/// Run a program elevated (UAC prompt) and wait for it. Returns the exit code.
#[cfg(windows)]
pub fn run_elevated(exe: &Path, args: &str) -> Result<u32, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_CANCELLED};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let verb = wide("runas");
    let file = wide(&exe.to_string_lossy());
    let params = wide(args);
    // SAFETY: all strings outlive the call; process handle closed below
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_HIDE;
        if ShellExecuteExW(&mut info) == 0 {
            let e = GetLastError();
            return Err(if e == ERROR_CANCELLED {
                "Abgebrochen (UAC-Abfrage abgelehnt)".into()
            } else {
                format!("Start fehlgeschlagen (Fehler {e})")
            });
        }
        if info.hProcess.is_null() {
            return Err("Kein Prozess gestartet".into());
        }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 0u32;
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        Ok(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
    use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT};

    /// Test pipe server for one client; `answer` = None never replies
    fn serve_once(name: &str, answer: Option<Response>) -> std::thread::JoinHandle<()> {
        let wide_name = wide(name);
        // SAFETY: default security, handle owned by the File below
        let handle = unsafe {
            CreateNamedPipeW(wide_name.as_ptr(), PIPE_ACCESS_DUPLEX, PIPE_TYPE_BYTE | PIPE_WAIT, 1, 4096, 4096, 0, std::ptr::null())
        };
        assert_ne!(handle, INVALID_HANDLE_VALUE);
        let handle = handle as usize;
        std::thread::spawn(move || {
            let handle = handle as windows_sys::Win32::Foundation::HANDLE;
            // SAFETY: valid pipe handle
            unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) };
            let file = unsafe { std::fs::File::from_raw_handle(handle as _) };
            let mut line = String::new();
            let _ = BufReader::new(&file).read_line(&mut line);
            match answer {
                Some(r) => {
                    let mut out = serde_json::to_string(&r).unwrap();
                    out.push('\n');
                    let _ = (&file).write_all(out.as_bytes());
                }
                // Hold the connection without answering until the client gives up
                None => std::thread::sleep(Duration::from_secs(3)),
            }
        })
    }

    fn pipe_name(tag: &str) -> String {
        format!(r"\\.\pipe\fw-ams-test-{tag}-{}", std::process::id())
    }

    #[test]
    fn hung_service_times_out() {
        let name = pipe_name("hung");
        let server = serve_once(&name, None);
        let start = Instant::now();
        let r = request_at(&name, &Request::GetState, Duration::from_millis(300));
        assert_eq!(r, Err(ClientError::Timeout));
        assert!(start.elapsed() < Duration::from_secs(2), "took {:?}", start.elapsed());
        let _ = server.join();
    }

    #[test]
    fn answer_arrives() {
        let name = pipe_name("ok");
        let answer = Response::Error { message: "test".into() };
        let server = serve_once(&name, Some(answer.clone()));
        assert_eq!(request_at(&name, &Request::GetState, Duration::from_secs(5)), Ok(answer));
        let _ = server.join();
    }

    #[test]
    fn missing_service_is_not_running() {
        let r = request_at(&pipe_name("missing"), &Request::GetState, Duration::from_secs(1));
        assert_eq!(r, Err(ClientError::NotRunning));
    }
}
