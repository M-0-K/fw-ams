//! Notification rules with hysteresis, so each condition fires once until it clears.

use crate::model::{CardKind, ChargeState, PowerRole, Snapshot};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AlertSettings {
    pub battery_low: bool,
    pub battery_low_percent: u8,
    pub charge_limit_reached: bool,
    pub weak_charger: bool,
    pub weak_charger_watts: u32,
    pub throttling: bool,
    pub high_temp: bool,
    pub high_temp_celsius: u8,
    pub card_changes: bool,
}

impl Default for AlertSettings {
    fn default() -> Self {
        Self {
            battery_low: true,
            battery_low_percent: 15,
            charge_limit_reached: true,
            weak_charger: true,
            weak_charger_watts: 45,
            throttling: true,
            high_temp: true,
            high_temp_celsius: 95,
            card_changes: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub title: String,
    pub body: String,
}

/// Re-arm margins
const SOC_HYSTERESIS: f32 = 5.0;
const TEMP_HYSTERESIS: f32 = 5.0;

#[derive(Debug, Default)]
pub struct AlertState {
    battery_low_fired: bool,
    limit_fired: bool,
    weak_charger_fired: bool,
    throttle_fired: bool,
    temp_fired: bool,
    last_cards: Option<Vec<CardKind>>,
}

impl AlertState {
    pub fn evaluate(&mut self, s: &Snapshot, cfg: &AlertSettings) -> Vec<Alert> {
        let mut out = vec![];
        let mut alert = |title: &str, body: String| {
            out.push(Alert {
                title: title.into(),
                body,
            })
        };

        if let Some(b) = &s.battery {
            let low = cfg.battery_low_percent as f32;
            if b.state == ChargeState::Discharging && b.soc_percent <= low {
                if !self.battery_low_fired && cfg.battery_low {
                    alert(
                        "Akku fast leer",
                        format!("Noch {:.0} % – Netzteil anschließen.", b.soc_percent),
                    );
                }
                self.battery_low_fired = true;
            } else if b.soc_percent > low + SOC_HYSTERESIS || b.state == ChargeState::Charging {
                self.battery_low_fired = false;
            }

            if let Some(limit) = b.charge_limit_percent.filter(|l| *l < 100) {
                let limit = limit as f32;
                if b.state == ChargeState::Idle && s.charger.ac_present && b.soc_percent >= limit - 1.0 {
                    if !self.limit_fired && cfg.charge_limit_reached {
                        alert(
                            "Ladelimit erreicht",
                            format!("Akku bei {:.0} % – Ladung pausiert.", b.soc_percent),
                        );
                    }
                    self.limit_fired = true;
                } else if b.soc_percent < limit - SOC_HYSTERESIS {
                    self.limit_fired = false;
                }
            }
        }

        let charger = s.ports.iter().find(|p| p.power_role == PowerRole::Sink);
        match charger {
            Some(p) if p.power_w > 0.0 && p.power_w < cfg.weak_charger_watts as f32 => {
                if !self.weak_charger_fired && cfg.weak_charger {
                    alert(
                        "Schwaches Ladegerät",
                        format!(
                            "{} liefert nur {:.0} W ({:.0} V × {:.2} A).",
                            p.location.label, p.power_w, p.voltage_v, p.current_a
                        ),
                    );
                }
                self.weak_charger_fired = true;
            }
            Some(_) => {}
            None => self.weak_charger_fired = false,
        }

        if let Some(t) = &s.thermal {
            let throttled = t.throttle.is_some_and(|t| t.soft || t.hard);
            if throttled {
                if !self.throttle_fired && cfg.throttling {
                    let hard = t.throttle.is_some_and(|t| t.hard);
                    alert(
                        "CPU wird gedrosselt",
                        if hard {
                            "Harte Drosselung (PROCHOT) aktiv.".into()
                        } else {
                            "Leistungsbegrenzung durch den EC aktiv.".into()
                        },
                    );
                }
                self.throttle_fired = true;
            } else {
                self.throttle_fired = false;
            }

            let limit = cfg.high_temp_celsius as f32;
            let hottest = t
                .sensors
                .iter()
                .filter_map(|s| s.celsius.map(|c| (s, c)))
                .max_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((sensor, c)) = hottest {
                if c >= limit {
                    if !self.temp_fired && cfg.high_temp {
                        alert("Hohe Temperatur", format!("{}: {:.0} °C", sensor.label, c));
                    }
                    self.temp_fired = true;
                } else if c < limit - TEMP_HYSTERESIS {
                    self.temp_fired = false;
                }
            }
        }

        let cards: Vec<CardKind> = s.slots.iter().map(|s| s.card.kind).collect();
        if let Some(prev) = &self.last_cards {
            if cfg.card_changes && prev.len() == cards.len() {
                for ((slot, old), new) in s.slots.iter().zip(prev).zip(&cards) {
                    if old != new {
                        alert(
                            "Expansion Card geändert",
                            format!(
                                "Slot {} ({}): {}",
                                slot.number, slot.location.label, slot.card.name
                            ),
                        );
                    }
                }
            }
        }
        self.last_cards = Some(cards);

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock;
    use crate::model::ChargeState;

    #[test]
    fn battery_low_fires_once_and_rearms() {
        let cfg = AlertSettings::default();
        let mut st = AlertState::default();
        let mut s = mock::snapshot();
        let b = s.battery.as_mut().unwrap();
        b.state = ChargeState::Discharging;
        b.soc_percent = 14.0;
        assert_eq!(st.evaluate(&s, &cfg).len(), 1);
        assert!(st.evaluate(&s, &cfg).is_empty());

        s.battery.as_mut().unwrap().soc_percent = 12.0;
        assert!(st.evaluate(&s, &cfg).is_empty());

        s.battery.as_mut().unwrap().soc_percent = 25.0;
        st.evaluate(&s, &cfg);
        s.battery.as_mut().unwrap().soc_percent = 14.0;
        assert_eq!(st.evaluate(&s, &cfg).len(), 1);
    }

    #[test]
    fn charge_limit_reached() {
        let cfg = AlertSettings::default();
        let mut st = AlertState::default();
        let mut s = mock::snapshot();
        let b = s.battery.as_mut().unwrap();
        b.state = ChargeState::Idle;
        b.soc_percent = 80.0;
        let alerts = st.evaluate(&s, &cfg);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].title, "Ladelimit erreicht");
    }

    #[test]
    fn weak_charger_and_card_change() {
        let cfg = AlertSettings {
            weak_charger_watts: 100,
            card_changes: true,
            ..Default::default()
        };
        let mut st = AlertState::default();
        let mut s = mock::snapshot();
        let first = st.evaluate(&s, &cfg);
        assert!(first.iter().any(|a| a.title == "Schwaches Ladegerät"));

        s.slots[3].card.kind = CardKind::EmptyOrPassive;
        let second = st.evaluate(&s, &cfg);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].title, "Expansion Card geändert");
    }

    #[test]
    fn mock_snapshot_is_quiet_with_defaults() {
        let mut st = AlertState::default();
        assert!(st.evaluate(&mock::snapshot(), &AlertSettings::default()).is_empty());
    }
}
