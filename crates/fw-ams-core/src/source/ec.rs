//! Framework embedded controller access via `framework_lib`.
//!
//! framework_lib unwraps internally in a few places, so every call is wrapped in
//! `catch_unwind` and failures become `None`. The returned raw structs are plain data so the
//! mapping logic can be unit tested without hardware.

use crate::model::{Model, Partner, PowerRole};
use framework_lib::chromium_ec::command::EcCommands;
use framework_lib::chromium_ec::commands::{ChargeLimitControlModes, EcRequestGetPdPortState};
use framework_lib::chromium_ec::{CrosEc, CrosEcDriver, EcRequestRaw};
use framework_lib::power::{self, FanSpeed, TempSensor, UsbPowerRoles};
use framework_lib::smbios;
use framework_lib::smbios::Platform;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub const PD_PORTS: u8 = 4;

fn guarded<T>(what: &str, f: impl FnOnce() -> Option<T>) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(_) => {
            log::warn!("framework_lib panicked in {what}");
            None
        }
    }
}

/// Like [`guarded`] for write commands, keeping the error text for the UI
fn guarded_write<E: std::fmt::Debug>(what: &str, f: impl FnOnce() -> Result<(), E>) -> Result<(), String> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            log::warn!("{what} failed: {e:?}");
            Err(format!("{e:?}"))
        }
        Err(_) => {
            log::warn!("framework_lib panicked in {what}");
            Err("framework_lib panic".into())
        }
    }
}

/// Lowest maximum charge limit the EC accepts (same check as framework_tool)
pub const CHARGE_LIMIT_MIN_MAX: u8 = 25;

#[derive(Debug, Clone, PartialEq)]
pub struct RawBattery {
    pub voltage_mv: u32,
    pub rate_ma: u32,
    pub remaining_mah: u32,
    pub full_mah: u32,
    pub design_mah: u32,
    pub cycle_count: u32,
    pub charging: bool,
    pub discharging: bool,
    pub critical: bool,
    pub manufacturer: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawPower {
    pub ac_present: bool,
    pub battery: Option<RawBattery>,
}

/// Framework-specific PD port state (EC host command 0x3E23, Cypress/Infineon PD controllers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RawPdState {
    pub c_state: u8,
    pub pd_contract: bool,
    pub power_role: u8,
    pub data_role: u8,
    pub vconn: bool,
    pub epr_active: bool,
    pub epr_supported: bool,
    pub voltage_mv: u16,
    pub current_ma: u16,
    pub active_port: bool,
    pub alt_mode: u8,
}

impl RawPdState {
    pub fn partner(&self) -> Partner {
        match self.c_state {
            0 => Partner::Nothing,
            1 => Partner::Sink,
            2 => Partner::Source,
            3 => Partner::DebugAccessory,
            4 => Partner::AudioAccessory,
            5 => Partner::PoweredAccessory,
            _ => Partner::Unknown,
        }
    }
}

