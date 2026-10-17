//! Windows power settings: power mode (overlay), power plan values and AC state.
//!
//! The power mode (Efficiency/Balanced/Performance slider in Windows settings) works per
//! user without admin rights; Framework maps it to its 15/25/30 W firmware profiles.
//! Power plan values (max. frequency, boost, EPP) are written by the service.

use crate::types::ParamId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PowerMode {
    /// "Beste Energieeffizienz"
    Efficiency,
    /// "Ausbalanciert"
    Balanced,
    /// "Beste Leistung"
    Performance,
    /// Some other overlay (e.g. vendor specific)
    Other,
}

#[cfg(windows)]
mod imp {
    use super::PowerMode;
    use crate::types::ParamId;
    use windows_sys::core::GUID;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::System::Power::{
        GetSystemPowerStatus, PowerGetActiveScheme, PowerReadACValueIndex, PowerReadDCValueIndex,
        PowerSetActiveScheme, PowerWriteACValueIndex, PowerWriteDCValueIndex, SYSTEM_POWER_STATUS,
    };

    // Undocumented but stable since Windows 10 1709 (used by the Settings app itself). Not in
    // the import library, so resolved at runtime.
    type FnGetOverlay = unsafe extern "system" fn(*mut GUID) -> u32;
    type FnSetOverlay = unsafe extern "system" fn(*const GUID) -> u32;

