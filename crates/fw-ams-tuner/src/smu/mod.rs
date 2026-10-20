//! Access to the AMD System Management Unit (SMU).

pub mod mock;
#[cfg(windows)]
pub mod pawnio;

use crate::family::Mailbox;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmuError {
    /// Driver (PawnIO) is not installed or could not be opened
    DriverMissing(String),
    /// The RyzenSMU module file was not found
    ModuleMissing(String),
    /// SMU answered with an error code
    Response(u32),
    Timeout,
    Io(String),
}

impl fmt::Display for SmuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmuError::DriverMissing(e) => write!(f, "PawnIO-Treiber nicht verfügbar: {e}"),
            SmuError::ModuleMissing(e) => write!(f, "RyzenSMU-Modul fehlt: {e}"),
            SmuError::Response(code) => write!(f, "SMU lehnt ab ({})", response_text(*code)),
            SmuError::Timeout => write!(f, "SMU antwortet nicht (Timeout)"),
            SmuError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SmuError {}

/// SMU mailbox response codes
pub const SMU_OK: u32 = 0x01;

pub fn response_text(code: u32) -> &'static str {
    match code {
        0x01 => "OK",
        0xFC => "beschäftigt",
        0xFD => "Voraussetzung fehlt",
        0xFE => "unbekannter Befehl",
        0xFF => "fehlgeschlagen",
        _ => "unbekannte Antwort",
    }
}

pub trait SmuBackend: Send {
    /// Code name enum of the PawnIO RyzenSMU module (see `family_from_code_name`)
    fn code_name(&mut self) -> Result<u64, SmuError>;
    /// Raw SMU firmware version
    fn smu_version(&mut self) -> Result<u32, SmuError>;
    fn read_register(&mut self, addr: u32) -> Result<u32, SmuError>;
    fn write_register(&mut self, addr: u32, value: u32) -> Result<(), SmuError>;
    /// PM table version and a fresh copy of the table
    fn pm_table(&mut self) -> Result<(u32, Vec<u8>), SmuError>;

    /// Send a message through a register mailbox (RyzenAdj `smu_service_req`)
    fn send(&mut self, mb: Mailbox, msg: u32, args: [u32; 6]) -> Result<[u32; 6], SmuError> {
        // Wait until a previous command has finished
        let mut tries = 0;
        while self.read_register(mb.rsp)? == 0 {
            tries += 1;
            if tries > 2000 {
                return Err(SmuError::Timeout);
            }
            std::thread::sleep(std::time::Duration::from_micros(500));
        }
        self.write_register(mb.rsp, 0)?;
        for (i, a) in args.iter().enumerate() {
            self.write_register(mb.args + 4 * i as u32, *a)?;
        }
        self.write_register(mb.msg, msg)?;
        let mut tries = 0;
        let rsp = loop {
            let r = self.read_register(mb.rsp)?;
            if r != 0 {
                break r;
            }
            tries += 1;
            if tries > 2000 {
                return Err(SmuError::Timeout);
            }
            std::thread::sleep(std::time::Duration::from_micros(500));
        };
        if rsp != SMU_OK {
            return Err(SmuError::Response(rsp));
        }
        let mut out = [0u32; 6];
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.read_register(mb.args + 4 * i as u32)?;
        }
        Ok(out)
    }
}

/// "93.37.0" from the raw version register
pub fn format_smu_version(v: u32) -> String {
    format!("{}.{}.{}", (v >> 16) & 0xFF, (v >> 8) & 0xFF, v & 0xFF)
}