/// Generic ChromeOS EC USB-PD power info (available on all Framework ECs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawPdPower {
    pub role: PowerRole,
    pub voltage_now_mv: u16,
    pub voltage_max_mv: u16,
    pub current_max_ma: u16,
    pub max_power_mw: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RawTemp {
    Celsius(u8),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawThermal {
    pub sensors: Vec<(String, RawTemp)>,
    /// (name, rpm, stalled) – fans that are not present are omitted
    pub fans: Vec<(String, Option<u32>, bool)>,
    pub throttle: Option<(bool, bool)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RawVersions {
    pub ec: Option<String>,
    pub pd: Vec<String>,
}

pub struct EcSource {
    ec: CrosEc,
}

impl EcSource {
    /// Returns `None` when no EC driver is reachable.
    pub fn open() -> Option<Self> {
        let ec = guarded("CrosEc::new", || Some(CrosEc::new()))?;
        // version_info is a cheap command that only succeeds with working driver access
        guarded("version_info", || ec.version_info().ok())?;
        Some(Self { ec })
    }

    pub fn power(&self) -> Option<RawPower> {
        guarded("power_info", || power::power_info(&self.ec)).map(|p| RawPower {
            ac_present: p.ac_present,
            battery: p.battery.map(|b| RawBattery {
                voltage_mv: b.present_voltage,
                rate_ma: b.present_rate,
                remaining_mah: b.remaining_capacity,
                full_mah: b.last_full_charge_capacity,
                design_mah: b.design_capacity,
                cycle_count: b.cycle_count,
                charging: b.charging,
                discharging: b.discharging,
                critical: b.level_critical,
                manufacturer: b.manufacturer,
                model: b.model_number,
            }),
        })
    }

    /// Charge limit as (min %, max %) – `None` if the EC does not support it
    pub fn charge_limit(&self) -> Option<(u8, u8)> {
        guarded("get_charge_limit", || self.ec.get_charge_limit().ok())
    }

    /// Set the sustainer range. Current Framework firmware only enforces `max`.
    pub fn set_charge_limit(&self, min: u8, max: u8) -> Result<(), String> {
        let max = max.clamp(CHARGE_LIMIT_MIN_MAX, 100);
        let min = min.min(max);
        guarded_write("set_charge_limit", || self.ec.set_charge_limit(min, max))
    }

    /// Charge to 100 % once, the limit applies again on the next cycle
    pub fn charge_limit_override(&self) -> Result<(), String> {
        let (min, max) = self.charge_limit().unwrap_or((0, 100));
        guarded_write("charge_limit_override", || {
            self.ec
                .send_command(
                    EcCommands::ChargeLimitControl as u16,
                    0,
                    &[ChargeLimitControlModes::Override as u8, max, min],
                )
                .map(|_| ())
        })
    }

    /// Limit the battery charge current; `None` removes the limit.
    /// The EC forgets this on reboot or shutdown, so callers re-apply it.
    pub fn set_charge_current_limit(&self, ma: Option<u32>) -> Result<(), String> {
        // ChromeOS EC treats u32::MAX as "no limit"
        let current = ma.unwrap_or(u32::MAX);
        guarded_write("set_charge_current_limit", || self.ec.set_charge_current_limit(current, None))
    }

    pub fn pd_states(&self) -> Vec<Option<RawPdState>> {
        (0..PD_PORTS)
            .map(|port| {
                guarded("get_pd_port_state", || {
                    let s = (EcRequestGetPdPortState { port }).send_command(&self.ec).ok()?;
                    Some(RawPdState {
                        c_state: s.c_state,
                        pd_contract: s.pd_state != 0,
                        power_role: s.power_role,
                        data_role: s.data_role,
                        vconn: s.vconn != 0,
                        epr_active: s.epr_active != 0,
                        epr_supported: s.epr_support != 0,
                        voltage_mv: { s.voltage },
                        current_ma: { s.current },
                        active_port: s.active_port != 0,
                        alt_mode: s.pd_alt_mode_status,
                    })
                })
            })
            .collect()
    }

    pub fn pd_power(&self) -> Vec<Option<RawPdPower>> {
        let infos = guarded("get_pd_info", || Some(power::get_pd_info(&self.ec, PD_PORTS)))
            .unwrap_or_default();
        let mut out: Vec<Option<RawPdPower>> = infos
            .into_iter()
            .map(|r| {
                r.ok().map(|i| RawPdPower {
                    role: match i.role {
                        UsbPowerRoles::Disconnected => PowerRole::None,
                        UsbPowerRoles::Source => PowerRole::Source,
                        UsbPowerRoles::Sink | UsbPowerRoles::SinkNotCharging => PowerRole::Sink,
                    },
                    voltage_now_mv: { i.meas.voltage_now },
                    voltage_max_mv: { i.meas.voltage_max },
                    current_max_ma: { i.meas.current_max },
                    max_power_mw: { i.max_power } / 1000,
                })
            })
            .collect();
        out.resize(PD_PORTS as usize, None);
        out
    }

    pub fn thermal(&self) -> Option<RawThermal> {
        let t = guarded("get_thermal", || power::get_thermal(&self.ec).ok())?;
        Some(RawThermal {
            sensors: t
                .sensors
                .into_iter()
                .map(|s| {
                    let temp = match s.temp {
                        TempSensor::Ok(c) => RawTemp::Celsius(c),
                        other => RawTemp::Error(format!("{other:?}")),
                    };
                    (s.name, temp)
                })
                .collect(),
            fans: t
                .fans
                .into_iter()
                .filter_map(|f| match f.speed {
                    FanSpeed::Rpm(rpm) => Some((f.name, Some(rpm as u32), false)),
                    FanSpeed::Stalled => Some((f.name, None, true)),
                    FanSpeed::NotPresent => None,
                })
                .collect(),
            throttle: t.throttle.map(|t| (t.soft, t.hard)),
        })
    }

    pub fn versions(&self) -> RawVersions {
        let ec = guarded("version_info", || self.ec.version_info().ok());
        let pd = guarded("read_pd_version", || power::read_pd_version(&self.ec).ok())
            .map(|v| {
                use framework_lib::ccgx::MainPdVersions;
                // Same format framework_tool prints (app version; 11th Gen uses base version)
                let gen11 = smbios::get_platform() == Some(Platform::IntelGen11);
                let fmt = |c: &framework_lib::ccgx::ControllerVersion| {
                    if gen11 {
                        c.base.to_dec_string()
                    } else {
                        c.app.to_string()
                    }
                };
                match v {
                    MainPdVersions::Single(c) => vec![fmt(&c)],
                    MainPdVersions::RightLeft((r, l)) => vec![fmt(&r), fmt(&l)],
                    MainPdVersions::Many(v) => v.iter().map(fmt).collect(),
                }
            })
            .unwrap_or_default();
        RawVersions { ec, pd }
    }
}

/// SMBIOS based identification – works without EC driver access.
pub struct SmbiosInfo {
    pub is_framework: bool,
    pub model: Model,
    pub product: Option<String>,
    pub bios_version: Option<String>,
}

pub fn read_smbios() -> SmbiosInfo {
    let platform = guarded("get_platform", smbios::get_platform);
    let is_framework = guarded("is_framework", || Some(smbios::is_framework())).unwrap_or(false);
    let product = guarded("get_product_name", smbios::get_product_name);
    let bios_version = guarded("bios_version", || {
        smbios::get_smbios()?.structures().find_map(|s| match s {
            Ok(dmidecode::Structure::Bios(b)) => Some(b.bios_version.to_string()),
            _ => None,
        })
    });
    SmbiosInfo {
        is_framework,
        model: platform.map(model_from_platform).unwrap_or(Model::Unknown),
        product,
        bios_version,
    }
}

fn model_from_platform(p: Platform) -> Model {
    match p {
        Platform::Framework12IntelGen13 | Platform::Framework12IntelCore3 => Model::Laptop12,
        Platform::IntelGen11 | Platform::IntelGen12 | Platform::IntelGen13 => Model::Laptop13Intel,
        Platform::IntelCoreUltra1 | Platform::IntelCoreUltra3 => Model::Laptop13CoreUltra,
        Platform::Framework13Amd7080 => Model::Laptop13Amd7040,
        Platform::Framework13AmdAi300 => Model::Laptop13AmdAi300,
        Platform::Framework16Amd7080 => Model::Laptop16Amd7040,
        Platform::Framework16AmdAi300 => Model::Laptop16AmdAi300,
        Platform::FrameworkDesktopAmdAiMax300 => Model::Desktop,
        Platform::GenericFramework(..) | Platform::UnknownSystem => Model::Unknown,
    }
}
