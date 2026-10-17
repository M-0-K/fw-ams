//! Charge limit and charge current control through the EC (no admin rights needed).
//!
//! Keeps what was sent and compares it with what the EC reports, using the same status
//! model as the CPU tuner so the UI can show both the same way.

use crate::settings::BatterySettings;
use fw_ams_core::model::Snapshot;
use fw_ams_core::Collector;
use fw_ams_tuner::{now_ms, ParamStatus};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChargeParam {
    pub desired: Option<f32>,
    pub sent: Option<f32>,
    pub actual: Option<f32>,
    pub status: ParamStatus,
    pub error: Option<String>,
}

impl ChargeParam {
    fn new() -> Self {
        Self {
            desired: None,
            sent: None,
            actual: None,
            status: ParamStatus::Default,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChargeControl {
    /// EC reachable for writes
    pub available: bool,
    pub max: ChargeParam,
    pub min: ChargeParam,
    /// Charge current limit, mA
    pub current: ChargeParam,
    pub design_mah: Option<u32>,
    /// Unix ms of the last "charge to 100 % once"
    #[ts(type = "number | null")]
    pub full_once_ms: Option<u64>,
}

pub struct ChargeController {
    mock: bool,
    view: ChargeControl,
    sent_limit: Option<(u8, u8)>,
    /// `Some(None)` = limit removed
    sent_current: Option<Option<u32>>,
    last_ac: Option<bool>,
    last_ec: bool,
}

impl ChargeController {
    pub fn new(mock: bool) -> Self {
        Self {
            mock,
            view: ChargeControl {
                available: mock,
                max: ChargeParam::new(),
                min: ChargeParam::new(),
                current: ChargeParam::new(),
                design_mah: None,
                full_once_ms: None,
            },
            sent_limit: None,
            sent_current: None,
            last_ac: None,
            last_ec: false,
        }
    }

    pub fn view(&self) -> ChargeControl {
        self.view.clone()
    }

    fn current_ma(cfg: &BatterySettings, design_mah: Option<u32>) -> Option<u32> {
        (cfg.charge_rate > 0.0)
            .then(|| design_mah.map(|d| (cfg.charge_rate * d as f32).round() as u32))
            .flatten()
    }

    /// Called after every snapshot. Writes when the settings differ from what was sent, after
    /// the EC (re)appeared, and when the charger gets plugged in (the EC may reset values).
    pub fn sync(&mut self, collector: Option<&Collector>, s: &Snapshot, cfg: &BatterySettings) {
        let ec = collector.and_then(|c| c.ec());
        let ec_ok = self.mock || ec.is_some();
        let ac = s.charger.ac_present;
        let replug = self.last_ac == Some(false) && ac;
        let reconnect = ec_ok && !self.last_ec;
        if replug || reconnect {
            self.sent_limit = None;
            self.sent_current = None;
        }
        self.last_ac = Some(ac);
        self.last_ec = ec_ok;
        self.view.available = ec_ok;
        self.view.design_mah = s.battery.as_ref().and_then(|b| b.design_mah);

        // Charge limit
        let want_limit = cfg.manage_charge_limit.then_some((cfg.charge_min, cfg.charge_max));
        self.view.max.desired = want_limit.map(|l| l.1 as f32);
        self.view.min.desired = want_limit.map(|l| l.0 as f32);
        if let Some((min, max)) = want_limit {
            if ec_ok && self.sent_limit != Some((min, max)) {
                let r = match ec {
                    Some(ec) => ec.set_charge_limit(min, max),
                    None => Ok(()),
                };
                self.sent_limit = Some((min, max));
                let at = now_ms();
                for (p, v) in [(&mut self.view.max, max), (&mut self.view.min, min)] {
                    p.sent = Some(v as f32);
                    p.error = r.as_ref().err().cloned();
                    p.status = if r.is_ok() { ParamStatus::Pending } else { ParamStatus::Rejected };
                }
                match &r {
                    Ok(()) => log::info!("charge limit set to {min}..{max} % ({at})"),
                    Err(e) => log::warn!("charge limit: {e}"),
                }
            }
        } else if self.sent_limit.take().is_some() {
            // Management switched off: leave the EC as it is, just stop tracking
            for p in [&mut self.view.max, &mut self.view.min] {
                p.sent = None;
                p.status = ParamStatus::Default;
                p.error = None;
            }
        }
        let b = s.battery.as_ref();
        let actual_max = if self.mock { self.sent_limit.map(|l| l.1).or(Some(80)) } else { b.and_then(|b| b.charge_limit_percent) };
        let actual_min = if self.mock { self.sent_limit.map(|l| l.0).or(Some(0)) } else { b.and_then(|b| b.charge_limit_min_percent) };
        Self::judge(&mut self.view.max, actual_max.map(f32::from));
        Self::judge(&mut self.view.min, actual_min.map(f32::from));

        // Charge current
        let want_current = Self::current_ma(cfg, self.view.design_mah);
        self.view.current.desired = want_current.map(|c| c as f32);
        let need_write = match (self.sent_current, want_current) {
            (None, None) => false,
            (Some(sent), want) => sent != want,
            (None, Some(_)) => true,
        };
        if ec_ok && need_write {
            let r = match ec {
                Some(ec) => ec.set_charge_current_limit(want_current),
                None => Ok(()),
            };
            self.sent_current = Some(want_current);
            let p = &mut self.view.current;
            p.sent = want_current.map(|c| c as f32);
            p.error = r.as_ref().err().cloned();
            p.status = match (&r, want_current) {
                (Err(_), _) => ParamStatus::Rejected,
                (Ok(()), Some(_)) => ParamStatus::Unverified,
                (Ok(()), None) => ParamStatus::Default,
            };
            if want_current.is_none() {
                self.sent_current = None;
            }
        }
        if !ec_ok {
            for p in [&mut self.view.max, &mut self.view.min, &mut self.view.current] {
                if p.desired.is_some() && p.sent.is_none() {
                    p.status = ParamStatus::Pending;
                }
            }
        }
    }

    fn judge(p: &mut ChargeParam, actual: Option<f32>) {
        p.actual = actual;
        if p.status == ParamStatus::Rejected {
            return;
        }
        match (p.sent, actual) {
            (Some(s), Some(a)) if (s - a).abs() < 0.5 => {
                p.status = ParamStatus::Applied;
                p.error = None;
            }
            (Some(_), Some(a)) => {
                p.status = ParamStatus::Limited;
                p.error = Some(format!("EC meldet {a:.0} %"));
            }
            (Some(_), None) => p.status = ParamStatus::Unverified,
            (None, _) => p.status = ParamStatus::Default,
        }
    }

    pub fn full_once(&mut self, collector: Option<&Collector>) -> Result<(), String> {
        if !self.mock {
            collector
                .and_then(|c| c.ec())
                .ok_or("Kein EC-Zugriff")?
                .charge_limit_override()?;
        }
        self.view.full_once_ms = Some(now_ms());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fw_ams_core::mock;

    #[test]
    fn mock_applies_and_reads_back() {
        let mut c = ChargeController::new(true);
        let cfg = BatterySettings {
            manage_charge_limit: true,
            charge_max: 70,
            charge_rate: 0.5,
            ..Default::default()
        };
        c.sync(None, &mock::snapshot(), &cfg);
        let v = c.view();
        assert_eq!(v.max.sent, Some(70.0));
        assert_eq!(v.max.status, ParamStatus::Applied);
        assert_eq!(v.current.sent, Some(1958.0));
        assert_eq!(v.current.status, ParamStatus::Unverified);

        let off = BatterySettings::default();
        c.sync(None, &mock::snapshot(), &off);
        let v = c.view();
        assert_eq!(v.max.status, ParamStatus::Default);
        assert_eq!(v.current.sent, None);
    }
}
