//! Phase-0 spike: dump raw EC + USB data as JSON to verify what the hardware delivers.

use framework_lib::chromium_ec::EcRequestRaw;
use framework_lib::chromium_ec::commands::EcRequestGetPdPortState;
use framework_lib::chromium_ec::CrosEc;
use framework_lib::power::{self, FanSpeed, TempSensor};
use framework_lib::smbios;
use nusb::MaybeFuture;
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn guarded<T>(name: &str, f: impl FnOnce() -> T) -> Option<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(v) => Some(v),
        Err(_) => {
            eprintln!("[spike] {name} panicked");
            None
        }
    }
}

pub fn run() {
    let ec = CrosEc::new();
    let mut out = serde_json::Map::new();

    out.insert(
        "smbios".into(),
        json!({
            "is_framework": guarded("is_framework", smbios::is_framework),
            "product": guarded("product", smbios::get_product_name).flatten(),
            "platform": guarded("platform", || format!("{:?}", smbios::get_platform())),
            "family": guarded("family", || format!("{:?}", smbios::get_family())),
        }),
    );

    out.insert(
        "ec_version".into(),
        guarded("version", || format!("{:?}", ec.version_info())).into(),
    );

    out.insert(
        "power_info".into(),
        guarded("power_info", || power::power_info(&ec))
            .flatten()
            .map(|p| {
                json!({
                    "ac_present": p.ac_present,
                    "battery": p.battery.map(|b| json!({
                        "present_voltage": b.present_voltage,
                        "present_rate": b.present_rate,
                        "remaining_capacity": b.remaining_capacity,
                        "design_capacity": b.design_capacity,
                        "design_voltage": b.design_voltage,
                        "lfcc": b.last_full_charge_capacity,
                        "cycle_count": b.cycle_count,
                        "charge_percentage": b.charge_percentage,
                        "manufacturer": b.manufacturer,
                        "model": b.model_number,
                        "type": b.battery_type,
                        "charging": b.charging,
                        "discharging": b.discharging,
                        "critical": b.level_critical,
                    })),
                })
            })
            .unwrap_or(Value::Null),
    );

    out.insert(
        "charge_limit".into(),
        guarded("charge_limit", || format!("{:?}", ec.get_charge_limit())).into(),
    );

    out.insert(
        "pd_power_info".into(),
        guarded("pd_info", || {
            power::get_pd_info(&ec, 4)
                .into_iter()
                .map(|r| match r {
                    Ok(i) => {
                        // Fields of packed structs must be copied before referencing.
                        let (v_now, v_max) = ({ i.meas.voltage_now }, { i.meas.voltage_max });
                        let (c_lim, c_max) = ({ i.meas.current_lim }, { i.meas.current_max });
                        let max_power = { i.max_power };
                        json!({
                            "role": format!("{:?}", i.role),
                            "charging_type": format!("{:?}", i.charging_type),
                            "dualrole": i.dualrole,
                            "voltage_now_mv": v_now,
                            "voltage_max_mv": v_max,
                            "current_lim_ma": c_lim,
                            "current_max_ma": c_max,
                            "max_power_uw": max_power,
                        })
                    }
                    Err(e) => json!({ "error": format!("{e:?}") }),
                })
                .collect::<Vec<_>>()
        })
        .into(),
    );

    out.insert(
        "pd_port_state".into(),
        guarded("pd_port_state", || {
            (0..4u8)
                .map(|port| match (EcRequestGetPdPortState { port }).send_command(&ec) {
                    Ok(s) => {
                        let (voltage, current) = ({ s.voltage }, { s.current });
                        json!({
                            "c_state": s.c_state,
                            "pd_state": s.pd_state,
                            "power_role": s.power_role,
                            "data_role": s.data_role,
                            "vconn": s.vconn,
                            "epr_active": s.epr_active,
                            "epr_support": s.epr_support,
                            "cc_polarity": s.cc_polarity,
                            "voltage_mv": voltage,
                            "current_ma": current,
                            "active_port": s.active_port,
                            "alt_mode": format!("0x{:02X}", s.pd_alt_mode_status),
                        })
                    }
                    Err(e) => json!({ "error": format!("{e:?}") }),
                })
                .collect::<Vec<_>>()
        })
        .into(),
    );

    out.insert(
        "thermal".into(),
        guarded("thermal", || match power::get_thermal(&ec) {
            Ok(t) => json!({
                "sensors": t.sensors.iter().map(|s| json!({
                    "index": s.index,
                    "name": s.name,
                    "temp_c": match s.temp {
                        TempSensor::Ok(c) => json!(c),
                        other => json!(format!("{other:?}")),
                    },
                })).collect::<Vec<_>>(),
                "fans": t.fans.iter().map(|f| json!({
                    "name": f.name,
                    "rpm": match f.speed {
                        FanSpeed::Rpm(r) => json!(r),
                        other => json!(format!("{other:?}")),
                    },
                })).collect::<Vec<_>>(),
                "throttle": t.throttle.map(|th| json!({ "soft": th.soft, "hard": th.hard })),
            }),
            Err(e) => json!({ "error": format!("{e:?}") }),
        })
        .unwrap_or(Value::Null),
    );

    out.insert(
        "pd_versions".into(),
        guarded("pd_versions", || format!("{:?}", power::read_pd_version(&ec))).into(),
    );

    out.insert("usb".into(), usb_devices());

    println!("{}", serde_json::to_string_pretty(&Value::Object(out)).unwrap());
}

fn usb_devices() -> Value {
    match nusb::list_devices().wait() {
        Ok(devs) => devs
            .map(|d| {
                #[allow(unused_mut)]
                let mut v = json!({
                    "id": format!("{:04x}:{:04x}", d.vendor_id(), d.product_id()),
                    "product": d.product_string(),
                    "bus_id": d.bus_id(),
                    "port_chain": d.port_chain(),
                    "speed": format!("{:?}", d.speed()),
                    "class": d.class(),
                    "usb_version": format!("{:04x}", d.usb_version()),
                });
                #[cfg(windows)]
                {
                    v["location_paths"] = json!(d
                        .location_paths()
                        .iter()
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect::<Vec<_>>());
                }
                #[cfg(target_os = "linux")]
                {
                    v["sysfs"] = json!(d.sysfs_path().display().to_string());
                }
                v
            })
            .collect::<Vec<_>>()
            .into(),
        Err(e) => json!({ "error": e.to_string() }),
    }
}
