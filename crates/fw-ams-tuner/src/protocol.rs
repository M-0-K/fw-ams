//! Named pipe protocol between app and service: one JSON request line, one JSON response line.
//!
//! The service only accepts these typed requests and clamps every value; raw SMU commands
//! are never exposed over the pipe.

use crate::types::{TunerConfig, TunerState};
use serde::{Deserialize, Serialize};

pub const PIPE_NAME: &str = r"\\.\pipe\fw-ams-tuner";
pub const SERVICE_NAME: &str = "fw-ams-tuner";
pub const SERVICE_DISPLAY_NAME: &str = "fw-ams Tuner";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum Request {
    GetState,
    SetConfig { config: TunerConfig },
    SetSaver { active: bool },
    Reset,
    /// Raw PM table for diagnostics
    PmDump,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Response {
    State { state: Box<TunerState> },
    PmDump { version: u32, data: Vec<u8> },
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut config = TunerConfig::default();
        config.ac.stapm_w = Some(20.0);
        let r = Request::SetConfig { config };
        let line = serde_json::to_string(&r).unwrap();
        assert!(!line.contains('\n'));
        assert_eq!(serde_json::from_str::<Request>(&line).unwrap(), r);
    }
}
