//! SMU access through the PawnIO driver (https://pawnio.eu, namazso) and its signed
//! `RyzenSMU` module (PawnIO.Modules, LGPL-2.1).
//!
//! PawnIO replaces WinRing0, which Windows Defender blocks as `HackTool:Win32/Winring0`.
//! `PawnIOLib.dll` is loaded at runtime so a missing driver is a status, not a crash.
//! Opening the driver requires administrator rights; the fw-ams service runs as LocalSystem.

use super::{SmuBackend, SmuError};
use crate::pm_table::{qwords_to_bytes, READ_BYTES};
use libloading::Library;
use std::ffi::{c_char, c_void, CString};
use std::path::PathBuf;

type Handle = *mut c_void;
type FnOpen = unsafe extern "system" fn(*mut Handle) -> i32;
type FnLoad = unsafe extern "system" fn(Handle, *const u8, usize) -> i32;
type FnExecute =
    unsafe extern "system" fn(Handle, *const c_char, *const u64, usize, *mut u64, usize, *mut usize) -> i32;
type FnClose = unsafe extern "system" fn(Handle) -> i32;
type FnVersion = unsafe extern "system" fn(*mut u32) -> i32;

pub const MODULE_FILE: &str = "RyzenSMU.bin";

pub struct PawnIo {
    // Keeps the DLL loaded for the function pointers below
    _lib: Library,
    execute: FnExecute,
    close: FnClose,
    handle: Handle,
    pm_table_version: Option<u32>,
    pub driver_version: Option<u32>,
    pub module_path: PathBuf,
}

// The handle is only used behind the engine mutex
unsafe impl Send for PawnIo {}

/// Places PawnIOLib.dll is looked up, in order
pub fn library_candidates() -> Vec<PathBuf> {
    let mut v = vec![];
    for var in ["ProgramFiles", "ProgramW6432"] {
        if let Some(pf) = std::env::var_os(var) {
            v.push(PathBuf::from(pf).join("PawnIO").join("PawnIOLib.dll"));
        }
    }
    v.push(PathBuf::from("PawnIOLib.dll"));
    v
}

/// Places the RyzenSMU module is looked up, in order
pub fn module_candidates() -> Vec<PathBuf> {
    let mut v = vec![];
    if let Some(p) = std::env::var_os("FW_AMS_PAWNIO_MODULE") {
        v.push(PathBuf::from(p));
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
        v.push(dir.join(MODULE_FILE));
        v.push(dir.join("resources").join(MODULE_FILE));
    }
    if let Some(pd) = std::env::var_os("ProgramData") {
        v.push(PathBuf::from(pd).join("fw-ams").join(MODULE_FILE));
    }
    v
}

pub fn find_module() -> Option<PathBuf> {
    module_candidates().into_iter().find(|p| p.is_file())
}

pub fn driver_installed() -> bool {
    library_candidates().iter().take(2).any(|p| p.is_file())
}

fn check(what: &str, hr: i32) -> Result<(), SmuError> {
    if hr >= 0 {
        Ok(())
    } else {
        Err(SmuError::Io(format!("{what}: HRESULT 0x{:08X}", hr as u32)))
    }
}

