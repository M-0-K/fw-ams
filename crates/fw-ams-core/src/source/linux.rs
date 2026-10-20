//! Linux fallbacks without EC access: power_supply class and the cros_ec hwmon driver.

use crate::model::{Battery, ChargeState, DataSource, Fan, TempSensor, Thermal};
use std::fs;
use std::path::{Path, PathBuf};

const POWER_SUPPLY: &str = "/sys/class/power_supply";
const HWMON: &str = "/sys/class/hwmon";
pub const CROS_EC_DEV: &str = "/dev/cros_ec";

fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_owned())
}

fn read_num(path: &Path) -> Option<i64> {
    read(path)?.parse().ok()
}

fn supplies_of_type(kind: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(POWER_SUPPLY) else {
        return vec![];
    };
    entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| read(&p.join("type")).as_deref() == Some(kind))
        .collect()
}

pub fn ac_online() -> Option<bool> {
    let mains = supplies_of_type("Mains");
    if mains.is_empty() {
        return None;
    }
    Some(mains.iter().any(|p| read_num(&p.join("online")) == Some(1)))
}

pub fn battery() -> Option<Battery> {
    let bat = supplies_of_type("Battery").into_iter().next()?;
    let soc = read_num(&bat.join("capacity"))? as f32;
    let state = match read(&bat.join("status")).as_deref() {
        Some("Charging") => ChargeState::Charging,
        Some("Discharging") => ChargeState::Discharging,
        Some("Full") | Some("Not charging") => ChargeState::Idle,
        _ => ChargeState::Unknown,
    };
    let sign = if state == ChargeState::Discharging { -1.0 } else { 1.0 };

    // Values are µV, µA, µW, µAh
    let voltage_v = read_num(&bat.join("voltage_now")).map(|v| v as f32 / 1e6);
    let current_a = read_num(&bat.join("current_now")).map(|v| sign * (v.abs() as f32) / 1e6);
    let power_w = read_num(&bat.join("power_now"))
        .map(|v| sign * (v.abs() as f32) / 1e6)
        .or_else(|| Some(voltage_v? * current_a?));
    let mah = |name: &str| read_num(&bat.join(name)).map(|v| (v / 1000) as u32);
    let remaining_mah = mah("charge_now");
    let full_mah = mah("charge_full");
    let design_mah = mah("charge_full_design");

    Some(Battery {
        soc_percent: soc,
        state,
        critical: read(&bat.join("capacity_level")).as_deref() == Some("Critical"),
        voltage_v,
        current_a,
        power_w,
        remaining_mah,
        full_mah,
        design_mah,
        health_percent: match (full_mah, design_mah) {
            (Some(f), Some(d)) if d > 0 => Some(100.0 * f as f32 / d as f32),
            _ => None,
        },
        cycle_count: read_num(&bat.join("cycle_count")).map(|v| v as u32),
        charge_limit_percent: read_num(&bat.join("charge_control_end_threshold"))
            .map(|v| v as u8),
        charge_limit_min_percent: read_num(&bat.join("charge_control_start_threshold"))
            .map(|v| v as u8),
        minutes_remaining: None,
        temperature_c: read_num(&bat.join("temp")).map(|v| v as f32 / 10.0),
        manufacturer: read(&bat.join("manufacturer")),
        model: read(&bat.join("model_name")),
        source: DataSource::Sysfs,
    })
}

/// Temperatures and fans from the `cros_ec` hwmon driver (Linux >= 6.11)
pub fn thermal() -> Option<Thermal> {
    let hwmon = fs::read_dir(HWMON)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| read(&p.join("name")).as_deref() == Some("cros_ec"))?;

    let mut sensors = vec![];
    let mut fans = vec![];
    for i in 1..=16 {
        if let Some(milli) = read_num(&hwmon.join(format!("temp{i}_input"))) {
            let name = read(&hwmon.join(format!("temp{i}_label"))).unwrap_or(format!("temp{i}"));
            sensors.push(TempSensor {
                label: crate::collector::sensor_label(&name),
                name,
                celsius: Some(milli as f32 / 1000.0),
                error: None,
            });
        }
        if let Some(rpm) = read_num(&hwmon.join(format!("fan{i}_input"))) {
            fans.push(Fan {
                name: read(&hwmon.join(format!("fan{i}_label"))).unwrap_or(format!("Lüfter {i}")),
                rpm: Some(rpm as u32),
                stalled: read_num(&hwmon.join(format!("fan{i}_fault"))) == Some(1),
            });
        }
    }
    Some(Thermal {
        sensors,
        fans,
        throttle: None,
        source: DataSource::Sysfs,
    })
}

/// Explains why the EC is not reachable.
pub fn access_hint() -> String {
    let dev = Path::new(CROS_EC_DEV);
    if !dev.exists() {
        "Kein /dev/cros_ec gefunden. Kernel-Modul cros_ec_lpcs laden (Linux ≥ 5.19) – \
         bis dahin werden sysfs-Werte angezeigt."
            .into()
    } else {
        "Kein Zugriff auf /dev/cros_ec. udev-Regel 99-fw-ams.rules installieren \
         (TAG+=\"uaccess\") und neu anmelden – bis dahin werden sysfs-Werte angezeigt."
            .into()
    }
}