    fn powrprof() -> Option<&'static libloading::Library> {
        static LIB: std::sync::OnceLock<Option<libloading::Library>> = std::sync::OnceLock::new();
        // SAFETY: powrprof.dll is a system library without problematic init code
        LIB.get_or_init(|| unsafe { libloading::Library::new("powrprof.dll") }.ok())
            .as_ref()
    }

    unsafe fn power_get_effective_overlay(overlay: *mut GUID) -> u32 {
        match powrprof().and_then(|l| l.get::<FnGetOverlay>(b"PowerGetEffectiveOverlayScheme\0").ok()) {
            Some(f) => f(overlay),
            None => 1,
        }
    }

    unsafe fn power_set_active_overlay(overlay: *const GUID) -> u32 {
        match powrprof().and_then(|l| l.get::<FnSetOverlay>(b"PowerSetActiveOverlayScheme\0").ok()) {
            Some(f) => f(overlay),
            None => 1,
        }
    }

    const OVERLAY_EFFICIENCY: u128 = 0x961cc777_2547_4f9d_8174_7d86181b8a7a;
    const OVERLAY_BALANCED: u128 = 0;
    const OVERLAY_PERFORMANCE: u128 = 0xded574b5_45a0_4f42_8737_46345c09c238;

    const SUB_PROCESSOR: u128 = 0x54533251_82be_4824_96c1_47b60b740d00;
    const PROCFREQMAX: u128 = 0x75b0ae3f_bce0_45a7_8c89_c9611c25e100;
    const PERFBOOSTMODE: u128 = 0xbe337238_0d82_4146_a960_4f3749d470c7;
    const PERFEPP: u128 = 0x36687f9e_e3a5_4dbf_b1dc_15eb381c6863;

    fn as_u128(g: &GUID) -> u128 {
        ((g.data1 as u128) << 96)
            | ((g.data2 as u128) << 80)
            | ((g.data3 as u128) << 64)
            | u64::from_be_bytes(g.data4) as u128
    }

    pub fn power_mode() -> Option<PowerMode> {
        let mut g = GUID::from_u128(0);
        // SAFETY: out pointer is valid
        if unsafe { power_get_effective_overlay(&mut g) } != 0 {
            return None;
        }
        Some(match as_u128(&g) {
            OVERLAY_EFFICIENCY => PowerMode::Efficiency,
            OVERLAY_BALANCED => PowerMode::Balanced,
            OVERLAY_PERFORMANCE => PowerMode::Performance,
            _ => PowerMode::Other,
        })
    }

    pub fn set_power_mode(mode: PowerMode) -> Result<(), String> {
        let g = GUID::from_u128(match mode {
            PowerMode::Efficiency => OVERLAY_EFFICIENCY,
            PowerMode::Balanced | PowerMode::Other => OVERLAY_BALANCED,
            PowerMode::Performance => OVERLAY_PERFORMANCE,
        });
        // SAFETY: GUID pointer is valid for the call
        match unsafe { power_set_active_overlay(&g) } {
            0 => Ok(()),
            e => Err(format!("Energiemodus nicht gesetzt (Fehler {e})")),
        }
    }

    pub fn on_ac() -> Option<bool> {
        let mut s: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
        // SAFETY: out pointer is valid
        if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
            return None;
        }
        match s.ACLineStatus {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    fn setting(id: ParamId) -> Option<u128> {
        match id {
            ParamId::MaxFreqMhz => Some(PROCFREQMAX),
            ParamId::Boost => Some(PERFBOOSTMODE),
            ParamId::Epp => Some(PERFEPP),
            _ => None,
        }
    }

    fn with_scheme<T>(f: impl FnOnce(*const GUID) -> Result<T, String>) -> Result<T, String> {
        let mut scheme: *mut GUID = std::ptr::null_mut();
        // SAFETY: Windows allocates the GUID, freed with LocalFree below
        let e = unsafe { PowerGetActiveScheme(std::ptr::null_mut(), &mut scheme) };
        if e != 0 || scheme.is_null() {
            return Err(format!("Aktiver Energieplan nicht lesbar (Fehler {e})"));
        }
        let r = f(scheme);
        unsafe { LocalFree(scheme as _) };
        r
    }

    pub fn read_plan(id: ParamId, ac: bool) -> Result<f32, String> {
        let s = setting(id).ok_or("kein Energieplan-Wert")?;
        with_scheme(|scheme| {
            let (sub, set) = (GUID::from_u128(SUB_PROCESSOR), GUID::from_u128(s));
            let mut v = 0u32;
            // SAFETY: all pointers valid for the call
            let e = unsafe {
                if ac {
                    PowerReadACValueIndex(std::ptr::null_mut(), scheme, &sub, &set, &mut v)
                } else {
                    PowerReadDCValueIndex(std::ptr::null_mut(), scheme, &sub, &set, &mut v)
                }
            };
            if e == 0 {
                Ok(v as f32)
            } else {
                Err(format!("Energieplan-Wert nicht lesbar (Fehler {e})"))
            }
        })
    }

    pub fn write_plan(id: ParamId, ac: bool, value: f32) -> Result<(), String> {
        let s = setting(id).ok_or("kein Energieplan-Wert")?;
        with_scheme(|scheme| {
            let (sub, set) = (GUID::from_u128(SUB_PROCESSOR), GUID::from_u128(s));
            let v = value.round().max(0.0) as u32;
            // SAFETY: all pointers valid for the call
            let e = unsafe {
                if ac {
                    PowerWriteACValueIndex(std::ptr::null_mut(), scheme, &sub, &set, v)
                } else {
                    PowerWriteDCValueIndex(std::ptr::null_mut(), scheme, &sub, &set, v)
                }
            };
            if e != 0 {
                return Err(format!("Energieplan-Wert nicht geschrieben (Fehler {e}, Admin-Rechte?)"));
            }
            // Re-activating the scheme makes Windows pick up the new value
            let e = unsafe { PowerSetActiveScheme(std::ptr::null_mut(), scheme) };
            if e != 0 {
                return Err(format!("Energieplan nicht neu aktiviert (Fehler {e})"));
            }
            Ok(())
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use super::PowerMode;
    use crate::types::ParamId;

    pub fn power_mode() -> Option<PowerMode> {
        None
    }
    pub fn set_power_mode(_: PowerMode) -> Result<(), String> {
        Err("Nur unter Windows verfügbar".into())
    }
    pub fn on_ac() -> Option<bool> {
        None
    }
    pub fn read_plan(_: ParamId, _: bool) -> Result<f32, String> {
        Err("Nur unter Windows verfügbar".into())
    }
    pub fn write_plan(_: ParamId, _: bool, _: f32) -> Result<(), String> {
        Err("Nur unter Windows verfügbar".into())
    }
}

pub use imp::{on_ac, power_mode, set_power_mode};

/// Power plan access, abstracted so the engine can be tested and mocked
pub trait PowerPlan: Send {
    fn read(&mut self, id: ParamId, ac: bool) -> Result<f32, String>;
    fn write(&mut self, id: ParamId, ac: bool, value: f32) -> Result<(), String>;
}

pub struct WindowsPowerPlan;

impl PowerPlan for WindowsPowerPlan {
    fn read(&mut self, id: ParamId, ac: bool) -> Result<f32, String> {
        imp::read_plan(id, ac)
    }
    fn write(&mut self, id: ParamId, ac: bool, value: f32) -> Result<(), String> {
        imp::write_plan(id, ac, value)
    }
}

/// In-memory power plan for `--mock` and tests
#[derive(Default)]
pub struct MockPowerPlan {
    values: std::collections::HashMap<(ParamId, bool), f32>,
}

impl PowerPlan for MockPowerPlan {
    fn read(&mut self, id: ParamId, ac: bool) -> Result<f32, String> {
        let default = match id {
            ParamId::MaxFreqMhz => 0.0,
            ParamId::Boost => 2.0,
            ParamId::Epp => if ac { 33.0 } else { 50.0 },
            _ => return Err("kein Energieplan-Wert".into()),
        };
        Ok(*self.values.get(&(id, ac)).unwrap_or(&default))
    }
    fn write(&mut self, id: ParamId, ac: bool, value: f32) -> Result<(), String> {
        self.values.insert((id, ac), value);
        Ok(())
    }
}
