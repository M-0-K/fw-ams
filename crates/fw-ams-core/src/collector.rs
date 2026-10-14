//! Builds a [`Snapshot`] from all available sources.
//!
//! The mapping functions (`build_*`) are pure so they can be tested with recorded raw data.

use crate::cards::{detect, slots};
use crate::model::*;
use crate::source::ec::{
    self, EcSource, RawPdPower, RawPdState, RawPower, RawTemp, RawThermal, RawVersions, SmbiosInfo,
};
use crate::source::usb;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Collector {
    ec: Option<EcSource>,
    smbios: SmbiosInfo,
    versions: RawVersions,
    layout: Vec<slots::SlotDef>,
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}

impl Collector {
    pub fn new() -> Self {
        let smbios = ec::read_smbios();
        let layout = slots::layout(smbios.model);
        let mut c = Self {
            ec: None,
            smbios,
            versions: RawVersions::default(),
            layout,
        };
        c.retry_ec();
        c
    }

    pub fn ec_available(&self) -> bool {
        self.ec.is_some()
    }

    /// Try to (re)open the EC, e.g. after the user installed the driver or udev rule.
    pub fn retry_ec(&mut self) -> bool {
        if self.ec.is_none() {
            self.ec = EcSource::open();
            if let Some(ec) = &self.ec {
                self.versions = ec.versions();
                log::info!("EC available: {:?}", self.versions.ec);
            }
        }
        self.ec.is_some()
    }

    pub fn snapshot(&self) -> Snapshot {
        let usb = usb::list();
        let model = self.smbios.model;
        let system = SystemInfo {
            model,
            product: self.smbios.product.clone(),
            bios_version: self.smbios.bios_version.clone(),
            ec_version: self.versions.ec.clone(),
            pd_versions: self.versions.pd.clone(),
        };

        let Some(ec) = &self.ec else {
            return self.fallback_snapshot(system, &usb);
        };

        let power = ec.power();
        let limit = ec.charge_limit();
        let ports = build_ports(model, &ec.pd_states(), &ec.pd_power());
        let battery = power
            .as_ref()
            .and_then(|p| build_battery(p, limit))
            .map(|b| with_os_temperature(b));
        let charger = build_charger(power.as_ref().is_some_and(|p| p.ac_present), &ports);
        let thermal = ec.thermal().map(build_thermal);
        let slots = detect::detect(model, &self.layout, &ports, &usb);

        Snapshot {
            timestamp_ms: now_ms(),
            system,
            access: AccessStatus {
                is_framework: self.smbios.is_framework,
                ec: EcState::Ok,
                hint: None,
            },
            battery,
            charger,
            ports,
            thermal,
            slots,
        }
    }