impl PawnIo {
    pub fn open() -> Result<Self, SmuError> {
        let lib = library_candidates()
            .into_iter()
            .find_map(|p| unsafe { Library::new(&p) }.ok())
            .ok_or_else(|| SmuError::DriverMissing("PawnIOLib.dll nicht gefunden".into()))?;
        let module_path = find_module().ok_or_else(|| {
            SmuError::ModuleMissing(format!(
                "{MODULE_FILE} nicht gefunden (gesucht: {})",
                module_candidates()
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
        let blob = std::fs::read(&module_path).map_err(|e| SmuError::ModuleMissing(e.to_string()))?;

        // SAFETY: symbol signatures follow PawnIOLib.h (HRESULT variants)
        unsafe {
            let open: FnOpen = *lib
                .get::<FnOpen>(b"pawnio_open\0")
                .map_err(|e| SmuError::DriverMissing(e.to_string()))?;
            let load: FnLoad = *lib
                .get::<FnLoad>(b"pawnio_load\0")
                .map_err(|e| SmuError::DriverMissing(e.to_string()))?;
            let execute: FnExecute = *lib
                .get::<FnExecute>(b"pawnio_execute\0")
                .map_err(|e| SmuError::DriverMissing(e.to_string()))?;
            let close: FnClose = *lib
                .get::<FnClose>(b"pawnio_close\0")
                .map_err(|e| SmuError::DriverMissing(e.to_string()))?;
            let driver_version = lib.get::<FnVersion>(b"pawnio_version\0").ok().and_then(|f| {
                let mut v = 0u32;
                (f(&mut v) >= 0).then_some(v)
            });

            let mut handle: Handle = std::ptr::null_mut();
            let hr = open(&mut handle);
            if hr < 0 {
                return Err(SmuError::DriverMissing(format!(
                    "Treiber lässt sich nicht öffnen (HRESULT 0x{:08X}; Admin-Rechte?)",
                    hr as u32
                )));
            }
            let hr = load(handle, blob.as_ptr(), blob.len());
            if hr < 0 {
                close(handle);
                return Err(SmuError::ModuleMissing(format!(
                    "Modul wird nicht geladen (HRESULT 0x{:08X}, CPU nicht unterstützt oder Signatur ungültig)",
                    hr as u32
                )));
            }
            Ok(Self {
                _lib: lib,
                execute,
                close,
                handle,
                pm_table_version: None,
                driver_version,
                module_path,
            })
        }
    }

    pub fn call(&mut self, name: &str, input: &[u64], out_len: usize) -> Result<Vec<u64>, SmuError> {
        let name = CString::new(name).map_err(|e| SmuError::Io(e.to_string()))?;
        let mut out = vec![0u64; out_len];
        let mut returned = 0usize;
        // SAFETY: buffers are valid for the given sizes for the duration of the call
        let hr = unsafe {
            (self.execute)(
                self.handle,
                name.as_ptr(),
                input.as_ptr(),
                input.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut returned,
            )
        };
        check(name.to_str().unwrap_or("execute"), hr)?;
        out.truncate(returned.min(out_len));
        Ok(out)
    }

    fn one(&mut self, name: &str) -> Result<u64, SmuError> {
        self.call(name, &[], 1)?
            .first()
            .copied()
            .ok_or_else(|| SmuError::Io(format!("{name}: keine Daten")))
    }
}

impl Drop for PawnIo {
    fn drop(&mut self) {
        // SAFETY: handle came from pawnio_open and is closed exactly once
        unsafe {
            (self.close)(self.handle);
        }
    }
}

impl SmuBackend for PawnIo {
    fn code_name(&mut self) -> Result<u64, SmuError> {
        self.one("ioctl_get_code_name")
    }

    fn smu_version(&mut self) -> Result<u32, SmuError> {
        self.one("ioctl_get_smu_version").map(|v| v as u32)
    }

    fn read_register(&mut self, addr: u32) -> Result<u32, SmuError> {
        let v = self.call("ioctl_read_smu_register", &[addr as u64], 1)?;
        v.first()
            .map(|v| *v as u32)
            .ok_or_else(|| SmuError::Io("read_smu_register: keine Daten".into()))
    }

    fn write_register(&mut self, addr: u32, value: u32) -> Result<(), SmuError> {
        self.call("ioctl_write_smu_register", &[addr as u64, value as u64], 0)
            .map(|_| ())
    }

    fn pm_table(&mut self) -> Result<(u32, Vec<u8>), SmuError> {
        let version = match self.pm_table_version {
            Some(v) => v,
            None => {
                let r = self.call("ioctl_resolve_pm_table", &[], 2)?;
                let v = r.first().copied().unwrap_or(0) as u32;
                self.pm_table_version = Some(v);
                v
            }
        };
        self.call("ioctl_update_pm_table", &[], 0)?;
        let q = self.call("ioctl_read_pm_table", &[], READ_BYTES / 8)?;
        Ok((version, qwords_to_bytes(&q)))
    }
}
