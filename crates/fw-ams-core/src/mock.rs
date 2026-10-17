//! Mock snapshot built from raw data recorded on a Framework Laptop 13 (AMD Ryzen AI 300):
//! HDMI card right front, USB-A card left front, 65 W charger via USB-C card left back,
//! right back empty. Used for UI development (`--mock`) and as test fixture.

use crate::cards::{detect, slots};
use crate::collector::{build_battery, build_charger, build_ports, build_thermal};
use crate::model::*;
use crate::source::ec::{RawBattery, RawPdPower, RawPdState, RawPower, RawTemp, RawThermal};

pub fn raw_power() -> RawPower {
    RawPower {
        ac_present: true,
        battery: Some(RawBattery {
            voltage_mv: 16495,
            rate_ma: 2074,
            remaining_mah: 1518,
            full_mah: 3647,
            design_mah: 3915,
            cycle_count: 194,
            charging: true,
            discharging: false,
            critical: false,
            manufacturer: "NVT".into(),
            model: "FRANGWA".into(),
        }),
    }
}

pub fn raw_pd_states() -> Vec<Option<RawPdState>> {
    vec![
        Some(RawPdState::default()),
        // HDMI card: partner sink, PD contract, VCONN on, laptop is host
        Some(RawPdState {
            c_state: 1,
            pd_contract: true,
            power_role: 1,
            data_role: 1,
            vconn: true,
            voltage_mv: 5000,
            current_ma: 680,
            ..Default::default()
        }),
        // USB-A card: 5 V without PD contract
        Some(RawPdState {
            c_state: 1,
            power_role: 1,
            data_role: 1,
            voltage_mv: 5000,
            current_ma: 1500,
            ..Default::default()
        }),
        // 65 W charger
        Some(RawPdState {
            c_state: 2,
            pd_contract: true,
            power_role: 0,
            data_role: 0,
            voltage_mv: 20000,
            current_ma: 3250,
            active_port: true,
            alt_mode: 0x80,
            ..Default::default()
        }),
    ]
}

pub fn raw_pd_power() -> Vec<Option<RawPdPower>> {
    let none = RawPdPower {
        role: PowerRole::None,
        voltage_now_mv: 0,
        voltage_max_mv: 0,
        current_max_ma: 1500,
        max_power_mw: 0,
    };
    let source = RawPdPower {
        role: PowerRole::Source,
        voltage_now_mv: 5000,
        ..none
    };
    vec![
        Some(none),
        Some(source),
        Some(source),
        Some(RawPdPower {
            role: PowerRole::Sink,
            voltage_now_mv: 0,
            voltage_max_mv: 20000,
            current_max_ma: 3250,
            max_power_mw: 65000,
        }),
    ]
}

pub fn raw_thermal() -> RawThermal {
    RawThermal {
        sensors: vec![
            ("local_f75303@4d".into(), RawTemp::Celsius(55)),
            ("cpu_f75303@4d".into(), RawTemp::Celsius(62)),
            ("ddr_f75303@4d".into(), RawTemp::Celsius(40)),
            ("cpu@4c".into(), RawTemp::Celsius(74)),
        ],
        fans: vec![("APU Fan".into(), Some(4703), false)],
        throttle: Some((false, false)),
    }
}

pub fn usb_devices() -> Vec<UsbDevice> {
    let dev = |vid: u16, pid: u16, product: &str, speed, root: &str| UsbDevice {
        vid,
        pid,
        id: format!("{vid:04x}:{pid:04x}"),
        product: Some(product.into()),
        speed,
        usb_version: "2.01".into(),
        root_port: Some(root.into()),
        port_chain: vec![],
    };
    vec![
        dev(0x32ac, 0x001c, "Laptop Webcam Module (2nd Gen)", UsbSpeed::High, "XHC0.PRT3"),
        dev(0x27c6, 0x609c, "Goodix Fingerprint USB Device", UsbSpeed::Full, "XHC1.PRT1"),
        dev(0x0e8d, 0x0717, "Wireless_Device", UsbSpeed::High, "XHC0.PRT5"),
        dev(0x32ac, 0x0002, "HDMI Expansion Card", UsbSpeed::Full, "XHC0.PRT2"),
    ]
}