    fn fallback_snapshot(&self, system: SystemInfo, usb: &[UsbDevice]) -> Snapshot {
        #[cfg(target_os = "linux")]
        let (battery, ac, thermal, hint) = {
            use crate::source::linux;
            let battery = linux::battery();
            let ac = linux::ac_online().unwrap_or(false);
            (battery, ac, linux::thermal(), linux::access_hint())
        };
        #[cfg(not(target_os = "linux"))]
        let (battery, ac, thermal, hint) = {
            let battery = crate::source::os_battery::battery();
            let ac = battery
                .as_ref()
                .is_some_and(|b| matches!(b.state, ChargeState::Charging | ChargeState::Idle));
            let hint = "Kein Zugriff auf den Framework-EC. Aktuelles BIOS und das Framework \
                        Driver Bundle (enthält den EC-Treiber) installieren – bis dahin werden \
                        nur Windows-Akkuwerte angezeigt."
                .to_string();
            (battery, ac, None::<Thermal>, hint)
        };

        let hint = if self.smbios.is_framework {
            hint
        } else {
            "Kein Framework-Gerät erkannt – nur allgemeine Akkuwerte verfügbar.".to_string()
        };

        Snapshot {
            timestamp_ms: now_ms(),
            system,
            access: AccessStatus {
                is_framework: self.smbios.is_framework,
                ec: EcState::Unavailable,
                hint: Some(hint),
            },
            battery,
            charger: Charger {
                ac_present: ac,
                ..Charger::default()
            },
            ports: vec![],
            thermal,
            slots: detect::detect(self.smbios.model, &self.layout, &[], usb),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The EC does not expose the battery temperature; take it from the OS when available.
fn with_os_temperature(mut b: Battery) -> Battery {
    #[cfg(target_os = "linux")]
    let t = crate::source::linux::battery().and_then(|b| b.temperature_c);
    #[cfg(not(target_os = "linux"))]
    let t = crate::source::os_battery::battery().and_then(|b| b.temperature_c);
    b.temperature_c = t;
    b
}

pub fn build_battery(power: &RawPower, charge_limit: Option<u8>) -> Option<Battery> {
    let b = power.battery.as_ref()?;
    let state = if b.charging {
        ChargeState::Charging
    } else if b.discharging {
        ChargeState::Discharging
    } else if power.ac_present {
        ChargeState::Idle
    } else {
        ChargeState::Unknown
    };
    let sign = match state {
        ChargeState::Discharging => -1.0,
        ChargeState::Charging => 1.0,
        _ => 0.0,
    };
    let voltage = b.voltage_mv as f32 / 1000.0;
    let current = sign * b.rate_ma as f32 / 1000.0;
    let soc = if b.full_mah > 0 {
        (100.0 * b.remaining_mah as f32 / b.full_mah as f32).min(100.0)
    } else {
        0.0
    };
    let minutes_remaining = match state {
        ChargeState::Discharging if b.rate_ma > 0 => {
            Some((b.remaining_mah as f32 / b.rate_ma as f32 * 60.0) as u32)
        }
        ChargeState::Charging if b.rate_ma > 0 => {
            let target = b.full_mah * charge_limit.unwrap_or(100).min(100) as u32 / 100;
            target
                .checked_sub(b.remaining_mah)
                .map(|missing| (missing as f32 / b.rate_ma as f32 * 60.0) as u32)
        }
        _ => None,
    };

    Some(Battery {
        soc_percent: soc,
        state,
        critical: b.critical,
        voltage_v: Some(voltage),
        current_a: Some(current),
        power_w: Some(voltage * current),
        remaining_mah: Some(b.remaining_mah),
        full_mah: Some(b.full_mah),
        design_mah: Some(b.design_mah),
        health_percent: (b.design_mah > 0)
            .then(|| 100.0 * b.full_mah as f32 / b.design_mah as f32),
        cycle_count: Some(b.cycle_count),
        charge_limit_percent: charge_limit,
        minutes_remaining,
        temperature_c: None,
        manufacturer: Some(b.manufacturer.clone()).filter(|s| !s.is_empty()),
        model: Some(b.model.clone()).filter(|s| !s.is_empty()),
        source: DataSource::Ec,
    })
}

pub fn build_ports(
    model: Model,
    states: &[Option<RawPdState>],
    powers: &[Option<RawPdPower>],
) -> Vec<PdPort> {
    (0..ec::PD_PORTS)
        .filter_map(|i| {
            let state = states.get(i as usize).copied().flatten();
            let power = powers.get(i as usize).copied().flatten();
            build_port(model, i, state, power)
        })
        .collect()
}

fn build_port(model: Model, index: u8, state: Option<RawPdState>, power: Option<RawPdPower>) -> Option<PdPort> {
    let location = slots::pd_port_location(model, index);
    let max_power_w = power
        .filter(|p| p.role == PowerRole::Sink && p.max_power_mw > 0)
        .map(|p| p.max_power_mw as f32 / 1000.0);

    if let Some(s) = state {
        let partner = s.partner();
        let connected = partner != Partner::Nothing;
        let power_role = match (connected, s.power_role) {
            (false, _) => PowerRole::None,
            (true, 0) => PowerRole::Sink,
            (true, 1) => PowerRole::Source,
            _ => PowerRole::None,
        };
        let data_role = match (connected, s.data_role) {
            (true, 0) => Some(DataRole::Device),
            (true, 1) => Some(DataRole::Host),
            _ => None,
        };
        let (v, a) = if connected {
            (s.voltage_mv as f32 / 1000.0, s.current_ma as f32 / 1000.0)
        } else {
            (0.0, 0.0)
        };
        return Some(PdPort {
            index,
            location,
            partner,
            power_role,
            data_role,
            pd_contract: s.pd_contract,
            voltage_v: v,
            current_a: a,
            power_w: v * a,
            max_power_w,
            vconn: connected && s.vconn,
            epr_active: s.epr_active,
            epr_supported: s.epr_supported,
            dp_alt_mode: connected && s.alt_mode & 0x03 != 0,
            alt_mode_raw: s.alt_mode,
            active_charger: power_role == PowerRole::Sink && s.active_port,
        });
    }

    // Older ECs without the Framework PD state command: use the generic power info
    let p = power?;
    let v = p.voltage_now_mv.max(p.voltage_max_mv) as f32 / 1000.0;
    let a = if p.role == PowerRole::Sink { p.current_max_ma as f32 / 1000.0 } else { 0.0 };
    Some(PdPort {
        index,
        location,
        partner: match p.role {
            PowerRole::None => Partner::Nothing,
            PowerRole::Source => Partner::Sink,
            PowerRole::Sink => Partner::Source,
        },
        power_role: p.role,
        data_role: None,
        pd_contract: max_power_w.is_some(),
        voltage_v: v,
        current_a: a,
        power_w: v * a,
        max_power_w,
        vconn: false,
        epr_active: false,
        epr_supported: false,
        dp_alt_mode: false,
        alt_mode_raw: 0,
        active_charger: p.role == PowerRole::Sink,
    })
}

pub fn build_charger(ac_present: bool, ports: &[PdPort]) -> Charger {
    let port = ports
        .iter()
        .find(|p| p.active_charger)
        .or_else(|| ports.iter().find(|p| p.power_role == PowerRole::Sink));
    Charger {
        ac_present,
        port: port.map(|p| p.index),
        voltage_v: port.map(|p| p.voltage_v),
        current_a: port.map(|p| p.current_a),
        contract_w: port.map(|p| p.power_w),
        epr: port.is_some_and(|p| p.epr_active),
    }
}

pub fn build_thermal(raw: RawThermal) -> Thermal {
    Thermal {
        sensors: raw
            .sensors
            .into_iter()
            .map(|(name, temp)| {
                let (celsius, error) = match temp {
                    RawTemp::Celsius(c) => (Some(c as f32), None),
                    RawTemp::Error(e) => (None, Some(e)),
                };
                TempSensor {
                    label: sensor_label(&name),
                    name,
                    celsius,
                    error,
                }
            })
            .collect(),
        fans: raw
            .fans
            .into_iter()
            .map(|(name, rpm, stalled)| Fan {
                name: fan_label(&name),
                rpm,
                stalled,
            })
            .collect(),
        throttle: raw.throttle.map(|(soft, hard)| Throttle { soft, hard }),
        source: DataSource::Ec,
    }
}

/// Friendly German names for the EC's sensor names (e.g. "cpu_f75303@4d")
pub fn sensor_label(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    let label = if n.starts_with("cpu@") || n.contains("peci") || n == "cpu" {
        "CPU"
    } else if n.starts_with("cpu_") {
        "CPU-Umgebung"
    } else if n.contains("ddr") || n.contains("mem") {
        "Arbeitsspeicher"
    } else if n.contains("local") {
        "Mainboard"
    } else if n.contains("batt") {
        "Akku"
    } else if n.contains("gpu") {
        "GPU"
    } else if n.contains("apu") {
        "APU"
    } else if n.contains("charger") {
        "Ladeschaltung"
    } else {
        return name.to_owned();
    };
    label.to_owned()
}

fn fan_label(name: &str) -> String {
    match name {
        "APU Fan" => "APU-Lüfter".into(),
        "Left Fan" => "Lüfter links".into(),
        "Right Fan" => "Lüfter rechts".into(),
        "Front Fan" => "Lüfter vorne".into(),
        other => other.replace("Fan", "Lüfter"),
    }
}
