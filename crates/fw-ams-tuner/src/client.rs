//! App side of the service connection (Windows): pipe requests, service status, elevated
//! install via UAC.

use crate::protocol::{Request, Response, PIPE_NAME, SERVICE_NAME};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// Pipe does not exist: service not installed or not running
    NotRunning,
    /// Pipe exists but this user may not use it
    AccessDenied,
    Io(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::NotRunning => write!(f, "Dienst nicht erreichbar"),
            ClientError::AccessDenied => write!(f, "Kein Zugriff auf den Dienst (anderer Benutzer?)"),
            ClientError::Io(e) => write!(f, "{e}"),
        }
    }
}

pub fn request(req: &Request) -> Result<Response, ClientError> {
    let mut tries = 0;
    let file = loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(PIPE_NAME) {
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