pub fn snapshot() -> Snapshot {
    let model = Model::Laptop13AmdAi300;
    let power = raw_power();
    let ports = build_ports(model, &raw_pd_states(), &raw_pd_power());
    let layout = slots::layout(model);
    let mut battery = build_battery(&power, Some((0, 80)));
    if let Some(b) = &mut battery {
        b.source = DataSource::Mock;
    }
    let mut thermal = build_thermal(raw_thermal());
    thermal.source = DataSource::Mock;

    Snapshot {
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        system: SystemInfo {
            model,
            product: Some("Laptop 13 (AMD Ryzen AI 300 Series)".into()),
            bios_version: Some("03.05".into()),
            ec_version: Some("lilac-3.0.5-9010bdf 2025-10-30".into()),
            pd_versions: vec!["0.0.21".into(), "0.0.21".into()],
        },
        access: AccessStatus {
            is_framework: true,
            ec: EcState::Mock,
            hint: Some("Mock-Daten (aufgezeichnet auf einem Framework Laptop 13 AMD AI 300)".into()),
        },
        charger: build_charger(power.ac_present, &ports),
        battery,
        slots: detect::detect(model, &layout, &ports, &usb_devices()),
        ports,
        thermal: Some(thermal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(s: &Snapshot, side: Side, pos: Position) -> &Slot {
        s.slots
            .iter()
            .find(|sl| sl.location.side == side && sl.location.position == pos)
            .unwrap()
    }

    #[test]
    fn detects_recorded_card_layout() {
        let s = snapshot();
        assert_eq!(s.slots.len(), 4);

        let rf = slot(&s, Side::Right, Position::Front);
        assert_eq!(rf.pd_port, Some(1));
        assert_eq!(rf.card.kind, CardKind::Hdmi);
        assert_eq!(rf.card.confidence, Confidence::Certain);
        assert!(rf.hints.iter().any(|h| h.message.contains("Kein Monitor")));

        let lf = slot(&s, Side::Left, Position::Front);
        assert_eq!(lf.card.kind, CardKind::UsbA);

        let lb = slot(&s, Side::Left, Position::Back);
        assert_eq!(lb.card.kind, CardKind::UsbC);
        assert!(lb.protocols.iter().any(|p| p.contains("65.0 W")));

        let rb = slot(&s, Side::Right, Position::Back);
        assert_eq!(rb.card.kind, CardKind::EmptyOrPassive);
    }

    #[test]
    fn internal_devices_are_not_assigned_to_slots() {
        let s = snapshot();
        let all: Vec<&str> = s
            .slots
            .iter()
            .flat_map(|sl| sl.devices.iter().map(|d| d.id.as_str()))
            .collect();
        assert_eq!(all, vec!["32ac:0002"]);
    }

    #[test]
    fn battery_values() {
        let b = snapshot().battery.unwrap();
        assert_eq!(b.state, ChargeState::Charging);
        assert!((b.soc_percent - 41.6).abs() < 0.1);
        assert!((b.power_w.unwrap() - 34.2).abs() < 0.1);
        assert!((b.health_percent.unwrap() - 93.2).abs() < 0.1);
        // to 80 % limit: 2917 - 1518 = 1399 mAh at 2074 mA ≈ 40 min
        assert_eq!(b.minutes_remaining, Some(40));
    }

    #[test]
    fn charger_from_active_sink_port() {
        let c = snapshot().charger;
        assert!(c.ac_present);
        assert_eq!(c.port, Some(3));
        assert_eq!(c.contract_w, Some(65.0));
    }

    #[test]
    fn uncalibrated_card_is_placed_by_pd_state() {
        let model = Model::Laptop13AmdAi300;
        let ports = build_ports(model, &raw_pd_states(), &raw_pd_power());
        let mut usb = usb_devices();
        usb.last_mut().unwrap().root_port = Some("XHC1.PRT9".into());
        let slots = detect::detect(model, &slots::layout(model), &ports, &usb);
        let rf = slots.iter().find(|s| s.pd_port == Some(1)).unwrap();
        assert_eq!(rf.card.kind, CardKind::Hdmi);
        assert_eq!(rf.card.confidence, Confidence::Likely);
    }

    #[test]
    fn weak_charger_warning() {
        let model = Model::Laptop13AmdAi300;
        let mut states = raw_pd_states();
        if let Some(s) = states[3].as_mut() {
            s.voltage_mv = 15000;
            s.current_ma = 2000;
        }
        let ports = build_ports(model, &states, &raw_pd_power());
        let slots = detect::detect(model, &slots::layout(model), &ports, &[]);
        let lb = slots.iter().find(|s| s.pd_port == Some(3)).unwrap();
        assert!(lb.hints.iter().any(|h| h.severity == Severity::Warning));
    }
}
