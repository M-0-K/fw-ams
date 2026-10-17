//! Generic OS battery API (Windows/macOS) as fallback when the EC is not reachable.

use crate::model::{Battery, ChargeState, DataSource};
use starship_battery::units::{
    electric_potential::volt, energy::watt_hour, power::watt, ratio::percent,
    thermodynamic_temperature::degree_celsius, time::minute,
};
use starship_battery::{Manager, State};

pub fn battery() -> Option<Battery> {
    let manager = Manager::new().ok()?;
    let bat = manager.batteries().ok()?.flatten().next()?;

    let state = match bat.state() {
        State::Charging => ChargeState::Charging,
        State::Discharging => ChargeState::Discharging,
        State::Full => ChargeState::Idle,
        _ => ChargeState::Unknown,
    };
    let sign = if state == ChargeState::Discharging { -1.0 } else { 1.0 };
    let voltage = bat.voltage().get::<volt>();
    let power = sign * bat.energy_rate().get::<watt>().abs();
    let minutes = match state {
        ChargeState::Discharging => bat.time_to_empty(),
        ChargeState::Charging => bat.time_to_full(),
        _ => None,
    }
    .map(|t| t.get::<minute>() as u32);

    // The OS reports energy (Wh); convert to mAh with the present voltage
    let to_mah = |wh: f32| (voltage > 0.0).then(|| (wh / voltage * 1000.0) as u32);

    Some(Battery {
        soc_percent: bat.state_of_charge().get::<percent>(),
        state,
        critical: false,
        voltage_v: Some(voltage),
        current_a: (voltage > 0.0).then(|| power / voltage),
        power_w: Some(power),
        remaining_mah: to_mah(bat.energy().get::<watt_hour>()),
        full_mah: to_mah(bat.energy_full().get::<watt_hour>()),
        design_mah: to_mah(bat.energy_full_design().get::<watt_hour>()),
        health_percent: Some(bat.state_of_health().get::<percent>()),
        cycle_count: bat.cycle_count(),
        charge_limit_percent: None,
        charge_limit_min_percent: None,
        minutes_remaining: minutes,
        temperature_c: bat.temperature().map(|t| t.get::<degree_celsius>()),
        manufacturer: bat.vendor().map(str::to_owned),
        model: bat.model().map(str::to_owned),
        source: DataSource::Os,
    })
}
